//! 字符串与空白字符工具函数。
//!
//! 提供与 JavaScript 正则表达式 `\s` 语义兼容的空白字符判断（`is_js_regex_space`），
//! 以及对应的字符串裁剪函数（`js_trim`/`js_trim_start`/`js_trim_end`），
//! 确保 Rust 侧与 JS 侧的名称处理行为一致。

pub fn is_js_regex_space(ch: char) -> bool {
    matches!(
        ch as u32,
        9..=13 | 32 | 160 | 5760 | 8192..=8202 | 8232..=8233 | 8239 | 8287 | 12288 | 65279
    )
}

pub fn is_js_trim_name_char(ch: char) -> bool {
    matches!(
        ch as u32,
        9..=13 | 32 | 133 | 160 | 5760 | 8192..=8202 | 8232..=8233 | 8239 | 8287 | 12288 | 65279
    )
}

pub fn trim_js_line_end(s: &str) -> &str { s.trim_end_matches(is_js_regex_space) }

pub fn trim_js_name_like(s: &str) -> &str {
    let trimmed = s.trim_matches(is_js_regex_space);
    let Some(first) = trimmed.chars().next() else {
        return trimmed;
    };

    let start = if first == '\u{0085}' {
        trimmed
            .char_indices()
            .find_map(|(idx, ch)| (!is_js_trim_name_char(ch)).then_some(idx))
            .unwrap_or(trimmed.len())
    } else {
        0
    };

    let trimmed = &trimmed[start..];
    let Some(last) = trimmed.chars().next_back() else {
        return trimmed;
    };

    if last == '\u{0085}' {
        let end = trimmed
            .char_indices()
            .rev()
            .find_map(|(idx, ch)| (!is_js_trim_name_char(ch)).then_some(idx + ch.len_utf8()))
            .unwrap_or(0);
        &trimmed[..end]
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_regex_space_matches_expected_cases() {
        assert!(is_js_regex_space('\u{3000}'));
        assert!(is_js_regex_space('\u{2029}'));
        assert!(is_js_regex_space('\u{feff}'));
        assert!(!is_js_regex_space('\u{0085}'));
        assert!(!is_js_regex_space('a'));
    }

    #[test]
    fn js_trim_name_char_matches_expected_cases() {
        assert!(is_js_trim_name_char('\u{3000}'));
        assert!(is_js_trim_name_char('\u{0085}'));
        assert!(is_js_trim_name_char('\u{000c}'));
        assert!(is_js_trim_name_char(' '));
        assert!(!is_js_trim_name_char('a'));
    }

    #[test]
    fn trim_js_line_end_matches_expected_cases() {
        assert_eq!(trim_js_line_end("abc\u{3000}"), "abc");
        assert_eq!(trim_js_line_end("abc\u{feff}"), "abc");
        assert_eq!(trim_js_line_end("abc\u{0085}"), "abc\u{0085}");
        assert_eq!(trim_js_line_end("\u{3000}abc"), "\u{3000}abc");
    }

    #[test]
    fn trim_js_name_like_matches_expected_cases() {
        assert_eq!(trim_js_name_like("\u{3000}w\u{3000}"), "w");
        assert_eq!(trim_js_name_like("\u{0085}\u{3000}w\u{3000}\u{0085}"), "w");
        assert_eq!(trim_js_name_like("ab\u{3000}cd"), "ab\u{3000}cd");
        assert_eq!(trim_js_name_like("\u{0085}abc"), "abc");
        assert_eq!(trim_js_name_like("abc\u{0085}"), "abc");
    }
}
