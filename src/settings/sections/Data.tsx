import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openShell } from "@tauri-apps/plugin-shell";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import Section, { Row } from "../components/Section";
import { formatBytes, useDataInfo, useSetting } from "../useSettings";
import { DATA_RETENTION, DATA_RETENTION_DEFAULT } from "../../lib/settings-keys";

const RETENTION_OPTIONS: Array<{ value: string; label: string; hint: string }> = [
  { value: "forever", label: "永久", hint: "不自动清理（默认）" },
  { value: "1y", label: "最近 1 年", hint: "超过 1 年的记录启动时自动清除" },
  { value: "6m", label: "最近 6 月", hint: "超过 6 个月的记录启动时自动清除" },
  { value: "1m", label: "最近 1 月", hint: "超过 30 天的记录启动时自动清除" },
];

type ExportStatus = null | { tone: "ok" | "err"; text: string };

export default function Data() {
  const { info, refresh } = useDataInfo();
  const [retention, setRetention] = useSetting(DATA_RETENTION, DATA_RETENTION_DEFAULT);
  const [exportStatus, setExportStatus] = useState<ExportStatus>(null);
  const [exporting, setExporting] = useState(false);
  const [clearing, setClearing] = useState(false);

  const retentionHint =
    RETENTION_OPTIONS.find((o) => o.value === retention)?.hint ??
    "下次启动时生效";

  const openDir = async () => {
    if (!info) return;
    try {
      await openShell(info.data_dir);
    } catch (e) {
      console.error("open data dir", e);
    }
  };

  const doExport = async (format: "json" | "markdown") => {
    // Phase 1 用 dialog.save 让用户选目标目录；Phase 2 换专门的 selectDirectory
    let target: string | null = null;
    try {
      target = await saveDialog({
        title: "选择导出目录（将创建 teamo-export-*/ 子目录）",
        defaultPath: "teamo-export",
      });
    } catch (e) {
      console.error("save dialog", e);
      return;
    }
    if (!target) return;
    // dialog.save 返回的是文件路径，取其父目录作为 target_parent
    const parent = target.replace(/[\\/][^\\/]*$/, "");
    setExporting(true);
    setExportStatus(null);
    try {
      const result = await invoke<{
        exported_count: number;
        image_count: number;
        missing_images: number;
        target_dir: string;
      }>("export_data", { format, targetDir: parent });
      setExportStatus({
        tone: "ok",
        text: `导出 ${result.exported_count} 条 + ${result.image_count} 张图片 → ${result.target_dir}`,
      });
    } catch (e) {
      setExportStatus({ tone: "err", text: `导出失败：${e}` });
    } finally {
      setExporting(false);
    }
  };

  const doClear = async () => {
    const ok = window.confirm(
      "确认清空所有本地剪切板数据？\n\n此操作删除 clipboard_local 全部行 + images/ 所有图片。\n设置/规则不受影响。此操作不可撤销。",
    );
    if (!ok) return;
    setClearing(true);
    try {
      await invoke("clear_all_data");
      await refresh();
      setExportStatus({ tone: "ok", text: "已清空本地数据" });
    } catch (e) {
      setExportStatus({ tone: "err", text: `清空失败：${e}` });
    } finally {
      setClearing(false);
    }
  };

  return (
    <Section title="数据" description="本地存储位置 · 导出 · 清空">
      <Row label="本地存储位置" hint={info?.data_dir ?? "加载中..."}>
        <button
          onClick={openDir}
          disabled={!info}
          className="text-[11px] px-2 py-1 bg-stone-100 hover:bg-stone-200 rounded disabled:opacity-40"
        >
          在文件管理器打开
        </button>
      </Row>
      <Row
        label="存储占用"
        hint={
          info
            ? `clipboard.db ${formatBytes(info.db_bytes)} · ${info.image_count} 张图 ${formatBytes(info.image_bytes)}`
            : "加载中..."
        }
      >
        <button
          onClick={() => void refresh()}
          className="text-[11px] text-stone-500 hover:text-stone-700"
        >
          刷新
        </button>
      </Row>
      <Row label="保留时长" hint={retentionHint}>
        <select
          value={retention}
          onChange={(e) => void setRetention(e.target.value)}
          className="text-[11px] px-2 py-1 bg-white border border-stone-200 rounded focus:outline-none focus:border-stone-400"
        >
          {RETENTION_OPTIONS.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
      </Row>
      <Row label="导出全部数据" hint="JSON 或 Markdown + 图片副本">
        <div className="flex items-center gap-2">
          <button
            onClick={() => void doExport("json")}
            disabled={exporting}
            className="text-[11px] px-2 py-1 bg-stone-900 text-white rounded disabled:opacity-40"
          >
            导出 JSON
          </button>
          <button
            onClick={() => void doExport("markdown")}
            disabled={exporting}
            className="text-[11px] px-2 py-1 bg-white border border-stone-300 rounded disabled:opacity-40"
          >
            导出 Markdown
          </button>
        </div>
      </Row>
      <Row
        danger
        label="清空本地数据"
        hint="不可撤销 — 删除所有剪切板记录 + 图片，但保留设置和规则"
      >
        <button
          onClick={() => void doClear()}
          disabled={clearing}
          className="text-[11px] px-2 py-1 bg-red-50 text-red-700 border border-red-200 rounded hover:bg-red-100 disabled:opacity-40"
        >
          {clearing ? "清空中..." : "清空数据"}
        </button>
      </Row>
      {exportStatus && (
        <div
          className={`px-4 py-2 text-[11px] ${
            exportStatus.tone === "ok"
              ? "bg-emerald-50 text-emerald-700"
              : "bg-red-50 text-red-700"
          }`}
        >
          {exportStatus.text}
        </div>
      )}
    </Section>
  );
}
