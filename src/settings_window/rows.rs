use std::ptr;
use std::sync::Once;

use objc2::declare::ClassBuilder;
use objc2::runtime::{AnyClass, AnyObject, Sel};
use objc2::{class, msg_send, sel};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::config::{ConfigKey, LanguageSetting};
use crate::i18n::{self, Lang, Msg};

use super::{config_key_to_tag, LocalizedControl, LocalizedKind};

// NSWindowStyleMask: Titled | Closable
pub(super) const SETTINGS_STYLE_MASK: usize = 1 | 2;

pub(super) const SETTINGS_WINDOW_WIDTH: f64 = 520.0;
const SETTINGS_ROW_HEIGHT: f64 = 34.0;
const SETTINGS_ROW_COUNT: usize = 10;
// HUD の見た目を決める上記10行とは別に、区切り行・言語・ログイン項目の 3 行を末尾に置く。
// 各行の番号は build_settings_window が出現順に払い出す。
pub(super) const SETTINGS_TOTAL_ROW_COUNT: usize = SETTINGS_ROW_COUNT + 3;
const SETTINGS_TOP_MARGIN: f64 = 20.0;
const SETTINGS_BOTTOM_MARGIN: f64 = 20.0;
// ウィンドウ下部、行の並びとは別に「デフォルトに戻す」「お試し表示」「保存」ボタンを置くための領域
const SETTINGS_BUTTON_AREA_HEIGHT: f64 = 44.0;
const SETTINGS_BUTTON_WIDTH: f64 = 130.0;
const SETTINGS_BUTTON_HEIGHT: f64 = 24.0;
const SETTINGS_BUTTON_GAP: f64 = 12.0;
const SETTINGS_BUTTON_RIGHT_MARGIN: f64 = 20.0;
/// 設定タブのペインの高さ。固定。行が増えたら documentView がスクロールする（#29）。
/// ウィンドウ自体の高さは `NSTabViewController` が選択中のペインに合わせて決める。
pub(super) const SETTINGS_PANE_HEIGHT: f64 = 542.0;
/// ペイン下部、行スタックとは別に確保された絵文字メッセージ・ボタン領域の高さ。
/// この帯は documentView に含めず、ペイン直下に固定表示する。
const SETTINGS_FOOTER_HEIGHT: f64 =
    SETTINGS_EMOJI_MESSAGE_HEIGHT + SETTINGS_BUTTON_AREA_HEIGHT + SETTINGS_BOTTOM_MARGIN;
/// フッターを除いた、NSScrollView が占める領域の高さ。
const SETTINGS_SCROLL_HEIGHT: f64 = SETTINGS_PANE_HEIGHT - SETTINGS_FOOTER_HEIGHT;

/// 行数から documentView の高さを算出する。`SETTINGS_SCROLL_HEIGHT` を下回らないように
/// する（`.max` を外すと、行数が減ったとき非 flipped の documentView がスクロール領域の
/// 下端に張り付き、上部に空白ができる）。
fn document_height_for(row_count: usize) -> f64 {
    (SETTINGS_TOP_MARGIN + SETTINGS_ROW_HEIGHT * row_count as f64).max(SETTINGS_SCROLL_HEIGHT)
}

/// 行スタックを収める documentView の高さ。
pub(super) fn document_height() -> f64 {
    document_height_for(SETTINGS_TOTAL_ROW_COUNT)
}

