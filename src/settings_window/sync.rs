use objc2::msg_send;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSRange, NSString};

use crate::app::{
    apply_settings_now, apply_start_at_login_if_changed, present_hud, show_sample_image_content,
    show_text_content, AppState,
};
use crate::config::{
    apply_config_file, default_display_settings, hud_emoji_validation_error, load_config_file,
    merge_display_settings, save_config_file, set_config_value, settings_to_config_file,
    start_at_login_from_config, ConfigKey, DisplaySettings, LanguageSetting,
};
use crate::i18n::{self, Lang, Msg};
use crate::objc_helpers::nsstring_to_string;
use crate::png::create_preview_sample_image;

use super::input_filter::filter_digits;
use super::range_hint::range_hint_text;
use super::rows::{
    resize_help_popover_to_fit_label, select_language_item, select_popup_item, set_string_value,
    set_toggle_state,
};
use super::tooltip::tooltip_text;
use super::{tag_to_config_key, LocalizedKind, RangeHint, SettingsControls};

enum PreviewSample {
    ShortText,
    LongText,
    Image,
}

const PREVIEW_SAMPLES: [PreviewSample; 3] = [
    PreviewSample::ShortText,
    PreviewSample::LongText,
    PreviewSample::Image,
];

/// 既定の `max_chars_per_line`・`max_lines` で行内と行数の両方の切り詰めが起きる分量にしてある。
fn preview_long_text(lang: Lang) -> String {
    let mut lines = vec![i18n::preview_long_line(lang)];
    lines.extend((2..=7).map(|n| i18n::preview_numbered_line(lang, n)));
    lines.join("\n")
}

/// 言語ポップアップで選択されている設定値を返す。
unsafe fn selected_language_value(popup: *mut AnyObject) -> Option<LanguageSetting> {
    if popup.is_null() {
        return None;
    }
    let index: isize = msg_send![popup, indexOfSelectedItem];
    let index = usize::try_from(index).ok()?;
    i18n::LANGUAGE_CHOICES.get(index).copied()
}

/// 言語切り替えに合わせて選択肢のラベルを差し替える。選択位置は動かさない。
///
/// `setTitle:` は action を送らないので、この関数を `settingChanged:` の処理中に
/// 呼んでも再入は起きない。
unsafe fn retitle_language_popup(popup: *mut AnyObject, lang: Lang) {
    if popup.is_null() {
        return;
    }
    for (index, setting) in i18n::LANGUAGE_CHOICES.iter().enumerate() {
        let item: *mut AnyObject = msg_send![popup, itemAtIndex: index as isize];
        if item.is_null() {
            continue;
        }
        let ns = NSString::from_str(i18n::language_label(lang, *setting));
        let () = msg_send![item, setTitle: &*ns];
    }
}

unsafe fn sync_slider(slider: *mut AnyObject, value_label: *mut AnyObject, value: f64) {
    let () = msg_send![slider, setDoubleValue: value];
    set_string_value(value_label, &format!("{value:.2}"));
}

/// field・stepper 双方を `value` へ揃える。初期同期・確定後の再同期・拒否された値の
/// revert のいずれも同じ操作（両方へ書き戻す）なので共通化してある。確定後の再同期・
/// revert はどちらも `settingChanged:`（編集終了後にしか発火せず、Enter は保存ボタンに
/// 先取りされる）の処理中にしか呼ばないため、sender 自身のフィールドへ書いても
/// 編集中の内容を壊さない。
unsafe fn sync_stepper_field(field: *mut AnyObject, stepper: *mut AnyObject, value: usize) {
    let () = msg_send![stepper, setIntegerValue: value as isize];
    set_string_value(field, &value.to_string());
}

/// `hud_emoji_field` への書き込みを一本化する。不変条件（`SettingsControls::hud_emoji_shadow`
/// のドキュメント参照）: フィールドへ書く箇所は必ずこれを通し、shadow も一緒に更新する。
unsafe fn set_emoji_field(controls: &mut SettingsControls, value: &str) {
    set_string_value(controls.hud_emoji_field, value);
    controls.hud_emoji_shadow = value.to_string();
}

