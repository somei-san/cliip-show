//! UI 文言の言語ごとの表記。
//!
//! `.lproj` と `NSLocalizedString` は使わない。バンドルが読む言語は起動時に決まるため、
//! 設定からの切り替えに再起動が要るようになる。文言は Rust 側の表として持ち、
//! `Msg` を増やしたときに訳が欠けていればコンパイルエラーになるようにしている。
//!
//! stderr に出す診断メッセージと `--help` は英語固定で、この表には載せない。

use objc2::runtime::AnyObject;
use objc2::{class, msg_send};

use crate::config::types::LanguageSetting;
use crate::objc_helpers::nsstring_to_string;

/// UI に出すアプリ名。言語によらずこの表記にする。
///
/// コマンド名・設定ファイルのパス・LaunchAgent のラベルは `cliip-show` のままにする。
/// 既存のインストールが読み書きしている先なので、変えるとログイン項目が二重になり、
/// 保存済みの設定も読めなくなる。
pub const APP_NAME: &str = "Cliip Show";

/// 表示に使う言語。`LanguageSetting::Auto` を OS のロケールで解決した結果でもある。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ja,
    En,
}

/// 言語ごとの表記を持つ UI 文言。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Msg {
    MenuSettings,
    MenuPause,
    MenuAbout,
    MenuQuit,
    MenuEdit,
    MenuCut,
    MenuCopy,
    MenuPaste,
    MenuSelectAll,

    LoginPromptMessage,
    LoginPromptDetail,
    LoginPromptEnable,
    LoginPromptLater,
    LoginPromptSuppress,

    MenuBarIconHiddenMessage,
    MenuBarIconHiddenDetail,

    SettingsTitle,
    TabSettings,
    TabSupport,
    SettingsStartAtLogin,
    SettingsShowMenuBarIcon,
    LabelPollInterval,
    LabelHudDuration,
    LabelHudFadeDuration,
    LabelHudScale,
    LabelMaxCharsPerLine,
    LabelMaxLines,
    LabelHudImageMaxHeight,
    LabelHudPosition,
    LabelHudBackgroundColor,
    LabelHudBackgroundOpacity,
    LabelHudEmoji,
    TooltipPollInterval,
    TooltipHudDuration,
    TooltipHudFadeDuration,
    TooltipHudScale,
    TooltipMaxCharsPerLine,
    TooltipMaxLines,
    TooltipHudImageMaxHeight,
    TooltipHudBackgroundOpacity,
    TooltipHudEmoji,
    /// 数値欄の範囲ヒント popover の接頭辞。`range_hint::range_hint_text` が
    /// 半角の範囲表記（例: "1–500"）の先頭に付ける。
    RangeHintPrefix,
    LabelLanguage,
    LanguageAuto,
    ButtonRestoreDefaults,
    ButtonPreview,
    ButtonSave,

    SupportBuyBeer,
    SupportMessage,
    /// About パネルのクレジット欄。
    AboutDescription,

    EmojiTooLong,
    EmojiNotEmoji,

    PreviewShortText,
    LaunchNotice,
}

