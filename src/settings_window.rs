use std::ptr;

use objc2::runtime::{AnyObject, Sel};
use objc2::{class, msg_send, sel};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::app::{
    apply_settings_now, present_hud, show_sample_image_content, show_text_content, AppState,
};
use crate::config::{
    apply_config_file, default_display_settings, hud_emoji_validation_error, save_config_file,
    set_config_value, settings_to_config_file, ConfigKey, DisplaySettings, MAX_HUD_DURATION_SECS,
    MAX_HUD_FADE_DURATION_SECS, MAX_HUD_IMAGE_MAX_HEIGHT, MAX_HUD_SCALE, MAX_POLL_INTERVAL_SECS,
    MAX_TRUNCATE_MAX_LINES, MAX_TRUNCATE_MAX_WIDTH, MIN_HUD_DURATION_SECS,
    MIN_HUD_FADE_DURATION_SECS, MIN_HUD_IMAGE_MAX_HEIGHT, MIN_HUD_SCALE, MIN_POLL_INTERVAL_SECS,
    MIN_TRUNCATE_MAX_LINES, MIN_TRUNCATE_MAX_WIDTH,
};
use crate::hud::BACKING_BUFFERED;
use crate::objc_helpers::{nsstring_from_str, nsstring_to_string};
use crate::png::create_preview_sample_image;

enum PreviewSample {
    ShortText,
    LongText,
    Image,
}

/// 既定の `max_chars_per_line`・`max_lines` で行内と行数の両方の切り詰めが起きる分量にしてある。
fn preview_long_text() -> String {
    let mut lines = vec!["サンプル表示の見た目を確認するための長めの一行です。".repeat(5)];
    lines.extend((2..=7).map(|n| format!("これは{n}行目のサンプル行です。")));
    lines.join("\n")
}

const PREVIEW_SAMPLES: [PreviewSample; 3] = [
    PreviewSample::ShortText,
    PreviewSample::LongText,
    PreviewSample::Image,
];

// NSWindowStyleMask: Titled | Closable
const SETTINGS_STYLE_MASK: usize = 1 | 2;

const SETTINGS_WINDOW_WIDTH: f64 = 520.0;
const SETTINGS_ROW_HEIGHT: f64 = 34.0;
const SETTINGS_ROW_COUNT: usize = 10;
const SETTINGS_TOP_MARGIN: f64 = 20.0;
const SETTINGS_BOTTOM_MARGIN: f64 = 20.0;
// ウィンドウ下部、行の並びとは別に「デフォルトに戻す」「お試し表示」「保存」ボタンを置くための領域
const SETTINGS_BUTTON_AREA_HEIGHT: f64 = 44.0;
const SETTINGS_BUTTON_WIDTH: f64 = 130.0;
const SETTINGS_BUTTON_HEIGHT: f64 = 24.0;
const SETTINGS_BUTTON_GAP: f64 = 12.0;
const SETTINGS_BUTTON_RIGHT_MARGIN: f64 = 20.0;
const SETTINGS_WINDOW_HEIGHT: f64 = SETTINGS_TOP_MARGIN
    + SETTINGS_ROW_HEIGHT * SETTINGS_ROW_COUNT as f64
    + SETTINGS_EMOJI_MESSAGE_HEIGHT
    + SETTINGS_BUTTON_AREA_HEIGHT
    + SETTINGS_BOTTOM_MARGIN;

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
// 絵文字フィールドの直下に表示するバリデーションメッセージの高さ。行スタックとボタン領域の間に確保する
const SETTINGS_EMOJI_MESSAGE_HEIGHT: f64 = 16.0;

/// 設定ウィンドウを構成するコントロールへのポインタ。`AppState` に保持し、
/// `openSettings:` で使い回す（ウィンドウは初回だけ生成する）。
pub struct SettingsControls {
    pub window: *mut AnyObject,
    pub poll_interval_slider: *mut AnyObject,
    pub poll_interval_value_label: *mut AnyObject,
    pub hud_duration_slider: *mut AnyObject,
    pub hud_duration_value_label: *mut AnyObject,
    pub hud_fade_duration_slider: *mut AnyObject,
    pub hud_fade_duration_value_label: *mut AnyObject,
    pub hud_scale_slider: *mut AnyObject,
    pub hud_scale_value_label: *mut AnyObject,
    pub max_chars_per_line_field: *mut AnyObject,
    pub max_chars_per_line_stepper: *mut AnyObject,
    pub max_lines_field: *mut AnyObject,
    pub max_lines_stepper: *mut AnyObject,
    pub hud_image_max_height_field: *mut AnyObject,
    pub hud_image_max_height_stepper: *mut AnyObject,
    pub hud_position_popup: *mut AnyObject,
    pub hud_background_color_popup: *mut AnyObject,
    pub hud_emoji_field: *mut AnyObject,
    /// 絵文字フィールドの入力中バリデーションメッセージ。妥当なときは空文字。
    pub hud_emoji_message_label: *mut AnyObject,
    /// ウィンドウ内で編集中の下書き。「保存」（`saveSettings:`）を押すまで設定ファイルには
    /// 反映しない。`settingChanged:` はこの下書きだけを更新する。
    pub draft: DisplaySettings,
    /// 「お試し表示」を押すたびに進める、次に表示するサンプルの番号（`PREVIEW_SAMPLES` を巡回）。
    pub preview_sample_index: usize,
}

