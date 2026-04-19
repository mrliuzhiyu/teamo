import { useEffect } from "react";
import { createPortal } from "react-dom";
import type { ClipboardRow } from "./types";
import { formatPreview, formatRelativeTime } from "./utils";

interface Props {
  row: ClipboardRow;
  onClose: () => void;
}

/// 长文本预览浮层 — 选中某条按 Space / F3 或右键菜单"查看全文"触发。
/// 对标 Ditto F3 / CopyQ F7 Preview dock / Maccy hover tooltip。
/// Esc / 点外部关闭；data-teamo-dialog 标记避免 PanelApp 的 Esc handler 把 Esc 吃掉
/// 触发 hidePanel。
export default function PreviewOverlay({ row, onClose }: Props) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        onClose();
      }
    };
    // capture phase 抢在 PanelApp window-level listener 前处理
    window.addEventListener("keydown", onKey, { capture: true });
    return () => window.removeEventListener("keydown", onKey, { capture: true });
  }, [onClose]);

  const isImage = row.content_type === "image";
  const preview = isImage ? null : row.content ?? formatPreview(row);

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      data-teamo-dialog="open"
      className="fixed inset-0 z-[80] flex items-center justify-center bg-black/30 backdrop-blur-[1px] p-4"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="bg-white rounded-lg shadow-2xl border border-stone-200 max-w-[95%] w-full max-h-[85vh] flex flex-col overflow-hidden">
        <div className="px-3 py-2 border-b border-stone-200 flex items-center gap-2 text-[11px] text-stone-500 flex-shrink-0 bg-stone-50">
          <span className="font-semibold text-stone-700">全文预览</span>
          {row.source_app && <span>· {row.source_app}</span>}
          <span>· {formatRelativeTime(row.captured_at)}</span>
          <span className="ml-auto text-[10px]">Esc 关闭</span>
          <button
            onClick={onClose}
            className="w-5 h-5 flex items-center justify-center rounded hover:bg-stone-200 text-stone-400 hover:text-stone-700"
            aria-label="关闭"
          >
            <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
              <path d="M2 2L8 8M8 2L2 8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
            </svg>
          </button>
        </div>
        <div className="px-4 py-3 overflow-auto flex-1 text-[12px] text-stone-800 whitespace-pre-wrap break-all font-mono leading-relaxed">
          {preview ?? "[图片内容 — 在列表直接查看缩略图]"}
        </div>
      </div>
    </div>,
    document.body,
  );
}
