import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { ClipboardRow, TodayStats } from "./types";

const PAGE_SIZE = 20;
const SEARCH_DEBOUNCE_MS = 300;
export const UNDO_WINDOW_MS = 5000;

interface PendingForget {
  row: ClipboardRow;
  originalIndex: number;
  timeoutId: number;
}

export interface PanelState {
  list: ClipboardRow[];
  stats: TodayStats | null;
  query: string;
  loading: boolean;
  searching: boolean;
  selectedIndex: number;
  error: string | null;
  isPaused: boolean;
  pendingForget: PendingForget | null;
  setQuery: (q: string) => void;
  setSelectedIndex: (i: number) => void;
  refresh: () => Promise<void>;
  forget: (row: ClipboardRow, index: number) => void;
  undoForget: () => void;
  pauseCapture: (minutes: number | null) => Promise<void>;
  resumeCapture: () => Promise<void>;
}

export function usePanel(): PanelState {
  const [list, setList] = useState<ClipboardRow[]>([]);
  const [stats, setStats] = useState<TodayStats | null>(null);
  const [query, setQueryState] = useState("");
  const [loading, setLoading] = useState(true);
  const [searching, setSearching] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [isPaused, setIsPaused] = useState(false);
  const [pendingForget, setPendingForget] = useState<PendingForget | null>(null);

  const debounceTimer = useRef<number | null>(null);
  const pendingRef = useRef<PendingForget | null>(null);
  // 镜像 pendingForget 到 ref，供闭包（flushPending/undoForget）读最新值。
  // 放 useEffect 里而非 render body，符合 React 规范（避免 StrictMode 双 render 下 ref 被赋值两次等）
  useEffect(() => {
    pendingRef.current = pendingForget;
  }, [pendingForget]);

  const loadRecent = useCallback(async () => {
    try {
      const rows = await invoke<ClipboardRow[]>("list_recent_clipboard", {
        limit: PAGE_SIZE,
        offset: 0,
      });
      setList(rows);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const loadStats = useCallback(async () => {
    try {
      const s = await invoke<TodayStats>("get_today_stats");
      setStats(s);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const loadPauseState = useCallback(async () => {
    try {
      const paused = await invoke<boolean>("is_capture_paused");
      setIsPaused(paused);
    } catch {
      /* 忽略：暂停状态查询失败不阻塞 UI */
    }
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    await Promise.all([loadRecent(), loadStats(), loadPauseState()]);
    setLoading(false);
    setSelectedIndex(0);
  }, [loadRecent, loadStats, loadPauseState]);

  const doSearch = useCallback(async (q: string) => {
    const trimmed = q.trim();
    if (!trimmed) {
      setSearching(false);
      await loadRecent();
      setSelectedIndex(0);
      return;
    }
    setSearching(true);
    setError(null);
    try {
      const rows = await invoke<ClipboardRow[]>("search_clipboard", {
        query: trimmed,
        limit: PAGE_SIZE,
      });
      setList(rows);
      setSelectedIndex(0);
    } catch (e) {
      setError(String(e));
    } finally {
      setSearching(false);
    }
  }, [loadRecent]);

  const setQuery = useCallback((q: string) => {
    setQueryState(q);
    if (debounceTimer.current !== null) {
      window.clearTimeout(debounceTimer.current);
    }
    debounceTimer.current = window.setTimeout(() => {
      void doSearch(q);
    }, SEARCH_DEBOUNCE_MS);
  }, [doSearch]);

  // flush 当前 pending forget（立即真删），用于新一次 forget 前清理
  const flushPending = useCallback(async () => {
    const pending = pendingRef.current;
    if (!pending) return;
    window.clearTimeout(pending.timeoutId);
    setPendingForget(null);
    try {
      await invoke<boolean>("forget_clipboard", { id: pending.row.id });
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const forget = useCallback((row: ClipboardRow, index: number) => {
    // 若已有 pending，先 flush
    void flushPending();

    // 乐观 UI：从列表移除
    setList((prev) => prev.filter((r) => r.id !== row.id));
    setSelectedIndex((idx) => Math.min(idx, Math.max(0, list.length - 2)));

    const timeoutId = window.setTimeout(() => {
      // 关键顺序：**先清 pending 让 UndoToast 立即 unmount，再调真删**。
      // 避免 "invoke 执行期间 UndoToast 仍显示 0s 撤销 → 用户点撤销 → 插回已真删 row → UI/DB 不一致" 的 race。
      setPendingForget((cur) => (cur?.row.id === row.id ? null : cur));
      invoke<boolean>("forget_clipboard", { id: row.id }).catch((e) =>
        setError(String(e)),
      );
    }, UNDO_WINDOW_MS);

    setPendingForget({ row, originalIndex: index, timeoutId });
  }, [list.length, flushPending]);

  const undoForget = useCallback(() => {
    const pending = pendingRef.current;
    if (!pending) return;
    window.clearTimeout(pending.timeoutId);
    setList((prev) => {
      const next = [...prev];
      const idx = Math.min(pending.originalIndex, next.length);
      next.splice(idx, 0, pending.row);
      return next;
    });
    setPendingForget(null);
  }, []);

  const pauseCapture = useCallback(async (minutes: number | null) => {
    try {
      await invoke("pause_capture", { minutes });
      setIsPaused(true);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const resumeCapture = useCallback(async () => {
    try {
      await invoke("resume_capture");
      setIsPaused(false);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const win = getCurrentWebviewWindow();
    const unlistenPromise = win.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        // 只刷新元信息（stats/暂停状态），**不刷 list**。
        // 如果用户正在搜索，refresh→loadRecent 会覆盖搜索结果导致状态不一致
        // （搜索框仍显示 query，列表却变回最近 20 条）。
        void loadStats();
        void loadPauseState();
      } else {
        // 失焦时立即 flush pending forget，避免 5s 窗口横跨会话
        void flushPending();
      }
    });
    return () => {
      void unlistenPromise.then((un) => un());
    };
  }, [loadStats, loadPauseState, flushPending]);

  return {
    list,
    stats,
    query,
    loading,
    searching,
    selectedIndex,
    error,
    isPaused,
    pendingForget,
    setQuery,
    setSelectedIndex,
    refresh,
    forget,
    undoForget,
    pauseCapture,
    resumeCapture,
  };
}
