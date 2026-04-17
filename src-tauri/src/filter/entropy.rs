//! Shannon 熵计算 — 判断字符串随机性。
//!
//! 用途：过滤「看起来像密码/token」的高熵串。
//! - 自然语言熵约 1.5-3.0 bit/char
//! - 强密码/token 通常 > 3.5 bit/char
//! - 纯随机 base64 ~ 6.0 bit/char

use std::collections::HashMap;

/// 返回每字符 Shannon 熵（bit/char）。空串返回 0。
///
/// Phase 1 的 password detector 最终没用这个（见 sensitive.rs），
/// 但保留它作为工具层 —— Phase 2 的 App 规则或云端侧判重可能会用。
#[allow(dead_code)]
pub fn shannon_entropy(s: &str) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    let mut counts: HashMap<char, usize> = HashMap::new();
    let mut total = 0usize;
    for c in s.chars() {
        *counts.entry(c).or_insert(0) += 1;
        total += 1;
    }
    let total_f = total as f64;
    let mut entropy = 0.0_f64;
    for &c in counts.values() {
        let p = c as f64 / total_f;
        entropy -= p * p.log2();
    }
    entropy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        assert_eq!(shannon_entropy(""), 0.0);
    }

    #[test]
    fn test_single_char() {
        // 单字符重复 → 熵 0
        assert_eq!(shannon_entropy("aaaaaa"), 0.0);
    }

    #[test]
    fn test_binary() {
        // 01 各半 → 熵 1.0
        let e = shannon_entropy("aabb");
        assert!((e - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_natural_language_low() {
        // 英文自然语言 < 3.5
        assert!(shannon_entropy("hello world this is a test") < 3.5);
        assert!(shannon_entropy("password") < 3.5);
    }

    #[test]
    fn test_random_like_high() {
        // 强密码 > 3.5
        assert!(shannon_entropy("Xk7!pQ2@mB9#") > 3.5);
        assert!(shannon_entropy("sk-a3b5c7d9e1f2g4h6i8j0k2l4m6n8") > 3.5);
    }
}
