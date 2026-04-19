import { useEffect, useState } from "react";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import Section, { Row } from "../components/Section";
import Switch from "../components/Switch";
import { shortcutLabel } from "../../lib/platform";

export default function General() {
  const [autostart, setAutostart] = useState<boolean | null>(null);

  useEffect(() => {
    isEnabled()
      .then(setAutostart)
      .catch(() => setAutostart(false));
  }, []);

  const toggleAutostart = async (v: boolean) => {
    setAutostart(v);
    try {
      if (v) await enable();
      else await disable();
    } catch (e) {
      console.error("toggle autostart failed", e);
      // 回滚 UI
      setAutostart(!v);
    }
  };

  return (
    <Section
      title="通用"
      description="应用级的基础设置"
    >
      <Row
        label="开机自启动"
        hint="登录系统后 Teamo 自动在后台运行"
      >
        <Switch
          checked={autostart ?? false}
          onChange={toggleAutostart}
          disabled={autostart === null}
        />
      </Row>
      <Row
        label="全局快捷键"
        hint="按下即在任意 App 唤起快速面板"
      >
        <kbd className="px-2 py-0.5 text-[11px] bg-stone-100 border border-stone-200 rounded">
          {shortcutLabel}
        </kbd>
      </Row>
      <Row label="语言" hint="后续版本支持更多语言">
        <span className="text-[12px] text-stone-600">简体中文</span>
      </Row>
    </Section>
  );
}
