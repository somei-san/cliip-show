use std::ptr;
use std::sync::Once;

use objc2::declare::ClassBuilder;
use objc2::runtime::{AnyClass, AnyObject, Sel};
use objc2::{class, msg_send, sel};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::config::{ConfigKey, LanguageSetting};
use crate::i18n::{self, Lang, Msg};

use super::range_hint::range_hint_text;
use super::tooltip::tooltip_text;
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
/// ペイン下部、行スタックとは別に確保されたボタン領域の高さ。
/// この帯は documentView に含めず、ペイン直下に固定表示する。
const SETTINGS_FOOTER_HEIGHT: f64 = SETTINGS_BUTTON_AREA_HEIGHT + SETTINGS_BOTTOM_MARGIN;
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
// 数値欄（ステッパー行）の右に添える有効範囲の補助テキスト。右端はコントロール列の右端に揃える
const SETTINGS_NUMERIC_HINT_X: f64 = SETTINGS_STEPPER_X + SETTINGS_STEPPER_WIDTH + 8.0;
const SETTINGS_NUMERIC_HINT_WIDTH: f64 = SETTINGS_CONTROL_RIGHT_X - SETTINGS_NUMERIC_HINT_X;
const SETTINGS_NUMERIC_HINT_HEIGHT: f64 = 14.0;
// NSFont の smallSystemFontSize 相当
const SETTINGS_NUMERIC_HINT_FONT_SIZE: f64 = 11.0;
// 絵文字欄の右隣に置くヘルプボタンの x 座標
const SETTINGS_EMOJI_HELP_BUTTON_X: f64 = SETTINGS_CONTROL_X + SETTINGS_EMOJI_FIELD_WIDTH + 6.0;
const SETTINGS_EMOJI_HELP_BUTTON_SIZE: f64 = 16.0;
// SF Symbols のシンボル名。ロード不可（macOS 11 未満）のときは NS_BEZEL_STYLE_HELP_BUTTON の
// 従来ボタンにフォールバックする（make_help_button 参照）。
const EMOJI_HELP_SYMBOL_NAME: &str = "questionmark.circle";
// ヘルプボタンの popover 本文（NSViewController の view）の大きさ。ja/en とも複数文にわたる
// ため、行の幅（270pt）より広めに取り、内側にラベル用の余白を残す。
const EMOJI_HELP_POPOVER_WIDTH: f64 = 340.0;
const EMOJI_HELP_POPOVER_HEIGHT: f64 = 120.0;
const EMOJI_HELP_POPOVER_INSET: f64 = 12.0;
// NSLineBreakMode: ByWordWrapping
const NS_LINE_BREAK_BY_WORD_WRAPPING: isize = 0;
// NSBezelStyleHelpButton
const NS_BEZEL_STYLE_HELP_BUTTON: usize = 9;
// NSButton.ImagePosition: imageOnly（画像のみ表示。タイトルは空文字のため実質無関係だが、
// レイアウト計算を素直にするため明示する）
const NS_IMAGE_ONLY: isize = 1;
// NSPopoverBehavior: transient（popover の外をクリックすると自動で閉じる）
const NS_POPOVER_BEHAVIOR_TRANSIENT: isize = 1;

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

/// `make_label` の亜種。数値欄の有効範囲のような、本文ラベルより控えめに見せたい
/// 補助テキスト用に、小さいシステムフォント（`smallSystemFontSize` 相当）と
/// セカンダリカラーを当てる。
unsafe fn make_hint_label(text: &str, frame: NSRect) -> *mut AnyObject {
    let label = make_label(text, frame);
    let font: *mut AnyObject =
        msg_send![class!(NSFont), systemFontOfSize: SETTINGS_NUMERIC_HINT_FONT_SIZE];
    if !font.is_null() {
        let () = msg_send![label, setFont: font];
    }
    let color: *mut AnyObject = msg_send![class!(NSColor), secondaryLabelColor];
    let () = msg_send![label, setTextColor: color];
    label
}

pub(super) unsafe fn set_string_value(control: *mut AnyObject, text: &str) {
    let ns = NSString::from_str(text);
    let () = msg_send![control, setStringValue: &*ns];
}

