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

fn emoji_palette_hint(lang: Lang) -> String {
    match lang {
        Lang::Ja => format!("\n入力には絵文字パレット（{EMOJI_PALETTE_SHORTCUT}）が便利です。"),
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
    match lang {
        Lang::Ja => format!("\n有効範囲: {min:.2}–{max:.2} 秒"),
        Lang::En => format!("\nValid range: {min:.2}–{max:.2} sec"),
    }
}

fn scale_range(lang: Lang, min: f64, max: f64) -> String {
    match lang {
        Lang::Ja => format!("\n有効範囲: {min:.2}–{max:.2} 倍"),
        Lang::En => format!("\nValid range: {min:.2}–{max:.2}×"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 範囲を持つツールチップのドリフト防止用。定数を変えたのにツールチップの表記を
    /// 直し忘れると落ちる。文言全体の丸写しは避け、min/max が含まれることだけ見る。
    #[test]
    fn tooltip_text_ranges_match_config_constants() {
        let cases: [(Msg, String, String); 7] = [
            (
                Msg::TooltipPollInterval,
                format!("{MIN_POLL_INTERVAL_SECS:.2}"),
                format!("{MAX_POLL_INTERVAL_SECS:.2}"),
            ),
            (
                Msg::TooltipHudDuration,
                format!("{MIN_HUD_DURATION_SECS:.2}"),
                format!("{MAX_HUD_DURATION_SECS:.2}"),
            ),
            (
                Msg::TooltipHudFadeDuration,
                format!("{MIN_HUD_FADE_DURATION_SECS:.2}"),
                format!("{MAX_HUD_FADE_DURATION_SECS:.2}"),
            ),
            (
                Msg::TooltipHudScale,
                format!("{MIN_HUD_SCALE:.2}"),
                format!("{MAX_HUD_SCALE:.2}"),
            ),
            (
                Msg::TooltipMaxCharsPerLine,
                MIN_TRUNCATE_MAX_WIDTH.to_string(),
                MAX_TRUNCATE_MAX_WIDTH.to_string(),
            ),
            (
                Msg::TooltipMaxLines,
                MIN_TRUNCATE_MAX_LINES.to_string(),
                MAX_TRUNCATE_MAX_LINES.to_string(),
            ),
            (
                Msg::TooltipHudImageMaxHeight,
                MIN_HUD_IMAGE_MAX_HEIGHT.to_string(),
                MAX_HUD_IMAGE_MAX_HEIGHT.to_string(),
            ),
        ];

        for (msg, min_text, max_text) in cases {
            for lang in [Lang::Ja, Lang::En] {
                let rendered = tooltip_text(lang, msg);
                assert!(
                    rendered.contains(&min_text),
                    "{msg:?} ({lang:?}) に min {min_text} が無い: {rendered}"
                );
                assert!(
                    rendered.contains(&max_text),
                    "{msg:?} ({lang:?}) に max {max_text} が無い: {rendered}"
                );
            }
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
}
