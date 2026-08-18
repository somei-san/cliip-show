use std::ptr;

use objc2::runtime::AnyObject;
use objc2::{class, msg_send, sel};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::config::{
    default_display_settings, ConfigKey, MAX_HUD_DURATION_SECS, MAX_HUD_FADE_DURATION_SECS,
    MAX_HUD_IMAGE_MAX_HEIGHT, MAX_HUD_SCALE, MAX_POLL_INTERVAL_SECS, MAX_TRUNCATE_MAX_LINES,
    MAX_TRUNCATE_MAX_WIDTH, MIN_HUD_DURATION_SECS, MIN_HUD_FADE_DURATION_SECS,
    MIN_HUD_IMAGE_MAX_HEIGHT, MIN_HUD_SCALE, MIN_POLL_INTERVAL_SECS, MIN_TRUNCATE_MAX_LINES,
    MIN_TRUNCATE_MAX_WIDTH,
};
use crate::hud::BACKING_BUFFERED;
use crate::i18n::{self, Lang, Msg};
use crate::objc_helpers::template_image_from_png;

use super::rows::{
    add_button_row, add_divider_row, add_field_row, add_language_row, add_login_item_row,
    add_popup_row, add_slider_row, add_stepper_row, document_height, make_button, make_label,
    make_pane_view, make_scroll_view, SETTINGS_PANE_HEIGHT, SETTINGS_STYLE_MASK,
    SETTINGS_TOTAL_ROW_COUNT, SETTINGS_WINDOW_WIDTH,
};
use super::{LocalizedControl, LocalizedKind, SettingsControls};

const GEAR_TEMPLATE_PNG: &[u8] = include_bytes!("../../assets/gear-template.png");
const BEER_TEMPLATE_PNG: &[u8] = include_bytes!("../../assets/beer-template.png");

/// タブの並び。`build_settings_window` の `add_pane_tab` の呼び出し順がこの値を決める。
/// タブを増やす・並び替えるときは呼び出し順とここを同時に更新すること
/// （VRT の `--render-settings-png` がこの index でペインを選ぶ）。
pub(crate) const TAB_INDEX_SETTINGS: isize = 0;
pub(crate) const TAB_INDEX_SUPPORT: isize = 1;
const TAB_ICON_SIZE: f64 = 18.0;
/// アイコンとタブ名の間隔。AppKit 側では詰まって見えるので、画像の下に透明な余白を足して空ける。
const TAB_ICON_BOTTOM_PADDING: f64 = 4.0;

// NSTabViewControllerTabStyle: Toolbar
const NS_TAB_VIEW_CONTROLLER_TAB_STYLE_TOOLBAR: isize = 2;
// NSLineBreakMode: ByWordWrapping
const NS_LINE_BREAK_BY_WORD_WRAPPING: isize = 0;

// 設定タブ以外のペインは中央寄せで縦に積む。要素の高さと直前との間隔を並べ、
// ペインの高さはその合計にする。
const PANE_SIDE_MARGIN: f64 = 32.0;
const PANE_TOP_MARGIN: f64 = 28.0;
const PANE_BOTTOM_MARGIN: f64 = 28.0;
const PANE_BUTTON_WIDTH: f64 = 200.0;
const PANE_BUTTON_HEIGHT: f64 = 32.0;

const SUPPORT_MESSAGE_HEIGHT: f64 = 36.0;
const SUPPORT_BUTTON_GAP: f64 = 20.0;
const SUPPORT_PANE_HEIGHT: f64 = PANE_TOP_MARGIN
    + SUPPORT_MESSAGE_HEIGHT
    + SUPPORT_BUTTON_GAP
    + PANE_BUTTON_HEIGHT
    + PANE_BOTTOM_MARGIN;

/// ペイン上端から下向きに要素を積む。`gap` は直前の要素との間隔で、`top` は
/// 積んだ分だけ下がる。左端は設定タブの行と同じく左寄せで揃える。
fn stack_down(top: &mut f64, gap: f64, height: f64, width: f64) -> NSRect {
    *top -= gap + height;
    NSRect {
        origin: NSPoint {
            x: PANE_SIDE_MARGIN,
            y: *top,
        },
        size: NSSize { width, height },
    }
}