/// 現在の `DisplaySettings` の値をすべてのコントロールに反映する。
/// ウィンドウを開くたびに呼び、外部（ファイル監視等）での変更との食い違いを防ぐ。
///
/// # Safety
/// - `controls` は `build_settings_window` が返した有効なポインタを保持していること。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn sync_controls_from_settings(
    controls: &mut SettingsControls,
    settings: &DisplaySettings,
) {
    sync_slider(
        controls.poll_interval_slider,
        controls.poll_interval_value_label,
        settings.poll_interval_secs,
    );
    sync_slider(
        controls.hud_duration_slider,
        controls.hud_duration_value_label,
        settings.hud_duration_secs,
    );
    sync_slider(
        controls.hud_fade_duration_slider,
        controls.hud_fade_duration_value_label,
        settings.hud_fade_duration_secs,
    );
    sync_slider(
        controls.hud_scale_slider,
        controls.hud_scale_value_label,
        settings.hud_scale,
    );
    sync_slider(
        controls.hud_background_opacity_slider,
        controls.hud_background_opacity_value_label,
        settings.hud_background_opacity,
    );

    sync_stepper_field(
        controls.max_chars_per_line_field,
        controls.max_chars_per_line_stepper,
        settings.truncate_max_width,
    );
    sync_stepper_field(
        controls.max_lines_field,
        controls.max_lines_stepper,
        settings.truncate_max_lines,
    );
    sync_stepper_field(
        controls.hud_image_max_height_field,
        controls.hud_image_max_height_stepper,
        settings.hud_image_max_height,
    );

    select_popup_item(controls.hud_position_popup, settings.hud_position.as_str());
    select_popup_item(
        controls.hud_background_color_popup,
        settings.hud_background_color.as_str(),
    );
    select_language_item(controls.language_popup, settings.language);
    sync_menu_bar_icon_toggle(controls, settings.show_menu_bar_icon);

    set_emoji_field(controls, &settings.hud_emoji);
}

/// ログイン項目トグルの表示を `start_at_login` に合わせる。下書きモデルの対象外の
/// ため、ウィンドウを開くたび・ファイル監視の再読み込み・`toggleLoginItem:` が
/// `enable`/`disable` に失敗したときの巻き戻しの各経路で、呼び出し側から実効値を渡す。
///
/// # Safety
/// AppKit のメインスレッドから呼ぶこと。
pub unsafe fn sync_login_item_toggle(controls: &SettingsControls, start_at_login: bool) {
    set_toggle_state(controls.login_item_toggle, start_at_login);
}

/// メニューバーアイコン表示トグルの表示を `show_menu_bar_icon` に合わせる。下書きモデルの
/// 対象外のため、`sync_controls_from_settings`・ファイル監視の再読み込み・
/// `toggleMenuBarIcon:` が保存に失敗したときの巻き戻しの各経路から呼ぶ。
///
/// # Safety
/// AppKit のメインスレッドから呼ぶこと。
pub unsafe fn sync_menu_bar_icon_toggle(controls: &SettingsControls, show_menu_bar_icon: bool) {
    set_toggle_state(controls.show_menu_bar_icon_toggle, show_menu_bar_icon);
}

/// 言語ポップアップの選択を、いま効いている設定値に合わせる。
///
/// # Safety
/// AppKit のメインスレッドから呼ぶこと。
pub unsafe fn sync_language_popup(controls: &SettingsControls, setting: LanguageSetting) {
    select_language_item(controls.language_popup, setting);
}

/// `field`/`stepper` はペアで同じ値を表示する。どちらが `sender` でも、変更後の値を
/// もう一方に同期しつつ文字列として返す。
///
/// `sender == field` でパースできない場合（空欄・オーバーフロー等）は `None` を返す前に
/// フィールドを `draft_value` へ戻す。クランプではなく revert になるのは、キーストローク
/// 単位のフィルタ（`handle_text_change`）をすり抜けた入力（巨大数字ペースト等）を
/// 確定時点で下書きへ引き戻すための最終防衛線のため。
unsafe fn sync_paired_value(
    sender: *mut AnyObject,
    field: *mut AnyObject,
    stepper: *mut AnyObject,
    draft_value: usize,
) -> Option<String> {
    if sender == field {
        let raw: *mut AnyObject = msg_send![field, stringValue];
        let text = nsstring_to_string(raw).unwrap_or_default();
        let Ok(parsed) = text.trim().parse::<isize>() else {
            set_string_value(field, &draft_value.to_string());
            return None;
        };
        let () = msg_send![stepper, setIntegerValue: parsed];
        Some(parsed.to_string())
    } else {
        let value: isize = msg_send![stepper, integerValue];
        let text = value.to_string();
        set_string_value(field, &text);
        Some(text)
    }
}

/// `settingChanged:` の `sender` から現在値を文字列で読み出す。field と stepper のペアだけは
/// どちらが `sender` かで読み出し元が変わり、もう一方への同期も伴うため個別に扱う。
/// それ以外のコントロールは `sender` 自身が入力元なので `raw_value_for_key` と同じ読み方になる。
unsafe fn raw_value_for_control(
    key: ConfigKey,
    sender: *mut AnyObject,
    controls: &SettingsControls,
) -> Option<String> {
    match key {
        ConfigKey::MaxCharsPerLine => sync_paired_value(
            sender,
            controls.max_chars_per_line_field,
            controls.max_chars_per_line_stepper,
            controls.draft.truncate_max_width,
        ),
        ConfigKey::MaxLines => sync_paired_value(
            sender,
            controls.max_lines_field,
            controls.max_lines_stepper,
            controls.draft.truncate_max_lines,
        ),
        ConfigKey::HudImageMaxHeight => sync_paired_value(
            sender,
            controls.hud_image_max_height_field,
            controls.hud_image_max_height_stepper,
            controls.draft.hud_image_max_height,
        ),
        _ => raw_value_for_key(key, sender),
    }
}