const SETTINGS_LABEL_X: f64 = 20.0;
const SETTINGS_LABEL_WIDTH: f64 = 270.0;
const SETTINGS_LABEL_HEIGHT: f64 = 17.0;
const SETTINGS_CONTROL_X: f64 = 296.0;
const SETTINGS_CONTROL_HEIGHT: f64 = 20.0;
const SETTINGS_SLIDER_WIDTH: f64 = 150.0;
const SETTINGS_VALUE_LABEL_X: f64 = 452.0;
const SETTINGS_VALUE_LABEL_WIDTH: f64 = 48.0;
const SETTINGS_FIELD_WIDTH: f64 = 80.0;
const SETTINGS_STEPPER_X: f64 = SETTINGS_CONTROL_X + SETTINGS_FIELD_WIDTH + 6.0;
const SETTINGS_STEPPER_WIDTH: f64 = 19.0;
const SETTINGS_POPUP_WIDTH: f64 = 200.0;
const SETTINGS_EMOJI_FIELD_WIDTH: f64 = 120.0;
// コントロール列の右端。幅が固定でないコントロールはここに右端を合わせる
const SETTINGS_CONTROL_RIGHT_X: f64 = SETTINGS_CONTROL_X + SETTINGS_POPUP_WIDTH;
// 絵文字フィールドの直下に表示するバリデーションメッセージの高さ。行スタックとボタン領域の間に確保する
const SETTINGS_EMOJI_MESSAGE_HEIGHT: f64 = 16.0;

/// 背景をクリックしたときに編集中のテキストフィールドを確定させるためのビュー。
///
/// AppKit の既定では、コントロールの無い場所をクリックしても first responder は外れず、
/// フィールドが編集中のまま残る。`mouseDown:` を受けて明示的に手放す。
fn background_view_class() -> &'static AnyClass {
    static ONCE: Once = Once::new();
    static mut CLASS: *const AnyClass = ptr::null();

    ONCE.call_once(|| unsafe {
        let mut builder = ClassBuilder::new("CliipShowSettingsBackgroundView", class!(NSView))
            .expect("settings background view class creation failed");
        builder.add_method(
            sel!(mouseDown:),
            background_mouse_down as extern "C" fn(_, _, _),
        );
        CLASS = builder.register() as *const AnyClass;
    });

    unsafe { &*CLASS }
}

extern "C" fn background_mouse_down(this: &AnyObject, _: Sel, _: *mut AnyObject) {
    unsafe {
        let window: *mut AnyObject = msg_send![this, window];
        if window.is_null() {
            return;
        }
        // 確定に伴い settingChanged: が飛ぶが、ロックは取っていないので再入の心配はない
        let _: bool = msg_send![window, makeFirstResponder: ptr::null_mut::<AnyObject>()];
    }
}

fn row_bottom_y(index: usize) -> f64 {
    let row_top = document_height() - SETTINGS_TOP_MARGIN - (index as f64) * SETTINGS_ROW_HEIGHT;
    row_top - SETTINGS_ROW_HEIGHT
}

fn centered_rect(x: f64, width: f64, height: f64, row_bottom: f64) -> NSRect {
    NSRect {
        origin: NSPoint {
            x,
            y: row_bottom + (SETTINGS_ROW_HEIGHT - height) / 2.0,
        },
        size: NSSize { width, height },
    }
}

pub(super) unsafe fn make_label(text: &str, frame: NSRect) -> *mut AnyObject {
    let label: *mut AnyObject = msg_send![class!(NSTextField), alloc];
    let label: *mut AnyObject = msg_send![label, initWithFrame: frame];
    let () = msg_send![label, setBezeled: false];
    let () = msg_send![label, setBordered: false];
    let () = msg_send![label, setEditable: false];
    let () = msg_send![label, setSelectable: false];
    let () = msg_send![label, setDrawsBackground: false];
    set_string_value(label, text);
    label
}

pub(super) unsafe fn set_string_value(control: *mut AnyObject, text: &str) {
    let ns = NSString::from_str(text);
    let () = msg_send![control, setStringValue: &*ns];
}

unsafe fn make_slider(
    min: f64,
    max: f64,
    value: f64,
    tag: isize,
    frame: NSRect,
    delegate: &AnyObject,
) -> *mut AnyObject {
    let slider: *mut AnyObject = msg_send![class!(NSSlider), alloc];
    let slider: *mut AnyObject = msg_send![slider, initWithFrame: frame];
    let () = msg_send![slider, setMinValue: min];
    let () = msg_send![slider, setMaxValue: max];
    let () = msg_send![slider, setDoubleValue: value];
    let () = msg_send![slider, setTag: tag];
    let () = msg_send![slider, setTarget: delegate];
    let () = msg_send![slider, setAction: sel!(settingChanged:)];
    // NSSlider は既定で continuous=YES のため、指定しないとドラッグ中に action が連発し
    // その都度ファイル保存が走る。mouse-up 時の 1 回だけに寄せる。
    let () = msg_send![slider, setContinuous: false];
    slider
}

