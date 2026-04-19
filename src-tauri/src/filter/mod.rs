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

pub mod cache;
pub mod entropy;
pub mod idcard;
pub mod luhn;
pub mod sensitive;
pub mod url_match;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[cfg(test)]
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

/// 闸门结果。写入 clipboard_local 的 state / blocked_reason / sensitive_type / matched_domain_rule 列。
#[derive(Debug, Clone, Serialize, Default)]
pub struct FilterDecision {
    /// `state` 列值："captured"（可上云）/ "local_only"（闸门拦截，永不上云）
    pub state: String,
    /// 人可读的拦截原因（比如 "sensitive:password"）。captured 时为 None。
    pub blocked_reason: Option<String>,
    /// 具体的敏感类型（password/token/...），未命中为 None。
    pub sensitive_type: Option<String>,
    /// 如果 URL 命中了 domain_rules 的 `parse_as_content` / `skip_parse` 类型，
    /// 格式为 "rule_type:pattern"（比如 "parse_as_content:v.douyin.com/*"）。
    /// skip_upload 不写这里（状态已经是 local_only + blocked_reason）。
    /// M3 云端 parse_worker 读此字段决定是 link_cards 解析还是跳过。
    pub matched_domain_rule: Option<String>,
}

impl FilterDecision {
    pub fn captured() -> Self {
        Self {
            state: "captured".to_string(),
            ..Default::default()
        }
    }

    /// captured 但记录了 URL 命中的 domain_rule（parse_as_content / skip_parse）
    pub fn captured_with_domain_rule(rule_type: &str, pattern: &str) -> Self {
        Self {
            state: "captured".to_string(),
            matched_domain_rule: Some(format!("{rule_type}:{pattern}")),
            ..Default::default()
        }
    }

    pub fn blocked_sensitive(kind: SensitiveType) -> Self {
        Self {
            state: "local_only".to_string(),
            blocked_reason: Some(format!("sensitive:{}", kind.as_str())),
            sensitive_type: Some(kind.as_str().to_string()),
            matched_domain_rule: None,
        }
    }

    pub fn blocked_short_text() -> Self {
        Self {
            state: "local_only".to_string(),
            blocked_reason: Some("short_text".to_string()),
            ..Default::default()
        }
    }

    pub fn blocked_app(app: &str) -> Self {
        Self {
            state: "local_only".to_string(),
            blocked_reason: Some(format!("app_blacklist:{app}")),
            ..Default::default()
        }
    }

    pub fn blocked_domain(pattern: &str) -> Self {
        Self {
            state: "local_only".to_string(),
            blocked_reason: Some(format!("domain_skip_upload:{pattern}")),
            matched_domain_rule: Some(format!("skip_upload:{pattern}")),
            ..Default::default()
        }
    }
}

/// App 黑白名单独立查询 —— 供 text 分支（apply_filters）和 image 分支（clipboard capture）共用。
///
/// 返回 `Some(FilterDecision)` 表示命中规则：
/// - 白名单 → `FilterDecision::captured()`
/// - 黑名单 → `FilterDecision::blocked_app(app)`
/// - elevated 哨兵 + 用户已配置任何 blacklist → 保守拦截（视同黑名单）
///
/// 返回 `None` 表示无规则命中，调用方继续后续 filter 层（sensitive / domain / short_text）。
///
/// 为什么抽出来：图片分支不跑 sensitive/domain/short_text（图片无文本内容），
/// 但 App 黑白名单对图片也生效（1Password 截屏等）。两分支共用这一个查询点，
/// 避免各自独立重写 `app_rule_match` 调用 + FilterDecision 构造。
pub fn check_app_rules(conn: &Connection, source_app: Option<&str>) -> Option<FilterDecision> {
    let app = source_app?;

    // 哨兵：elevated 进程 source_app=None 会让整块 app_rules 跳过，
    // 用户加的 "KeePass.exe 黑名单" 对 KeePass-as-admin 失效（outside voice Issue 1）。
    //
    // 保守策略：用户**配置过任何 app_rule**（不论 blacklist 还是 whitelist）即
    // 表达了 app-level 过滤意愿 —— 白名单模式下"只信任白名单 App"隐含"未在白名单
    // 就不记"，elevated 进程自然不在白名单里，也应视同拒绝。
    // 旧版本只查 blacklist 计数 → 纯白名单用户（有 whitelist 无 blacklist）elevated
    // 进程照样进入后续 sensitive/URL 检测流程，不符合 whitelist 语义。
    if app == crate::window::platform::ELEVATED_APP_SENTINEL {
        let has_any_rule: i64 = conn
            .query_row("SELECT COUNT(*) FROM app_rules", [], |row| row.get(0))
            .unwrap_or(0);
        if has_any_rule > 0 {
            return Some(FilterDecision::blocked_app(app));
        }
        return None;
    }

    match crate::storage::repository::app_rule_match(conn, app).ok().flatten()?.as_str() {
        "whitelist" => Some(FilterDecision::captured()),
        "blacklist" => Some(FilterDecision::blocked_app(app)),
        _ => None,
    }
}