/// `stack_down` と同じ積み方で、横位置だけペインの中央にする。
fn stack_down_centered(top: &mut f64, gap: f64, height: f64, width: f64) -> NSRect {
    let mut rect = stack_down(top, gap, height, width);
    rect.origin.x = (SETTINGS_WINDOW_WIDTH - width) / 2.0;
    rect
}

/// 寄付タブのペイン。
unsafe fn make_support_pane(
    delegate: &AnyObject,
    lang: Lang,
    localized: &mut Vec<LocalizedControl>,
) -> *mut AnyObject {
    let pane = make_pane_view(SUPPORT_PANE_HEIGHT);
    let content_width = SETTINGS_WINDOW_WIDTH - PANE_SIDE_MARGIN * 2.0;
    let mut top = SUPPORT_PANE_HEIGHT - PANE_TOP_MARGIN;

    let message = make_label(
        i18n::text(lang, Msg::SupportMessage),
        stack_down(&mut top, 0.0, SUPPORT_MESSAGE_HEIGHT, content_width),
    );
    // 文ごとの改行を反映させる。NSTextField の既定は単一行で、改行以降が切り詰められる
    let () = msg_send![message, setUsesSingleLineMode: false];
    let message_cell: *mut AnyObject = msg_send![message, cell];
    let () = msg_send![message_cell, setWraps: true];
    let () = msg_send![message_cell, setLineBreakMode: NS_LINE_BREAK_BY_WORD_WRAPPING];
    let () = msg_send![pane, addSubview: message];
    localized.push(LocalizedControl {
        control: message,
        msg: Msg::SupportMessage,
        kind: LocalizedKind::StringValue,
    });

    let support_button = make_button(
        i18n::text(lang, Msg::SupportBuyBeer),
        sel!(openSupportPage:),
        "",
        stack_down_centered(
            &mut top,
            SUPPORT_BUTTON_GAP,
            PANE_BUTTON_HEIGHT,
            PANE_BUTTON_WIDTH,
        ),
        delegate,
    );
    let () = msg_send![pane, addSubview: support_button];
    localized.push(LocalizedControl {
        control: support_button,
        msg: Msg::SupportBuyBeer,
        kind: LocalizedKind::Title,
    });

    pane
}

/// タブに載せるアイコン。素材の下に透明な帯を足した高さにして、タブ名との間隔を稼ぐ。
unsafe fn make_tab_icon(png: &[u8]) -> *mut AnyObject {
    let icon = template_image_from_png(png, TAB_ICON_SIZE);
    if icon.is_null() {
        return ptr::null_mut();
    }

    let padded: *mut AnyObject = msg_send![class!(NSImage), alloc];
    let padded: *mut AnyObject = msg_send![
        padded,
        initWithSize: NSSize {
            width: TAB_ICON_SIZE,
            height: TAB_ICON_SIZE + TAB_ICON_BOTTOM_PADDING,
        }
    ];
    let () = msg_send![padded, lockFocus];
    let () = msg_send![
        icon,
        drawInRect: NSRect {
            origin: NSPoint {
                x: 0.0,
                y: TAB_ICON_BOTTOM_PADDING,
            },
            size: NSSize {
                width: TAB_ICON_SIZE,
                height: TAB_ICON_SIZE,
            },
        }
    ];
    let () = msg_send![padded, unlockFocus];
    let () = msg_send![icon, release];
    let () = msg_send![padded, setTemplate: true];
    padded
}

/// 設定タブの行番号を出現順に払い出す。行を挿入したとき、後続の呼び出しの番号を
/// 手で振り直さなくて済むようにする。
struct RowCounter(usize);

impl RowCounter {
    fn next(&mut self) -> usize {
        let index = self.0;
        self.0 += 1;
        index
    }
}

