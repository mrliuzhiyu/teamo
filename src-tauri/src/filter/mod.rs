//! 端侧闸门（filter-engine）— 决定剪切板内容能否进入 pending_upload 状态。
//!
//! 当前覆盖（架构 §3.3 的子集）：
//! - L1.3 敏感检测（6 类，开关可配）
//! - L1.5 短文本阈值（filter.min_text_len，URL 豁免）
//!
//! Phase B 留：
//! - L1.2 App 黑白名单（依赖 Windows/macOS 平台 API 抓 source_app）
//! - L1.4 URL 域名规则（domain_rules 表 CRUD + YAML seed）
//!
//! 规则即数据：所有开关/阈值读自 SQLite `settings` 表（走 `settings_keys` 常量）。
//! 单次调用成本：5-6 次 SQLite `get_setting` + O(n) 正则扫描。capture loop 每 500ms
//! 最多跑一次，不阻塞其他调用。

pub mod entropy;
pub mod idcard;
pub mod luhn;
pub mod sensitive;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::settings_keys;

/// 敏感数据类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitiveType {
    Password,
    Token,
    CreditCard,
    IdCard,
    Phone,
    Email,
}

impl SensitiveType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SensitiveType::Password => "password",
            SensitiveType::Token => "token",
            SensitiveType::CreditCard => "credit_card",
            SensitiveType::IdCard => "id_card",
            SensitiveType::Phone => "phone",
            SensitiveType::Email => "email",
        }
    }
}

/// 闸门结果。写入 clipboard_local 的 state/blocked_reason/sensitive_type 列。
#[derive(Debug, Clone, Serialize)]
pub struct FilterDecision {
    /// `state` 列值："captured"（可上云）/ "local_only"（闸门拦截，永不上云）
    pub state: String,
    /// 人可读的拦截原因（比如 "sensitive:password"）。captured 时为 None。
    pub blocked_reason: Option<String>,
    /// 具体的敏感类型（password/token/...），未命中为 None。
    pub sensitive_type: Option<String>,
}

impl FilterDecision {
    pub fn captured() -> Self {
        Self {
            state: "captured".to_string(),
            blocked_reason: None,
            sensitive_type: None,
        }
    }

    pub fn blocked_sensitive(kind: SensitiveType) -> Self {
        Self {
            state: "local_only".to_string(),
            blocked_reason: Some(format!("sensitive:{}", kind.as_str())),
            sensitive_type: Some(kind.as_str().to_string()),
        }
    }

    pub fn blocked_short_text() -> Self {
        Self {
            state: "local_only".to_string(),
            blocked_reason: Some("short_text".to_string()),
            sensitive_type: None,
        }
    }
}

/// 对文本内容应用所有过滤器。
///
/// 顺序（架构 L1 优先级）：
/// 1. 短文本（非 URL）→ local_only:short_text
/// 2. 敏感检测 6 类 → local_only:sensitive:*
/// 3. 其余 → captured
pub fn apply_filters(conn: &Connection, content: &str) -> FilterDecision {
    // L1.5 短文本：长度 < min_text_len 且不是 URL → local_only
    let min_len = settings_keys::read_i64(
        conn,
        settings_keys::FILTER_MIN_TEXT_LEN,
        settings_keys::FILTER_MIN_TEXT_LEN_DEFAULT
            .parse()
            .unwrap_or(0),
    );
    if min_len > 0 {
        let trimmed = content.trim();
        let char_count = trimmed.chars().count() as i64;
        if char_count < min_len && !looks_like_url(trimmed) {
            return FilterDecision::blocked_short_text();
        }
    }

    // L1.3 敏感检测
    if let Some(kind) = sensitive::detect(conn, content) {
        return FilterDecision::blocked_sensitive(kind);
    }

    FilterDecision::captured()
}

fn looks_like_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://") || s.starts_with("ftp://")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::schema;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        schema::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn test_apply_filters_clean_text() {
        let conn = setup_db();
        let d = apply_filters(&conn, "just some random notes about my day");
        assert_eq!(d.state, "captured");
        assert!(d.blocked_reason.is_none());
        assert!(d.sensitive_type.is_none());
    }

    #[test]
    fn test_apply_filters_sensitive_token() {
        let conn = setup_db();
        let d = apply_filters(&conn, "sk-abc123def456ghi789jklmnopqrstuvwx");
        assert_eq!(d.state, "local_only");
        assert_eq!(d.blocked_reason.as_deref(), Some("sensitive:token"));
        assert_eq!(d.sensitive_type.as_deref(), Some("token"));
    }

    #[test]
    fn test_apply_filters_sensitive_credit_card() {
        let conn = setup_db();
        let d = apply_filters(&conn, "4111 1111 1111 1111");
        assert_eq!(d.state, "local_only");
        assert_eq!(d.sensitive_type.as_deref(), Some("credit_card"));
    }

    #[test]
    fn test_short_text_filter_default_off() {
        // FILTER_MIN_TEXT_LEN 默认为 "0"（不过滤）
        let conn = setup_db();
        let d = apply_filters(&conn, "hi");
        assert_eq!(d.state, "captured");
    }

    #[test]
    fn test_short_text_filter_when_enabled() {
        let conn = setup_db();
        crate::storage::repository::set_setting(
            &conn,
            settings_keys::FILTER_MIN_TEXT_LEN,
            Some("8"),
        )
        .unwrap();
        // 短于 8 字且非 URL → blocked
        let d = apply_filters(&conn, "hi bro");
        assert_eq!(d.state, "local_only");
        assert_eq!(d.blocked_reason.as_deref(), Some("short_text"));
    }

    #[test]
    fn test_short_text_url_exempt() {
        let conn = setup_db();
        crate::storage::repository::set_setting(
            &conn,
            settings_keys::FILTER_MIN_TEXT_LEN,
            Some("50"),
        )
        .unwrap();
        // 短 URL 不应被短文本拦
        let d = apply_filters(&conn, "https://a.co");
        assert_eq!(d.state, "captured");
    }

    #[test]
    fn test_sens_switch_off_disables_detector() {
        let conn = setup_db();
        // 关掉 token 检测
        crate::storage::repository::set_setting(&conn, settings_keys::SENS_TOKEN, Some("0"))
            .unwrap();
        let d = apply_filters(&conn, "sk-abc123def456ghi789jklmnopqrstuvwx");
        // token 开关关了 → 不走 token 分支；但它仍可能命中 password 检测（类数 + 长度满足）
        // 断言：即便命中其他 detector，也不会是 Token
        assert_ne!(d.sensitive_type.as_deref(), Some("token"));
    }

    #[test]
    fn test_sens_all_off_captures_normally() {
        let conn = setup_db();
        for key in [
            settings_keys::SENS_PASSWORD,
            settings_keys::SENS_TOKEN,
            settings_keys::SENS_CREDIT_CARD,
            settings_keys::SENS_ID_CARD,
            settings_keys::SENS_PHONE,
            settings_keys::SENS_EMAIL,
        ] {
            crate::storage::repository::set_setting(&conn, key, Some("0")).unwrap();
        }
        // 所有敏感检测关闭，连明显的邮箱也会按 captured
        let d = apply_filters(&conn, "user@example.com");
        assert_eq!(d.state, "captured");
    }
}
