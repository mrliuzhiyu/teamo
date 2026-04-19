// clipboard/ · 剪切板监听与读取（事件驱动）
//
// v0.2 重构：500ms 轮询 → 事件驱动（对标 Ditto / CopyQ / Maccy）
//
// 架构：
//   Windows：AddClipboardFormatListener → WM_CLIPBOARDUPDATE → mpsc::send(())
//   macOS：   Phase 4 改 NSPasteboard KVO（暂不支持，compile-gated）
//
//   消费线程从 channel 接 () → ingest_once（读 text/image → 闸门 → 去重 → DB）
//
// 收益：CPU idle 时真 idle；响应 <10ms；快速连续复制不再被 500ms 窗口漏记

use crate::storage::{self, repository};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
mod windows_listener;

#[cfg(target_os = "windows")]
mod hidden_formats;

#[cfg(not(target_os = "windows"))]
compile_error!(
    "Teamo v0.2 只支持 Windows（事件驱动剪贴板监听）。macOS Phase 4 用 NSPasteboard KVO 补齐。"
);

/// 剪切板捕获器状态
pub struct CaptureState {
    /// 是否暂停
    pub paused: AtomicBool,
    /// 暂停到期时间（None = 未暂停 / 手动暂停需手动恢复）
    pub paused_until: std::sync::Mutex<Option<Instant>>,
    /// 最后一次处理 ingest 事件的时间戳（Unix ms）。
    /// 事件驱动下 idle 是常态，此字段仅用于"最后活动时间"展示，不是 liveness 判据
    pub last_heartbeat_ms: AtomicI64,
}

impl CaptureState {
    pub fn new() -> Self {
        Self {
            paused: AtomicBool::new(false),
            paused_until: std::sync::Mutex::new(None),
            last_heartbeat_ms: AtomicI64::new(0),
        }
    }

    pub fn is_paused(&self) -> bool {
        if !self.paused.load(Ordering::Relaxed) {
            return false;
        }
        // 检查定时暂停是否到期
        if let Ok(guard) = self.paused_until.lock() {
            if let Some(until) = *guard {
                if Instant::now() >= until {
                    drop(guard);
                    self.resume();
                    return false;
                }
            }
        }
        true
    }

    pub fn pause(&self, duration: Option<Duration>) {
        self.paused.store(true, Ordering::Relaxed);
        if let Ok(mut guard) = self.paused_until.lock() {
            *guard = duration.map(|d| Instant::now() + d);
        }
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
        if let Ok(mut guard) = self.paused_until.lock() {
            *guard = None;
        }
    }
}

/// 启动事件驱动的剪切板捕获子系统。
///
/// 创建 2 个线程：
/// 1. Listener thread（`windows_listener::start`）：跑 Windows message loop，
///    收到 WM_CLIPBOARDUPDATE 后 `tx.send(())` 通知消费端
/// 2. Consumer thread：阻塞 `rx.recv()` 等事件，收到后 `ingest_once` 处理
///
/// 为什么分两个线程而不是直接在 message loop 里做 ingest：
/// - message loop 必须保持 responsive，ingest 里有 DB 写入、图片 PNG 编码、
///   filter 正则匹配（重度操作）。堵住 message loop 会让 Windows 误判窗口死亡
/// - mpsc 天然限流：并发事件排队处理，不会同时跑多份 ingest
///
/// 消费线程有 panic 自愈包装：panic 后 sleep 1s 重新 init arboard 继续 recv。
/// Listener thread 若失败（CreateWindow / AddListener）log 后退出，整个捕获子系统 offline。
pub fn start_capture(db: Arc<storage::AppDatabase>, capture_state: Arc<CaptureState>) {
    let (tx, rx) = mpsc::channel::<()>();

    // 启动 Windows 监听线程
    windows_listener::start(tx.clone());

    // 启动时主动触发一次 ingest —— 否则 app 启动前已 copy 的内容会被漏掉
    // （AddClipboardFormatListener 只对后续变化触发）
    let _ = tx.send(());
    // tx 在此 drop 也无妨：windows_listener 已把 clone 的 tx 存进全局静态

    // 消费线程：从 channel 接事件 → ingest
    std::thread::Builder::new()
        .name("teamo-clipboard-consumer".to_string())
        .spawn(move || {
            consume_loop(db, capture_state, rx);
        })
        .expect("failed to spawn clipboard consumer thread");
}

