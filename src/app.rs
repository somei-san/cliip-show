use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{Mutex, Once};
use std::time::SystemTime;

use objc2::declare::ClassBuilder;
use objc2::runtime::{AnyClass, AnyObject, Sel};
use objc2::{class, msg_send, sel};
use objc2_foundation::NSSize;

use crate::config::{
    apply_config_file, apply_env_overrides, default_display_settings, display_settings,
    load_config_file, DisplaySettings,
};
use crate::error::AppError;
use crate::hud::{
    create_hud_window, fit_thumbnail_size, hud_background_rgba, hud_border_white_alpha, layout_hud,
    layout_hud_image,
};
use crate::objc_helpers::nsstring_from_str;
use crate::settings_window::SettingsControls;
use crate::text::truncate_text;

pub const FADE_TICK_INTERVAL_SECS: f64 = 1.0 / 60.0;

pub struct AppState {
    pub last_change_count: isize,
    pub pasteboard: *mut AnyObject,
    pub window: *mut AnyObject,
    pub icon_label: *mut AnyObject,
    pub label: *mut AnyObject,
    pub image_view: *mut AnyObject,
    pub hide_timer: *mut AnyObject,
    pub fade_timer: *mut AnyObject,
    pub fade_ticks_elapsed: u32,
    pub fade_total_ticks: u32,
    pub settings: DisplaySettings,
    pub config_path: Option<PathBuf>,
    pub config_mtime: Option<SystemTime>,
    pub config_check_counter: u32,
    pub paused: bool,
    pub status_item: *mut AnyObject,
    pub pause_menu_item: *mut AnyObject,
    pub settings_controls: SettingsControls,
    pub poll_timer: *mut AnyObject,
    /// タイマーの張り替えに使う AppDelegate。アプリの生存期間中有効。
    pub delegate: *mut AnyObject,
}

// SAFETY: AppState は APP_STATE の Mutex で排他制御されており、
// 実際の AppKit 操作はすべてメインスレッドのタイマーコールバック内でのみ行われる。
// Mutex<Option<AppState>> が Sync であるためにはラップする型が Send である必要があり、
// ここで明示的に実装する。
unsafe impl Send for AppState {}

pub static APP_STATE: Mutex<Option<AppState>> = Mutex::new(None);

pub fn get_delegate_class() -> &'static AnyClass {
    static ONCE: Once = Once::new();
    static mut CLASS: *const AnyClass = ptr::null();

    ONCE.call_once(|| unsafe {
        let mut builder = ClassBuilder::new("ClipboardHudAppDelegate", class!(NSObject))
            .expect("delegate class creation failed");

        builder.add_method(
            sel!(applicationDidFinishLaunching:),
            application_did_finish_launching as extern "C" fn(_, _, _),
        );
        builder.add_method(
            sel!(pollPasteboard:),
            poll_pasteboard as extern "C" fn(_, _, _),
        );
        builder.add_method(sel!(hideHud:), hide_hud as extern "C" fn(_, _, _));
        builder.add_method(sel!(fadeTick:), fade_tick as extern "C" fn(_, _, _));
        builder.add_method(sel!(togglePause:), toggle_pause as extern "C" fn(_, _, _));
        builder.add_method(sel!(quitApp:), quit_app as extern "C" fn(_, _, _));
        builder.add_method(sel!(openSettings:), open_settings as extern "C" fn(_, _, _));
        builder.add_method(
            sel!(settingChanged:),
            setting_changed as extern "C" fn(_, _, _),
        );
        builder.add_method(
            sel!(resetSettings:),
            reset_settings as extern "C" fn(_, _, _),
        );
        builder.add_method(
            sel!(previewSettings:),
            preview_settings as extern "C" fn(_, _, _),
        );
        builder.add_method(sel!(saveSettings:), save_settings as extern "C" fn(_, _, _));
        builder.add_method(
            sel!(windowWillClose:),
            window_will_close as extern "C" fn(_, _, _),
        );
        builder.add_method(
            sel!(controlTextDidChange:),
            control_text_did_change as extern "C" fn(_, _, _),
        );

        let class = builder.register();
        CLASS = class as *const AnyClass;
    });

    unsafe { &*CLASS }
}

