use std::path::PathBuf;
use std::ptr;
use std::sync::{Mutex, Once};
use std::time::SystemTime;

use objc2::declare::ClassBuilder;
use objc2::runtime::{AnyClass, AnyObject, Bool, Sel};
use objc2::{class, msg_send, sel};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::config::{display_settings, DisplaySettings};
use crate::hud::create_hud_window;
use crate::i18n;
use crate::menu::MenuHandles;
use crate::settings_window::SettingsControls;

mod config_reload;
mod hud_show;
mod panels;

pub(crate) use config_reload::{
    apply_settings_now, apply_start_at_login_if_changed, display_settings_from_file,
    resolved_config_from_file,
};
pub(crate) use hud_show::{present_hud, show_sample_image_content, show_text_content};

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
    /// ログイン時の自動起動の実効値。設定ファイルの `start_at_login` を正とし、
    /// 未設定なら plist の有無を初期値とする（`login_item::resolved_start_at_login`）。
    pub start_at_login: bool,
    pub config_path: Option<PathBuf>,
    pub config_mtime: Option<SystemTime>,
    pub config_check_counter: u32,
    pub paused: bool,
    pub menu_handles: MenuHandles,
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

static APP_STATE: Mutex<Option<AppState>> = Mutex::new(None);

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
            sel!(applicationShouldHandleReopen:hasVisibleWindows:),
            application_should_handle_reopen as extern "C" fn(_, _, _, _) -> Bool,
        );
        builder.add_method(
            sel!(pollPasteboard:),
            poll_pasteboard as extern "C" fn(_, _, _),
        );
        builder.add_method(sel!(hideHud:), hide_hud as extern "C" fn(_, _, _));
        builder.add_method(
            sel!(fadeTick:),
            hud_show::fade_tick as extern "C" fn(_, _, _),
        );
        builder.add_method(sel!(togglePause:), toggle_pause as extern "C" fn(_, _, _));
        builder.add_method(sel!(quitApp:), quit_app as extern "C" fn(_, _, _));
        builder.add_method(sel!(openSettings:), open_settings as extern "C" fn(_, _, _));
        builder.add_method(
            sel!(openSupportPage:),
            panels::open_support_page as extern "C" fn(_, _, _),
        );
        builder.add_method(
            sel!(showAboutPanel:),
            panels::show_about_panel as extern "C" fn(_, _, _),
        );
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
            sel!(toggleLoginItem:),
            toggle_login_item as extern "C" fn(_, _, _),
        );
        builder.add_method(
            sel!(showEmojiHelp:),
            show_emoji_help as extern "C" fn(_, _, _),
        );
        builder.add_method(
            sel!(closeRangeHintPopover:),
            close_range_hint_popover as extern "C" fn(_, _, _),
        );
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
        let settings = display_settings();
        // メインメニュー・メニューバーの初期表示言語は、起動時点の設定ファイルの言語設定から決める
        let lang = i18n::resolve(settings.language);

        // メインメニューを持たないと、設定ウィンドウ等のテキストフィールドで Cmd+V 等の
        // 標準ショートカットが responder chain に届かない
        let edit_handles = crate::menu::install_main_menu(lang);

        let pasteboard: *mut AnyObject = msg_send![class!(NSPasteboard), generalPasteboard];
        let last_change_count: isize = msg_send![pasteboard, changeCount];

        let (window, icon_label, label, image_view) = create_hud_window(settings.clone());
        if window.is_null() {
            eprintln!("fatal: failed to create HUD window");
            std::process::exit(1);
        }

        let status_handles = crate::menu::create_status_item(this, lang);
        let menu_handles = MenuHandles {
            status: status_handles,
            edit: edit_handles,
        };

        // パスが解決できない場合もパスだけは保持し、後でファイルが作成されても検知できるようにする
        let config_path = crate::config::config_file_path().ok();
        let config_mtime = config_path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok());
        // `login_item::sync_plist_with_config`（main）が既に設定ファイルへ書き戻し済みのはずだが、
        // その書き込みに失敗した場合に備えて plist の有無へのフォールバックも保つ。
        let start_at_login = config_path
            .as_ref()
            .and_then(|path| crate::config::load_config_file(path).ok())
            .map(|(config, _)| crate::login_item::resolved_start_at_login(&config))
            .unwrap_or_else(crate::login_item::is_enabled);

        let poll_timer = schedule_poll_timer(this, settings.poll_interval_secs);

        // Homebrew の LaunchAgent が残っていると、アプリ自身が管理する LaunchAgent と
        // 二重起動する。他者が置いたものを勝手に消さず、停止を促すだけに留める。
        if crate::login_item::homebrew_agent_path().is_some_and(|p| p.exists()) {
            eprintln!(
                "warning: Homebrew login item settings remain. Run `brew services stop cliip-show` to avoid running twice."
            );
        }

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
            start_at_login,
            config_path,
            config_mtime,
            config_check_counter: 0,
            paused: false,
            menu_handles,
            settings_controls: SettingsControls::default(),
            poll_timer,
            delegate: this as *const AnyObject as *mut AnyObject,
        });

        // runModal は実行ループを止めるため、他の経路がロック待ちで固まらないよう
        // APP_STATE のロックを手放した後（上の代入文で既に解放済み）に呼ぶ。
        panels::prompt_login_item_if_needed(lang, start_at_login);

        // 常駐アプリはウィンドウを持たないので、自分で起動したときは立ち上がった
        // ことが分からない。ログイン時の起動は本人が起動を待っていないため出さない。
        if !crate::login_item::started_at_login() {
            announce_launch(this, lang);
        }
    }
}