unsafe fn set_tool_tip(control: *mut AnyObject, text: &str) {
    let ns = NSString::from_str(text);
    let () = msg_send![control, setToolTip: &*ns];
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

/// 編集可能な `NSTextField` を作る。`setDelegate:` により `controlTextDidChange:`
/// （入力中フィルタ・バリデーション）を受け取れるようにする。
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
    let () = msg_send![field, setDelegate: delegate];
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
    tooltip_msg: Msg,
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
    let tooltip = tooltip_text(lang, tooltip_msg);
    set_tool_tip(label, &tooltip);
    set_tool_tip(slider, &tooltip);
    let value_label = make_label(&format!("{value:.2}"), value_rect);

    let () = msg_send![document_view, addSubview: label];
    let () = msg_send![document_view, addSubview: slider];
    let () = msg_send![document_view, addSubview: value_label];

    localized.push(LocalizedControl {
        control: label,
        msg: label_msg,
        kind: LocalizedKind::StringValue,
    });
    localized.push(LocalizedControl {
        control: label,
        msg: tooltip_msg,
        kind: LocalizedKind::ToolTip,
    });
    localized.push(LocalizedControl {
        control: slider,
        msg: tooltip_msg,
        kind: LocalizedKind::ToolTip,
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
    tooltip_msg: Msg,
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
    let hint_rect = centered_rect(
        SETTINGS_NUMERIC_HINT_X,
        SETTINGS_NUMERIC_HINT_WIDTH,
        SETTINGS_NUMERIC_HINT_HEIGHT,
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
    let tooltip = tooltip_text(lang, tooltip_msg);
    set_tool_tip(label, &tooltip);
    set_tool_tip(field, &tooltip);
    set_tool_tip(stepper, &tooltip);
    // 有効範囲は言語に依存しない半角表記のため、言語切替の対象（localized）には乗せない
    let hint = make_hint_label(&range_hint_text(label_msg), hint_rect);

    let () = msg_send![document_view, addSubview: label];
    let () = msg_send![document_view, addSubview: field];
    let () = msg_send![document_view, addSubview: stepper];
    let () = msg_send![document_view, addSubview: hint];

    localized.push(LocalizedControl {
        control: label,
        msg: label_msg,
        kind: LocalizedKind::StringValue,
    });
    localized.push(LocalizedControl {
        control: label,
        msg: tooltip_msg,
        kind: LocalizedKind::ToolTip,
    });
    localized.push(LocalizedControl {
        control: field,
        msg: tooltip_msg,
        kind: LocalizedKind::ToolTip,
    });
    localized.push(LocalizedControl {
        control: stepper,
        msg: tooltip_msg,
        kind: LocalizedKind::ToolTip,
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

/// 絵文字欄の右隣に置く、控えめなヘルプボタン（`?`）を作る。SF Symbols の
/// `questionmark.circle` をボーダレスボタンの画像として使い、macOS 標準の丸いベゼル付き
/// ヘルプボタンより小さく目立たない見た目にする。SF Symbols がロードできない環境
/// （macOS 11 未満）では、従来のベゼル付きヘルプボタンにフォールバックする。
unsafe fn make_help_button(delegate: &AnyObject, row_bottom: f64) -> *mut AnyObject {
    let button: *mut AnyObject = msg_send![class!(NSButton), alloc];
    let button: *mut AnyObject = msg_send![button, init];
    let title = NSString::from_str("");
    let () = msg_send![button, setTitle: &*title];
    let () = msg_send![button, setTarget: delegate];
    let () = msg_send![button, setAction: sel!(showEmojiHelp:)];

    let symbol_name = NSString::from_str(EMOJI_HELP_SYMBOL_NAME);
    let image: *mut AnyObject = msg_send![
        class!(NSImage),
        imageWithSystemSymbolName: &*symbol_name
        accessibilityDescription: ptr::null_mut::<AnyObject>()
    ];
    let rect = if image.is_null() {
        eprintln!(
            "warning: SF Symbol \"{EMOJI_HELP_SYMBOL_NAME}\" is unavailable; \
             falling back to the default help button"
        );
        let () = msg_send![button, setBezelStyle: NS_BEZEL_STYLE_HELP_BUTTON];
        let size: NSSize = msg_send![button, intrinsicContentSize];
        centered_rect(
            SETTINGS_EMOJI_HELP_BUTTON_X,
            size.width,
            size.height,
            row_bottom,
        )
    } else {
        let () = msg_send![button, setImage: image];
        let () = msg_send![button, setBordered: false];
        let () = msg_send![button, setImagePosition: NS_IMAGE_ONLY];
        let tint: *mut AnyObject = msg_send![class!(NSColor), secondaryLabelColor];
        let () = msg_send![button, setContentTintColor: tint];
        centered_rect(
            SETTINGS_EMOJI_HELP_BUTTON_X,
            SETTINGS_EMOJI_HELP_BUTTON_SIZE,
            SETTINGS_EMOJI_HELP_BUTTON_SIZE,
            row_bottom,
        )
    };
    let () = msg_send![button, setFrame: rect];
    button
}

/// ヘルプボタンの `NSPopover` と、その本文ラベルを作る。返り値の `label` は
/// `contentViewController.view` の子として popover が所有するため、popover 自身を
/// 手放さない限り生存する（`SettingsControls::hud_emoji_help_popover` のドキュメント参照）。
unsafe fn make_help_popover(text: &str) -> (*mut AnyObject, *mut AnyObject) {
    let label_rect = NSRect {
        origin: NSPoint {
            x: EMOJI_HELP_POPOVER_INSET,
            y: EMOJI_HELP_POPOVER_INSET,
        },
        size: NSSize {
            width: EMOJI_HELP_POPOVER_WIDTH - EMOJI_HELP_POPOVER_INSET * 2.0,
            height: EMOJI_HELP_POPOVER_HEIGHT - EMOJI_HELP_POPOVER_INSET * 2.0,
        },
    };
    let label = make_label(text, label_rect);
    // 支援ページのメッセージ欄と同じ理由（改行を反映させ、折り返させる）
    let () = msg_send![label, setUsesSingleLineMode: false];
    let label_cell: *mut AnyObject = msg_send![label, cell];
    let () = msg_send![label_cell, setWraps: true];
    let () = msg_send![label_cell, setLineBreakMode: NS_LINE_BREAK_BY_WORD_WRAPPING];

    let content_view: *mut AnyObject = msg_send![class!(NSView), alloc];
    let content_view: *mut AnyObject = msg_send![
        content_view,
        initWithFrame: NSRect {
            origin: NSPoint { x: 0.0, y: 0.0 },
            size: NSSize {
                width: EMOJI_HELP_POPOVER_WIDTH,
                height: EMOJI_HELP_POPOVER_HEIGHT,
            },
        }
    ];
    let () = msg_send![content_view, addSubview: label];

    let controller: *mut AnyObject = msg_send![class!(NSViewController), alloc];
    let controller: *mut AnyObject = msg_send![controller, init];
    let () = msg_send![controller, setView: content_view];
    // content_view の所有は controller に移った（addSubview 前に持っていた自前の参照を手放す）
    let () = msg_send![content_view, release];

    let popover: *mut AnyObject = msg_send![class!(NSPopover), alloc];
    let popover: *mut AnyObject = msg_send![popover, init];
    let () = msg_send![popover, setContentViewController: controller];
    // controller の所有は popover に移った
    let () = msg_send![controller, release];
    let () = msg_send![popover, setBehavior: NS_POPOVER_BEHAVIOR_TRANSIENT];

    (popover, label)
}

/// 絵文字フィールド専用の行。他のテキストフィールド行（`add_stepper_row`）と違い
/// ペア表示のコントロールを持たないため専用の関数にしている。
///
/// ツールチップ・ヘルプボタン popover の文言はどちらも `Msg::TooltipHudEmoji` 固定で、
/// 引数化していない。この関数自体が絵文字欄専用（呼び出しは build.rs に1箇所のみ）なので、
/// 引数にすると `sync.rs` の popover 直書き（`apply_language`）と二重管理になり、
/// 呼び出し側だけ別の Msg に差し替えても popover 側が追随しないバグを作れてしまう。
#[allow(clippy::too_many_arguments)]
pub(super) unsafe fn add_field_row(
    document_view: *mut AnyObject,
    delegate: &AnyObject,
    index: usize,
    lang: Lang,
    label_msg: Msg,
    key: ConfigKey,
    value: &str,
    localized: &mut Vec<LocalizedControl>,
) -> (*mut AnyObject, *mut AnyObject, *mut AnyObject) {
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

    let label = make_label(i18n::text(lang, label_msg), label_rect);
    let field = make_editable_field(value, config_key_to_tag(key), field_rect, delegate);
    let tooltip = tooltip_text(lang, Msg::TooltipHudEmoji);
    set_tool_tip(label, &tooltip);
    set_tool_tip(field, &tooltip);

    let help_button = make_help_button(delegate, row_bottom);
    let (help_popover, help_label) = make_help_popover(&tooltip);

    let () = msg_send![document_view, addSubview: label];
    let () = msg_send![document_view, addSubview: field];
    let () = msg_send![document_view, addSubview: help_button];

    localized.push(LocalizedControl {
        control: label,
        msg: label_msg,
        kind: LocalizedKind::StringValue,
    });
    localized.push(LocalizedControl {
        control: label,
        msg: Msg::TooltipHudEmoji,
        kind: LocalizedKind::ToolTip,
    });
    localized.push(LocalizedControl {
        control: field,
        msg: Msg::TooltipHudEmoji,
        kind: LocalizedKind::ToolTip,
    });

    (field, help_popover, help_label)
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

    // document_height() は SETTINGS_SCROLL_HEIGHT へ .max でクランプされるため、
    // 行スタック自身の高さ（462pt）より 16pt 大きい。非 flipped の documentView は
    // 上端を基準に積むため、この差分は最終行の下（documentView 下端）に空きとして残る。
    #[test]
    fn row_bottom_y_leaves_clamp_gap_below_last_row() {
        assert_eq!(row_bottom_y(SETTINGS_TOTAL_ROW_COUNT - 1), 16.0);
    }

    // 行を足してこれが落ちたらスクロールが現れる = 設定ペインのベースライン更新と
    // documentView 撮影モードの追加が必要
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

    /// 絵文字ヘルプボタン・popover が使う ObjC API のセレクタを突き合わせる。セレクタ名は
    /// 文字列なのでコンパイルでは食い違いを検出できず、綴りを間違えると呼び出し時に
    /// unrecognized selector で落ちる。
    #[test]
    fn appkit_responds_to_emoji_help_selectors() {
        use objc2::{class, sel};

        assert!(class!(NSButton).responds_to(sel!(setBezelStyle:)));
        assert!(class!(NSButton).responds_to(sel!(intrinsicContentSize)));
        assert!(class!(NSButton).responds_to(sel!(setImagePosition:)));
        assert!(class!(NSButton).responds_to(sel!(setContentTintColor:)));
        assert!(class!(NSImage)
            .metaclass()
            .responds_to(sel!(imageWithSystemSymbolName:accessibilityDescription:)));
        assert!(class!(NSColor)
            .metaclass()
            .responds_to(sel!(secondaryLabelColor)));
        assert!(class!(NSPopover).responds_to(sel!(setContentViewController:)));
        assert!(class!(NSPopover).responds_to(sel!(setBehavior:)));
        assert!(class!(NSPopover).responds_to(sel!(showRelativeToRect:ofView:preferredEdge:)));
    }

    /// popover 本文（`Msg::TooltipHudEmoji` の文言）が、実際に表示に使う枠
    /// （`EMOJI_HELP_POPOVER_WIDTH`/`HEIGHT` からインセットを引いた領域）に収まるかを
    /// 近似で確認する。AppKit を起動せず折返しを計算する手段が無いため、13pt システム
    /// フォントのおおよその字幅（全角 ja ≈ 13pt/字、半角 en ≈ 7pt/字）と行高（16pt）で
    /// 見積もる。実測とは誤差がありうるが、文言が伸びて明らかに入り切らなくなる変更を
    /// 検出する保険としては十分な精度とする。
    #[test]
    fn emoji_help_popover_text_fits_within_its_frame() {
        use super::super::tooltip::tooltip_text;
        use super::{
            EMOJI_HELP_POPOVER_HEIGHT, EMOJI_HELP_POPOVER_INSET, EMOJI_HELP_POPOVER_WIDTH,
        };
        use crate::i18n::{Lang, Msg};

        const CHAR_WIDTH_JA: f64 = 13.0;
        const CHAR_WIDTH_EN: f64 = 7.0;
        const LINE_HEIGHT: f64 = 16.0;

        let available_width = EMOJI_HELP_POPOVER_WIDTH - EMOJI_HELP_POPOVER_INSET * 2.0;
        let available_height = EMOJI_HELP_POPOVER_HEIGHT - EMOJI_HELP_POPOVER_INSET * 2.0;

        for (lang, char_width) in [(Lang::Ja, CHAR_WIDTH_JA), (Lang::En, CHAR_WIDTH_EN)] {
            let text = tooltip_text(lang, Msg::TooltipHudEmoji);
            let chars_per_line = (available_width / char_width).floor().max(1.0) as usize;
            // 明示的な改行（\n）は別パラグラフとして扱い、折返しを跨いで詰めない
            let estimated_lines: usize = text
                .split('\n')
                .map(|paragraph| paragraph.chars().count().div_ceil(chars_per_line).max(1))
                .sum();
            let estimated_height = estimated_lines as f64 * LINE_HEIGHT;
            assert!(
                estimated_height <= available_height,
                "{lang:?}: 見積もり高さ {estimated_height} が枠 {available_height} を超える\
                 （{estimated_lines} 行、本文: {text}）"
            );
        }
    }
}
