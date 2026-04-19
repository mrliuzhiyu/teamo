use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow};

pub const PANEL_LABEL: &str = "panel";

/// 把 panel 窗口居中到"鼠标所在显示器"(多屏场景常识:调出面板跟随鼠标)。
/// 拿不到 cursor / monitor 时 fallback 到 tauri 默认 center(主显示器居中)。
fn center_on_cursor_monitor(app: &AppHandle, window: &WebviewWindow) {
    let Ok(cursor) = app.cursor_position() else {
        let _ = window.center();
        return;
    };
    let Ok(monitors) = app.available_monitors() else {
        let _ = window.center();
        return;
    };
    let cx = cursor.x;
    let cy = cursor.y;
    // 找鼠标所在的 monitor(矩形包含判断)
    let target = monitors.iter().find(|m| {
        let pos = m.position();
        let size = m.size();
        let mx = pos.x as f64;
        let my = pos.y as f64;
        let mw = size.width as f64;
        let mh = size.height as f64;
        cx >= mx && cx < mx + mw && cy >= my && cy < my + mh
    });
    let Some(m) = target else {
        let _ = window.center();
        return;
    };
    let Ok(win_size) = window.outer_size() else {
        let _ = window.center();
        return;
    };
    let mx = m.position().x as f64;
    let my = m.position().y as f64;
    let mw = m.size().width as f64;
    let mh = m.size().height as f64;
    let ww = win_size.width as f64;
    let wh = win_size.height as f64;
    // 水平居中 + 垂直略偏上(35% 处,对标 Raycast/Alfred 的常识 — 完全居中会挡视线)
    let x = mx + (mw - ww) / 2.0;
    let y = my + (mh - wh) * 0.35;
    let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

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
            center_on_cursor_monitor(app, &window);
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
    center_on_cursor_monitor(app, &window);
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