/// 設定ウィンドウのコントロール変更を下書き（`SettingsControls::draft`）に反映するだけで、
/// ファイル保存も HUD への適用もしない。確定させるには「保存」（`saveSettings:`）を押す。
///
/// クランプ・バリデーションは `set_config_value` に委ねる。
///
/// 戻り値が `Some` のときは、数値欄の入力が revert・クランプされたことを示す。呼び出し側
/// （`setting_changed`）はロックを手放してから範囲ヒント popover を表示すること
/// （`RangeHint` のドキュメント参照）。
///
/// # Safety
/// - `APP_STATE` をロックしないこと（呼び出し側が既にロックを保持している）。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn apply_setting_change(
    state: &mut AppState,
    sender: *mut AnyObject,
) -> Option<RangeHint> {
    let tag: isize = msg_send![sender, tag];
    let key = tag_to_config_key(tag)?;

    // 言語は下書き（保存ボタンで確定するモデル）の対象外。選んだ瞬間に設定ファイルへ
    // 保存し、HUD・設定ウィンドウ・メニューへ即時反映する（自動起動トグルと同じ扱い）。
    if key == ConfigKey::Language {
        apply_language_setting_change(state, sender);
        return None;
    }

    let Some(raw_value) = raw_value_for_control(key, sender, &state.settings_controls) else {
        // 数値欄でパースに失敗したとき（フィールド表示は既に下書きへ戻している）。
        // それ以外の key の None は range_hint_for が対象外として弾く。
        return range_hint_for(&state.settings_controls, state.settings.language, key);
    };

    let mut config = settings_to_config_file(state.settings_controls.draft.clone());
    let warning = match set_config_value(&mut config, key, &raw_value) {
        Ok(warning) => warning,
        Err(error) => {
            eprintln!("warning: {error}");
            // 拒否された文字列がフィールドに残ると下書きと食い違うので、下書きの値に戻す。
            // setStringValue: は action を発火しないため settingChanged: の再入は起きない。
            match key {
                ConfigKey::HudEmoji => {
                    let draft_emoji = state.settings_controls.draft.hud_emoji.clone();
                    set_emoji_field(&mut state.settings_controls, &draft_emoji);
                }
                ConfigKey::MaxCharsPerLine => sync_stepper_field(
                    state.settings_controls.max_chars_per_line_field,
                    state.settings_controls.max_chars_per_line_stepper,
                    state.settings_controls.draft.truncate_max_width,
                ),
                ConfigKey::MaxLines => sync_stepper_field(
                    state.settings_controls.max_lines_field,
                    state.settings_controls.max_lines_stepper,
                    state.settings_controls.draft.truncate_max_lines,
                ),
                ConfigKey::HudImageMaxHeight => sync_stepper_field(
                    state.settings_controls.hud_image_max_height_field,
                    state.settings_controls.hud_image_max_height_stepper,
                    state.settings_controls.draft.hud_image_max_height,
                ),
                _ => {}
            }
            return None;
        }
    };
    // クランプが起きたときだけ範囲ヒントの対象にする。拒否（Err）はクランプではないため対象外
    let hint = if warning.is_some() {
        range_hint_for(&state.settings_controls, state.settings.language, key)
    } else {
        None
    };
    if let Some(warning) = &warning {
        eprintln!("warning: {warning}");
    }

    state.settings_controls.draft =
        apply_config_file(state.settings_controls.draft.clone(), &config);

    // クランプ・拒否された場合に備え、実際に反映された値でコントロール表示を揃える。
    // settingChanged: は編集終了後（フォーカス喪失か Enter。Enter は保存ボタンに先取り
    // される）にしか発火しないため、sender 自身のフィールドへ書き戻してよい。万一
    // 編集中に再入しても controlTextDidChange: 側は try_lock で弾かれる。
    resync_controls_after_apply(
        &state.settings_controls,
        &state.settings_controls.draft,
        key,
    );

    hint
}

/// `key` が範囲ヒント popover の対象（数値欄3つ）なら、案内文言と表示位置（対象フィールド）
/// を返す。対象外なら `None`。
unsafe fn range_hint_for(
    controls: &SettingsControls,
    language: LanguageSetting,
    key: ConfigKey,
) -> Option<RangeHint> {
    let msg = match key {
        ConfigKey::MaxCharsPerLine => Msg::LabelMaxCharsPerLine,
        ConfigKey::MaxLines => Msg::LabelMaxLines,
        ConfigKey::HudImageMaxHeight => Msg::LabelHudImageMaxHeight,
        _ => return None,
    };
    let anchor = control_for_key(controls, key);
    if anchor.is_null() {
        return None;
    }
    let lang = i18n::resolve(language);
    Some(RangeHint {
        anchor,
        text: range_hint_text(lang, msg),
    })
}

