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
    menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder},
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
    pub const PAUSE_5M: &str = "pause_5m";
    pub const PAUSE_1H: &str = "pause_1h";
    pub const PAUSE_MANUAL: &str = "pause_manual";
    pub const RESUME: &str = "resume";
    pub const SETTINGS: &str = "settings";
    pub const QUIT: &str = "quit";
}

pub fn setup_tray(app: &App) -> tauri::Result<()> {
    let search = MenuItemBuilder::with_id(ids::SEARCH, "🔍 快速搜索 (Cmd/Ctrl+Shift+V)").build(app)?;

    let pause_5m = MenuItemBuilder::with_id(ids::PAUSE_5M, "5 分钟").build(app)?;
    let pause_1h = MenuItemBuilder::with_id(ids::PAUSE_1H, "1 小时").build(app)?;
    let pause_manual = MenuItemBuilder::with_id(ids::PAUSE_MANUAL, "直到我恢复").build(app)?;
    let pause_submenu = SubmenuBuilder::new(app, "⏸ 暂停记录")
        .item(&pause_5m)
        .item(&pause_1h)
        .item(&pause_manual)
        .build()?;

    let resume = MenuItemBuilder::with_id(ids::RESUME, "▶ 继续记录").build(app)?;
    let settings = MenuItemBuilder::with_id(ids::SETTINGS, "⚙️ 设置").build(app)?;
    let quit = MenuItemBuilder::with_id(ids::QUIT, "🚪 退出 Teamo").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&search)
        .separator()
        .item(&pause_submenu)
        .item(&resume)
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
        ids::PAUSE_5M => pause(app, Some(5)),
        ids::PAUSE_1H => pause(app, Some(60)),
        ids::PAUSE_MANUAL => pause(app, None),
        ids::RESUME => resume(app),
        ids::SETTINGS => {
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.show();
                let _ = main.unminimize();
                let _ = main.set_focus();
            }
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

fn pause(app: &AppHandle, minutes: Option<u64>) {
    if let Some(state) = app.try_state::<AppState>() {
        do_pause_capture(&state, minutes);
    } else {
        tracing::warn!("tray pause: AppState not ready");
    }
}

fn resume(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        do_resume_capture(&state);
    } else {
        tracing::warn!("tray resume: AppState not ready");
    }
}
