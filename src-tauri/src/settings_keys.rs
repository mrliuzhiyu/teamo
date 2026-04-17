// 本模块是 Rust/TS 双源常量对齐的 Rust 侧，部分常量在 Rust 业务代码里暂未消费
// 但 TS 侧（src/lib/settings-keys.ts）正在用 —— Rust 编译器看不见 TS 的使用，
// 会对 `UI_THEME` / `CLOUD_*` 等报 dead_code。保留这些常量是**架构约定**：
// 将来 Rust 侧接入这些设置项（比如 Phase 2 做主题切换 / Phase 3 做云端同步）
// 时就能直接用常量，避免再开一套命名。所以模块级 allow 下这类警告。
#![allow(dead_code)]

//! Settings 表 key 常量 + 默认值单一源。
//!
//! 架构约定：
//! - 所有业务层读写 settings 表必须走这里的常量，不允许内联字符串
//! - 命名空间分层：`ui.*` / `filter.*` / `sens.*` / `capture.*` / `data.*` / `cloud.*`
//! - 默认值作为 `*_DEFAULT` 常量并列定义，业务层 `unwrap_or(DEFAULT)`
//! - 前端 `src/lib/settings-keys.ts` 镜像同样的常量，两端通过 migration 测试对齐
//!
//! 为什么这样：原先 migration 001 里 INSERT 一堆 `autostart`/`theme`/`min_text_length` 简单键，
//! 前端却用 `ui.theme`/`filter.min_text_len` 点分键，两套并行谁都读不到谁。
//! 现在统一：migration 002 清理老键，业务层只用本模块的常量。

// ── App（全局应用状态）──

/// 首次启动引导完成标记 —— 为空表示首次启动，setup 里会自动 show main window
/// 展示设置页作为引导，之后默认静默到 tray（符合后台守护应用的行为预期）
pub const APP_FIRST_RUN_COMPLETED: &str = "app.first_run_completed";

// ── UI ──

pub const UI_THEME: &str = "ui.theme";
pub const UI_THEME_DEFAULT: &str = "system"; // "system" / "light" / "dark"

// ── Filter（端侧闸门）──

/// 短于此字符数的纯文本不计入（不是 local_only，是直接跳过 insert）
pub const FILTER_MIN_TEXT_LEN: &str = "filter.min_text_len";
pub const FILTER_MIN_TEXT_LEN_DEFAULT: &str = "0"; // Phase 1 默认 0 不过滤，架构设想 8

/// DB 里当前内置 domain_rules 的源版本号。seed_rules 启动时比对 YAML 顶部 version：
/// - 相等 / DB 版本更高 → 不 seed（用户可能 user-modified 过，别碰）
/// - DB 版本较低（或无记录）→ 清空 builtin 重 seed（保留 user/cloud 规则）
pub const FILTER_BUILTIN_RULES_VERSION: &str = "filter.builtin_rules_version";

// ── Sensitive 6 类开关（默认全开 = "1"）──

pub const SENS_PASSWORD: &str = "sens.password";
pub const SENS_TOKEN: &str = "sens.token";
pub const SENS_CREDIT_CARD: &str = "sens.credit_card";
pub const SENS_ID_CARD: &str = "sens.id_card";
pub const SENS_PHONE: &str = "sens.phone";
pub const SENS_EMAIL: &str = "sens.email";

pub const SENS_DEFAULT_ON: &str = "1";

// ── Capture ──

/// 暂停状态（"manual" / Unix ms timestamp / 空）
pub const CAPTURE_PAUSED_UNTIL: &str = "capture.paused_until";

// ── Data ──

/// 保留时长，枚举值："forever" / "1y" / "6m" / "1m"
pub const DATA_RETENTION: &str = "data.retention";
pub const DATA_RETENTION_DEFAULT: &str = "forever";

// ── Cloud ──

