// Tauri commands · 前端可调用的 Rust 函数入口

use crate::clipboard::CaptureState;
use crate::export::{self, ExportFormat, ExportResult};
use crate::settings_keys;
use crate::storage::{self, repository};
use crate::window::platform::{self, PrevForeground};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tauri::State;

/// Tauri 管理的全局状态
pub struct AppState {
    pub db: Arc<storage::AppDatabase>,
    pub capture: Arc<CaptureState>,
    /// 唤起快捷键触发时抓取的前景窗口句柄（供 paste_to_previous 还原焦点）
    pub prev_foreground: PrevForeground,
    /// Tray 暂停菜单项 handle — 用于动态更新文字（暂停记录 ⇄ 继续记录）。
    /// setup_tray 注入，pause/resume 后调 sync_tray_pause_text 同步显示。
    pub tray_pause_item: std::sync::Mutex<Option<tauri::menu::MenuItem<tauri::Wry>>>,
}

impl AppState {
    /// 根据当前暂停状态刷新 tray 菜单项文字
    pub fn sync_tray_pause_text(&self) {
        let paused = self.capture.is_paused();
        if let Ok(guard) = self.tray_pause_item.lock() {
            if let Some(item) = guard.as_ref() {
                let _ = item.set_text(if paused { "继续记录" } else { "暂停记录" });
            }
        }
    }
}

// ── 搜索 ──

#[tauri::command]
pub fn search_clipboard(
    state: State<'_, AppState>,
    query: String,
    limit: i64,
) -> Result<Vec<repository::ClipboardRow>, String> {
    let conn = state.db.conn();
    repository::search_clipboard(&conn, &query, limit).map_err(|e| e.to_string())
}

// ── 列表 ──

#[tauri::command]
pub fn list_recent_clipboard(
    state: State<'_, AppState>,
    limit: i64,
    offset: i64,
) -> Result<Vec<repository::ClipboardRow>, String> {
    let conn = state.db.conn();
    repository::list_recent(&conn, limit, offset).map_err(|e| e.to_string())
}

// ── 详情 ──

#[tauri::command]
pub fn get_clipboard_detail(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<repository::ClipboardRow>, String> {
    let conn = state.db.conn();
    repository::get_detail(&conn, &id).map_err(|e| e.to_string())
}

// ── 今日统计 ──

#[tauri::command]
pub fn get_today_stats(
    state: State<'_, AppState>,
) -> Result<repository::TodayStats, String> {
    let conn = state.db.conn();
    repository::get_today_stats(&conn).map_err(|e| e.to_string())
}

// ── 图片缩略图（data URL）──

/// 读取 image 类型记录的 PNG 文件，返回 `data:image/png;base64,...` URL，
/// 供快速面板 <img> 标签直接渲染。
///
/// 参数 `max_size`（可选）：
/// - `Some(n)`：缩放到最大 n×n（保持宽高比）再编码 PNG，用于**列表缩略图**。
///   1920×1080 原图 ~1MB → 128×128 缩略 ~5KB，20 条 image 列表从 ~20MB 降到
///   ~100KB，IPC 传输 + 前端渲染 O(10ms) 对比 O(数百ms)。
/// - `None`：原图 base64，用于 PreviewOverlay 全尺寸预览。
///
/// 为什么不用 asset protocol：tauri 2.x asset 需要 capabilities fs:allow-read +
/// scope 配置 data_dir 路径，而 data_dir 是 runtime 决定的动态路径。base64 方案
/// 零权限配置。
#[tauri::command]
pub fn get_image_data_url(
    state: State<'_, AppState>,
    id: String,
    max_size: Option<u32>,
) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};

    let filename = {
        let conn = state.db.conn();
        let row = repository::get_detail(&conn, &id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("row not found: {id}"))?;
        row.image_path
            .ok_or_else(|| format!("row {id} has no image_path"))?
    };
    let path = state.db.images_dir().join(&filename);
    let bytes = std::fs::read(&path)
        .map_err(|e| format!("read {}: {}", path.display(), e))?;

    // 原图模式：直接 base64（PreviewOverlay 走这条）
    let Some(max) = max_size else {
        return Ok(format!("data:image/png;base64,{}", STANDARD.encode(&bytes)));
    };

    // 缩略图模式：decode → thumbnail → re-encode PNG（CardItem 列表走这条）
    let img = image::load_from_memory(&bytes)
        .map_err(|e| format!("decode image failed: {e}"))?;
    // thumbnail 保持宽高比，输出宽高 ≤ max×max（image crate 用 lanczos3 算法质量高）
    let thumb = img.thumbnail(max, max);
    let mut out: Vec<u8> = Vec::new();
    thumb
        .write_to(
            &mut std::io::Cursor::new(&mut out),
            image::ImageFormat::Png,
        )
        .map_err(|e| format!("encode thumbnail failed: {e}"))?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(&out)))
}

