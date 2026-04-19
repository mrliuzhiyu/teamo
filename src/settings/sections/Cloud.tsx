import { open as openShell } from "@tauri-apps/plugin-shell";
import Section, { Row } from "../components/Section";

/// 云端连接区（v0.1 纯本地，M3 接 OAuth + PKCE 真连接）
/// 不用大卡片 disabled 按钮 — 那种"做不到"的展示反而消耗信任感。一行提示 + 外链足够。
export default function Cloud() {
  const openSite = () => {
    void openShell("https://textview.cn").catch(() => undefined);
  };

  return (
    <Section
      title="云端连接"
      description="Teamo 可独立使用。连接 TextView 云端后，精选内容同步到云端由 AI 整理成日卡片"
    >
      <Row
        label="当前模式"
        hint="本地优先 · 所有数据仅存本地 · 云端同步 M3 版本支持"
      >
        <button
          onClick={openSite}
          className="text-[11px] text-stone-500 hover:text-stone-700 underline"
        >
          了解 TextView →
        </button>
      </Row>
    </Section>
  );
}