/// 消费线程主体 + panic 自愈外层
fn consume_loop(
    db: Arc<storage::AppDatabase>,
    capture_state: Arc<CaptureState>,
    rx: mpsc::Receiver<()>,
) {
    tracing::info!("Clipboard consumer thread started");

    // 单次消费循环 — panic 后整段重启（重新 init arboard）
    loop {
        let db_inner = Arc::clone(&db);
        let state_inner = Arc::clone(&capture_state);
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            run_consumer(&db_inner, &state_inner, &rx);
        }));
        match result {
            Ok(()) => {
                tracing::info!("Clipboard consumer exited normally (channel closed)");
                return;
            }
            Err(panic_info) => {
                tracing::error!("Clipboard consumer panicked: {panic_info:?} — restarting in 1s");
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

fn run_consumer(
    db: &Arc<storage::AppDatabase>,
    capture_state: &Arc<CaptureState>,
    rx: &mpsc::Receiver<()>,
) {
    let mut last_hash: Option<String> = None;
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to initialize arboard clipboard: {e}");
            return;
        }
    };

    while rx.recv().is_ok() {
        // 记录"最后活动时间"（不是 liveness，仅展示用）
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        capture_state.last_heartbeat_ms.store(now_ms, Ordering::Relaxed);

        // 暂停检查（is_paused 内部会自动 resume 过期的定时暂停）
        let was_paused = capture_state.paused.load(Ordering::Relaxed);
        if capture_state.is_paused() {
            continue;
        }
        if was_paused {
            // 刚刚从"暂停过期"自动恢复 → 清 DB 里的 CAPTURE_PAUSED_UNTIL
            let conn = db.conn();
            let _ = repository::set_setting(
                &conn,
                crate::settings_keys::CAPTURE_PAUSED_UNTIL,
                None,
            );
        }

        ingest_once(db, &mut clipboard, &mut last_hash);
    }
}