impl Default for SettingsControls {
    fn default() -> Self {
        Self {
            window: ptr::null_mut(),
            poll_interval_slider: ptr::null_mut(),
            poll_interval_value_label: ptr::null_mut(),
            hud_duration_slider: ptr::null_mut(),
            hud_duration_value_label: ptr::null_mut(),
            hud_fade_duration_slider: ptr::null_mut(),
            hud_fade_duration_value_label: ptr::null_mut(),
            hud_scale_slider: ptr::null_mut(),
            hud_scale_value_label: ptr::null_mut(),
            max_chars_per_line_field: ptr::null_mut(),
            max_chars_per_line_stepper: ptr::null_mut(),
            max_lines_field: ptr::null_mut(),
            max_lines_stepper: ptr::null_mut(),
            hud_image_max_height_field: ptr::null_mut(),
            hud_image_max_height_stepper: ptr::null_mut(),
            hud_position_popup: ptr::null_mut(),
            hud_background_color_popup: ptr::null_mut(),
            hud_emoji_field: ptr::null_mut(),
            hud_emoji_message_label: ptr::null_mut(),
            draft: default_display_settings(),
            preview_sample_index: 0,
        }
    }
}

/// `ConfigKey` を `NSControl` の `tag` にエンコードする。
pub fn config_key_to_tag(key: ConfigKey) -> isize {
    match key {
        ConfigKey::PollIntervalSecs => 0,
        ConfigKey::HudDurationSecs => 1,
        ConfigKey::HudFadeDurationSecs => 2,
        ConfigKey::MaxCharsPerLine => 3,
        ConfigKey::MaxLines => 4,
        ConfigKey::HudPosition => 5,
        ConfigKey::HudScale => 6,
        ConfigKey::HudBackgroundColor => 7,
        ConfigKey::HudEmoji => 8,
        ConfigKey::HudImageMaxHeight => 9,
    }
}

pub fn tag_to_config_key(tag: isize) -> Option<ConfigKey> {
    match tag {
        0 => Some(ConfigKey::PollIntervalSecs),
        1 => Some(ConfigKey::HudDurationSecs),
        2 => Some(ConfigKey::HudFadeDurationSecs),
        3 => Some(ConfigKey::MaxCharsPerLine),
        4 => Some(ConfigKey::MaxLines),
        5 => Some(ConfigKey::HudPosition),
        6 => Some(ConfigKey::HudScale),
        7 => Some(ConfigKey::HudBackgroundColor),
        8 => Some(ConfigKey::HudEmoji),
        9 => Some(ConfigKey::HudImageMaxHeight),
        _ => None,
    }
}

