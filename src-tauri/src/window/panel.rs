use tauri::{AppHandle, Manager};

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
