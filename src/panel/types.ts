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
  /// 置顶时间戳（Unix ms），null = 未置顶。pin 项聚集在列表顶部
  pinned_at: number | null;
  /// 上次被使用时间戳（Unix ms），null = 从未使用。用过的项会 promote 到顶部
  last_used_at: number | null;
  /// 图片宽高（仅 content_type='image' 有值）— ingest 时存下避免前端再 decode
  image_width: number | null;
  image_height: number | null;
  /// L1 session 归属（聚合 tab 按此分组）
  session_id: string | null;
  /// L1 字符串子集父 row id（on-the-fly 计算，不写 DB；聚合 tab 用于展示缩进父子）
  parent_id: string | null;
}

export interface TodayStats {
  captured: number;
  blocked: number;
  uploaded: number;
}
