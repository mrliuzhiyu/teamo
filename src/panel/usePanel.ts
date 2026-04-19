import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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
  /// 是否还有更多历史可加载（当前 list.length < DB 总量）
  hasMore: boolean;
  /// 正在加载下一页（防重入）
  loadingMore: boolean;
  setQuery: (q: string) => void;
  setSelectedIndex: (i: number) => void;
  refresh: () => Promise<void>;
  forget: (row: ClipboardRow, index: number) => void;
  undoForget: () => void;
  pauseCapture: (minutes: number | null) => Promise<void>;
  resumeCapture: () => Promise<void>;
  togglePin: (row: ClipboardRow) => Promise<void>;
  /// 加载下一页历史（搜索模式下 no-op）
  loadMore: () => Promise<void>;
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
  const [hasMore, setHasMore] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);

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
      // 满一页 → 可能还有更多；不满 → 到底了
      setHasMore(rows.length >= PAGE_SIZE);
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
      // 搜索模式下不分页（FTS 结果自然 cap；用户看不到更多结果是符合预期的）
      setHasMore(false);
    } catch (e) {
      setError(String(e));
    } finally {
      setSearching(false);
    }
  }, [loadRecent]);

  const loadMore = useCallback(async () => {
    if (loadingMore || !hasMore || query.trim()) return;
    setLoadingMore(true);
    try {
      const next = await invoke<ClipboardRow[]>("list_recent_clipboard", {
        limit: PAGE_SIZE,
        offset: list.length,
      });
      if (next.length === 0) {
        setHasMore(false);
        return;
      }
      setList((prev) => [...prev, ...next]);
      if (next.length < PAGE_SIZE) setHasMore(false);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoadingMore(false);
    }
  }, [loadingMore, hasMore, query, list.length]);

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

  const togglePin = useCallback(
    async (row: ClipboardRow) => {
      try {
        const newPinnedAt = await invoke<number | null>("toggle_pin", { id: row.id });
        // 乐观更新本地 list；立刻 reload 让 ORDER BY pinned_at 生效把置顶项排到顶部
        setList((prev) =>
          prev.map((r) => (r.id === row.id ? { ...r, pinned_at: newPinnedAt } : r)),
        );
        await loadRecent();
      } catch (e) {
        setError(String(e));
      }
    },
    [loadRecent],
  );

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
    const queryRef = { current: query };
    queryRef.current = query;

    const unlistenPromise = win.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        // focus 时刷新 stats + 暂停状态（轻量，必刷）
        void loadStats();
        void loadPauseState();
        // 列表：仅当用户没在搜索时才刷新。
        // 正在搜索时刷 list 会把搜索结果覆盖成最近 20 条，query 却还显示在搜索框 → 不一致。
        // query 为空时 list 展示的是"最近 20 条"，此时刷新能让新 capture 的内容显示出来。
        if (!queryRef.current.trim()) {
          void loadRecent();
        }
      } else {
        // 失焦时立即 flush pending forget，避免 5s 窗口横跨会话
        void flushPending();
      }
    });
    return () => {
      void unlistenPromise.then((un) => un());
    };
  }, [loadStats, loadPauseState, loadRecent, flushPending, query]);

  // 清空数据事件监听：Settings 页点"清空本地数据"成功后 emit `data:cleared`，
  // panel 如果正开着就刷新列表（否则下次 mount 自然 fresh）
  useEffect(() => {
    const unlistenPromise = listen<void>("data:cleared", () => {
      setList([]);
      setSelectedIndex(0);
      void loadStats();
    });
    return () => {
      void unlistenPromise.then((un) => un());
    };
  }, [loadStats]);

  // 新记录事件监听：clipboard/mod.rs ingest_once insert 成功后 emit `clipboard:new`。
  // panel 常驻时自动刷新列表（不再需要关了再开才看到新内容）。
  // query 非空（搜索中）时只刷 stats 不覆盖搜索结果；用 queryRef 读最新值避免
  // 每次 query 变化 re-subscribe listener
  const queryRef = useRef(query);
  useEffect(() => {
    queryRef.current = query;
  }, [query]);
  useEffect(() => {
    const unlistenPromise = listen<void>("clipboard:new", () => {
      void loadStats();
      if (!queryRef.current.trim()) {
        void loadRecent();
      }
    });
    return () => {
      void unlistenPromise.then((un) => un());
    };
  }, [loadRecent, loadStats]);

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
    togglePin,
    hasMore,
    loadingMore,
    loadMore,
  };
}
