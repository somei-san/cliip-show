use std::ptr;

use objc2::runtime::{AnyObject, Sel};
use objc2::{class, msg_send, sel};

use crate::objc_helpers::nsstring_from_str;

const NS_VARIABLE_STATUS_ITEM_LENGTH: f64 = -1.0;
// NSControlStateValue
const NS_CONTROL_STATE_VALUE_OFF: isize = 0;
const NS_CONTROL_STATE_VALUE_ON: isize = 1;

const STATUS_ICON_ALPHA_ACTIVE: f64 = 1.0;
const STATUS_ICON_ALPHA_PAUSED: f64 = 0.4;

/// メニューバーの常駐アイコンとメニューを構築する。
///
/// 返すポインタは `apply_paused_state` で状態を切り替えるため、呼び出し側が生存期間中保持すること。
///
/// # Safety
/// - `delegate` は `showPreview:` / `togglePause:` / `quitApp:` セレクタを実装していること。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn create_status_item(
    delegate: &AnyObject,
    fallback_emoji: &str,
) -> (*mut AnyObject, *mut AnyObject) {
    let status_bar: *mut AnyObject = msg_send![class!(NSStatusBar), systemStatusBar];
    let status_item: *mut AnyObject =
        msg_send![status_bar, statusItemWithLength: NS_VARIABLE_STATUS_ITEM_LENGTH];
    // NSStatusBar は status item を保持しない。retain しないと autorelease pool の
    // 解放時に解放され、アイコンがメニューバーから消える。
    let status_item: *mut AnyObject = msg_send![status_item, retain];

    set_status_icon(status_item, fallback_emoji);

    let menu: *mut AnyObject = msg_send![class!(NSMenu), alloc];
    let menu: *mut AnyObject = msg_send![menu, init];

    let preview_item = make_menu_item(delegate, "プレビュー表示", sel!(showPreview:));
    let () = msg_send![menu, addItem: preview_item];

    let pause_item = make_menu_item(delegate, "一時停止", sel!(togglePause:));
    let () = msg_send![pause_item, setState: NS_CONTROL_STATE_VALUE_OFF];
    let () = msg_send![menu, addItem: pause_item];

    let separator: *mut AnyObject = msg_send![class!(NSMenuItem), separatorItem];
    let () = msg_send![menu, addItem: separator];

    let quit_item = make_menu_item(delegate, "cliip-show を終了", sel!(quitApp:));
    let () = msg_send![menu, addItem: quit_item];

    let () = msg_send![status_item, setMenu: menu];

    (status_item, pause_item)
}

unsafe fn make_menu_item(delegate: &AnyObject, title: &str, action: Sel) -> *mut AnyObject {
    let title_str = nsstring_from_str(title);
    let key_equivalent = nsstring_from_str("");
    let item: *mut AnyObject = msg_send![class!(NSMenuItem), alloc];
    let item: *mut AnyObject = msg_send![
        item,
        initWithTitle: title_str
        action: action
        keyEquivalent: key_equivalent
    ];
    let () = msg_send![title_str, release];
    let () = msg_send![key_equivalent, release];
    let () = msg_send![item, setTarget: delegate];
    item
}

/// status item の button に SF Symbols アイコンを設定する。
///
/// `setTemplate: true` により、ライト/ダーク・メニューバーの濃淡に自動追従する。
/// `imageWithSystemSymbolName:` は Big Sur で追加されたため、セレクタの有無を確かめてから
/// 呼ぶ。持たない OS では未知セレクタで例外になり、`fallback_emoji` に到達できない。
unsafe fn set_status_icon(status_item: *mut AnyObject, fallback_emoji: &str) {
    let button: *mut AnyObject = msg_send![status_item, button];
    if button.is_null() {
        return;
    }

    let symbol_selector = sel!(imageWithSystemSymbolName:accessibilityDescription:);
    let supports_symbols: bool = msg_send![class!(NSImage), respondsToSelector: symbol_selector];
    let image: *mut AnyObject = if supports_symbols {
        let symbol_name = nsstring_from_str("doc.on.clipboard");
        let image: *mut AnyObject = msg_send![
            class!(NSImage),
            imageWithSystemSymbolName: symbol_name
            accessibilityDescription: ptr::null_mut::<AnyObject>()
        ];
        let () = msg_send![symbol_name, release];
        image
    } else {
        ptr::null_mut()
    };

    if image.is_null() {
        let title = nsstring_from_str(fallback_emoji);
        let () = msg_send![button, setTitle: title];
        let () = msg_send![title, release];
        return;
    }

    let () = msg_send![image, setTemplate: true];
    let () = msg_send![button, setImage: image];
}

/// 一時停止状態に応じてメニューのチェックマークとアイコンの alpha を更新する。
///
/// # Safety
/// - `status_item`・`pause_item` は `create_status_item` が返した有効なポインタであること。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn apply_paused_state(
    status_item: *mut AnyObject,
    pause_item: *mut AnyObject,
    paused: bool,
) {
    let state = if paused {
        NS_CONTROL_STATE_VALUE_ON
    } else {
        NS_CONTROL_STATE_VALUE_OFF
    };
    let () = msg_send![pause_item, setState: state];

    let button: *mut AnyObject = msg_send![status_item, button];
    if button.is_null() {
        return;
    }
    let alpha = if paused {
        STATUS_ICON_ALPHA_PAUSED
    } else {
        STATUS_ICON_ALPHA_ACTIVE
    };
    let () = msg_send![button, setAlphaValue: alpha];
}
