// 与 Rust 后端 DataInfo 对齐（serde 默认 snake_case）
export interface DataInfo {
  data_dir: string;
  db_path: string;
  db_bytes: number;
  image_count: number;
  image_bytes: number;
}

// 设置 key 常量已迁移到 src/lib/settings-keys.ts（与 Rust settings_keys.rs 双源对齐）。
// 请从那里 import 使用。