/// `controlTextDidChange:` の実処理。数値 3 欄は入力文字を数字だけへ絞り込み、絵文字欄は
/// 確定前の全文を検証して不正なら直前の妥当な内容（shadow）へ丸ごと戻す。
///
/// IME 変換中（`hasMarkedText`）はキーストローク単位の書き換えが合成中の文字を壊すため
/// 何もしない。変換確定時に同じ通知が改めて届くので、そこが最終防衛線になる。
///
/// # Safety
/// - `APP_STATE` のロックは呼び出し側が `try_lock` で取得済みであること。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn handle_text_change(state: &mut AppState, object: *mut AnyObject) {
    if object.is_null() || is_marked_text_active(object) {
        return;
    }
    let tag: isize = msg_send![object, tag];
    let Some(key) = tag_to_config_key(tag) else {
        return;
    };

    match key {
        ConfigKey::MaxCharsPerLine | ConfigKey::MaxLines | ConfigKey::HudImageMaxHeight => {
            filter_digit_field(object);
        }
        ConfigKey::HudEmoji => validate_emoji_field(state, object),
        _ => {}
    }
}

/// `field` の `currentEditor` が変換中の合成文字（marked text）を持っているか。
/// エディタが取れない（フィールドが編集中でない）場合は変換中ではないとみなす。
unsafe fn is_marked_text_active(field: *mut AnyObject) -> bool {
    let editor: *mut AnyObject = msg_send![field, currentEditor];
    if editor.is_null() {
        return false;
    }
    msg_send![editor, hasMarkedText]
}

/// `field` のフィールドエディタのキャレット位置（UTF-16 単位）。編集中でなければ `None`。
unsafe fn caret_position(field: *mut AnyObject) -> Option<usize> {
    let editor: *mut AnyObject = msg_send![field, currentEditor];
    if editor.is_null() {
        return None;
    }
    let range: NSRange = msg_send![editor, selectedRange];
    Some(range.location)
}

/// `field` のフィールドエディタのキャレットを `position`（UTF-16 単位）へ移す。
/// 編集中でなければ何もしない（`setStringValue:` 側の反映だけで足りる）。
///
/// `position` はエディタの実テキスト長（`[editor string] length`）へクランプする。
/// 絵文字欄で全選択 → 1 文字入力すると、直前に読んだ shadow の長さがエディタの
/// 現在の内容より長くなり、範囲外の `setSelectedRange:` になりうるため。
unsafe fn set_caret_position(field: *mut AnyObject, position: usize) {
    let editor: *mut AnyObject = msg_send![field, currentEditor];
    if editor.is_null() {
        return;
    }
    let text: *mut AnyObject = msg_send![editor, string];
    let length: usize = if text.is_null() {
        0
    } else {
        msg_send![text, length]
    };
    let range = NSRange {
        location: position.min(length),
        length: 0,
    };
    let () = msg_send![editor, setSelectedRange: range];
}

/// 数値 3 欄の入力を数字だけへ絞り込む。変化が無ければ `setStringValue:` を呼ばない
/// （キャレットを動かさないため）。
unsafe fn filter_digit_field(field: *mut AnyObject) {
    let raw: *mut AnyObject = msg_send![field, stringValue];
    let text = nsstring_to_string(raw).unwrap_or_default();
    let caret = caret_position(field).unwrap_or_else(|| text.encode_utf16().count());
    let (filtered, caret_out) = filter_digits(&text, caret);
    if filtered == text {
        return;
    }
    set_string_value(field, &filtered);
    set_caret_position(field, caret_out);
}

/// 絵文字欄の全文を検証する。妥当なら shadow を更新し、不正なら shadow の内容へ丸ごと戻す
/// （キーストローク単位の文字除去はキーキャップ・国旗・ZWJ 合成を壊すため行わない）。
unsafe fn validate_emoji_field(state: &mut AppState, field: *mut AnyObject) {
    let raw: *mut AnyObject = msg_send![field, stringValue];
    let text = nsstring_to_string(raw).unwrap_or_default();
    match hud_emoji_validation_error(&text) {
        None => state.settings_controls.hud_emoji_shadow = text,
        Some(_) => {
            let shadow = state.settings_controls.hud_emoji_shadow.clone();
            set_emoji_field(&mut state.settings_controls, &shadow);
            set_caret_position(field, shadow.encode_utf16().count());
        }
    }
}

