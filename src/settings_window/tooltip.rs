//! コントロールのツールチップ文言を組み立てる純関数。AppKit に依存しないので `cargo test`
//! だけで検証できる。
//!
//! 有効範囲の表記は数値欄の補助テキスト（`range_hint.rs`）とスライダー自体の可動域に
//! 任せてあるため、ここでは挙動の説明に専念する。

use crate::i18n::{self, Lang, Msg};

/// 絵文字欄のツールチップで案内する、macOS の絵文字入力パレットを開くショートカット。
pub(super) const EMOJI_PALETTE_SHORTCUT: &str = "Ctrl + Cmd + Space";

/// コントロールのツールチップ文言。`i18n::text` の固定文言に、絵文字欄だけ
/// `EMOJI_PALETTE_SHORTCUT` など定数から組んだ案内を付け足す。値を文字列へ直書きしないのは、
/// ショートカットを変えたときツールチップだけ古い値が残るのを防ぐため。
pub(super) fn tooltip_text(lang: Lang, msg: Msg) -> String {
    let base = i18n::text(lang, msg);
    match msg {
        Msg::TooltipHudEmoji => format!("{base}{}", emoji_palette_hint(lang)),
        _ => base.to_string(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn tooltip_text_passes_through_msgs_without_special_handling() {
        for lang in [Lang::Ja, Lang::En] {
            assert_eq!(
                tooltip_text(lang, Msg::TooltipHudScale),
                i18n::text(lang, Msg::TooltipHudScale)
            );
        }
    }
}
