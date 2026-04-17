// clipboard/ · 剪切板监听与读取
//
// 职责：
// 1. 事件式监听剪切板变化（clipboard-master）
// 2. 读取内容（arboard）
// 3. 分类型处理（text / image / file / html）
// 4. 交给 storage 层持久化

use crate::storage::{self, repository};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 剪切板捕获器状态
pub struct CaptureState {
    /// 是否暂停
    pub paused: AtomicBool,
    /// 暂停到期时间（None = 未暂停 / 手动暂停需手动恢复）
    pub paused_until: std::sync::Mutex<Option<Instant>>,
    /// 心跳：capture loop 每次迭代前写入当前 Unix ms 时间戳。
    /// Tray/health command 可读此值判断"capture 是否死了"（> 10s 无心跳 → dead）。
    /// 初始 0 = 还没启动第一圈。
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

/// 在后台线程启动剪切板监听循环
///
/// 使用 tauri-plugin-clipboard-manager 的 API 通过定时轮询检测变化。
/// clipboard-master crate 在 Windows/macOS 都有平台原生 listener，但
/// 与 Tauri 2.x 集成有 threading 问题，所以 v0.1 用轻量轮询（500ms 间隔）。
///
/// **Supervisor 结构**：外层 `catch_unwind` 包裹内层真正的 capture loop。
/// 任何 panic（arboard OOM / rusqlite Mutex poison / image decode assert 等）
/// 都被捕获后 sleep 1s 重启内层，避免 capture 线程静默死亡而用户不知道
/// "剪切板已经好几天没记录"。`CaptureState::last_heartbeat_ms` 每次迭代
/// 写入时间戳，未来 Tray UI 可用它显示 "Capture: Running / Dead" 状态。
pub fn start_capture_loop(
    db: Arc<storage::AppDatabase>,
    capture_state: Arc<CaptureState>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || loop {
        let db_inner = Arc::clone(&db);
        let state_inner = Arc::clone(&capture_state);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            capture_loop_inner(db_inner, state_inner);
        }));
        match result {
            Ok(()) => {
                tracing::info!("Clipboard capture loop exited normally");
                return;
            }
            Err(panic_info) => {
                tracing::error!(
                    "Clipboard capture loop panicked: {panic_info:?} — restarting in 1s"
                );
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    })
}

fn capture_loop_inner(
    db: Arc<storage::AppDatabase>,
    capture_state: Arc<CaptureState>,
) {
    tracing::info!("Clipboard capture loop started");

    let mut last_text_hash: Option<String> = None;
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to initialize clipboard: {e}");
            return;
        }
    };

    loop {
        std::thread::sleep(Duration::from_millis(500));

        // 心跳：每次迭代前记录时间戳，供 health check 判断线程是否活
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        capture_state
            .last_heartbeat_ms
            .store(now_ms, Ordering::Relaxed);

            // 检查暂停。is_paused() 内部若检测到定时暂停过期会自动 resume 内存态，
            // 这里在 resume 发生时同步把 DB 里的 capture.paused_until 也清掉，
            // 避免"内存是未暂停 / DB 还记着过期时间"两源不一致。
            let was_paused = capture_state
                .paused
                .load(std::sync::atomic::Ordering::Relaxed);
            if capture_state.is_paused() {
                continue;
            }
            if was_paused {
                // 刚刚从"暂停过期"自动恢复过来 → 清 DB
                let conn = db.conn();
                let _ = repository::set_setting(
                    &conn,
                    crate::settings_keys::CAPTURE_PAUSED_UNTIL,
                    None,
                );
            }

            // 读文本
            if let Ok(text) = clipboard.get_text() {
                if text.is_empty() {
                    continue;
                }
                let text_hash = format!("{:x}", {
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};
                    let mut h = DefaultHasher::new();
                    text.hash(&mut h);
                    h.finish()
                });

                if last_text_hash.as_deref() == Some(&text_hash) {
                    continue; // 没变化
                }
                last_text_hash = Some(text_hash);

                // 抓当前前景 App（Windows 实现；macOS/Linux Phase 4）
                // 必须在 filter + insert 前抓，因为 arboard get_text 本身可能唤起其他窗口
                // （实际不会，但流程上抓在最早）
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
            }

            // 读图片
            if let Ok(image) = clipboard.get_image() {
                let pixels = image.bytes.as_ref();
                if pixels.is_empty() {
                    continue;
                }

                // 内存变化检测 hash —— 必须用全量 pixel 指纹防误判。
                //
                // 历史 bug：早期用首 4096 字节 DefaultHasher（outside voice review 发现）。
                // 2560×1440 RGBA 截图约 14 MB，两张"顶栏都白 + 不同主体"的截图前 4KB
                // 常常 bit-identical → 第二张被整个 continue 跳过，PNG 不写、DB 不插，
                // 截图静默丢失。下游 repository::insert_clipboard 里的 `canonicalize + sha256`
                // 是对 content 文本做的，救不了图片（图片的 content 是我们构造的 fingerprint 而非 pixel）。
                //
                // 修复：这里直接用全量 sha256。Rust sha256_hex 在现代机器上对 14MB
                // 约 30-50ms，capture loop 500ms 轮询完全可吸收。与下面的 pixel_fingerprint
                // 是同一个值（复用），也省一次重复 hash 运算。
                let img_hash = repository::sha256_hex(pixels);

                // 图片去重靠 hash 比较（和文本共用 last_text_hash 足够，因为同一时刻剪切板只有一种类型）
                if last_text_hash.as_deref() == Some(&img_hash) {
                    continue;
                }
                last_text_hash = Some(img_hash.clone());

                // 保存图片到文件（真 PNG 编码，自带宽高，Phase 3C 粘贴时可还原）
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
                    continue;
                }

                // image 的 dedup 指纹：复用上方 img_hash（同为全量 pixel SHA256），
                // 避免对 14MB 截图算 2 次 hash（大图下 30-50ms × 2 = 60-100ms 纯浪费）。
                // 解决 bug：之前 content=None → canonicalize("") → sha256("")，所有图片共享同一 hash 误判重复
                let pixel_fingerprint = img_hash.clone();

                // 图片不扫内容；但 App 黑白名单对图片也生效（比如 1Password 截屏 → 拦）。
                // 走 filter::check_app_rules 共用函数，保持和文本分支对称。
                let source_app = crate::window::platform::capture_foreground_app_name();
                let (state, blocked_reason) = {
                    let conn = db.conn();
                    match crate::filter::check_app_rules(&conn, source_app.as_deref()) {
                        Some(decision) if decision.state == "local_only" => {
                            (Some(decision.state), decision.blocked_reason)
                        }
                        // 白名单（captured）或无命中：都走默认，不写 state/blocked_reason
                        _ => (None, None),
                    }
                };

                let req = repository::InsertRequest {
                    id,
                    content: Some(pixel_fingerprint),
                    content_type: "image".to_string(),
                    image_path: Some(filename),
                    file_path: None,
                    source_app,
                    state,
                    blocked_reason,
                    sensitive_type: None,
                    matched_domain_rule: None, // 图片无 URL，此字段永远 None
                };

                let conn = db.conn();
                match repository::insert_clipboard(&conn, req) {
                    Ok(_) => tracing::debug!("Captured image clipboard entry"),
                    Err(e) => tracing::error!("Failed to insert image clipboard: {e}"),
                }
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
