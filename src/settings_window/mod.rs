mod build;
mod input_filter;
mod range_hint;
mod rows;
mod sync;
mod tooltip;

use std::ptr;

use objc2::runtime::AnyObject;

use crate::config::{default_display_settings, ConfigKey, DisplaySettings};
use crate::i18n::Msg;

pub use build::build_settings_window;
pub(crate) use build::{TAB_INDEX_SETTINGS, TAB_INDEX_SUPPORT};
pub use sync::{
    apply_language, apply_setting_change, handle_text_change, preview_settings, reset_settings,
    save_settings, save_show_menu_bar_icon, sync_controls_from_settings, sync_language_popup,
    sync_login_item_toggle, sync_menu_bar_icon_toggle,
};

/// 言語切り替えで表示テキストを差し替える対象コントロール。
pub struct LocalizedControl {
    pub control: *mut AnyObject,
    pub msg: Msg,
    pub kind: LocalizedKind,
}

pub enum LocalizedKind {
    /// NSTextField のラベル。`setStringValue:` で差し替える。
    StringValue,
    /// NSButton / NSMenuItem / NSMenu / NSWindow のタイトル。`setTitle:` で差し替える。
    Title,
    /// NSTabViewItem のタブ名。`NSControl` ではないので `setLabel:` を使う。
    TabLabel,
    /// コントロールのツールチップ。`setToolTip:` で差し替える。
    ToolTip,
}

/// `apply_setting_change` が「数値欄の範囲ヒント popover を出すべき」と判断したときに返す情報。
/// popover 自体の表示は `APP_STATE` のロックを手放してから行うため、ロック中に確定できる
/// この2つ（どこに・何を出すか）だけを運ぶ。
pub struct RangeHint {
    /// popover を表示する基準ビュー（値を入力したフィールド）。
    pub anchor: *mut AnyObject,
    /// popover 本文（言語・接頭辞込みで解決済み）。
    pub text: String,
}

