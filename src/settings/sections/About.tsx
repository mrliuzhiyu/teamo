import { useEffect, useState } from "react";
import { open as openShell } from "@tauri-apps/plugin-shell";
import Section, { Row } from "../components/Section";

export default function About() {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    import("@tauri-apps/api/app")
      .then((api) => api.getVersion())
      .then(setVersion)
      .catch(() => setVersion("dev"));
  }, []);

  const link = (url: string) => () => {
    void openShell(url).catch(() => undefined);
  };

  return (
    <Section title="关于 Teamo">
      <Row label="产品" hint="你的人生记录 Agent · 本地优先 + 可选云端">
        <span className="text-[12px] text-stone-600">Teamo</span>
      </Row>
      <Row label="版本" hint="检查更新 Phase 2 支持">
        {version === null ? (
          <span className="inline-block w-20 h-3 bg-stone-100 rounded animate-pulse" />
        ) : (
          <span className="text-[12px] text-stone-600">v{version} · pre-alpha</span>
        )}
      </Row>
      <Row label="开源协议">
        <button
          onClick={link("https://github.com/mrliuzhiyu/teamo/blob/main/LICENSE")}
          className="text-[11px] text-brand-600 hover:text-brand-700 underline"
        >
          Apache-2.0
        </button>
      </Row>
      <Row label="源代码">
        <button
          onClick={link("https://github.com/mrliuzhiyu/teamo")}
          className="text-[11px] text-brand-600 hover:text-brand-700 underline"
        >
          github.com/mrliuzhiyu/teamo
        </button>
      </Row>
      <Row label="反馈 / 问题">
        <button
          onClick={link("https://github.com/mrliuzhiyu/teamo/issues")}
          className="text-[11px] text-brand-600 hover:text-brand-700 underline"
        >
          GitHub Issues
        </button>
      </Row>
    </Section>
  );
}