extern "C" fn application_did_finish_launching(this: &AnyObject, _: Sel, _: *mut AnyObject) {
    unsafe {
        // メインメニューを持たないと、設定ウィンドウ等のテキストフィールドで Cmd+V 等の
        // 標準ショートカットが responder chain に届かない
        crate::menu::install_main_menu();

        let settings = display_settings();
        let pasteboard: *mut AnyObject = msg_send![class!(NSPasteboard), generalPasteboard];
        let last_change_count: isize = msg_send![pasteboard, changeCount];

        let (window, icon_label, label, image_view) = create_hud_window(settings.clone());
        if window.is_null() {
            eprintln!("fatal: HUD ウィンドウの作成に失敗しました");
            std::process::exit(1);
        }

        let (status_item, pause_menu_item) = crate::menu::create_status_item(this);

        // パスが解決できない場合もパスだけは保持し、後でファイルが作成されても検知できるようにする
        let config_path = crate::config::config_file_path().ok();
        let config_mtime = config_path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok());

        let poll_timer = schedule_poll_timer(this, settings.poll_interval_secs);

        // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
        *APP_STATE.lock().expect("APP_STATE lock poisoned") = Some(AppState {
            last_change_count,
            pasteboard,
            window,
            icon_label,
            label,
            image_view,
            hide_timer: ptr::null_mut(),
            fade_timer: ptr::null_mut(),
            fade_ticks_elapsed: 0,
            fade_total_ticks: 0,
            settings,
            config_path,
            config_mtime,
            config_check_counter: 0,
            paused: false,
            status_item,
            pause_menu_item,
            settings_controls: SettingsControls::default(),
            poll_timer,
            delegate: this as *const AnyObject as *mut AnyObject,
        });
    }
}

/// ペーストボード監視のタイマーを張る。
///
/// # Safety
/// AppKit のメインスレッドから呼ぶこと。
unsafe fn schedule_poll_timer(delegate: *const AnyObject, interval: f64) -> *mut AnyObject {
    msg_send![
        class!(NSTimer),
        scheduledTimerWithTimeInterval: interval
        target: delegate
        selector: sel!(pollPasteboard:)
        userInfo: ptr::null_mut::<AnyObject>()
        repeats: true
    ]
}

// poll_pasteboard が呼ばれるたびにカウントし、この回数ごとに mtime チェックを行う
// デフォルト poll_interval_secs=0.3 × 10 = 約3秒ごと
const CONFIG_CHECK_EVERY_N_POLLS: u32 = 10;

/// 設定ファイルを読み込み、既定値・環境変数オーバーライドを適用した `DisplaySettings` を返す。
/// ファイル監視の再読み込み（`reload_config_if_changed`）とウィンドウを閉じたときの
/// 再読み込み（`window_will_close`）の両方から使う。
fn display_settings_from_file(path: &Path) -> Result<DisplaySettings, AppError> {
    let (config, _) = load_config_file(path)?;
    let base = default_display_settings();
    Ok(apply_env_overrides(apply_config_file(base, &config)))
}

unsafe fn reload_config_if_changed(state: &mut AppState) {
    let Some(ref path) = state.config_path else {
        return;
    };
    state.config_check_counter += 1;
    if state.config_check_counter < CONFIG_CHECK_EVERY_N_POLLS {
        return;
    }
    state.config_check_counter = 0;
    let current_mtime = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());
    if current_mtime == state.config_mtime {
        return;
    }
    let new_settings = match display_settings_from_file(path) {
        Ok(settings) => settings,
        Err(err) => {
            eprintln!("warning: config reload failed, keeping current settings: {err}");
            return;
        }
    };
    state.config_mtime = current_mtime;

    apply_settings_now(state, new_settings);
    eprintln!("config reloaded");
}