/// `build_settings_window` へ渡す、下書きモデルの対象外なトグル行の初期表示値。
/// bool を並べて渡すと引数の取り違えでもコンパイルが通ってしまうため、
/// 意味のある名前を持つ struct にまとめる。
pub struct InitialToggles {
    pub start_at_login: bool,
    pub show_menu_bar_icon: bool,
}
/// 設定ウィンドウを構成するコントロールへのポインタ。`AppState` に保持し、
/// `openSettings:` で使い回す（ウィンドウは初回だけ生成する）。
pub struct SettingsControls {
    pub window: *mut AnyObject,
    /// 行スタック全体（`NSScrollView` の documentView）。VRT が可視域の外の行まで撮るために持つ
    /// （`render_settings_png` の `SettingsRows`）。
    pub document_view: *mut AnyObject,
    pub poll_interval_slider: *mut AnyObject,
    pub poll_interval_value_label: *mut AnyObject,
    pub hud_duration_slider: *mut AnyObject,
    pub hud_duration_value_label: *mut AnyObject,
    pub hud_fade_duration_slider: *mut AnyObject,
    pub hud_fade_duration_value_label: *mut AnyObject,
    pub hud_scale_slider: *mut AnyObject,
    pub hud_scale_value_label: *mut AnyObject,
    pub max_chars_per_line_field: *mut AnyObject,
    pub max_chars_per_line_stepper: *mut AnyObject,
    pub max_lines_field: *mut AnyObject,
    pub max_lines_stepper: *mut AnyObject,
    pub hud_image_max_height_field: *mut AnyObject,
    pub hud_image_max_height_stepper: *mut AnyObject,
    pub hud_position_popup: *mut AnyObject,
    pub hud_background_color_popup: *mut AnyObject,
    pub hud_background_opacity_slider: *mut AnyObject,
    pub hud_background_opacity_value_label: *mut AnyObject,
    pub hud_emoji_field: *mut AnyObject,
    /// 絵文字欄の右隣のヘルプボタン（`?`）がクリックされたときに出す `NSPopover`。
    /// アプリの生存期間中保持し、`showEmojiHelp:` が使い回す。中身（`hud_emoji_help_label`）は
    /// popover の contentViewController.view が所有しており、popover 自体を手放さない限り
    /// 生存するため、ここでは追加の retain も release も行わない。
    pub hud_emoji_help_popover: *mut AnyObject,
    /// popover 本文のラベル。`apply_language` が言語切替のたびに文言を差し替える。
    pub hud_emoji_help_label: *mut AnyObject,
    /// 数値欄（3つ）が共用する範囲ヒント popover。値が revert・クランプされたときだけ
    /// `RangeHint` 経由で表示する。所有権の考え方は `hud_emoji_help_popover` と同じ
    /// （アプリの生存期間中保持し、release しない）。
    pub range_hint_popover: *mut AnyObject,
    /// 範囲ヒント popover の本文ラベル。表示のたびに `range_hint::range_hint_text` の
    /// 結果へ差し替える（3つの数値欄で使い回すため、固定文言ではない）。
    pub range_hint_label: *mut AnyObject,
    /// 範囲ヒント popover の自動 close タイマー。表示のたびに前回分を `invalidate` して
    /// 張り直す（`present_hud` の `hide_timer` と同じ考え方）。
    pub range_hint_close_timer: *mut AnyObject,
    /// 絵文字フィールドに最後に書き込んだ妥当な内容。`hud_emoji_field` への書き込みは
    /// `sync.rs` の `set_emoji_field` に一本化されており、これを通せば食い違いは起きない
    /// （直書きしないこと）。不正な入力を検出したときフィールドを丸ごと戻す先として使う
    /// （`handle_text_change`）。
    pub hud_emoji_shadow: String,
    /// 表示言語のポップアップ。他の設定行と違い下書き→保存のモデルには乗らず、選択した瞬間に
    /// 設定ファイルへ保存する（`apply_language_setting_change`）。
    pub language_popup: *mut AnyObject,
    /// ログイン時の自動起動トグル。下書き→保存のモデルには乗らず、チェックした瞬間に
    /// 設定ファイルへ保存し、`login_item::enable`/`disable` で plist をそれに合わせる
    /// （`toggleLoginItem:`）。
    pub login_item_toggle: *mut AnyObject,
    /// メニューバーアイコン表示のトグル。下書き→保存のモデルには乗らず、チェックした瞬間に
    /// 設定ファイルへ保存し、`menu::apply_menu_bar_icon_visibility` で status item をそれに
    /// 合わせる（`toggleMenuBarIcon:`）。
    pub show_menu_bar_icon_toggle: *mut AnyObject,
    /// ウィンドウ内で編集中の下書き。「保存」（`saveSettings:`）を押すまで設定ファイルには
    /// 反映しない。`settingChanged:` はこの下書きだけを更新する。
    ///
    /// `language`・`show_menu_bar_icon` はこの下書きの対象外だが、フィールド自体は
    /// `DisplaySettings` の一部として常に持つ。「保存」時に古い値で上書きしないよう、
    /// 値が変わるたびに `apply_settings_now`／`reset_settings` の側でここも一緒に同期すること。
    pub draft: DisplaySettings,
    /// 「お試し表示」を押すたびに進める、次に表示するサンプルの番号（`PREVIEW_SAMPLES` を巡回）。
    pub preview_sample_index: usize,
    /// 言語切り替え（`apply_language`）で文言を差し替える対象の一覧。
    pub localized: Vec<LocalizedControl>,
}

