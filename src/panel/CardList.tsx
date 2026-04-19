import { useEffect, useRef } from "react";
import type { ClipboardRow } from "./types";
import CardItem from "./CardItem";

interface Props {
  list: ClipboardRow[];
  selectedIndex: number;
  query: string;
  loading: boolean;
  onSelect: (i: number) => void;
  onCopy: (row: ClipboardRow) => void;
  onForget: (row: ClipboardRow, index: number) => void;
  onEnter: (row: ClipboardRow) => void;
  onTogglePin: (row: ClipboardRow) => void;
  onPreview: (row: ClipboardRow) => void;
}

export default function CardList({
  list,
  selectedIndex,
  query,
  loading,
  onSelect,
  onCopy,
  onForget,
  onEnter,
  onTogglePin,
  onPreview,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = containerRef.current?.querySelector<HTMLElement>(`[data-idx="${selectedIndex}"]`);
    el?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

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
            index={i}
            onMouseEnter={() => onSelect(i)}
            onCopy={onCopy}
            onForget={(r) => onForget(r, i)}
            onEnter={onEnter}
            onTogglePin={onTogglePin}
            onPreview={onPreview}
          />
        </div>
      ))}
    </div>
  );
}