fn row_bottom_y(index: usize) -> f64 {
    let row_top =
        SETTINGS_WINDOW_HEIGHT - SETTINGS_TOP_MARGIN - (index as f64) * SETTINGS_ROW_HEIGHT;
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

unsafe fn make_label(text: &str, frame: NSRect) -> *mut AnyObject {
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

unsafe fn set_string_value(control: *mut AnyObject, text: &str) {
    let ns = nsstring_from_str(text);
    let () = msg_send![control, setStringValue: ns];
    let () = msg_send![ns, release];
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
        let ns = nsstring_from_str(item);
        let () = msg_send![popup, addItemWithTitle: ns];
        let () = msg_send![ns, release];
    }
    select_popup_item(popup, selected);
    let () = msg_send![popup, setTag: tag];
    let () = msg_send![popup, setTarget: delegate];
    let () = msg_send![popup, setAction: sel!(settingChanged:)];
    popup
}

unsafe fn select_popup_item(popup: *mut AnyObject, title: &str) {
    let ns = nsstring_from_str(title);
    let () = msg_send![popup, selectItemWithTitle: ns];
    let () = msg_send![ns, release];
}

#[allow(clippy::too_many_arguments)]
unsafe fn add_slider_row(
    content_view: *mut AnyObject,
    delegate: &AnyObject,
    index: usize,
    label_text: &str,
    key: ConfigKey,
    min: f64,
    max: f64,
    value: f64,
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

    let label = make_label(label_text, label_rect);
    let slider = make_slider(
        min,
        max,
        value,
        config_key_to_tag(key),
        slider_rect,
        delegate,
    );
    let value_label = make_label(&format!("{value:.2}"), value_rect);

    let () = msg_send![content_view, addSubview: label];
    let () = msg_send![content_view, addSubview: slider];
    let () = msg_send![content_view, addSubview: value_label];

    (slider, value_label)
}

#[allow(clippy::too_many_arguments)]
unsafe fn add_stepper_row(
    content_view: *mut AnyObject,
    delegate: &AnyObject,
    index: usize,
    label_text: &str,
    key: ConfigKey,
    min: usize,
    max: usize,
    value: usize,
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
    let label = make_label(label_text, label_rect);
    let field = make_editable_field(&value.to_string(), tag, field_rect, delegate);
    let stepper = make_stepper(
        min as f64,
        max as f64,
        value as f64,
        tag,
        stepper_rect,
        delegate,
    );

    let () = msg_send![content_view, addSubview: label];
    let () = msg_send![content_view, addSubview: field];
    let () = msg_send![content_view, addSubview: stepper];

    (field, stepper)
}

unsafe fn add_popup_row(
    content_view: *mut AnyObject,
    delegate: &AnyObject,
    index: usize,
    label_text: &str,
    key: ConfigKey,
    items: &[&str],
    selected: &str,
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

    let label = make_label(label_text, label_rect);
    let popup = make_popup(
        items,
        selected,
        config_key_to_tag(key),
        popup_rect,
        delegate,
    );

    let () = msg_send![content_view, addSubview: label];
    let () = msg_send![content_view, addSubview: popup];

    popup
}

/// 絵文字フィールド専用の行。入力中バリデーションのため `controlTextDidChange:` を
/// 受け取れるよう `setDelegate:` し、フィールド直下にメッセージラベルを追加する。
unsafe fn add_field_row(
    content_view: *mut AnyObject,
    delegate: &AnyObject,
    index: usize,
    label_text: &str,
    key: ConfigKey,
    value: &str,
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

    let label = make_label(label_text, label_rect);
    let field = make_editable_field(value, config_key_to_tag(key), field_rect, delegate);
    let () = msg_send![field, setDelegate: delegate];

    let message_label = make_label("", message_rect);
    let red: *mut AnyObject = msg_send![class!(NSColor), systemRedColor];
    let () = msg_send![message_label, setTextColor: red];

    let () = msg_send![content_view, addSubview: label];
    let () = msg_send![content_view, addSubview: field];
    let () = msg_send![content_view, addSubview: message_label];

    (field, message_label)
}

/// 設定ウィンドウとすべてのコントロールを生成する。呼び出し側（`openSettings:`）が
/// 初回のみ呼び、以後は返り値の `SettingsControls` を使い回すこと。
///
/// 生成直後の値はプレースホルダ（既定値）。実際の値は呼び出し側が続けて
/// `sync_controls_from_settings` で反映すること。
///
/// # Safety
/// AppKit のメインスレッドから呼ぶこと。
pub unsafe fn build_settings_window(delegate: &AnyObject) -> SettingsControls {
    let placeholder = default_display_settings();

    let rect = NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size: NSSize {
            width: SETTINGS_WINDOW_WIDTH,
            height: SETTINGS_WINDOW_HEIGHT,
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
    let title = nsstring_from_str("cliip-show 設定");
    let () = msg_send![window, setTitle: title];
    let () = msg_send![title, release];
    // rect の origin は (0, 0)（画面左下隅）のままなので、明示的に画面中央へ寄せる
    let () = msg_send![window, center];
    // 保存せずに閉じたときに設定ファイルの内容へ戻すため windowWillClose: を受け取る
    let () = msg_send![window, setDelegate: delegate];

    let content_view: *mut AnyObject = msg_send![window, contentView];

    let (poll_interval_slider, poll_interval_value_label) = add_slider_row(
        content_view,
        delegate,
        0,
        "ポーリング間隔（秒）",
        ConfigKey::PollIntervalSecs,
        MIN_POLL_INTERVAL_SECS,
        MAX_POLL_INTERVAL_SECS,
        placeholder.poll_interval_secs,
    );
    let (hud_duration_slider, hud_duration_value_label) = add_slider_row(
        content_view,
        delegate,
        1,
        "表示時間（秒）",
        ConfigKey::HudDurationSecs,
        MIN_HUD_DURATION_SECS,
        MAX_HUD_DURATION_SECS,
        placeholder.hud_duration_secs,
    );
    let (hud_fade_duration_slider, hud_fade_duration_value_label) = add_slider_row(
        content_view,
        delegate,
        2,
        "フェード時間（秒）",
        ConfigKey::HudFadeDurationSecs,
        MIN_HUD_FADE_DURATION_SECS,
        MAX_HUD_FADE_DURATION_SECS,
        placeholder.hud_fade_duration_secs,
    );
    let (hud_scale_slider, hud_scale_value_label) = add_slider_row(
        content_view,
        delegate,
        3,
        "HUDサイズ倍率",
        ConfigKey::HudScale,
        MIN_HUD_SCALE,
        MAX_HUD_SCALE,
        placeholder.hud_scale,
    );
    let (max_chars_per_line_field, max_chars_per_line_stepper) = add_stepper_row(
        content_view,
        delegate,
        4,
        "1行の最大文字数",
        ConfigKey::MaxCharsPerLine,
        MIN_TRUNCATE_MAX_WIDTH,
        MAX_TRUNCATE_MAX_WIDTH,
        placeholder.truncate_max_width,
    );
    let (max_lines_field, max_lines_stepper) = add_stepper_row(
        content_view,
        delegate,
        5,
        "最大行数",
        ConfigKey::MaxLines,
        MIN_TRUNCATE_MAX_LINES,
        MAX_TRUNCATE_MAX_LINES,
        placeholder.truncate_max_lines,
    );
    let (hud_image_max_height_field, hud_image_max_height_stepper) = add_stepper_row(
        content_view,
        delegate,
        6,
        "画像サムネイル高さ上限（px）",
        ConfigKey::HudImageMaxHeight,
        MIN_HUD_IMAGE_MAX_HEIGHT,
        MAX_HUD_IMAGE_MAX_HEIGHT,
        placeholder.hud_image_max_height,
    );
    let hud_position_popup = add_popup_row(
        content_view,
        delegate,
        7,
        "表示位置",
        ConfigKey::HudPosition,
        &["top", "center", "bottom"],
        placeholder.hud_position.as_str(),
    );
    let hud_background_color_popup = add_popup_row(
        content_view,
        delegate,
        8,
        "背景色",
        ConfigKey::HudBackgroundColor,
        &["default", "yellow", "blue", "green", "red", "purple"],
        placeholder.hud_background_color.as_str(),
    );
    let (hud_emoji_field, hud_emoji_message_label) = add_field_row(
        content_view,
        delegate,
        9,
        "アイコン絵文字",
        ConfigKey::HudEmoji,
        &placeholder.hud_emoji,
    );

    add_button_row(content_view, delegate);

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
        hud_emoji_message_label,
        draft: placeholder,
        preview_sample_index: 0,
    }
}

/// ボタンは右寄せで、右端が「保存」になるよう逆順に座標を計算する。
unsafe fn add_button_row(content_view: *mut AnyObject, delegate: &AnyObject) {
    let y = SETTINGS_BOTTOM_MARGIN + (SETTINGS_BUTTON_AREA_HEIGHT - SETTINGS_BUTTON_HEIGHT) / 2.0;
    let save_x = SETTINGS_WINDOW_WIDTH - SETTINGS_BUTTON_RIGHT_MARGIN - SETTINGS_BUTTON_WIDTH;
    let preview_x = save_x - SETTINGS_BUTTON_GAP - SETTINGS_BUTTON_WIDTH;
    let reset_x = preview_x - SETTINGS_BUTTON_GAP - SETTINGS_BUTTON_WIDTH;
    let button_size = NSSize {
        width: SETTINGS_BUTTON_WIDTH,
        height: SETTINGS_BUTTON_HEIGHT,
    };

    let reset_button = make_button(
        "デフォルトに戻す",
        sel!(resetSettings:),
        "",
        NSRect {
            origin: NSPoint { x: reset_x, y },
            size: button_size,
        },
        delegate,
    );
    let preview_button = make_button(
        "お試し表示",
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
        "保存",
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
}

unsafe fn make_button(
    title: &str,
    action: Sel,
    key_equivalent: &str,
    frame: NSRect,
    delegate: &AnyObject,
) -> *mut AnyObject {
    let button: *mut AnyObject = msg_send![class!(NSButton), alloc];
    let button: *mut AnyObject = msg_send![button, initWithFrame: frame];
    let title_ns = nsstring_from_str(title);
    let () = msg_send![button, setTitle: title_ns];
    let () = msg_send![title_ns, release];
    let () = msg_send![button, setTarget: delegate];
    let () = msg_send![button, setAction: action];
    if !key_equivalent.is_empty() {
        let key_ns = nsstring_from_str(key_equivalent);
        let () = msg_send![button, setKeyEquivalent: key_ns];
        let () = msg_send![key_ns, release];
    }
    button
}

unsafe fn sync_slider(slider: *mut AnyObject, value_label: *mut AnyObject, value: f64) {
    let () = msg_send![slider, setDoubleValue: value];
    set_string_value(value_label, &format!("{value:.2}"));
}

unsafe fn sync_stepper_field(field: *mut AnyObject, stepper: *mut AnyObject, value: usize) {
    let () = msg_send![stepper, setIntegerValue: value as isize];
    set_string_value(field, &value.to_string());
}

/// 現在の `DisplaySettings` の値をすべてのコントロールに反映する。
/// ウィンドウを開くたびに呼び、外部（ファイル監視等）での変更との食い違いを防ぐ。
///
/// # Safety
/// - `controls` は `build_settings_window` が返した有効なポインタを保持していること。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn sync_controls_from_settings(controls: &SettingsControls, settings: &DisplaySettings) {
    sync_slider(
        controls.poll_interval_slider,
        controls.poll_interval_value_label,
        settings.poll_interval_secs,
    );
    sync_slider(
        controls.hud_duration_slider,
        controls.hud_duration_value_label,
        settings.hud_duration_secs,
    );
    sync_slider(
        controls.hud_fade_duration_slider,
        controls.hud_fade_duration_value_label,
        settings.hud_fade_duration_secs,
    );
    sync_slider(
        controls.hud_scale_slider,
        controls.hud_scale_value_label,
        settings.hud_scale,
    );

    sync_stepper_field(
        controls.max_chars_per_line_field,
        controls.max_chars_per_line_stepper,
        settings.truncate_max_width,
    );
    sync_stepper_field(
        controls.max_lines_field,
        controls.max_lines_stepper,
        settings.truncate_max_lines,
    );
    sync_stepper_field(
        controls.hud_image_max_height_field,
        controls.hud_image_max_height_stepper,
        settings.hud_image_max_height,
    );

    select_popup_item(controls.hud_position_popup, settings.hud_position.as_str());
    select_popup_item(
        controls.hud_background_color_popup,
        settings.hud_background_color.as_str(),
    );

    set_string_value(controls.hud_emoji_field, &settings.hud_emoji);
    // プログラムによる同期はすべて妥当な値のみを書き戻すため、残っているメッセージは消す。
    // controlTextDidChange: 側の再入防止（try_lock）に任せると、ロック保持中の呼び出しでは
    // 通知が黙って捨てられメッセージが古いまま残りうるため、ここで確実にクリアする。
    set_string_value(controls.hud_emoji_message_label, "");
}

/// `controlTextDidChange:` から呼ぶ。絵文字フィールドの現在の入力値を判定し、
/// 不正なら理由をメッセージラベルに表示する。妥当なら空にする。
///
/// # Safety
/// - `APP_STATE` のロックは呼び出し側が `try_lock` で取得済みであること。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn update_emoji_validation_message(state: &AppState) {
    let raw: *mut AnyObject = msg_send![state.settings_controls.hud_emoji_field, stringValue];
    let text = nsstring_to_string(raw).unwrap_or_default();
    let message = hud_emoji_validation_error(&text).unwrap_or("");
    set_string_value(state.settings_controls.hud_emoji_message_label, message);
}

