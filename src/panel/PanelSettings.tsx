import General from "../settings/sections/General";
import Privacy from "../settings/sections/Privacy";
import Cloud from "../settings/sections/Cloud";
import Data from "../settings/sections/Data";
import About from "../settings/sections/About";
import ErrorBoundary from "../lib/ErrorBoundary";

interface Props {
  onBack: () => void;
}

/// panel 窗口内的 Settings 视图（复用五个 section）。
/// 按 ← 或 Esc 返回列表。header 布局注意：返回 button 必须在 drag-region 容器外，
/// 否则 Tauri 2.x 会把 pointerdown 归为拖动导致 button 不响应。
export default function PanelSettings({ onBack }: Props) {
  return (
    <div className="h-screen flex flex-col bg-stone-50 overflow-hidden">
      <header className="flex items-center gap-2 px-3 py-2 border-b border-stone-200 bg-white flex-shrink-0">
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
        {/* drag 区：标题部分独立 div，按钮不在其中 */}
        <div
          data-tauri-drag-region
          className="flex-1 text-[13px] font-semibold text-stone-900 tracking-tight cursor-default select-none"
        >
          设置
        </div>
      </header>
      <div className="flex-1 overflow-y-auto py-3">
        <ErrorBoundary label="General">
          <General />
        </ErrorBoundary>
        <ErrorBoundary label="Privacy">
          <Privacy />
        </ErrorBoundary>
        <ErrorBoundary label="Cloud">
          <Cloud />
        </ErrorBoundary>
        <ErrorBoundary label="Data">
          <Data />
        </ErrorBoundary>
        <ErrorBoundary label="About">
          <About />
        </ErrorBoundary>
      </div>
    </div>
  );
}
