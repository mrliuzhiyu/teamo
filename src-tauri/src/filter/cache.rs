//! Filter 内存缓存 —— 消除 apply_filters 每次 capture 的 9 次 DB 查询开销。
//!
//! 缓存的内容：
//! - 6 个 sens.* 开关 + filter.min_text_len（共 7 个 settings key 的值）
//! - 所有 domain_rules（70+ 条，按 priority DESC 排好序）
//!
//! 缓存不存 app_rules：app_rule_match 查询是按 key 精确查询（LOWER 比较），
//! 命中率常规情况 0（用户只加几个 App），单次 SELECT < 0.1ms，缓存不划算。
//!
//! **失效策略**：手动 invalidate —— 任何修改 settings / domain_rules 的 Tauri command
//! 执行完调 `FilterCache::global().invalidate()`。下次 apply_filters 自动 reload。
//! 不做 time-based TTL（避免"改完设置立即 capture 但缓存还是旧值"的尴尬）。
//!
//! **线程安全**：RwLock，apply_filters 走读锁多并发，invalidate 写锁独占（极少）。

use once_cell::sync::Lazy;
use std::sync::RwLock;

use crate::settings_keys;
use crate::storage::repository::{self, DomainRule};

/// 缓存的 filter 配置快照
#[derive(Debug, Clone)]
pub struct FilterSnapshot {
    pub min_text_len: i64,
    pub sens_password: bool,
    pub sens_token: bool,
    pub sens_credit_card: bool,
    pub sens_id_card: bool,
    pub sens_phone: bool,
    pub sens_email: bool,
    pub domain_rules: Vec<DomainRule>,
}

impl FilterSnapshot {
    /// 默认值（缓存未加载时用）—— 行为等同"全部开 + 阈值 0 + 无 domain 规则"，
    /// 宁错杀不漏杀
    fn defaults() -> Self {
        Self {
            min_text_len: 0,
            sens_password: true,
            sens_token: true,
            sens_credit_card: true,
            sens_id_card: true,
            sens_phone: true,
            sens_email: true,
            domain_rules: Vec::new(),
        }
    }

    /// 从 DB fresh load 一次
    fn load(conn: &rusqlite::Connection) -> Self {
        Self {
            min_text_len: settings_keys::read_i64(
                conn,
                settings_keys::FILTER_MIN_TEXT_LEN,
                settings_keys::FILTER_MIN_TEXT_LEN_DEFAULT
                    .parse()
                    .unwrap_or(0),
            ),
            sens_password: settings_keys::read_bool_flag(conn, settings_keys::SENS_PASSWORD, true),
            sens_token: settings_keys::read_bool_flag(conn, settings_keys::SENS_TOKEN, true),
            sens_credit_card: settings_keys::read_bool_flag(
                conn,
                settings_keys::SENS_CREDIT_CARD,
                true,
            ),
            sens_id_card: settings_keys::read_bool_flag(conn, settings_keys::SENS_ID_CARD, true),
            sens_phone: settings_keys::read_bool_flag(conn, settings_keys::SENS_PHONE, true),
            sens_email: settings_keys::read_bool_flag(conn, settings_keys::SENS_EMAIL, true),
            domain_rules: repository::list_domain_rules(conn).unwrap_or_default(),
        }
    }
}

/// 全局 filter cache 单例
static FILTER_CACHE: Lazy<RwLock<Option<FilterSnapshot>>> = Lazy::new(|| RwLock::new(None));

/// 取快照 —— 如果缓存为空（首次调用或 invalidate 之后），fresh load 一次。
/// 返回值是 `FilterSnapshot` 克隆（内部 domain_rules Vec 70 条 ~KB，克隆成本 < 10 μs）
pub fn snapshot(conn: &rusqlite::Connection) -> FilterSnapshot {
    // 快路径：读锁，cache hit
    if let Ok(guard) = FILTER_CACHE.read() {
        if let Some(snap) = guard.as_ref() {
            return snap.clone();
        }
    }
    // 慢路径：写锁，cache miss → load
    if let Ok(mut guard) = FILTER_CACHE.write() {
        if guard.is_none() {
            *guard = Some(FilterSnapshot::load(conn));
        }
        // 这里 Option 要么是刚刚填的要么是别的线程先填的，都可以 clone 返
        if let Some(snap) = guard.as_ref() {
            return snap.clone();
        }
    }
    // 理论不可达（锁 poison 才会到）—— fallback 到默认
    FilterSnapshot::defaults()
}

/// 失效缓存。下次 snapshot 时 fresh load。
/// 由修改 settings / domain_rules 的 Tauri command 在操作成功后调用。
pub fn invalidate() {
    if let Ok(mut guard) = FILTER_CACHE.write() {
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{repository, schema};

    fn setup() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        schema::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn test_snapshot_loads_defaults_on_empty_settings() {
        let conn = setup();
        // 这里全局 cache 可能被其他测试污染 —— 强制 invalidate
        invalidate();
        let snap = snapshot(&conn);
        assert_eq!(snap.min_text_len, 0);
        assert!(snap.sens_password); // 所有 sens.* 默认 true
        assert!(snap.sens_token);
        assert!(snap.domain_rules.is_empty());
    }

    #[test]
    fn test_invalidate_triggers_reload() {
        let conn = setup();
        invalidate();
        let _snap1 = snapshot(&conn);

        // 改 setting
        repository::set_setting(&conn, settings_keys::SENS_PASSWORD, Some("0")).unwrap();

        // 没 invalidate → 可能读到旧值（测试不断言值，仅断言 invalidate 后拿到新值）
        invalidate();
        let snap2 = snapshot(&conn);
        assert!(!snap2.sens_password, "invalidate 后应该读到新值 sens_password=false");
    }
}
