import type { ClipboardRow } from "./types";
import SessionCard from "./SessionCard";
import { useAggregated } from "./useAggregated";

interface Props {
  enabled: boolean;
  onPasteItem: (row: ClipboardRow) => void;
  onPreviewItem: (row: ClipboardRow) => void;
  onForgetItem: (row: ClipboardRow) => void;
}

/// 聚合 tab 主视图 — 按 session 折叠展示 L1 规则分组结果
export default function AggregatedView({
  enabled,
  onPasteItem,
  onPreviewItem,
  onForgetItem,
}: Props) {
  const agg = useAggregated(enabled);

  if (agg.loading && agg.sessions.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-sm text-stone-400">
        加载聚合中…
      </div>
    );
  }

  if (agg.error) {
    return (
      <div className="flex-1 flex items-center justify-center text-sm text-red-500 px-4 text-center">
        {agg.error}
      </div>
    );
  }

  if (agg.sessions.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-sm text-stone-400 px-4 text-center">
        还没有会话。复制几段内容后 Teamo 会按来源 App 和时间自动聚合。
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto px-2 py-2 space-y-2 bg-white">
      {agg.sessions.map((s) => (
        <SessionCard
          key={s.session_id}
          session={s}
          expanded={agg.expandedId === s.session_id}
          items={agg.expandedId === s.session_id ? agg.expandedItems : []}
          expandLoading={agg.expandedId === s.session_id && agg.expandLoading}
          onToggleExpand={() => void agg.toggleExpand(s.session_id)}
          onPasteItem={onPasteItem}
          onPreviewItem={onPreviewItem}
          onForgetItem={onForgetItem}
        />
      ))}
    </div>
  );
}