/// 言語ポップアップの `settingChanged:` から呼ぶ。他の設定と違い下書きを経由せず、選択した
/// 瞬間に読み込み→検証→保存でファイルへ書き、`apply_settings_now` で HUD・設定ウィンドウ・
/// メニューへ即時反映する。
///
/// `apply_settings_now` の言語差分側で `draft.language` も一緒に同期する。ここで揃えておかないと
/// 「保存」ボタンが `draft`（開いた時点のスナップショット）を丸ごと書き戻す際に、今しがた
/// 選んだ言語を古い値で上書きしてしまう。
///
/// # Safety
/// - `APP_STATE` をロックしないこと（呼び出し側が既にロックを保持している）。
/// - AppKit のメインスレッドから呼ぶこと。
unsafe fn apply_language_setting_change(state: &mut AppState, sender: *mut AnyObject) {
    // 表示ラベル（「システムに合わせる」等）は設定値ではないので、選択位置から値を引く。
    let Some(selected) = selected_language_value(sender) else {
        return;
    };

    match save_language_setting(state, selected.as_str()) {
        Ok(new_settings) => apply_settings_now(state, new_settings),
        Err(error) => {
            eprintln!("warning: {error}");
            // 保存できなかったので、ポップアップの表示を実際に効いている値へ戻す。
            // 下書きを経由しないコントロールなので、放置すると適用されていない値を
            // 表示し続ける（ログイン項目トグルの失敗時と同じ扱い）。
            select_language_item(
                state.settings_controls.language_popup,
                state.settings.language,
            );
        }
    }
}

/// 言語設定を設定ファイルへ保存し、保存後のファイル内容から `DisplaySettings` を組み直す。
///
/// 組み直しに `display_settings_from_file` を使うのは、`language` だけ差し替えると
/// 外部エディタでの変更を取り込まないまま `config_mtime` だけ進んでしまい、
/// ファイル監視がその変更を二度と拾わなくなるため。`start_at_login` は読んだ値を
/// `apply_start_at_login_if_changed` へ渡して同じ理由で反映する。
///
/// 副作用として、ファイルに保存していない状態は捨てられる。「お試し表示」で下書きを
/// HUD に当てている最中に言語を切り替えると、HUD はファイルの値に戻る。
unsafe fn save_language_setting(
    state: &mut AppState,
    raw: &str,
) -> Result<DisplaySettings, String> {
    let Some(path) = state.config_path.clone() else {
        return Err("config path is not resolved; cannot save language setting".to_string());
    };
    let (mut config, _) = load_config_file(&path).map_err(|error| error.to_string())?;
    let start_at_login = start_at_login_from_config(&config);
    set_config_value(&mut config, ConfigKey::Language, raw).map_err(|error| error.to_string())?;
    save_config_file(&path, &config).map_err(|error| error.to_string())?;
    state.config_mtime = std::fs::metadata(&path)
        .ok()
        .and_then(|m| m.modified().ok());
    apply_start_at_login_if_changed(state, start_at_login);

    crate::app::display_settings_from_file(&path).map_err(|error| error.to_string())
}

/// メニューバーアイコン表示の設定を保存し、保存後のファイル内容から `DisplaySettings` を
/// 組み直す。`show_menu_bar_icon` だけ差し替えると、外部エディタでの変更を取り込まないまま
/// `config_mtime` だけ進み、ファイル監視がその変更を二度と拾わなくなるため、
/// `display_settings_from_file` でファイルから組み直す。`toggleMenuBarIcon:` から呼ぶ。
///
/// # Safety
/// - `APP_STATE` をロックしないこと（呼び出し側が既にロックを保持している）。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn save_show_menu_bar_icon(
    state: &mut AppState,
    value: bool,
) -> Result<DisplaySettings, String> {
    let Some(path) = state.config_path.clone() else {
        return Err(
            "config path is not resolved; cannot save show_menu_bar_icon setting".to_string(),
        );
    };
    let (mut config, _) = load_config_file(&path).map_err(|error| error.to_string())?;
    let start_at_login = start_at_login_from_config(&config);
    let raw = if value { "true" } else { "false" };
    set_config_value(&mut config, ConfigKey::ShowMenuBarIcon, raw)
        .map_err(|error| error.to_string())?;
    save_config_file(&path, &config).map_err(|error| error.to_string())?;
    state.config_mtime = std::fs::metadata(&path)
        .ok()
        .and_then(|m| m.modified().ok());
    apply_start_at_login_if_changed(state, start_at_login);

    crate::app::display_settings_from_file(&path).map_err(|error| error.to_string())
}

