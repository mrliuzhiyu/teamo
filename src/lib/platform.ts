/**
 * 平台感知常量 —— 跨 UI 组件共享，避免每个组件各自 navigator.platform 判断
 * 造成文案不一致。
 *
 * 用法：
 *   import { isMac, shortcutLabel, enterHintLabel } from "@/lib/platform";
 *
 * Tauri 的 webview 在 Windows 是 WebView2、macOS 是 WKWebView、Linux 是 WebKitGTK。
 * `navigator.platform` 三处都可靠（都包含 "Win32" / "MacIntel" / "Linux"）。
 */

function detectIsMac(): boolean {
  if (typeof navigator === "undefined") return false;
  return (
    navigator.platform.startsWith("Mac") || navigator.platform.includes("Mac")
  );
}

export const isMac = detectIsMac();

/// 全局快捷键展示文案
export const shortcutLabel = isMac ? "⌘⇧V" : "Ctrl+Shift+V";

/// Enter 键在快速面板里的行为提示文案
/// Windows：`paste_to_previous` 会真的模拟 Ctrl+V 粘贴到目标 App → "粘贴"
/// 非 Windows：目前回退为"复制 + 关闭"，用户需手动粘贴 → 明说
export const enterHintLabel = isMac ? "复制并关闭" : "粘贴";

/// 系统级粘贴键提示（用在"手动粘贴"引导文案里）
export const pasteShortcut = isMac ? "⌘V" : "Ctrl+V";
