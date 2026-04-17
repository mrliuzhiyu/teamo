//! 敏感数据检测 — 6 种类型的正则 + 启发式。
//!
//! 优先级（命中即返回，后续跳过）：
//! Token > IdCard > CreditCard > Phone > Email > Password
//!
//! 设计原则（高精度 > 高召回）：
//! - 宁少报不错报 —— 误报会让用户失去信任（"Teamo 把我正常的文字拦了"）
//! - Password 检测靠"整段短串 + 多字符类型 + 高熵"，不做正则模糊匹配，
//!   避免自然语言中带数字/符号的句子被误杀
//! - 其他类型都走严格边界（\b）或长度约束，降低 false positive

use once_cell::sync::Lazy;
use regex::Regex;
use rusqlite::Connection;

use super::cache::FilterSnapshot;
use super::idcard::check_id_card;
use super::luhn::check_luhn;
use super::SensitiveType;

/// 快照版入口（apply_filters 用）—— 从预加载的 FilterSnapshot 读 sens.* 开关，
/// 零 SQLite 查询。
///
/// 每个 detector 跑前先查对应开关（默认全开）。关闭的 detector 跳过，
/// 不会影响其他 detector 的优先级判断。
pub fn detect_with_snapshot(content: &str, snap: &FilterSnapshot) -> Option<SensitiveType> {
    let trimmed = content.trim();
    if trimmed.is_empty() || trimmed.len() > 10_000 {
        return None;
    }

    if snap.sens_token && detect_token(trimmed) {
        return Some(SensitiveType::Token);
    }
    if snap.sens_id_card && detect_id_card(trimmed) {
        return Some(SensitiveType::IdCard);
    }
    if snap.sens_credit_card && detect_credit_card(trimmed) {
        return Some(SensitiveType::CreditCard);
    }
    if snap.sens_phone && detect_phone(trimmed) {
        return Some(SensitiveType::Phone);
    }
    if snap.sens_email && detect_email(trimmed) {
        return Some(SensitiveType::Email);
    }
    if snap.sens_password && detect_password(trimmed) {
        return Some(SensitiveType::Password);
    }
    None
}

/// 从 DB 加载开关后跑 detect —— 保留作为独立入口（tests 用、未来其他调用点用）
pub fn detect(conn: &Connection, content: &str) -> Option<SensitiveType> {
    let snap = super::cache::snapshot(conn);
    detect_with_snapshot(content, &snap)
}

// ── Token（API key / JWT / Slack / GitHub） ──
static TOKEN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?x)
        \b(?:sk|pk|rk)-[A-Za-z0-9_-]{20,}\b                   # OpenAI/Stripe 风格
        | \b(?:gh[pousrb])_[A-Za-z0-9]{30,}\b                 # GitHub PAT
        | \b(?:xox[abpr])-[A-Za-z0-9-]{10,}\b                 # Slack
        | \bBearer\s+[A-Za-z0-9._~+/=_-]{20,}\b               # HTTP Bearer
        | \beyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b  # JWT 三段式
        ",
    )
    .expect("token regex compile")
});

fn detect_token(s: &str) -> bool {
    TOKEN_RE.is_match(s)
}

// ── 中国身份证 ──
static ID_CARD_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b\d{17}[\dXx]\b").expect("id card regex compile")
});

fn detect_id_card(s: &str) -> bool {
    for m in ID_CARD_RE.find_iter(s) {
        if check_id_card(m.as_str()) {
            return true;
        }
    }
    false
}

// ── 银行卡（Luhn 过滤） ──
static CREDIT_CARD_RE: Lazy<Regex> = Lazy::new(|| {
    // 13-19 位数字，允许中间单个空格或连字符分隔。
    // 正则匹配出候选串后再剥离分隔符跑 Luhn。
    Regex::new(r"\b\d(?:[ -]?\d){12,18}\b").expect("credit card regex compile")
});

