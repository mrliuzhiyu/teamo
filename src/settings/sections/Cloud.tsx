import { useState } from "react";
import { open as openShell } from "@tauri-apps/plugin-shell";
import Section, { Row } from "../components/Section";
import { useAuth } from "../useAuth";
import { useToast } from "../../lib/toast";
import { useConfirm } from "../../lib/ConfirmDialog";

/// 云端连接 — 邮箱 OTP 登录 TextView
/// 未登录：占一行说明 + 登录按钮，点击展开登录表单
/// 已登录：显示 email + 退出按钮
/// 方向：登录是"加强"，不破坏本地能力，未登录 Teamo 完整可用
export default function Cloud() {
  const { state, sendOtp, verifyOtp, logout } = useAuth();
  const [expanded, setExpanded] = useState(false);
  const [email, setEmail] = useState("");
  const [code, setCode] = useState("");
  const [otpSent, setOtpSent] = useState(false);
  const [busy, setBusy] = useState(false);
  const toast = useToast();
  const confirm = useConfirm();

  const openSite = () => {
    void openShell("https://textview.cn").catch(() => undefined);
  };

  const handleSendOtp = async () => {
    if (!email.includes("@")) {
      toast("error", "邮箱格式不正确");
      return;
    }
    setBusy(true);
    try {
      await sendOtp(email.trim());
      setOtpSent(true);
      toast("success", `验证码已发送到 ${email}`);
    } catch (e) {
      toast("error", `发送失败：${e}`);
    } finally {
      setBusy(false);
    }
  };

  const handleVerify = async () => {
    if (code.trim().length < 4) {
      toast("error", "验证码格式不正确");
      return;
    }
    setBusy(true);
    try {
      const user = await verifyOtp(email.trim(), code.trim());
      toast("success", `登录成功：${user.email}`);
      setExpanded(false);
      setOtpSent(false);
      setEmail("");
      setCode("");
    } catch (e) {
      toast("error", `登录失败：${e}`);
    } finally {
      setBusy(false);
    }
  };

  const handleLogout = async () => {
    const ok = await confirm({
      title: "退出 TextView 登录？",
      body: "退出后 Teamo 回到纯本地模式。\n本地数据（剪贴板 / session 分组）不受影响。\n已上云的 memo 仍保留在 TextView。",
      confirmText: "退出",
      cancelText: "保持登录",
    });
    if (!ok) return;
    setBusy(true);
    try {
      await logout();
      toast("info", "已退出登录");
    } catch (e) {
      toast("error", `退出失败：${e}`);
    } finally {
      setBusy(false);
    }
  };

  // 已登录
  if (state?.logged_in && state.user) {
    return (
      <Section
        title="云端连接"
        description="已连接 TextView · session 可整理上云 · 本地能力不受影响"
      >
        <Row label="当前账号" hint={state.user.email}>
          <button
            onClick={() => void handleLogout()}
            disabled={busy}
            className="text-[11px] px-2 py-1 bg-white border border-stone-300 rounded hover:bg-stone-100 disabled:opacity-40"
          >
            退出登录
          </button>
        </Row>
      </Section>
    );
  }

  // 未登录
  return (
    <Section
      title="云端连接"
      description="Teamo 可独立使用（纯本地）。登录 TextView 后可将 session 整理成 memo 同步云端"
    >
      {!expanded ? (
        <Row label="状态" hint="本地模式 · 仅使用端侧能力">
          <div className="flex items-center gap-2">
            <button
              onClick={() => setExpanded(true)}
              className="text-[11px] px-2 py-1 bg-stone-900 text-white rounded hover:bg-stone-800"
            >
              登录 TextView
            </button>
            <button
              onClick={openSite}
              className="text-[11px] text-stone-500 hover:text-stone-700 underline"
            >
              了解 →
            </button>
          </div>
        </Row>
      ) : (
        <div className="px-4 py-3">
          {!otpSent ? (
            <div className="flex items-center gap-2">
              <input
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="邮箱"
                className="flex-1 px-2 py-1 text-[12px] bg-stone-50 border border-stone-200 rounded outline-none focus:border-stone-400"
                autoFocus
              />
              <button
                onClick={() => void handleSendOtp()}
                disabled={busy || !email.includes("@")}
                className="text-[11px] px-2 py-1 bg-stone-900 text-white rounded hover:bg-stone-800 disabled:opacity-40"
              >
                {busy ? "发送中…" : "发送验证码"}
              </button>
              <button
                onClick={() => setExpanded(false)}
                className="text-[11px] text-stone-500 hover:text-stone-700"
              >
                取消
              </button>
            </div>
          ) : (
            <div className="space-y-2">
              <div className="text-[11px] text-stone-500">
                验证码已发送到 <strong className="text-stone-700">{email}</strong>
              </div>
              <div className="flex items-center gap-2">
                <input
                  type="text"
                  value={code}
                  onChange={(e) => setCode(e.target.value)}
                  placeholder="6 位验证码"
                  maxLength={10}
                  className="flex-1 px-2 py-1 text-[12px] bg-stone-50 border border-stone-200 rounded outline-none focus:border-stone-400 font-mono"
                  autoFocus
                />
                <button
                  onClick={() => void handleVerify()}
                  disabled={busy || code.trim().length < 4}
                  className="text-[11px] px-2 py-1 bg-stone-900 text-white rounded hover:bg-stone-800 disabled:opacity-40"
                >
                  {busy ? "验证中…" : "登录"}
                </button>
                <button
                  onClick={() => {
                    setOtpSent(false);
                    setCode("");
                  }}
                  className="text-[11px] text-stone-500 hover:text-stone-700"
                >
                  重发
                </button>
              </div>
            </div>
          )}
        </div>
      )}
    </Section>
  );
}
