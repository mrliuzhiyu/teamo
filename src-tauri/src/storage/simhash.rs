// simhash.rs · 64-bit SimHash (Charikar) 指纹
//
// 自实现，零外部依赖。用于剪切板内容近似去重。

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// 计算文本的 64-bit SimHash 指纹
pub fn simhash(text: &str) -> u64 {
    let mut weights = [0i32; 64];

    // 把文本切成 3-gram 特征
    let chars: Vec<char> = text.chars().collect();
    if chars.len() < 3 {
        // 短文本直接用哈希
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        return hasher.finish();
    }

    for window in chars.windows(3) {
        let token: String = window.iter().collect();
        let mut hasher = DefaultHasher::new();
        token.hash(&mut hasher);
        let hash = hasher.finish();

        for (i, w) in weights.iter_mut().enumerate() {
            if (hash >> i) & 1 == 1 {
                *w += 1;
            } else {
                *w -= 1;
            }
        }
    }

    let mut fingerprint: u64 = 0;
    for (i, w) in weights.iter().enumerate() {
        if *w > 0 {
            fingerprint |= 1u64 << i;
        }
    }

    fingerprint
}

/// 计算两个 SimHash 之间的汉明距离
pub fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// 判断两个文本是否近似重复（汉明距离 <= threshold）
pub fn is_near_duplicate(a: u64, b: u64, threshold: u32) -> bool {
    hamming_distance(a, b) <= threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_text() {
        let a = simhash("hello world 你好世界");
        let b = simhash("hello world 你好世界");
        assert_eq!(a, b);
        assert_eq!(hamming_distance(a, b), 0);
    }

    #[test]
    fn test_similar_text() {
        let a = simhash("hello world 你好世界");
        let b = simhash("hello world 你好世界！"); // 改 1 字
        let dist = hamming_distance(a, b);
        assert!(dist <= 3, "similar text should have distance <= 3, got {dist}");
    }

    #[test]
    fn test_different_text() {
        let a = simhash("hello world 你好世界");
        let b = simhash("完全不同的一段文本内容，和上面没有关系");
        let dist = hamming_distance(a, b);
        assert!(dist > 3, "different text should have distance > 3, got {dist}");
    }

    #[test]
    fn test_modified_paragraph() {
        let original = "Rust 是一门注重安全、速度和并发的编程语言。它在不使用垃圾收集器的情况下保证内存安全。";
        let modified = "Rust 是一门注重安全、速度和并发的编程语言。它在没有垃圾收集器的情况下保证内存安全。";
        let a = simhash(original);
        let b = simhash(modified);
        let dist = hamming_distance(a, b);
        assert!(
            dist <= 3,
            "slightly modified paragraph should be near-duplicate, got distance {dist}"
        );
    }

    #[test]
    fn test_short_text() {
        // 短文本（< 3 字符）也不 panic
        let a = simhash("ab");
        let b = simhash("ab");
        assert_eq!(a, b);
    }

    #[test]
    fn test_empty_text() {
        let a = simhash("");
        let b = simhash("");
        assert_eq!(a, b);
    }

    #[test]
    fn test_is_near_duplicate() {
        let a = simhash("hello world");
        let b = simhash("hello world!");
        assert!(is_near_duplicate(a, b, 3));
    }
}
