use std::ptr;

use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::config::{
    DisplaySettings, HudBackgroundColor, HudPosition, DEFAULT_HUD_BACKGROUND_OPACITY,
    DEFAULT_HUD_SCALE, MAX_HUD_BACKGROUND_OPACITY, MAX_HUD_SCALE, MIN_HUD_BACKGROUND_OPACITY,
    MIN_HUD_SCALE,
};

pub const BORDERLESS_MASK: usize = 0;
pub const BACKING_BUFFERED: isize = 2;
pub const FLOATING_WINDOW_LEVEL: isize = 3;

// NSLineBreakMode: ByCharWrapping。HUD は CJK を含む任意テキストの切り詰め表示のため、
// 単語境界ではなく文字単位で折り返す。
const NS_LINE_BREAK_BY_CHAR_WRAPPING: isize = 1;
// NSTextAlignment
const NS_TEXT_ALIGNMENT_LEFT: isize = 0;
// NSImageScaling
const NS_IMAGE_SCALE_PROPORTIONALLY_UP_OR_DOWN: usize = 3;

const HUD_MIN_WIDTH: f64 = 200.0;
const HUD_MAX_WIDTH: f64 = 820.0;
const HUD_MIN_HEIGHT: f64 = 52.0;
const HUD_MAX_HEIGHT: f64 = 280.0;
const HUD_HORIZONTAL_PADDING: f64 = 16.0;
const HUD_VERTICAL_PADDING: f64 = 10.0;
const HUD_ICON_WIDTH: f64 = 22.0;
const HUD_ICON_HEIGHT: f64 = 22.0;
const HUD_GAP: f64 = 8.0;
const HUD_CHAR_WIDTH_ESTIMATE: f64 = 9.6;
const HUD_LINE_HEIGHT_ESTIMATE: f64 = 22.0;
pub const HUD_TEXT_MEASURE_HEIGHT: f64 = 10_000.0;
pub const HUD_TEXT_MEASURE_MAX_WIDTH: f64 = 1_000_000.0;
const HUD_CORNER_RADIUS: f64 = 14.0;
const HUD_BORDER_WIDTH: f64 = 1.0;
const HUD_ICON_FONT_SIZE: f64 = 18.0;
const HUD_TEXT_FONT_SIZE: f64 = 18.0;

/// HUD ウィンドウの外形。テキスト・画像どちらの内容でも使う。
/// `icon_y` はテキストの行高を下限にした位置合わせを含む
/// （画像でもアイコンの縦位置がテキスト表示時とずれないようにするため）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudFrameMetrics {
    pub width: f64,
    pub height: f64,
    pub icon_y: f64,
}

/// テキスト表示専用のレイアウト。`text_width` は最小幅由来、`text_height` は行高由来の
/// 下限を含むため、テキスト以外の内容のフレームに流用してはいけない。
/// 画像レイアウトが誤って参照できないよう、外形（`HudFrameMetrics`）と型を分けている。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudTextMetrics {
    pub frame: HudFrameMetrics,
    pub text_width: f64,
    pub text_height: f64,
    pub label_y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudDimensions {
    pub min_width: f64,
    pub max_width: f64,
    pub min_height: f64,
    pub max_height: f64,
    pub horizontal_padding: f64,
    pub vertical_padding: f64,
    pub icon_width: f64,
    pub icon_height: f64,
    pub gap: f64,
    pub line_height_estimate: f64,
    pub char_width_estimate: f64,
}

pub fn hud_dimensions(scale: f64) -> HudDimensions {
    use crate::config::parse_f64_value;
    let clamped_scale = parse_f64_value(scale, DEFAULT_HUD_SCALE, MIN_HUD_SCALE, MAX_HUD_SCALE);
    HudDimensions {
        min_width: HUD_MIN_WIDTH * clamped_scale,
        max_width: HUD_MAX_WIDTH * clamped_scale,
        min_height: HUD_MIN_HEIGHT * clamped_scale,
        max_height: HUD_MAX_HEIGHT * clamped_scale,
        horizontal_padding: HUD_HORIZONTAL_PADDING * clamped_scale,
        vertical_padding: HUD_VERTICAL_PADDING * clamped_scale,
        icon_width: HUD_ICON_WIDTH * clamped_scale,
        icon_height: HUD_ICON_HEIGHT * clamped_scale,
        gap: HUD_GAP * clamped_scale,
        line_height_estimate: HUD_LINE_HEIGHT_ESTIMATE * clamped_scale,
        char_width_estimate: HUD_CHAR_WIDTH_ESTIMATE * clamped_scale,
    }
}