/// 单次 ingest：读 text / image → 闸门过滤 → 去重 → 写 DB
///
/// 对应原 capture_loop_inner 的单圈 body（不含 sleep）。
/// `last_hash` 跨调用保留避免重复 ingest 同一内容（比如监听器偶尔重复 fire）。
fn ingest_once(
    db: &Arc<storage::AppDatabase>,
    clipboard: &mut arboard::Clipboard,
    last_hash: &mut Option<String>,
) {
    // OS 级"请忽略我"检查 —— 密码管理器 / 银行 App 设置的官方 MIME 标记
    // （Clipboard Viewer Ignore / ExcludeClipboardContentFromMonitorProcessing 等）。
    // 对标 CopyQ isHidden()；Windows 原生 Clipboard History 也按此语义过滤。
    // 这道比 filter-engine 的 sensitive 正则更权威（密码管理器自己声明"别记我"）。
    if hidden_formats::is_clipboard_hidden() {
        tracing::debug!("Clipboard content marked as hidden by source app — skipping");
        return;
    }

    // 读文本（非空才处理；空文本可能是"截图附带空 CF_UNICODETEXT" 的情况，需 fall-through 到图片分支）
    if let Ok(text) = clipboard.get_text() {
        if !text.is_empty() {
            // 用 SHA256 和图片分支对称，避免 DefaultHasher 的两个问题：
            // 1) 种子随机，跨进程重启 hash 值变（虽然 last_hash 是 in-memory 不跨进程，
            //    但统一哈希算法减少心智负担）
            // 2) SipHash 不是密码学 hash，极低概率碰撞；SHA256 碰撞实际不可能
            let text_hash = repository::sha256_hex(text.as_bytes());

            if last_hash.as_deref() == Some(&text_hash) {
                return; // 没变化
            }
            *last_hash = Some(text_hash);

        // 抓当前前景 App（Windows 实现；macOS/Linux Phase 4）
        let source_app = crate::window::platform::capture_foreground_app_name();

        // 闸门：App 黑白名单 + 敏感数据 → state=local_only，其余 captured
        let decision = {
            let conn = db.conn();
            crate::filter::apply_filters(&conn, &text, source_app.as_deref())
        };
        let id = generate_id();
        let req = repository::InsertRequest {
            id,
            content: Some(text),
            content_type: "text".to_string(),
            image_path: None,
            file_path: None,
            source_app: source_app.clone(),
            state: Some(decision.state),
            blocked_reason: decision.blocked_reason,
            sensitive_type: decision.sensitive_type,
            matched_domain_rule: decision.matched_domain_rule,
        };

            let conn = db.conn();
            match repository::insert_clipboard(&conn, req) {
                Ok(repository::InsertResult::Inserted) => {
                    tracing::debug!("Captured new text clipboard entry");
                }
                Ok(repository::InsertResult::Deduplicated { existing_id }) => {
                    tracing::debug!("Deduplicated with {existing_id}");
                }
                Err(e) => {
                    tracing::error!("Failed to insert clipboard: {e}");
                }
            }
            return;
        }
        // 空文本落下去尝试图片（修复 bug：截图 App 同时放空 CF_UNICODETEXT + CF_BITMAP 时原来图片丢失）
    }

    // 读图片
    if let Ok(image) = clipboard.get_image() {
        let pixels = image.bytes.as_ref();
        if pixels.is_empty() {
            return;
        }

        // 大图阈值保护：避免超大截图（多屏 8K、PhotoShop 导出原图等边界）
        // 阻塞消费线程 100+ ms 做 SHA256 + PNG 编码，甚至触发 OOM。
        // 50 MB RGBA 约 3.6K×3.6K，实际 99.9% 截图远低于此。超限时 log warn 跳过，
        // 用户可以手动保存到文件再复制文件路径（走 content_type=file 分支未实现，留 Phase 2）
        const MAX_IMAGE_BYTES: usize = 50 * 1024 * 1024;
        if pixels.len() > MAX_IMAGE_BYTES {
            tracing::warn!(
                "Image too large ({} MB > {} MB limit) — skipping to avoid blocking / OOM",
                pixels.len() / (1024 * 1024),
                MAX_IMAGE_BYTES / (1024 * 1024),
            );
            return;
        }

        // 全量 pixel sha256 做 dedup 指纹（见原注释：首 4KB hash 会误判截图静默丢失）
        let img_hash = repository::sha256_hex(pixels);

        if last_hash.as_deref() == Some(&img_hash) {
            return;
        }
        *last_hash = Some(img_hash.clone());

        let id = generate_id();
        let filename = format!("{}.png", &id);
        let img_path = db.images_dir().join(&filename);

        if let Err(e) = image::save_buffer_with_format(
            &img_path,
            pixels,
            image.width as u32,
            image.height as u32,
            image::ExtendedColorType::Rgba8,
            image::ImageFormat::Png,
        ) {
            tracing::error!("Failed to encode image as PNG: {e}");
            return;
        }

        // 图片 App 黑白名单走 filter::check_app_rules 保持和文本对称
        let source_app = crate::window::platform::capture_foreground_app_name();
        let (state, blocked_reason) = {
            let conn = db.conn();
            match crate::filter::check_app_rules(&conn, source_app.as_deref()) {
                Some(decision) if decision.state == "local_only" => {
                    (Some(decision.state), decision.blocked_reason)
                }
                _ => (None, None),
            }
        };

        let req = repository::InsertRequest {
            id,
            content: Some(img_hash),
            content_type: "image".to_string(),
            image_path: Some(filename),
            file_path: None,
            source_app,
            state,
            blocked_reason,
            sensitive_type: None,
            matched_domain_rule: None,
        };

        let conn = db.conn();
        match repository::insert_clipboard(&conn, req) {
            Ok(_) => tracing::debug!("Captured image clipboard entry"),
            Err(e) => tracing::error!("Failed to insert image clipboard: {e}"),
        }
    }
}

/// 生成简单 UUID（v4 格式，不引 uuid crate）
fn generate_id() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    let mut hasher = DefaultHasher::new();
    now.hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);
    let h1 = hasher.finish();

    let mut hasher2 = DefaultHasher::new();
    (now + 1).hash(&mut hasher2);
    let h2 = hasher2.finish();

    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (h1 >> 32) as u32,
        (h1 >> 16) as u16,
        (h1 & 0x0FFF) as u16,
        ((h2 >> 48) as u16 & 0x3FFF) | 0x8000,
        h2 & 0xFFFF_FFFF_FFFF,
    )
}
