//! 行ラベルの文言を組み立てる純関数。AppKit に依存しないので `cargo test` だけで検証できる。
//!
//! 範囲を持つ設定は、ラベルに常に見える形で有効範囲を括弧書きで付け足す
//! （例:「ポーリング間隔（0.05–5 秒）」）。ツールチップ（`tooltip.rs`）にも同じ範囲を
//! 出すと二重表示になるため、範囲の表示先はラベル側に一本化している。

use crate::config::{
    MAX_HUD_DURATION_SECS, MAX_HUD_FADE_DURATION_SECS, MAX_HUD_IMAGE_MAX_HEIGHT, MAX_HUD_SCALE,
    MAX_POLL_INTERVAL_SECS, MAX_TRUNCATE_MAX_LINES, MAX_TRUNCATE_MAX_WIDTH, MIN_HUD_DURATION_SECS,
    MIN_HUD_FADE_DURATION_SECS, MIN_HUD_IMAGE_MAX_HEIGHT, MIN_HUD_SCALE, MIN_POLL_INTERVAL_SECS,
    MIN_TRUNCATE_MAX_LINES, MIN_TRUNCATE_MAX_WIDTH,
};
use crate::i18n::{self, Lang, Msg};

/// 範囲表記を持つ行ラベルの Msg 一覧。新しく範囲付きラベルを追加したら、
/// `label_text` の match 分岐に加えてここにも足すこと。`label_text` 側は既存 Msg
/// を足し忘れても `_ => base.to_string()` でコンパイルが通ってしまうため、この配列を
/// 回す UT（`label_text_ranges_match_config_constants`）が唯一の歯止めになる。
/// UT 専用のため production では参照しない。
#[cfg(test)]
const RANGE_LABEL_MSGS: [Msg; 7] = [
    Msg::LabelPollInterval,
    Msg::LabelHudDuration,
    Msg::LabelHudFadeDuration,
    Msg::LabelHudScale,
    Msg::LabelMaxCharsPerLine,
    Msg::LabelMaxLines,
    Msg::LabelHudImageMaxHeight,
];

/// 行ラベルの文言。`i18n::text` の固定文言に、範囲を MIN_/MAX_ 定数から組んだ括弧書きで
/// 付け足す。値を文字列へ直書きしないのは、設定の許容範囲を変えたときラベルだけ古い値が
/// 残るのを防ぐため。範囲を持たない Msg はそのまま素通しする。
pub(super) fn label_text(lang: Lang, msg: Msg) -> String {
    let base = i18n::text(lang, msg);
    match msg {
        Msg::LabelPollInterval => format!(
            "{base}{}",
            seconds_range_suffix(lang, MIN_POLL_INTERVAL_SECS, MAX_POLL_INTERVAL_SECS)
        ),
        Msg::LabelHudDuration => format!(
            "{base}{}",
            seconds_range_suffix(lang, MIN_HUD_DURATION_SECS, MAX_HUD_DURATION_SECS)
        ),
        Msg::LabelHudFadeDuration => format!(
            "{base}{}",
            seconds_range_suffix(lang, MIN_HUD_FADE_DURATION_SECS, MAX_HUD_FADE_DURATION_SECS)
        ),
        Msg::LabelHudScale => format!(
            "{base}{}",
            scale_range_suffix(lang, MIN_HUD_SCALE, MAX_HUD_SCALE)
        ),
        Msg::LabelMaxCharsPerLine => format!(
            "{base}{}",
            count_range_suffix(lang, MIN_TRUNCATE_MAX_WIDTH, MAX_TRUNCATE_MAX_WIDTH)
        ),
        Msg::LabelMaxLines => format!(
            "{base}{}",
            count_range_suffix(lang, MIN_TRUNCATE_MAX_LINES, MAX_TRUNCATE_MAX_LINES)
        ),
        Msg::LabelHudImageMaxHeight => format!(
            "{base}{}",
            px_range_suffix(lang, MIN_HUD_IMAGE_MAX_HEIGHT, MAX_HUD_IMAGE_MAX_HEIGHT)
        ),
        _ => base.to_string(),
    }
}

/// `value` を小数第 2 位までで丸め、末尾の 0（と不要な小数点）を落とす。
/// `5.0` → `"5"`、`0.05` → `"0.05"`。範囲表記が `0.00`–`2.00` のように
/// 意味のない桁を出さないようにするための整形専用の純関数。
fn format_trimmed(value: f64) -> String {
    let formatted = format!("{value:.2}");
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
    trimmed.to_string()
}

