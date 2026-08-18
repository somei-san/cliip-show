//! 数値入力欄（ステッパー行）の有効範囲を案内する popover 本文を組み立てる純関数。
//! スライダー行はつまみの可動域自体が範囲を強制するため対象外。

use crate::config::{
    MAX_HUD_IMAGE_MAX_HEIGHT, MAX_TRUNCATE_MAX_LINES, MAX_TRUNCATE_MAX_WIDTH,
    MIN_HUD_IMAGE_MAX_HEIGHT, MIN_TRUNCATE_MAX_LINES, MIN_TRUNCATE_MAX_WIDTH,
};
use crate::i18n::{self, Lang, Msg};

/// 案内文言を持つ数値欄（行ラベルの Msg で識別）の一覧。新しく数値欄を追加したら、
/// `range_hint_text` の match 分岐に加えてここにも足すこと。`range_hint_text` 側は既存 Msg
/// を足し忘れても `_ => String::new()` でコンパイルが通ってしまうため、この配列を回す UT
/// （`range_hint_text_matches_config_constants`）が唯一の歯止めになる。
/// UT 専用のため production では参照しない。
#[cfg(test)]
const RANGE_HINT_MSGS: [Msg; 3] = [
    Msg::LabelMaxCharsPerLine,
    Msg::LabelMaxLines,
    Msg::LabelHudImageMaxHeight,
];

/// 数値欄の範囲ヒント popover の本文。`Msg::RangeHintPrefix`（「有効範囲: 」等）に続けて、
/// 半角の範囲表記を MIN_/MAX_ 定数から組む。値を文字列へ直書きしないのは、設定の許容範囲を
/// 変えたとき文言だけ古い値が残るのを防ぐため。対象外の Msg は空文字列を返す
/// （呼び出し側はこれを「対象外」として扱う）。
pub(super) fn range_hint_text(lang: Lang, msg: Msg) -> String {
    let prefix = i18n::text(lang, Msg::RangeHintPrefix);
    match msg {
        Msg::LabelMaxCharsPerLine => {
            format!(
                "{prefix}{}",
                count_range(MIN_TRUNCATE_MAX_WIDTH, MAX_TRUNCATE_MAX_WIDTH)
            )
        }
        Msg::LabelMaxLines => {
            format!(
                "{prefix}{}",
                count_range(MIN_TRUNCATE_MAX_LINES, MAX_TRUNCATE_MAX_LINES)
            )
        }
        Msg::LabelHudImageMaxHeight => format!(
            "{prefix}{}",
            px_range(MIN_HUD_IMAGE_MAX_HEIGHT, MAX_HUD_IMAGE_MAX_HEIGHT)
        ),
        _ => String::new(),
    }
}

fn count_range(min: usize, max: usize) -> String {
    format!("{min}–{max}")
}

fn px_range(min: usize, max: usize) -> String {
    format!("{min}–{max} px")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Msg::RangeHintPrefix` の文言そのものを固定する。`range_hint_text_matches_config_constants`
    /// はこの値をリテラルで直書きして使うため（production の `i18n::text` 呼び出しに頼ると
    /// 接頭辞の食い違いを検出できないトートロジーになる）、この UT が唯一の歯止めになる。
    #[test]
    fn range_hint_prefix_text_is_fixed() {
        assert_eq!(i18n::text(Lang::Ja, Msg::RangeHintPrefix), "有効範囲：");
        assert_eq!(i18n::text(Lang::En, Msg::RangeHintPrefix), "Valid range: ");
    }

    /// Msg と定数ペアの配線を守る UT。誤った MIN_/MAX_ 定数を参照していたり、
    /// `RANGE_HINT_MSGS` に足し忘れて `range_hint_text` の `_` 分岐へ落ちて文言が
    /// 消えていたりすると落ちる。期待値は production の `count_range`/`px_range` を呼ばず、
    /// この関数群で独立に組み立てる。範囲の数字自体は言語に依存しないため Lang::Ja のみで
    /// 配線を検証する（接頭辞は上の `range_hint_prefix_text_is_fixed` で別途固定済み）。
    #[test]
    fn range_hint_text_matches_config_constants() {
        const PREFIX: &str = "有効範囲：";

        fn expected_count(min: usize, max: usize) -> String {
            format!("{PREFIX}{min}–{max}")
        }
        fn expected_px(min: usize, max: usize) -> String {
            format!("{PREFIX}{min}–{max} px")
        }

        let expectations: [(Msg, String); 3] = [
            (
                Msg::LabelMaxCharsPerLine,
                expected_count(MIN_TRUNCATE_MAX_WIDTH, MAX_TRUNCATE_MAX_WIDTH),
            ),
            (
                Msg::LabelMaxLines,
                expected_count(MIN_TRUNCATE_MAX_LINES, MAX_TRUNCATE_MAX_LINES),
            ),
            (
                Msg::LabelHudImageMaxHeight,
                expected_px(MIN_HUD_IMAGE_MAX_HEIGHT, MAX_HUD_IMAGE_MAX_HEIGHT),
            ),
        ];
        assert_eq!(
            expectations.len(),
            RANGE_HINT_MSGS.len(),
            "RANGE_HINT_MSGS と期待値の数が食い違っている"
        );

        for (msg, expected) in expectations {
            assert!(
                RANGE_HINT_MSGS.contains(&msg),
                "{msg:?} が RANGE_HINT_MSGS に無い"
            );
            assert_eq!(range_hint_text(Lang::Ja, msg), expected, "{msg:?}");
        }
    }

    #[test]
    fn range_hint_text_is_empty_for_msgs_without_a_range() {
        for lang in [Lang::Ja, Lang::En] {
            assert_eq!(range_hint_text(lang, Msg::LabelHudPosition), "");
        }
    }
}
