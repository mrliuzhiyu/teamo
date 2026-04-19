//! Tray icon + 菜单
//!
//! Phase 1 范围（最小可用）：
//! - 图标出现（Windows 任务栏/macOS 菜单栏）
//! - 菜单：快速搜索 / 暂停子菜单 / 继续记录 / 设置 / 退出
//! - 主窗口 close → hide（见 lib.rs 的 on_window_event）
//!
//! Phase 2 留：
//! - 状态色（绿/黄/红/灰）图标切换
//! - 动态 tooltip + 菜单状态行（记录中 · 今日 N 条）
//! - 自启动引导弹窗
//! - About 弹窗
//! - 退出前 5 秒强杀兜底

use crate::commands::{do_pause_capture, do_resume_capture, AppState};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager,
};

/// 区分"用户点 X 关主窗"和"tray 菜单点退出 → app.exit(0)"。
///
/// 背景：main window 的 CloseRequested handler 无条件 `prevent_close + hide` 会让
/// `app.exit(0)` 也被拦截，永远退不出。tray QUIT handler 先把这个 flag 置 true，
/// CloseRequested handler 检查 flag 决定是否放行。
///
/// 放 pub static 因为 lib.rs 的 on_window_event 也要读。
pub static IS_QUITTING: AtomicBool = AtomicBool::new(false);

/// 菜单项 id — 在单一常量源定义避免字符串漂移
mod ids {
    pub const SEARCH: &str = "search";
    pub const TOGGLE_PAUSE: &str = "toggle_pause";
    pub const SETTINGS: &str = "settings";
    pub const QUIT: &str = "quit";
}

pub fn setup_tray(app: &App) -> tauri::Result<()> {
    let search = MenuItemBuilder::with_id(ids::SEARCH, "🔍 快速搜索 (Cmd/Ctrl+Shift+V)").build(app)?;

    // 暂停简化为单一 toggle 项（点击根据当前状态切 pause/resume）
    // 之前的 5m/1h/手动 三选项实际使用 95% 是手动恢复，时间预设属于过度设计
    let toggle_pause = MenuItemBuilder::with_id(ids::TOGGLE_PAUSE, "⏸ 暂停 / ▶ 继续记录").build(app)?;

    let settings = MenuItemBuilder::with_id(ids::SETTINGS, "⚙️ 设置").build(app)?;
    let quit = MenuItemBuilder::with_id(ids::QUIT, "🚪 退出 Teamo").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&search)
        .separator()
        .item(&toggle_pause)
        .separator()
        .item(&settings)
        .separator()
        .item(&quit)
        .build()?;

    // Tray 专用图标：单色白 T，透明背景，融入系统任务栏（区别于彩色主 logo）
    // @2x 让 Windows HiDPI / macOS Retina 屏幕清晰；include_image! 编译期嵌入 + 解码
    let icon = tauri::include_image!("icons/tray-icon@2x.png");

    // Tauri 2.x 内部会把 TrayIcon clone 一份存进 manager.tray.icons，
    // 这里的 handle 即使 drop 也不会销毁 tray；显式 `_tray` binding 只是为了代码意图清晰。
    // Windows/Linux 惯例：左键 = 主操作（切换快速面板），右键 = 展开菜单。
    // macOS 惯例：左键展开菜单（延后 Phase 4 按平台区分）。
    let _tray = TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .tooltip("Teamo · 你的人生记录 Agent")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_panel_capturing_foreground(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        ids::SEARCH => {
            // 和全局快捷键 handler 同样的"show 前抓前景 App"逻辑——
            // 否则从 tray 唤起的 panel 按 Enter 会粘到错误的窗口。
            show_panel_capturing_foreground(app);
        }
        ids::TOGGLE_PAUSE => toggle_pause(app),
        ids::SETTINGS => {
            // 统一走 panel 内 settings 视图，不再开独立 main window
            // main window 还保留在 conf 里作为未来冗余，但 v0.1 没有正常入口
            crate::window::panel::open_panel_settings(app);
        }
        ids::QUIT => {
            // 先置 flag 让 CloseRequested handler 放行，再 exit
            IS_QUITTING.store(true, Ordering::SeqCst);
            app.exit(0);
        }
        other => {
            tracing::debug!("tray: unhandled menu id {other}");
        }
    }
}

/// 抓前景 App（仅 will_show 分支）+ toggle panel。
/// 复制自 lib.rs 的全局快捷键 handler — 之所以不抽共享函数，是因为会引入
/// window → commands 的反向模块依赖（window/platform 依赖 commands::AppState），
/// 收益不抵。两处代码完全对称，后续修改需要两处同步。
fn show_panel_capturing_foreground(app: &AppHandle) {
    let will_show = app
        .get_webview_window(crate::window::panel::PANEL_LABEL)
        .map(|w| !w.is_visible().unwrap_or(false))
        .unwrap_or(false);
    if will_show {
        let fg = crate::window::platform::capture_foreground();
        if let Some(state) = app.try_state::<AppState>() {
            if let Ok(mut guard) = state.prev_foreground.lock() {
                *guard = fg;
            }
        }
    }
    crate::window::panel::toggle_panel(app);
}

fn toggle_pause(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        if state.capture.is_paused() {
            do_resume_capture(&state);
        } else {
            // None = 直到手动恢复；简化后没有时间预设
            do_pause_capture(&state, None);
        }
    } else {
        tracing::warn!("tray toggle_pause: AppState not ready");
    }
}