/// 起動したことを HUD で知らせる。
///
/// # Safety
/// AppKit のメインスレッドから、`APP_STATE` のロックを持たずに呼ぶこと。
unsafe fn announce_launch(this: &AnyObject, lang: i18n::Lang) {
    // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
    let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
    let Some(state) = guard.as_mut() else {
        return;
    };
    hud_show::show_text_content(state, i18n::text(lang, i18n::Msg::LaunchNotice));
    hud_show::present_hud(this, state);
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

extern "C" fn poll_pasteboard(this: &AnyObject, _: Sel, _: *mut AnyObject) {
    unsafe {
        // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
        let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
        let Some(state) = guard.as_mut() else {
            return;
        };

        config_reload::reload_config_if_changed(state);

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

        hud_show::present_pasteboard_content(this, state);
    }
}

/// Spotlight や Finder から起動し直したときに AppKit が呼ぶ。Launch Services が
/// 二重起動を止めるため、この経路では新しいプロセスは立ち上がらない。
///
/// 常駐アプリはウィンドウを持たないので、既定の復帰動作に任せると何も起きたように
/// 見えない。設定ウィンドウを開いて、起動済みであることを示す。`false` を返すのは
/// 復帰させるウィンドウが無いため。
extern "C" fn application_should_handle_reopen(
    this: &AnyObject,
    _: Sel,
    _: *mut AnyObject,
    _has_visible_windows: Bool,
) -> Bool {
    open_settings(this, sel!(openSettings:), ptr::null_mut());
    Bool::NO
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
                let lang = i18n::resolve(state.settings.language);
                state.settings_controls =
                    crate::settings_window::build_settings_window(this, lang, state.start_at_login);
            }

            // 既に開いているウィンドウは前面に出すだけにする。他のインスタンスの起動でも
            // ここへ来るため、下書きを作り直すと編集中の内容がユーザーの操作なしに消える。
            let already_visible: bool = msg_send![state.settings_controls.window, isVisible];
            if !already_visible {
                // 下書きは開くたびに現在の実効設定へ合わせる。ファイル監視の再読み込み等で
                // state.settings が外部から変わっていても、開き直せば食い違わない。
                state.settings_controls.draft = state.settings.clone();
                crate::settings_window::sync_controls_from_settings(
                    &mut state.settings_controls,
                    &state.settings,
                );
            }
            // ログイン項目は下書きモデルの対象外なので、下書きとは別に毎回同期する。
            crate::settings_window::sync_login_item_toggle(
                &state.settings_controls,
                state.start_at_login,
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
        let hint = {
            // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
            let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
            let Some(state) = guard.as_mut() else {
                return;
            };

            crate::settings_window::apply_setting_change(state, sender)
        };

        // 範囲ヒント popover の表示はロックを手放してから行う（show_range_hint_popover のドキュメント参照）
        if let Some(hint) = hint {
            show_range_hint_popover(hint.anchor, &hint.text);
        }
    }
}