/// `field`/`stepper` はペアで同じ値を表示する。どちらが `sender` でも、変更後の値を
/// もう一方に同期しつつ文字列として返す。
unsafe fn sync_paired_value(
    sender: *mut AnyObject,
    field: *mut AnyObject,
    stepper: *mut AnyObject,
) -> Option<String> {
    if sender == field {
        let raw: *mut AnyObject = msg_send![field, stringValue];
        let text = nsstring_to_string(raw)?;
        let parsed: isize = text.trim().parse().ok()?;
        let () = msg_send![stepper, setIntegerValue: parsed];
        Some(parsed.to_string())
    } else {
        let value: isize = msg_send![stepper, integerValue];
        let text = value.to_string();
        set_string_value(field, &text);
        Some(text)
    }
}

unsafe fn raw_value_for_control(
    key: ConfigKey,
    sender: *mut AnyObject,
    controls: &SettingsControls,
) -> Option<String> {
    match key {
        ConfigKey::PollIntervalSecs
        | ConfigKey::HudDurationSecs
        | ConfigKey::HudFadeDurationSecs
        | ConfigKey::HudScale => {
            let value: f64 = msg_send![sender, doubleValue];
            // スライダーは連続値を返すため、そのまま文字列化すると 1.0700000000000001 のような
            // 値が設定ファイルに残る。値ラベルの表示と同じ桁で丸めて揃える。
            Some(format!("{value:.2}"))
        }
        ConfigKey::MaxCharsPerLine => sync_paired_value(
            sender,
            controls.max_chars_per_line_field,
            controls.max_chars_per_line_stepper,
        ),
        ConfigKey::MaxLines => {
            sync_paired_value(sender, controls.max_lines_field, controls.max_lines_stepper)
        }
        ConfigKey::HudImageMaxHeight => sync_paired_value(
            sender,
            controls.hud_image_max_height_field,
            controls.hud_image_max_height_stepper,
        ),
        ConfigKey::HudPosition | ConfigKey::HudBackgroundColor => {
            let title: *mut AnyObject = msg_send![sender, titleOfSelectedItem];
            nsstring_to_string(title)
        }
        ConfigKey::HudEmoji => {
            let value: *mut AnyObject = msg_send![sender, stringValue];
            nsstring_to_string(value)
        }
    }
}