/// 新しい設定を state に反映する。ファイル監視の再読み込み・設定ウィンドウからの変更の両方から呼ばれる。
///
/// # Safety
/// - `APP_STATE` をロックしないこと（呼び出し側が既にロックを保持している）。
/// - AppKit のメインスレッドから呼ぶこと。
pub(crate) unsafe fn apply_settings_now(state: &mut AppState, new_settings: DisplaySettings) {
    // hud_emoji が変わったらアイコンラベルを即時更新
    if new_settings.hud_emoji != state.settings.hud_emoji {
        let emoji = nsstring_from_str(&new_settings.hud_emoji);
        let () = msg_send![state.icon_label, setStringValue: emoji];
        let () = msg_send![emoji, release];
    }

    // hud_background_color が変わったら背景レイヤーを即時更新
    if new_settings.hud_background_color != state.settings.hud_background_color {
        let content_view: *mut AnyObject = msg_send![state.window, contentView];
        if content_view.is_null() {
            eprintln!("warning: contentView が null です。背景色の更新をスキップします");
        } else {
            let layer: *mut AnyObject = msg_send![content_view, layer];
            if layer.is_null() {
                eprintln!("warning: layer が null です。背景色の更新をスキップします");
            } else {
                let (r, g, b, a) = hud_background_rgba(new_settings.hud_background_color);
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
        state.poll_timer = schedule_poll_timer(state.delegate, new_settings.poll_interval_secs);
    }

    state.settings = new_settings;
}

pub(crate) unsafe fn show_text_content(state: &mut AppState, text: &str) {
    let truncated = truncate_text(
        text,
        state.settings.truncate_max_width,
        state.settings.truncate_max_lines,
    );
    let message = nsstring_from_str(&truncated);
    let () = msg_send![state.label, setStringValue: message];
    let () = msg_send![message, release];

    let () = msg_send![state.image_view, setImage: ptr::null_mut::<AnyObject>()];
    let () = msg_send![state.image_view, setHidden: true];
    let () = msg_send![state.label, setHidden: false];

    layout_hud(
        state.window,
        state.icon_label,
        state.label,
        state.settings.clone(),
    );
}

/// ペーストボードに載っている画像を NSImage として取り出す。画像が無ければ null を返す。
///
/// PNG・TIFF・JPEG・file-url のいずれも `initWithPasteboard:` が処理するため、型ごとの分岐は持たない。
///
/// # Safety
/// - `pasteboard` は有効な NSPasteboard であること。
/// - 戻り値は所有権 +1。呼び出し側が `release` すること。
unsafe fn image_from_pasteboard(pasteboard: *mut AnyObject) -> *mut AnyObject {
    let image: *mut AnyObject = msg_send![class!(NSImage), alloc];
    msg_send![image, initWithPasteboard: pasteboard]
}

/// `image` を HUD の画像ビューにセットしてレイアウトを更新する共通処理。
/// 呼び出し側から所有権（+1）を引き継ぎ、ここで `release` する。
unsafe fn set_hud_image(state: &mut AppState, image: *mut AnyObject) {
    let size: NSSize = msg_send![image, size];
    let thumbnail = fit_thumbnail_size(
        size.width,
        size.height,
        state.settings.hud_image_max_height,
        state.settings.hud_scale,
        !state.settings.hud_emoji.is_empty(),
    );

    // NSImageView が retain するので、こちらが持っていた所有権は手放す
    let () = msg_send![state.image_view, setImage: image];
    let () = msg_send![image, release];

    let () = msg_send![state.label, setHidden: true];
    let () = msg_send![state.image_view, setHidden: false];

    layout_hud_image(
        state.window,
        state.icon_label,
        state.image_view,
        thumbnail,
        state.settings.clone(),
    );
}

/// クリップボードの画像を HUD にセットできたら `true` を返す。
unsafe fn show_image_content(state: &mut AppState) -> bool {
    let image = image_from_pasteboard(state.pasteboard);
    if image.is_null() {
        return false;
    }
    set_hud_image(state, image);
    true
}

/// 直接渡した NSImage を HUD にセットする。お試し表示のサンプル画像用（ペーストボードを経由しない）。
///
/// # Safety
/// - `image` は有効な NSImage への所有権 +1 のポインタであること（内部で `release` する）。
/// - AppKit のメインスレッドから呼ぶこと。
pub(crate) unsafe fn show_sample_image_content(state: &mut AppState, image: *mut AnyObject) {
    set_hud_image(state, image);
}

/// フェード中なら止めてアルファを戻し、HUD を最前面に出して自動非表示タイマーを再設定する。
/// `show_text_content`/`show_image_content` で内容をセットした後に呼ぶこと。
pub(crate) unsafe fn present_hud(this: &AnyObject, state: &mut AppState) {
    if !state.fade_timer.is_null() {
        let () = msg_send![state.fade_timer, invalidate];
        state.fade_timer = ptr::null_mut();
    }
    let () = msg_send![state.window, setAlphaValue: 1.0f64];

    let () = msg_send![state.window, orderFrontRegardless];

    if !state.hide_timer.is_null() {
        let () = msg_send![state.hide_timer, invalidate];
    }

    let hide_timer: *mut AnyObject = msg_send![
        class!(NSTimer),
        scheduledTimerWithTimeInterval: state.settings.hud_duration_secs
        target: this
        selector: sel!(hideHud:)
        userInfo: ptr::null_mut::<AnyObject>()
        repeats: false
    ];
    state.hide_timer = hide_timer;
}

/// 現在のペーストボード内容を HUD に表示する。表示できる内容（テキストまたは画像）が
/// 無ければ何もせず `false` を返す。
pub(crate) unsafe fn present_pasteboard_content(this: &AnyObject, state: &mut AppState) -> bool {
    let text_type = nsstring_from_str("public.utf8-plain-text");
    let raw_text: *mut AnyObject = msg_send![state.pasteboard, stringForType: text_type];
    let () = msg_send![text_type, release];

    // ブラウザや表計算からのコピーは画像とテキストの両方を載せるため、テキストを先に見る
    match crate::objc_helpers::nsstring_to_string(raw_text) {
        Some(text) => show_text_content(state, &text),
        None => {
            if !show_image_content(state) {
                return false;
            }
        }
    }

    present_hud(this, state);
    true
}

extern "C" fn poll_pasteboard(this: &AnyObject, _: Sel, _: *mut AnyObject) {
    unsafe {
        // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
        let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
        let Some(state) = guard.as_mut() else {
            return;
        };

        reload_config_if_changed(state);

        let change_count: isize = msg_send![state.pasteboard, changeCount];
        if change_count == state.last_change_count {
            return;
        }
        state.last_change_count = change_count;

        // 一時停止中も changeCount の追随だけは続ける。再開直後に古いコピー内容が
        // 表示されてしまうのを防ぐため。
        if state.paused {
            return;
        }

        present_pasteboard_content(this, state);
    }
}

extern "C" fn open_settings(this: &AnyObject, _: Sel, _: *mut AnyObject) {
    unsafe {
        let window = {
            // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
            let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
            let Some(state) = guard.as_mut() else {
                return;
            };

            if state.settings_controls.window.is_null() {
                state.settings_controls = crate::settings_window::build_settings_window(this);
            }
            // 下書きは開くたびに現在の実効設定へ合わせる。ファイル監視の再読み込み等で
            // state.settings が外部から変わっていても、開き直せば食い違わない。
            state.settings_controls.draft = state.settings.clone();
            crate::settings_window::sync_controls_from_settings(
                &state.settings_controls,
                &state.settings,
            );
            state.settings_controls.window
        };

        // accessory アプリはウィンドウを前面に出すだけではキー入力を受け取れない
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let () = msg_send![app, activateIgnoringOtherApps: true];
        let () = msg_send![window, makeKeyAndOrderFront: ptr::null_mut::<AnyObject>()];

        // AppKit は key view loop の先頭のテキストフィールドを自動で first responder に
        // するため、開いた直後に特定の入力欄が選択された状態になる。これを外す。
        // 確定に伴い settingChanged: が飛びうるので、ロックを手放してから呼ぶ。
        let _: bool = msg_send![window, makeFirstResponder: ptr::null_mut::<AnyObject>()];
    }
}

extern "C" fn setting_changed(_: &AnyObject, _: Sel, sender: *mut AnyObject) {
    unsafe {
        // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
        let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
        let Some(state) = guard.as_mut() else {
            return;
        };

        crate::settings_window::apply_setting_change(state, sender);
    }
}

extern "C" fn reset_settings(_: &AnyObject, _: Sel, _: *mut AnyObject) {
    unsafe {
        // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
        let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
        let Some(state) = guard.as_mut() else {
            return;
        };

        crate::settings_window::reset_settings(state);
    }
}

extern "C" fn preview_settings(this: &AnyObject, _: Sel, sender: *mut AnyObject) {
    unsafe {
        commit_pending_field_edit(view_window(sender));

        // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
        let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
        let Some(state) = guard.as_mut() else {
            return;
        };

        crate::settings_window::preview_settings(this, state);
    }
}

extern "C" fn save_settings(_: &AnyObject, _: Sel, sender: *mut AnyObject) {
    unsafe {
        commit_pending_field_edit(view_window(sender));

        let window = {
            // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
            let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
            let Some(state) = guard.as_mut() else {
                return;
            };
            if !crate::settings_window::save_settings(state) {
                return;
            }
            state.settings_controls.window
        };

        // windowWillClose: が APP_STATE をロックするため、ガードを手放してから閉じる
        if !window.is_null() {
            let () = msg_send![window, performClose: ptr::null_mut::<AnyObject>()];
        }
    }
}

/// ボタン（`NSView`）が属する `NSWindow` を返す。`view` が null なら null を返す。
unsafe fn view_window(view: *mut AnyObject) -> *mut AnyObject {
    if view.is_null() {
        return ptr::null_mut();
    }
    msg_send![view, window]
}

/// 編集中のテキストフィールドがあれば確定させ、下書きに反映されていない入力を残さない。
/// `makeFirstResponder:` はフィールドの `settingChanged:` を同期的に発火させうるため、
/// `APP_STATE` をロックする前に呼ぶこと（ロック中に呼ぶと再入でハングしうる）。
unsafe fn commit_pending_field_edit(window: *mut AnyObject) {
    if window.is_null() {
        return;
    }
    let _: bool = msg_send![window, makeFirstResponder: ptr::null_mut::<AnyObject>()];
}

/// 設定ウィンドウを閉じたときに呼ぶ。保存せずに「お試し表示」した場合、設定ファイルの内容と
/// HUD の挙動がずれたまま残るのを防ぐため、ファイルを読み直して適用し直す。
extern "C" fn window_will_close(_: &AnyObject, _: Sel, notification: *mut AnyObject) {
    unsafe {
        // 閉じる操作自体が編集中フィールドの確定を伴う場合があるため、ロック前に済ませておく
        let window: *mut AnyObject = if notification.is_null() {
            ptr::null_mut()
        } else {
            msg_send![notification, object]
        };
        commit_pending_field_edit(window);

        // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
        let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
        let Some(state) = guard.as_mut() else {
            return;
        };

        let Some(path) = state.config_path.clone() else {
            return;
        };
        let new_settings = match display_settings_from_file(&path) {
            Ok(settings) => settings,
            Err(err) => {
                eprintln!("warning: config reload failed, keeping current settings: {err}");
                return;
            }
        };
        apply_settings_now(state, new_settings);
        state.config_mtime = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok());
    }
}

/// 絵文字フィールドの入力中バリデーション表示を更新する。
///
/// `sync_controls_from_settings` 等がロック保持中にこのフィールドへ `setStringValue:` すると、
/// それがこの通知を同期的に誘発して再入しうる。`try_lock` で失敗時は何もせず戻ることで、
/// プログラム側の更新中の再入ハングを避ける（その場合は更新側が表示の同期を担う）。
extern "C" fn control_text_did_change(_: &AnyObject, _: Sel, notification: *mut AnyObject) {
    unsafe {
        let Ok(mut guard) = APP_STATE.try_lock() else {
            return;
        };
        let Some(state) = guard.as_mut() else {
            return;
        };

        let object: *mut AnyObject = if notification.is_null() {
            ptr::null_mut()
        } else {
            msg_send![notification, object]
        };
        if object != state.settings_controls.hud_emoji_field {
            return;
        }

        crate::settings_window::update_emoji_validation_message(state);
    }
}

extern "C" fn toggle_pause(_: &AnyObject, _: Sel, _: *mut AnyObject) {
    unsafe {
        // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
        let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
        let Some(state) = guard.as_mut() else {
            return;
        };

        state.paused = !state.paused;
        crate::menu::apply_paused_state(state.status_item, state.pause_menu_item, state.paused);
    }
}

extern "C" fn quit_app(_: &AnyObject, _: Sel, _: *mut AnyObject) {
    unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let () = msg_send![app, terminate: ptr::null_mut::<AnyObject>()];
    }
}

