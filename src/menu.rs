use objc2::runtime::{AnyObject, Sel};
use objc2::{class, msg_send, sel};

use crate::i18n::{self, Lang, Msg};
use crate::objc_helpers::{nsstring_from_str, template_image_from_png};

const NS_VARIABLE_STATUS_ITEM_LENGTH: f64 = -1.0;

/// アイコン素材。元データは `assets/peanut-template.svg` で、パスが多く手書きに落とせないため
/// 白抜き・背景透過の PNG に変換したものを埋め込む。テンプレート画像はアルファだけを使うので
/// 色は問わない。生成手順は `docs/development.md` を参照。
const PEANUT_TEMPLATE_PNG: &[u8] = include_bytes!("../assets/peanut-template.png");

/// メニューバーに描くアイコンの一辺（pt）。
const STATUS_ICON_SIZE: f64 = 18.0;
// NSControlStateValue
const NS_CONTROL_STATE_VALUE_OFF: isize = 0;
const NS_CONTROL_STATE_VALUE_ON: isize = 1;

const STATUS_ICON_ALPHA_ACTIVE: f64 = 1.0;
const STATUS_ICON_ALPHA_PAUSED: f64 = 0.4;

/// メニューバー常駐アイコンのメニュー項目群。`apply_paused_state`／`apply_language` で
/// 状態・表示言語を切り替えるため、呼び出し側（`AppState`）が生存期間中保持すること。
pub struct StatusItemHandles {
    pub status_item: *mut AnyObject,
    pub pause_item: *mut AnyObject,
    pub settings_item: *mut AnyObject,
    pub about_item: *mut AnyObject,
    pub quit_item: *mut AnyObject,
}

/// メインメニューの「編集」サブメニューとその項目群。`cut:`/`copy:`/`paste:`/`selectAll:` は
/// responder chain に流す標準アクションで target を持たないため、`apply_language` で
/// タイトルを差し替える以外の用途では触らない。
pub struct EditMenuHandles {
    pub edit_menu_item: *mut AnyObject,
    pub edit_menu: *mut AnyObject,
    pub cut_item: *mut AnyObject,
    pub copy_item: *mut AnyObject,
    pub paste_item: *mut AnyObject,
    pub select_all_item: *mut AnyObject,
}

/// `create_status_item`／`install_main_menu` が返す項目群をまとめたもの。`apply_language` の
/// 引数としてまとめて渡す。
pub struct MenuHandles {
    pub status: StatusItemHandles,
    pub edit: EditMenuHandles,
}

/// メニューバーの常駐アイコンとメニューを構築する。
///
/// # Safety
/// - `delegate` は `openSettings:` / `togglePause:` / `quitApp:` セレクタを実装していること。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn create_status_item(delegate: &AnyObject, lang: Lang) -> StatusItemHandles {
    let status_bar: *mut AnyObject = msg_send![class!(NSStatusBar), systemStatusBar];
    let status_item: *mut AnyObject =
        msg_send![status_bar, statusItemWithLength: NS_VARIABLE_STATUS_ITEM_LENGTH];
    // NSStatusBar は status item を保持しない。retain しないと autorelease pool の
    // 解放時に解放され、アイコンがメニューバーから消える。
    let status_item: *mut AnyObject = msg_send![status_item, retain];

    set_status_icon(status_item);

    let menu: *mut AnyObject = msg_send![class!(NSMenu), alloc];
    let menu: *mut AnyObject = msg_send![menu, init];

    let settings_item = make_menu_item(
        delegate,
        i18n::text(lang, Msg::MenuSettings),
        sel!(openSettings:),
    );
    let () = msg_send![menu, addItem: settings_item];

    let pause_item = make_menu_item(
        delegate,
        i18n::text(lang, Msg::MenuPause),
        sel!(togglePause:),
    );
    let () = msg_send![pause_item, setState: NS_CONTROL_STATE_VALUE_OFF];
    let () = msg_send![menu, addItem: pause_item];

    let separator: *mut AnyObject = msg_send![class!(NSMenuItem), separatorItem];
    let () = msg_send![menu, addItem: separator];

    let about_item = make_menu_item(
        delegate,
        i18n::text(lang, Msg::MenuAbout),
        sel!(showAboutPanel:),
    );
    let () = msg_send![menu, addItem: about_item];

    let quit_item = make_menu_item(delegate, i18n::text(lang, Msg::MenuQuit), sel!(quitApp:));
    let () = msg_send![menu, addItem: quit_item];

    let () = msg_send![status_item, setMenu: menu];

    StatusItemHandles {
        status_item,
        pause_item,
        settings_item,
        about_item,
        quit_item,
    }
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
unsafe fn set_status_icon(status_item: *mut AnyObject) {
    let button: *mut AnyObject = msg_send![status_item, button];
    if button.is_null() {
        return;
    }

    let image = template_image_from_png(PEANUT_TEMPLATE_PNG, STATUS_ICON_SIZE);
    if image.is_null() {
        eprintln!("warning: failed to load menu bar icon");
        return;
    }
    let () = msg_send![button, setImage: image];
    let () = msg_send![image, release];
}