/// 設定ウィンドウのコントロール変更を下書き（`SettingsControls::draft`）に反映するだけで、
/// ファイル保存も HUD への適用もしない。確定させるには「保存」（`saveSettings:`）を押す。
///
/// クランプ・バリデーションは既存の `set_config_value`（CLI の `--config set` と共通）に委ねる。
///
/// # Safety
/// - `APP_STATE` をロックしないこと（呼び出し側が既にロックを保持している）。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn apply_setting_change(state: &mut AppState, sender: *mut AnyObject) {
    let tag: isize = msg_send![sender, tag];
    let Some(key) = tag_to_config_key(tag) else {
        return;
    };
    let Some(raw_value) = raw_value_for_control(key, sender, &state.settings_controls) else {
        return;
    };

    let mut config = settings_to_config_file(state.settings_controls.draft.clone());
    let warning = match set_config_value(&mut config, key, &raw_value) {
        Ok(warning) => warning,
        Err(error) => {
            eprintln!("warning: {error}");
            // 拒否された文字列がフィールドに残ると下書きと食い違うので、下書きの値に戻す。
            // setStringValue: は action を発火しないため settingChanged: の再入は起きない。
            if key == ConfigKey::HudEmoji {
                set_string_value(
                    state.settings_controls.hud_emoji_field,
                    &state.settings_controls.draft.hud_emoji,
                );
                set_string_value(state.settings_controls.hud_emoji_message_label, "");
            }
            return;
        }
    };
    if let Some(warning) = warning {
        eprintln!("warning: {warning}");
    }

    state.settings_controls.draft =
        apply_config_file(state.settings_controls.draft.clone(), &config);

    // クランプ・拒否された場合に備え、実際に反映された値でコントロール表示を揃える。
    // sender（今まさに編集/操作していたコントロール）自身への setStringValue は避ける:
    // フィールドエディタが終了処理中に再入すると settingChanged: が再度発火し、
    // 非再入の APP_STATE ロックでハングしうるため。
    resync_controls_after_apply(
        &state.settings_controls,
        &state.settings_controls.draft,
        key,
        sender,
    );
}