fn count_range_suffix(lang: Lang, min: usize, max: usize) -> String {
    match lang {
        Lang::Ja => format!("（{min}–{max}）"),
        Lang::En => format!(" ({min}–{max})"),
    }
}

fn px_range_suffix(lang: Lang, min: usize, max: usize) -> String {
    match lang {
        Lang::Ja => format!("（{min}–{max} px）"),
        Lang::En => format!(" ({min}–{max} px)"),
    }
}

fn seconds_range_suffix(lang: Lang, min: f64, max: f64) -> String {
    let min = format_trimmed(min);
    let max = format_trimmed(max);
    match lang {
        Lang::Ja => format!("（{min}–{max} 秒）"),
        Lang::En => format!(" ({min}–{max} sec)"),
    }
}

fn scale_range_suffix(lang: Lang, min: f64, max: f64) -> String {
    let min = format_trimmed(min);
    let max = format_trimmed(max);
    match lang {
        Lang::Ja => format!("（{min}–{max} 倍）"),
        Lang::En => format!(" ({min}–{max}×)"),
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

    /// 期待するサフィックスは production の `seconds_range_suffix`/`scale_range_suffix`/
    /// `count_range_suffix`/`px_range_suffix` を呼ばず、この関数群で独立に組み立てる。
    /// 同じコードを呼んで比べるだけでは、Msg に誤った定数を配線しても（あるいは `_` へ
    /// 落ちて範囲が消えても）検出できないため。
    fn expected_seconds_suffix(min: f64, max: f64) -> (String, String) {
        let min = format_trimmed(min);
        let max = format_trimmed(max);
        (format!("（{min}–{max} 秒）"), format!(" ({min}–{max} sec)"))
    }

    fn expected_scale_suffix(min: f64, max: f64) -> (String, String) {
        let min = format_trimmed(min);
        let max = format_trimmed(max);
        (format!("（{min}–{max} 倍）"), format!(" ({min}–{max}×)"))
    }

    fn expected_count_suffix(min: usize, max: usize) -> (String, String) {
        (format!("（{min}–{max}）"), format!(" ({min}–{max})"))
    }

    fn expected_px_suffix(min: usize, max: usize) -> (String, String) {
        (format!("（{min}–{max} px）"), format!(" ({min}–{max} px)"))
    }

    /// Msg と定数ペアの配線を守る UT。Msg が誤った MIN_/MAX_ 定数を参照していたり、
    /// `RANGE_LABEL_MSGS` に足し忘れて `label_text` の `_` 分岐へ落ちて範囲表記ごと
    /// 消えていたりすると落ちる。
    #[test]
    fn label_text_ranges_match_config_constants() {
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
            expected_px_suffix(MIN_HUD_IMAGE_MAX_HEIGHT, MAX_HUD_IMAGE_MAX_HEIGHT);

        let expectations: [(Msg, String, String); 7] = [
            (Msg::LabelPollInterval, poll_ja, poll_en),
            (Msg::LabelHudDuration, duration_ja, duration_en),
            (Msg::LabelHudFadeDuration, fade_ja, fade_en),
            (Msg::LabelHudScale, scale_ja, scale_en),
            (Msg::LabelMaxCharsPerLine, chars_ja, chars_en),
            (Msg::LabelMaxLines, lines_ja, lines_en),
            (Msg::LabelHudImageMaxHeight, height_ja, height_en),
        ];
        assert_eq!(
            expectations.len(),
            RANGE_LABEL_MSGS.len(),
            "RANGE_LABEL_MSGS と期待値の数が食い違っている"
        );

        for (msg, ja_suffix, en_suffix) in expectations {
            assert!(
                RANGE_LABEL_MSGS.contains(&msg),
                "{msg:?} が RANGE_LABEL_MSGS に無い"
            );
            let rendered_ja = label_text(Lang::Ja, msg);
            assert!(
                rendered_ja.ends_with(&ja_suffix),
                "{msg:?} (Ja) の末尾が {ja_suffix:?} でない: {rendered_ja}"
            );
            let rendered_en = label_text(Lang::En, msg);
            assert!(
                rendered_en.ends_with(&en_suffix),
                "{msg:?} (En) の末尾が {en_suffix:?} でない: {rendered_en}"
            );
        }
    }

    #[test]
    fn label_text_passes_through_msgs_without_a_range() {
        for lang in [Lang::Ja, Lang::En] {
            assert_eq!(
                label_text(lang, Msg::LabelHudPosition),
                i18n::text(lang, Msg::LabelHudPosition)
            );
        }
    }
}
