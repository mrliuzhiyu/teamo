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
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::OnceLock;
use std::time::Duration;

use winapi::shared::minwindef::{DWORD, LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::HWND;
use winapi::um::processthreadsapi::GetCurrentThreadId;
use winapi::um::winuser::{
    AddClipboardFormatListener, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetClipboardSequenceNumber, GetMessageW, PostThreadMessageW, RegisterClassW,
    RemoveClipboardFormatListener, TranslateMessage, HWND_MESSAGE, MSG, WM_CLIPBOARDUPDATE,
    WM_QUIT, WNDCLASSW,
};

/// 全局 sender：WndProc 回调是 `unsafe extern "system" fn`，无法携带上下文，
/// 只能从静态拿。
///
/// 为什么不用 `Mutex<Option<Sender>>`：旧版本三层套娃 + 每次事件 lock()。
/// Mutex poisoning（虽概率低）会让 `mu.lock()` 返 PoisonError，`if let Ok(guard)`
/// silent drop 所有后续事件 —— listener 静默失聪。
/// `Sender` 本身 `send(&self, _)` 无需可变访问；启动时注入后从不改 → 直接
/// `OnceLock<Sender>` 无 Mutex 无 poisoning 风险，wnd_proc 路径也少一次 lock。
static EVENT_SENDER: OnceLock<Sender<()>> = OnceLock::new();

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

/// 当前 message loop 线程 ID（用于 PostThreadMessageW 发 WM_QUIT 停旧 loop）
static LISTENER_THREAD_ID: AtomicU32 = AtomicU32::new(0);

/// 当前 message-only window 句柄 isize（HWND 持有跨线程需要）
/// 重连时用于 RemoveClipboardFormatListener + DestroyWindow 清理
static LISTENER_HWND: AtomicIsize = AtomicIsize::new(0);

/// 防止并发重连（两个 health check 同时触发 reconnect 会双起 message loop）
static RECONNECTING: AtomicBool = AtomicBool::new(false);

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

        if let Some(tx) = EVENT_SENDER.get() {
            // send 失败 = 接收端已 drop = 应用正在退出，忽略即可
            let _ = tx.send(());
        }
        return 0;
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// 健康检查线程：每 5 分钟对比 OS seq 与 listener 看到的最后 seq。
/// OS seq 显著领先 = listener 链被系统/安全软件摘除 → 自动重连恢复。
/// Ditto 对应 `SetEnsureConnectedTimer` 机制（ClipboardViewer.cpp:107-110, 439-461）。
fn start_health_check() {
    std::thread::Builder::new()
        .name("teamo-clipboard-health".to_string())
        .spawn(|| loop {
            std::thread::sleep(HEALTH_CHECK_INTERVAL);
            let current: DWORD = unsafe { GetClipboardSequenceNumber() };
            let last = LAST_SEEN_SEQ.load(Ordering::Relaxed);
            let diff = (current as u32).saturating_sub(last);
            if diff > SEQ_BREAK_THRESHOLD {
                tracing::warn!(
                    "Clipboard listener disconnected: OS seq={current}, last event seq={last}, \
                     diff={diff}. Attempting auto-reconnect..."
                );
                reconnect_listener();
            }
        })
        .expect("failed to spawn clipboard health check thread");
}

/// 重建 message-only window + AddClipboardFormatListener。
/// 流程：
///   1. 抢 RECONNECTING flag（避免两个 health check 并发重连双起 message loop）
///   2. 给旧 message loop thread 发 WM_QUIT → GetMessageW 返 0 → loop 退出 → 触发
///      run_message_loop 底部的 cleanup（RemoveClipboardFormatListener + DestroyWindow）
///   3. sleep 200ms 等旧 thread 退出 + 句柄释放（不 join 因 thread handle 不持有）
///   4. spawn 新 message loop
fn reconnect_listener() {
    if RECONNECTING.swap(true, Ordering::AcqRel) {
        tracing::debug!("reconnect_listener: another reconnect in progress, skipping");
        return;
    }

    let old_tid = LISTENER_THREAD_ID.load(Ordering::Relaxed);
    if old_tid != 0 {
        unsafe {
            // PostThreadMessageW 发 WM_QUIT 到指定线程的 message queue → GetMessageW 返 0 退出 loop
            PostThreadMessageW(old_tid, WM_QUIT, 0, 0);
        }
        // 让旧 thread 跑完 cleanup（RemoveListener + DestroyWindow）
        std::thread::sleep(Duration::from_millis(200));
    }

    // 启动新 message loop（复用 run_message_loop，内部重新 RegisterClass + CreateWindow + AddListener）
    std::thread::Builder::new()
        .name("teamo-clipboard-listener".to_string())
        .spawn(|| unsafe {
            run_message_loop();
        })
        .ok();

    // 同步 seq baseline，避免新 listener 启动后 health check 立即又判断断链
    let initial_seq: DWORD = unsafe { GetClipboardSequenceNumber() };
    LAST_SEEN_SEQ.store(initial_seq as u32, Ordering::Relaxed);

    RECONNECTING.store(false, Ordering::Release);
    tracing::info!("Clipboard listener reconnected");
}

/// 启动 Windows 剪贴板监听器线程。
/// 调用一次即可；线程跟 message loop 一起跑到进程退出。
pub fn start(tx: Sender<()>) {
    // 注入 sender 到全局静态。重复调用时 set 会返 Err，忽略（幂等启动）
    let _ = EVENT_SENDER.set(tx);

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
/// 退出前做 cleanup：RemoveClipboardFormatListener + DestroyWindow（重连时必须干净）
unsafe fn run_message_loop() {
    // 记录本线程 ID，供 reconnect_listener 用 PostThreadMessageW 停止
    LISTENER_THREAD_ID.store(GetCurrentThreadId(), Ordering::Relaxed);

    let class_name: Vec<u16> = OsStr::new("TeamoClipboardListener")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let wc = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        lpszClassName: class_name.as_ptr(),
        ..std::mem::zeroed()
    };
    // RegisterClassW 首次调用成功；后续重连调用返 0（class 已注册），但不影响后续 CreateWindow
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
        LISTENER_THREAD_ID.store(0, Ordering::Relaxed);
        return;
    }

    if AddClipboardFormatListener(hwnd) == 0 {
        tracing::error!(
            "AddClipboardFormatListener failed — clipboard events won't be delivered"
        );
        DestroyWindow(hwnd);
        LISTENER_THREAD_ID.store(0, Ordering::Relaxed);
        return;
    }

    LISTENER_HWND.store(hwnd as isize, Ordering::Relaxed);
    tracing::info!("Windows clipboard event listener started (event-driven capture)");

    // 消息循环。PostThreadMessageW(WM_QUIT) 会让 GetMessageW 返回 0 退出
    let mut msg: MSG = std::mem::zeroed();
    while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    // Cleanup：在线程退出前解绑 listener + 销毁 window，避免资源泄漏 + 防止 reconnect 时
    // 新旧 hwnd 双收 WM_CLIPBOARDUPDATE 产生 double capture
    RemoveClipboardFormatListener(hwnd);
    DestroyWindow(hwnd);
    LISTENER_HWND.store(0, Ordering::Relaxed);
    LISTENER_THREAD_ID.store(0, Ordering::Relaxed);
    tracing::info!("Windows clipboard listener message loop exited + cleaned up");
}
