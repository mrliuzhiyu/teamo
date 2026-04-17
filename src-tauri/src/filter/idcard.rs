//! 中国大陆居民身份证校验（GB 11643-1999 / ISO 7064:1983, MOD 11-2）。
//!
//! 18 位：前 17 位为地区+生日+顺序码（全数字），末位为校验码（0-9 或 X）。
//! 校验码 = 前 17 位加权和 % 11 → 查表。

const WEIGHTS: [u32; 17] = [7, 9, 10, 5, 8, 4, 2, 1, 6, 3, 7, 9, 10, 5, 8, 4, 2];
const CHECK_TABLE: [char; 11] = ['1', '0', 'X', '9', '8', '7', '6', '5', '4', '3', '2'];

/// 校验 18 位身份证。接受 X/x 作为末位。
/// 只做校验码算法，不做地区/生日语义校验（避免误杀边境地区 / 历史地区码）。
pub fn check_id_card(s: &str) -> bool {
    if s.len() != 18 {
        return false;
    }
    let bytes = s.as_bytes();
    let mut sum = 0u32;
    for i in 0..17 {
        let c = bytes[i];
        if !c.is_ascii_digit() {
            return false;
        }
        sum += (c - b'0') as u32 * WEIGHTS[i];
    }
    let expected = CHECK_TABLE[(sum % 11) as usize];
    let last = bytes[17] as char;
    let actual = if last == 'x' { 'X' } else { last };
    actual == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_samples() {
        // 公开测试号（构造的校验码正确的假号，地区/生日/序号均合法结构）
        assert!(check_id_card("11010519491231002X"));
        assert!(check_id_card("440524188001010014"));
    }

    #[test]
    fn test_invalid_checksum() {
        // 最后一位改错
        assert!(!check_id_card("110105194912310021"));
        assert!(!check_id_card("440524188001010015"));
    }

    #[test]
    fn test_wrong_length() {
        assert!(!check_id_card("110105194912310"));
        assert!(!check_id_card("11010519491231002X1"));
    }

    #[test]
    fn test_non_digit_in_first_17() {
        assert!(!check_id_card("1101051949123100XX"));
    }

    #[test]
    fn test_lowercase_x() {
        assert!(check_id_card("11010519491231002x"));
    }
}