unsafe fn make_editable_field(
    text: &str,
    tag: isize,
    frame: NSRect,
    delegate: &AnyObject,
) -> *mut AnyObject {
    let field: *mut AnyObject = msg_send![class!(NSTextField), alloc];
    let field: *mut AnyObject = msg_send![field, initWithFrame: frame];
    set_string_value(field, text);
    let () = msg_send![field, setEditable: true];
    let () = msg_send![field, setBezeled: true];
    let () = msg_send![field, setTag: tag];
    let () = msg_send![field, setTarget: delegate];
    let () = msg_send![field, setAction: sel!(settingChanged:)];
    field
}

unsafe fn make_stepper(
    min: f64,
    max: f64,
    value: f64,
    tag: isize,
    frame: NSRect,
    delegate: &AnyObject,
) -> *mut AnyObject {
    let stepper: *mut AnyObject = msg_send![class!(NSStepper), alloc];
    let stepper: *mut AnyObject = msg_send![stepper, initWithFrame: frame];
    let () = msg_send![stepper, setMinValue: min];
    let () = msg_send![stepper, setMaxValue: max];
    let () = msg_send![stepper, setDoubleValue: value];
    let () = msg_send![stepper, setTag: tag];
    let () = msg_send![stepper, setTarget: delegate];
    let () = msg_send![stepper, setAction: sel!(settingChanged:)];
    stepper
}

/// 言語ポップアップを作る。
///
/// 他のポップアップは表示タイトルがそのまま設定値だが、言語だけは表示ラベルと設定値が
/// 別（「システムに合わせる」に対して `auto`）。そのためタイトル一致では往復できず、
/// `LANGUAGE_CHOICES` の並び順とインデックスで対応づける。
unsafe fn make_language_popup(
    lang: Lang,
    selected: LanguageSetting,
    tag: isize,
    frame: NSRect,
    delegate: &AnyObject,
) -> *mut AnyObject {
    let popup: *mut AnyObject = msg_send![class!(NSPopUpButton), alloc];
    let popup: *mut AnyObject = msg_send![popup, initWithFrame: frame pullsDown: false];
    for setting in i18n::LANGUAGE_CHOICES {
        let ns = NSString::from_str(i18n::language_label(lang, setting));
        let () = msg_send![popup, addItemWithTitle: &*ns];
    }
    select_language_item(popup, selected);
    let () = msg_send![popup, setTag: tag];
    let () = msg_send![popup, setTarget: delegate];
    let () = msg_send![popup, setAction: sel!(settingChanged:)];
    popup
}

/// 言語ポップアップの選択を `setting` に合わせる。
pub(super) unsafe fn select_language_item(popup: *mut AnyObject, setting: LanguageSetting) {
    if popup.is_null() {
        return;
    }
    let index = i18n::LANGUAGE_CHOICES
        .iter()
        .position(|choice| *choice == setting)
        .unwrap_or(0);
    let () = msg_send![popup, selectItemAtIndex: index as isize];
}

unsafe fn make_popup(
    items: &[&str],
    selected: &str,
    tag: isize,
    frame: NSRect,
    delegate: &AnyObject,
) -> *mut AnyObject {
    let popup: *mut AnyObject = msg_send![class!(NSPopUpButton), alloc];
    let popup: *mut AnyObject = msg_send![popup, initWithFrame: frame pullsDown: false];
    for item in items {
        let ns = NSString::from_str(item);
        let () = msg_send![popup, addItemWithTitle: &*ns];
    }
    select_popup_item(popup, selected);
    let () = msg_send![popup, setTag: tag];
    let () = msg_send![popup, setTarget: delegate];
    let () = msg_send![popup, setAction: sel!(settingChanged:)];
    popup
}

