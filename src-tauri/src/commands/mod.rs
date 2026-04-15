// Tauri commands · 前端可调用的 Rust 函数入口
//
// 脚手架阶段仅一个占位 greet 命令；Phase 1A 业务模块就位后会拆分为：
//   - capture.rs   剪切板捕获
//   - search.rs    本地 FTS5 搜索
//   - filter.rs    敏感检测 / 黑白名单查询
//   - settings.rs  设置读写

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Teamo 在守候你的剪切板。", name)
}
