import type { TodayStats } from "./types";

interface Props {
  stats: TodayStats | null;
  isPaused: boolean;
  onClose: () => void;
}

/// 快速面板顶部：今日统计 + 记录状态点 + × 关闭按钮
/// 整条 Header 带 data-tauri-drag-region —— 因为面板 decorations=false 没系统标题栏，
/// 必须显式给一块区域让用户拖动窗口。子元素 button 会自动 opt-out 不触发拖动。
export default function StatsHeader({ stats, isPaused, onClose }: Props) {
  const fmt = (n: number | undefined) => (n === undefined ? "--" : String(n));

  return (
    <div
      className="flex items-center gap-3 px-3 py-2 text-[11px] text-stone-500 border-b border-stone-200 bg-white/70 cursor-default"
      data-tauri-drag-region
    >
      <span
        className="inline-flex items-center gap-1.5"
        title={isPaused ? "已暂停记录 — 新复制的内容不会入库" : "正在记录剪切板"}
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
      <span className="text-stone-300">·</span>
      <span>
        拦截 <strong className="text-stone-800">{fmt(stats?.blocked)}</strong>
      </span>
      <button
        onClick={onClose}
        className="ml-auto w-6 h-6 flex items-center justify-center rounded hover:bg-stone-100 text-stone-400 hover:text-stone-700 transition-colors"
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
