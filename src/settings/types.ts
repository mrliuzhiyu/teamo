// 与 Rust 后端 DataInfo 对齐（serde 默认 snake_case）
export interface DataInfo {
  data_dir: string;
  db_path: string;
  db_bytes: number;
  image_count: number;
  image_bytes: number;
}

/// settings 表存储的键位（key），值统一 string 类型，语义见各 section
export const SETTING_KEYS = {
  theme: "ui.theme",                   // "system" | "light" | "dark"
  minTextLen: "filter.min_text_len",  // string int, 默认 "8"
  retention: "data.retention",         // "forever" | "1y" | "6m" | "1m"
  // 敏感检测 6 个开关
  sensPassword: "sens.password",
  sensToken: "sens.token",
  sensCreditCard: "sens.credit_card",
  sensIdCard: "sens.id_card",
  sensPhone: "sens.phone",
  sensEmail: "sens.email",
} as const;
