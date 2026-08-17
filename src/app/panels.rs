use objc2::runtime::{AnyObject, Sel};
use objc2::{class, msg_send};
use objc2_foundation::{NSRange, NSString};

use crate::i18n::{self, Lang, Msg};
use crate::objc_helpers::nsstring_from_str;

use super::APP_STATE;

/// 寄付タブの「ビールを奢る…」から開く支援ページ。
const SUPPORT_URL: &str = "https://buymeacoffee.com/somei";
/// About パネルのクレジット欄に載せるリポジトリ。
const REPOSITORY_URL: &str = "https://github.com/somei-san/cliip-show";

// NSAboutPanelOptionKey の実体。`.app` バンドルではないため Info.plist から
// 名前とバージョンを読めず、options で明示的に渡す。
const ABOUT_OPTION_APPLICATION_NAME: &str = "ApplicationName";
const ABOUT_OPTION_APPLICATION_VERSION: &str = "ApplicationVersion";
const ABOUT_OPTION_CREDITS: &str = "Credits";
/// NSAttributedString のリンク属性キー。
const NS_LINK_ATTRIBUTE_NAME: &str = "NSLink";

/// 自動起動が OFF なら、起動のたびに有効化を促すダイアログを出す。
///
/// 常駐が前提のアプリで自動起動が切れている状態は設定が未完了なので、断られても促し続ける。
/// 抑止チェックボックスを入れて閉じたときだけ `mark_prompted` を記録し、以後は出さない。
///
/// # Safety
/// - `APP_STATE` のロックを保持したまま呼ばないこと。`runModal` が実行ループを止めるため、
///   他の経路がロック待ちで固まる。
/// - AppKit のメインスレッドから呼ぶこと。
pub(super) unsafe fn prompt_login_item_if_needed(lang: Lang) {
    if crate::login_item::is_enabled() || crate::login_item::has_prompted() {
        return;
    }

    // accessory アプリ（setActivationPolicy: 1）はこれを呼ばないとダイアログが前面に来ない
    let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
    let () = msg_send![app, activateIgnoringOtherApps: true];

    let alert: *mut AnyObject = msg_send![class!(NSAlert), alloc];
    let alert: *mut AnyObject = msg_send![alert, init];

    let message = NSString::from_str(i18n::text(lang, Msg::LoginPromptMessage));
    let () = msg_send![alert, setMessageText: &*message];

    let informative = NSString::from_str(i18n::text(lang, Msg::LoginPromptDetail));
    let () = msg_send![alert, setInformativeText: &*informative];

    // 最初に追加したボタンが既定（Enterで発火）になる
    let enable_title = NSString::from_str(i18n::text(lang, Msg::LoginPromptEnable));
    let _: *mut AnyObject = msg_send![alert, addButtonWithTitle: &*enable_title];

    let later_title = NSString::from_str(i18n::text(lang, Msg::LoginPromptLater));
    let _: *mut AnyObject = msg_send![alert, addButtonWithTitle: &*later_title];

    let () = msg_send![alert, setShowsSuppressionButton: true];
    let suppression_button: *mut AnyObject = msg_send![alert, suppressionButton];
    if !suppression_button.is_null() {
        let title = NSString::from_str(i18n::text(lang, Msg::LoginPromptSuppress));
        let () = msg_send![suppression_button, setTitle: &*title];
    }

    const NS_ALERT_FIRST_BUTTON_RETURN: isize = 1000;
    const NS_CONTROL_STATE_VALUE_ON: isize = 1;
    let response: isize = msg_send![alert, runModal];

    let suppressed = if suppression_button.is_null() {
        false
    } else {
        let state: isize = msg_send![suppression_button, state];
        state == NS_CONTROL_STATE_VALUE_ON
    };
    let () = msg_send![alert, release];

    if suppressed {
        if let Err(error) = crate::login_item::mark_prompted() {
            eprintln!("warning: {error}");
        }
    }

    if response == NS_ALERT_FIRST_BUTTON_RETURN {
        if let Err(error) = crate::login_item::enable() {
            eprintln!("warning: {error}");
        }
    }
}

