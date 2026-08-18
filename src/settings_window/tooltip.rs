//! コントロールのツールチップ文言を組み立てる純関数。AppKit に依存しないので `cargo test`
//! だけで検証できる。

use crate::config::{
    MAX_HUD_DURATION_SECS, MAX_HUD_FADE_DURATION_SECS, MAX_HUD_IMAGE_MAX_HEIGHT, MAX_HUD_SCALE,
    MAX_POLL_INTERVAL_SECS, MAX_TRUNCATE_MAX_LINES, MAX_TRUNCATE_MAX_WIDTH, MIN_HUD_DURATION_SECS,
    MIN_HUD_FADE_DURATION_SECS, MIN_HUD_IMAGE_MAX_HEIGHT, MIN_HUD_SCALE, MIN_POLL_INTERVAL_SECS,
    MIN_TRUNCATE_MAX_LINES, MIN_TRUNCATE_MAX_WIDTH,
};
use crate::i18n::{self, Lang, Msg};

/// 絵文字欄のツールチップで案内する、macOS の絵文字入力パレットを開くショートカット。
pub(super) const EMOJI_PALETTE_SHORTCUT: &str = "Ctrl + Cmd + Space";

/// 範囲表記を持つツールチップの Msg 一覧。新しく範囲付きツールチップを追加したら、
/// `tooltip_text` の match 分岐に加えてここにも足すこと。`tooltip_text` 側は既存 Msg
/// を足し忘れても `_ => base.to_string()` でコンパイルが通ってしまうため、この配列を
/// 回す UT（`tooltip_text_ranges_match_config_constants`）が唯一の歯止めになる。
/// UT 専用のため production では参照しない。
#[cfg(test)]
const RANGE_TOOLTIP_MSGS: [Msg; 7] = [
    Msg::TooltipPollInterval,
    Msg::TooltipHudDuration,
    Msg::TooltipHudFadeDuration,
    Msg::TooltipHudScale,
    Msg::TooltipMaxCharsPerLine,
    Msg::TooltipMaxLines,
    Msg::TooltipHudImageMaxHeight,
];

/// コントロールのツールチップ文言。`i18n::text` の固定文言に、範囲や
/// `EMOJI_PALETTE_SHORTCUT` など定数から組んだ表記を付け足す。値を文字列へ直書きしないのは、
/// 設定の許容範囲やショートカットを変えたときツールチップだけ古い値が残るのを防ぐため。
pub(super) fn tooltip_text(lang: Lang, msg: Msg) -> String {
    let base = i18n::text(lang, msg);
    match msg {
        Msg::TooltipPollInterval => format!(
            "{base}{}",
            seconds_range(lang, MIN_POLL_INTERVAL_SECS, MAX_POLL_INTERVAL_SECS)
        ),
        Msg::TooltipHudDuration => format!(
            "{base}{}",
            seconds_range(lang, MIN_HUD_DURATION_SECS, MAX_HUD_DURATION_SECS)
        ),
        Msg::TooltipHudFadeDuration => format!(
            "{base}{}",
            seconds_range(lang, MIN_HUD_FADE_DURATION_SECS, MAX_HUD_FADE_DURATION_SECS)
        ),
        Msg::TooltipHudScale => {
            format!("{base}{}", scale_range(lang, MIN_HUD_SCALE, MAX_HUD_SCALE))
        }
        Msg::TooltipMaxCharsPerLine => format!(
            "{base}{}",
            count_range(lang, MIN_TRUNCATE_MAX_WIDTH, MAX_TRUNCATE_MAX_WIDTH)
        ),
        Msg::TooltipMaxLines => format!(
            "{base}{}",
            count_range(lang, MIN_TRUNCATE_MAX_LINES, MAX_TRUNCATE_MAX_LINES)
        ),
        Msg::TooltipHudImageMaxHeight => format!(
            "{base}{}",
            count_range(lang, MIN_HUD_IMAGE_MAX_HEIGHT, MAX_HUD_IMAGE_MAX_HEIGHT)
        ),
        Msg::TooltipHudEmoji => format!("{base}{}", emoji_palette_hint(lang)),
        _ => base.to_string(),
    }
}

/// `value` を小数第 2 位までで丸め、末尾の 0（と不要な小数点）を落とす。
/// `5.0` → `"5"`、`0.05` → `"0.05"`。ツールチップの範囲表記が `0.00`–`2.00` のように
/// 意味のない桁を出さないようにするための整形専用の純関数。
fn format_trimmed(value: f64) -> String {
    let formatted = format!("{value:.2}");
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
    trimmed.to_string()
}

fn emoji_palette_hint(lang: Lang) -> String {
    match lang {
        // macOS 日本語 UI（編集メニュー）の正式名称「絵文字と記号」に合わせる。
        Lang::Ja => {
            format!("\n入力には「絵文字と記号」パレット（{EMOJI_PALETTE_SHORTCUT}）が便利です。")
        }
        Lang::En => format!(
            "\nThe Emoji & Symbols palette ({EMOJI_PALETTE_SHORTCUT}) is handy for entering it."
        ),
    }
}

fn count_range(lang: Lang, min: usize, max: usize) -> String {
    match lang {
        Lang::Ja => format!("\n有効範囲: {min}–{max}"),
        Lang::En => format!("\nValid range: {min}–{max}"),
    }
}

fn seconds_range(lang: Lang, min: f64, max: f64) -> String {
    let min = format_trimmed(min);
    let max = format_trimmed(max);
    match lang {
        Lang::Ja => format!("\n有効範囲: {min}–{max} 秒"),
        Lang::En => format!("\nValid range: {min}–{max} sec"),
    }
}

