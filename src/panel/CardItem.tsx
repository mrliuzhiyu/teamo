import { useEffect, useRef, useState, type MouseEvent as ReactMouseEvent } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import type { ClipboardRow } from "./types";
import { formatPreview, formatRelativeTime, formatSource, getStateBadge, highlightMatches } from "./utils";

interface Props {
  row: ClipboardRow;
  selected: boolean;
  query: string;
  onMouseEnter: () => void;
  onCopy: (row: ClipboardRow) => void;
  onForget: (row: ClipboardRow) => void;
  onEnter: (row: ClipboardRow) => void;
  onTogglePin: (row: ClipboardRow) => void;
  onPreview: (row: ClipboardRow) => void;
}

const badgeClass: Record<string, string> = {
  local: "bg-stone-200/60 text-stone-500",
  cloud: "bg-emerald-100/60 text-emerald-700",
  blocked: "bg-amber-100/60 text-amber-700",
};

export default function CardItem({
  row,
  selected,
  query,
  onMouseEnter,
  onCopy,
  onForget,
  onEnter,
  onTogglePin,
  onPreview,
}: Props) {
  const isImage = row.content_type === "image" && row.image_path;
  const isPinned = row.pinned_at !== null && row.pinned_at !== undefined;
  const badge = getStateBadge(row);
  const preview = formatPreview(row, 80, query);
  const parts = row.sensitive_type ? [{ text: preview, hit: false }] : highlightMatches(preview, query);

  // 图片缩略图：仅 1 次 invoke 拿 128 缩略；原图尺寸直接从 row.image_width/height 读
  // （migration 006 起由 ingest 存到 DB，避免 20 条列表 × 2 invoke × 200MB IPC 浪费）
  const [thumbnail, setThumbnail] = useState<string | null>(null);
  useEffect(() => {
    if (!isImage) return;
    let cancelled = false;
    invoke<string>("get_image_data_url", { id: row.id, maxSize: 128 })
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
    const W = window.innerWidth;
    const H = window.innerHeight;
    const x = e.clientX + 140 > W ? e.clientX - 140 : e.clientX;
    const y = e.clientY + 140 > H ? e.clientY - 140 : e.clientY;
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

  // 块化设计：
  // - 卡片去 border，用浅灰 stone-100 填充（列表容器是 white 会让卡片"凸"出）
  // - selected 态：stone-200 更深灰 + 淡 ring 提示键盘焦点
  // - pinned 态：amber-100 暖色（保留 pin 识别度）
  // - 鼠标党主动作用右键菜单 "粘贴"；双击不再关闭 panel 避免快速点击误触发
  // tailwind stone 没 150 色阶；selected 用 stone-200 + ring 区分，hover 态也用 stone-200
  //（hover 时和 selected 相同颜色视觉 OK，因为 selected 带 ring 可区分）
  const baseBg = selected
    ? "bg-stone-200"
    : isPinned
      ? "bg-amber-100 hover:bg-amber-200/70"
      : "bg-stone-100 hover:bg-stone-200";

  return (
    <div
      data-selected={selected}
      onMouseEnter={onMouseEnter}
      onContextMenu={handleContextMenu}
      className={`relative px-3 py-2.5 rounded-lg transition-colors group ${baseBg} ${
        selected ? "ring-1 ring-stone-300" : ""
      }`}
      title="右键更多操作 · Enter 粘贴 · Space 预览"
    >
      {isImage ? (
        <div className="flex items-start gap-3 pr-20">
          {/* 点击缩略图 → 直接打开全尺寸预览（之前用户反馈"看不了截的图"） */}
          <button
            onClick={stopAnd(() => onPreview(row))}
            className="flex-shrink-0 w-16 h-16 rounded bg-white overflow-hidden flex items-center justify-center hover:ring-2 hover:ring-stone-300 transition-all cursor-zoom-in"
            title="点击查看全尺寸"
          >
            {thumbnail ? (
              <img src={thumbnail} alt="截图" className="max-w-full max-h-full object-contain" />
            ) : (
              <span className="text-[10px] text-stone-400">加载中</span>
            )}
          </button>
          <div className="flex-1 min-w-0 text-[12px] text-stone-600">
            <div className="text-stone-800 font-medium">截图</div>
            <div className="mt-0.5 text-stone-500 truncate">
              {row.image_width && row.image_height
                ? `${row.image_width} × ${row.image_height}`
                : "尺寸未知"}
              {formatSource(row, 40) && ` · 来自 ${formatSource(row, 40)}`}
            </div>
          </div>
        </div>
      ) : (
        <div className="text-sm text-stone-800 break-all line-clamp-2 pr-20">
          {parts.map((p, i) =>
            p.hit ? (
              <mark key={i} className="bg-amber-200 text-stone-900 rounded px-0.5">
                {p.text}
              </mark>
            ) : (
              <span key={i}>{p.text}</span>
            ),
          )}
        </div>
      )}

      <div className="mt-1.5 flex items-center gap-2 text-[11px] text-stone-500">
        <span className={`px-1.5 py-0.5 rounded ${badgeClass[badge.tone]}`}>{badge.label}</span>
        {!isImage && formatSource(row, 50) && (
          <span
            className="truncate max-w-[180px]"
            title={row.source_title ?? row.source_app ?? ""}
          >
            {formatSource(row, 50)}
          </span>
        )}
        <span className="ml-auto" title={new Date(row.captured_at).toLocaleString()}>
          {formatRelativeTime(row.captured_at)}
        </span>
      </div>

      {/* 操作按钮：hover 或选中都显示 */}
      <div
        className={`absolute right-2 top-2 flex items-center gap-1 transition-opacity ${
          selected ? "opacity-100" : "opacity-0 group-hover:opacity-100"
        }`}
      >
        <button
          onClick={stopAnd(() => onCopy(row))}
          className="px-2 py-0.5 text-[11px] bg-white rounded hover:bg-stone-50 text-stone-600 shadow-sm"
          title="仅复制（不关闭、不自动粘贴）"
        >
          仅复制
        </button>
        <button
          onClick={stopAnd(() => onForget(row))}
          className="px-2 py-0.5 text-[11px] bg-white rounded hover:bg-red-50 hover:text-red-600 text-stone-600 shadow-sm"
          title="删除这条"
        >
          删除
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
              粘贴到当前窗口 <span className="text-stone-400 text-[10px] ml-1">Enter</span>
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
              {isImage ? "查看大图" : "查看全文"}{" "}
              <span className="text-stone-400 text-[10px] ml-1">Space</span>
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
              删除这条
            </button>
          </div>,
          document.body,
        )}
    </div>
  );
}