/// アイコン表示時に幅計算へ加算する「アイコン幅 + テキストとの隙間」。
/// アイコンなし（`hud_emoji` が空）のときは 0 を返し、左側の余白を詰める。
fn icon_reserve(dims: &HudDimensions, has_icon: bool) -> f64 {
    if has_icon {
        dims.icon_width + dims.gap
    } else {
        0.0
    }
}

/// 内容（テキスト・画像）の左右に付く固定幅。左右パディングとアイコン列の合計で、
/// 「HUD 全幅 − この値 = 内容に使える幅」の関係が全レイアウト計算で共通に成り立つ。
fn horizontal_chrome_width(dims: &HudDimensions, has_icon: bool) -> f64 {
    dims.horizontal_padding * 2.0 + icon_reserve(dims, has_icon)
}

/// `opacity` を「背景色ごとの既定アルファに対する倍率」としてクランプする。
/// 設定ファイルを手編集した値や NaN が直接届いても壊れないよう、`hud_dimensions` が
/// `hud_scale` に対して行っているのと同じガードをここでも行う。
fn clamped_background_opacity(opacity: f64) -> f64 {
    crate::config::parse_f64_value(
        opacity,
        DEFAULT_HUD_BACKGROUND_OPACITY,
        MIN_HUD_BACKGROUND_OPACITY,
        MAX_HUD_BACKGROUND_OPACITY,
    )
}

/// ボーダーカラーの (white, alpha) を返す。
/// app.rs のホットリロード時にも同じ値を使うため一元管理する。
pub fn hud_border_white_alpha(color: HudBackgroundColor, opacity: f64) -> (f64, f64) {
    let alpha = match color {
        HudBackgroundColor::Default => 0.14,
        _ => 0.2,
    };
    (1.0, alpha * clamped_background_opacity(opacity))
}

/// 背景レイヤーの (r, g, b, a) を返す。`opacity` は色ごとの既定アルファに掛ける倍率で、
/// 絶対的な不透明度ではない（RGB はそのまま、a だけ変わる）。
pub fn hud_background_rgba(color: HudBackgroundColor, opacity: f64) -> (f64, f64, f64, f64) {
    let clamped_opacity = clamped_background_opacity(opacity);
    let (r, g, b, a) = match color {
        HudBackgroundColor::Default => (0.0, 0.0, 0.0, 0.78),
        HudBackgroundColor::Yellow => (0.43, 0.34, 0.04, 0.9),
        HudBackgroundColor::Blue => (0.08, 0.22, 0.53, 0.9),
        HudBackgroundColor::Green => (0.08, 0.35, 0.22, 0.9),
        HudBackgroundColor::Red => (0.47, 0.14, 0.14, 0.9),
        HudBackgroundColor::Purple => (0.36, 0.16, 0.47, 0.9),
    };
    (r, g, b, a * clamped_opacity)
}

/// クリップボードの画像を HUD に収めるときのサムネイル寸法 (幅, 高さ) を返す。
///
/// 元画像より大きくは表示しない（ビットマップの引き伸ばしを避けるため）。
/// `max_height_setting` は `hud_scale` 倍したうえで、HUD 自体の高さ・幅の上限でさらに抑える。
pub fn fit_thumbnail_size(
    image_width: f64,
    image_height: f64,
    max_height_setting: usize,
    scale: f64,
    has_icon: bool,
) -> (f64, f64) {
    use crate::config::parse_f64_value;
    let clamped_scale = parse_f64_value(scale, DEFAULT_HUD_SCALE, MIN_HUD_SCALE, MAX_HUD_SCALE);
    let dims = hud_dimensions(clamped_scale);

    let max_height = (max_height_setting as f64 * clamped_scale)
        .min(dims.max_height - dims.vertical_padding * 2.0)
        .max(1.0);
    let max_width = (dims.max_width - horizontal_chrome_width(&dims, has_icon)).max(1.0);

    let has_usable_size = image_width.is_finite()
        && image_height.is_finite()
        && image_width > 0.0
        && image_height > 0.0;
    if !has_usable_size {
        // NSImage が寸法を返さない場合でも枠だけは出す
        return (max_height, max_height);
    }

    let ratio = (max_width / image_width)
        .min(max_height / image_height)
        .min(1.0);
    (
        (image_width * ratio).round().max(1.0),
        (image_height * ratio).round().max(1.0),
    )
}