pub(super) unsafe fn select_popup_item(popup: *mut AnyObject, title: &str) {
    let ns = NSString::from_str(title);
    let () = msg_send![popup, selectItemWithTitle: &*ns];
}

#[allow(clippy::too_many_arguments)]
pub(super) unsafe fn add_slider_row(
    document_view: *mut AnyObject,
    delegate: &AnyObject,
    index: usize,
    lang: Lang,
    label_msg: Msg,
    key: ConfigKey,
    min: f64,
    max: f64,
    value: f64,
    localized: &mut Vec<LocalizedControl>,
) -> (*mut AnyObject, *mut AnyObject) {
    let row_bottom = row_bottom_y(index);
    let label_rect = centered_rect(
        SETTINGS_LABEL_X,
        SETTINGS_LABEL_WIDTH,
        SETTINGS_LABEL_HEIGHT,
        row_bottom,
    );
    let slider_rect = centered_rect(
        SETTINGS_CONTROL_X,
        SETTINGS_SLIDER_WIDTH,
        SETTINGS_CONTROL_HEIGHT,
        row_bottom,
    );
    let value_rect = centered_rect(
        SETTINGS_VALUE_LABEL_X,
        SETTINGS_VALUE_LABEL_WIDTH,
        SETTINGS_LABEL_HEIGHT,
        row_bottom,
    );

    let label = make_label(i18n::text(lang, label_msg), label_rect);
    let slider = make_slider(
        min,
        max,
        value,
        config_key_to_tag(key),
        slider_rect,
        delegate,
    );
    let value_label = make_label(&format!("{value:.2}"), value_rect);

    let () = msg_send![document_view, addSubview: label];
    let () = msg_send![document_view, addSubview: slider];
    let () = msg_send![document_view, addSubview: value_label];

    localized.push(LocalizedControl {
        control: label,
        msg: label_msg,
        kind: LocalizedKind::StringValue,
    });

    (slider, value_label)
}

#[allow(clippy::too_many_arguments)]
pub(super) unsafe fn add_stepper_row(
    document_view: *mut AnyObject,
    delegate: &AnyObject,
    index: usize,
    lang: Lang,
    label_msg: Msg,
    key: ConfigKey,
    min: usize,
    max: usize,
    value: usize,
    localized: &mut Vec<LocalizedControl>,
) -> (*mut AnyObject, *mut AnyObject) {
    let row_bottom = row_bottom_y(index);
    let label_rect = centered_rect(
        SETTINGS_LABEL_X,
        SETTINGS_LABEL_WIDTH,
        SETTINGS_LABEL_HEIGHT,
        row_bottom,
    );
    let field_rect = centered_rect(
        SETTINGS_CONTROL_X,
        SETTINGS_FIELD_WIDTH,
        SETTINGS_CONTROL_HEIGHT,
        row_bottom,
    );
    let stepper_rect = centered_rect(
        SETTINGS_STEPPER_X,
        SETTINGS_STEPPER_WIDTH,
        SETTINGS_CONTROL_HEIGHT,
        row_bottom,
    );

    let tag = config_key_to_tag(key);
    let label = make_label(i18n::text(lang, label_msg), label_rect);
    let field = make_editable_field(&value.to_string(), tag, field_rect, delegate);
    let stepper = make_stepper(
        min as f64,
        max as f64,
        value as f64,
        tag,
        stepper_rect,
        delegate,
    );

    let () = msg_send![document_view, addSubview: label];
    let () = msg_send![document_view, addSubview: field];
    let () = msg_send![document_view, addSubview: stepper];

    localized.push(LocalizedControl {
        control: label,
        msg: label_msg,
        kind: LocalizedKind::StringValue,
    });

    (field, stepper)
}

