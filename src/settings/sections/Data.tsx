import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openShell } from "@tauri-apps/plugin-shell";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import Section, { Row } from "../components/Section";
import Select from "../components/Select";
import { formatBytes, useDataInfo, useSetting } from "../useSettings";
import { DATA_RETENTION, DATA_RETENTION_DEFAULT } from "../../lib/settings-keys";
import { useToast } from "../../lib/toast";
import { useConfirm } from "../../lib/ConfirmDialog";

const RETENTION_OPTIONS: Array<{ value: string; label: string; hint: string }> = [
  { value: "forever", label: "永久", hint: "不自动清理（默认）" },
  { value: "1y", label: "最近 1 年", hint: "超过 1 年的记录启动时自动清除" },
  { value: "6m", label: "最近 6 月", hint: "超过 6 个月的记录启动时自动清除" },
  { value: "1m", label: "最近 1 月", hint: "超过 30 天的记录启动时自动清除" },
];

export default function Data() {
  const { info, refresh } = useDataInfo();
  const [retention, setRetention] = useSetting(DATA_RETENTION, DATA_RETENTION_DEFAULT);
  const [exporting, setExporting] = useState(false);
  const [clearing, setClearing] = useState(false);
  const toast = useToast();
  const confirm = useConfirm();

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
    let parent: string | null = null;
    try {
      const picked = await openDialog({
        title: "选择导出目录（将创建 teamo-export-* 子目录）",
        directory: true,
        multiple: false,
      });
      parent = typeof picked === "string" ? picked : null;
    } catch (e) {
      console.error("open dir dialog", e);
      return;
    }
    if (!parent) return;
    setExporting(true);
    try {
      const result = await invoke<{
        exported_count: number;
        image_count: number;
        missing_images: number;
        target_dir: string;
      }>("export_data", { format, targetDir: parent });
      toast("success", `导出 ${result.exported_count} 条 + ${result.image_count} 张图片`);
      // 导出成功后自动打开目录方便用户查看
      void openShell(result.target_dir).catch(() => undefined);
    } catch (e) {
      toast("error", `导出失败：${e}`);
    } finally {
      setExporting(false);
    }
  };

  const doClear = async () => {
    const ok = await confirm({
      title: "清空所有本地剪切板数据？",
      body: "此操作删除全部剪切板记录 + 图片。\n设置和规则不受影响。\n此操作不可撤销。",
      confirmText: "清空",
      cancelText: "取消",
      danger: true,
    });
    if (!ok) return;
    setClearing(true);
    try {
      await invoke("clear_all_data");
      await refresh();
      toast("success", "已清空本地数据");
    } catch (e) {
      toast("error", `清空失败：${e}`);
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
          className="inline-flex items-center gap-1.5 text-[11px] px-2 py-1 bg-stone-100 hover:bg-stone-200 rounded disabled:opacity-40"
        >
          <svg width="11" height="11" viewBox="0 0 12 12" fill="none" className="flex-shrink-0">
            <path
              d="M1.5 3.5C1.5 2.94772 1.94772 2.5 2.5 2.5H5L6 3.5H9.5C10.0523 3.5 10.5 3.94772 10.5 4.5V9C10.5 9.55228 10.0523 10 9.5 10H2.5C1.94772 10 1.5 9.55228 1.5 9V3.5Z"
              stroke="currentColor"
              strokeWidth="1.2"
              fill="none"
              strokeLinejoin="round"
            />
          </svg>
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
          className="inline-flex items-center gap-1 text-[11px] px-2 py-1 bg-stone-100 hover:bg-stone-200 rounded text-stone-600"
          title="重新计算存储占用"
        >
          <svg width="11" height="11" viewBox="0 0 12 12" fill="none" className="flex-shrink-0">
            <path
              d="M10 3V5.5H7.5M2 9V6.5H4.5M10 5.5C9.5 3.5 7.8 2 6 2C4.5 2 3.2 2.8 2.5 4M2 6.5C2.5 8.5 4.2 10 6 10C7.5 10 8.8 9.2 9.5 8"
              stroke="currentColor"
              strokeWidth="1.2"
              fill="none"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
          刷新
        </button>
      </Row>
      <Row label="保留时长" hint={retentionHint}>
        <Select
          value={retention}
          options={RETENTION_OPTIONS.map((o) => ({ value: o.value, label: o.label }))}
          onChange={(v) => void setRetention(v)}
        />
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
    </Section>
  );
}
