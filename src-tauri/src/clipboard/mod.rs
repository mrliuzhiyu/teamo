// clipboard/ · 剪切板监听与读取
//
// 职责：
// 1. 事件式监听剪切板变化（clipboard-master）
// 2. 读取内容（arboard）
// 3. 分类型处理（text / image / file / html）
// 4. 交给 storage 层持久化

use crate::storage::{self, repository};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 剪切板捕获器状态
pub struct CaptureState {
    /// 是否暂停
    pub paused: AtomicBool,
    /// 暂停到期时间（None = 未暂停 / 手动暂停需手动恢复）
    pub paused_until: std::sync::Mutex<Option<Instant>>,
}

impl CaptureState {
    pub fn new() -> Self {
        Self {
            paused: AtomicBool::new(false),
            paused_until: std::sync::Mutex::new(None),
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
pub fn start_capture_loop(
    db: Arc<storage::AppDatabase>,
    capture_state: Arc<CaptureState>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
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

            if capture_state.is_paused() {
                continue;
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

                let id = generate_id();
                let req = repository::InsertRequest {
                    id,
                    content: Some(text),
                    content_type: "text".to_string(),
                    image_path: None,
                    file_path: None,
                    source_app: None, // TODO: 后续通过平台 API 获取前台应用名
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

                // 用像素数据的简单哈希做变化检测
                let img_hash = {
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};
                    let mut h = DefaultHasher::new();
                    // 只取前 4096 字节做快速哈希（避免大图慢）
                    let sample = &pixels[..pixels.len().min(4096)];
                    sample.hash(&mut h);
                    format!("{:x}", h.finish())
                };

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

                // image 的 dedup 指纹：用像素 SHA256 作为 content 字符串
                // 解决 bug：之前 content=None → canonicalize("") → sha256("")，所有图片共享同一 hash 误判重复
                let pixel_fingerprint = repository::sha256_hex(pixels);

                let req = repository::InsertRequest {
                    id,
                    content: Some(pixel_fingerprint),
                    content_type: "image".to_string(),
                    image_path: Some(filename),
                    file_path: None,
                    source_app: None,
                };

                let conn = db.conn();
                match repository::insert_clipboard(&conn, req) {
                    Ok(_) => tracing::debug!("Captured image clipboard entry"),
                    Err(e) => tracing::error!("Failed to insert image clipboard: {e}"),
                }
            }
        }
    })
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
