// Teamo · 你的人生记录 Agent
// 入口仅做日志初始化 + 委托给 lib::run

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    teamo_lib::run()
}