/// macOS 標準の About パネルを出す。バンドルを持たないので、表示内容は options で渡す。
pub(super) extern "C" fn show_about_panel(_: &AnyObject, _: Sel, _: *mut AnyObject) {
    unsafe {
        let lang = {
            // AppKit メインスレッドからのみ呼ばれるため、Mutex が poison されるケースは実質発生しない
            let guard = APP_STATE.lock().expect("APP_STATE lock poisoned");
            let Some(state) = guard.as_ref() else {
                return;
            };
            i18n::resolve(state.settings.language)
        };

        let keys = [
            nsstring_from_str(ABOUT_OPTION_APPLICATION_NAME),
            nsstring_from_str(ABOUT_OPTION_APPLICATION_VERSION),
            nsstring_from_str(ABOUT_OPTION_CREDITS),
        ];
        let values = [
            nsstring_from_str(i18n::APP_NAME),
            nsstring_from_str(env!("CARGO_PKG_VERSION")),
            about_credits(lang),
        ];
        let options: *mut AnyObject = msg_send![
            class!(NSDictionary),
            dictionaryWithObjects: values.as_ptr()
            forKeys: keys.as_ptr()
            count: keys.len()
        ];

        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        // パネルは通常のウィンドウなので、アクセサリアプリでは前面に出すため activate が要る
        let () = msg_send![app, activateIgnoringOtherApps: true];
        let () = msg_send![app, orderFrontStandardAboutPanelWithOptions: options];

        for object in keys.into_iter().chain(values) {
            let () = msg_send![object, release];
        }
    }
}

/// About パネルのクレジット欄。説明の下にリポジトリへのリンクを置く。
unsafe fn about_credits(lang: Lang) -> *mut AnyObject {
    let text = format!(
        "{}\n{REPOSITORY_URL}",
        i18n::text(lang, Msg::AboutDescription)
    );
    let text_ns = NSString::from_str(&text);
    let credits: *mut AnyObject = msg_send![class!(NSMutableAttributedString), alloc];
    let credits: *mut AnyObject = msg_send![credits, initWithString: &*text_ns];

    // URL の範囲だけリンクにする。NSRange が数えるのは Rust の文字数ではなく UTF-16 の要素数
    let first_line_len = text
        .split('\n')
        .next()
        .unwrap_or_default()
        .encode_utf16()
        .count();
    let link_range = NSRange {
        location: first_line_len + 1,
        length: REPOSITORY_URL.encode_utf16().count(),
    };
    let link_key = NSString::from_str(NS_LINK_ATTRIBUTE_NAME);
    let url_ns = NSString::from_str(REPOSITORY_URL);
    let url: *mut AnyObject = msg_send![class!(NSURL), URLWithString: &*url_ns];
    if !url.is_null() {
        let () = msg_send![
            credits,
            addAttribute: &*link_key
            value: url
            range: link_range
        ];
    }

    credits
}

/// 支援ページを既定のブラウザで開く。URL は言語で変えない。
pub(super) extern "C" fn open_support_page(_: &AnyObject, _: Sel, _: *mut AnyObject) {
    unsafe {
        let url_string = NSString::from_str(SUPPORT_URL);
        let url: *mut AnyObject = msg_send![class!(NSURL), URLWithString: &*url_string];
        if url.is_null() {
            eprintln!("warning: failed to build support URL: {SUPPORT_URL}");
            return;
        }
        let workspace: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
        let opened: bool = msg_send![workspace, openURL: url];
        if !opened {
            eprintln!("warning: failed to open support URL: {SUPPORT_URL}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SUPPORT_URL;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use objc2_foundation::NSString;

    /// 開く URL が壊れていると `NSURL` が nil を返し、何も起きない。
    #[test]
    fn support_url_is_a_valid_url() {
        unsafe {
            let url_string = NSString::from_str(SUPPORT_URL);
            let url: *mut AnyObject = msg_send![class!(NSURL), URLWithString: &*url_string];
            assert!(!url.is_null(), "{SUPPORT_URL} is not a valid URL");
        }
    }
}
