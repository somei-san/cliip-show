use std::path::PathBuf;
use std::ptr;
use std::sync::{Mutex, Once};
use std::time::SystemTime;

use objc2::declare::ClassBuilder;
use objc2::runtime::{AnyClass, AnyObject, Sel};
use objc2::{class, msg_send, sel};

use crate::config::{
    apply_config_file, apply_env_overrides, default_display_settings, display_settings,
    load_config_file, DisplaySettings,
};
use crate::hud::{create_hud_window, hud_background_rgba, hud_border_white_alpha, layout_hud};
use crate::objc_helpers::nsstring_from_str;
use crate::text::truncate_text;

pub const FADE_TICK_INTERVAL_SECS: f64 = 1.0 / 60.0;

pub struct AppState {
    pub last_change_count: isize,
    pub pasteboard: *mut AnyObject,
    pub window: *mut AnyObject,
    pub icon_label: *mut AnyObject,
    pub label: *mut AnyObject,
    pub hide_timer: *mut AnyObject,
    pub fade_timer: *mut AnyObject,
    pub fade_ticks_elapsed: u32,
    pub fade_total_ticks: u32,
    pub settings: DisplaySettings,
    pub config_path: Option<PathBuf>,
    pub config_mtime: Option<SystemTime>,
    pub config_check_counter: u32,
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

        let class = builder.register();
        CLASS = class as *const AnyClass;
    });

    unsafe { &*CLASS }
}

extern "C" fn application_did_finish_launching(this: &AnyObject, _: Sel, _: *mut AnyObject) {
    unsafe {
        let settings = display_settings();
        let pasteboard: *mut AnyObject = msg_send![class!(NSPasteboard), generalPasteboard];
        let last_change_count: isize = msg_send![pasteboard, changeCount];

        let (window, icon_label, label) = create_hud_window(settings.clone());
        if window.is_null() {
            eprintln!("fatal: HUD ウィンドウの作成に失敗しました");
            std::process::exit(1);
        }

        // パスが解決できない場合もパスだけは保持し、後でファイルが作成されても検知できるようにする
        let config_path = crate::config::config_file_path().ok();
        let config_mtime = config_path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok());

        let poll_interval = settings.poll_interval_secs;

        // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
        *APP_STATE.lock().expect("APP_STATE lock poisoned") = Some(AppState {
            last_change_count,
            pasteboard,
            window,
            icon_label,
            label,
            hide_timer: ptr::null_mut(),
            fade_timer: ptr::null_mut(),
            fade_ticks_elapsed: 0,
            fade_total_ticks: 0,
            settings,
            config_path,
            config_mtime,
            config_check_counter: 0,
        });

        let _: *mut AnyObject = msg_send![
            class!(NSTimer),
            scheduledTimerWithTimeInterval: poll_interval
            target: this
            selector: sel!(pollPasteboard:)
            userInfo: ptr::null_mut::<AnyObject>()
            repeats: true
        ];
    }
}

// poll_pasteboard が呼ばれるたびにカウントし、この回数ごとに mtime チェックを行う
// デフォルト poll_interval_secs=0.3 × 10 = 約3秒ごと
const CONFIG_CHECK_EVERY_N_POLLS: u32 = 10;

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
    let new_settings = match load_config_file(path) {
        Ok((config, _)) => {
            let base = default_display_settings();
            apply_env_overrides(apply_config_file(base, &config))
        }
        Err(err) => {
            eprintln!("warning: config reload failed, keeping current settings: {err}");
            return;
        }
    };
    state.config_mtime = current_mtime;

    // hud_emoji が変わったらアイコンラベルを即時更新
    if new_settings.hud_emoji != state.settings.hud_emoji {
        let emoji = nsstring_from_str(&new_settings.hud_emoji);
        let () = msg_send![state.icon_label, setStringValue: emoji];
        let () = msg_send![emoji, release];
    }

    // hud_background_color が変わったら背景レイヤーを即時更新
    // content_view・layer が nil の場合は msg_send が no-op となり更新がスキップされる（ObjC 仕様）
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

    let poll_changed =
        (new_settings.poll_interval_secs - state.settings.poll_interval_secs).abs() > 1e-9;
    state.settings = new_settings;

    if poll_changed {
        eprintln!("config reloaded (note: poll_interval_secs change takes effect after restart)");
    } else {
        eprintln!("config reloaded");
    }
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

        let text_type = nsstring_from_str("public.utf8-plain-text");
        let raw_text: *mut AnyObject = msg_send![state.pasteboard, stringForType: text_type];
        let () = msg_send![text_type, release];

        let Some(text) = crate::objc_helpers::nsstring_to_string(raw_text) else {
            return;
        };

        let truncated = truncate_text(
            &text,
            state.settings.truncate_max_width,
            state.settings.truncate_max_lines,
        );
        let message = nsstring_from_str(&truncated);
        let () = msg_send![state.label, setStringValue: message];
        let () = msg_send![message, release];

        layout_hud(
            state.window,
            state.icon_label,
            state.label,
            state.settings.clone(),
        );

        // フェード中なら止めてアルファを戻す
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
    use super::FADE_TICK_INTERVAL_SECS;
    use crate::config::DEFAULT_HUD_FADE_DURATION_SECS;

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