/// ペインを 1 つのタブとして登録する。`NSTabViewController` は選択中のタブの
/// `preferredContentSize` に合わせてウィンドウの高さを変える。
#[allow(clippy::too_many_arguments)]
unsafe fn add_pane_tab(
    tab_controller: *mut AnyObject,
    pane: *mut AnyObject,
    height: f64,
    lang: Lang,
    label_msg: Msg,
    icon_png: &[u8],
    localized: &mut Vec<LocalizedControl>,
) {
    let controller: *mut AnyObject = msg_send![class!(NSViewController), alloc];
    let controller: *mut AnyObject = msg_send![controller, init];
    // view を与える前に参照されると nib を読みに行って落ちるため、init 直後に渡す
    let () = msg_send![controller, setView: pane];
    let () = msg_send![
        controller,
        setPreferredContentSize: NSSize {
            width: SETTINGS_WINDOW_WIDTH,
            height,
        }
    ];

    let item: *mut AnyObject =
        msg_send![class!(NSTabViewItem), tabViewItemWithViewController: controller];
    let label = NSString::from_str(i18n::text(lang, label_msg));
    let () = msg_send![item, setLabel: &*label];

    let icon = make_tab_icon(icon_png);
    if icon.is_null() {
        eprintln!("warning: failed to load tab icon");
    } else {
        let () = msg_send![item, setImage: icon];
        let () = msg_send![icon, release];
    }

    let () = msg_send![tab_controller, addTabViewItem: item];
    let () = msg_send![controller, release];

    localized.push(LocalizedControl {
        control: item,
        msg: label_msg,
        kind: LocalizedKind::TabLabel,
    });
}