/// 言語切り替えで、設定ウィンドウ内の静的なラベル・タイトル・ツールチップをすべて差し替える。
///
/// `APP_STATE` のロックを保持したまま呼んでよい: `NSTextField` ラベル・`NSButton`/`NSWindow`
/// のタイトルに加え、編集可能なフィールド・スライダー・ステッパーへの `setToolTip:` も含めて
/// 触るが、いずれも action やテキスト編集のデリゲート通知（`controlTextDidChange:` 等）を
/// 発火しないため、Objective-C 側からの再入は起きない。
///
/// # Safety
/// AppKit のメインスレッドから呼ぶこと。
pub unsafe fn apply_language(controls: &SettingsControls, lang: Lang) {
    for entry in &controls.localized {
        match entry.kind {
            LocalizedKind::StringValue => {
                set_string_value(entry.control, i18n::text(lang, entry.msg))
            }
            LocalizedKind::Title => {
                let ns = NSString::from_str(i18n::text(lang, entry.msg));
                let () = msg_send![entry.control, setTitle: &*ns];
            }
            LocalizedKind::TabLabel => {
                let ns = NSString::from_str(i18n::text(lang, entry.msg));
                let () = msg_send![entry.control, setLabel: &*ns];
            }
            LocalizedKind::ToolTip => {
                let ns = NSString::from_str(&tooltip_text(lang, entry.msg));
                let () = msg_send![entry.control, setToolTip: &*ns];
            }
        }
    }
    // 「システムに合わせる」は表示言語で書き分けるため、選択肢のラベルも差し替える。
    retitle_language_popup(controls.language_popup, lang);

    // 絵文字ヘルプの popover 本文はツールチップと同じ文言（tooltip_text）を使うため、
    // 汎用ループ（StringValue は i18n::text 経由）には乗せずここで直接差し替える。
    // ウィンドウ未生成（ポインタが null）のときは何もしない。文言の行数は言語で変わるため、
    // 差し替えのたびに popover の大きさも実測し直す。
    if !controls.hud_emoji_help_label.is_null() {
        set_string_value(
            controls.hud_emoji_help_label,
            &tooltip_text(lang, Msg::TooltipHudEmoji),
        );
        resize_help_popover_to_fit_label(
            controls.hud_emoji_help_popover,
            controls.hud_emoji_help_label,
        );
    }
}

/// 下書きを既定値に戻し、コントロール表示を同期する。保存も HUD への適用もしない。
/// 言語・メニューバーアイコン表示は下書きモデルの対象外のため、既定値へは戻さず現在値を保つ。
///
/// # Safety
/// - `APP_STATE` をロックしないこと（呼び出し側が既にロックを保持している）。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn reset_settings(state: &mut AppState) {
    let language = state.settings_controls.draft.language;
    let show_menu_bar_icon = state.settings_controls.draft.show_menu_bar_icon;
    state.settings_controls.draft = default_display_settings();
    state.settings_controls.draft.language = language;
    state.settings_controls.draft.show_menu_bar_icon = show_menu_bar_icon;
    let draft = state.settings_controls.draft.clone();
    sync_controls_from_settings(&mut state.settings_controls, &draft);
}

/// 下書きを HUD に適用したうえで固定サンプルを表示する。クリップボードの内容には依存せず、
/// 押すたびに短文 → 長文 → 画像 → 短文 … と巡回する。設定の見た目を毎回同じ内容で
/// 比較できるようにするため、クリップボードは参照しない。ファイル保存はしない。
///
/// # Safety
/// - `APP_STATE` をロックしないこと（呼び出し側が既にロックを保持している）。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn preview_settings(this: &AnyObject, state: &mut AppState) {
    sync_draft_from_controls(&mut state.settings_controls);

    let draft = state.settings_controls.draft.clone();
    apply_settings_now(state, draft);

    let index = state.settings_controls.preview_sample_index;
    state.settings_controls.preview_sample_index = (index + 1) % PREVIEW_SAMPLES.len();

    let lang = i18n::resolve(state.settings.language);
    match PREVIEW_SAMPLES[index] {
        PreviewSample::ShortText => {
            show_text_content(state, i18n::text(lang, Msg::PreviewShortText))
        }
        PreviewSample::LongText => show_text_content(state, &preview_long_text(lang)),
        PreviewSample::Image => match create_preview_sample_image() {
            Ok(image) => show_sample_image_content(state, image),
            Err(error) => {
                eprintln!("warning: {error}");
                show_text_content(state, i18n::text(lang, Msg::PreviewShortText));
            }
        },
    }

    present_hud(this, state);
}

