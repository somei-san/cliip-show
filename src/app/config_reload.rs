use std::path::Path;

use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_foundation::NSString;

use crate::config::{
    apply_config_file, apply_env_overrides, default_display_settings, load_config_file,
    start_at_login_from_config, DisplaySettings,
};
use crate::error::AppError;
use crate::hud::{hud_background_rgba, hud_border_white_alpha};
use crate::i18n;

use super::AppState;

// poll_pasteboard が呼ばれるたびにカウントし、この回数ごとに mtime チェックを行う
// デフォルト poll_interval_secs=0.3 × 10 = 約3秒ごと
const CONFIG_CHECK_EVERY_N_POLLS: u32 = 10;

/// 設定ファイルを1回読み込み、既定値・環境変数オーバーライドを適用した `DisplaySettings` と
/// `start_at_login`（`[startup]` 節が無い、またはキーが無ければ `None`）の両方を導く。
/// ファイル監視の再読み込み（`reload_config_if_changed`）とウィンドウを閉じたときの
/// 再読み込み（`window_will_close`）の両方から使う。読み込みを1回にまとめているのは、
/// 2回読むと1回目と2回目の間の外部変更を取りこぼしうるため。
pub(crate) fn resolved_config_from_file(
    path: &Path,
) -> Result<(DisplaySettings, Option<bool>), AppError> {
    let (config, _) = load_config_file(path)?;
    let base = default_display_settings();
    let settings = apply_env_overrides(apply_config_file(base, &config));
    Ok((settings, start_at_login_from_config(&config)))
}

/// `resolved_config_from_file` のうち `DisplaySettings` だけを使う呼び出し元向け。
pub(crate) fn display_settings_from_file(path: &Path) -> Result<DisplaySettings, AppError> {
    resolved_config_from_file(path).map(|(settings, _)| settings)
}

pub(super) unsafe fn reload_config_if_changed(state: &mut AppState) {
    let Some(path) = state.config_path.clone() else {
        return;
    };
    state.config_check_counter += 1;
    if state.config_check_counter < CONFIG_CHECK_EVERY_N_POLLS {
        return;
    }
    state.config_check_counter = 0;
    let current_mtime = std::fs::metadata(&path)
        .ok()
        .and_then(|m| m.modified().ok());
    if current_mtime == state.config_mtime {
        return;
    }
    let (new_settings, start_at_login) = match resolved_config_from_file(&path) {
        Ok(result) => result,
        Err(err) => {
            eprintln!("warning: config reload failed, keeping current settings: {err}");
            return;
        }
    };
    state.config_mtime = current_mtime;

    apply_settings_now(state, new_settings);
    apply_start_at_login_if_changed(state, start_at_login);
    eprintln!("config reloaded");
}

/// 手で設定ファイルを書き換えたときも LaunchAgent へ反映する。設定ファイルを読み直す経路と
/// 保存する経路から使う。`desired` が `None`（`[startup]` 節が無い、または `start_at_login`
/// キーが無い）なら何もしない。
pub(crate) unsafe fn apply_start_at_login_if_changed(state: &mut AppState, desired: Option<bool>) {
    let Some(desired) = desired else {
        return;
    };
    if desired == state.start_at_login {
        return;
    }

    // 設定ファイルは既に新しい値を持っている（手で書き換えたのはここに来た理由）ため、
    // plist の操作が失敗しても state はその値に揃える。次回起動の
    // login_item::sync_plist_with_config が plist を設定に合わせ直す。
    if let Err(error) = crate::login_item::apply_desired_state(desired) {
        eprintln!("warning: {error}");
    }
    state.start_at_login = desired;
    crate::settings_window::sync_login_item_toggle(&state.settings_controls, desired);
}

/// 新しい設定を state に反映する。ファイル監視の再読み込み・設定ウィンドウからの変更の両方から呼ばれる。
///
/// # Safety
/// - `APP_STATE` をロックしないこと（呼び出し側が既にロックを保持している）。
/// - AppKit のメインスレッドから呼ぶこと。
pub(crate) unsafe fn apply_settings_now(state: &mut AppState, new_settings: DisplaySettings) {
    // hud_emoji が変わったらアイコンラベルを即時更新
    if new_settings.hud_emoji != state.settings.hud_emoji {
        let emoji = NSString::from_str(&new_settings.hud_emoji);
        let () = msg_send![state.icon_label, setStringValue: &*emoji];
    }

    // hud_background_color・hud_background_opacity が変わったら背景レイヤーを即時更新
    let background_changed = new_settings.hud_background_color
        != state.settings.hud_background_color
        || new_settings.hud_background_opacity != state.settings.hud_background_opacity;
    if background_changed {
        let content_view: *mut AnyObject = msg_send![state.window, contentView];
        if content_view.is_null() {
            eprintln!("warning: contentView is null; skipping background color update");
        } else {
            let layer: *mut AnyObject = msg_send![content_view, layer];
            if layer.is_null() {
                eprintln!("warning: layer is null; skipping background color update");
            } else {
                let (r, g, b, a) = hud_background_rgba(
                    new_settings.hud_background_color,
                    new_settings.hud_background_opacity,
                );
                let bg: *mut AnyObject =
                    msg_send![class!(NSColor), colorWithCalibratedRed: r green: g blue: b alpha: a];
                let cg_color: *mut std::ffi::c_void = msg_send![bg, CGColor];
                let () = msg_send![layer, setBackgroundColor: cg_color];
                let (border_white, border_alpha) =
                    hud_border_white_alpha(new_settings.hud_background_color);
                let border_obj: *mut AnyObject = msg_send![class!(NSColor), colorWithCalibratedWhite: border_white alpha: border_alpha];
                let border_cg: *mut std::ffi::c_void = msg_send![border_obj, CGColor];
                let () = msg_send![layer, setBorderColor: border_cg];
            }
        }
    }

    // 間隔は NSTimer の生成時にしか渡せないため、変わったら張り替える
    let poll_changed =
        (new_settings.poll_interval_secs - state.settings.poll_interval_secs).abs() > 1e-9;
    if poll_changed {
        if !state.poll_timer.is_null() {
            let () = msg_send![state.poll_timer, invalidate];
        }
        state.poll_timer =
            super::schedule_poll_timer(state.delegate, new_settings.poll_interval_secs);
    }

    let language_changed = new_settings.language != state.settings.language;
    state.settings = new_settings;

    // language が変わったら設定ウィンドウとメニューの文言を即時更新。
    // ファイル監視の再読み込みと設定ウィンドウの言語ポップアップのどちらの経路もここを
    // 通るため、更新箇所は一箇所で済む。
    //
    // ここで触るのは非編集ラベルと NSButton/NSMenuItem/NSMenu/NSWindow のタイトルだけで
    // action もテキスト編集のデリゲート通知も発火しないため、APP_STATE のロックを
    // 保持したまま呼んでも再入は起きない。
    if language_changed {
        let lang = i18n::resolve(state.settings.language);
        crate::settings_window::apply_language(&state.settings_controls, lang);
        crate::menu::apply_language(&state.menu_handles, lang);
        // 外部から言語が変わったときはポップアップの選択も追随させる。文言の差し替えは
        // 選択位置を動かさないため、これが無いと選択だけ古い値を指したままになる。
        crate::settings_window::sync_language_popup(
            &state.settings_controls,
            state.settings.language,
        );
        // draft は「保存」で丸ごと書き戻されるため、古い言語で上書きしないようここでも揃える。
        state.settings_controls.draft.language = state.settings.language;
    }
}
