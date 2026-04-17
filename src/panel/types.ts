export interface ClipboardRow {
  id: string;
  content_hash: string;
  content: string | null;
  content_type: string;
  size_bytes: number | null;
  image_path: string | null;
  file_path: string | null;
  source_app: string | null;
  source_url: string | null;
  source_title: string | null;
  captured_at: number;
  sensitive_type: string | null;
  blocked_reason: string | null;
  state: string;
  server_id: string | null;
  occurrence_count: number;
  last_seen_at: number | null;
  created_at: number;
  updated_at: number;
  /// URL 命中的 domain_rule（"parse_as_content:v.douyin.com/*" 等，可选显示）
  matched_domain_rule: string | null;
}

export interface TodayStats {
  captured: number;
  blocked: number;
  uploaded: number;
}
