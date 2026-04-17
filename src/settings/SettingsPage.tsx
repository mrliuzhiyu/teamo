import General from "./sections/General";
import Privacy from "./sections/Privacy";
import Cloud from "./sections/Cloud";
import Data from "./sections/Data";
import About from "./sections/About";

/// Teamo 主窗口唯一页面（v0.1）：5 区纵向滚动设置页。
/// 未来若增加其他页面再改为 route 分发。
export default function SettingsPage() {
  return (
    <div className="min-h-screen bg-stone-50 pb-12">
      <header className="px-6 py-6 border-b border-stone-200 bg-white">
        <div className="flex items-center gap-3">
          <div className="w-9 h-9 rounded-lg bg-white border border-stone-200 flex items-center justify-center">
            <span className="text-lg font-semibold text-stone-900">T</span>
          </div>
          <div>
            <h1 className="text-[15px] font-semibold tracking-tight text-stone-900">
              Teamo 设置
            </h1>
            <p className="text-[11px] text-stone-500">你的人生记录 Agent</p>
          </div>
        </div>
      </header>

      <main className="max-w-2xl mx-auto pt-6">
        <General />
        <Privacy />
        <Cloud />
        <Data />
        <About />
      </main>
    </div>
  );
}