/// `msg` を `lang` の表記で返す。
pub fn text(lang: Lang, msg: Msg) -> &'static str {
    let (ja, en) = match msg {
        Msg::MenuSettings => ("設定…", "Settings…"),
        Msg::MenuPause => ("一時停止", "Pause"),
        Msg::MenuAbout => ("Cliip Show について", "About Cliip Show"),
        Msg::MenuQuit => ("Cliip Show を終了", "Quit Cliip Show"),
        Msg::MenuEdit => ("編集", "Edit"),
        Msg::MenuCut => ("切り取り", "Cut"),
        Msg::MenuCopy => ("コピー", "Copy"),
        Msg::MenuPaste => ("ペースト", "Paste"),
        Msg::MenuSelectAll => ("すべてを選択", "Select All"),

        Msg::LoginPromptMessage => (
            "ログイン時に Cliip Show を自動起動しますか？",
            "Start Cliip Show automatically at login?",
        ),
        Msg::LoginPromptDetail => (
            "設定ウィンドウの「ログイン時に自動起動」からいつでも変更できます。",
            "You can change this any time from “Start at login” in the settings window.",
        ),
        Msg::LoginPromptEnable => ("有効にする", "Enable"),
        Msg::LoginPromptLater => ("あとで", "Not Now"),
        Msg::LoginPromptSuppress => ("今後表示しない", "Don’t ask again"),

        Msg::MenuBarIconHiddenMessage => (
            "メニューバーのアイコンを非表示にしました",
            "The menu bar icon is now hidden",
        ),
        Msg::MenuBarIconHiddenDetail => (
            "Cliip Show をもう一度起動すると設定ウィンドウが開きます。アイコンはそこから表示に戻せます。",
            "Launch Cliip Show again to open the settings window, where you can show the icon again.",
        ),

        Msg::SettingsTitle => ("Cliip Show 設定", "Cliip Show Settings"),
        Msg::TabSettings => ("設定", "Settings"),
        Msg::TabSupport => ("寄付", "Support"),
        Msg::SettingsStartAtLogin => ("ログイン時に自動起動", "Start at login"),
        Msg::SettingsShowMenuBarIcon => ("メニューバーにアイコンを表示", "Show icon in menu bar"),
        Msg::LabelPollInterval => ("ポーリング間隔（秒）", "Polling interval (sec)"),
        Msg::LabelHudDuration => ("表示時間（秒）", "Display duration (sec)"),
        Msg::LabelHudFadeDuration => ("フェード時間（秒）", "Fade duration (sec)"),
        Msg::LabelHudScale => ("HUDサイズ倍率", "HUD scale"),
        Msg::LabelMaxCharsPerLine => ("1行の最大文字数", "Max characters per line"),
        Msg::LabelMaxLines => ("最大行数", "Max lines"),
        Msg::LabelHudImageMaxHeight => {
            ("画像サムネイル高さ上限（px）", "Max thumbnail height (px)")
        }
        Msg::LabelHudPosition => ("表示位置", "Position"),
        Msg::LabelHudBackgroundColor => ("背景色", "Background color"),
        Msg::LabelHudBackgroundOpacity => ("背景の不透明度", "Background opacity"),
        Msg::LabelHudEmoji => ("アイコン絵文字", "Icon emoji"),
        Msg::TooltipPollInterval => (
            "クリップボードの内容を確認する間隔。",
            "How often the clipboard is checked for changes.",
        ),
        Msg::TooltipHudDuration => ("HUD を表示し続ける時間。", "How long the HUD stays visible."),
        Msg::TooltipHudFadeDuration => (
            "HUD が消えるときのフェード時間。0 秒にするとフェードせず即座に消えます。",
            "Fade-out duration when the HUD disappears. Set to 0 to disappear instantly with no fade.",
        ),
        Msg::TooltipHudScale => (
            "HUD 全体の表示サイズの倍率。画像サムネイル高さの上限にも掛かります。",
            "Overall display scale of the HUD. Also multiplies the image thumbnail height limit.",
        ),
        Msg::TooltipMaxCharsPerLine => (
            "1行に表示する最大文字数。超えた分は省略されます。",
            "Maximum number of characters shown per line. Extra characters are truncated.",
        ),
        Msg::TooltipMaxLines => (
            "表示する最大行数。超えた行は省略されます。",
            "Maximum number of lines shown. Extra lines are truncated.",
        ),
        Msg::TooltipHudImageMaxHeight => (
            "画像プレビューのサムネイル高さの上限（px）。実際の上限はこの値に HUDサイズ倍率を掛けた値になります。",
            "Maximum thumbnail height for image previews (px). The actual limit is this value multiplied by the HUD scale.",
        ),
        Msg::TooltipHudBackgroundOpacity => (
            "HUD の背景の不透明度。1.00 で背景が透けなくなります。文字とアイコンは常に透けません。",
            "Opacity of the HUD background. At 1.00 the background is fully opaque. Text and icon are always opaque.",
        ),
        Msg::TooltipHudEmoji => (
            "HUD の左端に表示するアイコン絵文字。絵文字1文字だけ入力できます。空にするとアイコンは出ません。",
            "The icon emoji shown at the left edge of the HUD. Only a single emoji is allowed. Leave it empty to show no icon.",
        ),
        Msg::RangeHintPrefix => ("有効範囲：", "Valid range: "),
        Msg::LabelLanguage => ("言語", "Language"),
        Msg::LanguageAuto => ("システムに合わせる", "Match System"),
        Msg::ButtonRestoreDefaults => ("デフォルトに戻す", "Restore Defaults"),
        Msg::ButtonPreview => ("お試し表示", "Preview"),
        Msg::ButtonSave => ("保存", "Save"),

        Msg::SupportBuyBeer => ("ビールを奢る…", "Buy me a beer…"),
        Msg::SupportMessage => (
            "Cliip Show はすべての機能を無償で提供しています。\nビールを奢ってもらえると開発の励みになります 🍺",
            "Cliip Show is free, with every feature included.\nBuying me a beer keeps the work going 🍺",
        ),
        Msg::AboutDescription => (
            "コピーした内容を画面中央に表示する macOS の常駐アプリ 🥜",
            "A macOS menu bar app that shows what you just copied 🥜",
        ),
        Msg::EmojiTooLong => ("絵文字1文字だけ入力できます", "Enter a single emoji"),
        Msg::EmojiNotEmoji => ("絵文字を入力してください", "Enter an emoji"),

        Msg::PreviewShortText => (
            "サンプル表示：短いテキストです",
            "Preview: a short line of text",
        ),

        Msg::LaunchNotice => ("Cliip Show を起動しました", "Cliip Show started"),
    };

    match lang {
        Lang::Ja => ja,
        Lang::En => en,
    }
}

