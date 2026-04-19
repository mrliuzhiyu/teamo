// Teamo lib · 业务模块入口
//
// 启动流程：
// 1. 初始化 tracing
// 2. 初始化 SQLite（storage + migration）
// 3. 启动剪切板捕获循环
// 4. 注册 Tauri 插件 + commands

mod auth;
mod clipboard;
mod cloud_sync;
mod commands;
mod export;
mod filter;
mod settings_keys;
mod storage;
mod tray;
mod window;

use commands::AppState;
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "teamo_lib=debug,info".into()),
        )
        .init();

    tauri::Builder::default()
        .on_window_event(|window, event| {
            // 主窗口点 X 不退出应用（Slack 风格）：只隐藏，Tray 菜单 [退出] 才真正退出。
            // panel 窗口 decorations=false 用户无法点 X，不会触发 CloseRequested。
            // tray QUIT 走 app.exit(0) 路径会触发这里，必须靠 IS_QUITTING flag 放行，
            // 否则 prevent_close 会把退出流程也拦住，永远退不出应用。
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                use std::sync::atomic::Ordering;
                if window.label() == "main" && !tray::IS_QUITTING.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    use tauri_plugin_global_shortcut::ShortcutState;
                    if event.state != ShortcutState::Pressed {
                        return;
                    }
                    // 只在即将 show panel 时抓前景窗口（hide 时 panel 自己才是前景，抓了没用）
                    let will_show = app
                        .get_webview_window(window::panel::PANEL_LABEL)
                        .map(|w| !w.is_visible().unwrap_or(false))
                        .unwrap_or(false);
                    if will_show {
                        let fg = window::platform::capture_foreground();
                        if let Some(state) = app.try_state::<AppState>() {
                            if let Ok(mut guard) = state.prev_foreground.lock() {
                                *guard = fg;
                            }
                        }
                    }
                    window::panel::toggle_panel(app);
                })
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            commands::search_clipboard,
            commands::list_recent_clipboard,
            commands::get_clipboard_detail,
            commands::get_today_stats,
            commands::copy_image_to_clipboard,
            commands::get_image_data_url,
            commands::export_data,
            commands::import_data,
            commands::list_sessions,
            commands::list_session_items,
            commands::auth_send_otp,
            commands::auth_verify_otp,
            commands::auth_logout,
            commands::auth_state,
            commands::upload_session,
            commands::get_data_info,
            commands::clear_all_data,
            commands::list_app_rules,
            commands::add_app_rule,
            commands::remove_app_rule,
            commands::get_current_foreground_app,
            commands::paste_to_previous,
            commands::forget_clipboard,
            commands::toggle_pin,
            commands::mark_used,
            commands::pause_capture,
            commands::resume_capture,
            commands::is_capture_paused,
            commands::get_capture_health,
            commands::get_setting,
            commands::set_setting,
        ])
        .setup(|app| {
            // 1. 确定数据目录
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");

            tracing::info!("Data directory: {}", data_dir.display());

            // 2. 初始化 SQLite
            let db = Arc::new(
                storage::AppDatabase::init(data_dir)
                    .expect("failed to initialize database"),
            );

            // 2b. 首次启动或 YAML 升级时 seed 内置 domain_rules（70+ 条 YAML 规则）
            {
                let conn = db.conn();
                if let Err(e) = storage::seed_rules::seed_if_outdated(&conn) {
                    tracing::warn!("seed domain_rules failed: {e}");
                }
            }
            // 启动 seed 可能改了 domain_rules，invalidate filter cache 让 capture loop 读到新规则
            filter::cache::invalidate();

            // 2c. 启动时清理过期数据（按 data.retention 设置）
            {
                let conn = db.conn();
                let images_dir = db.images_dir();
                match storage::retention::prune_expired(&conn, &images_dir) {
                    Ok(n) if n > 0 => tracing::info!("pruned {n} expired entries on startup"),
                    Ok(_) => {}
                    Err(e) => tracing::warn!("retention prune failed: {e}"),
                }
            }

            // 2d. 孤儿 PNG 清理：capture 流程是"先写 PNG → INSERT row"两步非原子，
            //     崩溃/kill 时 PNG 已存但 DB 行缺失，长期积累 images/ 会膨胀
            {
                let conn = db.conn();
                let images_dir = db.images_dir();
                match storage::reconcile::cleanup_orphan_images(&conn, &images_dir) {
                    Ok(n) if n > 0 => tracing::info!("cleaned {n} orphan image(s) on startup"),
                    Ok(_) => {}
                    Err(e) => tracing::warn!("orphan image cleanup failed: {e}"),
                }
            }

            // 3. 初始化捕获状态（检查是否有持久化的暂停状态）
            let capture_state = Arc::new(clipboard::CaptureState::new());
            {
                let conn = db.conn();
                if let Ok(Some(paused_val)) =
                    storage::repository::get_setting(&conn, settings_keys::CAPTURE_PAUSED_UNTIL)
                {
                    if paused_val == "manual" {
                        capture_state.pause(None);
                        tracing::info!("Restored manual pause state");
                    } else if let Ok(until_ms) = paused_val.parse::<i64>() {
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as i64;
                        if until_ms > now_ms {
                            let remaining =
                                std::time::Duration::from_millis((until_ms - now_ms) as u64);
                            capture_state.pause(Some(remaining));
                            tracing::info!(
                                "Restored timed pause, {}s remaining",
                                remaining.as_secs()
                            );
                        } else {
                            // 暂停已过期，清除
                            let _ = storage::repository::set_setting(
                                &conn,
                                settings_keys::CAPTURE_PAUSED_UNTIL,
                                None,
                            );
                        }
                    }
                }
            }

            // 4. 启动剪切板捕获循环
            clipboard::start_capture(
                Arc::clone(&db),
                Arc::clone(&capture_state),
                app.handle().clone(),
            );

            // 5. 注册全局状态
            app.manage(AppState {
                db,
                capture: capture_state,
                prev_foreground: Mutex::new(None),
                tray_pause_item: Mutex::new(None),
            });

            // 6. 注册全局快捷键：Cmd+Shift+V (macOS) / Ctrl+Shift+V (其他) → toggle panel
            #[cfg(desktop)]
            {
                use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

                #[cfg(target_os = "macos")]
                let modifiers = Modifiers::SUPER | Modifiers::SHIFT;
                #[cfg(not(target_os = "macos"))]
                let modifiers = Modifiers::CONTROL | Modifiers::SHIFT;

                let shortcut = Shortcut::new(Some(modifiers), Code::KeyV);
                app.global_shortcut().register(shortcut)?;
                tracing::info!("Global shortcut registered: toggle panel (Cmd/Ctrl+Shift+V)");
            }

            // 7. Tray 图标 + 菜单
            tray::setup_tray(app)?;

            // 8. 首次启动决策 —— show panel 做引导（欢迎横幅由前端 list 视图渲染）
            //    原本 show main window，但 v0.2 后所有设置统一走 panel 内 settings 视图，
            //    main window 失去自然入口，首次启动改 show panel 保持一致性
            {
                let state: tauri::State<'_, AppState> = app.state();
                let is_first_run = {
                    let conn = state.db.conn();
                    storage::repository::get_setting(
                        &conn,
                        settings_keys::APP_FIRST_RUN_COMPLETED,
                    )
                    .ok()
                    .flatten()
                    .is_none()
                };

                if is_first_run {
                    window::panel::show_panel(&app.handle());
                    let conn = state.db.conn();
                    let _ = storage::repository::set_setting(
                        &conn,
                        settings_keys::APP_FIRST_RUN_COMPLETED,
                        Some("1"),
                    );
                    tracing::info!("First run — showing panel as onboarding");
                } else {
                    tracing::info!("Not first run — staying silent in tray");
                }
            }

            tracing::info!("Teamo started · clipboard capture active");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