/// HUD ウィンドウと3つのサブビュー（アイコン・テキスト・画像）を生成して返す。
///
/// 画像ビューは初期状態では非表示で、画像がコピーされたときだけテキストと入れ替えて見せる。
///
/// # Safety
/// - AppKit のメインスレッドから呼ぶこと。
/// - 戻り値は `alloc/init` で確保した生ポインタ。`window` は使用後に
///   `msg_send![window, close]` で解放すること。`icon_label`・`label`・`image_view` は
///   ウィンドウの contentView に追加済みのため、ウィンドウのクローズとともに解放される。
pub unsafe fn create_hud_window(
    settings: DisplaySettings,
) -> (
    *mut AnyObject,
    *mut AnyObject,
    *mut AnyObject,
    *mut AnyObject,
) {
    use crate::config::parse_f64_value;
    let clamped_scale = parse_f64_value(
        settings.hud_scale,
        DEFAULT_HUD_SCALE,
        MIN_HUD_SCALE,
        MAX_HUD_SCALE,
    );
    let dims = hud_dimensions(clamped_scale);
    let has_icon = !settings.hud_emoji.is_empty();
    let default_width = (600.0 * clamped_scale).clamp(dims.min_width, dims.max_width);
    let default_height = dims.min_height;
    let mut rect = NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size: NSSize {
            width: default_width,
            height: default_height,
        },
    };

    if let Some((x, y)) = hud_origin(default_width, default_height, settings.hud_position) {
        rect.origin = NSPoint { x, y };
    }

    let window: *mut AnyObject = msg_send![class!(NSWindow), alloc];
    let window: *mut AnyObject = msg_send![
        window,
        initWithContentRect: rect
        styleMask: BORDERLESS_MASK
        backing: BACKING_BUFFERED
        defer: false
    ];

    let () = msg_send![window, setOpaque: false];
    let () = msg_send![window, setHasShadow: true];
    let () = msg_send![window, setIgnoresMouseEvents: true];
    let () = msg_send![window, setLevel: FLOATING_WINDOW_LEVEL];

    let clear: *mut AnyObject = msg_send![class!(NSColor), clearColor];
    let () = msg_send![window, setBackgroundColor: clear];

    // ObjC では nil へのメッセージ送信は no-op（nil を返す）。
    // content_view・layer が万一 nil でもスタイリングがスキップされるだけで動作は継続する。
    let content_view: *mut AnyObject = msg_send![window, contentView];
    let () = msg_send![content_view, setWantsLayer: true];
    let layer: *mut AnyObject = msg_send![content_view, layer];
    let corner_radius = (HUD_CORNER_RADIUS * clamped_scale).clamp(8.0, 30.0);
    let () = msg_send![layer, setCornerRadius: corner_radius];
    let () = msg_send![layer, setMasksToBounds: true];

    let (bg_r, bg_g, bg_b, bg_a) = hud_background_rgba(
        settings.hud_background_color,
        settings.hud_background_opacity,
    );
    let bg: *mut AnyObject = msg_send![
        class!(NSColor),
        colorWithCalibratedRed: bg_r
        green: bg_g
        blue: bg_b
        alpha: bg_a
    ];
    let cg_color: *mut std::ffi::c_void = msg_send![bg, CGColor];
    let () = msg_send![layer, setBackgroundColor: cg_color];
    let (border_white, border_alpha) = hud_border_white_alpha(
        settings.hud_background_color,
        settings.hud_background_opacity,
    );
    let border_color_obj: *mut AnyObject =
        msg_send![class!(NSColor), colorWithCalibratedWhite: border_white alpha: border_alpha];
    let border_color: *mut std::ffi::c_void = msg_send![border_color_obj, CGColor];
    let () = msg_send![layer, setBorderColor: border_color];
    let border_width = (HUD_BORDER_WIDTH * clamped_scale).clamp(1.0, 2.5);
    let () = msg_send![layer, setBorderWidth: border_width];

    let icon_rect = NSRect {
        origin: NSPoint {
            x: dims.horizontal_padding,
            y: (default_height - dims.line_height_estimate) / 2.0,
        },
        size: NSSize {
            width: dims.icon_width,
            height: dims.icon_height,
        },
    };

    let icon_label: *mut AnyObject = msg_send![class!(NSTextField), alloc];
    let icon_label: *mut AnyObject = msg_send![icon_label, initWithFrame: icon_rect];
    let () = msg_send![icon_label, setBezeled: false];
    let () = msg_send![icon_label, setBordered: false];
    let () = msg_send![icon_label, setEditable: false];
    let () = msg_send![icon_label, setSelectable: false];
    let () = msg_send![icon_label, setDrawsBackground: false];
    let () = msg_send![icon_label, setHidden: !has_icon];

    let icon_font_size = (HUD_ICON_FONT_SIZE * clamped_scale).clamp(10.0, 44.0);
    let system_font: *mut AnyObject = msg_send![class!(NSFont), systemFontOfSize: icon_font_size];
    if !system_font.is_null() {
        let () = msg_send![icon_label, setFont: system_font];
    }

    let emoji = NSString::from_str(&settings.hud_emoji);
    let () = msg_send![icon_label, setStringValue: &*emoji];

    let label_rect = NSRect {
        origin: NSPoint {
            x: dims.horizontal_padding + icon_reserve(&dims, has_icon),
            y: (default_height - dims.line_height_estimate) / 2.0,
        },
        size: NSSize {
            width: default_width - horizontal_chrome_width(&dims, has_icon),
            height: dims.line_height_estimate,
        },
    };

    let label: *mut AnyObject = msg_send![class!(NSTextField), alloc];
    let label: *mut AnyObject = msg_send![label, initWithFrame: label_rect];

    let () = msg_send![label, setBezeled: false];
    let () = msg_send![label, setBordered: false];
    let () = msg_send![label, setEditable: false];
    let () = msg_send![label, setSelectable: false];
    let () = msg_send![label, setDrawsBackground: false];
    let () = msg_send![label, setLineBreakMode: NS_LINE_BREAK_BY_CHAR_WRAPPING];
    let () = msg_send![label, setUsesSingleLineMode: false];
    let () = msg_send![label, setMaximumNumberOfLines: 0isize];
    let () = msg_send![label, setAlignment: NS_TEXT_ALIGNMENT_LEFT];

    let white: *mut AnyObject = msg_send![class!(NSColor), whiteColor];
    let () = msg_send![label, setTextColor: white];

    let menlo_name = NSString::from_str("Menlo");
    let text_font_size = (HUD_TEXT_FONT_SIZE * clamped_scale).clamp(10.0, 44.0);
    let font: *mut AnyObject =
        msg_send![class!(NSFont), fontWithName: &*menlo_name size: text_font_size];
    if !font.is_null() {
        let () = msg_send![label, setFont: font];
    }

    let cell: *mut AnyObject = msg_send![label, cell];
    if !cell.is_null() {
        let () = msg_send![cell, setWraps: true];
        let () = msg_send![cell, setScrollable: false];
        let () = msg_send![cell, setLineBreakMode: NS_LINE_BREAK_BY_CHAR_WRAPPING];
    }

    let default_text = NSString::from_str("Clipboard text");
    let () = msg_send![label, setStringValue: &*default_text];

    let image_view: *mut AnyObject = msg_send![class!(NSImageView), alloc];
    let image_view: *mut AnyObject = msg_send![image_view, initWithFrame: label_rect];
    let () = msg_send![image_view, setImageScaling: NS_IMAGE_SCALE_PROPORTIONALLY_UP_OR_DOWN];
    let () = msg_send![image_view, setEditable: false];
    let () = msg_send![image_view, setHidden: true];

    let () = msg_send![content_view, addSubview: icon_label];
    let () = msg_send![content_view, addSubview: label];
    let () = msg_send![content_view, addSubview: image_view];
    let () = msg_send![window, orderOut: ptr::null_mut::<AnyObject>()];

    (window, icon_label, label, image_view)
}

