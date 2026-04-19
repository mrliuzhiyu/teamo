import { useEffect, useRef, useState, type MouseEvent as ReactMouseEvent } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import type { ClipboardRow } from "./types";
import { formatPreview, formatRelativeTime, getStateBadge, highlightMatches } from "./utils";

interface Props {
  row: ClipboardRow;
  selected: boolean;
  query: string;
  /// 列表 index（0-based）。前 9 条显示数字徽章，对应 Ctrl+1..9 快选
  index: number;
  onMouseEnter: () => void;
  onCopy: (row: ClipboardRow) => void;
  onForget: (row: ClipboardRow) => void;
  onEnter: (row: ClipboardRow) => void;
  onTogglePin: (row: ClipboardRow) => void;
  onPreview: (row: ClipboardRow) => void;
}

const badgeClass: Record<string, string> = {
  local: "bg-stone-100 text-stone-500",
  cloud: "bg-emerald-50 text-emerald-700",
  blocked: "bg-amber-50 text-amber-700",
};

export default function CardItem({
  row,
  selected,
  query,
  index,
  onMouseEnter,
  onCopy,
  onForget,
  onEnter,
  onTogglePin,
  onPreview,
}: Props) {
  const isImage = row.content_type === "image" && row.image_path;
  const isPinned = row.pinned_at !== null && row.pinned_at !== undefined;
  const showNumberBadge = index < 9;
  const badge = getStateBadge(row);
  const preview = formatPreview(row);
  const parts = row.sensitive_type ? [{ text: preview, hit: false }] : highlightMatches(preview, query);

  // 图片缩略图：一次 invoke 读 data URL，缓存在 state
  const [thumbnail, setThumbnail] = useState<string | null>(null);
  useEffect(() => {
    if (!isImage) return;
    let cancelled = false;
    invoke<string>("get_image_data_url", { id: row.id })
      .then((url) => !cancelled && setThumbnail(url))
      .catch(() => !cancelled && setThumbnail(null));
    return () => {
      cancelled = true;
    };
  }, [row.id, isImage]);

  // 右键菜单
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!menu) return;
    const onDocDown = (e: Event) => {
      if (!menuRef.current?.contains(e.target as Node)) setMenu(null);
    };
    document.addEventListener("mousedown", onDocDown);
    document.addEventListener("contextmenu", onDocDown, { capture: true });
    return () => {
      document.removeEventListener("mousedown", onDocDown);
      document.removeEventListener("contextmenu", onDocDown, { capture: true });
    };
  }, [menu]);

  const handleContextMenu = (e: ReactMouseEvent) => {
    e.preventDefault();
    // 菜单预估 140×100，超出窗口边界则向左/向上翻转
    const W = window.innerWidth;
    const H = window.innerHeight;
    const x = e.clientX + 140 > W ? e.clientX - 140 : e.clientX;
    const y = e.clientY + 100 > H ? e.clientY - 100 : e.clientY;
    setMenu({ x, y });
  };

  const stopAnd = (fn: () => void) => (e: ReactMouseEvent) => {
    e.stopPropagation();
    e.preventDefault();
    fn();
  };

  const closeMenuAnd = (fn: () => void) => () => {
    setMenu(null);
    fn();
  };

  return (
    <div
      data-selected={selected}
      onMouseEnter={onMouseEnter}
      onDoubleClick={() => onEnter(row)}
      onContextMenu={handleContextMenu}
      className={`relative px-3 py-2.5 rounded-lg border cursor-pointer transition-all group ${
        selected
          ? "bg-stone-50 border-stone-300 shadow-sm"
          : isPinned
            ? "bg-amber-50/30 border-amber-200/60 hover:bg-amber-50/50"
            : "bg-white border-stone-200 hover:bg-stone-50/60 hover:border-stone-300"
      }`}
      title="双击粘贴 · Space 预览 · 右键更多操作"
    >
      {/* 左上：置顶图标（pin 项）或数字徽章（前 9 条） */}
      {isPinned ? (
        <span
          className="absolute left-1 top-1 text-amber-600 flex-shrink-0"
          title="已置顶"
          aria-label="已置顶"
        >
          <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor">
            <path d="M5 0.5L6 3L8.5 3.5L6.7 5.3L7.2 8L5 6.7L2.8 8L3.3 5.3L1.5 3.5L4 3L5 0.5Z" />
          </svg>
        </span>
      ) : showNumberBadge ? (
        <span
          className="absolute left-1 top-1 text-[8px] text-stone-400 bg-stone-100 rounded-sm px-1 leading-[1.4] select-none"
          title={`Ctrl+${index + 1} 快速粘贴`}
        >
          {index + 1}
        </span>
      ) : null}
      {isImage ? (
        <div className="flex items-start gap-3 pr-24">
          <div className="flex-shrink-0 w-16 h-16 rounded border border-stone-200 bg-stone-100 overflow-hidden flex items-center justify-center">
            {thumbnail ? (
              <img src={thumbnail} alt="截图" className="max-w-full max-h-full object-contain" />
            ) : (
              <span className="text-[10px] text-stone-400">加载中</span>
            )}
          </div>
          <div className="flex-1 min-w-0 text-[12px] text-stone-500">
            <div className="text-stone-700">截图</div>
            <div className="mt-0.5 truncate">{row.image_path}</div>
          </div>
        </div>
      ) : (
        <div className="text-sm text-stone-800 break-all line-clamp-2 pr-24">
          {parts.map((p, i) =>
            p.hit ? (
              <mark key={i} className="bg-amber-100 text-stone-900 rounded px-0.5">
                {p.text}
              </mark>
            ) : (
              <span key={i}>{p.text}</span>
            ),
          )}
        </div>
      )}

      <div className="mt-1.5 flex items-center gap-2 text-[11px] text-stone-400">
        <span className={`px-1.5 py-0.5 rounded ${badgeClass[badge.tone]}`}>{badge.label}</span>
        {row.source_app && <span className="truncate max-w-[120px]">{row.source_app}</span>}
        <span className="ml-auto" title={new Date(row.captured_at).toLocaleString()}>
          {formatRelativeTime(row.captured_at)}
        </span>
      </div>

      {/* 操作按钮：hover 或选中都显示（鼠标党 / 键盘党都友好） */}
      <div
        className={`absolute right-2 top-2 flex items-center gap-1 transition-opacity ${
          selected ? "opacity-100" : "opacity-0 group-hover:opacity-100"
        }`}
      >
        <button
          onClick={stopAnd(() => onCopy(row))}
          className="px-2 py-0.5 text-[11px] bg-white border border-stone-200 rounded hover:bg-stone-100 text-stone-600 shadow-sm"
          title="仅复制（不关闭、不自动粘贴）"
        >
          仅复制
        </button>
        <button
          onClick={stopAnd(() => onForget(row))}
          className="px-2 py-0.5 text-[11px] bg-white border border-stone-200 rounded hover:bg-red-50 hover:border-red-200 hover:text-red-600 text-stone-600 shadow-sm"
          title="忘记这条"
        >
          忘记
        </button>
      </div>

      {/* 右键菜单 — portal 到 body 避免被列表 overflow 裁切 */}
      {menu &&
        createPortal(
          <div
            ref={menuRef}
            className="fixed z-50 min-w-[160px] bg-white border border-stone-200 rounded-md shadow-lg py-1 text-[12px]"
            style={{ left: menu.x, top: menu.y }}
          >
            <button
              onClick={closeMenuAnd(() => onEnter(row))}
              className="w-full text-left px-3 py-1.5 hover:bg-stone-100 text-stone-700"
            >
              粘贴到当前窗口
            </button>
            <button
              onClick={closeMenuAnd(() => onCopy(row))}
              className="w-full text-left px-3 py-1.5 hover:bg-stone-100 text-stone-700"
            >
              仅复制到剪切板
            </button>
            <button
              onClick={closeMenuAnd(() => onPreview(row))}
              className="w-full text-left px-3 py-1.5 hover:bg-stone-100 text-stone-700"
            >
              查看全文 <span className="text-stone-400 text-[10px] ml-1">Space</span>
            </button>
            <div className="my-1 border-t border-stone-100" />
            <button
              onClick={closeMenuAnd(() => onTogglePin(row))}
              className="w-full text-left px-3 py-1.5 hover:bg-stone-100 text-stone-700"
            >
              {isPinned ? "取消置顶" : "置顶到最前"}
            </button>
            <div className="my-1 border-t border-stone-100" />
            <button
              onClick={closeMenuAnd(() => onForget(row))}
              className="w-full text-left px-3 py-1.5 hover:bg-red-50 text-red-600"
            >
              忘记这条
            </button>
          </div>,
          document.body,
        )}
    </div>
  );
}