/// 下書きを既定値に戻し、コントロール表示を同期する。保存も HUD への適用もしない。
///
/// # Safety
/// - `APP_STATE` をロックしないこと（呼び出し側が既にロックを保持している）。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn reset_settings(state: &mut AppState) {
    state.settings_controls.draft = default_display_settings();
    sync_controls_from_settings(&state.settings_controls, &state.settings_controls.draft);
}

/// 下書きを HUD に適用したうえで固定サンプルを表示する。クリップボードの内容には依存せず、
/// 押すたびに短文 → 長文 → 画像 → 短文 … と巡回する。設定の見た目を毎回同じ内容で
/// 比較できるようにするため、クリップボードは参照しない。ファイル保存はしない。
///
/// # Safety
/// - `APP_STATE` をロックしないこと（呼び出し側が既にロックを保持している）。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn preview_settings(this: &AnyObject, state: &mut AppState) {
    sync_draft_from_controls(&mut state.settings_controls);

    let draft = state.settings_controls.draft.clone();
    apply_settings_now(state, draft);

    let index = state.settings_controls.preview_sample_index;
    state.settings_controls.preview_sample_index = (index + 1) % PREVIEW_SAMPLES.len();

    match PREVIEW_SAMPLES[index] {
        PreviewSample::ShortText => show_text_content(state, "サンプル表示：短いテキストです"),
        PreviewSample::LongText => show_text_content(state, &preview_long_text()),
        PreviewSample::Image => match create_preview_sample_image() {
            Ok(image) => show_sample_image_content(state, image),
            Err(error) => {
                eprintln!("warning: {error}");
                show_text_content(state, "サンプル表示：短いテキストです");
            }
        },
    }

    present_hud(this, state);
}

