import type { ClipboardRow } from "./types";

export function formatRelativeTime(tsMs: number): string {
  const now = Date.now();
  const diff = Math.max(0, now - tsMs);
  const sec = Math.floor(diff / 1000);
  if (sec < 60) return "刚刚";
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min} 分钟前`;
  const hour = Math.floor(min / 60);
  if (hour < 24) return `${hour} 小时前`;

  const d = new Date(tsMs);
  const today = new Date();
  const yest = new Date(today.getFullYear(), today.getMonth(), today.getDate() - 1);
  const pad = (n: number) => String(n).padStart(2, "0");
  if (d.toDateString() === yest.toDateString()) {
    return `昨天 ${pad(d.getHours())}:${pad(d.getMinutes())}`;
  }
  return `${d.getMonth() + 1} 月 ${d.getDate()} 日`;
}

export function formatPreview(row: ClipboardRow, maxLen = 80): string {
  if (row.sensitive_type) {
    const src = row.source_app ? ` 从 ${row.source_app}` : "";
    return `••••••• ${row.sensitive_type}${src}`;
  }
  if (row.content_type === "image") {
    return `[图片] ${row.image_path ?? ""}`;
  }
  if (row.content_type === "file") {
    return `[文件] ${row.file_path ?? ""}`;
  }
  const raw = row.content ?? "";
  const clean = raw.replace(/\s+/g, " ").trim();
  return clean.length > maxLen ? clean.slice(0, maxLen) + "…" : clean;
}

export type StateBadge = { label: string; tone: "local" | "cloud" | "blocked" };

export function getStateBadge(row: ClipboardRow): StateBadge {
  if (row.blocked_reason) return { label: "拦截", tone: "blocked" };
  if (row.state === "uploaded") return { label: "已上云", tone: "cloud" };
  return { label: "仅本地", tone: "local" };
}

export function highlightMatches(text: string, query: string): Array<{ text: string; hit: boolean }> {
  if (!query.trim()) return [{ text, hit: false }];
  const q = query.trim().toLowerCase();
  const lower = text.toLowerCase();
  const parts: Array<{ text: string; hit: boolean }> = [];
  let i = 0;
  while (i < text.length) {
    const idx = lower.indexOf(q, i);
    if (idx === -1) {
      parts.push({ text: text.slice(i), hit: false });
      break;
    }
    if (idx > i) parts.push({ text: text.slice(i, idx), hit: false });
    parts.push({ text: text.slice(idx, idx + q.length), hit: true });
    i = idx + q.length;
  }
  return parts;
}