/// メインスクリーンの可視領域（Dock・メニューバーを除いた矩形）を返す。
///
/// # Safety
/// AppKit のメインスレッドから呼ぶこと。
unsafe fn main_screen_visible_frame() -> Option<NSRect> {
    let screen: *mut AnyObject = msg_send![class!(NSScreen), mainScreen];
    if screen.is_null() {
        return None;
    }

    let frame: NSRect = msg_send![screen, visibleFrame];
    Some(frame)
}

pub fn hud_origin_for_frame(
    frame: NSRect,
    width: f64,
    height: f64,
    position: HudPosition,
) -> (f64, f64) {
    let min_x = frame.origin.x;
    let max_x = frame.origin.x + (frame.size.width - width).max(0.0);
    let min_y = frame.origin.y;
    let max_y = frame.origin.y + (frame.size.height - height).max(0.0);

    let x = frame.origin.x + (frame.size.width - width) / 2.0;
    let available_height = max_y - min_y;
    let center_y = frame.origin.y + available_height / 2.0;
    let vertical_quarter = available_height / 4.0;
    // AppKit screen coordinates increase upward. "Top" means a larger y value.
    let upper_half_mid_y = center_y + vertical_quarter;
    let lower_half_mid_y = center_y - vertical_quarter;
    let y = match position {
        HudPosition::Top => upper_half_mid_y,
        HudPosition::Center => center_y,
        HudPosition::Bottom => lower_half_mid_y,
    };
    let x = x.clamp(min_x, max_x);
    let y = y.clamp(min_y, max_y);
    (x, y)
}

