import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ClipboardRow } from "./types";
import type { SessionSummary } from "./useAggregated";
import { formatPreview, formatRelativeTime } from "./utils";
import { useAuth } from "../settings/useAuth";
import { useToast } from "../lib/toast";

interface UploadProgress {
  session_id: string;
  stage: "preparing" | "uploading_images" | "creating_memo";
  current: number;
  total: number;
}

interface Props {
  session: SessionSummary;
  expanded: boolean;
  items: ClipboardRow[];
  expandLoading: boolean;
  onToggleExpand: () => void;
  onPasteItem: (row: ClipboardRow) => void;
  onPreviewItem: (row: ClipboardRow) => void;
  onForgetItem: (row: ClipboardRow) => void;
}

/// 聚合 tab 一个 session 卡片：
/// - 收起态：来源 App + 预览 + 片段数 + 时间跨度（像 Gmail 会话线程）
/// - 展开态：session 内所有 items（类似文件夹展开）
///
/// 暂不做组级操作（上云/pin 整组），R3 上云时补。
interface UploadResult {
  uploaded_count: number;
  skipped_items: number;
  included_items: number;
}

export default function SessionCard({
  session,
  expanded,
  items,
  expandLoading,
  onToggleExpand,
  onPasteItem,
  onPreviewItem,
  onForgetItem,
}: Props) {
  const timeSpan = formatTimeSpan(session.started_at, session.ended_at);
  const { state: authState } = useAuth();
  const toast = useToast();
  const [uploading, setUploading] = useState(false);
  const [progress, setProgress] = useState<UploadProgress | null>(null);
  const [justUploaded, setJustUploaded] = useState(false);
  const isUploaded = !!session.uploaded_at || justUploaded;

  // 订阅 upload:progress,只认 session_id 匹配的事件(多 session 并行上云时互不干扰)
  useEffect(() => {
    if (!uploading) return;
    const unlistenPromise = listen<UploadProgress>("upload:progress", (e) => {
      if (e.payload.session_id === session.session_id) {
        setProgress(e.payload);
      }
    });
    return () => {
      void unlistenPromise.then((un) => un());
    };
  }, [uploading, session.session_id]);

  // 根据 stage 生成按钮文字
  const uploadingLabel = (() => {
    if (!progress) return "⏳ 准备中";
    switch (progress.stage) {
      case "preparing":
        return "⏳ 准备中";
      case "uploading_images":
        return progress.total > 0
          ? `⏳ 上传图片 ${progress.current}/${progress.total}`
          : "⏳ 上传图片";
      case "creating_memo":
        return "⏳ 整理 memo";
      default:
        return "⏳ 上云中";
    }
  })();

  const handleUpload = async (e: React.MouseEvent) => {
    e.stopPropagation(); // 防止点按钮时误触发展开
    if (!authState?.logged_in) {
      toast("error", "请先在设置里登录 TextView");
      return;
    }
    setUploading(true);
    setProgress(null);
    try {
      const result = await invoke<UploadResult>("upload_session", {
        sessionId: session.session_id,
      });
      const msg =
        result.skipped_items > 0
          ? `已上云 ${result.included_items} 条 · 跳过 ${result.skipped_items} 条（敏感/图片）`
          : `已上云 ${result.included_items} 条`;
      toast("success", msg);
      setJustUploaded(true);
    } catch (err) {
      toast("error", `上云失败：${err}`);
    } finally {
      setUploading(false);
      setProgress(null);
    }
  };

  return (
    <div className="rounded-lg bg-stone-100 overflow-hidden">
      {/* 卡片头：点击展开 + 右上角上云按钮 */}
      <div className="relative">
        <button
          onClick={onToggleExpand}
          className="w-full text-left px-3 py-2.5 flex items-start gap-2 hover:bg-stone-200 transition-colors"
        >
          <div className="flex-shrink-0 mt-0.5 text-stone-400">
            <svg
              width="10"
              height="10"
              viewBox="0 0 10 10"
              className={`transition-transform ${expanded ? "rotate-90" : ""}`}
            >
              <path d="M3 2L7 5L3 8" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </div>
          <div className="flex-1 min-w-0 pr-16">
            <div className="flex items-center gap-2 text-[11px] text-stone-500 mb-0.5">
              {session.primary_source_app && (
                <span className="text-stone-700 font-medium truncate max-w-[140px]">
                  {session.primary_source_app}
                </span>
              )}
              <span className="px-1.5 py-0.5 rounded bg-stone-200/60 text-stone-600 flex-shrink-0">
                {session.item_count} 条
              </span>
              {session.has_image && (
                <span className="px-1.5 py-0.5 rounded bg-emerald-100/60 text-emerald-700 flex-shrink-0" title="含图片">
                  📷
                </span>
              )}
              {session.has_sensitive && (
                <span className="px-1.5 py-0.5 rounded bg-amber-100/60 text-amber-700 flex-shrink-0" title="含敏感内容">
                  🔒
                </span>
              )}
              <span className="ml-auto text-stone-400 flex-shrink-0" title={new Date(session.ended_at).toLocaleString()}>
                {timeSpan}
              </span>
            </div>
            <div className="text-sm text-stone-800 line-clamp-2 break-all">
              {session.first_preview}
            </div>
          </div>
        </button>
        {/* 上云按钮：绝对定位右上，点击不触发展开 */}
        <button
          onClick={handleUpload}
          disabled={uploading || !authState?.logged_in}
          className={`absolute right-2 top-2 text-[10px] px-2 py-0.5 rounded shadow-sm transition-colors ${
            isUploaded
              ? "bg-emerald-50 text-emerald-700 border border-emerald-200"
              : authState?.logged_in
                ? "bg-white text-stone-600 border border-stone-200 hover:bg-stone-50"
                : "bg-stone-50 text-stone-400 border border-stone-200 cursor-not-allowed"
          }`}
          title={
            !authState?.logged_in
              ? "登录 TextView 后可上云"
              : isUploaded
                ? `已上云 · ${session.uploaded_at ? new Date(session.uploaded_at).toLocaleString() : "刚刚"}（可再次上云覆盖）`
                : "将此 session 整理成 memo 上云"
          }
        >
          {uploading ? uploadingLabel : isUploaded ? "✓ 已上云" : "📤 上云"}
        </button>
      </div>

      {/* 展开态：items 子列表（带父子结构） */}
      {expanded && (
        <div className="bg-white border-t border-stone-200">
          {expandLoading ? (
            <div className="px-4 py-3 text-[12px] text-stone-400">加载中…</div>
          ) : items.length === 0 ? (
            <div className="px-4 py-3 text-[12px] text-stone-400">此 session 无 items</div>
          ) : (
            <ExpandedItems
              items={items}
              onPasteItem={onPasteItem}
              onPreviewItem={onPreviewItem}
              onForgetItem={onForgetItem}
            />
          )}
        </div>
      )}
    </div>
  );
}

