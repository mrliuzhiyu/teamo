import { open as openShell } from "@tauri-apps/plugin-shell";

interface Props {
  /** "compact" 用在 panel ActionBar（一行小按钮）；"full" 用在 Settings Cloud 区（大卡片） */
  variant: "compact" | "full";
}

/**
 * 云端连接 CTA —— Panel 和 Settings 共用。
 *
 * 当前状态（v0.1）：未登录模式，没有实际连接能力。统一行为 = 跳 textview.cn 了解，
 * 不伪装可点"连接"。等 M3 OAuth + PKCE 上线后再升级为真连接流程。
 *
 * 为什么抽这个组件：原先 panel ActionBar 的"🌐 连接云端"直接跳外链，
 * Settings Cloud 区却显示"连接 TextView 云端（即将支持）"灰显按钮——两处
 * 行为不一致让用户困惑。统一走这个组件，文案和交互唯一来源。
 */
export default function CloudCtaButton({ variant }: Props) {
  const go = () => {
    void openShell("https://textview.cn").catch(() => undefined);
  };

  if (variant === "compact") {
    return (
      <button
        onClick={go}
        className="px-2 py-1 rounded hover:bg-stone-200 text-stone-600 transition-colors"
        title="了解 TextView 云端（v0.1 未登录即可用，云端连接 M3 上线）"
      >
        🌐 了解云端
      </button>
    );
  }

  return (
    <div className="px-4 py-6 flex flex-col items-center text-center">
      <div className="w-12 h-12 rounded-xl bg-brand-50 text-brand-600 flex items-center justify-center text-2xl mb-3">
        🌐
      </div>
      <div className="text-[14px] text-stone-800 font-medium">
        未来连接 TextView 云端
      </div>
      <div className="mt-1 text-[12px] text-stone-500 max-w-sm">
        M3 上线后，精选内容将同步到云端，AI 帮你整理成日卡片 ——
        敏感数据永远只在本地
      </div>
      <button
        disabled
        className="mt-4 px-4 py-1.5 text-[12px] bg-stone-100 text-stone-400 rounded-md cursor-not-allowed"
        title="M3 OAuth + PKCE 上线"
      >
        连接 TextView 云端（即将支持）
      </button>
      <button
        onClick={go}
        className="mt-2 text-[11px] text-stone-400 hover:text-stone-600"
      >
        了解 TextView →
      </button>
    </div>
  );
}