fn detect_credit_card(s: &str) -> bool {
    for m in CREDIT_CARD_RE.find_iter(s) {
        let digits: String = m.as_str().chars().filter(|c| c.is_ascii_digit()).collect();
        if check_luhn(&digits) {
            return true;
        }
    }
    false
}

// ── 手机号（中国大陆） ──
static PHONE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b1[3-9]\d{9}\b").expect("phone regex compile"));

fn detect_phone(s: &str) -> bool {
    PHONE_RE.is_match(s)
}

// ── Email（RFC 5322 简化版） ──
static EMAIL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b")
        .expect("email regex compile")
});

fn detect_email(s: &str) -> bool {
    EMAIL_RE.is_match(s)
}

// ── 密码（独立短串启发式） ──
//
// 约束全部满足才判定为密码：
// - 不含空白字符（整段是一个 token）
// - 不含 URL scheme（`://` 排除 http(s)/ftp 等）
// - 长度 8-64 个字符
// - 至少 3 种字符类型（小写/大写/数字/符号）
//
// 不再用 Shannon 熵做门槛 —— 经典短密码如 `Aa1@bcdefg` 的熵 ~3.32，
// 低于任何合理的"强密码"阈值，但用户的确把它当密码用。
// 靠"多字符类型 + 无空格"组合已经能过滤绝大多数自然语言；
// URL 另外加 `://` 排除防 `https://foo.com/bar?q=x&y=1` 这类被误判。
fn detect_password(s: &str) -> bool {
    if s.contains(char::is_whitespace) {
        return false;
    }
    if s.contains("://") {
        return false;
    }
    let len = s.chars().count();
    if !(8..=64).contains(&len) {
        return false;
    }
    let mut has_lower = false;
    let mut has_upper = false;
    let mut has_digit = false;
    let mut has_symbol = false;
    for c in s.chars() {
        if c.is_ascii_lowercase() {
            has_lower = true;
        } else if c.is_ascii_uppercase() {
            has_upper = true;
        } else if c.is_ascii_digit() {
            has_digit = true;
        } else if c.is_ascii() && !c.is_alphanumeric() {
            has_symbol = true;
        }
    }
    let class_count = [has_lower, has_upper, has_digit, has_symbol]
        .iter()
        .filter(|&&x| x)
        .count();
    class_count >= 3
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::schema;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        schema::run_migrations(&conn).unwrap();
        super::super::cache::invalidate();
        conn
    }

    /// Helper：默认全开（所有 sens.* 开关为默认 on）状态下跑 detect。
    /// 保持原有 tests 的简洁签名。测"开关关闭"的 case 单独用 detect(&conn, ...) 走全路径。
    fn detect_content(s: &str) -> Option<SensitiveType> {
        let conn = setup_db();
        detect(&conn, s)
    }

    // ── Token ──

    #[test]
    fn test_detect_openai_key() {
        assert_eq!(
            detect_content("sk-abc123def456ghi789jklmnopqrstuvwx"),
            Some(SensitiveType::Token)
        );
    }

    #[test]
    fn test_detect_github_pat() {
        assert_eq!(
            detect_content("ghp_1234567890abcdefghijklmnopqrstuvwxyz"),
            Some(SensitiveType::Token)
        );
    }

    #[test]
    fn test_detect_bearer() {
        assert_eq!(
            detect_content("Bearer eyJhbGciOiJIUzI1NiJ9.abcdefghij"),
            Some(SensitiveType::Token)
        );
    }

    #[test]
    fn test_detect_jwt() {
        assert_eq!(
            detect_content("eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"),
            Some(SensitiveType::Token)
        );
    }

    // ── 银行卡 ──

    #[test]
    fn test_detect_visa() {
        assert_eq!(detect_content("4111111111111111"), Some(SensitiveType::CreditCard));
    }

    #[test]
    fn test_detect_visa_with_spaces() {
        assert_eq!(
            detect_content("4111 1111 1111 1111"),
            Some(SensitiveType::CreditCard)
        );
    }

    #[test]
    fn test_detect_visa_with_dashes() {
        assert_eq!(
            detect_content("4111-1111-1111-1111"),
            Some(SensitiveType::CreditCard)
        );
    }

    #[test]
    fn test_not_credit_card_luhn_fail() {
        // 16 位数字但 Luhn 失败 → 不判为银行卡
        let result = detect_content("1234567890123456");
        assert_ne!(result, Some(SensitiveType::CreditCard));
    }

    // ── 身份证 ──

    #[test]
    fn test_detect_id_card() {
        assert_eq!(detect_content("11010519491231002X"), Some(SensitiveType::IdCard));
    }

    #[test]
    fn test_not_id_card_bad_checksum() {
        let result = detect_content("110105194912310021");
        assert_ne!(result, Some(SensitiveType::IdCard));
    }

    // ── 手机 ──

    #[test]
    fn test_detect_phone() {
        assert_eq!(detect_content("13800138000"), Some(SensitiveType::Phone));
    }

    #[test]
    fn test_not_phone_landline() {
        // 座机号码（010-12345678）不应该当手机
        let result = detect_content("010-12345678");
        assert_ne!(result, Some(SensitiveType::Phone));
    }

    // ── Email ──

    #[test]
    fn test_detect_email() {
        assert_eq!(detect_content("user@example.com"), Some(SensitiveType::Email));
    }

    #[test]
    fn test_detect_email_in_sentence() {
        assert_eq!(
            detect_content("Contact me at user@example.com for details"),
            Some(SensitiveType::Email)
        );
    }

    // ── 密码 ──

    #[test]
    fn test_detect_password_strong() {
        assert_eq!(detect_content("Aa1@bcdefg"), Some(SensitiveType::Password));
    }

    #[test]
    fn test_detect_password_mixed() {
        assert_eq!(detect_content("MyP@ss123!"), Some(SensitiveType::Password));
    }

    #[test]
    fn test_not_password_weak_single_class() {
        // 纯小写 → 类别 1，不满足
        assert_eq!(detect_content("abcdefghij"), None);
    }

    #[test]
    fn test_not_password_common_weak() {
        // password 纯小写 → 类别 1
        assert_eq!(detect_content("password"), None);
        // password123 小写+数字 → 类别 2，仍不满足 >= 3
        assert_eq!(detect_content("password123"), None);
    }

    #[test]
    fn test_not_password_has_whitespace() {
        // 含空格不算密码（自然语言）
        assert_eq!(detect_content("hello World 123!"), None);
    }

    #[test]
    fn test_not_password_too_short() {
        assert_eq!(detect_content("Aa1@"), None);
    }

    #[test]
    fn test_not_password_natural_english() {
        // 纯自然语言句子 — 有空格直接过滤
        assert_eq!(detect_content("hello world this is a test"), None);
    }

    #[test]
    fn test_not_password_url() {
        // URL 含 3 种字符类型（小写+数字+符号）长度也在 8-64，但 `://` 应该排除
        assert_eq!(detect_content("https://example.com/path?q=123&id=456"), None);
    }

    // ── 优先级 ──

    #[test]
    fn test_token_before_password() {
        // sk-xxx 也符合 password 规则（混合字符类型+长），但应判为 Token
        assert_eq!(
            detect_content("sk-abc123def456ghi789jklmnopqrstuvwx"),
            Some(SensitiveType::Token)
        );
    }

    // ── 空 / 超长 ──

    #[test]
    fn test_empty_content() {
        assert_eq!(detect_content(""), None);
        assert_eq!(detect_content("   \n\t  "), None);
    }

    #[test]
    fn test_oversized_content_skipped() {
        // 超过 10k 字符直接跳过检测（即便里面有 token 也不拦）
        let mut big = "x".repeat(10_001);
        big.push_str(" sk-abcdefghijklmnopqrstuvwxyz");
        assert_eq!(detect_content(&big), None);
    }
}
