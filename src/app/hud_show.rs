use std::ptr;

use objc2::runtime::{AnyObject, Sel};
use objc2::{class, msg_send, sel};
use objc2_foundation::{NSSize, NSString};

use crate::hud::{fit_thumbnail_size, layout_hud, layout_hud_image};
use crate::text::truncate_text;

use super::{AppState, APP_STATE};

const FADE_TICK_INTERVAL_SECS: f64 = 1.0 / 60.0;

pub(crate) unsafe fn show_text_content(state: &mut AppState, text: &str) {
    let truncated = truncate_text(
        text,
        state.settings.truncate_max_width,
        state.settings.truncate_max_lines,
    );
    let message = NSString::from_str(&truncated);
    let () = msg_send![state.label, setStringValue: &*message];

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
pub(super) unsafe fn present_pasteboard_content(this: &AnyObject, state: &mut AppState) -> bool {
    let text_type = NSString::from_str("public.utf8-plain-text");
    let raw_text: *mut AnyObject = msg_send![state.pasteboard, stringForType: &*text_type];

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

/// `hideHud:` の実処理。`fade_timer`/`hide_timer` の後始末とフェードアウト開始をまとめて行う。
///
/// # Safety
/// - `APP_STATE` のロックは呼び出し側が取得済みであること。
/// - AppKit のメインスレッドから呼ぶこと。
pub(super) unsafe fn hide_hud_now(this: &AnyObject, state: &mut AppState) {
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

/// フェードアウトの 1 ティック。`hide_hud_now` が張った `fadeTick:` タイマーから呼ばれる。
///
/// フィールドの更新が終わったら `drop(guard)` でロックを手放してから AppKit を呼ぶ。
/// ティックは高頻度に発火するので、AppKit の描画中までロックを持ち続けない。
/// この手放しのタイミングが本質のため、ロック取得だけを mod.rs のシムに切り出す形には
/// していない。
pub(super) extern "C" fn fade_tick(_: &AnyObject, _: Sel, timer: *mut AnyObject) {
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
    use super::{image_from_pasteboard, FADE_TICK_INTERVAL_SECS};
    use crate::config::DEFAULT_HUD_FADE_DURATION_SECS;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use objc2_foundation::{NSSize, NSString};
    use std::ptr;

    /// 一般ペーストボード（ユーザーのクリップボード）を汚さないよう、専用の名前付きペーストボードを使う。
    #[test]
    fn image_from_pasteboard_reads_png_and_ignores_plain_text() {
        unsafe {
            let pasteboard: *mut AnyObject =
                msg_send![class!(NSPasteboard), pasteboardWithUniqueName];
            assert!(!pasteboard.is_null());

            let text_type = NSString::from_str("public.utf8-plain-text");
            let types: *mut AnyObject = msg_send![class!(NSArray), arrayWithObject: &*text_type];
            let _: isize =
                msg_send![pasteboard, declareTypes: types owner: ptr::null_mut::<AnyObject>()];
            let text = NSString::from_str("no image here");
            let _: bool = msg_send![pasteboard, setString: &*text forType: &*text_type];

            let no_image = image_from_pasteboard(pasteboard);
            assert!(
                no_image.is_null(),
                "text-only pasteboard must yield no image"
            );

            let png_type = NSString::from_str("public.png");
            let types: *mut AnyObject = msg_send![class!(NSArray), arrayWithObject: &*png_type];
            let _: isize =
                msg_send![pasteboard, declareTypes: types owner: ptr::null_mut::<AnyObject>()];
            let png_path = NSString::from_str(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/visual/baseline/image_tiny.png"
            ));
            let png_data: *mut AnyObject =
                msg_send![class!(NSData), dataWithContentsOfFile: &*png_path];
            assert!(!png_data.is_null(), "fixture PNG must be readable");
            let _: bool = msg_send![pasteboard, setData: png_data forType: &*png_type];

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
