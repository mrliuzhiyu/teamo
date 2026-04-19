import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface AuthUser {
  id: string;
  email: string;
}

interface AuthState {
  logged_in: boolean;
  user: AuthUser | null;
}

/// TextView 登录状态 + 操作。所有 API 调用通过 Rust 命令（token 在 Rust 侧
/// 管理，前端不接触 access_token / refresh_token —— 更安全）
export function useAuth() {
  const [state, setState] = useState<AuthState | null>(null);

  const refresh = useCallback(async () => {
    try {
      const s = await invoke<AuthState>("auth_state");
      setState(s);
    } catch {
      setState({ logged_in: false, user: null });
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const sendOtp = useCallback(async (email: string) => {
    await invoke("auth_send_otp", { email });
  }, []);

  const verifyOtp = useCallback(
    async (email: string, code: string) => {
      const user = await invoke<AuthUser>("auth_verify_otp", { email, code });
      await refresh();
      return user;
    },
    [refresh],
  );

  const logout = useCallback(async () => {
    await invoke("auth_logout");
    await refresh();
  }, [refresh]);

  return { state, refresh, sendOtp, verifyOtp, logout };
}
