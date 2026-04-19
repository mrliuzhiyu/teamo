import { useCallback, useEffect, useRef } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { ClipboardRow } from "./types";
import { usePanel, UNDO_WINDOW_MS } from "./usePanel";
import StatsHeader from "./StatsHeader";
import SearchBar from "./SearchBar";
import CardList from "./CardList";
import ActionBar from "./ActionBar";
import UndoToast from "./UndoToast";
import { enterHintLabel } from "../lib/platform";

export default function PanelApp() {
  const panel = usePanel();
  const searchRef = useRef<HTMLInputElement>(null);

  const hidePanel = useCallback(async () => {
    await getCurrentWebviewWindow().hide();
  }, []);

  // 复制到系统剪切板（仅 text/url；image 粘贴留 Phase 3B），不关闭窗口
  // 返回 true = 确实写入剪切板；false = 跳过（file 类型 / 空内容 / 写入异常）
  // text/url → writeText；image → 后端 copy_image_to_clipboard（读 PNG + arboard set_image）
  const copyToClipboard = useCallback(async (row: ClipboardRow): Promise<boolean> => {
    try {
      if (row.content_type === "image") {
        if (!row.image_path) return false;
        await invoke("copy_image_to_clipboard", { id: row.id });
        return true;
      }
      if (row.content_type === "text" || row.content_type === "url") {
        const text = row.content ?? "";
        if (!text) return false;
        await writeText(text);
        return true;
      }
      // file 类型：CF_HDROP 粘贴需要平台特殊化，留更后（v0.2+）
      return false;
    } catch (e) {
      console.error("copyToClipboard failed", e);
      return false;
    }
  }, []);

  // 粘贴某条：复制 + 关面板 + 系统 Ctrl+V（Windows），失败则用户手动粘贴
  // 关键：若 copyToClipboard 返回 false（图片/文件/空/写入失败），
  // 不能 invoke paste_to_previous — 否则 Ctrl+V 会粘贴用户上一次手动复制的内容，
  // 不符合用户选中此条的意图。
  const pasteRow = useCallback(
    async (row: ClipboardRow) => {
      const copied = await copyToClipboard(row);
      await hidePanel();
      if (!copied) return;
      try {
        await invoke("paste_to_previous");
      } catch (e) {
        console.debug("paste_to_previous unavailable:", e);
      }
    },
    [copyToClipboard, hidePanel],
  );

  // Enter / 双击 都走 pasteRow
  const handleEnter = useCallback(async () => {
    const row = panel.list[panel.selectedIndex];
    if (!row) return;
    await pasteRow(row);
  }, [panel.list, panel.selectedIndex, pasteRow]);

  // 右侧按钮「复制」：仅写入剪切板、不关闭（UI 可见反馈留给后续增强）
  const handleCopy = useCallback(
    (row: ClipboardRow) => {
      void copyToClipboard(row);
    },
    [copyToClipboard],
  );

  const handleForget = useCallback(
    (row: ClipboardRow, index: number) => {
      panel.forget(row, index);
    },
    [panel],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        void hidePanel();
        return;
      }
      if (e.key === "ArrowDown") {
        e.preventDefault();
        if (panel.list.length === 0) return;
        panel.setSelectedIndex(Math.min(panel.selectedIndex + 1, panel.list.length - 1));
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        if (panel.list.length === 0) return;
        panel.setSelectedIndex(Math.max(panel.selectedIndex - 1, 0));
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        void handleEnter();
        return;
      }
      if (e.key === "Delete" || e.key === "Backspace") {
        // Backspace 在搜索框输入时不触发 forget：只有光标不在 input 或搜索框空时才拦
        const target = e.target as HTMLElement;
        const inInput = target.tagName === "INPUT";
        if (e.key === "Backspace" && inInput) return;
        const row = panel.list[panel.selectedIndex];
        if (!row) return;
        e.preventDefault();
        panel.forget(row, panel.selectedIndex);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [panel, handleEnter, hidePanel]);

  useEffect(() => {
    const win = getCurrentWebviewWindow();
    const unlistenPromise = win.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        // 只 focus 不 select：保留用户正在输入的 query。
        // 切窗再切回不应覆盖输入内容（想清空可用清除按钮或 Escape）。
        searchRef.current?.focus();
      }
    });
    return () => {
      void unlistenPromise.then((un) => un());
    };
  }, []);

  return (
    <div className="h-screen flex flex-col bg-white select-none relative">
      <StatsHeader stats={panel.stats} onClose={() => void hidePanel()} />
      <SearchBar
        ref={searchRef}
        value={panel.query}
        onChange={panel.setQuery}
        searching={panel.searching}
      />
      <CardList
        list={panel.list}
        selectedIndex={panel.selectedIndex}
        query={panel.query}
        loading={panel.loading}
        onSelect={panel.setSelectedIndex}
        onCopy={handleCopy}
        onForget={handleForget}
        onEnter={(r) => void pasteRow(r)}
      />
      <div className="px-3 py-1 text-[10px] text-stone-400 bg-stone-50 border-t border-stone-200 flex items-center gap-2">
        <kbd className="px-1 py-0.5 bg-white border border-stone-200 rounded text-[9px]">↑↓</kbd>
        <span>选择</span>
        <kbd className="px-1 py-0.5 bg-white border border-stone-200 rounded text-[9px]">Enter</kbd>
        <span>{enterHintLabel}</span>
        <kbd className="px-1 py-0.5 bg-white border border-stone-200 rounded text-[9px]">Del</kbd>
        <span>忘记</span>
        <kbd className="px-1 py-0.5 bg-white border border-stone-200 rounded text-[9px]">Esc</kbd>
        <span>关闭</span>
        {panel.error && <span className="ml-auto text-red-500 truncate">{panel.error}</span>}
      </div>
      <ActionBar
        isPaused={panel.isPaused}
        onPause={(m) => void panel.pauseCapture(m)}
        onResume={() => void panel.resumeCapture()}
      />
      {panel.pendingForget && (
        <UndoToast
          pendingId={panel.pendingForget.row.id}
          onUndo={panel.undoForget}
          durationMs={UNDO_WINDOW_MS}
        />
      )}
    </div>
  );
}