/// 設定ウィンドウとすべてのコントロールを生成する。呼び出し側（`openSettings:`）が
/// 初回のみ呼び、以後は返り値の `SettingsControls` を使い回すこと。
///
/// 生成直後の値はプレースホルダ（既定値）。実際の値は呼び出し側が続けて
/// `sync_controls_from_settings` で反映すること。`lang` は生成時点の表示言語で、
/// 以後の切り替えは `apply_language` が担う。
///
/// # Safety
/// AppKit のメインスレッドから呼ぶこと。
pub unsafe fn build_settings_window(delegate: &AnyObject, lang: Lang) -> SettingsControls {
    let placeholder = default_display_settings();
    let mut localized: Vec<LocalizedControl> = Vec::new();

    let rect = NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size: NSSize {
            width: SETTINGS_WINDOW_WIDTH,
            height: SETTINGS_PANE_HEIGHT,
        },
    };
    let window: *mut AnyObject = msg_send![class!(NSWindow), alloc];
    let window: *mut AnyObject = msg_send![
        window,
        initWithContentRect: rect
        styleMask: SETTINGS_STYLE_MASK
        backing: BACKING_BUFFERED
        defer: false
    ];
    // 閉じても解放させず、AppState に持ったポインタを再利用して開き直す
    let () = msg_send![window, setReleasedWhenClosed: false];
    let title = NSString::from_str(i18n::text(lang, Msg::SettingsTitle));
    let () = msg_send![window, setTitle: &*title];
    // 保存せずに閉じたときに設定ファイルの内容へ戻すため windowWillClose: を受け取る
    let () = msg_send![window, setDelegate: delegate];
    localized.push(LocalizedControl {
        control: window,
        msg: Msg::SettingsTitle,
        kind: LocalizedKind::Title,
    });

    let content_view = make_pane_view(SETTINGS_PANE_HEIGHT);
    let document_view = make_pane_view(document_height());
    let mut row = RowCounter(0);

    let (poll_interval_slider, poll_interval_value_label) = add_slider_row(
        document_view,
        delegate,
        row.next(),
        lang,
        Msg::LabelPollInterval,
        ConfigKey::PollIntervalSecs,
        MIN_POLL_INTERVAL_SECS,
        MAX_POLL_INTERVAL_SECS,
        placeholder.poll_interval_secs,
        &mut localized,
    );
    let (hud_duration_slider, hud_duration_value_label) = add_slider_row(
        document_view,
        delegate,
        row.next(),
        lang,
        Msg::LabelHudDuration,
        ConfigKey::HudDurationSecs,
        MIN_HUD_DURATION_SECS,
        MAX_HUD_DURATION_SECS,
        placeholder.hud_duration_secs,
        &mut localized,
    );
    let (hud_fade_duration_slider, hud_fade_duration_value_label) = add_slider_row(
        document_view,
        delegate,
        row.next(),
        lang,
        Msg::LabelHudFadeDuration,
        ConfigKey::HudFadeDurationSecs,
        MIN_HUD_FADE_DURATION_SECS,
        MAX_HUD_FADE_DURATION_SECS,
        placeholder.hud_fade_duration_secs,
        &mut localized,
    );
    let (hud_scale_slider, hud_scale_value_label) = add_slider_row(
        document_view,
        delegate,
        row.next(),
        lang,
        Msg::LabelHudScale,
        ConfigKey::HudScale,
        MIN_HUD_SCALE,
        MAX_HUD_SCALE,
        placeholder.hud_scale,
        &mut localized,
    );
    let (max_chars_per_line_field, max_chars_per_line_stepper) = add_stepper_row(
        document_view,
        delegate,
        row.next(),
        lang,
        Msg::LabelMaxCharsPerLine,
        ConfigKey::MaxCharsPerLine,
        MIN_TRUNCATE_MAX_WIDTH,
        MAX_TRUNCATE_MAX_WIDTH,
        placeholder.truncate_max_width,
        &mut localized,
    );
    let (max_lines_field, max_lines_stepper) = add_stepper_row(
        document_view,
        delegate,
        row.next(),
        lang,
        Msg::LabelMaxLines,
        ConfigKey::MaxLines,
        MIN_TRUNCATE_MAX_LINES,
        MAX_TRUNCATE_MAX_LINES,
        placeholder.truncate_max_lines,
        &mut localized,
    );
    let (hud_image_max_height_field, hud_image_max_height_stepper) = add_stepper_row(
        document_view,
        delegate,
        row.next(),
        lang,
        Msg::LabelHudImageMaxHeight,
        ConfigKey::HudImageMaxHeight,
        MIN_HUD_IMAGE_MAX_HEIGHT,
        MAX_HUD_IMAGE_MAX_HEIGHT,
        placeholder.hud_image_max_height,
        &mut localized,
    );
    let hud_position_popup = add_popup_row(
        document_view,
        delegate,
        row.next(),
        lang,
        Msg::LabelHudPosition,
        ConfigKey::HudPosition,
        &["top", "center", "bottom"],
        placeholder.hud_position.as_str(),
        &mut localized,
    );
    let hud_background_color_popup = add_popup_row(
        document_view,
        delegate,
        row.next(),
        lang,
        Msg::LabelHudBackgroundColor,
        ConfigKey::HudBackgroundColor,
        &["default", "yellow", "blue", "green", "red", "purple"],
        placeholder.hud_background_color.as_str(),
        &mut localized,
    );
    let (hud_emoji_field, _hud_emoji_message_label) = add_field_row(
        document_view,
        content_view,
        delegate,
        row.next(),
        lang,
        Msg::LabelHudEmoji,
        ConfigKey::HudEmoji,
        &placeholder.hud_emoji,
        &mut localized,
    );
    // 区切りから下（言語・ログイン項目）は下書き→保存のモデルに乗らず、操作した瞬間に保存・反映する
    add_divider_row(document_view, row.next());
    let language_popup = add_language_row(
        document_view,
        delegate,
        row.next(),
        lang,
        placeholder.language,
        &mut localized,
    );
    let login_item_toggle = add_login_item_row(
        document_view,
        delegate,
        row.next(),
        lang,
        crate::login_item::is_enabled(),
        &mut localized,
    );
    // 行数が定数からずれると document_height と row_bottom_y の前提が崩れ、行がはみ出す
    debug_assert_eq!(row.0, SETTINGS_TOTAL_ROW_COUNT);

    let scroll_view = make_scroll_view(document_view);
    let () = msg_send![document_view, release];
    let () = msg_send![content_view, addSubview: scroll_view];
    let () = msg_send![scroll_view, release];

    add_button_row(content_view, delegate, lang, &mut localized);

    let support_pane = make_support_pane(delegate, lang, &mut localized);

    let tab_controller: *mut AnyObject = msg_send![class!(NSTabViewController), alloc];
    let tab_controller: *mut AnyObject = msg_send![tab_controller, init];
    let () = msg_send![
        tab_controller,
        setTabStyle: NS_TAB_VIEW_CONTROLLER_TAB_STYLE_TOOLBAR
    ];
    // 追加順が TAB_INDEX_SETTINGS / TAB_INDEX_SUPPORT を決める
    add_pane_tab(
        tab_controller,
        content_view,
        SETTINGS_PANE_HEIGHT,
        lang,
        Msg::TabSettings,
        GEAR_TEMPLATE_PNG,
        &mut localized,
    );
    add_pane_tab(
        tab_controller,
        support_pane,
        SUPPORT_PANE_HEIGHT,
        lang,
        Msg::TabSupport,
        BEER_TEMPLATE_PNG,
        &mut localized,
    );
    let () = msg_send![content_view, release];
    let () = msg_send![support_pane, release];

    let () = msg_send![window, setContentViewController: tab_controller];
    let () = msg_send![tab_controller, release];
    // contentViewController を入れるとウィンドウの大きさが変わる。
    // rect の origin も (0, 0)（画面左下隅）のままなので、ここで画面中央へ寄せる
    let () = msg_send![window, center];

    SettingsControls {
        window,
        poll_interval_slider,
        poll_interval_value_label,
        hud_duration_slider,
        hud_duration_value_label,
        hud_fade_duration_slider,
        hud_fade_duration_value_label,
        hud_scale_slider,
        hud_scale_value_label,
        max_chars_per_line_field,
        max_chars_per_line_stepper,
        max_lines_field,
        max_lines_stepper,
        hud_image_max_height_field,
        hud_image_max_height_stepper,
        hud_position_popup,
        hud_background_color_popup,
        hud_emoji_field,
        hud_emoji_shadow: placeholder.hud_emoji.clone(),
        language_popup,
        login_item_toggle,
        draft: placeholder,
        preview_sample_index: 0,
        localized,
    }
}