/// 把 items 按 parent_id 分组成父子树（扁平两层）展示。
/// - parent_id=null 的是顶层（主文 / 独立片段）
/// - parent_id 非空的挂到对应父下（缩进）
/// - 找不到对应父的孤儿当顶层处理
function ExpandedItems({
  items,
  onPasteItem,
  onPreviewItem,
  onForgetItem,
}: {
  items: ClipboardRow[];
  onPasteItem: (row: ClipboardRow) => void;
  onPreviewItem: (row: ClipboardRow) => void;
  onForgetItem: (row: ClipboardRow) => void;
}) {
  // index by id
  const byId = new Map(items.map((r) => [r.id, r]));
  const childrenOf = new Map<string, ClipboardRow[]>();
  const topLevel: ClipboardRow[] = [];

  for (const row of items) {
    const pid = row.parent_id;
    if (pid && byId.has(pid)) {
      const arr = childrenOf.get(pid) ?? [];
      arr.push(row);
      childrenOf.set(pid, arr);
    } else {
      topLevel.push(row);
    }
  }

  // 顶层按 captured_at DESC（最新的主文在前）
  topLevel.sort((a, b) => b.captured_at - a.captured_at);
  // 子项按 captured_at ASC（按用户复制的原始顺序展示）
  for (const [, arr] of childrenOf) {
    arr.sort((a, b) => a.captured_at - b.captured_at);
  }

  return (
    <div className="divide-y divide-stone-100">
      {topLevel.map((parent) => {
        const kids = childrenOf.get(parent.id) ?? [];
        return (
          <div key={parent.id}>
            <SessionItemRow
              row={parent}
              isParent={kids.length > 0}
              onPaste={() => onPasteItem(parent)}
              onPreview={() => onPreviewItem(parent)}
              onForget={() => onForgetItem(parent)}
            />
            {kids.map((k) => (
              <SessionItemRow
                key={k.id}
                row={k}
                isChild
                onPaste={() => onPasteItem(k)}
                onPreview={() => onPreviewItem(k)}
                onForget={() => onForgetItem(k)}
              />
            ))}
          </div>
        );
      })}
    </div>
  );
}