fn scale_range(lang: Lang, min: f64, max: f64) -> String {
    let min = format_trimmed(min);
    let max = format_trimmed(max);
    match lang {
        Lang::Ja => format!("\n有効範囲: {min}–{max} 倍"),
        Lang::En => format!("\nValid range: {min}–{max}×"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_trimmed_drops_trailing_zeros() {
        assert_eq!(format_trimmed(0.05), "0.05");
        assert_eq!(format_trimmed(0.1), "0.1");
        assert_eq!(format_trimmed(0.5), "0.5");
        assert_eq!(format_trimmed(0.0), "0");
        assert_eq!(format_trimmed(2.0), "2");
        assert_eq!(format_trimmed(5.0), "5");
        assert_eq!(format_trimmed(10.0), "10");
    }

    /// 期待するサフィックスは production の `seconds_range`/`scale_range`/`count_range` を
    /// 呼ばず、この関数群で独立に組み立てる。同じコードを呼んで比べるだけでは、Msg に
    /// 誤った定数を配線しても（あるいは `_` へ落ちて範囲が消えても）検出できないため。
    fn expected_seconds_suffix(min: f64, max: f64) -> (String, String) {
        let min = format_trimmed(min);
        let max = format_trimmed(max);
        (
            format!("\n有効範囲: {min}–{max} 秒"),
            format!("\nValid range: {min}–{max} sec"),
        )
    }

    fn expected_scale_suffix(min: f64, max: f64) -> (String, String) {
        let min = format_trimmed(min);
        let max = format_trimmed(max);
        (
            format!("\n有効範囲: {min}–{max} 倍"),
            format!("\nValid range: {min}–{max}×"),
        )
    }

    fn expected_count_suffix(min: usize, max: usize) -> (String, String) {
        (
            format!("\n有効範囲: {min}–{max}"),
            format!("\nValid range: {min}–{max}"),
        )
    }

    /// Msg と定数ペアの配線を守る UT。Msg が誤った MIN_/MAX_ 定数を参照していたり、
    /// `RANGE_TOOLTIP_MSGS` に足し忘れて `tooltip_text` の `_` 分岐へ落ちて範囲表記ごと
    /// 消えていたりすると落ちる。
    #[test]
    fn tooltip_text_ranges_match_config_constants() {
        let (poll_ja, poll_en) =
            expected_seconds_suffix(MIN_POLL_INTERVAL_SECS, MAX_POLL_INTERVAL_SECS);
        let (duration_ja, duration_en) =
            expected_seconds_suffix(MIN_HUD_DURATION_SECS, MAX_HUD_DURATION_SECS);
        let (fade_ja, fade_en) =
            expected_seconds_suffix(MIN_HUD_FADE_DURATION_SECS, MAX_HUD_FADE_DURATION_SECS);
        let (scale_ja, scale_en) = expected_scale_suffix(MIN_HUD_SCALE, MAX_HUD_SCALE);
        let (chars_ja, chars_en) =
            expected_count_suffix(MIN_TRUNCATE_MAX_WIDTH, MAX_TRUNCATE_MAX_WIDTH);
        let (lines_ja, lines_en) =
            expected_count_suffix(MIN_TRUNCATE_MAX_LINES, MAX_TRUNCATE_MAX_LINES);
        let (height_ja, height_en) =
            expected_count_suffix(MIN_HUD_IMAGE_MAX_HEIGHT, MAX_HUD_IMAGE_MAX_HEIGHT);

        let expectations: [(Msg, String, String); 7] = [
            (Msg::TooltipPollInterval, poll_ja, poll_en),
            (Msg::TooltipHudDuration, duration_ja, duration_en),
            (Msg::TooltipHudFadeDuration, fade_ja, fade_en),
            (Msg::TooltipHudScale, scale_ja, scale_en),
            (Msg::TooltipMaxCharsPerLine, chars_ja, chars_en),
            (Msg::TooltipMaxLines, lines_ja, lines_en),
            (Msg::TooltipHudImageMaxHeight, height_ja, height_en),
        ];
        assert_eq!(
            expectations.len(),
            RANGE_TOOLTIP_MSGS.len(),
            "RANGE_TOOLTIP_MSGS と期待値の数が食い違っている"
        );

        for (msg, ja_suffix, en_suffix) in expectations {
            assert!(
                RANGE_TOOLTIP_MSGS.contains(&msg),
                "{msg:?} が RANGE_TOOLTIP_MSGS に無い"
            );
            let rendered_ja = tooltip_text(Lang::Ja, msg);
            assert!(
                rendered_ja.ends_with(&ja_suffix),
                "{msg:?} (Ja) の末尾が {ja_suffix:?} でない: {rendered_ja}"
            );
            let rendered_en = tooltip_text(Lang::En, msg);
            assert!(
                rendered_en.ends_with(&en_suffix),
                "{msg:?} (En) の末尾が {en_suffix:?} でない: {rendered_en}"
            );
        }
    }

    #[test]
    fn tooltip_text_for_emoji_mentions_the_palette_shortcut() {
        for lang in [Lang::Ja, Lang::En] {
            let rendered = tooltip_text(lang, Msg::TooltipHudEmoji);
            assert!(
                rendered.contains(EMOJI_PALETTE_SHORTCUT),
                "{lang:?} に {EMOJI_PALETTE_SHORTCUT} が無い: {rendered}"
            );
        }
    }

    #[test]
    fn tooltip_text_for_emoji_uses_the_macos_palette_name_in_japanese() {
        let rendered = tooltip_text(Lang::Ja, Msg::TooltipHudEmoji);
        assert!(
            rendered.contains("絵文字と記号"),
            "macOS の編集メニュー表記「絵文字と記号」が無い: {rendered}"
        );
    }
}
