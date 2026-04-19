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
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use winapi::shared::minwindef::{DWORD, LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::HWND;
use winapi::um::winuser::{
    AddClipboardFormatListener, CreateWindowExW, DefWindowProcW, DispatchMessageW,
    GetClipboardSequenceNumber, GetMessageW, RegisterClassW, TranslateMessage, HWND_MESSAGE, MSG,
    WM_CLIPBOARDUPDATE, WNDCLASSW,
};

/// 全局 sender：WndProc 回调是 `unsafe extern "system" fn`，无法携带上下文，
/// 只能从静态拿。初始化时被 `start_windows_listener` 赋值一次，之后只读。
static EVENT_SENDER: OnceLock<Mutex<Option<Sender<()>>>> = OnceLock::new();

/// 上次 WndProc 收到 WM_CLIPBOARDUPDATE 时的剪贴板 sequence number。
/// 两用途：
/// 1. dedup：OS 偶发 spurious 重复 event，seq 未变就不通知消费者（对标 CopyQ winplatformclipboard.cpp:52-56）
/// 2. health check：定时线程对比 GetClipboardSequenceNumber 与此值，
///    如果 OS 剪贴板明显变了但我们没收到事件 → listener 链可能断（对标 Ditto SetEnsureConnectedTimer）
static LAST_SEEN_SEQ: AtomicU32 = AtomicU32::new(0);

/// 健康检查周期：每 5 分钟查一次。Ditto 用相同周期
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(300);

/// seq diff 超过此值判断链断（正常情况下消费事件应 ≤ seq 变化次数；
/// 允许一点偏差是因为 IsClipboardFormatAvailable 等内部操作也会动 seq）
const SEQ_BREAK_THRESHOLD: u32 = 10;

/// WndProc —— 剪贴板变化时 Windows 回调这里
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CLIPBOARDUPDATE {
        // GetClipboardSequenceNumber 无锁无开销，返回 OS 层累计 clipboard 变化计数。
        // 用它做二层 dedup：OS 偶发 duplicate 事件时 seq 未变 → skip 不惊扰消费者
        let current_seq: DWORD = unsafe { GetClipboardSequenceNumber() };
        let prev_seq = LAST_SEEN_SEQ.swap(current_seq as u32, Ordering::Relaxed);
        if current_seq as u32 == prev_seq && prev_seq != 0 {
            // Spurious WM_CLIPBOARDUPDATE（OS 偶发 bug / 第三方 hook 引起），不转发
            return 0;
        }

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

/// 健康检查线程：每 5 分钟对比 OS seq 与 listener 看到的最后 seq。
/// 如果 OS seq 显著领先 → listener 链可能被系统/安全软件摘除 → log warn。
/// v0.2 简版只报警不重连；Phase 2 补真重连（DestroyWindow + CreateWindow + AddListener）。
fn start_health_check() {
    std::thread::Builder::new()
        .name("teamo-clipboard-health".to_string())
        .spawn(|| loop {
            std::thread::sleep(HEALTH_CHECK_INTERVAL);
            let current: DWORD = unsafe { GetClipboardSequenceNumber() };
            let last = LAST_SEEN_SEQ.load(Ordering::Relaxed);
            // 只有 current > last 且差 > 阈值才 warn（wrap-around 罕见，u32 ~4B 次 clip 变化）
            if (current as u32).saturating_sub(last) > SEQ_BREAK_THRESHOLD {
                tracing::warn!(
                    "Clipboard listener may be disconnected: OS seq={}, last event seq={}, \
                     diff={}. If this persists, restart Teamo (Phase 2: auto-reconnect).",
                    current,
                    last,
                    (current as u32).saturating_sub(last),
                );
            }
        })
        .expect("failed to spawn clipboard health check thread");
}

/// 启动 Windows 剪贴板监听器线程。
/// 调用一次即可；线程跟 message loop 一起跑到进程退出。
pub fn start(tx: Sender<()>) {
    // 注入 sender 到全局静态
    let mu = EVENT_SENDER.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = mu.lock() {
        *guard = Some(tx);
    }

    // 初始化 seq baseline：不初始化的话首次 health check 会误判（OS seq 自开机以来
    // 累计了很多 clip 变化，但 LAST_SEEN_SEQ=0，差值会是亿级）
    let initial_seq: DWORD = unsafe { GetClipboardSequenceNumber() };
    LAST_SEEN_SEQ.store(initial_seq as u32, Ordering::Relaxed);

    std::thread::Builder::new()
        .name("teamo-clipboard-listener".to_string())
        .spawn(|| unsafe {
            run_message_loop();
        })
        .expect("failed to spawn clipboard listener thread");

    // 启动 health check 线程（v0.2 只报警；Phase 2 补自动重连）
    start_health_check();
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
