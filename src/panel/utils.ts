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
  const now_d = new Date();
  const yest = new Date(now_d.getFullYear(), now_d.getMonth(), now_d.getDate() - 1);
  const pad = (n: number) => String(n).padStart(2, "0");
  if (d.toDateString() === yest.toDateString()) {
    return `昨天 ${pad(d.getHours())}:${pad(d.getMinutes())}`;
  }
  // 同年不显示年份；跨年显示 "2024 年 12 月 15 日" 避免歧义
  if (d.getFullYear() === now_d.getFullYear()) {
    return `${d.getMonth() + 1} 月 ${d.getDate()} 日`;
  }
  return `${d.getFullYear()} 年 ${d.getMonth() + 1} 月 ${d.getDate()} 日`;
}

export function formatPreview(row: ClipboardRow, maxLen = 80, query?: string): string {
  if (row.sensitive_type) {
    // AC-4 原型："••••••• 从 1Password"（只显示来源，不显示 sensitive_type 冗余信息）
    return row.source_app
      ? `••••••• 从 ${row.source_app}`
      : `••••••• (${row.sensitive_type})`;
  }
  if (row.content_type === "image") {
    return `[图片] ${row.image_path ?? ""}`;
  }
  if (row.content_type === "file") {
    return `[文件] ${row.file_path ?? ""}`;
  }
  const raw = row.content ?? "";
  const clean = raw.replace(/\s+/g, " ").trim();
  // Unicode code point 切片，避免 emoji surrogate pair 中间截断
  const chars = Array.from(clean);
  if (chars.length <= maxLen) return clean;

  // 搜索模式：命中词在截断范围外时，抽取命中前后 ~30 字的上下文窗口
  // （对标 Ditto F3 预览外的"命中定位"需求；避免长文命中在后半段 line-clamp
  //  只显示前 2 行导致 hit 看不见的 bug）
  const q = query?.trim().toLowerCase();
  if (q) {
    const lower = clean.toLowerCase();
    const hitIdx = lower.indexOf(q);
    // 命中在 maxLen 边界外（或虽在内但距开头 > 40 更易看不清）时取窗口
    if (hitIdx > 40 && hitIdx + q.length < chars.length) {
      const hitChars = Array.from(clean.slice(0, hitIdx)).length;
      const CONTEXT = 30;
      const start = Math.max(0, hitChars - CONTEXT);
      const hitEndChars = hitChars + Array.from(q).length;
      const end = Math.min(chars.length, hitEndChars + CONTEXT);
      const prefix = start > 0 ? "…" : "";
      const suffix = end < chars.length ? "…" : "";
      return prefix + chars.slice(start, end).join("") + suffix;
    }
  }

  return chars.slice(0, maxLen).join("") + "…";
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
