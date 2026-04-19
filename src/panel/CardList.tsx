import { useEffect, useRef } from "react";
import type { ClipboardRow } from "./types";
import CardItem from "./CardItem";

interface Props {
  list: ClipboardRow[];
  selectedIndex: number;
  query: string;
  loading: boolean;
  hasMore: boolean;
  loadingMore: boolean;
  onSelect: (i: number) => void;
  onCopy: (row: ClipboardRow) => void;
  onForget: (row: ClipboardRow, index: number) => void;
  onEnter: (row: ClipboardRow) => void;
  onTogglePin: (row: ClipboardRow) => void;
  onPreview: (row: ClipboardRow) => void;
  onLoadMore: () => void;
}

export default function CardList({
  list,
  selectedIndex,
  query,
  loading,
  hasMore,
  loadingMore,
  onSelect,
  onCopy,
  onForget,
  onEnter,
  onTogglePin,
  onPreview,
  onLoadMore,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const sentinelRef = useRef<HTMLDivElement>(null);
  // 标记：下一次 selectedIndex 变化由 mouse hover 触发 → 不做 scrollIntoView。
  // 原因：hover 时鼠标已经指在目标卡片上，再触发 scrollIntoView 会让视口轻微
  // 滚动，改变鼠标相对卡片的坐标，可能反向触发 mouseenter/leave 抖动。
  // 只有键盘导航（↑↓ → setSelectedIndex 在 PanelApp keydown handler）需要滚动。
  const skipScrollRef = useRef(false);

  useEffect(() => {
    if (skipScrollRef.current) {
      skipScrollRef.current = false;
      return;
    }
    const el = containerRef.current?.querySelector<HTMLElement>(`[data-idx="${selectedIndex}"]`);
    el?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  // IntersectionObserver：sentinel 进视野 → 触发加载下一页。
  // threshold 0.1 = 刚进就触发，不用等完全可见；root = containerRef 在 panel 内滚动
  useEffect(() => {
    const sentinel = sentinelRef.current;
    const root = containerRef.current;
    if (!sentinel || !root || !hasMore) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) {
          onLoadMore();
        }
      },
      { root, threshold: 0.1 },
    );
    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [hasMore, onLoadMore]);

  if (loading) {
    return <div className="flex-1 flex items-center justify-center text-sm text-stone-400">加载中...</div>;
  }

  if (list.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-sm text-stone-400">
        {query ? "没找到匹配的内容" : "还没有记录，复制点什么试试"}
      </div>
    );
  }

  return (
    <div ref={containerRef} className="flex-1 overflow-y-auto px-2 py-2 space-y-1.5 bg-stone-50/40">
      {list.map((row, i) => (
        <div key={row.id} data-idx={i}>
          <CardItem
            row={row}
            selected={i === selectedIndex}
            query={query}
            onMouseEnter={() => {
              skipScrollRef.current = true;
              onSelect(i);
            }}
            onCopy={onCopy}
            onForget={(r) => onForget(r, i)}
            onEnter={onEnter}
            onTogglePin={onTogglePin}
            onPreview={onPreview}
          />
        </div>
      ))}
      {/* 分页 sentinel：进入视野自动加载下一页。hasMore=false 则不渲染（到底了） */}
      {hasMore && (
        <div ref={sentinelRef} className="h-6 flex items-center justify-center text-[11px] text-stone-400">
          {loadingMore ? "加载中…" : "滚动加载更多"}
        </div>
      )}
    </div>
  );
}
