mod build;
mod rows;
mod sync;

use std::ptr;

use objc2::runtime::AnyObject;

use crate::config::{default_display_settings, ConfigKey, DisplaySettings};
use crate::i18n::Msg;

pub use build::build_settings_window;
pub(crate) use build::{TAB_INDEX_SETTINGS, TAB_INDEX_SUPPORT};
pub use sync::{
    apply_language, apply_setting_change, preview_settings, reset_settings, save_settings,
    sync_controls_from_settings, sync_language_popup, sync_login_item_toggle,
    update_emoji_validation_message,
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
}
/// 設定ウィンドウを構成するコントロールへのポインタ。`AppState` に保持し、
/// `openSettings:` で使い回す（ウィンドウは初回だけ生成する）。
pub struct SettingsControls {
    pub window: *mut AnyObject,
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
    pub hud_emoji_field: *mut AnyObject,
    /// 絵文字フィールドの入力中バリデーションメッセージ。妥当なときは空文字。
    pub hud_emoji_message_label: *mut AnyObject,
    /// 表示言語のポップアップ。他の設定行と違い下書き→保存のモデルには乗らず、選択した瞬間に
    /// 設定ファイルへ保存する（`apply_language_setting_change`）。
    pub language_popup: *mut AnyObject,
    /// ログイン時の自動起動トグル。下書き→保存のモデルには乗らず、
    /// チェックした瞬間に `login_item::enable`/`disable` を呼ぶ（`toggleLoginItem:`）。
    pub login_item_toggle: *mut AnyObject,
    /// ウィンドウ内で編集中の下書き。「保存」（`saveSettings:`）を押すまで設定ファイルには
    /// 反映しない。`settingChanged:` はこの下書きだけを更新する。
    ///
    /// `language` はこの下書きの対象外だが、フィールド自体は `DisplaySettings` の一部として
    /// 常に持つ。「保存」時に古い言語で上書きしないよう、言語が変わるたびに
    /// `apply_settings_now`／`reset_settings` の側でここも一緒に同期すること。
    pub draft: DisplaySettings,
    /// 「お試し表示」を押すたびに進める、次に表示するサンプルの番号（`PREVIEW_SAMPLES` を巡回）。
    pub preview_sample_index: usize,
    /// 言語切り替え（`apply_language`）で文言を差し替える対象の一覧。
    pub localized: Vec<LocalizedControl>,
}

impl Default for SettingsControls {
    fn default() -> Self {
        Self {
            window: ptr::null_mut(),
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
            hud_emoji_field: ptr::null_mut(),
            hud_emoji_message_label: ptr::null_mut(),
            language_popup: ptr::null_mut(),
            login_item_toggle: ptr::null_mut(),
            draft: default_display_settings(),
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
        assert_eq!(tag_to_config_key(11), None);
        assert_eq!(tag_to_config_key(9999), None);
    }
}
