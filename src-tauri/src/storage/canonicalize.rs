// canonicalize.rs · 剪切板内容归一化
//
// 剪切板去重的正确策略（参考 Maccy / CopyQ / Ditto）：
// 对"用户连续复制同一段内容，末尾多/少空白或标点"这种典型重复做归一化 + 精确匹配。
// 真正的词级近似去重（SimHash / MinHash）对短文本数学上不稳定，不在本模块范围。
//
// 归一化规则：
// 1. trim 首尾空白
// 2. 折叠连续空白为单个空格
// 3. 去除零宽字符（BOM / ZWSP / ZWNJ / ZWJ）
// 4. 去除末尾常见句末标点（。！？.!?，,;；:：）

/// 归一化文本：用于"剪切板内容等价"判断
pub fn canonicalize(text: &str) -> String {
    let trimmed = text.trim();
    let mut out = String::with_capacity(trimmed.len());
    let mut prev_space = false;

    for c in trimmed.chars() {
        // 跳过零宽字符
        if matches!(c, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}') {
            continue;
        }
        if c.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(c);
            prev_space = false;
        }
    }

    // 去末尾句末标点
    while let Some(last) = out.chars().last() {
        if matches!(
            last,
            '。' | '！' | '？' | '.' | '!' | '?' | '，' | ',' | ';' | '；' | ':' | '：'
        ) {
            out.pop();
        } else {
            break;
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical() {
        assert_eq!(canonicalize("hello"), canonicalize("hello"));
    }

    #[test]
    fn test_trailing_whitespace() {
        assert_eq!(canonicalize("hello"), canonicalize("hello  "));
        assert_eq!(canonicalize("hello"), canonicalize("  hello\n"));
    }

    #[test]
    fn test_collapse_whitespace() {
        assert_eq!(canonicalize("hello world"), canonicalize("hello    world"));
        assert_eq!(canonicalize("hello world"), canonicalize("hello\tworld"));
    }

    #[test]
    fn test_trailing_punctuation() {
        assert_eq!(canonicalize("hello"), canonicalize("hello."));
        assert_eq!(canonicalize("hello world"), canonicalize("hello world!"));
        assert_eq!(canonicalize("你好世界"), canonicalize("你好世界。"));
        assert_eq!(canonicalize("你好世界"), canonicalize("你好世界！"));
    }

    #[test]
    fn test_zero_width() {
        assert_eq!(canonicalize("hello"), canonicalize("hel\u{200B}lo"));
        assert_eq!(canonicalize("hello"), canonicalize("\u{FEFF}hello"));
    }

    #[test]
    fn test_different_content_stays_different() {
        assert_ne!(canonicalize("hello"), canonicalize("world"));
        assert_ne!(canonicalize("第 1 条"), canonicalize("第 2 条"));
    }

    #[test]
    fn test_word_change_stays_different() {
        // 词级修改（真实不同的复制）不应被归一化合并
        let a = canonicalize("Rust 是一门注重安全的编程语言");
        let b = canonicalize("Rust 是一门注重速度的编程语言");
        assert_ne!(a, b);
    }
}
