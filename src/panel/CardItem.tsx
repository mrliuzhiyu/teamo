import type { MouseEvent } from "react";
import type { ClipboardRow } from "./types";
import { formatPreview, formatRelativeTime, getStateBadge, highlightMatches } from "./utils";

interface Props {
  row: ClipboardRow;
  selected: boolean;
  query: string;
  onMouseEnter: () => void;
  onCopy: (row: ClipboardRow) => void;
  onForget: (row: ClipboardRow) => void;
}

const badgeClass: Record<string, string> = {
  local: "bg-stone-100 text-stone-500",
  cloud: "bg-emerald-50 text-emerald-700",
  blocked: "bg-amber-50 text-amber-700",
};

export default function CardItem({ row, selected, query, onMouseEnter, onCopy, onForget }: Props) {
  const preview = formatPreview(row);
  const badge = getStateBadge(row);
  const parts = row.sensitive_type ? [{ text: preview, hit: false }] : highlightMatches(preview, query);

  const stopAnd = (fn: () => void) => (e: MouseEvent) => {
    e.stopPropagation();
    e.preventDefault();
    fn();
  };

  return (
    <div
      data-selected={selected}
      onMouseEnter={onMouseEnter}
      className={`relative px-3 py-2.5 rounded-lg border cursor-pointer transition-all group ${
        selected
          ? "bg-stone-50 border-stone-300 shadow-sm"
          : "bg-white border-stone-200 hover:bg-stone-50/60 hover:border-stone-300"
      }`}
    >
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
      <div className="mt-1.5 flex items-center gap-2 text-[11px] text-stone-400">
        <span className={`px-1.5 py-0.5 rounded ${badgeClass[badge.tone]}`}>{badge.label}</span>
        {row.source_app && <span className="truncate max-w-[120px]">{row.source_app}</span>}
        <span className="ml-auto">{formatRelativeTime(row.captured_at)}</span>
      </div>
      {selected && (
        <div className="absolute right-2 top-2 flex items-center gap-1">
          <button
            onClick={stopAnd(() => onCopy(row))}
            className="px-2 py-0.5 text-[11px] bg-white border border-stone-200 rounded hover:bg-stone-100 text-stone-600 shadow-sm"
            title="仅复制到剪切板（不关闭面板，不自动粘贴）"
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
      )}
    </div>
  );
}
