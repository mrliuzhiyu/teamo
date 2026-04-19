import type { ClipboardRow } from "./types";
import type { SessionSummary } from "./useAggregated";
import { formatPreview, formatRelativeTime } from "./utils";

interface Props {
  session: SessionSummary;
  expanded: boolean;
  items: ClipboardRow[];
  expandLoading: boolean;
  onToggleExpand: () => void;
  onPasteItem: (row: ClipboardRow) => void;
  onPreviewItem: (row: ClipboardRow) => void;
  onForgetItem: (row: ClipboardRow) => void;
}

/// 聚合 tab 一个 session 卡片：
/// - 收起态：来源 App + 预览 + 片段数 + 时间跨度（像 Gmail 会话线程）
/// - 展开态：session 内所有 items（类似文件夹展开）
///
/// 暂不做组级操作（上云/pin 整组），R3 上云时补。
export default function SessionCard({
  session,
  expanded,
  items,
  expandLoading,
  onToggleExpand,
  onPasteItem,
  onPreviewItem,
  onForgetItem,
}: Props) {
  const timeSpan = formatTimeSpan(session.started_at, session.ended_at);

  return (
    <div className="rounded-lg bg-stone-100 overflow-hidden">
      {/* 卡片头：点击展开 */}
      <button
        onClick={onToggleExpand}
        className="w-full text-left px-3 py-2.5 flex items-start gap-2 hover:bg-stone-200 transition-colors"
      >
        <div className="flex-shrink-0 mt-0.5 text-stone-400">
          <svg
            width="10"
            height="10"
            viewBox="0 0 10 10"
            className={`transition-transform ${expanded ? "rotate-90" : ""}`}
          >
            <path d="M3 2L7 5L3 8" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </div>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 text-[11px] text-stone-500 mb-0.5">
            {session.primary_source_app && (
              <span className="text-stone-700 font-medium truncate max-w-[140px]">
                {session.primary_source_app}
              </span>
            )}
            <span className="px-1.5 py-0.5 rounded bg-stone-200/60 text-stone-600 flex-shrink-0">
              {session.item_count} 条
            </span>
            {session.has_image && (
              <span className="px-1.5 py-0.5 rounded bg-emerald-100/60 text-emerald-700 flex-shrink-0" title="含图片">
                📷
              </span>
            )}
            {session.has_sensitive && (
              <span className="px-1.5 py-0.5 rounded bg-amber-100/60 text-amber-700 flex-shrink-0" title="含敏感内容">
                🔒
              </span>
            )}
            <span className="ml-auto text-stone-400 flex-shrink-0" title={new Date(session.ended_at).toLocaleString()}>
              {timeSpan}
            </span>
          </div>
          <div className="text-sm text-stone-800 line-clamp-2 break-all">
            {session.first_preview}
          </div>
        </div>
      </button>

      {/* 展开态：items 子列表 */}
      {expanded && (
        <div className="bg-white border-t border-stone-200 divide-y divide-stone-100">
          {expandLoading ? (
            <div className="px-4 py-3 text-[12px] text-stone-400">加载中…</div>
          ) : items.length === 0 ? (
            <div className="px-4 py-3 text-[12px] text-stone-400">此 session 无 items</div>
          ) : (
            items.map((row) => (
              <SessionItemRow
                key={row.id}
                row={row}
                onPaste={() => onPasteItem(row)}
                onPreview={() => onPreviewItem(row)}
                onForget={() => onForgetItem(row)}
              />
            ))
          )}
        </div>
      )}
    </div>
  );
}

function SessionItemRow({
  row,
  onPaste,
  onPreview,
  onForget,
}: {
  row: ClipboardRow;
  onPaste: () => void;
  onPreview: () => void;
  onForget: () => void;
}) {
  const isImage = row.content_type === "image";
  const preview = formatPreview(row, 120);
  return (
    <div className="px-3 py-2 flex items-start gap-2 hover:bg-stone-50 group">
      <div className="flex-1 min-w-0 text-[12px] text-stone-700 line-clamp-2 break-all">
        {isImage
          ? `🖼️ ${row.image_width ?? "?"} × ${row.image_height ?? "?"}`
          : preview}
      </div>
      <div className="flex-shrink-0 flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
        <button
          onClick={onPreview}
          className="text-[10px] px-1.5 py-0.5 rounded bg-white hover:bg-stone-100 text-stone-500 shadow-sm"
          title="预览"
        >
          预览
        </button>
        <button
          onClick={onPaste}
          className="text-[10px] px-1.5 py-0.5 rounded bg-white hover:bg-stone-100 text-stone-500 shadow-sm"
          title="粘贴到当前窗口"
        >
          粘贴
        </button>
        <button
          onClick={onForget}
          className="text-[10px] px-1.5 py-0.5 rounded bg-white hover:bg-red-50 hover:text-red-600 text-stone-500 shadow-sm"
          title="忘记这条"
        >
          忘记
        </button>
      </div>
      <span className="flex-shrink-0 text-[10px] text-stone-400 self-center" title={new Date(row.captured_at).toLocaleString()}>
        {formatRelativeTime(row.captured_at)}
      </span>
    </div>
  );
}

/// 时间跨度友好展示
/// 1 分钟内：刚刚
/// 同一小时：N 分钟前
/// 同一天：HH:mm-HH:mm
/// 更早：用 formatRelativeTime 统一
function formatTimeSpan(startMs: number, endMs: number): string {
  const span = endMs - startMs;
  const now = Date.now();
  const sinceEnd = now - endMs;

  if (sinceEnd < 60_000) return "刚刚";
  if (sinceEnd < 60 * 60_000) return `${Math.floor(sinceEnd / 60_000)} 分钟前`;

  if (span < 60_000) {
    // 单时刻
    const d = new Date(endMs);
    return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  }

  const startD = new Date(startMs);
  const endD = new Date(endMs);
  const sameDay = startD.toDateString() === endD.toDateString();
  if (sameDay) {
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${pad(startD.getHours())}:${pad(startD.getMinutes())} – ${pad(endD.getHours())}:${pad(endD.getMinutes())}`;
  }

  // 跨天：简化显示 N 天前
  const days = Math.floor(sinceEnd / (24 * 60 * 60_000));
  return `${days} 天前`;
}