/// HUD の表示位置 (x, y) をスクリーン座標で返す。スクリーン情報が取得できない場合は `None`。
///
/// # Safety
/// AppKit のメインスレッドから呼ぶこと。
pub unsafe fn hud_origin(width: f64, height: f64, position: HudPosition) -> Option<(f64, f64)> {
    let frame = main_screen_visible_frame()?;
    Some(hud_origin_for_frame(frame, width, height, position))
}

/// ウィンドウをスクリーン上の指定位置に移動・リサイズする。
///
/// # Safety
/// - `window` は有効な NSWindow インスタンスであること。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn position_window(
    window: *mut AnyObject,
    width: f64,
    height: f64,
    position: HudPosition,
) {
    let (x, y) = hud_origin(width, height, position).unwrap_or((0.0, 0.0));

    let rect = NSRect {
        origin: NSPoint { x, y },
        size: NSSize { width, height },
    };
    let () = msg_send![window, setFrame: rect display: true];
}

/// テキスト内容に合わせて HUD のサイズ・位置・ラベルフレームを再計算して適用する。
///
/// # Safety
/// - `window`・`icon_label`・`label` はいずれも有効な ObjC オブジェクトであること。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn layout_hud(
    window: *mut AnyObject,
    icon_label: *mut AnyObject,
    label: *mut AnyObject,
    settings: DisplaySettings,
) {
    let dims = hud_dimensions(settings.hud_scale);
    let has_icon = !settings.hud_emoji.is_empty();
    let clamped_width = measure_text_natural_width(label, settings.hud_scale, has_icon)
        .clamp(dims.min_width, dims.max_width);
    let text_width = clamped_width - horizontal_chrome_width(&dims, has_icon);
    let measured_text_height = measure_text_height(label, text_width, settings.hud_scale);
    let metrics = compute_hud_text_metrics(
        clamped_width,
        measured_text_height,
        settings.hud_scale,
        has_icon,
    );

    let icon_rect = NSRect {
        origin: NSPoint {
            x: dims.horizontal_padding,
            y: metrics.frame.icon_y,
        },
        size: NSSize {
            width: dims.icon_width,
            height: dims.icon_height,
        },
    };
    let label_rect = NSRect {
        origin: NSPoint {
            x: dims.horizontal_padding + icon_reserve(&dims, has_icon),
            y: metrics.label_y,
        },
        size: NSSize {
            width: metrics.text_width,
            height: metrics.text_height,
        },
    };

    let () = msg_send![icon_label, setHidden: !has_icon];
    let () = msg_send![icon_label, setFrame: icon_rect];
    let () = msg_send![label, setFrame: label_rect];
    position_window(
        window,
        metrics.frame.width,
        metrics.frame.height,
        settings.hud_position,
    );
}

/// サムネイル寸法に合わせて HUD のサイズ・位置・画像ビューのフレームを再計算して適用する。
///
/// 画像ビューのフレームはサムネイル寸法そのままで、外形は `HudFrameMetrics` から取る
/// （テキスト専用の下限入りフィールドは型ごと存在しない）。
///
/// # Safety
/// - `window`・`icon_label`・`image_view` はいずれも有効な ObjC オブジェクトであること。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn layout_hud_image(
    window: *mut AnyObject,
    icon_label: *mut AnyObject,
    image_view: *mut AnyObject,
    thumbnail_size: (f64, f64),
    settings: DisplaySettings,
) {
    let dims = hud_dimensions(settings.hud_scale);
    let has_icon = !settings.hud_emoji.is_empty();
    let (thumbnail_width, thumbnail_height) = thumbnail_size;
    let natural_width = thumbnail_width + horizontal_chrome_width(&dims, has_icon);
    let frame = compute_hud_frame_metrics(
        natural_width,
        thumbnail_height,
        settings.hud_scale,
        has_icon,
    );

    let icon_rect = NSRect {
        origin: NSPoint {
            x: dims.horizontal_padding,
            y: frame.icon_y,
        },
        size: NSSize {
            width: dims.icon_width,
            height: dims.icon_height,
        },
    };
    let image_rect = NSRect {
        origin: NSPoint {
            x: dims.horizontal_padding + icon_reserve(&dims, has_icon),
            y: (frame.height - thumbnail_height) / 2.0,
        },
        size: NSSize {
            width: thumbnail_width,
            height: thumbnail_height,
        },
    };

    let () = msg_send![icon_label, setHidden: !has_icon];
    let () = msg_send![icon_label, setFrame: icon_rect];
    let () = msg_send![image_view, setFrame: image_rect];
    position_window(window, frame.width, frame.height, settings.hud_position);
}