extern "C" fn hide_hud(this: &AnyObject, _: Sel, _: *mut AnyObject) {
    unsafe {
        // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
        let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
        let Some(state) = guard.as_mut() else {
            return;
        };

        if !state.hide_timer.is_null() {
            let () = msg_send![state.hide_timer, invalidate];
            state.hide_timer = ptr::null_mut();
        }

        let fade_duration = state.settings.hud_fade_duration_secs;
        if fade_duration <= 0.0 {
            // フェードなし: 即時非表示
            if !state.fade_timer.is_null() {
                let () = msg_send![state.fade_timer, invalidate];
                state.fade_timer = ptr::null_mut();
            }
            let () = msg_send![state.window, orderOut: ptr::null_mut::<AnyObject>()];
            return;
        }

        // フェードアウト開始
        let total_fade_ticks = (fade_duration / FADE_TICK_INTERVAL_SECS).ceil() as u32;
        state.fade_total_ticks = total_fade_ticks;
        if !state.fade_timer.is_null() {
            let () = msg_send![state.fade_timer, invalidate];
            state.fade_timer = ptr::null_mut();
        }
        state.fade_ticks_elapsed = 0;

        let fade_timer: *mut AnyObject = msg_send![
            class!(NSTimer),
            scheduledTimerWithTimeInterval: FADE_TICK_INTERVAL_SECS
            target: this
            selector: sel!(fadeTick:)
            userInfo: ptr::null_mut::<AnyObject>()
            repeats: true
        ];
        state.fade_timer = fade_timer;
    }
}