function SessionItemRow({
  row,
  isParent,
  isChild,
  onPaste,
  onPreview,
  onForget,
}: {
  row: ClipboardRow;
  isParent?: boolean;
  isChild?: boolean;
  onPaste: () => void;
  onPreview: () => void;
  onForget: () => void;
}) {
  const isImage = row.content_type === "image";
  const preview = formatPreview(row, 120);
  return (
    <div
      className={`flex items-start gap-2 hover:bg-stone-50 group ${
        isChild ? "pl-7 pr-3 py-1.5 bg-stone-50/50" : "px-3 py-2"
      } ${isParent ? "bg-stone-50/70" : ""}`}
    >
      {isChild && (
        <span className="flex-shrink-0 text-stone-300 text-[14px] leading-4 self-start" title="引用自上方主文">
          ↳
        </span>
      )}
      <div
        className={`flex-1 min-w-0 break-all ${
          isChild ? "text-[11px] text-stone-600 line-clamp-1" : "text-[12px] text-stone-700 line-clamp-2"
        } ${isParent ? "font-medium text-stone-900" : ""}`}
      >
        {isImage
          ? `🖼️ ${row.image_width ?? "?"} × ${row.image_height ?? "?"}`
          : preview}
      </div>
      <div className="flex-shrink-0 flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
        <button
          onClick={onPreview}
          className="text-[10px] px-1.5 py-0.5 rounded bg-white hover:bg-stone-100 text-stone-500 shadow-sm"
          title="预览"
        >
          预览
        </button>
        <button
          onClick={onPaste}
          className="text-[10px] px-1.5 py-0.5 rounded bg-white hover:bg-stone-100 text-stone-500 shadow-sm"
          title="粘贴到当前窗口"
        >
          粘贴
        </button>
        <button
          onClick={onForget}
          className="text-[10px] px-1.5 py-0.5 rounded bg-white hover:bg-red-50 hover:text-red-600 text-stone-500 shadow-sm"
          title="删除这条"
        >
          删除
        </button>
      </div>
      <span className="flex-shrink-0 text-[10px] text-stone-400 self-center" title={new Date(row.captured_at).toLocaleString()}>
        {formatRelativeTime(row.captured_at)}
      </span>
    </div>
  );
}

/// 时间跨度友好展示
/// 1 分钟内：刚刚
/// 同一小时：N 分钟前
/// 同一天：HH:mm-HH:mm
/// 更早：用 formatRelativeTime 统一
function formatTimeSpan(startMs: number, endMs: number): string {
  const span = endMs - startMs;
  const now = Date.now();
  const sinceEnd = now - endMs;

  if (sinceEnd < 60_000) return "刚刚";
  if (sinceEnd < 60 * 60_000) return `${Math.floor(sinceEnd / 60_000)} 分钟前`;

  if (span < 60_000) {
    // 单时刻
    const d = new Date(endMs);
    return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  }

  const startD = new Date(startMs);
  const endD = new Date(endMs);
  const sameDay = startD.toDateString() === endD.toDateString();
  if (sameDay) {
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${pad(startD.getHours())}:${pad(startD.getMinutes())} – ${pad(endD.getHours())}:${pad(endD.getMinutes())}`;
  }

  // 跨天：简化显示 N 天前
  const days = Math.floor(sinceEnd / (24 * 60 * 60_000));
  return `${days} 天前`;
}