// ── 图片复制到剪切板 ──

/// 把数据库中某条 image 记录的 PNG 文件还原为 RGBA 写入系统剪切板。
///
/// 流程：查 row → 读 images_dir/image_path.png → decode → arboard set_image。
/// 之后上层配合 `paste_to_previous` 即可完成图片粘贴。
#[tauri::command]
pub fn copy_image_to_clipboard(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let image_path = {
        let conn = state.db.conn();
        let row = repository::get_detail(&conn, &id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("clipboard entry not found: {id}"))?;
        row.image_path
            .ok_or_else(|| format!("row {id} has no image_path"))?
    };
    let full_path = state.db.images_dir().join(&image_path);

    let img = image::ImageReader::open(&full_path)
        .map_err(|e| format!("open image failed: {e}"))?
        .decode()
        .map_err(|e| format!("decode image failed: {e}"))?
        .to_rgba8();
    let (w, h) = img.dimensions();
    let bytes = img.into_raw();

    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| format!("clipboard init failed: {e}"))?;
    clipboard
        .set_image(arboard::ImageData {
            width: w as usize,
            height: h as usize,
            bytes: std::borrow::Cow::Owned(bytes),
        })
        .map_err(|e| format!("set image failed: {e}"))?;

    Ok(())
}

// ── 系统粘贴（Windows 模拟 Ctrl+V）──

/// 粘贴到唤起 panel 之前的前景窗口。
///
/// 前端流程：
/// 1. `writeText(text)` 写剪切板
/// 2. `getCurrentWebviewWindow().hide()` 隐藏 panel
/// 3. `invoke('paste_to_previous')`
///
/// 本 command 在后端：
/// - 取出 `prev_foreground`（take，一次性消耗）
/// - 等 80ms 让系统焦点回落
/// - 激活前景窗口 + 模拟 Ctrl+V
///
/// 非 Windows 平台返回 Err，前端据此静默回退（已经 writeText + hide，不做 Ctrl+V）。
#[tauri::command]
pub async fn paste_to_previous(state: State<'_, AppState>) -> Result<(), String> {
    let handle = {
        let mut guard = state
            .prev_foreground
            .lock()
            .map_err(|e| format!("lock poisoned: {e}"))?;
        guard.take()
    };

    #[cfg(target_os = "windows")]
    {
        tokio::time::sleep(Duration::from_millis(80)).await;
        platform::activate_and_paste(handle)?;
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = handle;
        Err("system paste not implemented on this platform".to_string())
    }
}

// ── 置顶 / 取消置顶 ──

