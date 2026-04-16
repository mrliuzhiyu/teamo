// Tauri commands · 前端可调用的 Rust 函数入口

use crate::clipboard::CaptureState;
use crate::storage::{self, repository};
use std::sync::Arc;
use std::time::Duration;
use tauri::State;

/// Tauri 管理的全局状态
pub struct AppState {
    pub db: Arc<storage::AppDatabase>,
    pub capture: Arc<CaptureState>,
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