/// NSTextField のテキスト内容を1行で表示したときの自然幅（HUD 全幅）を返す。
///
/// # Safety
/// - `label` は有効な NSTextField インスタンスであること。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn measure_text_natural_width(label: *mut AnyObject, scale: f64, has_icon: bool) -> f64 {
    let dims = hud_dimensions(scale);
    let cell: *mut AnyObject = msg_send![label, cell];
    if cell.is_null() {
        return dims.min_width;
    }

    let bounds = NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size: NSSize {
            width: HUD_TEXT_MEASURE_MAX_WIDTH,
            height: HUD_TEXT_MEASURE_HEIGHT,
        },
    };
    let size: NSSize = msg_send![cell, cellSizeForBounds: bounds];
    let text_content_width = size.width.ceil();
    text_content_width + horizontal_chrome_width(&dims, has_icon)
}

/// NSTextField のテキスト内容を `text_width` 幅で折り返したときの高さを返す。
///
/// # Safety
/// - `label` は有効な NSTextField インスタンスであること。
/// - AppKit のメインスレッドから呼ぶこと。
pub unsafe fn measure_text_height(label: *mut AnyObject, text_width: f64, scale: f64) -> f64 {
    let dims = hud_dimensions(scale);
    let cell: *mut AnyObject = msg_send![label, cell];
    if cell.is_null() {
        return dims.line_height_estimate;
    }

    let bounds = NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size: NSSize {
            width: text_width.max(1.0),
            height: HUD_TEXT_MEASURE_HEIGHT,
        },
    };
    let size: NSSize = msg_send![cell, cellSizeForBounds: bounds];
    size.height.ceil().max(dims.line_height_estimate)
}

#[cfg(test)]
pub(crate) fn compute_hud_text_metrics_default(
    width: f64,
    measured_text_height: f64,
) -> HudTextMetrics {
    compute_hud_text_metrics(width, measured_text_height, DEFAULT_HUD_SCALE, true)
}

pub(crate) fn compute_hud_text_metrics(
    width: f64,
    measured_text_height: f64,
    scale: f64,
    has_icon: bool,
) -> HudTextMetrics {
    let dims = hud_dimensions(scale);
    let width = width.clamp(dims.min_width, dims.max_width);
    let text_width = width - horizontal_chrome_width(&dims, has_icon);
    let measured_text_height = measured_text_height
        .min((dims.max_height - dims.vertical_padding * 2.0).max(dims.line_height_estimate));
    let height = (measured_text_height + dims.vertical_padding * 2.0)
        .clamp(dims.min_height, dims.max_height);
    let text_height = (height - dims.vertical_padding * 2.0)
        .min(measured_text_height)
        .max(dims.line_height_estimate);
    let label_y = (height - text_height) / 2.0;
    let icon_y = (label_y + text_height - dims.icon_height)
        .max(dims.vertical_padding)
        .min(height - dims.icon_height - dims.vertical_padding);

    HudTextMetrics {
        frame: HudFrameMetrics {
            width,
            height,
            icon_y,
        },
        text_width,
        text_height,
        label_y,
    }
}

/// 画像など非テキスト内容用の外形計算。外形はテキストと同じ式で決める
/// （アイコンの縦位置は行高を下限にして揃える）が、テキスト専用フィールドを
/// 持たない型で返すことで、下限入りの `text_width` / `text_height` を
/// 内容のフレームに流用する誤りを防ぐ。
pub(crate) fn compute_hud_frame_metrics(
    natural_width: f64,
    content_height: f64,
    scale: f64,
    has_icon: bool,
) -> HudFrameMetrics {
    compute_hud_text_metrics(natural_width, content_height, scale, has_icon).frame
}

#[cfg(test)]
pub(crate) fn hud_width_for_text(text: &str) -> f64 {
    hud_width_for_text_with_scale(text, DEFAULT_HUD_SCALE, true)
}

#[cfg(test)]
pub(crate) fn hud_width_for_text_with_scale(text: &str, scale: f64, has_icon: bool) -> f64 {
    use crate::text::split_non_trailing_lines;
    let dims = hud_dimensions(scale);
    let lines = split_non_trailing_lines(text);
    let max_units = lines
        .iter()
        .map(|line| crate::text::line_display_units(line))
        .fold(1.0f64, f64::max);

    (max_units * dims.char_width_estimate + horizontal_chrome_width(&dims, has_icon))
        .clamp(dims.min_width, dims.max_width)
}

