export type PanelTab = "list" | "aggregated";

interface Props {
  tab: PanelTab;
  onChange: (t: PanelTab) => void;
}

/// 聚合 vs 剪贴板 tab 切换。放在 StatsHeader 和 SearchBar 之间。
/// 用浅色背景块分割，不用 border 线（符合 UI 块化原则）
export default function TabBar({ tab, onChange }: Props) {
  return (
    <div className="flex items-center gap-1 px-2 py-1 bg-stone-50 text-[12px]">
      <TabButton active={tab === "list"} onClick={() => onChange("list")}>
        📋 剪贴板
      </TabButton>
      <TabButton active={tab === "aggregated"} onClick={() => onChange("aggregated")}>
        ✨ 聚合
      </TabButton>
    </div>
  );
}

function TabButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={`px-3 py-1 rounded transition-colors ${
        active
          ? "bg-white text-stone-900 font-medium shadow-sm"
          : "text-stone-500 hover:bg-stone-200/60 hover:text-stone-700"
      }`}
    >
      {children}
    </button>
  );
}