/// 下書きを設定ファイルへ保存し、HUD に適用する。`config_mtime` を保存後のファイルの mtime に
/// 更新し、直後のファイル監視ポーリングによる二重適用を防ぐ。
///
/// 保存するのは `display` セクションだけ。丸ごと上書きすると、下書きの対象外である
/// `start_at_login` が下書きの既定値で塗り潰される。既存の設定ファイルを読めなければ、
/// 読めなかったファイルを上書きして `start_at_login` 等を消さないよう保存を中止する。
///
/// # Safety
/// - `APP_STATE` をロックしないこと（呼び出し側が既にロックを保持している）。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn save_settings(state: &mut AppState) -> bool {
    sync_draft_from_controls(&mut state.settings_controls);

    let Some(path) = state.config_path.clone() else {
        eprintln!("warning: config path is not resolved; cannot save settings");
        return false;
    };

    let (config, _) = match load_config_file(&path) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("warning: {error}; not saving settings");
            return false;
        }
    };
    // 下書きの対象外のキーも、ファイルの手編集で新しい値を持っているかもしれない。
    // merge_display_settings が draft の display で上書きする前に読んでおく。
    let start_at_login = start_at_login_from_config(&config);
    let config = merge_display_settings(config, state.settings_controls.draft.clone());
    if let Err(error) = save_config_file(&path, &config) {
        eprintln!("warning: {error}");
        return false;
    }

    let draft = state.settings_controls.draft.clone();
    apply_settings_now(state, draft);
    apply_start_at_login_if_changed(state, start_at_login);
    state.config_mtime = std::fs::metadata(&path)
        .ok()
        .and_then(|m| m.modified().ok());
    true
}

/// 全キーのコントロールの現在値から下書きを組み直す。
///
/// NSTextField の action はフォーカスが外れるか Enter を押すまで飛ばず、その Enter は
/// デフォルトボタン（保存）に先取りされる。保存・お試し表示の直前にここで読み直すことで、
/// 未確定の入力の取りこぼしと、拒否された値がフィールドに残ることの両方を防ぐ。
unsafe fn sync_draft_from_controls(controls: &mut SettingsControls) {
    const KEYS: [ConfigKey; 11] = [
        ConfigKey::PollIntervalSecs,
        ConfigKey::HudDurationSecs,
        ConfigKey::HudFadeDurationSecs,
        ConfigKey::HudScale,
        ConfigKey::MaxCharsPerLine,
        ConfigKey::MaxLines,
        ConfigKey::HudImageMaxHeight,
        ConfigKey::HudPosition,
        ConfigKey::HudBackgroundColor,
        ConfigKey::HudBackgroundOpacity,
        ConfigKey::HudEmoji,
    ];

    let mut config = settings_to_config_file(controls.draft.clone());
    for key in KEYS {
        let control = control_for_key(controls, key);
        if control.is_null() {
            continue;
        }
        let Some(raw) = raw_value_for_key(key, control) else {
            continue;
        };
        if let Err(error) = set_config_value(&mut config, key, &raw) {
            eprintln!("warning: {error}");
        }
    }
    controls.draft = apply_config_file(controls.draft.clone(), &config);

    let draft = controls.draft.clone();
    sync_controls_from_settings(controls, &draft);
}

/// キーに対応する「値の入力元」となるコントロール。field と stepper の組は、利用者が直接
/// 打ち込める field を正とする。
fn control_for_key(controls: &SettingsControls, key: ConfigKey) -> *mut AnyObject {
    match key {
        ConfigKey::PollIntervalSecs => controls.poll_interval_slider,
        ConfigKey::HudDurationSecs => controls.hud_duration_slider,
        ConfigKey::HudFadeDurationSecs => controls.hud_fade_duration_slider,
        ConfigKey::HudScale => controls.hud_scale_slider,
        ConfigKey::MaxCharsPerLine => controls.max_chars_per_line_field,
        ConfigKey::MaxLines => controls.max_lines_field,
        ConfigKey::HudImageMaxHeight => controls.hud_image_max_height_field,
        ConfigKey::HudPosition => controls.hud_position_popup,
        ConfigKey::HudBackgroundColor => controls.hud_background_color_popup,
        ConfigKey::HudBackgroundOpacity => controls.hud_background_opacity_slider,
        ConfigKey::HudEmoji => controls.hud_emoji_field,
        ConfigKey::Language => controls.language_popup,
        ConfigKey::ShowMenuBarIcon => controls.show_menu_bar_icon_toggle,
        ConfigKey::StartAtLogin => controls.login_item_toggle,
    }
}

