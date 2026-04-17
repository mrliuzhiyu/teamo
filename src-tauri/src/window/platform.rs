//! 平台特殊化：前景窗口抓取 + 系统粘贴模拟
//!
//! Phase 3B 仅实现 Windows；macOS/Linux 的 `capture_foreground` 返回 None，
//! `activate_and_paste` 返回 Err("not implemented")，前端据此回退到
//! 「复制 + hide」的手动粘贴流程。
//! 等 Phase 4 做 macOS NSPanel 时，连带补齐 macOS 的 CGEvent 粘贴路径。

use std::sync::Mutex;

/// 前景窗口句柄。用 `isize` 保存 Windows HWND（指针）以满足 Send + Sync。
#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Copy)]
pub struct ForegroundHandle {
    hwnd: isize,
}

#[cfg(not(target_os = "windows"))]
#[derive(Debug, Clone, Copy)]
pub struct ForegroundHandle;

/// 全局状态里用于存「唤起前的前景 App」句柄。
pub type PrevForeground = Mutex<Option<ForegroundHandle>>;

/// 抓取当前前景窗口句柄。必须在 panel.show() **之前** 调用。
///
/// 如果前景窗口属于 Teamo 自己（main window / panel），返回 None——
/// 避免 Enter 粘贴时把内容粘回 Teamo 自身窗口。
pub fn capture_foreground() -> Option<ForegroundHandle> {
    #[cfg(target_os = "windows")]
    unsafe {
        use winapi::um::processthreadsapi::GetCurrentProcessId;
        use winapi::um::winuser::{GetForegroundWindow, GetWindowThreadProcessId};

        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }

        // 过滤 Teamo 自身进程的窗口：避免把 Ctrl+V 粘到自己的 main window / panel
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == GetCurrentProcessId() {
            tracing::debug!("capture_foreground: skipping self window (pid={pid})");
            return None;
        }

        Some(ForegroundHandle { hwnd: hwnd as isize })
    }

    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// 抓取当前前景 App 的可识别名（Windows 下是 exe basename，例如 `Chrome.exe`）。
///
/// 供 filter-engine 的 app_rules 黑白名单匹配 + 写入 clipboard_local.source_app 列。
/// Teamo 自己进程被过滤（避免 panel/main 被当成黑名单源）。
///
/// - Windows：`GetForegroundWindow` → pid → `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)`
///   → `GetModuleFileNameExW` 拿完整路径 → 取 basename
/// - macOS：留 Phase 4 跟 NSPanel 一起做（需要 `NSWorkspace.frontmostApplication`）
/// - Linux：目前无需求
pub fn capture_foreground_app_name() -> Option<String> {
    #[cfg(target_os = "windows")]
    unsafe {
        use winapi::shared::minwindef::FALSE;
        use winapi::um::handleapi::CloseHandle;
        use winapi::um::processthreadsapi::{GetCurrentProcessId, OpenProcess};
        use winapi::um::psapi::GetModuleFileNameExW;
        use winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION;
        use winapi::um::winuser::{GetForegroundWindow, GetWindowThreadProcessId};

        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 || pid == GetCurrentProcessId() {
            return None; // Teamo 自己进程不算 source_app
        }

        let process_handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
        if process_handle.is_null() {
            return None;
        }

        let mut buffer = vec![0u16; 512];
        let len = GetModuleFileNameExW(
            process_handle,
            std::ptr::null_mut(),
            buffer.as_mut_ptr(),
            buffer.len() as u32,
        );
        CloseHandle(process_handle);

        if len == 0 {
            return None;
        }
        buffer.truncate(len as usize);
        let full_path = String::from_utf16_lossy(&buffer);
        // 取 basename：C:\Path\To\Chrome.exe → Chrome.exe
        Some(
            full_path
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or(&full_path)
                .to_string(),
        )
    }

    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// 激活句柄对应的窗口 + 模拟 Ctrl+V。
///
/// 调用前调用方应该已经：
/// 1. 把要粘贴的内容写入系统剪切板
/// 2. 隐藏 panel 窗口
/// 3. sleep ~80ms 让系统焦点切换生效
pub fn activate_and_paste(handle: Option<ForegroundHandle>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::thread::sleep;
        use std::time::Duration;

        if let Some(h) = handle {
            unsafe {
                use winapi::shared::windef::HWND;
                use winapi::um::winuser::SetForegroundWindow;
                let ok = SetForegroundWindow(h.hwnd as HWND);
                if ok == 0 {
                    // 失败不阻塞，继续尝试模拟 Ctrl+V（贴到当前前景也聊胜于无）
                    tracing::warn!(
                        "SetForegroundWindow failed for hwnd={:#x}; target may be gone",
                        h.hwnd
                    );
                }
            }
            sleep(Duration::from_millis(30));
        } else {
            tracing::debug!("activate_and_paste: no prev handle, skipping SetForegroundWindow");
        }

        use enigo::{Direction, Enigo, Key, Keyboard, Settings};
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| format!("enigo init failed: {e}"))?;
        enigo
            .key(Key::Control, Direction::Press)
            .map_err(|e| format!("ctrl press failed: {e}"))?;
        let result_v = enigo.key(Key::Unicode('v'), Direction::Click);
        // 无论 V 成功与否都要 release，避免 Ctrl 卡住
        let _ = enigo.key(Key::Control, Direction::Release);
        result_v.map_err(|e| format!("v click failed: {e}"))?;

        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = handle;
        Err("system paste not implemented on this platform".to_string())
    }
}
