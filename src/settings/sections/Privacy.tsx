import { useState } from "react";
import Section, { Row } from "../components/Section";
import Switch from "../components/Switch";
import { useBoolSetting } from "../useSettings";
import { useAppRules, type AppRule } from "../useAppRules";
import { isMac } from "../../lib/platform";
import {
  SENS_PASSWORD,
  SENS_TOKEN,
  SENS_CREDIT_CARD,
  SENS_ID_CARD,
  SENS_PHONE,
  SENS_EMAIL,
} from "../../lib/settings-keys";

/// 6 个敏感类型开关 —— filter::apply_filters 每个 detector 前读对应 key
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
        description="命中的内容标为 local_only，永不上云（开关实时生效）"
      >
        {SENS_ITEMS.map((item) => (
          <SensRow key={item.key} settingKey={item.key} label={item.label} hint={item.hint} />
        ))}
      </Section>

      <AppRulesSection />
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

// ── App 黑白名单 ──

function AppRulesSection() {
  const { blacklist, whitelist, add, remove, pickCurrentApp, error } = useAppRules();
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (ruleType: "blacklist" | "whitelist") => {
    const app = input.trim();
    if (!app) return;
    setBusy(true);
    await add(app, ruleType);
    setInput("");
    setBusy(false);
  };

  const fillCurrent = async () => {
    setBusy(true);
    const app = await pickCurrentApp();
    setBusy(false);
    if (app) setInput(app);
  };

  const description = isMac
    ? "从指定 App 复制的内容自动分类。白名单优先级高于黑名单 ·  ⚠ macOS source_app 抓取 Phase 4 上线前规则不生效"
    : "从指定 App 复制的内容自动分类。白名单优先级高于黑名单（命中规则即刻生效）";

  return (
    <Section title="隐私 · App 黑白名单" description={description}>
      <div className="px-4 py-3 border-b border-stone-100">
        <div className="flex items-center gap-2">
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="App exe 名（例：1Password.exe / Chrome.exe）"
            className="flex-1 px-2 py-1 text-[12px] bg-stone-50 border border-stone-200 rounded outline-none focus:border-stone-400"
            spellCheck={false}
          />
          <button
            onClick={() => void fillCurrent()}
            disabled={busy}
            className="text-[11px] px-2 py-1 bg-stone-100 hover:bg-stone-200 rounded disabled:opacity-40"
            title="抓取当前前景 App 名（Windows 有效）"
          >
            当前 App
          </button>
        </div>
        <div className="mt-2 flex items-center gap-2">
          <button
            onClick={() => void submit("blacklist")}
            disabled={busy || !input.trim()}
            className="text-[11px] px-2 py-1 bg-amber-50 text-amber-700 border border-amber-200 rounded hover:bg-amber-100 disabled:opacity-40"
          >
            加入黑名单
          </button>
          <button
            onClick={() => void submit("whitelist")}
            disabled={busy || !input.trim()}
            className="text-[11px] px-2 py-1 bg-emerald-50 text-emerald-700 border border-emerald-200 rounded hover:bg-emerald-100 disabled:opacity-40"
          >
            加入白名单
          </button>
        </div>
        {error && (
          <div className="mt-2 text-[11px] text-red-600">错误：{error}</div>
        )}
      </div>

      <RulesList
        title="黑名单"
        tone="blacklist"
        hint="命中 = 该 App 复制的内容进 local_only"
        rules={blacklist}
        onRemove={remove}
      />
      <RulesList
        title="白名单"
        tone="whitelist"
        hint="命中 = 跳过所有后续检测（敏感也放行）"
        rules={whitelist}
        onRemove={remove}
      />
    </Section>
  );
}

function RulesList({
  title,
  tone,
  hint,
  rules,
  onRemove,
}: {
  title: string;
  tone: "blacklist" | "whitelist";
  hint: string;
  rules: AppRule[];
  onRemove: (id: number) => Promise<void> | void;
}) {
  const tagClass =
    tone === "blacklist"
      ? "bg-amber-50 text-amber-700"
      : "bg-emerald-50 text-emerald-700";

  return (
    <div className="px-4 py-3 border-b border-stone-100">
      <div className="flex items-baseline justify-between">
        <div className="text-[12px] text-stone-700 font-medium">{title}</div>
        <div className="text-[10px] text-stone-400">{hint}</div>
      </div>
      {rules.length === 0 ? (
        <div className="mt-1 text-[11px] text-stone-400">暂无条目</div>
      ) : (
        <div className="mt-2 flex flex-wrap gap-1.5">
          {rules.map((r) => (
            <span
              key={r.id}
              className={`inline-flex items-center gap-1.5 px-2 py-0.5 text-[11px] rounded ${tagClass}`}
            >
              {r.app_identifier}
              <button
                onClick={() => void onRemove(r.id)}
                className="text-stone-500 hover:text-red-600 leading-none"
                title="删除"
              >
                ×
              </button>
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
