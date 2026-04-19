interface Props {
  isPaused: boolean;
  onTogglePause: () => void;
  onOpenSettings: () => void;
}

/// 面板底部操作栏：暂停 toggle + 设置入口。
/// v0.2 简化：之前的 5分钟/1小时/手动 下拉菜单删掉 — 实际使用 95% 是"手动恢复"，
/// 时间选项是过度设计；用户要记录敏感信息时点一下暂停、做完再点一下恢复。
export default function ActionBar({ isPaused, onTogglePause, onOpenSettings }: Props) {
  return (
    <div className="px-2 py-1.5 border-t border-stone-200 bg-stone-50 flex items-center gap-1 text-[11px]">
      <button
        onClick={onTogglePause}
        className="px-2 py-1 rounded hover:bg-stone-200 text-stone-600 transition-colors"
        title={isPaused ? "恢复记录" : "暂停记录（点击恢复）"}
      >
        {isPaused ? "▶ 继续记录" : "⏸ 暂停记录"}
      </button>
      <button
        onClick={onOpenSettings}
        className="px-2 py-1 rounded hover:bg-stone-200 text-stone-600 transition-colors ml-auto"
        title="打开设置"
      >
        ⚙️ 设置
      </button>
    </div>
  );
}