/// ログイン項目トグルの `toggleLoginItem:`。他の設定と違い下書きを経由せず、
/// チェックした瞬間に設定ファイルへ保存し、`login_item::enable`/`disable` で OS の
/// 状態を変える。設定ファイルを正とするため、保存を plist の操作より先に行う。
extern "C" fn toggle_login_item(_: &AnyObject, _: Sel, sender: *mut AnyObject) {
    unsafe {
        // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
        let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
        let Some(state) = guard.as_mut() else {
            return;
        };

        let checked: isize = msg_send![sender, state];
        let desired = checked != 0;

        if !persist_start_at_login(state, desired) {
            // 失敗したのにチェックだけ付いた状態を避け、実際の状態に表示を戻す。
            // 警告は persist_start_at_login が出力済み。
            crate::settings_window::sync_login_item_toggle(
                &state.settings_controls,
                state.start_at_login,
            );
            return;
        }

        if let Err(error) = if desired {
            crate::login_item::enable()
        } else {
            crate::login_item::disable()
        } {
            // 設定ファイルは正しい値のまま。次回起動の login_item::sync_plist_with_config が
            // plist を設定に合わせ直すため、ここでは警告に留める。
            eprintln!("warning: {error}");
        }
    }
}

/// 自動起動の値を設定ファイルへ保存し、成功したら `state.start_at_login`／`config_mtime` を
/// 更新する。設定ファイルを正とするため、`login_item::enable`/`disable` より必ず先に呼ぶこと。
/// 保存に失敗したら state は変えず `false` を返す（呼び出し側は表示を実際の値へ戻すだけでよい）
/// （`toggleLoginItem:` と自動起動確認ダイアログの「有効にする」の両方から使う）。
fn persist_start_at_login(state: &mut AppState, value: bool) -> bool {
    let Some(path) = state.config_path.clone() else {
        eprintln!("warning: config path is not resolved; cannot save start_at_login");
        return false;
    };
    if let Err(error) = crate::login_item::save_start_at_login(&path, value) {
        eprintln!("warning: {error}");
        return false;
    }
    state.start_at_login = value;
    state.config_mtime = std::fs::metadata(&path)
        .ok()
        .and_then(|m| m.modified().ok());
    true
}

// NSRectEdge: maxX。ボタン/フィールドの右側に popover を出す（`show_emoji_help_popover`・
// `show_range_hint_popover` で共用）。
const NS_RECT_EDGE_MAX_X: usize = 2;

/// 絵文字欄のヘルプボタン（`?`）の `showEmojiHelp:`。クリックで popover を出す。
/// マウスオーバーでも同じ popover を出す（`settings_window::rows::emoji_help_mouse_entered`）
/// ため、実処理は共用の `show_emoji_help_popover` に切り出してある。
extern "C" fn show_emoji_help(_: &AnyObject, _: Sel, sender: *mut AnyObject) {
    unsafe {
        show_emoji_help_popover(sender);
    }
}

/// 絵文字欄のヘルプ popover を `anchor`（ヘルプボタン）の右側に出す。クリック
/// （`show_emoji_help`）とホバー（`mouseEntered:`）の両方から呼ぶ。
/// `SettingsControls::hud_emoji_help_popover` のドキュメント参照。
///
/// `showRelativeToRect:` は `APP_STATE` のロックを手放してから呼ぶこと。popover の表示は
/// 同期的に別のイベント（`settingChanged:` 等）を誘発することがあり、それらは `lock()` する
/// ため、ロック保持中に呼ぶとデッドロックする（`open_settings`/`save_settings` と同じ理由）。
///
/// # Safety
/// - `APP_STATE` をロックしないこと（内部で取得・解放する）。
/// - AppKit のメインスレッドから呼ぶこと。
pub(crate) unsafe fn show_emoji_help_popover(anchor: *mut AnyObject) {
    if anchor.is_null() {
        return;
    }

    let popover = {
        // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
        let guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
        let Some(state) = guard.as_ref() else {
            return;
        };
        state.settings_controls.hud_emoji_help_popover
    };
    if popover.is_null() {
        return;
    }

    // 空の矩形を渡すとボタンの bounds が使われる（NSPopover のドキュメント参照）
    let zero_rect = NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size: NSSize {
            width: 0.0,
            height: 0.0,
        },
    };
    let () = msg_send![
        popover,
        showRelativeToRect: zero_rect
        ofView: anchor
        preferredEdge: NS_RECT_EDGE_MAX_X
    ];
}

