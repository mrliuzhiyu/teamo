//! Windows 剪贴板事件驱动监听器（AddClipboardFormatListener + WM_CLIPBOARDUPDATE）
//!
//! 对标 Ditto / CopyQ / Maccy（macOS）的事件驱动方案——取代 500ms 轮询。
//!
//! 架构：
//! 1. 独立 std::thread 跑 Windows message loop
//! 2. 创建 message-only window（HWND_MESSAGE）—— 不显示、不占 taskbar
//! 3. `AddClipboardFormatListener(hwnd)` 订阅剪贴板变化
//! 4. WndProc 收到 `WM_CLIPBOARDUPDATE` → 通过全局 mpsc Sender 发一个 ()
//! 5. 消费线程（在 `clipboard/mod.rs`）从 Receiver 接事件触发 ingest
//!
//! 收益：
//! - CPU：idle 时真 idle，不再 500ms 空转
//! - 延迟：<10ms（OS 消息机制）vs 轮询 0-500ms
//! - 无漏记：快速连续 copy 在轮询间隔内会被漏掉，事件驱动全收
//!
//! v0.1 Windows-only；macOS 走 NSPasteboard KVO（Phase 4）。

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};

use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::HWND;
use winapi::um::winuser::{
    AddClipboardFormatListener, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW,
    RegisterClassW, TranslateMessage, HWND_MESSAGE, MSG, WM_CLIPBOARDUPDATE, WNDCLASSW,
};

/// 全局 sender：WndProc 回调是 `unsafe extern "system" fn`，无法携带上下文，
/// 只能从静态拿。初始化时被 `start_windows_listener` 赋值一次，之后只读。
static EVENT_SENDER: OnceLock<Mutex<Option<Sender<()>>>> = OnceLock::new();

/// WndProc —— 剪贴板变化时 Windows 回调这里
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CLIPBOARDUPDATE {
        if let Some(mu) = EVENT_SENDER.get() {
            if let Ok(guard) = mu.lock() {
                if let Some(tx) = guard.as_ref() {
                    // send 失败 = 接收端已 drop = 应用正在退出，忽略即可
                    let _ = tx.send(());
                }
            }
        }
        return 0;
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// 启动 Windows 剪贴板监听器线程。
/// 调用一次即可；线程跟 message loop 一起跑到进程退出。
pub fn start(tx: Sender<()>) {
    // 注入 sender 到全局静态
    let mu = EVENT_SENDER.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = mu.lock() {
        *guard = Some(tx);
    }

    std::thread::Builder::new()
        .name("teamo-clipboard-listener".to_string())
        .spawn(|| unsafe {
            run_message_loop();
        })
        .expect("failed to spawn clipboard listener thread");
}

/// 消息循环主体。panic / 初始化失败时记 log 后线程退出（整个捕获子系统 offline）。
unsafe fn run_message_loop() {
    let class_name: Vec<u16> = OsStr::new("TeamoClipboardListener")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let wc = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        lpszClassName: class_name.as_ptr(),
        ..std::mem::zeroed()
    };
    // RegisterClassW 若 class 名重复会返回 0，但这是单例场景 OK
    RegisterClassW(&wc);

    // message-only window：不显示、不进 z-order、不占 taskbar
    // parent = HWND_MESSAGE 是关键标记
    let hwnd = CreateWindowExW(
        0,
        class_name.as_ptr(),
        class_name.as_ptr(),
        0,
        0,
        0,
        0,
        0,
        HWND_MESSAGE,
        ptr::null_mut(),
        ptr::null_mut(),
        ptr::null_mut(),
    );

    if hwnd.is_null() {
        tracing::error!("Failed to create message-only window for clipboard listener");
        return;
    }

    if AddClipboardFormatListener(hwnd) == 0 {
        tracing::error!(
            "AddClipboardFormatListener failed — clipboard events won't be delivered"
        );
        return;
    }

    tracing::info!("Windows clipboard event listener started (event-driven capture)");

    // 消息循环
    let mut msg: MSG = std::mem::zeroed();
    while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    tracing::info!("Windows clipboard listener message loop exited");
}