/// 对文本内容应用所有过滤器。
///
/// 优先级（架构 L1 的子集，按"早出"原则排序）：
/// 1. **App 白名单**命中 → 直接 captured，跳过所有后续（信任用户对白名单 App 的选择）
/// 2. **App 黑名单**命中 → local_only:app_blacklist
/// 3. **URL domain_rules** skip_upload 命中 → local_only:domain_skip_upload
///    （skip_parse / parse_as_content 仅云端用，Phase 1 不影响 state）
/// 4. 短文本（非 URL）→ local_only:short_text
/// 5. 敏感检测 6 类 → local_only:sensitive:*
/// 6. 其余 → captured
pub fn apply_filters(
    conn: &Connection,
    content: &str,
    source_app: Option<&str>,
) -> FilterDecision {
    // 从缓存取 filter 快照 —— 避免每次 capture 9 次 SQLite 查询（见 cache.rs 设计说明）
    let snap = cache::snapshot(conn);

    // L1.2 App 黑白名单（共用 check_app_rules，与 clipboard 图片分支对称。
    // 这层不走快照：app_rules 查询是按 key 精确查，单次 SELECT 微秒级不值得缓存）
    if let Some(decision) = check_app_rules(conn, source_app) {
        return decision;
    }

    // L1.4 URL domain_rules（content 是 URL 时才跑，降低无谓开销）
    let mut first_non_block_match: Option<(String, String)> = None; // (rule_type, pattern)
    if let Some(parsed_url) = url_match::extract_url(content) {
        let haystack = url_match::haystack(&parsed_url);
        // 规则已按 priority DESC 排序，高优命中先
        for rule in &snap.domain_rules {
            if url_match::pattern_matches(&rule.pattern, &haystack) {
                if rule.rule_type == "skip_upload" {
                    return FilterDecision::blocked_domain(&rule.pattern);
                }
                if first_non_block_match.is_none() {
                    first_non_block_match =
                        Some((rule.rule_type.clone(), rule.pattern.clone()));
                }
            }
        }
    }

    // L1.5 短文本：长度 < min_text_len 且不是 URL → local_only
    if snap.min_text_len > 0 {
        let trimmed = content.trim();
        let char_count = trimmed.chars().count() as i64;
        if char_count < snap.min_text_len && !looks_like_url(trimmed) {
            return FilterDecision::blocked_short_text();
        }
    }

    // L1.3 敏感检测（用快照里的开关，避免 6 次 get_setting）
    if let Some(kind) = sensitive::detect_with_snapshot(content, &snap) {
        return FilterDecision::blocked_sensitive(kind);
    }

    // 默认 captured —— 若前面记录了 parse_as_content / skip_parse 命中，一起写进 decision
    if let Some((rule_type, pattern)) = first_non_block_match {
        FilterDecision::captured_with_domain_rule(&rule_type, &pattern)
    } else {
        FilterDecision::captured()
    }
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
        // filter cache 是全局单例，跨测试会污染 —— 每次 setup 都先 invalidate
        // 确保 apply_filters/detect 读到本 test 自己的 settings/domain_rules
        cache::invalidate();
        conn
    }

    /// 测试里改了 settings 之后调这个确保下一次 apply_filters/detect 读新值
    fn reload_filter_cache() {
        cache::invalidate();
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
        reload_filter_cache();
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
        reload_filter_cache();
        let d = apply_filters(&conn, "https://a.co", None);
        assert_eq!(d.state, "captured");
    }

    #[test]
    fn test_sens_switch_off_disables_detector() {
        let conn = setup_db();
        crate::storage::repository::set_setting(&conn, settings_keys::SENS_TOKEN, Some("0"))
            .unwrap();
        reload_filter_cache();
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
        reload_filter_cache();
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

    // ── URL domain_rules ──

    #[test]
    fn test_domain_skip_upload_bank() {
        let conn = setup_db();
        crate::storage::repository::bulk_insert_domain_rules(
            &conn,
            &[("*.cmbchina.com/*".to_string(), "skip_upload".to_string(), 200)],
            "builtin",
        )
        .unwrap();
        reload_filter_cache();
        let d = apply_filters(&conn, "https://personal.cmbchina.com/login", None);
        assert_eq!(d.state, "local_only");
        assert!(d
            .blocked_reason
            .as_deref()
            .unwrap_or("")
            .starts_with("domain_skip_upload:"));
    }

    #[test]
    fn test_domain_skip_upload_login_wildcard() {
        let conn = setup_db();
        crate::storage::repository::bulk_insert_domain_rules(
            &conn,
            &[("*/login".to_string(), "skip_upload".to_string(), 200)],
            "builtin",
        )
        .unwrap();
        reload_filter_cache();
        let d = apply_filters(&conn, "https://example.com/login", None);
        assert_eq!(d.state, "local_only");
    }

    #[test]
    fn test_domain_skip_upload_localhost() {
        let conn = setup_db();
        crate::storage::repository::bulk_insert_domain_rules(
            &conn,
            &[("localhost*".to_string(), "skip_upload".to_string(), 200)],
            "builtin",
        )
        .unwrap();
        reload_filter_cache();
        let d = apply_filters(&conn, "http://localhost:3000/admin", None);
        assert_eq!(d.state, "local_only");
    }

    #[test]
    fn test_domain_parse_as_content_does_not_block() {
        let conn = setup_db();
        crate::storage::repository::bulk_insert_domain_rules(
            &conn,
            &[("v.douyin.com/*".to_string(), "parse_as_content".to_string(), 100)],
            "builtin",
        )
        .unwrap();
        // parse_as_content 命中不影响 state，仍 captured
        let d = apply_filters(&conn, "https://v.douyin.com/abc", None);
        assert_eq!(d.state, "captured");
    }

    #[test]
    fn test_non_url_content_skips_domain_rules() {
        let conn = setup_db();
        crate::storage::repository::bulk_insert_domain_rules(
            &conn,
            &[("*/login".to_string(), "skip_upload".to_string(), 200)],
            "builtin",
        )
        .unwrap();
        // 普通文本包含 "login" 字样但不是 URL → 不命中 domain_rules
        let d = apply_filters(&conn, "my login is Alice", None);
        // 注意 content 可能被敏感/短文本命中，我们只断言"不是 domain 原因"
        assert!(d
            .blocked_reason
            .as_deref()
            .unwrap_or("captured")
            .find("domain_")
            .is_none());
    }

    #[test]
    fn test_app_whitelist_overrides_domain_skip() {
        let conn = setup_db();
        crate::storage::repository::add_app_rule(&conn, "MyTrusted.exe", "whitelist").unwrap();
        crate::storage::repository::bulk_insert_domain_rules(
            &conn,
            &[("*/login".to_string(), "skip_upload".to_string(), 200)],
            "builtin",
        )
        .unwrap();
        // 白名单应用 + domain 命中 skip_upload → 白名单优先级更高
        let d = apply_filters(
            &conn,
            "https://example.com/login",
            Some("MyTrusted.exe"),
        );
        assert_eq!(d.state, "captured");
    }
}
