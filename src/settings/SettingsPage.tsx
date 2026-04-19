import { useState } from "react";
import General from "./sections/General";
import Privacy from "./sections/Privacy";
import Cloud from "./sections/Cloud";
import Data from "./sections/Data";
import About from "./sections/About";
import { shortcutLabel } from "../lib/platform";

const WELCOME_KEY = "teamo.welcome_dismissed";

/// Teamo 主窗口唯一页面（v0.1）：5 区纵向滚动设置页 + 首次启动欢迎横幅。
/// 未来若增加其他页面再改为 route 分发。
export default function SettingsPage() {
  const [showWelcome, setShowWelcome] = useState(() => {
    try {
      return localStorage.getItem(WELCOME_KEY) !== "1";
    } catch {
      return false;
    }
  });

  const dismissWelcome = () => {
    setShowWelcome(false);
    try {
      localStorage.setItem(WELCOME_KEY, "1");
    } catch {
      // localStorage 不可用忽略（隐身模式等极端情况）
    }
  };

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
        {showWelcome && (
          <section className="mx-6 mb-6 p-4 bg-brand-50 border border-brand-100 rounded-lg relative">
            <button
              onClick={dismissWelcome}
              className="absolute right-2 top-2 w-6 h-6 flex items-center justify-center rounded hover:bg-brand-100 text-stone-500 hover:text-stone-800"
              title="知道了"
              aria-label="关闭欢迎提示"
            >
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                <path d="M2 2L10 10M10 2L2 10" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              </svg>
            </button>
            <div className="text-[13px] font-medium text-stone-900 pr-6">
              👋 欢迎使用 Teamo
            </div>
            <ul className="mt-2 space-y-1 text-[12px] text-stone-700">
              <li>
                ·{" "}
                <kbd className="px-1 py-0.5 bg-white border border-stone-200 rounded text-[11px]">
                  {shortcutLabel}
                </kbd>{" "}
                在任何地方唤起快速面板
              </li>
              <li>· 关闭此窗口后 Teamo 会常驻托盘继续记录</li>
              <li>· 左键点击任务栏 T 图标可随时打开面板</li>
            </ul>
            <button
              onClick={dismissWelcome}
              className="mt-3 px-3 py-1 text-[11px] bg-stone-900 text-white rounded hover:bg-stone-800"
            >
              知道了
            </button>
          </section>
        )}
        <General />
        <Privacy />
        <Cloud />
        <Data />
        <About />
      </main>
    </div>
  );
}
