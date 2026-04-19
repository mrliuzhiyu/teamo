import { useCallback, useEffect, useRef, useState } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { ClipboardRow } from "./types";
import { usePanel, UNDO_WINDOW_MS } from "./usePanel";
import StatsHeader from "./StatsHeader";
import SearchBar from "./SearchBar";
import CardList from "./CardList";
import ActionBar from "./ActionBar";
import UndoToast from "./UndoToast";
import PanelSettings from "./PanelSettings";
import WelcomeBanner from "./WelcomeBanner";
import PreviewOverlay from "./PreviewOverlay";
import TabBar, { type PanelTab } from "./TabBar";
import AggregatedView from "./AggregatedView";
import { enterHintLabel } from "../lib/platform";
import { useToast } from "../lib/toast";

type View = "list" | "settings";

export default function PanelApp() {
  const [view, setView] = useState<View>("list");
  const [tab, setTab] = useState<PanelTab>("list");
  const [previewRow, setPreviewRow] = useState<ClipboardRow | null>(null);
  const panel = usePanel();
  const searchRef = useRef<HTMLInputElement>(null);
  const toast = useToast();

  // 监听后端 "panel:open-settings" 事件（tray 右键设置 / 首次启动如需要）
  useEffect(() => {
    const unlisten = listen<void>("panel:open-settings", () => {
      setView("settings");
    });
    return () => {
      void unlisten.then((un) => un());
    };
  }, []);

  const hidePanel = useCallback(async () => {
    await getCurrentWebviewWindow().hide();
  }, []);

  // 复制到系统剪切板（仅 text/url；image 粘贴留 Phase 3B），不关闭窗口
  // 返回 true = 确实写入剪切板；false = 跳过（file 类型 / 空内容 / 写入异常）
  // text/url → writeText；image → 后端 copy_image_to_clipboard（读 PNG + arboard set_image）
  //
  // 成功后 invoke mark_used 标记该条"刚被使用"→ 下次打开面板时 promote 到顶部。
  // 不当前 reload list 避免视觉跳动（该项从中间跳到顶让用户失去锚点）。
  const copyToClipboard = useCallback(async (row: ClipboardRow): Promise<boolean> => {
    try {
      if (row.content_type === "image") {
        if (!row.image_path) return false;
        await invoke("copy_image_to_clipboard", { id: row.id });
      } else if (row.content_type === "text" || row.content_type === "url") {
        const text = row.content ?? "";
        if (!text) return false;
        await writeText(text);
      } else {
        // file 类型：CF_HDROP 粘贴需要平台特殊化，留更后（v0.2+）
        return false;
      }
      // fire-and-forget；mark_used 失败不影响主流程
      void invoke("mark_used", { id: row.id }).catch(() => undefined);
      return true;
    } catch (e) {
      console.error("copyToClipboard failed", e);
      return false;
    }
  }, []);

  // 粘贴某条：复制 + 系统 Ctrl+V + 关面板。粘贴失败时 toast 提示用户手动粘贴。
  // 关键：若 copyToClipboard 返回 false（图片/文件/空/写入失败），
  // 不能 invoke paste_to_previous — 否则 Ctrl+V 会粘贴用户上一次手动复制的内容，
  // 不符合用户选中此条的意图。
  const pasteRow = useCallback(
    async (row: ClipboardRow) => {
      const copied = await copyToClipboard(row);
      if (!copied) {
        toast("error", "复制到剪贴板失败");
        return;
      }
      try {
        await invoke("paste_to_previous");
        await hidePanel();
      } catch (e) {
        // 粘贴链路失败（SetForegroundWindow 失败 / 目标窗口不可达等）：
        // 剪贴板内容已写好，toast 告诉用户手动 Ctrl+V 即可；panel 不 hide，
        // 让用户看得到 toast，自己 Esc 关
        console.warn("paste_to_previous failed:", e);
        toast("error", "目标窗口无法激活 — 内容已在剪贴板，请手动 Ctrl+V");
      }
    },
    [copyToClipboard, hidePanel, toast],
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
    // Settings 视图只处理 Esc 返回，不处理列表导航
    if (view === "settings") {
      const onKey = (e: KeyboardEvent) => {
        // 有 modal dialog 开着时不拦 Esc（dialog 自己处理）
        if (document.querySelector('[data-teamo-dialog="open"]')) return;
        if (e.key === "Escape") {
          e.preventDefault();
          setView("list");
        }
      };
      window.addEventListener("keydown", onKey);
      return () => window.removeEventListener("keydown", onKey);
    }

    // 聚合 tab 暂不支持键盘列表导航（session 展开/收起交互跟 list 不同），
    // 只响应 Esc 关 panel
    if (tab === "aggregated") {
      const onKey = (e: KeyboardEvent) => {
        if (document.querySelector('[data-teamo-dialog="open"]')) return;
        if (e.isComposing) return;
        if (e.key === "Escape") {
          e.preventDefault();
          void hidePanel();
        }
      };
      window.addEventListener("keydown", onKey);
      return () => window.removeEventListener("keydown", onKey);
    }

    const onKey = (e: KeyboardEvent) => {
      // 有 modal dialog 开着时不拦快捷键（Esc / Space 让 dialog 自己处理）
      if (document.querySelector('[data-teamo-dialog="open"]')) return;

      // IME 输入合成中（中文拼音候选未确认）一律不处理 —— 中文用户搜索时按 Enter
      // 是确认候选词，不是触发粘贴。e.isComposing 是浏览器标准属性
      if (e.isComposing) return;

      // F3 或 Space（仅当搜索框为空）→ 打开预览浮层（对标 Ditto F3 / CopyQ F7）
      if (e.key === "F3" || (e.key === " " && panel.query === "")) {
        const row = panel.list[panel.selectedIndex];
        if (row) {
          e.preventDefault();
          setPreviewRow(row);
        }
        return;
      }

      // Esc 渐进退出（VSCode / Obsidian 惯例）：搜索框非空时先清空搜索，再按才关 panel
      if (e.key === "Escape") {
        e.preventDefault();
        if (panel.query) {
          panel.setQuery("");
          return;
        }
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
      // Home / End 跳首尾 —— 大列表快速导航
      if (e.key === "Home") {
        e.preventDefault();
        if (panel.list.length === 0) return;
        panel.setSelectedIndex(0);
        return;
      }
      if (e.key === "End") {
        e.preventDefault();
        if (panel.list.length === 0) return;
        panel.setSelectedIndex(panel.list.length - 1);
        return;
      }
      // PageUp / PageDown 翻 10 条
      if (e.key === "PageUp") {
        e.preventDefault();
        if (panel.list.length === 0) return;
        panel.setSelectedIndex(Math.max(panel.selectedIndex - 10, 0));
        return;
      }
      if (e.key === "PageDown") {
        e.preventDefault();
        if (panel.list.length === 0) return;
        panel.setSelectedIndex(Math.min(panel.selectedIndex + 10, panel.list.length - 1));
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
  }, [panel, handleEnter, hidePanel, view, tab]);

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

  if (view === "settings") {
    return <PanelSettings onBack={() => setView("list")} />;
  }

  const handleForgetFromAggregated = useCallback(
    (row: ClipboardRow) => {
      // 聚合视图里的 forget 暂用简化路径：直接 invoke 后端 forget，不走 undo toast
      // （undo toast 要跟 panel.list 交互，聚合视图不持有 list）
      void invoke("forget_clipboard", { id: row.id }).catch(console.error);
    },
    [],
  );

  return (
    <div className="h-screen flex flex-col bg-white select-none relative">
      <StatsHeader
        stats={panel.stats}
        isPaused={panel.isPaused}
        onClose={() => void hidePanel()}
      />
      <TabBar tab={tab} onChange={setTab} />
      {tab === "list" ? (
        <>
          <WelcomeBanner onOpenSettings={() => setView("settings")} />
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
            hasMore={panel.hasMore}
            loadingMore={panel.loadingMore}
            onSelect={panel.setSelectedIndex}
            onCopy={handleCopy}
            onForget={handleForget}
            onEnter={(r) => void pasteRow(r)}
            onTogglePin={(r) => void panel.togglePin(r)}
            onPreview={(r) => setPreviewRow(r)}
            onLoadMore={() => void panel.loadMore()}
          />
          <div className="px-3 py-1 text-[10px] text-stone-400 bg-stone-50 flex items-center gap-2">
            <kbd className="px-1 py-0.5 bg-white border border-stone-200 rounded text-[9px]">↑↓</kbd>
            <span>选择</span>
            <kbd className="px-1 py-0.5 bg-white border border-stone-200 rounded text-[9px]">Enter</kbd>
            <span>{enterHintLabel}</span>
            <kbd className="px-1 py-0.5 bg-white border border-stone-200 rounded text-[9px]">Space</kbd>
            <span>预览</span>
            <kbd className="px-1 py-0.5 bg-white border border-stone-200 rounded text-[9px]">Del</kbd>
            <span>删除</span>
            {panel.error && <span className="ml-auto text-red-500 truncate">{panel.error}</span>}
          </div>
        </>
      ) : (
        <AggregatedView
          enabled={tab === "aggregated"}
          onPasteItem={(r) => void pasteRow(r)}
          onPreviewItem={(r) => setPreviewRow(r)}
          onForgetItem={handleForgetFromAggregated}
        />
      )}
      <ActionBar
        isPaused={panel.isPaused}
        onTogglePause={() => {
          if (panel.isPaused) void panel.resumeCapture();
          else void panel.pauseCapture(null);
        }}
        onOpenSettings={() => setView("settings")}
      />
      {panel.pendingForget && (
        <UndoToast
          pendingId={panel.pendingForget.row.id}
          onUndo={panel.undoForget}
          durationMs={UNDO_WINDOW_MS}
        />
      )}
      {previewRow && (
        <PreviewOverlay row={previewRow} onClose={() => setPreviewRow(null)} />
      )}
    </div>
  );
}