extern "C" fn fade_tick(_: &AnyObject, _: Sel, timer: *mut AnyObject) {
    unsafe {
        // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
        let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
        let Some(state) = guard.as_mut() else {
            let () = msg_send![timer, invalidate];
            return;
        };

        let window = state.window;
        state.fade_ticks_elapsed += 1;

        if state.fade_ticks_elapsed >= state.fade_total_ticks {
            debug_assert!(!state.fade_timer.is_null());
            let () = msg_send![timer, invalidate];
            state.fade_timer = ptr::null_mut();
            drop(guard);

            let () = msg_send![window, setAlphaValue: 0.0f64];
            let () = msg_send![window, orderOut: ptr::null_mut::<AnyObject>()];
            let () = msg_send![window, setAlphaValue: 1.0f64];
        } else {
            let alpha = 1.0 - (state.fade_ticks_elapsed as f64 / state.fade_total_ticks as f64);
            drop(guard);
            let () = msg_send![window, setAlphaValue: alpha];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::image_from_pasteboard;
    use super::FADE_TICK_INTERVAL_SECS;
    use crate::config::DEFAULT_HUD_FADE_DURATION_SECS;
    use crate::objc_helpers::nsstring_from_str;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use objc2_foundation::NSSize;
    use std::ptr;

    /// 一般ペーストボード（ユーザーのクリップボード）を汚さないよう、専用の名前付きペーストボードを使う。
    #[test]
    fn image_from_pasteboard_reads_png_and_ignores_plain_text() {
        unsafe {
            let pasteboard: *mut AnyObject =
                msg_send![class!(NSPasteboard), pasteboardWithUniqueName];
            assert!(!pasteboard.is_null());

            let text_type = nsstring_from_str("public.utf8-plain-text");
            let types: *mut AnyObject = msg_send![class!(NSArray), arrayWithObject: text_type];
            let _: isize =
                msg_send![pasteboard, declareTypes: types owner: ptr::null_mut::<AnyObject>()];
            let text = nsstring_from_str("no image here");
            let _: bool = msg_send![pasteboard, setString: text forType: text_type];

            let no_image = image_from_pasteboard(pasteboard);
            assert!(
                no_image.is_null(),
                "text-only pasteboard must yield no image"
            );
            let () = msg_send![text, release];
            let () = msg_send![text_type, release];

            let png_type = nsstring_from_str("public.png");
            let types: *mut AnyObject = msg_send![class!(NSArray), arrayWithObject: png_type];
            let _: isize =
                msg_send![pasteboard, declareTypes: types owner: ptr::null_mut::<AnyObject>()];
            let png_path = nsstring_from_str(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/visual/baseline/image_tiny.png"
            ));
            let png_data: *mut AnyObject =
                msg_send![class!(NSData), dataWithContentsOfFile: png_path];
            let () = msg_send![png_path, release];
            assert!(!png_data.is_null(), "fixture PNG must be readable");
            let _: bool = msg_send![pasteboard, setData: png_data forType: png_type];
            let () = msg_send![png_type, release];

            let image = image_from_pasteboard(pasteboard);
            assert!(!image.is_null(), "PNG pasteboard must yield an image");
            let size: NSSize = msg_send![image, size];
            assert!(size.width > 0.0 && size.height > 0.0);
            let () = msg_send![image, release];

            let () = msg_send![pasteboard, releaseGlobally];
        }
    }

    #[test]
    fn fade_total_ticks_calculation_is_exact() {
        // fade_duration=DEFAULT_HUD_FADE_DURATION_SECS, FADE_TICK_INTERVAL_SECS=1/60 → 18 ticks
        let total = (DEFAULT_HUD_FADE_DURATION_SECS / FADE_TICK_INTERVAL_SECS).ceil() as u32;
        assert_eq!(total, 18);
    }

    #[test]
    fn fade_alpha_is_positive_at_penultimate_tick() {
        let total: u32 = 18;
        let elapsed: u32 = total - 1;
        let alpha = 1.0 - (elapsed as f64 / total as f64);
        assert!(alpha > 0.0, "alpha should be > 0.0, got {}", alpha);
        assert!(
            (alpha - 1.0 / total as f64).abs() < 1e-10,
            "alpha should be approximately 1/total={}, got {}",
            1.0 / total as f64,
            alpha
        );
    }

    #[test]
    fn fade_alpha_is_half_at_midpoint() {
        let total: u32 = 18;
        let elapsed: u32 = 9;
        let alpha = 1.0 - (elapsed as f64 / total as f64);
        assert!((alpha - 0.5).abs() < 1e-10, "alpha={}", alpha);
    }
}
