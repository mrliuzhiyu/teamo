import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/// 对齐 Rust `repository::AppRule`（serde 默认 snake_case）
export interface AppRule {
  id: number;
  app_identifier: string;
  rule_type: "blacklist" | "whitelist";
  created_at: number;
}

/**
 * App 黑白名单 CRUD hook。
 *
 * 规则即数据：rule 变更直接走 SQLite settings 无关的 app_rules 表，
 * 不用发 event 通知其他 UI —— capture loop 每次 capture 都会 fresh 查询
 * app_rules 表做匹配（毫秒级 SELECT），不缓存，变更秒生效。
 */
export function useAppRules() {
  const [rules, setRules] = useState<AppRule[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await invoke<AppRule[]>("list_app_rules");
      setRules(list);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const add = useCallback(
    async (appIdentifier: string, ruleType: "blacklist" | "whitelist") => {
      try {
        await invoke<number>("add_app_rule", {
          appIdentifier,
          ruleType,
        });
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [refresh],
  );

  const remove = useCallback(
    async (id: number) => {
      try {
        await invoke<boolean>("remove_app_rule", { id });
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [refresh],
  );

  /// 抓当前前景 App 名（Windows 有效）—— 用于"添加当前 App"快捷
  const pickCurrentApp = useCallback(async (): Promise<string | null> => {
    try {
      return await invoke<string | null>("get_current_foreground_app");
    } catch {
      return null;
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const blacklist = rules.filter((r) => r.rule_type === "blacklist");
  const whitelist = rules.filter((r) => r.rule_type === "whitelist");

  return { rules, blacklist, whitelist, loading, error, add, remove, pickCurrentApp, refresh };
}
