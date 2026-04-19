import type { TodayStats } from "./types";

interface Props {
  stats: TodayStats | null;
  onClose: () => void;
}

export default function StatsHeader({ stats, onClose }: Props) {
  const fmt = (n: number | undefined) => (n === undefined ? "--" : String(n));
  return (
    <div className="flex items-center gap-3 px-3 py-2 text-[11px] text-stone-500 border-b border-stone-200 bg-white/70">
      <span>
        今日已记 <strong className="text-stone-800">{fmt(stats?.captured)}</strong>
      </span>
      <span className="text-stone-300">·</span>
      <span>
        拦截 <strong className="text-stone-800">{fmt(stats?.blocked)}</strong>
      </span>
      <span className="text-stone-300">·</span>
      <span className="text-stone-400">离线</span>
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
