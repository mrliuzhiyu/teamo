// Teamo lib · 业务模块入口
//
// 当前为 Phase 1A 脚手架阶段，仅启动 Tauri + 注册插件。
// 后续 issue 卷子会逐步填充：
//   - clipboard/   剪切板监听（issue: clipboard-capture）
//   - storage/     本地 SQLite + FTS5（issue: clipboard-capture）
//   - filter/      端侧闸门（issue: filter-engine）
//   - tray/        系统托盘 + 全局快捷键（issue: tray-menu）
//   - auth/        OAuth + Keychain（issue: desktop-oauth, M3）
//   - sync/        上行调度器（issue: upload-dispatcher, M3）

mod commands;

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
        .plugin(tauri_plugin_updater::Builder::default().build())
        .invoke_handler(tauri::generate_handler![commands::greet])
        .setup(|_app| {
            tracing::info!("Teamo started · pre-alpha scaffold");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
