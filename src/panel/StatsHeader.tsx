import type { TodayStats } from "./types";

interface Props {
  stats: TodayStats | null;
  isPaused: boolean;
  onClose: () => void;
}

/// 面板顶部条：记录状态 + 今日统计 + 关闭按钮
/// 关键布局：drag-region 不能包 button —— Tauri 2.x 里 button 是 drag-region 子元素时，
/// pointerdown 会被 drag 事件吃掉导致 click 不触发，且 button 也会把 pointerdown 阻挡
/// 让本该能拖的地方拖不动。**按钮必须在 drag-region 容器外**，内层 div 独立标记拖动区。
export default function StatsHeader({ stats, isPaused, onClose }: Props) {
  const fmt = (n: number | undefined) => (n === undefined ? "--" : String(n));
  const blocked = stats?.blocked ?? 0;

  return (
    <div className="flex items-center gap-2 px-3 py-2 text-[11px] text-stone-500 border-b border-stone-200 bg-white/70">
      {/* 拖动区：状态点 + 统计文字。点这块区域可拖动窗口 */}
      <div
        data-tauri-drag-region
        className="flex-1 flex items-center gap-3 cursor-default min-w-0 select-none"
      >
        <span
          className="inline-flex items-center gap-1.5 flex-shrink-0"
          title={
            isPaused
              ? "已暂停 — 新复制的内容不会入库"
              : "正在后台记录剪切板"
          }
        >
          <span
            className={`inline-block w-2 h-2 rounded-full ${
              isPaused ? "bg-amber-400" : "bg-emerald-500 animate-pulse"
            }`}
          />
          <span className={isPaused ? "text-amber-700" : "text-stone-600"}>
            {isPaused ? "已暂停" : "记录中"}
          </span>
        </span>
        <span className="text-stone-300">·</span>
        <span>
          今日 <strong className="text-stone-800">{fmt(stats?.captured)}</strong>
        </span>
        {blocked > 0 && (
          <>
            <span className="text-stone-300">·</span>
            <span title="包含敏感信息（密码/Token/银行卡等），端侧已标记不会上云">
              含敏感 <strong className="text-amber-700">{blocked}</strong>
            </span>
          </>
        )}
      </div>
      {/* × 按钮在 drag-region 外，单独 flex 子元素，click 不会被拖动吃掉 */}
      <button
        onClick={onClose}
        className="w-6 h-6 flex items-center justify-center rounded hover:bg-stone-100 text-stone-400 hover:text-stone-700 transition-colors flex-shrink-0"
        title="关闭 (Esc)"
        aria-label="关闭"
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
          <path d="M2 2L10 10M10 2L2 10" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
      </button>
    </div>
  );
}
