use objc2::runtime::{AnyObject, Sel};
use objc2::{class, msg_send, sel};
use objc2_foundation::{NSPoint, NSSize};

use crate::objc_helpers::nsstring_from_str;

const NS_VARIABLE_STATUS_ITEM_LENGTH: f64 = -1.0;

/// アイコンの元データは `assets/peanut-template.svg`。SVG を読む依存を足さずに済むよう、
/// パス（`M` と 8 本の `Q`）をここに書き起こしている。SVG を差し替えたらここも更新すること。
const PEANUT_VIEWBOX: f64 = 22.0;
const PEANUT_START: NSPoint = NSPoint {
    x: 9.42962691275,
    y: 8.89807183979,
};
/// 各要素は `(制御点x, 制御点y, 終点x, 終点y)`。
const PEANUT_SEGMENTS: [(f64, f64, f64, f64); 8] = [
    (8.46815309688, 6.27427144382, 9.84488270293, 4.53354131168),
    (12.2606207307, 1.13005012634, 16.4433682323, 2.29871736419),
    (19.3273552068, 5.54583591854, 17.3273074423, 9.20907920931),
    (16.3662680996, 11.2095614469, 13.5865295458, 11.495592894),
    (14.0804061246, 14.4052075006, 12.7297386237, 17.0292251331),
    (10.0542484905, 20.8484065817, 5.5337798045, 19.7577084228),
    (2.57182375107, 16.1728686841, 4.83162362094, 12.09393513),
    (6.59798138514, 9.72966960294, 9.42962691275, 8.89807183979),
];

/// メニューバーに描くアイコンの一辺（pt）。
const STATUS_ICON_SIZE: f64 = 18.0;
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
/// - `delegate` は `openSettings:` / `togglePause:` / `quitApp:` セレクタを実装していること。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn create_status_item(delegate: &AnyObject) -> (*mut AnyObject, *mut AnyObject) {
    let status_bar: *mut AnyObject = msg_send![class!(NSStatusBar), systemStatusBar];
    let status_item: *mut AnyObject =
        msg_send![status_bar, statusItemWithLength: NS_VARIABLE_STATUS_ITEM_LENGTH];
    // NSStatusBar は status item を保持しない。retain しないと autorelease pool の
    // 解放時に解放され、アイコンがメニューバーから消える。
    let status_item: *mut AnyObject = msg_send![status_item, retain];

    set_status_icon(status_item);

    let menu: *mut AnyObject = msg_send![class!(NSMenu), alloc];
    let menu: *mut AnyObject = msg_send![menu, init];

    let settings_item = make_menu_item(delegate, "設定…", sel!(openSettings:));
    let () = msg_send![menu, addItem: settings_item];

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

/// status item の button にピーナッツのアイコンを設定する。
///
/// `setTemplate: true` にすると描画色は無視され、アルファだけがマスクとして使われる。
/// macOS がメニューバーの濃淡に応じて白黒を決めるので、ライト/ダークに自動追従する。
unsafe fn set_status_icon(status_item: *mut AnyObject) {
    let button: *mut AnyObject = msg_send![status_item, button];
    if button.is_null() {
        return;
    }

    let size = NSSize {
        width: STATUS_ICON_SIZE,
        height: STATUS_ICON_SIZE,
    };
    let image: *mut AnyObject = msg_send![class!(NSImage), alloc];
    let image: *mut AnyObject = msg_send![image, initWithSize: size];

    let () = msg_send![image, lockFocus];
    let black: *mut AnyObject = msg_send![class!(NSColor), blackColor];
    let () = msg_send![black, set];
    let path = peanut_path(STATUS_ICON_SIZE / PEANUT_VIEWBOX);
    let () = msg_send![path, fill];
    let () = msg_send![image, unlockFocus];

    let () = msg_send![image, setTemplate: true];
    let () = msg_send![button, setImage: image];
    let () = msg_send![image, release];
}