/// 絵文字欄のヘルプ popover を閉じる。`mouseExited:`（`settings_window::rows`）から呼ぶ。
///
/// # Safety
/// - `APP_STATE` をロックしないこと（内部で取得・解放する）。
/// - AppKit のメインスレッドから呼ぶこと。
pub(crate) unsafe fn close_emoji_help_popover() {
    let popover = {
        // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
        let guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
        let Some(state) = guard.as_ref() else {
            return;
        };
        state.settings_controls.hud_emoji_help_popover
    };
    if popover.is_null() {
        return;
    }
    let () = msg_send![popover, close];
}

/// 数値欄の範囲ヒント popover の自動 close までの秒数。
const RANGE_HINT_AUTO_CLOSE_SECS: f64 = 2.5;

/// 数値欄の範囲ヒント popover を `anchor`（値を入力したフィールド）の右側に `text` で表示し、
/// `RANGE_HINT_AUTO_CLOSE_SECS` 秒後に自動で閉じるタイマーを張る。`setting_changed` が
/// `apply_setting_change` から `Some(RangeHint)` を受け取ったときに呼ぶ。
///
/// タイマーの張り替えは `present_hud`（`hide_timer`）と同じ考え方: 前回分がまだ生きていれば
/// `invalidate` してから新しく張る。`showRelativeToRect:` は再入の危険があるため
/// （`show_emoji_help_popover` のドキュメント参照）ロックを手放してから呼ぶ。
///
/// # Safety
/// - `APP_STATE` をロックしないこと（内部で取得・解放する）。
/// - AppKit のメインスレッドから呼ぶこと。
unsafe fn show_range_hint_popover(anchor: *mut AnyObject, text: &str) {
    if anchor.is_null() {
        return;
    }

    let popover = {
        // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
        let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
        let Some(state) = guard.as_mut() else {
            return;
        };
        let controls = &mut state.settings_controls;
        if controls.range_hint_popover.is_null() {
            return;
        }

        let ns = NSString::from_str(text);
        let () = msg_send![controls.range_hint_label, setStringValue: &*ns];

        if !controls.range_hint_close_timer.is_null() {
            let () = msg_send![controls.range_hint_close_timer, invalidate];
        }
        let timer: *mut AnyObject = msg_send![
            class!(NSTimer),
            scheduledTimerWithTimeInterval: RANGE_HINT_AUTO_CLOSE_SECS
            target: state.delegate
            selector: sel!(closeRangeHintPopover:)
            userInfo: ptr::null_mut::<AnyObject>()
            repeats: false
        ];
        controls.range_hint_close_timer = timer;

        controls.range_hint_popover
    };

    let zero_rect = NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size: NSSize {
            width: 0.0,
            height: 0.0,
        },
    };
    // 表示中に別フィールドで再トリガーされた場合の anchor 付け替えを AppKit 任せにせず、
    // 一旦明示的に閉じてから出し直す（close 済みの popover への close は無害）。
    let () = msg_send![popover, close];
    let () = msg_send![
        popover,
        showRelativeToRect: zero_rect
        ofView: anchor
        preferredEdge: NS_RECT_EDGE_MAX_X
    ];
}

/// 範囲ヒント popover の自動 close タイマー `closeRangeHintPopover:`。
extern "C" fn close_range_hint_popover(_: &AnyObject, _: Sel, _: *mut AnyObject) {
    unsafe {
        let popover = {
            // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
            let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
            let Some(state) = guard.as_mut() else {
                return;
            };
            // 発火済みタイマーへの再 invalidate を避けるため、フィールドを先に空にする
            state.settings_controls.range_hint_close_timer = ptr::null_mut();
            state.settings_controls.range_hint_popover
        };
        if popover.is_null() {
            return;
        }
        let () = msg_send![popover, close];
    }
}

