import { open as openShell } from "@tauri-apps/plugin-shell";
import Section from "../components/Section";

/// 云端连接区（未登录占位，M3 登录后补真实状态）
export default function Cloud() {
  return (
    <Section
      title="云端连接"
      description="连接后只有有价值的内容会同步，敏感数据永远本地"
    >
      <div className="px-4 py-6 flex flex-col items-center text-center">
        <div className="w-12 h-12 rounded-xl bg-brand-50 text-brand-600 flex items-center justify-center text-2xl mb-3">
          🌐
        </div>
        <div className="text-[14px] text-stone-800 font-medium">
          连接 TextView 云端
        </div>
        <div className="mt-1 text-[12px] text-stone-500 max-w-sm">
          第二天看到整理好的昨天 —— 碎片自动归类、写成日记
        </div>
        <button
          disabled
          className="mt-4 px-4 py-1.5 text-[12px] bg-stone-100 text-stone-400 rounded-md cursor-not-allowed"
          title="M3 上线"
        >
          连接 TextView 云端（即将支持）
        </button>
        <button
          onClick={() => void openShell("https://textview.cn").catch(() => undefined)}
          className="mt-2 text-[11px] text-stone-400 hover:text-stone-600"
        >
          了解 TextView →
        </button>
      </div>
    </Section>
  );
}