/// 下書きを設定ファイルへ保存し、HUD に適用する。`config_mtime` を保存後のファイルの mtime に
/// 更新し、直後のファイル監視ポーリングによる二重適用を防ぐ。
///
/// # Safety
/// - `APP_STATE` をロックしないこと（呼び出し側が既にロックを保持している）。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn save_settings(state: &mut AppState) -> bool {
    sync_draft_from_controls(&mut state.settings_controls);

    let Some(path) = state.config_path.clone() else {
        eprintln!("warning: config path is not resolved; cannot save settings");
        return false;
    };

    let config = settings_to_config_file(state.settings_controls.draft.clone());
    if let Err(error) = save_config_file(&path, &config) {
        eprintln!("warning: {error}");
        return false;
    }

    let draft = state.settings_controls.draft.clone();
    apply_settings_now(state, draft);
    state.config_mtime = std::fs::metadata(&path)
        .ok()
        .and_then(|m| m.modified().ok());
    true
}

/// 全キーのコントロールの現在値から下書きを組み直す。
///
/// NSTextField の action はフォーカスが外れるか Enter を押すまで飛ばず、その Enter は
/// デフォルトボタン（保存）に先取りされる。保存・お試し表示の直前にここで読み直すことで、
/// 未確定の入力の取りこぼしと、拒否された値がフィールドに残ることの両方を防ぐ。
unsafe fn sync_draft_from_controls(controls: &mut SettingsControls) {
    const KEYS: [ConfigKey; 10] = [
        ConfigKey::PollIntervalSecs,
        ConfigKey::HudDurationSecs,
        ConfigKey::HudFadeDurationSecs,
        ConfigKey::HudScale,
        ConfigKey::MaxCharsPerLine,
        ConfigKey::MaxLines,
        ConfigKey::HudImageMaxHeight,
        ConfigKey::HudPosition,
        ConfigKey::HudBackgroundColor,
        ConfigKey::HudEmoji,
    ];

    let mut config = settings_to_config_file(controls.draft.clone());
    for key in KEYS {
        let control = control_for_key(controls, key);
        if control.is_null() {
            continue;
        }
        let Some(raw) = raw_value_for_key(key, control) else {
            continue;
        };
        if let Err(error) = set_config_value(&mut config, key, &raw) {
            eprintln!("warning: {error}");
        }
    }
    controls.draft = apply_config_file(controls.draft.clone(), &config);

    let draft = controls.draft.clone();
    sync_controls_from_settings(controls, &draft);
}

/// キーに対応する「値の入力元」となるコントロール。field と stepper の組は、利用者が直接
/// 打ち込める field を正とする。
fn control_for_key(controls: &SettingsControls, key: ConfigKey) -> *mut AnyObject {
    match key {
        ConfigKey::PollIntervalSecs => controls.poll_interval_slider,
        ConfigKey::HudDurationSecs => controls.hud_duration_slider,
        ConfigKey::HudFadeDurationSecs => controls.hud_fade_duration_slider,
        ConfigKey::HudScale => controls.hud_scale_slider,
        ConfigKey::MaxCharsPerLine => controls.max_chars_per_line_field,
        ConfigKey::MaxLines => controls.max_lines_field,
        ConfigKey::HudImageMaxHeight => controls.hud_image_max_height_field,
        ConfigKey::HudPosition => controls.hud_position_popup,
        ConfigKey::HudBackgroundColor => controls.hud_background_color_popup,
        ConfigKey::HudEmoji => controls.hud_emoji_field,
    }
}