/// 最小構成のメインメニューを組む。このアプリはこれまでメインメニューを持たず、Cmd+V 等の
/// 標準ショートカットがどのウィンドウでも効かなかった（responder chain に流す先が無いため）。
///
/// `create_status_item` が作るメニューバー常駐アイコンのメニューとは別物。
///
/// # Safety
/// AppKit のメインスレッドから呼ぶこと。
pub unsafe fn install_main_menu(lang: Lang) -> EditMenuHandles {
    let main_menu: *mut AnyObject = msg_send![class!(NSMenu), alloc];
    let main_menu: *mut AnyObject = msg_send![main_menu, init];

    // アプリ名はローカライズ対象ではない（固有名詞のため Msg には持たせない）
    let app_menu_item = make_container_menu_item("cliip-show");
    let app_menu: *mut AnyObject = msg_send![class!(NSMenu), alloc];
    let app_menu: *mut AnyObject = msg_send![app_menu, init];
    let () = msg_send![app_menu_item, setSubmenu: app_menu];
    let () = msg_send![main_menu, addItem: app_menu_item];

    let edit_title = i18n::text(lang, Msg::MenuEdit);
    let edit_menu_item = make_container_menu_item(edit_title);
    let edit_menu_title = nsstring_from_str(edit_title);
    let edit_menu: *mut AnyObject = msg_send![class!(NSMenu), alloc];
    let edit_menu: *mut AnyObject = msg_send![edit_menu, initWithTitle: edit_menu_title];
    let () = msg_send![edit_menu_title, release];

    // cut:/copy:/paste:/selectAll: は responder chain に流す標準アクションのため target は
    // nil のまま（setTarget を呼ばない）。alloc/init 直後の NSMenuItem は target が既に nil。
    let cut_item = make_responder_menu_item(i18n::text(lang, Msg::MenuCut), sel!(cut:), "x");
    let copy_item = make_responder_menu_item(i18n::text(lang, Msg::MenuCopy), sel!(copy:), "c");
    let paste_item = make_responder_menu_item(i18n::text(lang, Msg::MenuPaste), sel!(paste:), "v");
    let select_all_item =
        make_responder_menu_item(i18n::text(lang, Msg::MenuSelectAll), sel!(selectAll:), "a");
    let () = msg_send![edit_menu, addItem: cut_item];
    let () = msg_send![edit_menu, addItem: copy_item];
    let () = msg_send![edit_menu, addItem: paste_item];
    let () = msg_send![edit_menu, addItem: select_all_item];
    let () = msg_send![edit_menu_item, setSubmenu: edit_menu];
    let () = msg_send![main_menu, addItem: edit_menu_item];

    let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
    let () = msg_send![app, setMainMenu: main_menu];

    EditMenuHandles {
        edit_menu_item,
        edit_menu,
        cut_item,
        copy_item,
        paste_item,
        select_all_item,
    }
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

/// 言語切り替えで、メニューバー常駐アイコンのメニューとメインメニューの「編集」サブメニューの
/// タイトルをすべて差し替える。
///
/// # Safety
/// - `handles` は `create_status_item`／`install_main_menu` が返した有効なポインタであること。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn apply_language(handles: &MenuHandles, lang: Lang) {
    set_title(
        handles.status.settings_item,
        i18n::text(lang, Msg::MenuSettings),
    );
    set_title(handles.status.pause_item, i18n::text(lang, Msg::MenuPause));
    set_title(handles.status.about_item, i18n::text(lang, Msg::MenuAbout));
    set_title(handles.status.quit_item, i18n::text(lang, Msg::MenuQuit));
    set_title(handles.edit.edit_menu_item, i18n::text(lang, Msg::MenuEdit));
    set_title(handles.edit.edit_menu, i18n::text(lang, Msg::MenuEdit));
    set_title(handles.edit.cut_item, i18n::text(lang, Msg::MenuCut));
    set_title(handles.edit.copy_item, i18n::text(lang, Msg::MenuCopy));
    set_title(handles.edit.paste_item, i18n::text(lang, Msg::MenuPaste));
    set_title(
        handles.edit.select_all_item,
        i18n::text(lang, Msg::MenuSelectAll),
    );
}

/// `NSMenuItem`／`NSMenu` の `setTitle:` 呼び出しをまとめた共通処理。
unsafe fn set_title(item: *mut AnyObject, text: &str) {
    let ns = nsstring_from_str(text);
    let () = msg_send![item, setTitle: ns];
    let () = msg_send![ns, release];
}
