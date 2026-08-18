//! コントロールのツールチップ文言を組み立てる純関数。AppKit に依存しないので `cargo test`
//! だけで検証できる。
//!
//! 有効範囲の表記は数値欄の範囲ヒント popover（`range_hint.rs`）とスライダー自体の
//! 可動域に任せてあるため、ここでは挙動の説明に専念する。

use crate::i18n::{self, Lang, Msg};

/// コントロールのツールチップ文言。現状は `i18n::text` の固定文言をそのまま返すだけだが、
/// ここに文言組み立てをまとめておくことで、将来 Msg ごとの装飾が要るときに `i18n` 側を
/// 汚さず追加できる。
pub(super) fn tooltip_text(lang: Lang, msg: Msg) -> String {
    i18n::text(lang, msg).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_text_passes_through_the_i18n_text() {
        for lang in [Lang::Ja, Lang::En] {
            for msg in [Msg::TooltipHudScale, Msg::TooltipHudEmoji] {
                assert_eq!(tooltip_text(lang, msg), i18n::text(lang, msg));
            }
        }
    }
}
