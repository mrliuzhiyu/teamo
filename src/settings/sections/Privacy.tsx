import Section, { Row } from "../components/Section";
import Switch from "../components/Switch";
import { useBoolSetting } from "../useSettings";
import {
  SENS_PASSWORD,
  SENS_TOKEN,
  SENS_CREDIT_CARD,
  SENS_ID_CARD,
  SENS_PHONE,
  SENS_EMAIL,
} from "../../lib/settings-keys";

/// 6 个敏感类型开关 —— 值写入 SQLite settings 表，后端 filter::apply_filters
/// 跑每个 detector 前读对应 key 决定是否启用（默认全开）。
const SENS_ITEMS: Array<{ key: string; label: string; hint: string }> = [
  { key: SENS_PASSWORD, label: "密码", hint: "Aa1@bcdefg 这类多字符类型短串" },
  { key: SENS_TOKEN, label: "Token", hint: "sk-xxx / ghp_xxx / Bearer / JWT" },
  { key: SENS_CREDIT_CARD, label: "银行卡", hint: "13-19 位数字 + Luhn 校验" },
  { key: SENS_ID_CARD, label: "身份证", hint: "18 位 + GB 11643 校验" },
  { key: SENS_PHONE, label: "手机号", hint: "中国大陆 1[3-9]xxxx" },
  { key: SENS_EMAIL, label: "邮箱", hint: "RFC 5322 简化" },
];

export default function Privacy() {
  return (
    <>
      <Section
        title="隐私 · 敏感检测"
        description="命中的内容会被标为 local_only，永不上云"
      >
        {SENS_ITEMS.map((item) => (
          <SensRow key={item.key} settingKey={item.key} label={item.label} hint={item.hint} />
        ))}
      </Section>

      <Section
        title="隐私 · App 黑白名单"
        description="从指定 App 复制的内容自动进 local_only（Phase 2 上线）"
      >
        <Row label="App 黑名单" hint="例：1Password / Bitwarden / 网银客户端">
          <span className="text-[11px] text-stone-400">Phase 2</span>
        </Row>
        <Row label="App 白名单" hint="始终允许上云的 App，优先级高于黑名单">
          <span className="text-[11px] text-stone-400">Phase 2</span>
        </Row>
      </Section>
    </>
  );
}

function SensRow({
  settingKey,
  label,
  hint,
}: {
  settingKey: string;
  label: string;
  hint: string;
}) {
  const [enabled, setEnabled] = useBoolSetting(settingKey, true);
  return (
    <Row label={label} hint={hint}>
      <Switch checked={enabled} onChange={setEnabled} />
    </Row>
  );
}