#[allow(clippy::too_many_arguments)]
pub(super) unsafe fn add_popup_row(
    document_view: *mut AnyObject,
    delegate: &AnyObject,
    index: usize,
    lang: Lang,
    label_msg: Msg,
    key: ConfigKey,
    items: &[&str],
    selected: &str,
    localized: &mut Vec<LocalizedControl>,
) -> *mut AnyObject {
    let row_bottom = row_bottom_y(index);
    let label_rect = centered_rect(
        SETTINGS_LABEL_X,
        SETTINGS_LABEL_WIDTH,
        SETTINGS_LABEL_HEIGHT,
        row_bottom,
    );
    let popup_rect = centered_rect(
        SETTINGS_CONTROL_X,
        SETTINGS_POPUP_WIDTH,
        SETTINGS_CONTROL_HEIGHT,
        row_bottom,
    );

    let label = make_label(i18n::text(lang, label_msg), label_rect);
    let popup = make_popup(
        items,
        selected,
        config_key_to_tag(key),
        popup_rect,
        delegate,
    );

    let () = msg_send![document_view, addSubview: label];
    let () = msg_send![document_view, addSubview: popup];

    localized.push(LocalizedControl {
        control: label,
        msg: label_msg,
        kind: LocalizedKind::StringValue,
    });

    popup
}

/// 言語専用のポップアップ行。表示ラベルと設定値が別なので `add_popup_row` とは
/// ポップアップの作り方だけが違う。
pub(super) unsafe fn add_language_row(
    document_view: *mut AnyObject,
    delegate: &AnyObject,
    index: usize,
    lang: Lang,
    selected: LanguageSetting,
    localized: &mut Vec<LocalizedControl>,
) -> *mut AnyObject {
    let row_bottom = row_bottom_y(index);
    let label_rect = centered_rect(
        SETTINGS_LABEL_X,
        SETTINGS_LABEL_WIDTH,
        SETTINGS_LABEL_HEIGHT,
        row_bottom,
    );
    let popup_rect = centered_rect(
        SETTINGS_CONTROL_X,
        SETTINGS_POPUP_WIDTH,
        SETTINGS_CONTROL_HEIGHT,
        row_bottom,
    );

    let label = make_label(i18n::text(lang, Msg::LabelLanguage), label_rect);
    let popup = make_language_popup(
        lang,
        selected,
        config_key_to_tag(ConfigKey::Language),
        popup_rect,
        delegate,
    );

    let () = msg_send![document_view, addSubview: label];
    let () = msg_send![document_view, addSubview: popup];

    localized.push(LocalizedControl {
        control: label,
        msg: Msg::LabelLanguage,
        kind: LocalizedKind::StringValue,
    });

    popup
}

/// 絵文字フィールド専用の行。入力中バリデーションのため `controlTextDidChange:` を
/// 受け取れるよう `setDelegate:` し、メッセージラベルを追加する。ラベルの位置は
/// 行の並びではなくウィンドウ下端が基準（`SETTINGS_EMOJI_MESSAGE_HEIGHT`）。
///
/// 行（label/field）はスクロールする documentView（`document_view`）へ、
/// メッセージラベルはスクロールしないペイン（`pane_view`）へ addSubview する。
#[allow(clippy::too_many_arguments)]
pub(super) unsafe fn add_field_row(
    document_view: *mut AnyObject,
    pane_view: *mut AnyObject,
    delegate: &AnyObject,
    index: usize,
    lang: Lang,
    label_msg: Msg,
    key: ConfigKey,
    value: &str,
    localized: &mut Vec<LocalizedControl>,
) -> (*mut AnyObject, *mut AnyObject) {
    let row_bottom = row_bottom_y(index);
    let label_rect = centered_rect(
        SETTINGS_LABEL_X,
        SETTINGS_LABEL_WIDTH,
        SETTINGS_LABEL_HEIGHT,
        row_bottom,
    );
    let field_rect = centered_rect(
        SETTINGS_CONTROL_X,
        SETTINGS_EMOJI_FIELD_WIDTH,
        SETTINGS_CONTROL_HEIGHT,
        row_bottom,
    );
    // 行スタックの下、ボタン領域の上に確保された専用の帯にメッセージを出す
    let message_rect = NSRect {
        origin: NSPoint {
            x: SETTINGS_LABEL_X,
            y: SETTINGS_BOTTOM_MARGIN + SETTINGS_BUTTON_AREA_HEIGHT,
        },
        size: NSSize {
            width: SETTINGS_WINDOW_WIDTH - SETTINGS_LABEL_X - SETTINGS_BUTTON_RIGHT_MARGIN,
            height: SETTINGS_EMOJI_MESSAGE_HEIGHT,
        },
    };

    let label = make_label(i18n::text(lang, label_msg), label_rect);
    let field = make_editable_field(value, config_key_to_tag(key), field_rect, delegate);
    let () = msg_send![field, setDelegate: delegate];

    let message_label = make_label("", message_rect);
    let red: *mut AnyObject = msg_send![class!(NSColor), systemRedColor];
    let () = msg_send![message_label, setTextColor: red];

    let () = msg_send![document_view, addSubview: label];
    let () = msg_send![document_view, addSubview: field];
    let () = msg_send![pane_view, addSubview: message_label];

    localized.push(LocalizedControl {
        control: label,
        msg: label_msg,
        kind: LocalizedKind::StringValue,
    });

    (field, message_label)
}