/// 切换置顶状态。返回新的 pinned_at（null = 已取消；数字 = 置顶时间戳 ms）。
/// 面板列表会自动按 pinned_at DESC 排序把置顶项聚集到顶部。
#[tauri::command]
pub fn toggle_pin(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<i64>, String> {
    let conn = state.db.conn();
    repository::toggle_pin(&conn, &id).map_err(|e| e.to_string())
}

// ── L1 聚合 tab 数据源 ──

#[tauri::command]
pub fn list_sessions(
    state: State<'_, AppState>,
    limit: i64,
) -> Result<Vec<repository::SessionSummary>, String> {
    let conn = state.db.conn();
    repository::list_sessions(&conn, limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_session_items(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Vec<repository::ClipboardRow>, String> {
    let conn = state.db.conn();
    repository::list_session_items(&conn, &session_id).map_err(|e| e.to_string())
}

// ── TextView 云端认证（R3.1）──
// 关键：async 命令不持 Connection 跨 await（Connection 不 Send）。
// 架构：HTTP 请求 async → 拿结果 → 短 scope 内同步写 DB + keyring

#[tauri::command]
pub async fn auth_send_otp(email: String) -> Result<(), String> {
    crate::auth::send_otp_http(&email).await
}

#[tauri::command]
pub async fn auth_verify_otp(
    state: State<'_, AppState>,
    email: String,
    code: String,
) -> Result<crate::auth::AuthUser, String> {
    // 1. 纯网络请求（async，不碰 DB）
    let resp = crate::auth::verify_otp_http(&email, &code).await?;

    // 2. 持久化（sync scope，conn 不跨 await）
    let user = resp.user.clone();
    crate::auth::save_refresh_token(&resp.refresh_token)?;
    crate::auth::set_access_token(resp.access_token);
    {
        let conn = state.db.conn();
        crate::auth::save_user_sync(&conn, &user).map_err(|e| format!("save user: {e}"))?;
    }
    Ok(user)
}

#[tauri::command]
pub async fn auth_logout(state: State<'_, AppState>) -> Result<(), String> {
    // 纯 sync 操作，不涉及 await
    crate::auth::delete_refresh_token().ok();
    crate::auth::clear_access_token();
    {
        let conn = state.db.conn();
        crate::auth::clear_user_sync(&conn).map_err(|e| format!("clear user: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn auth_state(state: State<'_, AppState>) -> crate::auth::AuthState {
    let conn = state.db.conn();
    crate::auth::current_auth_state(&conn)
}

// ── 数据导入（从 Teamo 自己的 JSON 导出恢复）──

#[tauri::command]
pub fn import_data(
    state: State<'_, AppState>,
    source_dir: String,
) -> Result<crate::export::import::ImportResult, String> {
    let conn = state.db.conn();
    let dest_images = state.db.images_dir();
    crate::export::import::import_from_dir(&conn, std::path::Path::new(&source_dir), &dest_images)
}

// ── 标记使用（粘贴后 promote 链路）──

/// 前端 copyToClipboard 成功后调用 → 更新 last_used_at → 列表重排把该项顶到前面。
/// "复制 A → 粘 B → 想再粘 A" 这种高频流不用重搜即可找到 A。
#[tauri::command]
pub fn mark_used(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let conn = state.db.conn();
    repository::mark_used(&conn, &id).map_err(|e| e.to_string())
}

// ── 忘记 ──

#[tauri::command]
pub fn forget_clipboard(
    state: State<'_, AppState>,
    id: String,
) -> Result<bool, String> {
    let conn = state.db.conn();
    let images_dir = state.db.images_dir();
    repository::forget(&conn, &id, &images_dir).map_err(|e| e.to_string())
}

// ── 暂停 / 恢复捕获 ──

/// pause_capture 的核心逻辑（同步函数，无 Result），供 Tauri command 和 tray menu handler 共享。
pub fn do_pause_capture(state: &AppState, minutes: Option<u64>) {
    let duration = minutes.map(|m| Duration::from_secs(m * 60));
    state.capture.pause(duration);

    let conn = state.db.conn();
    if let Some(mins) = minutes {
        let until = chrono_now_ms() + (mins as i64 * 60 * 1000);
        let _ = repository::set_setting(
            &conn,
            settings_keys::CAPTURE_PAUSED_UNTIL,
            Some(&until.to_string()),
        );
    } else {
        let _ = repository::set_setting(
            &conn,
            settings_keys::CAPTURE_PAUSED_UNTIL,
            Some("manual"),
        );
    }

    tracing::info!("Capture paused for {:?} minutes", minutes);
    state.sync_tray_pause_text();
}

/// resume_capture 的核心逻辑，供 Tauri command 和 tray menu handler 共享。
pub fn do_resume_capture(state: &AppState) {
    state.capture.resume();
    let conn = state.db.conn();
    let _ = repository::set_setting(&conn, settings_keys::CAPTURE_PAUSED_UNTIL, None);
    tracing::info!("Capture resumed");
    state.sync_tray_pause_text();
}

#[tauri::command]
pub fn pause_capture(
    state: State<'_, AppState>,
    minutes: Option<u64>,
) -> Result<(), String> {
    do_pause_capture(&state, minutes);
    Ok(())
}

#[tauri::command]
pub fn resume_capture(state: State<'_, AppState>) -> Result<(), String> {
    do_resume_capture(&state);
    Ok(())
}

#[tauri::command]
pub fn is_capture_paused(
    state: State<'_, AppState>,
) -> bool {
    state.capture.is_paused()
}

/// Capture loop 健康状态：`(last_heartbeat_ms, seconds_since_last_beat)`
/// - 正常运行时 seconds_since_last_beat < 2（capture loop 500ms 一圈）
/// - > 10 秒：capture 可能死了（panic 太频繁 supervisor 撑不回来，或被暂停）
/// - 暂停期间 heartbeat 仍会跳（暂停只是 skip 写入，不 skip 心跳）
///
/// Phase 1 后端就位，Tray UI 显示 "Capture: Dead" 逻辑留 Phase 2
#[tauri::command]
pub fn get_capture_health(state: State<'_, AppState>) -> (i64, i64) {
    let last = state
        .capture
        .last_heartbeat_ms
        .load(std::sync::atomic::Ordering::Relaxed);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let seconds_since = if last == 0 { -1 } else { (now - last) / 1000 };
    (last, seconds_since)
}

// ── App 黑白名单（filter-engine 的 app_rules） ──

#[tauri::command]
pub fn list_app_rules(
    state: State<'_, AppState>,
) -> Result<Vec<repository::AppRule>, String> {
    let conn = state.db.conn();
    repository::list_app_rules(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_app_rule(
    state: State<'_, AppState>,
    app_identifier: String,
    rule_type: String,
) -> Result<i64, String> {
    let conn = state.db.conn();
    repository::add_app_rule(&conn, &app_identifier, &rule_type).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_app_rule(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    let conn = state.db.conn();
    repository::remove_app_rule(&conn, id).map_err(|e| e.to_string())
}

/// 抓当前前景 App 名（Windows 实现）。UI 用它做"添加当前 App 到黑/白名单"快捷流程。
#[tauri::command]
pub fn get_current_foreground_app() -> Option<String> {
    crate::window::platform::capture_foreground_app_name()
}

// ── 数据管理（settings-page 数据区） ──

/// 本地数据信息：路径 + DB 文件大小 + 图片数量
#[derive(Debug, serde::Serialize)]
pub struct DataInfo {
    pub data_dir: String,
    pub db_path: String,
    pub db_bytes: u64,
    pub image_count: u64,
    pub image_bytes: u64,
}

#[tauri::command]
pub fn get_data_info(state: State<'_, AppState>) -> Result<DataInfo, String> {
    let db_path = state.db.images_dir().parent()
        .map(|p| p.join("clipboard.db"))
        .ok_or_else(|| "cannot resolve db path".to_string())?;
    let db_bytes = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    let images_dir = state.db.images_dir();
    let (image_count, image_bytes) = match std::fs::read_dir(&images_dir) {
        Ok(iter) => {
            let mut count = 0u64;
            let mut bytes = 0u64;
            for entry in iter.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        count += 1;
                        bytes += meta.len();
                    }
                }
            }
            (count, bytes)
        }
        Err(_) => (0, 0),
    };

    let data_dir = images_dir
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    Ok(DataInfo {
        data_dir,
        db_path: db_path.to_string_lossy().to_string(),
        db_bytes,
        image_count,
        image_bytes,
    })
}

/// 清空全部本地数据：删除 clipboard_local 所有行 + 删除 images/ 下全部文件
///
/// 危险操作！前端必须做二次确认后才调用。完成后 emit `data:cleared` event，
/// 让 panel（如果正开着）立即刷新列表——避免"清完数据 panel 里还显示旧条目"的
/// UX 不一致。
#[tauri::command]
pub fn clear_all_data(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    {
        let conn = state.db.conn();
        conn.execute("DELETE FROM clipboard_local", [])
            .map_err(|e| format!("delete rows: {e}"))?;
        // clipboard_fts 会通过触发器自动同步；其他表（settings）保留
    }

    let images_dir = state.db.images_dir();
    if images_dir.exists() {
        for entry in std::fs::read_dir(&images_dir)
            .map_err(|e| format!("read images dir: {e}"))?
            .flatten()
        {
            if entry.metadata().map(|m| m.is_file()).unwrap_or(false) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    use tauri::Emitter;
    if let Err(e) = app.emit("data:cleared", ()) {
        tracing::warn!("failed to emit data:cleared event: {e}");
    }

    tracing::info!("All clipboard data cleared");
    Ok(())
}

// ── 数据导出 ──

/// 导出 clipboard_local 全部数据 + 图片到 target_dir/teamo-export-YYYYMMDD-HHMMSS/
///
/// Phase 1 同步实现（可能阻塞 tauri async worker 几秒，10w 条数据量级）。
/// Phase 2 改 tokio::task::spawn_blocking + 进度 event。
#[tauri::command]
pub fn export_data(
    state: State<'_, AppState>,
    format: ExportFormat,
    target_dir: String,
) -> Result<ExportResult, String> {
    let target = PathBuf::from(target_dir);
    export::export_data(&state.db, format, &target)
}

// ── 设置 ──

#[tauri::command]
pub fn get_setting(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<String>, String> {
    let conn = state.db.conn();
    repository::get_setting(&conn, &key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_setting(
    state: State<'_, AppState>,
    key: String,
    value: Option<String>,
) -> Result<(), String> {
    let conn = state.db.conn();
    repository::set_setting(&conn, &key, value.as_deref()).map_err(|e| e.to_string())?;
    // 任何 setting 改动都 invalidate filter cache（sens.* / filter.min_text_len
    // 等可能影响 apply_filters 行为；其他 key 改动也 invalidate 无伤大雅）
    crate::filter::cache::invalidate();
    Ok(())
}

fn chrono_now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
