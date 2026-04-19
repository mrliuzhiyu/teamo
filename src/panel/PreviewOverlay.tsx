import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import type { ClipboardRow } from "./types";
import { formatPreview, formatRelativeTime } from "./utils";

interface Props {
  row: ClipboardRow;
  onClose: () => void;
}

/// 全文 / 全尺寸图片预览浮层。
/// 触发：选中行按 Space / F3 / 右键「查看全文」。
/// 对标 Ditto F3 全尺寸 + CopyQ F7 Preview dock（做深图片查看）。
///
/// 文本：font-mono + whitespace-pre-wrap + break-all 保留格式 + 长单词换行
/// 图片：invoke get_image_data_url 拿原图 data URL；onLoad 取 naturalWidth/Height 显示尺寸
export default function PreviewOverlay({ row, onClose }: Props) {
  const isImage = row.content_type === "image" && row.image_path;
  const [imgDataUrl, setImgDataUrl] = useState<string | null>(null);
  const [imgDims, setImgDims] = useState<{ w: number; h: number } | null>(null);
  const [imgLoading, setImgLoading] = useState(false);

  useEffect(() => {
    if (!isImage) return;
    let cancelled = false;
    setImgLoading(true);
    // maxSize=null 明确要原图（不缩放），PreviewOverlay 要全尺寸看清内容
    invoke<string>("get_image_data_url", { id: row.id, maxSize: null })
      .then((url) => {
        if (!cancelled) {
          setImgDataUrl(url);
          setImgLoading(false);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setImgDataUrl(null);
          setImgLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [row.id, isImage]);

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

  const textContent = isImage ? null : row.content ?? formatPreview(row);

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
      <div className="bg-white rounded-lg shadow-2xl border border-stone-200 max-w-[95%] w-full max-h-[90vh] flex flex-col overflow-hidden">
        <div className="px-3 py-2 border-b border-stone-200 flex items-center gap-2 text-[11px] text-stone-500 flex-shrink-0 bg-stone-50">
          <span className="font-semibold text-stone-700">{isImage ? "图片预览" : "全文预览"}</span>
          {imgDims && (
            <span className="text-stone-500">
              {imgDims.w} × {imgDims.h}
            </span>
          )}
          {row.source_app && <span className="text-stone-400">· {row.source_app}</span>}
          <span className="text-stone-400">· {formatRelativeTime(row.captured_at)}</span>
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
        <div className="overflow-auto flex-1 flex items-center justify-center bg-stone-50/50">
          {isImage ? (
            imgLoading ? (
              <div className="text-stone-400 text-[12px] py-8">加载图片中…</div>
            ) : imgDataUrl ? (
              <img
                src={imgDataUrl}
                alt="截图"
                onLoad={(e) => {
                  const img = e.currentTarget;
                  setImgDims({ w: img.naturalWidth, h: img.naturalHeight });
                }}
                className="max-w-full max-h-full object-contain"
              />
            ) : (
              <div className="text-red-500 text-[12px] py-8">图片加载失败 — 文件可能已丢失</div>
            )
          ) : (
            <div className="w-full px-4 py-3 text-[12px] text-stone-800 whitespace-pre-wrap break-all font-mono leading-relaxed self-start">
              {textContent}
            </div>
          )}
        </div>
      </div>
    </div>,
    document.body,
  );
}