// NSControlStateValue
const NS_CONTROL_STATE_VALUE_OFF: isize = 0;
const NS_CONTROL_STATE_VALUE_ON: isize = 1;
// NSControlSize: small
const NS_CONTROL_SIZE_SMALL: usize = 1;

/// 他の設定行（下書き→保存モデル）とログイン項目（OS へ即時反映）を見た目で区切るための
/// 区切り線。新規の ObjC API を増やさないよう、罫線文字のラベルで表現する。
pub(super) unsafe fn add_divider_row(document_view: *mut AnyObject, index: usize) {
    let row_bottom = row_bottom_y(index);
    let rect = centered_rect(
        SETTINGS_LABEL_X,
        SETTINGS_WINDOW_WIDTH - SETTINGS_LABEL_X - SETTINGS_BUTTON_RIGHT_MARGIN,
        SETTINGS_LABEL_HEIGHT,
        row_bottom,
    );
    let divider = make_label("────────────────────────────────────────────────────", rect);
    let gray: *mut AnyObject = msg_send![class!(NSColor), separatorColor];
    let () = msg_send![divider, setTextColor: gray];
    let () = msg_send![document_view, addSubview: divider];
}

/// ログイン時の自動起動の行。`enabled` は表示専用の初期値で、
/// 実際の値はウィンドウを開くたびに `sync_login_item_toggle` が上書きする。
pub(super) unsafe fn add_login_item_row(
    document_view: *mut AnyObject,
    delegate: &AnyObject,
    index: usize,
    lang: Lang,
    enabled: bool,
    localized: &mut Vec<LocalizedControl>,
) -> *mut AnyObject {
    let row_bottom = row_bottom_y(index);
    let label_rect = centered_rect(
        SETTINGS_LABEL_X,
        SETTINGS_LABEL_WIDTH,
        SETTINGS_LABEL_HEIGHT,
        row_bottom,
    );
    let label = make_label(i18n::text(lang, Msg::SettingsStartAtLogin), label_rect);
    let toggle: *mut AnyObject = msg_send![class!(NSSwitch), alloc];
    let toggle: *mut AnyObject = msg_send![toggle, init];
    let () = msg_send![toggle, setControlSize: NS_CONTROL_SIZE_SMALL];
    // NSSwitch は自前の寸法を持ち、こちらで渡したフレームの大きさに従わない。
    // 実寸を読んでから、他の行のコントロールと右端が揃う位置に置く。
    let size: NSSize = msg_send![toggle, intrinsicContentSize];
    let toggle_rect = centered_rect(
        SETTINGS_CONTROL_RIGHT_X - size.width,
        size.width,
        size.height,
        row_bottom,
    );
    let () = msg_send![toggle, setFrame: toggle_rect];
    let () = msg_send![toggle, setState: login_item_control_state(enabled)];
    let () = msg_send![toggle, setTarget: delegate];
    let () = msg_send![toggle, setAction: sel!(toggleLoginItem:)];

    let () = msg_send![document_view, addSubview: label];
    let () = msg_send![document_view, addSubview: toggle];

    localized.push(LocalizedControl {
        control: label,
        msg: Msg::SettingsStartAtLogin,
        kind: LocalizedKind::StringValue,
    });

    toggle
}