pub const CLOUD_LOGGED_IN_USER_ID: &str = "cloud.logged_in_user_id";
pub const CLOUD_SYNC_ENABLED: &str = "cloud.sync_enabled";
pub const CLOUD_SYNC_ENABLED_DEFAULT: &str = "0";

// ── 便利函数 ──

/// 读 bool 开关：有值按 "1"/"0" 解析，无值走默认
pub fn read_bool_flag(
    conn: &rusqlite::Connection,
    key: &str,
    default_on: bool,
) -> bool {
    use crate::storage::repository;
    match repository::get_setting(conn, key) {
        Ok(Some(v)) => v == "1",
        _ => default_on,
    }
}

/// 读 i64 整数设置：有值 parse，失败/无值走默认
pub fn read_i64(
    conn: &rusqlite::Connection,
    key: &str,
    default: i64,
) -> i64 {
    use crate::storage::repository;
    match repository::get_setting(conn, key) {
        Ok(Some(v)) => v.parse().unwrap_or(default),
        _ => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{repository, schema};

    fn setup_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        schema::run_migrations(&conn).unwrap();
        conn
    }

    /// 命名规范测试：所有 key 必须用点分命名空间
    #[test]
    fn test_key_naming_convention() {
        let all_keys = [
            APP_FIRST_RUN_COMPLETED,
            UI_THEME,
            FILTER_MIN_TEXT_LEN,
            FILTER_BUILTIN_RULES_VERSION,
            SENS_PASSWORD,
            SENS_TOKEN,
            SENS_CREDIT_CARD,
            SENS_ID_CARD,
            SENS_PHONE,
            SENS_EMAIL,
            CAPTURE_PAUSED_UNTIL,
            DATA_RETENTION,
            CLOUD_LOGGED_IN_USER_ID,
            CLOUD_SYNC_ENABLED,
        ];
        for key in all_keys {
            assert!(
                key.contains('.'),
                "key '{key}' must use dotted namespace (e.g. 'ui.theme')"
            );
            assert!(
                !key.contains(' '),
                "key '{key}' must not contain whitespace"
            );
        }
    }

    // ── T3 · read_bool_flag / read_i64 helper 补测 ──

    #[test]
    fn test_read_bool_flag_returns_default_when_unset() {
        let conn = setup_db();
        assert!(read_bool_flag(&conn, "nonexistent.key", true));
        assert!(!read_bool_flag(&conn, "nonexistent.key", false));
    }

    #[test]
    fn test_read_bool_flag_parses_1_0() {
        let conn = setup_db();
        repository::set_setting(&conn, "test.flag", Some("1")).unwrap();
        assert!(read_bool_flag(&conn, "test.flag", false));

        repository::set_setting(&conn, "test.flag", Some("0")).unwrap();
        assert!(!read_bool_flag(&conn, "test.flag", true));
    }

    #[test]
    fn test_read_bool_flag_non_1_is_false() {
        let conn = setup_db();
        // 任何非 "1" 值都视作 false（包括 "true" / "yes" / 空字符串）
        repository::set_setting(&conn, "test.flag", Some("true")).unwrap();
        assert!(!read_bool_flag(&conn, "test.flag", true));
    }

    #[test]
    fn test_read_i64_returns_default_when_unset() {
        let conn = setup_db();
        assert_eq!(read_i64(&conn, "nonexistent.key", 42), 42);
    }

    #[test]
    fn test_read_i64_parses_valid_numbers() {
        let conn = setup_db();
        repository::set_setting(&conn, "test.num", Some("100")).unwrap();
        assert_eq!(read_i64(&conn, "test.num", 0), 100);

        repository::set_setting(&conn, "test.num", Some("-5")).unwrap();
        assert_eq!(read_i64(&conn, "test.num", 0), -5);
    }

    #[test]
    fn test_read_i64_falls_back_on_garbage() {
        let conn = setup_db();
        repository::set_setting(&conn, "test.num", Some("not a number")).unwrap();
        assert_eq!(read_i64(&conn, "test.num", 999), 999);
    }
}
