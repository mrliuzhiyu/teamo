import General from "../settings/sections/General";
import Privacy from "../settings/sections/Privacy";
import Cloud from "../settings/sections/Cloud";
import Data from "../settings/sections/Data";
import About from "../settings/sections/About";

interface Props {
  onBack: () => void;
}

/// 在 panel 窗口内渲染的 Settings 视图（复用五个 section 组件）。
/// 不走独立 main window，用户点 ⚙️ 设置直接切视图，按 ← 返回列表。
export default function PanelSettings({ onBack }: Props) {
  return (
    <div className="h-screen flex flex-col bg-stone-50 overflow-hidden">
      <header
        className="flex items-center gap-2 px-3 py-2 border-b border-stone-200 bg-white flex-shrink-0"
        data-tauri-drag-region
      >
        <button
          onClick={onBack}
          className="w-7 h-7 flex items-center justify-center rounded hover:bg-stone-100 text-stone-600 flex-shrink-0"
          title="返回列表"
          aria-label="返回"
        >
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
            <path
              d="M9 2L3 7L9 12"
              stroke="currentColor"
              strokeWidth="1.5"
              fill="none"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </button>
        <div className="text-[13px] font-semibold text-stone-900 tracking-tight">设置</div>
      </header>
      <div className="flex-1 overflow-y-auto py-3">
        <General />
        <Privacy />
        <Cloud />
        <Data />
        <About />
      </div>
    </div>
  );
}
