import type { TodayStats } from "./types";

interface Props {
  stats: TodayStats | null;
}

export default function StatsHeader({ stats }: Props) {
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
      <span>
        上云 <strong className="text-stone-800">--</strong>
      </span>
    </div>
  );
}