pub(super) fn login_item_control_state(enabled: bool) -> isize {
    if enabled {
        NS_CONTROL_STATE_VALUE_ON
    } else {
        NS_CONTROL_STATE_VALUE_OFF
    }
}

/// タブ 1 枚分のビュー。余白クリックでテキストフィールドの編集を確定させる動きを
/// タブごとに保つため、どのペインも `background_view_class` で作る。
pub(super) unsafe fn make_pane_view(height: f64) -> *mut AnyObject {
    let rect = NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size: NSSize {
            width: SETTINGS_WINDOW_WIDTH,
            height,
        },
    };
    let view: *mut AnyObject = msg_send![background_view_class(), alloc];
    msg_send![view, initWithFrame: rect]
}

// NSBorderType: NSNoBorder
const NS_NO_BORDER: usize = 0;

/// 行スタック（`document_view`）をスクロール表示するための NSScrollView を作る。
/// フレームはペイン内でフッターを除いた領域（`SETTINGS_FOOTER_HEIGHT` の上）に固定し、
/// 初期スクロール位置は最上行（documentView の上端）に合わせる。
pub(super) unsafe fn make_scroll_view(document_view: *mut AnyObject) -> *mut AnyObject {
    let frame = NSRect {
        origin: NSPoint {
            x: 0.0,
            y: SETTINGS_FOOTER_HEIGHT,
        },
        size: NSSize {
            width: SETTINGS_WINDOW_WIDTH,
            height: SETTINGS_SCROLL_HEIGHT,
        },
    };
    let scroll: *mut AnyObject = msg_send![class!(NSScrollView), alloc];
    let scroll: *mut AnyObject = msg_send![scroll, initWithFrame: frame];
    let () = msg_send![scroll, setBorderType: NS_NO_BORDER];
    let () = msg_send![scroll, setDrawsBackground: false];
    let () = msg_send![scroll, setHasVerticalScroller: true];
    let () = msg_send![scroll, setHasHorizontalScroller: false];
    let () = msg_send![scroll, setAutohidesScrollers: true];
    let () = msg_send![scroll, setDocumentView: document_view];
    // document_height() を再計算せず、documentView 自身の実寸から上端を求める
    // （高さの二重管理を避ける）。非 flipped の documentView では、可視領域の上端に
    // documentView の上端を合わせる y はこの差分になる。
    let document_frame: NSRect = msg_send![document_view, frame];
    let () = msg_send![
        document_view,
        scrollPoint: NSPoint {
            x: 0.0,
            y: document_frame.size.height - SETTINGS_SCROLL_HEIGHT,
        }
    ];
    scroll
}

/// ボタンは右寄せで、右端が「保存」になるよう逆順に座標を計算する。
pub(super) unsafe fn add_button_row(
    content_view: *mut AnyObject,
    delegate: &AnyObject,
    lang: Lang,
    localized: &mut Vec<LocalizedControl>,
) {
    let y = SETTINGS_BOTTOM_MARGIN + (SETTINGS_BUTTON_AREA_HEIGHT - SETTINGS_BUTTON_HEIGHT) / 2.0;
    let save_x = SETTINGS_WINDOW_WIDTH - SETTINGS_BUTTON_RIGHT_MARGIN - SETTINGS_BUTTON_WIDTH;
    let preview_x = save_x - SETTINGS_BUTTON_GAP - SETTINGS_BUTTON_WIDTH;
    let reset_x = preview_x - SETTINGS_BUTTON_GAP - SETTINGS_BUTTON_WIDTH;
    let button_size = NSSize {
        width: SETTINGS_BUTTON_WIDTH,
        height: SETTINGS_BUTTON_HEIGHT,
    };

    let reset_button = make_button(
        i18n::text(lang, Msg::ButtonRestoreDefaults),
        sel!(resetSettings:),
        "",
        NSRect {
            origin: NSPoint { x: reset_x, y },
            size: button_size,
        },
        delegate,
    );
    let preview_button = make_button(
        i18n::text(lang, Msg::ButtonPreview),
        sel!(previewSettings:),
        "",
        NSRect {
            origin: NSPoint { x: preview_x, y },
            size: button_size,
        },
        delegate,
    );
    // Enter で発火するデフォルトボタンにする
    let save_button = make_button(
        i18n::text(lang, Msg::ButtonSave),
        sel!(saveSettings:),
        "\r",
        NSRect {
            origin: NSPoint { x: save_x, y },
            size: button_size,
        },
        delegate,
    );

    let () = msg_send![content_view, addSubview: reset_button];
    let () = msg_send![content_view, addSubview: preview_button];
    let () = msg_send![content_view, addSubview: save_button];

    localized.push(LocalizedControl {
        control: reset_button,
        msg: Msg::ButtonRestoreDefaults,
        kind: LocalizedKind::Title,
    });
    localized.push(LocalizedControl {
        control: preview_button,
        msg: Msg::ButtonPreview,
        kind: LocalizedKind::Title,
    });
    localized.push(LocalizedControl {
        control: save_button,
        msg: Msg::ButtonSave,
        kind: LocalizedKind::Title,
    });
}