/// 言語ポップアップに並べる選択肢と、その並び順。ラベルは `language_label` が返す。
pub const LANGUAGE_CHOICES: [LanguageSetting; 3] = [
    LanguageSetting::Auto,
    LanguageSetting::Ja,
    LanguageSetting::En,
];

/// 言語ポップアップに出す `setting` のラベル。
///
/// 日本語 / English は表示言語を切り替えても自国語表記のままにする。英語表示の人が
/// 日本語を探すときも、その逆も、探している言語がその言語で書かれている方が見つかる。
pub fn language_label(lang: Lang, setting: LanguageSetting) -> &'static str {
    match setting {
        LanguageSetting::Auto => text(lang, Msg::LanguageAuto),
        LanguageSetting::Ja => "日本語",
        LanguageSetting::En => "English",
    }
}

/// お試し表示で使う長い 1 行。折り返しを確認できる長さまで繰り返す。
pub fn preview_long_line(lang: Lang) -> String {
    let unit = match lang {
        Lang::Ja => "サンプル表示の見た目を確認するための長めの一行です。",
        Lang::En => "A fairly long line of sample text for checking how the preview looks. ",
    };
    unit.repeat(5)
}

/// お試し表示で使う `n` 行目のサンプル行。
pub fn preview_numbered_line(lang: Lang, n: usize) -> String {
    match lang {
        Lang::Ja => format!("これは{n}行目のサンプル行です。"),
        Lang::En => format!("This is sample line {n}."),
    }
}

/// 設定値を実際に表示に使う言語へ解決する。
///
/// # Safety
/// AppKit / Foundation を初期化済みのプロセスから呼ぶこと。
pub unsafe fn resolve(setting: LanguageSetting) -> Lang {
    // Auto のときだけ OS へ問い合わせる。絵文字欄の打鍵ごとに呼ばれる経路があるため、
    // 明示指定時に NSLocale を引かないようにしている。
    match setting {
        LanguageSetting::Auto => system_language(),
        LanguageSetting::Ja => Lang::Ja,
        LanguageSetting::En => Lang::En,
    }
}

/// `resolve` から OS のロケール参照を切り離したもの。
pub fn resolve_with_system(setting: LanguageSetting, system: Lang) -> Lang {
    match setting {
        LanguageSetting::Auto => system,
        LanguageSetting::Ja => Lang::Ja,
        LanguageSetting::En => Lang::En,
    }
}

