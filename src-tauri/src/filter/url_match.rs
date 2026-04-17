//! URL 提取 + domain_rules pattern 匹配工具。
//!
//! 职责：
//! 1. 从剪切板 content 里提取出 URL（如果内容就是 URL）
//! 2. 把 YAML pattern（`*.cmbchina.com/*` / `domain.com/path/*` / `*/login`）
//!    翻译成 regex 然后做匹配
//!
//! 匹配粒度：`<host><path>` 拼接后整体匹配 pattern（带 `*` 通配）。
//! 例如 URL `https://v.douyin.com/abc?x=1`
//!   - host = "v.douyin.com"
//!   - path = "/abc"
//!   - haystack = "v.douyin.com/abc"
//!   - pattern "v.douyin.com/*" → 匹配

use once_cell::sync::Lazy;
use regex::Regex;
use url::Url;

/// 提取 content 里的 URL。当前策略：content trim 后就是完整 URL 才认；
/// 不扫描"文本中嵌入的 URL"（避免对普通段落误杀）。
pub fn extract_url(content: &str) -> Option<Url> {
    let trimmed = content.trim();
    // 简单启发式：必须以 scheme 开头（http/https/ftp）才尝试解析，
    // 避免把"有空格有中文的自然语言"误当 URL
    if !(trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("ftp://"))
    {
        return None;
    }
    Url::parse(trimmed).ok()
}

/// 从 `url::Url` 取出匹配用的 haystack：`host + path`，忽略 scheme/query/fragment。
/// 例如 `https://v.douyin.com/abc?x=1` → `"v.douyin.com/abc"`
pub fn haystack(u: &Url) -> String {
    let host = u.host_str().unwrap_or("");
    let path = u.path();
    format!("{host}{path}")
}

/// pattern 匹配：把 YAML 里的 `*.cmbchina.com/*` 之类翻译成 regex。
///
/// - `*` 通配任意字符（不跨 `/` 的严格要求我们不做，YAML 规则里多数 `*/path` 要求跨段）
/// - 其他字符按字面，regex 特殊字符转义
pub fn pattern_matches(pattern: &str, haystack_str: &str) -> bool {
    let regex_source = pattern_to_regex_source(pattern);
    // Haystack 永远形如 "host/path"（至少一个 "/"）。
    // Pattern 如果不以 * 结尾，允许末尾有可选斜杠——让 `github.com`
    // 也能匹配 `github.com/`（host-only 视为 host + 首页）。
    let anchored = if pattern.ends_with('*') {
        format!("^{regex_source}$")
    } else {
        format!("^{regex_source}/?$")
    };
    match compile_cached(&anchored) {
        Some(r) => r.is_match(haystack_str),
        None => false,
    }
}

fn pattern_to_regex_source(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len() * 2);
    for c in pattern.chars() {
        match c {
            '*' => out.push_str(".*"),
            // regex 元字符全部转义
            '.' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\' | '^' | '$' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// 简单 regex 缓存：YAML pattern 数量有限（~70 条），每次 capture 转一次
/// regex 构造浪费。用 Mutex<HashMap> 按源字符串缓存编译结果。
static REGEX_CACHE: Lazy<std::sync::Mutex<std::collections::HashMap<String, Regex>>> =
    Lazy::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

fn compile_cached(source: &str) -> Option<Regex> {
    if let Ok(mut cache) = REGEX_CACHE.lock() {
        if let Some(r) = cache.get(source) {
            return Some(r.clone());
        }
        match Regex::new(source) {
            Ok(r) => {
                cache.insert(source.to_string(), r.clone());
                Some(r)
            }
            Err(e) => {
                tracing::warn!("invalid pattern regex '{source}': {e}");
                None
            }
        }
    } else {
        Regex::new(source).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_url_plain() {
        let u = extract_url("https://v.douyin.com/abc").unwrap();
        assert_eq!(u.host_str(), Some("v.douyin.com"));
    }

    #[test]
    fn test_extract_url_with_whitespace() {
        let u = extract_url("  https://v.douyin.com/abc  \n").unwrap();
        assert_eq!(u.host_str(), Some("v.douyin.com"));
    }

    #[test]
    fn test_extract_non_url() {
        assert!(extract_url("hello world").is_none());
        assert!(extract_url("not a url").is_none());
        // 仅 URL 开头 http 才认，防止把"一段包含 URL 的段落"误识别
        assert!(extract_url("看这个链接 https://example.com").is_none());
    }

    #[test]
    fn test_haystack_basic() {
        let u = Url::parse("https://v.douyin.com/abc?x=1").unwrap();
        assert_eq!(haystack(&u), "v.douyin.com/abc");
    }

    #[test]
    fn test_haystack_no_path() {
        let u = Url::parse("https://example.com").unwrap();
        assert_eq!(haystack(&u), "example.com/");
    }

    // ── pattern 匹配 ──

    #[test]
    fn test_pattern_exact_domain() {
        assert!(pattern_matches("github.com", "github.com/"));
        assert!(!pattern_matches("github.com", "sub.github.com/"));
    }

    #[test]
    fn test_pattern_path_wildcard() {
        assert!(pattern_matches("douyin.com/video/*", "douyin.com/video/123"));
        assert!(pattern_matches("v.douyin.com/*", "v.douyin.com/abc"));
        // 路径为空不匹配 /*
        assert!(!pattern_matches("douyin.com/video/*", "douyin.com/video"));
    }

    #[test]
    fn test_pattern_subdomain_wildcard() {
        assert!(pattern_matches("*.cmbchina.com/*", "www.cmbchina.com/login"));
        assert!(pattern_matches(
            "*.cmbchina.com/*",
            "personal.cmbchina.com/"
        ));
    }

    #[test]
    fn test_pattern_any_domain_login() {
        // */login 要跨任意段 → 我们简化为 * = .*（跨 /）
        assert!(pattern_matches("*/login", "example.com/login"));
        assert!(pattern_matches("*/login", "app.github.com/login"));
    }

    #[test]
    fn test_pattern_localhost() {
        assert!(pattern_matches("localhost*", "localhost/"));
        assert!(pattern_matches("localhost*", "localhost:3000/"));
    }

    #[test]
    fn test_pattern_ip_range() {
        assert!(pattern_matches("192.168.*.*", "192.168.1.1/"));
        assert!(pattern_matches("192.168.*.*", "192.168.100.200/dashboard"));
    }

    #[test]
    fn test_pattern_invalid_regex_char_escaped() {
        // path 里的点不能被当成 regex . 匹配任意字符
        // 模式 "example.com" 不该匹配 "exampleXcom/"
        assert!(!pattern_matches("example.com", "exampleXcom/"));
    }
}
