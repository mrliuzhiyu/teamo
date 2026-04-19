import General from "./sections/General";
import Privacy from "./sections/Privacy";
import Cloud from "./sections/Cloud";
import Data from "./sections/Data";
import About from "./sections/About";

/// Teamo main window 的 Settings 页（v0.2+ 已失去自然入口——首次启动和 tray
/// 设置都走 panel 内嵌 settings 视图）。保留此组件作为 main window 的 fallback
/// 内容：用户手动 show main window 或未来恢复 main 入口时可用。
/// 欢迎横幅已搬到 panel/WelcomeBanner.tsx。
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