/// OS の優先言語の先頭から表示言語を決める。日本語以外はすべて英語に倒す。
///
/// # Safety
/// Foundation を初期化済みのプロセスから呼ぶこと。
pub unsafe fn system_language() -> Lang {
    let languages: *mut AnyObject = msg_send![class!(NSLocale), preferredLanguages];
    if languages.is_null() {
        return Lang::En;
    }
    let count: usize = msg_send![languages, count];
    if count == 0 {
        return Lang::En;
    }
    let first: *mut AnyObject = msg_send![languages, objectAtIndex: 0usize];
    match nsstring_to_string(first) {
        Some(tag) => lang_from_bcp47(&tag),
        None => Lang::En,
    }
}

/// BCP 47 の言語タグ（`ja-JP` など）から表示言語を決める。
pub fn lang_from_bcp47(tag: &str) -> Lang {
    let primary = tag.split(['-', '_']).next().unwrap_or("");
    if primary.eq_ignore_ascii_case("ja") {
        Lang::Ja
    } else {
        Lang::En
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_from_bcp47_picks_japanese_only_for_ja() {
        assert_eq!(lang_from_bcp47("ja"), Lang::Ja);
        assert_eq!(lang_from_bcp47("ja-JP"), Lang::Ja);
        assert_eq!(lang_from_bcp47("ja_JP"), Lang::Ja);
        assert_eq!(lang_from_bcp47("JA-jp"), Lang::Ja);
        assert_eq!(lang_from_bcp47("en-US"), Lang::En);
        assert_eq!(lang_from_bcp47("jam"), Lang::En);
        assert_eq!(lang_from_bcp47(""), Lang::En);
    }

    /// 言語ポップアップは `LANGUAGE_CHOICES` の並び順とインデックスで値を往復させる。
    /// `LanguageSetting` に値を足してここへ足し忘れると、その値は選べなくなる。
    #[test]
    fn language_choices_cover_every_setting() {
        for setting in [
            LanguageSetting::Auto,
            LanguageSetting::Ja,
            LanguageSetting::En,
        ] {
            assert!(
                LANGUAGE_CHOICES.contains(&setting),
                "{setting:?} が LANGUAGE_CHOICES に無い"
            );
        }
        assert_eq!(LANGUAGE_CHOICES.len(), 3);
    }

    #[test]
    fn language_labels_are_distinct_in_both_languages() {
        for lang in [Lang::Ja, Lang::En] {
            let labels: Vec<&str> = LANGUAGE_CHOICES
                .iter()
                .map(|setting| language_label(lang, *setting))
                .collect();
            let mut unique = labels.clone();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(
                unique.len(),
                labels.len(),
                "{lang:?} でラベルが重複している"
            );
        }
    }

    /// アプリ名を含む文言。表記が `APP_NAME` からずれると、メニューバーのメニューと
    /// 設定ウィンドウのタイトルで名前が食い違って見える。
    #[test]
    fn messages_naming_the_app_use_the_display_name() {
        for msg in [
            Msg::MenuAbout,
            Msg::MenuQuit,
            Msg::LoginPromptMessage,
            Msg::MenuBarIconHiddenDetail,
            Msg::SettingsTitle,
            Msg::SupportMessage,
            Msg::LaunchNotice,
        ] {
            for lang in [Lang::Ja, Lang::En] {
                let rendered = text(lang, msg);
                assert!(
                    rendered.contains(APP_NAME),
                    "{msg:?} ({lang:?}) に {APP_NAME} が無い: {rendered}"
                );
            }
        }
    }

    #[test]
    fn resolve_with_system_follows_system_only_for_auto() {
        assert_eq!(
            resolve_with_system(LanguageSetting::Auto, Lang::Ja),
            Lang::Ja
        );
        assert_eq!(
            resolve_with_system(LanguageSetting::Auto, Lang::En),
            Lang::En
        );
        assert_eq!(resolve_with_system(LanguageSetting::Ja, Lang::En), Lang::Ja);
        assert_eq!(resolve_with_system(LanguageSetting::En, Lang::Ja), Lang::En);
    }
}