unsafe fn raw_value_for_key(key: ConfigKey, control: *mut AnyObject) -> Option<String> {
    match key {
        ConfigKey::PollIntervalSecs
        | ConfigKey::HudDurationSecs
        | ConfigKey::HudFadeDurationSecs
        | ConfigKey::HudScale
        | ConfigKey::HudBackgroundOpacity => {
            let value: f64 = msg_send![control, doubleValue];
            // スライダーは連続値を返すため、そのまま文字列化すると 1.0700000000000001 のような
            // 値が設定ファイルに残る。値ラベルの表示と同じ桁で丸めて揃える。
            Some(format!("{value:.2}"))
        }
        ConfigKey::HudPosition | ConfigKey::HudBackgroundColor => {
            let title: *mut AnyObject = msg_send![control, titleOfSelectedItem];
            nsstring_to_string(title)
        }
        // 言語は表示ラベルが設定値と別なので、タイトルではなく選択位置から引く。
        ConfigKey::Language => {
            selected_language_value(control).map(|value| value.as_str().to_string())
        }
        ConfigKey::MaxCharsPerLine
        | ConfigKey::MaxLines
        | ConfigKey::HudImageMaxHeight
        | ConfigKey::HudEmoji => {
            let value: *mut AnyObject = msg_send![control, stringValue];
            nsstring_to_string(value)
        }
        // 自動起動トグルは専用の toggleLoginItem: を持ち、この汎用パスからは呼ばれない
        ConfigKey::StartAtLogin => None,
        // メニューバーアイコン表示トグルも専用の toggleMenuBarIcon: を持ち、同じ理由で対象外
        ConfigKey::ShowMenuBarIcon => None,
    }
}

unsafe fn resync_controls_after_apply(
    controls: &SettingsControls,
    settings: &DisplaySettings,
    key: ConfigKey,
) {
    match key {
        ConfigKey::PollIntervalSecs => {
            set_string_value(
                controls.poll_interval_value_label,
                &format!("{:.2}", settings.poll_interval_secs),
            );
        }
        ConfigKey::HudDurationSecs => {
            set_string_value(
                controls.hud_duration_value_label,
                &format!("{:.2}", settings.hud_duration_secs),
            );
        }
        ConfigKey::HudFadeDurationSecs => {
            set_string_value(
                controls.hud_fade_duration_value_label,
                &format!("{:.2}", settings.hud_fade_duration_secs),
            );
        }
        ConfigKey::HudScale => {
            set_string_value(
                controls.hud_scale_value_label,
                &format!("{:.2}", settings.hud_scale),
            );
        }
        ConfigKey::HudBackgroundOpacity => {
            set_string_value(
                controls.hud_background_opacity_value_label,
                &format!("{:.2}", settings.hud_background_opacity),
            );
        }
        ConfigKey::MaxCharsPerLine => sync_stepper_field(
            controls.max_chars_per_line_field,
            controls.max_chars_per_line_stepper,
            settings.truncate_max_width,
        ),
        ConfigKey::MaxLines => sync_stepper_field(
            controls.max_lines_field,
            controls.max_lines_stepper,
            settings.truncate_max_lines,
        ),
        ConfigKey::HudImageMaxHeight => sync_stepper_field(
            controls.hud_image_max_height_field,
            controls.hud_image_max_height_stepper,
            settings.hud_image_max_height,
        ),
        // ポップアップの選択肢は常に妥当な値のみを取りうるためクランプが発生せず、再同期は不要
        ConfigKey::HudPosition | ConfigKey::HudBackgroundColor => {}
        // 拒否された入力は set_config_value のエラー側で戻すため、ここでは触らない。
        ConfigKey::HudEmoji => {}
        // 言語は下書きモデルの対象外で、保存の成否にかかわらず再同期する対象が無い。
        ConfigKey::Language => {}
        // 自動起動も下書きモデルの対象外（専用の toggleLoginItem: が設定ファイルへ保存し、
        // plist はその派生物として合わせる）。
        ConfigKey::StartAtLogin => {}
        // メニューバーアイコン表示も下書きモデルの対象外（専用の toggleMenuBarIcon: が
        // 設定ファイルへ保存し、status item の表示はその派生物として合わせる）。
        ConfigKey::ShowMenuBarIcon => {}
    }
}

#[cfg(test)]
mod tests {
    use objc2::{class, sel};

    /// `handle_text_change` が使う ObjC API のセレクタを突き合わせる。セレクタ名は文字列
    /// なのでコンパイルでは食い違いを検出できず、綴りを間違えると呼び出し時に
    /// unrecognized selector で落ちる。
    #[test]
    fn appkit_responds_to_text_editing_selectors() {
        assert!(class!(NSTextField).responds_to(sel!(currentEditor)));
        assert!(class!(NSTextView).responds_to(sel!(hasMarkedText)));
        assert!(class!(NSTextView).responds_to(sel!(selectedRange)));
        assert!(class!(NSTextView).responds_to(sel!(setSelectedRange:)));
        assert!(class!(NSTextView).responds_to(sel!(string)));
        assert!(class!(NSString).responds_to(sel!(length)));
    }

    /// 設定ウィンドウを一度も開いていないとトグルのコントロールは null。設定ファイルの
    /// 手編集はその状態でもトグルの同期まで来るため、null へ `setState:` を送らないこと。
    #[test]
    fn syncing_toggles_without_a_settings_window_does_not_message_nil() {
        let controls = super::SettingsControls::default();
        unsafe {
            super::sync_login_item_toggle(&controls, true);
            super::sync_menu_bar_icon_toggle(&controls, false);
        }
    }
}