unsafe fn raw_value_for_key(key: ConfigKey, control: *mut AnyObject) -> Option<String> {
    match key {
        ConfigKey::PollIntervalSecs
        | ConfigKey::HudDurationSecs
        | ConfigKey::HudFadeDurationSecs
        | ConfigKey::HudScale => {
            let value: f64 = msg_send![control, doubleValue];
            Some(format!("{value:.2}"))
        }
        ConfigKey::HudPosition | ConfigKey::HudBackgroundColor => {
            let title: *mut AnyObject = msg_send![control, titleOfSelectedItem];
            nsstring_to_string(title)
        }
        ConfigKey::MaxCharsPerLine
        | ConfigKey::MaxLines
        | ConfigKey::HudImageMaxHeight
        | ConfigKey::HudEmoji => {
            let value: *mut AnyObject = msg_send![control, stringValue];
            nsstring_to_string(value)
        }
    }
}

unsafe fn resync_controls_after_apply(
    controls: &SettingsControls,
    settings: &DisplaySettings,
    key: ConfigKey,
    sender: *mut AnyObject,
) {
    match key {
        ConfigKey::PollIntervalSecs => {
            set_string_value(
                controls.poll_interval_value_label,
                &format!("{:.2}", settings.poll_interval_secs),
            );
        }
        ConfigKey::HudDurationSecs => {
            set_string_value(
                controls.hud_duration_value_label,
                &format!("{:.2}", settings.hud_duration_secs),
            );
        }
        ConfigKey::HudFadeDurationSecs => {
            set_string_value(
                controls.hud_fade_duration_value_label,
                &format!("{:.2}", settings.hud_fade_duration_secs),
            );
        }
        ConfigKey::HudScale => {
            set_string_value(
                controls.hud_scale_value_label,
                &format!("{:.2}", settings.hud_scale),
            );
        }
        ConfigKey::MaxCharsPerLine => resync_stepper_field(
            sender,
            controls.max_chars_per_line_field,
            controls.max_chars_per_line_stepper,
            settings.truncate_max_width,
        ),
        ConfigKey::MaxLines => resync_stepper_field(
            sender,
            controls.max_lines_field,
            controls.max_lines_stepper,
            settings.truncate_max_lines,
        ),
        ConfigKey::HudImageMaxHeight => resync_stepper_field(
            sender,
            controls.hud_image_max_height_field,
            controls.hud_image_max_height_stepper,
            settings.hud_image_max_height,
        ),
        // ポップアップの選択肢は常に妥当な値のみを取りうるためクランプが発生せず、再同期は不要
        ConfigKey::HudPosition | ConfigKey::HudBackgroundColor => {}
        // 拒否された入力は set_config_value のエラー側で戻すため、ここでは触らない。
        // 編集中フィールドへの setStringValue を避ける狙いもある。
        ConfigKey::HudEmoji => {}
    }
}

/// stepper は常に更新する（テキスト編集の再入リスクがないため）。
/// field は `sender` 自身でない場合のみ更新する（sender の場合は編集中の可能性があるため触れない）。
unsafe fn resync_stepper_field(
    sender: *mut AnyObject,
    field: *mut AnyObject,
    stepper: *mut AnyObject,
    value: usize,
) {
    let () = msg_send![stepper, setIntegerValue: value as isize];
    if sender != field {
        set_string_value(field, &value.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::{config_key_to_tag, tag_to_config_key};
    use crate::config::ConfigKey;

    const ALL_KEYS: [ConfigKey; 10] = [
        ConfigKey::PollIntervalSecs,
        ConfigKey::HudDurationSecs,
        ConfigKey::HudFadeDurationSecs,
        ConfigKey::MaxCharsPerLine,
        ConfigKey::MaxLines,
        ConfigKey::HudPosition,
        ConfigKey::HudScale,
        ConfigKey::HudBackgroundColor,
        ConfigKey::HudEmoji,
        ConfigKey::HudImageMaxHeight,
    ];

    #[test]
    fn tag_round_trips_for_all_config_keys() {
        for key in ALL_KEYS {
            let tag = config_key_to_tag(key);
            assert_eq!(tag_to_config_key(tag), Some(key));
        }
    }

    #[test]
    fn tag_to_config_key_rejects_unknown_tags() {
        assert_eq!(tag_to_config_key(-1), None);
        assert_eq!(tag_to_config_key(10), None);
        assert_eq!(tag_to_config_key(9999), None);
    }
}
