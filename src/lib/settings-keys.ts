/**
 * Settings 表 key 常量 —— Rust `src-tauri/src/settings_keys.rs` 的 TS 镜像。
 *
 * 架构约定：
 * - 所有 `get_setting` / `set_setting` 调用必须走这里的常量，不允许内联字符串
 * - 两端常量必须对齐（Rust 侧有测试保证点分命名，TS 侧靠这份文件 + 代码审阅）
 * - 新增设置项：先改 Rust `settings_keys.rs`，再同步这里
 */

// ── UI ──

export const UI_THEME = "ui.theme";
export const UI_THEME_DEFAULT = "system"; // "system" | "light" | "dark"

// ── Filter（端侧闸门）──

export const FILTER_MIN_TEXT_LEN = "filter.min_text_len";
export const FILTER_MIN_TEXT_LEN_DEFAULT = "0";

// ── Sensitive 6 类开关（默认全开 = "1"）──

export const SENS_PASSWORD = "sens.password";
export const SENS_TOKEN = "sens.token";
export const SENS_CREDIT_CARD = "sens.credit_card";
export const SENS_ID_CARD = "sens.id_card";
export const SENS_PHONE = "sens.phone";
export const SENS_EMAIL = "sens.email";

export const SENS_DEFAULT_ON = "1";

// ── Capture ──

export const CAPTURE_PAUSED_UNTIL = "capture.paused_until";

// ── Data ──

export const DATA_RETENTION = "data.retention";
export const DATA_RETENTION_DEFAULT = "forever"; // "forever" | "1y" | "6m" | "1m"

// ── Cloud ──

export const CLOUD_LOGGED_IN_USER_ID = "cloud.logged_in_user_id";
export const CLOUD_SYNC_ENABLED = "cloud.sync_enabled";
export const CLOUD_SYNC_ENABLED_DEFAULT = "0";