#[cfg(test)]
mod tests {
    use objc2::{class, sel};

    /// AppKit へ送るセレクタ名は文字列なので、綴りを間違えてもコンパイルは通り、
    /// 設定ウィンドウを開いた瞬間に abort する。タブ構成で使う分を突き合わせる。
    #[test]
    fn appkit_responds_to_tab_selectors() {
        assert!(class!(NSTabViewItem)
            .metaclass()
            .responds_to(sel!(tabViewItemWithViewController:)));
        assert!(class!(NSTabViewItem).responds_to(sel!(setLabel:)));
        assert!(class!(NSTabViewController).responds_to(sel!(setTabStyle:)));
        assert!(class!(NSTabViewController).responds_to(sel!(addTabViewItem:)));
        assert!(class!(NSViewController).responds_to(sel!(setView:)));
        assert!(class!(NSViewController).responds_to(sel!(setPreferredContentSize:)));
        assert!(class!(NSWindow).responds_to(sel!(setContentViewController:)));
    }

    /// 設定タブをスクロール化する NSScrollView / documentView 側のセレクタも同じ理由で突き合わせる。
    #[test]
    fn appkit_responds_to_scroll_selectors() {
        assert!(class!(NSScrollView).responds_to(sel!(setBorderType:)));
        assert!(class!(NSScrollView).responds_to(sel!(setDrawsBackground:)));
        assert!(class!(NSScrollView).responds_to(sel!(setHasVerticalScroller:)));
        assert!(class!(NSScrollView).responds_to(sel!(setHasHorizontalScroller:)));
        assert!(class!(NSScrollView).responds_to(sel!(setAutohidesScrollers:)));
        assert!(class!(NSScrollView).responds_to(sel!(setDocumentView:)));
        assert!(class!(NSView).responds_to(sel!(scrollPoint:)));
    }
}