pub(super) unsafe fn make_button(
    title: &str,
    action: Sel,
    key_equivalent: &str,
    frame: NSRect,
    delegate: &AnyObject,
) -> *mut AnyObject {
    let button: *mut AnyObject = msg_send![class!(NSButton), alloc];
    let button: *mut AnyObject = msg_send![button, initWithFrame: frame];
    let title_ns = NSString::from_str(title);
    let () = msg_send![button, setTitle: &*title_ns];
    let () = msg_send![button, setTarget: delegate];
    let () = msg_send![button, setAction: action];
    if !key_equivalent.is_empty() {
        let key_ns = NSString::from_str(key_equivalent);
        let () = msg_send![button, setKeyEquivalent: &*key_ns];
    }
    button
}

#[cfg(test)]
mod tests {
    use super::{
        document_height, document_height_for, row_bottom_y, SETTINGS_FOOTER_HEIGHT,
        SETTINGS_SCROLL_HEIGHT, SETTINGS_TOTAL_ROW_COUNT,
    };

    // documentView 相対の行座標にフッター高を足すと、スクロール化前のペイン絶対座標に
    // 戻る。行スタックとフッターの取り違え（80pt ずれ）が起きるとここで落ちる
    #[test]
    fn row_bottom_y_plus_footer_matches_pre_scroll_first_row_top() {
        assert_eq!(row_bottom_y(0) + SETTINGS_FOOTER_HEIGHT, 488.0);
    }

    #[test]
    fn row_bottom_y_plus_footer_matches_pre_scroll_last_row_bottom() {
        assert_eq!(
            row_bottom_y(SETTINGS_TOTAL_ROW_COUNT - 1) + SETTINGS_FOOTER_HEIGHT,
            80.0
        );
    }

    #[test]
    fn row_bottom_y_reaches_document_bottom_for_last_row() {
        assert_eq!(row_bottom_y(SETTINGS_TOTAL_ROW_COUNT - 1), 0.0);
    }

    // 行を足してこれが落ちたらスクロールが現れる = 設定ペインのベースライン更新と
    // documentView 撮影モードの追加が必要。あわせて絵文字バリデーションメッセージの
    // 置き場所（フッター固定のままか、フィールド直下に移すか）も決めること
    #[test]
    fn document_height_still_fits_without_scrolling() {
        assert_eq!(document_height(), SETTINGS_SCROLL_HEIGHT);
    }

    #[test]
    fn document_height_for_grows_past_scroll_height_when_rows_overflow() {
        assert!(document_height_for(SETTINGS_TOTAL_ROW_COUNT + 5) > SETTINGS_SCROLL_HEIGHT);
    }

    #[test]
    fn document_height_for_clamps_to_scroll_height_when_rows_are_few() {
        assert_eq!(document_height_for(1), SETTINGS_SCROLL_HEIGHT);
    }
}
