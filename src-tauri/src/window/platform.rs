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

/// 哨兵名：前景进程以更高权限运行（管理员/LocalSystem/受保护进程），Teamo 的
/// `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` 被拒 → 拿不到 exe 路径。
/// 不返 None（None 会让 filter 整块跳过 app_rules 黑白名单，**用户加了 KeePass.exe
/// 到黑名单但 KeePass 以管理员跑时密码照样入库** —— 典型 silent fail）。
/// 返这个哨兵让 `filter::check_app_rules` 按"保守默认"处理（未知来源视同黑名单）。
pub const ELEVATED_APP_SENTINEL: &str = "<elevated>";

/// 抓取当前前景 App 的可识别名（Windows 下是 exe basename，例如 `Chrome.exe`）。
///
/// 供 filter-engine 的 app_rules 黑白名单匹配 + 写入 clipboard_local.source_app 列。
/// Teamo 自己进程被过滤（避免 panel/main 被当成黑名单源）。
///
/// - Windows：`GetForegroundWindow` → pid → `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)`
///   → `GetModuleFileNameExW` 拿完整路径 → 取 basename
///   - OpenProcess 失败（elevated / protected process）→ 返 `ELEVATED_APP_SENTINEL`
///     而非 None，让 filter 对未知来源采保守策略（避免 KeePass-as-admin 这类 bypass）
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
            // OpenProcess 被拒 —— 目标进程权限高于 Teamo（管理员运行的 App、LocalSystem
            // 服务、受保护进程如 Windows Defender）。返哨兵而非 None，强制 filter 对
            // 未知来源采保守策略，避免 KeePass-as-admin 这类 bypass。
            tracing::debug!("OpenProcess denied for pid={pid} — treating as elevated");
            return Some(ELEVATED_APP_SENTINEL.to_string());
        }

        // Windows long-path 模式下 exe 全路径可超 260 字符（MAX_PATH）直至 32767。
        // 512 默认够 99% 场景，满时 doubling 再试，避免截断后和 app_rules 模糊匹配错位
        // 或者 "C:\Pro…" 这种截断字符串永远匹配不上用户的规则
        let mut cap: usize = 512;
        let full_path = loop {
            let mut buffer = vec![0u16; cap];
            let len = GetModuleFileNameExW(
                process_handle,
                std::ptr::null_mut(),
                buffer.as_mut_ptr(),
                buffer.len() as u32,
            );
            if len == 0 {
                CloseHandle(process_handle);
                return None;
            }
            // len == buffer.len() 说明可能截断 —— Windows API 不保证设 ERROR_INSUFFICIENT_BUFFER
            // 到 32768 上限（MAX_PATH_LONG + 1）仍不够就放弃
            if (len as usize) < buffer.len() || cap >= 32768 {
                buffer.truncate(len as usize);
                CloseHandle(process_handle);
                break String::from_utf16_lossy(&buffer);
            }
            cap *= 2;
        };
        // 取 basename：C:\Path\To\Chrome.exe → Chrome.exe
        Some(basename(&full_path).to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// 从完整路径提取 basename（文件名部分）。纯函数便于单测。
/// - `C:\Path\To\Chrome.exe` → `Chrome.exe`
/// - `/usr/bin/code` → `code`
/// - `filename.exe`（无分隔符）→ `filename.exe`（原样返）
/// - 空串 → 空串
pub fn basename(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

#[cfg(test)]
mod basename_tests {
    use super::basename;

    #[test]
    fn windows_backslash_path() {
        assert_eq!(basename(r"C:\Program Files\Chrome\chrome.exe"), "chrome.exe");
    }

    #[test]
    fn unix_forward_slash_path() {
        assert_eq!(basename("/usr/bin/code"), "code");
    }

    #[test]
    fn mixed_separators() {
        assert_eq!(basename(r"C:/Users/Joy\App.exe"), "App.exe");
    }

    #[test]
    fn no_separator() {
        assert_eq!(basename("app.exe"), "app.exe");
    }

    #[test]
    fn empty_string() {
        assert_eq!(basename(""), "");
    }

    #[test]
    fn trailing_separator() {
        // rsplit 对尾斜杠返空 basename —— 符合 "目录没有文件名" 语义
        assert_eq!(basename(r"C:\Path\"), "");
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
