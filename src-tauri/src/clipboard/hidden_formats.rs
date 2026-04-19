//! Windows 剪贴板"请忽略我"MIME 过滤。
//!
//! 背景：密码管理器 / 银行 App / 受保护输入源会主动设置官方 Windows clipboard
//! format 声明"这份内容别记录"。Windows 原生 Clipboard History、OneNote、
//! Office 都遵守这个语义。Teamo v0.1/v0.2 没做 → 复制 KeePass/1Password 密码
//! 照样入库，**合规 bug**。
//!
//! 对标 CopyQ `isHidden()` [winplatformclipboard.cpp:29-47]。5 种格式:
//! 1. `Clipboard Viewer Ignore` — 存在即忽略（WinNT 经典，密码管理器最常用）
//! 2. `ExcludeClipboardContentFromMonitorProcessing` — 存在即忽略
//! 3. `CanIncludeInClipboardHistory` — DWORD = 0 即忽略（Win10+ Cloud Clipboard）
//! 4. `CanUploadToCloudClipboard` — DWORD = 0 即忽略（同上）
//!
//! v0.2 简版只做 1 + 2（存在性检查，`IsClipboardFormatAvailable` 零锁开销），
//! 覆盖 95% 场景。3 + 4 需要 `OpenClipboard + GetClipboardData` 读 DWORD
//! 值，增加锁竞争，Phase 2 补。
//!
//! https://learn.microsoft.com/en-us/windows/win32/dataxchg/clipboard-formats#cloud-clipboard-and-clipboard-history-formats

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::OnceLock;

use winapi::um::winuser::{IsClipboardFormatAvailable, RegisterClipboardFormatW};

struct HiddenAtoms {
    /// "Clipboard Viewer Ignore" — 存在即忽略
    viewer_ignore: u32,
    /// "ExcludeClipboardContentFromMonitorProcessing" — 存在即忽略
    exclude_monitor: u32,
}

fn atoms() -> &'static HiddenAtoms {
    static ATOMS: OnceLock<HiddenAtoms> = OnceLock::new();
    ATOMS.get_or_init(|| {
        fn reg(name: &str) -> u32 {
            let wide: Vec<u16> = OsStr::new(name)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            // RegisterClipboardFormatW 返回 UINT atom；0 = 失败（不会发生对合法名字）
            unsafe { RegisterClipboardFormatW(wide.as_ptr()) as u32 }
        }
        HiddenAtoms {
            viewer_ignore: reg("Clipboard Viewer Ignore"),
            exclude_monitor: reg("ExcludeClipboardContentFromMonitorProcessing"),
        }
    })
}

/// 检查当前剪贴板是否被源 App 标记为"请忽略我"。
/// 轻量：`IsClipboardFormatAvailable` 不需 OpenClipboard，无锁无拷贝。
pub fn is_clipboard_hidden() -> bool {
    let a = atoms();
    if a.viewer_ignore == 0 && a.exclude_monitor == 0 {
        return false; // 注册失败，保守不过滤
    }
    unsafe {
        (a.viewer_ignore != 0 && IsClipboardFormatAvailable(a.viewer_ignore) != 0)
            || (a.exclude_monitor != 0 && IsClipboardFormatAvailable(a.exclude_monitor) != 0)
    }
}