impl Default for SettingsControls {
    fn default() -> Self {
        let draft = default_display_settings();
        Self {
            hud_emoji_shadow: draft.hud_emoji.clone(),
            draft,
            window: ptr::null_mut(),
            document_view: ptr::null_mut(),
            poll_interval_slider: ptr::null_mut(),
            poll_interval_value_label: ptr::null_mut(),
            hud_duration_slider: ptr::null_mut(),
            hud_duration_value_label: ptr::null_mut(),
            hud_fade_duration_slider: ptr::null_mut(),
            hud_fade_duration_value_label: ptr::null_mut(),
            hud_scale_slider: ptr::null_mut(),
            hud_scale_value_label: ptr::null_mut(),
            max_chars_per_line_field: ptr::null_mut(),
            max_chars_per_line_stepper: ptr::null_mut(),
            max_lines_field: ptr::null_mut(),
            max_lines_stepper: ptr::null_mut(),
            hud_image_max_height_field: ptr::null_mut(),
            hud_image_max_height_stepper: ptr::null_mut(),
            hud_position_popup: ptr::null_mut(),
            hud_background_color_popup: ptr::null_mut(),
            hud_background_opacity_slider: ptr::null_mut(),
            hud_background_opacity_value_label: ptr::null_mut(),
            hud_emoji_field: ptr::null_mut(),
            hud_emoji_help_popover: ptr::null_mut(),
            hud_emoji_help_label: ptr::null_mut(),
            range_hint_popover: ptr::null_mut(),
            range_hint_label: ptr::null_mut(),
            range_hint_close_timer: ptr::null_mut(),
            language_popup: ptr::null_mut(),
            login_item_toggle: ptr::null_mut(),
            show_menu_bar_icon_toggle: ptr::null_mut(),
            preview_sample_index: 0,
            localized: Vec::new(),
        }
    }
}
/// `ConfigKey` を `NSControl` の `tag` にエンコードする。
pub fn config_key_to_tag(key: ConfigKey) -> isize {
    match key {
        ConfigKey::PollIntervalSecs => 0,
        ConfigKey::HudDurationSecs => 1,
        ConfigKey::HudFadeDurationSecs => 2,
        ConfigKey::MaxCharsPerLine => 3,
        ConfigKey::MaxLines => 4,
        ConfigKey::HudPosition => 5,
        ConfigKey::HudScale => 6,
        ConfigKey::HudBackgroundColor => 7,
        ConfigKey::HudEmoji => 8,
        ConfigKey::HudImageMaxHeight => 9,
        ConfigKey::Language => 10,
        ConfigKey::HudBackgroundOpacity => 11,
        ConfigKey::StartAtLogin => 12,
        ConfigKey::ShowMenuBarIcon => 13,
    }
}

pub fn tag_to_config_key(tag: isize) -> Option<ConfigKey> {
    match tag {
        0 => Some(ConfigKey::PollIntervalSecs),
        1 => Some(ConfigKey::HudDurationSecs),
        2 => Some(ConfigKey::HudFadeDurationSecs),
        3 => Some(ConfigKey::MaxCharsPerLine),
        4 => Some(ConfigKey::MaxLines),
        5 => Some(ConfigKey::HudPosition),
        6 => Some(ConfigKey::HudScale),
        7 => Some(ConfigKey::HudBackgroundColor),
        8 => Some(ConfigKey::HudEmoji),
        9 => Some(ConfigKey::HudImageMaxHeight),
        10 => Some(ConfigKey::Language),
        11 => Some(ConfigKey::HudBackgroundOpacity),
        12 => Some(ConfigKey::StartAtLogin),
        13 => Some(ConfigKey::ShowMenuBarIcon),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::{config_key_to_tag, tag_to_config_key};
    use crate::config::ConfigKey;

    #[test]
    fn tag_round_trips_for_all_config_keys() {
        for key in ConfigKey::ALL {
            let tag = config_key_to_tag(key);
            assert_eq!(tag_to_config_key(tag), Some(key));
        }
    }

    #[test]
    fn tag_to_config_key_rejects_unknown_tags() {
        assert_eq!(tag_to_config_key(-1), None);
        assert_eq!(tag_to_config_key(14), None);
        assert_eq!(tag_to_config_key(9999), None);
    }
}
