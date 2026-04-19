use tauri::{AppHandle, Emitter, Manager};

pub const PANEL_LABEL: &str = "panel";

pub fn toggle_panel(app: &AppHandle) {
    let Some(window) = app.get_webview_window(PANEL_LABEL) else {
        tracing::warn!("panel window not found");
        return;
    };

    match window.is_visible() {
        Ok(true) => {
            if let Err(e) = window.hide() {
                tracing::error!("failed to hide panel: {e}");
            }
        }
        _ => {
            let _ = window.center();
            if let Err(e) = window.show() {
                tracing::error!("failed to show panel: {e}");
                return;
            }
            if let Err(e) = window.set_focus() {
                tracing::error!("failed to focus panel: {e}");
            }
        }
    }
}

/// 只 show 不 toggle：用于 tray "⚙️ 设置" 和首次启动 —— 需要确保 panel 可见，
/// 不能因已 visible 就 hide。
pub fn show_panel(app: &AppHandle) {
    let Some(window) = app.get_webview_window(PANEL_LABEL) else {
        tracing::warn!("panel window not found");
        return;
    };
    let _ = window.center();
    if let Err(e) = window.show() {
        tracing::error!("failed to show panel: {e}");
        return;
    }
    if let Err(e) = window.set_focus() {
        tracing::error!("failed to focus panel: {e}");
    }
}

/// 打开面板并进 settings 视图（tray "⚙️ 设置" 入口统一走这里，替代独立 main window）
pub fn open_panel_settings(app: &AppHandle) {
    show_panel(app);
    // 通知前端切 view = "settings"
    if let Err(e) = app.emit_to(PANEL_LABEL, "panel:open-settings", ()) {
        tracing::error!("failed to emit panel:open-settings: {e}");
    }
}
