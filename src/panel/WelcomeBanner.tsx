import { useState } from "react";
import { shortcutLabel } from "../lib/platform";

const WELCOME_KEY = "teamo.welcome_dismissed";

interface Props {
  onOpenSettings: () => void;
}

/// 首次启动欢迎横幅 — 在 panel list 视图顶部显示
/// 一次性 dismiss，localStorage 记忆；关了就不再出现
export default function WelcomeBanner({ onOpenSettings }: Props) {
  const [show, setShow] = useState(() => {
    try {
      return localStorage.getItem(WELCOME_KEY) !== "1";
    } catch {
      return false;
    }
  });

  if (!show) return null;

  const dismiss = () => {
    setShow(false);
    try {
      localStorage.setItem(WELCOME_KEY, "1");
    } catch {
      /* 隐身模式等极端情况忽略 */
    }
  };

  return (
    <div className="mx-2 mt-2 p-3 bg-brand-50 border border-brand-100 rounded-lg relative text-[11px]">
      <button
        onClick={dismiss}
        className="absolute right-1.5 top-1.5 w-5 h-5 flex items-center justify-center rounded hover:bg-brand-100 text-stone-500 hover:text-stone-800"
        title="知道了"
        aria-label="关闭欢迎提示"
      >
        <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
          <path d="M2 2L8 8M8 2L2 8" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
        </svg>
      </button>
      <div className="text-[12px] font-semibold text-stone-900 pr-5">👋 欢迎使用 Teamo</div>
      <ul className="mt-1.5 space-y-0.5 text-stone-700">
        <li>
          ·{" "}
          <kbd className="px-1 py-0.5 bg-white border border-stone-200 rounded text-[10px]">
            {shortcutLabel}
          </kbd>{" "}
          在任何地方唤起面板
        </li>
        <li>· 关掉此面板后 Teamo 继续在托盘记录</li>
        <li>· 左键点托盘 T 图标可随时打开面板</li>
      </ul>
      <div className="mt-2 flex items-center gap-2">
        <button
          onClick={dismiss}
          className="px-2 py-0.5 text-[11px] bg-stone-900 text-white rounded hover:bg-stone-800"
        >
          知道了
        </button>
        <button
          onClick={onOpenSettings}
          className="px-2 py-0.5 text-[11px] text-stone-600 hover:text-stone-900 underline"
        >
          看看设置 →
        </button>
      </div>
    </div>
  );
}
