//! Luhn 算法 — 银行卡号校验。
//!
//! 步骤：从右到左对每一位数字 ×1 或 ×2（交替），×2 后若 > 9 则减 9，
//! 全部加总后 % 10 == 0 即通过。

/// 给定纯数字字符串，检验是否满足 Luhn。
/// 非数字字符会返回 false（调用方负责先清理空格/连字符）。
pub fn check_luhn(digits: &str) -> bool {
    if digits.len() < 13 || digits.len() > 19 {
        return false;
    }
    let mut sum = 0u32;
    let mut double = false;
    for c in digits.chars().rev() {
        let Some(d) = c.to_digit(10) else {
            return false;
        };
        let v = if double {
            let n = d * 2;
            if n > 9 {
                n - 9
            } else {
                n
            }
        } else {
            d
        };
        sum += v;
        double = !double;
    }
    sum % 10 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_visa() {
        assert!(check_luhn("4111111111111111"));
    }

    #[test]
    fn test_valid_mastercard() {
        assert!(check_luhn("5500000000000004"));
    }

    #[test]
    fn test_valid_amex() {
        assert!(check_luhn("340000000000009"));
    }

    #[test]
    fn test_invalid_checksum() {
        assert!(!check_luhn("4111111111111112"));
        assert!(!check_luhn("1234567890123456"));
    }

    #[test]
    fn test_too_short_or_long() {
        assert!(!check_luhn("411111111111")); // 12
        assert!(!check_luhn("41111111111111111111")); // 20
    }

    #[test]
    fn test_non_digit() {
        assert!(!check_luhn("4111-1111-1111-1111"));
        assert!(!check_luhn("4111 1111 1111 1111"));
    }
}
