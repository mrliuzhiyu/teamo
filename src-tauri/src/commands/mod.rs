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
}

/// resume_capture 的核心逻辑，供 Tauri command 和 tray menu handler 共享。
pub fn do_resume_capture(state: &AppState) {
    state.capture.resume();
    let conn = state.db.conn();
    let _ = repository::set_setting(&conn, settings_keys::CAPTURE_PAUSED_UNTIL, None);
    tracing::info!("Capture resumed");
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
    repository::set_setting(&conn, &key, value.as_deref()).map_err(|e| e.to_string())
}

fn chrono_now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