/// 設定ウィンドウを閉じる直前に、開いたままの範囲ヒント・絵文字ヘルプ popover を明示的に
/// 閉じる。閉じかけのウィンドウの view を anchor にした popover が表示され続けるのを防ぐ
/// （`save_settings`（`performClose:` の直前）と `window_will_close` の双方から呼ぶ）。
/// Enter/保存で確定する経路では新たに範囲ヒントを出さない仕様のため、ここでは既存分の
/// close のみを扱う。
///
/// # Safety
/// - `APP_STATE` をロックしないこと（内部で取得・解放する）。
/// - AppKit のメインスレッドから呼ぶこと。
unsafe fn close_settings_popovers() {
    let (range_hint_popover, emoji_help_popover) = {
        // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
        let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
        let Some(state) = guard.as_mut() else {
            return;
        };
        let controls = &mut state.settings_controls;
        if !controls.range_hint_close_timer.is_null() {
            let () = msg_send![controls.range_hint_close_timer, invalidate];
            controls.range_hint_close_timer = ptr::null_mut();
        }
        (controls.range_hint_popover, controls.hud_emoji_help_popover)
    };
    if !range_hint_popover.is_null() {
        let () = msg_send![range_hint_popover, close];
    }
    if !emoji_help_popover.is_null() {
        let () = msg_send![emoji_help_popover, close];
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
            // 閉じかけのウィンドウの view を anchor にした popover が残らないよう、
            // performClose: の前に明示的に閉じておく
            close_settings_popovers();
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
        // 保存せず × ボタン等で閉じた経路でも、開いたままの popover を残さない
        close_settings_popovers();

        // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
        let mut guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
        let Some(state) = guard.as_mut() else {
            return;
        };

        let Some(path) = state.config_path.clone() else {
            return;
        };
        let (new_settings, start_at_login) = match resolved_config_from_file(&path) {
            Ok(result) => result,
            Err(err) => {
                eprintln!("warning: config reload failed, keeping current settings: {err}");
                return;
            }
        };
        apply_settings_now(state, new_settings);
        apply_start_at_login_if_changed(state, start_at_login);
        state.config_mtime = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok());
    }
}

/// 設定ウィンドウのテキストフィールドの入力中フィルタ・バリデーションを行う。
///
/// `sync_controls_from_settings` 等がロック保持中にフィールドへ `setStringValue:` すると、
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

        crate::settings_window::handle_text_change(state, object);
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
        crate::menu::apply_paused_state(
            state.menu_handles.status.status_item,
            state.menu_handles.status.pause_item,
            state.paused,
        );
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

        hud_show::hide_hud_now(this, state);
    }
}

#[cfg(test)]
mod tests {
    use super::get_delegate_class;
    use objc2::sel;

    /// メニュー項目やボタン・タイマーが送るセレクタに応答しないと、発火した瞬間に
    /// unrecognized selector で落ちる。セレクタ名は文字列なのでコンパイルでは
    /// 食い違いを検出できない。特に hideHud: / fadeTick: は登録（mod.rs）と送信
    /// （hud_show.rs）が別ファイルで、片方だけ触る編集が起きやすい。
    #[test]
    fn delegate_responds_to_menu_selectors() {
        let class = get_delegate_class();
        for selector in [
            sel!(openSettings:),
            sel!(togglePause:),
            sel!(openSupportPage:),
            sel!(showAboutPanel:),
            sel!(quitApp:),
            sel!(hideHud:),
            sel!(fadeTick:),
        ] {
            assert!(
                class.responds_to(selector),
                "delegate does not respond to {selector:?}"
            );
        }
    }

    /// AppKit 自身が送るセレクタの分。`applicationShouldHandleReopen:hasVisibleWindows:` に
    /// 応答しないと、起動済みのアプリを Spotlight から選び直しても何も起きない。
    #[test]
    fn delegate_responds_to_appkit_selectors() {
        let class = get_delegate_class();
        for selector in [
            sel!(applicationDidFinishLaunching:),
            sel!(applicationShouldHandleReopen:hasVisibleWindows:),
        ] {
            assert!(
                class.responds_to(selector),
                "delegate does not respond to {selector:?}"
            );
        }
    }

    /// 設定ウィンドウのボタン・コントロールが送るセレクタの分。`controlTextDidChange:` は
    /// `make_editable_field`（rows.rs）が `setDelegate:` した全フィールドから届く。
    #[test]
    fn delegate_responds_to_settings_window_selectors() {
        let class = get_delegate_class();
        for selector in [
            sel!(settingChanged:),
            sel!(resetSettings:),
            sel!(previewSettings:),
            sel!(saveSettings:),
            sel!(toggleLoginItem:),
            sel!(showEmojiHelp:),
            sel!(closeRangeHintPopover:),
            sel!(windowWillClose:),
            sel!(controlTextDidChange:),
        ] {
            assert!(
                class.responds_to(selector),
                "delegate does not respond to {selector:?}"
            );
        }
    }
}
