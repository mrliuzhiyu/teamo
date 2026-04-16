// Teamo lib · 业务模块入口
//
// 启动流程：
// 1. 初始化 tracing
// 2. 初始化 SQLite（storage + migration）
// 3. 启动剪切板捕获循环
// 4. 注册 Tauri 插件 + commands

mod clipboard;
mod commands;
mod storage;

use commands::AppState;
use std::sync::Arc;
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
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            commands::search_clipboard,
            commands::list_recent_clipboard,
            commands::get_clipboard_detail,
            commands::forget_clipboard,
            commands::pause_capture,
            commands::resume_capture,
            commands::is_capture_paused,
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

            // 3. 初始化捕获状态（检查是否有持久化的暂停状态）
            let capture_state = Arc::new(clipboard::CaptureState::new());
            {
                let conn = db.conn();
                if let Ok(Some(paused_val)) = storage::repository::get_setting(&conn, "paused_until") {
                    if paused_val == "manual" {
                        capture_state.pause(None);
                        tracing::info!("Restored manual pause state");
                    } else if let Ok(until_ms) = paused_val.parse::<i64>() {
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as i64;
                        if until_ms > now_ms {
                            let remaining = std::time::Duration::from_millis((until_ms - now_ms) as u64);
                            capture_state.pause(Some(remaining));
                            tracing::info!("Restored timed pause, {}s remaining", remaining.as_secs());
                        } else {
                            // 暂停已过期，清除
                            let _ = storage::repository::set_setting(&conn, "paused_until", None);
                        }
                    }
                }
            }

            // 4. 启动剪切板捕获循环
            clipboard::start_capture_loop(Arc::clone(&db), Arc::clone(&capture_state));

            // 5. 注册全局状态
            app.manage(AppState {
                db,
                capture: capture_state,
            });

            tracing::info!("Teamo started · clipboard capture active");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
