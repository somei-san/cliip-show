//! 数値フィールドの入力文字を絞り込む純関数。AppKit に依存しないので `cargo test` だけで検証できる。

/// ASCII 数字だけを残す。全角数字（U+FF10..=U+FF19）は ASCII へ正規化して残し、それ以外は削除する。
///
/// `caret_utf16` は削除前の文字列に対する UTF-16 単位のキャレット位置（`NSRange.location` と同じ
/// 単位）。削除された文字の分だけ前に詰めた、削除後の文字列に対するキャレット位置を返す。
/// 範囲外の位置は文字列末尾にクランプする。
pub(super) fn filter_digits(raw: &str, caret_utf16: usize) -> (String, usize) {
    let mut out = String::with_capacity(raw.len());
    let mut consumed_utf16 = 0usize;
    let mut produced_utf16 = 0usize;
    let mut caret_out = 0usize;
    let mut caret_placed = false;

    for c in raw.chars() {
        if !caret_placed && consumed_utf16 >= caret_utf16 {
            caret_out = produced_utf16;
            caret_placed = true;
        }
        if let Some(normalized) = normalize_digit(c) {
            out.push(normalized);
            produced_utf16 += normalized.len_utf16();
        }
        consumed_utf16 += c.len_utf16();
    }
    if !caret_placed {
        caret_out = produced_utf16;
    }

    (out, caret_out)
}

fn normalize_digit(c: char) -> Option<char> {
    match c {
        '0'..='9' => Some(c),
        '\u{FF10}'..='\u{FF19}' => char::from_u32('0' as u32 + (c as u32 - 0xFF10)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::filter_digits;

    #[test]
    fn passes_through_ascii_digits() {
        assert_eq!(filter_digits("123", 3), ("123".to_string(), 3));
    }

    #[test]
    fn removes_non_digit_characters() {
        // "12a3" から 'a' が消え、消えた分だけキャレットが前に詰まる（12|3 相当）
        assert_eq!(filter_digits("12a3", 3), ("123".to_string(), 2));
    }

    #[test]
    fn normalizes_fullwidth_digits_to_ascii() {
        assert_eq!(filter_digits("\u{FF11}\u{FF12}", 2), ("12".to_string(), 2));
    }

    #[test]
    fn mixed_ascii_and_junk_keeps_only_digits() {
        assert_eq!(filter_digits("1a2b3", 5), ("123".to_string(), 3));
    }

    #[test]
    fn empty_input_stays_empty() {
        assert_eq!(filter_digits("", 0), (String::new(), 0));
    }

    #[test]
    fn caret_at_start_stays_at_start() {
        assert_eq!(filter_digits("abc123", 0), ("123".to_string(), 0));
    }

    #[test]
    fn caret_beyond_input_clamps_to_end() {
        assert_eq!(filter_digits("12a3", 100), ("123".to_string(), 3));
    }
}
