// Tauri commands · 前端可调用的 Rust 函数入口

use crate::clipboard::CaptureState;
use crate::storage::{self, repository};
use crate::window::platform::{self, PrevForeground};
use std::sync::Arc;
use std::time::Duration;
use tauri::State;

/// Tauri 管理的全局状态
pub struct AppState {
    pub db: Arc<storage::AppDatabase>,
    pub capture: Arc<CaptureState>,
    /// 唤起快捷键触发时抓取的前景窗口句柄（供 paste_to_previous 还原焦点）
    pub prev_foreground: PrevForeground,
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

#[tauri::command]
pub fn pause_capture(
    state: State<'_, AppState>,
    minutes: Option<u64>,
) -> Result<(), String> {
    let duration = minutes.map(|m| Duration::from_secs(m * 60));
    state.capture.pause(duration);

    // 持久化到 settings
    let conn = state.db.conn();
    if let Some(mins) = minutes {
        let until = chrono_now_ms() + (mins as i64 * 60 * 1000);
        let _ = repository::set_setting(&conn, "paused_until", Some(&until.to_string()));
    } else {
        let _ = repository::set_setting(&conn, "paused_until", Some("manual"));
    }

    tracing::info!("Capture paused for {:?} minutes", minutes);
    Ok(())
}

#[tauri::command]
pub fn resume_capture(
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.capture.resume();

    let conn = state.db.conn();
    let _ = repository::set_setting(&conn, "paused_until", None);

    tracing::info!("Capture resumed");
    Ok(())
}

#[tauri::command]
pub fn is_capture_paused(
    state: State<'_, AppState>,
) -> bool {
    state.capture.is_paused()
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
    repository::set_setting(&conn, &key, value.as_deref()).map_err(|e| e.to_string())
}

fn chrono_now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