/// `PEANUT_SEGMENTS` から `NSBezierPath` を組む。
///
/// SVG は y 軸が下向きなので上下を反転する。二次ベジェは
/// `c1 = p0 + 2/3 (q - p0)`, `c2 = p1 + 2/3 (q - p1)` で三次ベジェに変換する。
unsafe fn peanut_path(scale: f64) -> *mut AnyObject {
    let to_view = |x: f64, y: f64| NSPoint {
        x: x * scale,
        y: (PEANUT_VIEWBOX - y) * scale,
    };

    let path: *mut AnyObject = msg_send![class!(NSBezierPath), bezierPath];
    let mut current = to_view(PEANUT_START.x, PEANUT_START.y);
    let () = msg_send![path, moveToPoint: current];

    for (qx, qy, ex, ey) in PEANUT_SEGMENTS {
        let q = to_view(qx, qy);
        let end = to_view(ex, ey);
        let c1 = NSPoint {
            x: current.x + 2.0 / 3.0 * (q.x - current.x),
            y: current.y + 2.0 / 3.0 * (q.y - current.y),
        };
        let c2 = NSPoint {
            x: end.x + 2.0 / 3.0 * (q.x - end.x),
            y: end.y + 2.0 / 3.0 * (q.y - end.y),
        };
        let () = msg_send![path, curveToPoint: end controlPoint1: c1 controlPoint2: c2];
        current = end;
    }

    let () = msg_send![path, closePath];
    path
}

/// 最小構成のメインメニューを組む。このアプリはこれまでメインメニューを持たず、Cmd+V 等の
/// 標準ショートカットがどのウィンドウでも効かなかった（responder chain に流す先が無いため）。
///
/// `create_status_item` が作るメニューバー常駐アイコンのメニューとは別物。
///
/// # Safety
/// AppKit のメインスレッドから呼ぶこと。
pub unsafe fn install_main_menu() {
    let main_menu: *mut AnyObject = msg_send![class!(NSMenu), alloc];
    let main_menu: *mut AnyObject = msg_send![main_menu, init];

    let app_menu_item = make_container_menu_item("cliip-show");
    let app_menu: *mut AnyObject = msg_send![class!(NSMenu), alloc];
    let app_menu: *mut AnyObject = msg_send![app_menu, init];
    let () = msg_send![app_menu_item, setSubmenu: app_menu];
    let () = msg_send![main_menu, addItem: app_menu_item];

    let edit_menu_item = make_container_menu_item("編集");
    let edit_menu_title = nsstring_from_str("編集");
    let edit_menu: *mut AnyObject = msg_send![class!(NSMenu), alloc];
    let edit_menu: *mut AnyObject = msg_send![edit_menu, initWithTitle: edit_menu_title];
    let () = msg_send![edit_menu_title, release];

    // cut:/copy:/paste:/selectAll: は responder chain に流す標準アクションのため target は
    // nil のまま（setTarget を呼ばない）。alloc/init 直後の NSMenuItem は target が既に nil。
    let cut_item = make_responder_menu_item("切り取り", sel!(cut:), "x");
    let copy_item = make_responder_menu_item("コピー", sel!(copy:), "c");
    let paste_item = make_responder_menu_item("ペースト", sel!(paste:), "v");
    let select_all_item = make_responder_menu_item("すべてを選択", sel!(selectAll:), "a");
    let () = msg_send![edit_menu, addItem: cut_item];
    let () = msg_send![edit_menu, addItem: copy_item];
    let () = msg_send![edit_menu, addItem: paste_item];
    let () = msg_send![edit_menu, addItem: select_all_item];
    let () = msg_send![edit_menu_item, setSubmenu: edit_menu];
    let () = msg_send![main_menu, addItem: edit_menu_item];

    let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
    let () = msg_send![app, setMainMenu: main_menu];
}

/// サブメニューを持たせるだけの入れ物。`initWithTitle:action:keyEquivalent:` は使わず、
/// 素の `init` の後に `setTitle:` で名前を付ける（action は不要なため）。
unsafe fn make_container_menu_item(title: &str) -> *mut AnyObject {
    let item: *mut AnyObject = msg_send![class!(NSMenuItem), alloc];
    let item: *mut AnyObject = msg_send![item, init];
    let title_str = nsstring_from_str(title);
    let () = msg_send![item, setTitle: title_str];
    let () = msg_send![title_str, release];
    item
}

/// target を設定しない（nil のままにする）以外は `make_menu_item` と同じ組み立て。
unsafe fn make_responder_menu_item(
    title: &str,
    action: Sel,
    key_equivalent: &str,
) -> *mut AnyObject {
    let title_str = nsstring_from_str(title);
    let key_equivalent_str = nsstring_from_str(key_equivalent);
    let item: *mut AnyObject = msg_send![class!(NSMenuItem), alloc];
    let item: *mut AnyObject = msg_send![
        item,
        initWithTitle: title_str
        action: action
        keyEquivalent: key_equivalent_str
    ];
    let () = msg_send![title_str, release];
    let () = msg_send![key_equivalent_str, release];
    item
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