#[cfg(test)]
mod tests {
    use super::{
        compute_hud_frame_metrics, compute_hud_text_metrics_default, fit_thumbnail_size,
        horizontal_chrome_width, hud_background_rgba, hud_border_white_alpha, hud_dimensions,
        hud_origin_for_frame, hud_width_for_text,
    };
    use crate::config::{
        HudBackgroundColor, HudPosition, DEFAULT_HUD_IMAGE_MAX_HEIGHT, DEFAULT_HUD_SCALE,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    /// 既定の opacity（1.0）では背景・枠線のアルファが色ごとの既定値そのままになる。
    /// 倍率の掛け違いを拾えるよう、式を再計算せずリテラルで固定する。
    #[test]
    fn hud_background_rgba_at_default_opacity_keeps_the_color_defaults() {
        assert_eq!(
            hud_background_rgba(HudBackgroundColor::Default, 1.0),
            (0.0, 0.0, 0.0, 0.78)
        );
        assert_eq!(
            hud_border_white_alpha(HudBackgroundColor::Default, 1.0),
            (1.0, 0.14)
        );
    }

    /// opacity は「既定アルファに対する倍率」であって絶対値ではない。RGB は変えず、
    /// アルファだけ opacity 倍される。
    #[test]
    fn hud_background_rgba_at_half_opacity_scales_alpha_only() {
        let (r, g, b, a) = hud_background_rgba(HudBackgroundColor::Default, 0.5);
        assert_eq!((r, g, b), (0.0, 0.0, 0.0));
        assert_eq!(a, 0.39);

        let (r, g, b, a) = hud_background_rgba(HudBackgroundColor::Blue, 0.5);
        assert_eq!((r, g, b), (0.08, 0.22, 0.53));
        assert_eq!(a, 0.45);
    }

    /// 範囲外・NaN は `parse_f64_value` と同じルールでクランプ/既定値化される。
    #[test]
    fn hud_background_rgba_clamps_out_of_range_and_non_finite_opacity() {
        let low = hud_background_rgba(HudBackgroundColor::Default, 0.0);
        let clamped_low = hud_background_rgba(HudBackgroundColor::Default, 0.2);
        assert_eq!(low, clamped_low);

        let high = hud_background_rgba(HudBackgroundColor::Default, 5.0);
        let clamped_high = hud_background_rgba(HudBackgroundColor::Default, 1.0);
        assert_eq!(high, clamped_high);

        let nan = hud_background_rgba(HudBackgroundColor::Default, f64::NAN);
        let default = hud_background_rgba(HudBackgroundColor::Default, 1.0);
        assert_eq!(nan, default);
    }

    #[test]
    fn fit_thumbnail_size_keeps_aspect_ratio_when_shrinking() {
        let (width, height) = fit_thumbnail_size(320.0, 180.0, 100, 1.0, true);
        assert_eq!(height, 100.0);
        assert_eq!(width, (320.0_f64 * (100.0 / 180.0)).round());
    }

    #[test]
    fn fit_thumbnail_size_does_not_upscale_small_images() {
        let (width, height) =
            fit_thumbnail_size(16.0, 16.0, DEFAULT_HUD_IMAGE_MAX_HEIGHT, 2.0, true);
        assert_eq!((width, height), (16.0, 16.0));
    }

    #[test]
    fn fit_thumbnail_size_limits_wide_images_by_hud_width() {
        // 極端な横長は高さ上限ではなく HUD の幅上限で決まる
        let (width, height) =
            fit_thumbnail_size(8000.0, 200.0, DEFAULT_HUD_IMAGE_MAX_HEIGHT, 1.0, true);
        assert!(width <= 820.0 - (16.0 * 2.0 + 22.0 + 8.0));
        assert!(height < 200.0);
    }

    #[test]
    fn fit_thumbnail_size_scales_max_height_with_hud_scale() {
        let (_, small) = fit_thumbnail_size(1000.0, 1000.0, 100, 1.0, true);
        let (_, large) = fit_thumbnail_size(1000.0, 1000.0, 100, 2.0, true);
        assert_eq!(small, 100.0);
        assert_eq!(large, 200.0);
    }

    #[test]
    fn fit_thumbnail_size_caps_max_height_at_hud_height() {
        // 設定上限に hud_scale を掛けた値ではなく、HUD 自体の高さ上限から縦パディングを引いた値で抑えられる
        let (_, height) = fit_thumbnail_size(1000.0, 1000.0, 240, DEFAULT_HUD_SCALE, true);
        assert!(height <= 280.0 * DEFAULT_HUD_SCALE - 10.0 * DEFAULT_HUD_SCALE * 2.0);
    }

    #[test]
    fn fit_thumbnail_size_falls_back_for_unusable_sizes() {
        let expected = (100.0, 100.0);
        assert_eq!(fit_thumbnail_size(0.0, 0.0, 100, 1.0, true), expected);
        assert_eq!(fit_thumbnail_size(-10.0, 20.0, 100, 1.0, true), expected);
        assert_eq!(fit_thumbnail_size(f64::NAN, 20.0, 100, 1.0, true), expected);
        assert_eq!(
            fit_thumbnail_size(20.0, f64::INFINITY, 100, 1.0, true),
            expected
        );
    }

    #[test]
    fn fit_thumbnail_size_without_icon_allows_wider_thumbnail() {
        // アイコン分の幅（22.0 + 8.0 = 30.0）を確保しない分、横長画像がより広く収まる
        let (with_icon, _) =
            fit_thumbnail_size(8000.0, 200.0, DEFAULT_HUD_IMAGE_MAX_HEIGHT, 1.0, true);
        let (without_icon, _) =
            fit_thumbnail_size(8000.0, 200.0, DEFAULT_HUD_IMAGE_MAX_HEIGHT, 1.0, false);
        assert_eq!(without_icon - with_icon, 30.0);
    }

    #[test]
    fn hud_width_regression_snapshot() {
        let cases = [
            ("ascii_short", "hello".to_string()),
            ("ascii_40", "a".repeat(40)),
            ("wide_20", "あ".repeat(20)),
            ("ascii_very_long", "a".repeat(300)),
        ];

        let snapshot = cases
            .iter()
            .map(|(name, text)| format!("{name}: {:.1}", hud_width_for_text(text)))
            .collect::<Vec<_>>()
            .join("\n");

        let expected = "\
ascii_short: 220.0
ascii_40: 490.6
wide_20: 490.6
ascii_very_long: 902.0";

        assert_eq!(snapshot, expected);
    }

    #[test]
    fn hud_layout_regression_snapshot() {
        let cases = [
            ("one_line", 600.0, 22.0),
            ("three_lines", 600.0, 88.0),
            ("overflow", 600.0, 400.0),
            ("narrow_clamped", 100.0, 22.0),
        ];

        let snapshot = cases
            .iter()
            .map(|(name, width, measured)| {
                let metrics = compute_hud_text_metrics_default(*width, *measured);
                format!(
                    "{name}: w={:.1} text_w={:.1} h={:.1} text_h={:.1} label_y={:.1} icon_y={:.1}",
                    metrics.frame.width,
                    metrics.text_width,
                    metrics.frame.height,
                    metrics.text_height,
                    metrics.label_y,
                    metrics.frame.icon_y
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let expected = "\
one_line: w=600.0 text_w=531.8 h=57.2 text_h=24.2 label_y=16.5 icon_y=16.5
three_lines: w=600.0 text_w=531.8 h=110.0 text_h=88.0 label_y=11.0 icon_y=74.8
overflow: w=600.0 text_w=531.8 h=308.0 text_h=286.0 label_y=11.0 icon_y=272.8
narrow_clamped: w=220.0 text_w=151.8 h=57.2 text_h=24.2 label_y=16.5 icon_y=16.5";

        assert_eq!(snapshot, expected);
    }

    /// 画像経路の外形計算。VRT の image_tiny ベースライン（16x16・既定設定）が固定している
    /// 2 つの挙動を張る: 細い画像でも幅は最小幅までクランプされること、内容が行高より
    /// 低くてもアイコンの縦位置は行高を下限にして揃えること。
    #[test]
    fn frame_metrics_keep_icon_alignment_for_short_content() {
        let dims = hud_dimensions(DEFAULT_HUD_SCALE);
        let natural_width = 16.0 + horizontal_chrome_width(&dims, true);
        let frame = compute_hud_frame_metrics(natural_width, 16.0, DEFAULT_HUD_SCALE, true);
        assert_eq!(
            format!(
                "w={:.1} h={:.1} icon_y={:.1}",
                frame.width, frame.height, frame.icon_y
            ),
            "w=220.0 h=57.2 icon_y=16.5"
        );
    }

    #[test]
    fn hud_origin_for_frame_positions_by_setting() {
        let frame = NSRect {
            origin: NSPoint { x: 0.0, y: 0.0 },
            size: NSSize {
                width: 1000.0,
                height: 800.0,
            },
        };
        let hud_width = 600.0;
        let hud_height = 100.0;

        let (top_x, top_y) = hud_origin_for_frame(frame, hud_width, hud_height, HudPosition::Top);
        let (center_x, center_y) =
            hud_origin_for_frame(frame, hud_width, hud_height, HudPosition::Center);
        let (bottom_x, bottom_y) =
            hud_origin_for_frame(frame, hud_width, hud_height, HudPosition::Bottom);

        assert_eq!(top_x, 200.0);
        assert_eq!(center_x, 200.0);
        assert_eq!(bottom_x, 200.0);
        assert_eq!(top_y, 525.0);
        assert_eq!(center_y, 350.0);
        assert_eq!(bottom_y, 175.0);
    }
}
