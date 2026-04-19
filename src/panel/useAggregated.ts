import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ClipboardRow } from "./types";

/// 后端 list_sessions 返回的 session 摘要
export interface SessionSummary {
  session_id: string;
  first_preview: string;
  primary_source_app: string | null;
  started_at: number;
  ended_at: number;
  item_count: number;
  has_image: boolean;
  has_sensitive: boolean;
}

const PAGE_SIZE = 50;

/// 聚合 tab 数据源：L1 session 规则分组 + 展开某 session 看 items。
/// 监听 clipboard:new 新记录事件自动刷新。
export function useAggregated(enabled: boolean) {
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [expandedItems, setExpandedItems] = useState<ClipboardRow[]>([]);
  const [expandLoading, setExpandLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const rows = await invoke<SessionSummary[]>("list_sessions", { limit: PAGE_SIZE });
      setSessions(rows);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const toggleExpand = useCallback(
    async (sessionId: string) => {
      if (expandedId === sessionId) {
        setExpandedId(null);
        setExpandedItems([]);
        return;
      }
      setExpandLoading(true);
      try {
        const items = await invoke<ClipboardRow[]>("list_session_items", { sessionId });
        setExpandedId(sessionId);
        setExpandedItems(items);
      } catch (e) {
        setError(String(e));
      } finally {
        setExpandLoading(false);
      }
    },
    [expandedId],
  );

  // 切到聚合 tab 才 enabled；enabled=false 时不主动 fetch，避免白占 IPC
  useEffect(() => {
    if (!enabled) return;
    void refresh();
  }, [enabled, refresh]);

  // 新记录事件 → 刷新 session 列表（仅在 enabled 时订阅）
  useEffect(() => {
    if (!enabled) return;
    const unlistenPromise = listen<void>("clipboard:new", () => {
      void refresh();
    });
    return () => {
      void unlistenPromise.then((un) => un());
    };
  }, [enabled, refresh]);

  return {
    sessions,
    loading,
    expandedId,
    expandedItems,
    expandLoading,
    error,
    refresh,
    toggleExpand,
  };
}
