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

    pub fn blocked_app(app: &str) -> Self {
        Self {
            state: "local_only".to_string(),
            blocked_reason: Some(format!("app_blacklist:{app}")),
            sensitive_type: None,
        }
    }
}

/// 对文本内容应用所有过滤器。
///
/// 优先级（架构 L1）：
/// 1. **App 白名单**命中 → 直接 captured，跳过所有后续（信任用户对白名单 App 的选择）
/// 2. **App 黑名单**命中 → local_only:app_blacklist（例如 1Password/银行客户端）
/// 3. 短文本（非 URL）→ local_only:short_text
/// 4. 敏感检测 6 类 → local_only:sensitive:*
/// 5. 其余 → captured
pub fn apply_filters(
    conn: &Connection,
    content: &str,
    source_app: Option<&str>,
) -> FilterDecision {
    // L1.2 App 黑白名单（source_app 存在且命中规则时生效）
    if let Some(app) = source_app {
        if let Ok(Some(rule)) = crate::storage::repository::app_rule_match(conn, app) {
            match rule.as_str() {
                "whitelist" => return FilterDecision::captured(), // 白名单直接放行
                "blacklist" => return FilterDecision::blocked_app(app),
                _ => {}
            }
        }
    }

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
        let d = apply_filters(&conn, "just some random notes about my day", None);
        assert_eq!(d.state, "captured");
        assert!(d.blocked_reason.is_none());
        assert!(d.sensitive_type.is_none());
    }

    #[test]
    fn test_apply_filters_sensitive_token() {
        let conn = setup_db();
        let d = apply_filters(&conn, "sk-abc123def456ghi789jklmnopqrstuvwx", None);
        assert_eq!(d.state, "local_only");
        assert_eq!(d.blocked_reason.as_deref(), Some("sensitive:token"));
        assert_eq!(d.sensitive_type.as_deref(), Some("token"));
    }

    #[test]
    fn test_apply_filters_sensitive_credit_card() {
        let conn = setup_db();
        let d = apply_filters(&conn, "4111 1111 1111 1111", None);
        assert_eq!(d.state, "local_only");
        assert_eq!(d.sensitive_type.as_deref(), Some("credit_card"));
    }

    #[test]
    fn test_short_text_filter_default_off() {
        let conn = setup_db();
        let d = apply_filters(&conn, "hi", None);
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
        let d = apply_filters(&conn, "hi bro", None);
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
        let d = apply_filters(&conn, "https://a.co", None);
        assert_eq!(d.state, "captured");
    }

    #[test]
    fn test_sens_switch_off_disables_detector() {
        let conn = setup_db();
        crate::storage::repository::set_setting(&conn, settings_keys::SENS_TOKEN, Some("0"))
            .unwrap();
        let d = apply_filters(&conn, "sk-abc123def456ghi789jklmnopqrstuvwx", None);
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
        let d = apply_filters(&conn, "user@example.com", None);
        assert_eq!(d.state, "captured");
    }

    // ── App 黑白名单 ──

    #[test]
    fn test_app_blacklist_blocks_even_innocent_content() {
        let conn = setup_db();
        crate::storage::repository::add_app_rule(&conn, "1Password.exe", "blacklist").unwrap();
        // 正常内容在黑名单 App 下也 local_only
        let d = apply_filters(&conn, "some harmless notes", Some("1Password.exe"));
        assert_eq!(d.state, "local_only");
        assert!(d
            .blocked_reason
            .as_deref()
            .unwrap_or("")
            .starts_with("app_blacklist:"));
    }

    #[test]
    fn test_app_blacklist_case_insensitive() {
        let conn = setup_db();
        crate::storage::repository::add_app_rule(&conn, "chrome.exe", "blacklist").unwrap();
        // 规则存小写，source_app 传大写 → 仍命中（大小写不敏感）
        let d = apply_filters(&conn, "some content", Some("Chrome.exe"));
        assert_eq!(d.state, "local_only");
    }

    #[test]
    fn test_app_whitelist_bypasses_sensitive() {
        let conn = setup_db();
        // 白名单里的 App 即便复制了 token 也放行（用户明知选择）
        crate::storage::repository::add_app_rule(&conn, "TrustedApp.exe", "whitelist").unwrap();
        let d = apply_filters(
            &conn,
            "sk-abc123def456ghi789jklmnopqrstuvwx",
            Some("TrustedApp.exe"),
        );
        assert_eq!(d.state, "captured");
        assert!(d.blocked_reason.is_none());
    }

    #[test]
    fn test_no_app_rules_falls_through_to_sensitive() {
        let conn = setup_db();
        // 无 app_rules 命中 → 按正常流程（敏感检测仍拦）
        let d = apply_filters(
            &conn,
            "sk-abc123def456ghi789jklmnopqrstuvwx",
            Some("SomeApp.exe"),
        );
        assert_eq!(d.state, "local_only");
        assert_eq!(d.sensitive_type.as_deref(), Some("token"));
    }

    #[test]
    fn test_source_app_none_skips_app_rules() {
        let conn = setup_db();
        crate::storage::repository::add_app_rule(&conn, "chrome.exe", "blacklist").unwrap();
        // source_app=None（非 Windows / 抓取失败）→ 跳过 app_rules 检查
        let d = apply_filters(&conn, "some content", None);
        assert_eq!(d.state, "captured");
    }
}
