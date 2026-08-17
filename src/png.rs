use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::config::display_settings;
use crate::error::AppError;
use crate::hud::{create_hud_window, fit_thumbnail_size, layout_hud, layout_hud_image};
use crate::objc_helpers::nsstring_from_str;
use crate::text::truncate_text;

const BITMAP_IMAGE_FILE_TYPE_PNG: usize = 4;
const PIXEL_CHANNEL_TOLERANCE: u8 = 2;
/// フィクスチャ画像の市松模様を短辺方向に何分割するか。
/// ブロックを画像サイズに比例させることで、縮小表示されてもブロック境界以外の色が変わらず、
/// マシン間で補間結果がぶれてもベースラインが安定する。
const FIXTURE_BLOCKS_PER_SHORT_SIDE: usize = 4;
const FIXTURE_COLOR_A: [u8; 4] = [230, 80, 60, 255];
const FIXTURE_COLOR_B: [u8; 4] = [40, 120, 220, 255];

const PREVIEW_SAMPLE_IMAGE_WIDTH: usize = 320;
const PREVIEW_SAMPLE_IMAGE_HEIGHT: usize = 180;
const PREVIEW_SAMPLE_LABEL_FONT_SIZE: f64 = 28.0;

// AppKit の NSAttributedString 属性キー定数（NSAttributedString.h）。
// objc2-app-kit を新たに使わず、他の呼び出しと同じ手動 FFI のまま参照する。
extern "C" {
    static NSFontAttributeName: *mut AnyObject;
    static NSForegroundColorAttributeName: *mut AnyObject;
    static NSShadowAttributeName: *mut AnyObject;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffSummary {
    pub diff_pixels: usize,
    pub total_pixels: usize,
}

pub fn render_hud_png(text: &str, output_path: &str) -> Result<(), AppError> {
    unsafe {
        let _app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let settings = display_settings();
        let (window, icon_label, label, _image_view) = create_hud_window(settings.clone());
        if window.is_null() {
            return Err(AppError::RenderFailed(
                "failed to create HUD window".to_string(),
            ));
        }
        let truncated = truncate_text(
            text,
            settings.truncate_max_width,
            settings.truncate_max_lines,
        );
        let message = nsstring_from_str(&truncated);
        let () = msg_send![label, setStringValue: message];
        let () = msg_send![message, release];
        layout_hud(window, icon_label, label, settings);

        write_window_png(window, output_path)
    }
}

/// 指定サイズのフィクスチャ画像を載せた HUD のスナップショット PNG を書き出す（VRT 用）。
pub fn render_hud_image_png(
    image_width: usize,
    image_height: usize,
    output_path: &str,
) -> Result<(), AppError> {
    unsafe {
        let _app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let settings = display_settings();
        let (window, icon_label, label, image_view) = create_hud_window(settings.clone());
        if window.is_null() {
            return Err(AppError::RenderFailed(
                "failed to create HUD window".to_string(),
            ));
        }

        let image = match create_fixture_image(image_width, image_height) {
            Ok(image) => image,
            Err(error) => {
                let () = msg_send![window, close];
                return Err(error);
            }
        };
        let thumbnail = fit_thumbnail_size(
            image_width as f64,
            image_height as f64,
            settings.hud_image_max_height,
            settings.hud_scale,
            !settings.hud_emoji.is_empty(),
        );

        let () = msg_send![image_view, setImage: image];
        let () = msg_send![image, release];
        let () = msg_send![label, setHidden: true];
        let () = msg_send![image_view, setHidden: false];
        layout_hud_image(window, icon_label, image_view, thumbnail, settings);

        write_window_png(window, output_path)
    }
}

/// VRT 用の決定的な市松模様ビットマップを NSImage として生成する。
///
/// 外部の PNG ファイルを読み込ませないのは、色プロファイルやデコーダの差で
/// ベースラインがマシンごとにぶれるのを避けるため。
///
/// # Safety
/// AppKit のメインスレッドから呼ぶこと。戻り値は所有権 +1 の NSImage。
pub(crate) unsafe fn create_fixture_image(
    width: usize,
    height: usize,
) -> Result<*mut AnyObject, AppError> {
    if width == 0 || height == 0 {
        return Err(AppError::RenderFailed(
            "fixture image size must be positive".to_string(),
        ));
    }

    let size = NSSize {
        width: width as f64,
        height: height as f64,
    };
    let rep = create_bitmap_rep_for_bounds(NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size,
    })?;

    let pixels: *mut u8 = msg_send![rep, bitmapData];
    if pixels.is_null() {
        let () = msg_send![rep, release];
        return Err(AppError::RenderFailed(
            "failed to access fixture bitmap data".to_string(),
        ));
    }
    let bytes_per_row: usize = msg_send![rep, bytesPerRow];
    let buffer = std::slice::from_raw_parts_mut(pixels, bytes_per_row * height);

    let block_size = (width.min(height) / FIXTURE_BLOCKS_PER_SHORT_SIDE).max(1);
    for y in 0..height {
        for x in 0..width {
            let block_is_a = ((x / block_size) + (y / block_size)).is_multiple_of(2);
            let color = if block_is_a {
                FIXTURE_COLOR_A
            } else {
                FIXTURE_COLOR_B
            };
            let offset = y * bytes_per_row + x * 4;
            buffer[offset..offset + 4].copy_from_slice(&color);
        }
    }

    let image: *mut AnyObject = msg_send![class!(NSImage), alloc];
    let image: *mut AnyObject = msg_send![image, initWithSize: size];
    if image.is_null() {
        let () = msg_send![rep, release];
        return Err(AppError::RenderFailed(
            "failed to allocate fixture image".to_string(),
        ));
    }
    let () = msg_send![image, addRepresentation: rep];
    let () = msg_send![rep, release];

    Ok(image)
}

/// お試し表示の画像サンプルを生成する。`create_fixture_image` と同じ市松模様の上に
/// "Sample" の文字を重ねる。VRT が使う `create_fixture_image` の出力（ベースライン依存）は
/// 変えず、別関数として重ね書きする。
///
/// # Safety
/// AppKit のメインスレッドから呼ぶこと。戻り値は所有権 +1 の NSImage。
pub(crate) unsafe fn create_preview_sample_image() -> Result<*mut AnyObject, AppError> {
    let image = create_fixture_image(PREVIEW_SAMPLE_IMAGE_WIDTH, PREVIEW_SAMPLE_IMAGE_HEIGHT)?;

    let () = msg_send![image, lockFocus];
    draw_preview_sample_label(
        PREVIEW_SAMPLE_IMAGE_WIDTH as f64,
        PREVIEW_SAMPLE_IMAGE_HEIGHT as f64,
    );
    let () = msg_send![image, unlockFocus];

    Ok(image)
}

/// 現在の描画コンテキスト（`create_preview_sample_image` の `lockFocus` 中）に
/// 中央寄せ・白ボールド・影付きで "Sample" を描く。市松模様の上でも視認できるようにするため。
unsafe fn draw_preview_sample_label(canvas_width: f64, canvas_height: f64) {
    let text = nsstring_from_str("Sample");

    let shadow: *mut AnyObject = msg_send![class!(NSShadow), alloc];
    let shadow: *mut AnyObject = msg_send![shadow, init];
    let shadow_color: *mut AnyObject =
        msg_send![class!(NSColor), colorWithCalibratedWhite: 0.0f64 alpha: 0.85f64];
    let () = msg_send![shadow, setShadowColor: shadow_color];
    let () = msg_send![shadow, setShadowOffset: NSSize { width: 0.0, height: -1.0 }];
    let () = msg_send![shadow, setShadowBlurRadius: 3.0f64];

    let font: *mut AnyObject =
        msg_send![class!(NSFont), boldSystemFontOfSize: PREVIEW_SAMPLE_LABEL_FONT_SIZE];
    let white: *mut AnyObject = msg_send![class!(NSColor), whiteColor];

    let attributes: *mut AnyObject = msg_send![class!(NSMutableDictionary), dictionary];
    let () = msg_send![attributes, setObject: font forKey: NSFontAttributeName];
    let () = msg_send![attributes, setObject: white forKey: NSForegroundColorAttributeName];
    let () = msg_send![attributes, setObject: shadow forKey: NSShadowAttributeName];

    let text_size: NSSize = msg_send![text, sizeWithAttributes: attributes];
    let text_rect = NSRect {
        origin: NSPoint {
            x: ((canvas_width - text_size.width) / 2.0).max(0.0),
            y: ((canvas_height - text_size.height) / 2.0).max(0.0),
        },
        size: text_size,
    };
    let () = msg_send![text, drawInRect: text_rect withAttributes: attributes];

    let () = msg_send![text, release];
    let () = msg_send![shadow, release];
}

/// HUD ウィンドウの contentView を PNG にして書き出す。
///
/// # Safety
/// - `window` は有効な NSWindow であること。呼び出し後にクローズされる。
/// - AppKit のメインスレッドから呼ぶこと。
unsafe fn write_window_png(window: *mut AnyObject, output_path: &str) -> Result<(), AppError> {
    let content_view: *mut AnyObject = msg_send![window, contentView];
    if content_view.is_null() {
        let () = msg_send![window, close];
        return Err(AppError::RenderFailed(
            "failed to get contentView".to_string(),
        ));
    }

    let bounds: NSRect = msg_send![content_view, bounds];
    let bitmap = match create_bitmap_rep_for_bounds(bounds) {
        Ok(b) => b,
        Err(e) => {
            let () = msg_send![window, close];
            return Err(e);
        }
    };

    let () = msg_send![content_view, cacheDisplayInRect: bounds toBitmapImageRep: bitmap];
    let properties: *mut AnyObject = msg_send![class!(NSDictionary), dictionary];
    let data: *mut AnyObject = msg_send![
        bitmap,
        representationUsingType: BITMAP_IMAGE_FILE_TYPE_PNG
        properties: properties
    ];
    if data.is_null() {
        let () = msg_send![bitmap, release];
        let () = msg_send![window, close];
        return Err(AppError::RenderFailed(
            "failed to encode PNG data".to_string(),
        ));
    }

    let output_path_ns = nsstring_from_str(output_path);
    let success: bool = msg_send![data, writeToFile: output_path_ns atomically: true];
    let () = msg_send![output_path_ns, release];
    let () = msg_send![bitmap, release];
    let () = msg_send![window, close];

    if !success {
        return Err(AppError::RenderFailed(format!(
            "failed to write PNG: {output_path}"
        )));
    }

    Ok(())
}

pub fn generate_diff_png(
    baseline_path: &str,
    current_path: &str,
    output_path: &str,
) -> Result<DiffSummary, AppError> {
    unsafe {
        let baseline_path_ns = nsstring_from_str(baseline_path);
        let baseline_rep: *mut AnyObject =
            msg_send![class!(NSBitmapImageRep), imageRepWithContentsOfFile: baseline_path_ns];
        let () = msg_send![baseline_path_ns, release];
        if baseline_rep.is_null() {
            return Err(AppError::RenderFailed(format!(
                "failed to load baseline PNG: {baseline_path}"
            )));
        }
        // imageRepWithContentsOfFile: は autoreleased を返すため、明示的に retain して所有権を確保
        let _: *mut AnyObject = msg_send![baseline_rep, retain];

        let current_path_ns = nsstring_from_str(current_path);
        let current_rep: *mut AnyObject =
            msg_send![class!(NSBitmapImageRep), imageRepWithContentsOfFile: current_path_ns];
        let () = msg_send![current_path_ns, release];
        if current_rep.is_null() {
            let () = msg_send![baseline_rep, release];
            return Err(AppError::RenderFailed(format!(
                "failed to load current PNG: {current_path}"
            )));
        }
        let _: *mut AnyObject = msg_send![current_rep, retain];

        let baseline_width: isize = msg_send![baseline_rep, pixelsWide];
        let baseline_height: isize = msg_send![baseline_rep, pixelsHigh];
        let current_width: isize = msg_send![current_rep, pixelsWide];
        let current_height: isize = msg_send![current_rep, pixelsHigh];
        if baseline_width != current_width || baseline_height != current_height {
            let () = msg_send![baseline_rep, release];
            let () = msg_send![current_rep, release];
            return Err(AppError::RenderFailed(format!(
                "image size mismatch: baseline={}x{}, current={}x{}",
                baseline_width, baseline_height, current_width, current_height
            )));
        }

        let diff_rep: *mut AnyObject = msg_send![current_rep, copy];
        if diff_rep.is_null() {
            let () = msg_send![baseline_rep, release];
            let () = msg_send![current_rep, release];
            return Err(AppError::RenderFailed(
                "failed to create diff image".to_string(),
            ));
        }

        let mut diff_pixels: usize = 0;
        let total_pixels = (baseline_width * baseline_height) as usize;

        // bitmapData で直接バッファアクセスし、ピクセルごとの ObjC メッセージ送信を回避
        let baseline_data: *mut u8 = msg_send![baseline_rep, bitmapData];
        let current_data: *mut u8 = msg_send![current_rep, bitmapData];
        let diff_data: *mut u8 = msg_send![diff_rep, bitmapData];
        let bytes_per_row_baseline: isize = msg_send![baseline_rep, bytesPerRow];
        let bytes_per_row_current: isize = msg_send![current_rep, bytesPerRow];
        let bytes_per_row_diff: isize = msg_send![diff_rep, bytesPerRow];

        if bytes_per_row_baseline <= 0 || bytes_per_row_current <= 0 || bytes_per_row_diff <= 0 {
            let () = msg_send![baseline_rep, release];
            let () = msg_send![current_rep, release];
            let () = msg_send![diff_rep, release];
            return Err(AppError::RenderFailed(
                "bitmap bytesPerRow must be positive".to_string(),
            ));
        }
        let bytes_per_row_baseline = bytes_per_row_baseline as usize;
        let bytes_per_row_current = bytes_per_row_current as usize;
        let bytes_per_row_diff = bytes_per_row_diff as usize;

        let min_bytes = baseline_width as usize * 4;
        if bytes_per_row_baseline < min_bytes
            || bytes_per_row_current < min_bytes
            || bytes_per_row_diff < min_bytes
        {
            let () = msg_send![baseline_rep, release];
            let () = msg_send![current_rep, release];
            let () = msg_send![diff_rep, release];
            return Err(AppError::RenderFailed(
                "bitmap data layout mismatch: bytes_per_row too small".to_string(),
            ));
        }

        if baseline_data.is_null() || current_data.is_null() || diff_data.is_null() {
            // bitmapData が取れない場合は ObjC API にフォールバック
            for x in 0..baseline_width {
                for y in 0..baseline_height {
                    let baseline_color: *mut AnyObject = msg_send![baseline_rep, colorAtX: x y: y];
                    let current_color: *mut AnyObject = msg_send![current_rep, colorAtX: x y: y];
                    let Some((br, bg, bb, ba)) = color_components(baseline_color) else {
                        continue;
                    };
                    let Some((cr, cg, cb, ca)) = color_components(current_color) else {
                        continue;
                    };
                    let same = pixel_is_same(
                        PixelColor {
                            r: br,
                            g: bg,
                            b: bb,
                            a: ba,
                        },
                        PixelColor {
                            r: cr,
                            g: cg,
                            b: cb,
                            a: ca,
                        },
                    );
                    let color: *mut AnyObject = if same {
                        let gray = ((cr + cg + cb) / 3.0).clamp(0.0, 1.0);
                        msg_send![class!(NSColor), colorWithCalibratedRed: gray green: gray blue: gray alpha: 0.08f64]
                    } else {
                        diff_pixels += 1;
                        let delta = (to_u8(cr).abs_diff(to_u8(br)))
                            .max(to_u8(cg).abs_diff(to_u8(bg)))
                            .max(to_u8(cb).abs_diff(to_u8(bb)));
                        let intensity = (f64::from(delta.max(128))) / 255.0;
                        msg_send![class!(NSColor), colorWithCalibratedRed: intensity green: 0.0f64 blue: 0.0f64 alpha: 0.9f64]
                    };
                    let () = msg_send![diff_rep, setColor: color atX: x y: y];
                }
            }
        } else {
            // 直接バッファ操作: RGBA 8bpp を仮定（create_bitmap_rep_for_bounds と同じ設定）
            // Safety:
            // - bytes_per_row >= width * 4 は直前の境界チェックで保証済み
            // - データポインタは null でないことを直前でチェック済み
            // - ループ変数 y < height, x < width なのでオフセットはバッファ範囲内
            for y in 0..baseline_height as usize {
                for x in 0..baseline_width as usize {
                    let b_offset = y * bytes_per_row_baseline + x * 4;
                    let c_offset = y * bytes_per_row_current + x * 4;
                    let d_offset = y * bytes_per_row_diff + x * 4;
                    let br = *baseline_data.add(b_offset);
                    let bg = *baseline_data.add(b_offset + 1);
                    let bb = *baseline_data.add(b_offset + 2);
                    let ba = *baseline_data.add(b_offset + 3);
                    let cr = *current_data.add(c_offset);
                    let cg = *current_data.add(c_offset + 1);
                    let cb = *current_data.add(c_offset + 2);
                    let ca = *current_data.add(c_offset + 3);

                    let same = br.abs_diff(cr) <= PIXEL_CHANNEL_TOLERANCE
                        && bg.abs_diff(cg) <= PIXEL_CHANNEL_TOLERANCE
                        && bb.abs_diff(cb) <= PIXEL_CHANNEL_TOLERANCE
                        && ba.abs_diff(ca) <= PIXEL_CHANNEL_TOLERANCE;

                    if same {
                        let gray = (cr as u16 + cg as u16 + cb as u16) / 3;
                        let dimmed = (gray as u8).saturating_mul(2) / 25; // ~8% opacity
                        *diff_data.add(d_offset) = dimmed;
                        *diff_data.add(d_offset + 1) = dimmed;
                        *diff_data.add(d_offset + 2) = dimmed;
                        *diff_data.add(d_offset + 3) = 20; // alpha ~8%
                    } else {
                        diff_pixels += 1;
                        let delta = cr.abs_diff(br).max(cg.abs_diff(bg)).max(cb.abs_diff(bb));
                        let intensity = delta.max(128);
                        *diff_data.add(d_offset) = intensity;
                        *diff_data.add(d_offset + 1) = 0;
                        *diff_data.add(d_offset + 2) = 0;
                        *diff_data.add(d_offset + 3) = 230; // alpha ~90%
                    }
                }
            }
        }

        let properties: *mut AnyObject = msg_send![class!(NSDictionary), dictionary];
        let data: *mut AnyObject = msg_send![
            diff_rep,
            representationUsingType: BITMAP_IMAGE_FILE_TYPE_PNG
            properties: properties
        ];
        if data.is_null() {
            let () = msg_send![baseline_rep, release];
            let () = msg_send![current_rep, release];
            let () = msg_send![diff_rep, release];
            return Err(AppError::RenderFailed(
                "failed to encode diff PNG".to_string(),
            ));
        }

        let output_path_ns = nsstring_from_str(output_path);
        let success: bool = msg_send![data, writeToFile: output_path_ns atomically: true];
        let () = msg_send![output_path_ns, release];
        let () = msg_send![baseline_rep, release];
        let () = msg_send![current_rep, release];
        let () = msg_send![diff_rep, release];

        if !success {
            return Err(AppError::RenderFailed(format!(
                "failed to write diff PNG: {output_path}"
            )));
        }

        Ok(DiffSummary {
            diff_pixels,
            total_pixels,
        })
    }
}

unsafe fn color_components(color: *mut AnyObject) -> Option<(f64, f64, f64, f64)> {
    if color.is_null() {
        return None;
    }

    let device_rgb_name = nsstring_from_str("NSDeviceRGBColorSpace");
    let rgb_color: *mut AnyObject = msg_send![color, colorUsingColorSpaceName: device_rgb_name];
    let () = msg_send![device_rgb_name, release];
    if rgb_color.is_null() {
        return None;
    }

    let r: f64 = msg_send![rgb_color, redComponent];
    let g: f64 = msg_send![rgb_color, greenComponent];
    let b: f64 = msg_send![rgb_color, blueComponent];
    let a: f64 = msg_send![rgb_color, alphaComponent];
    Some((r, g, b, a))
}

fn to_u8(component: f64) -> u8 {
    (component.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[derive(Clone, Copy)]
struct PixelColor {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

fn pixel_is_same(baseline: PixelColor, current: PixelColor) -> bool {
    to_u8(baseline.r).abs_diff(to_u8(current.r)) <= PIXEL_CHANNEL_TOLERANCE
        && to_u8(baseline.g).abs_diff(to_u8(current.g)) <= PIXEL_CHANNEL_TOLERANCE
        && to_u8(baseline.b).abs_diff(to_u8(current.b)) <= PIXEL_CHANNEL_TOLERANCE
        && to_u8(baseline.a).abs_diff(to_u8(current.a)) <= PIXEL_CHANNEL_TOLERANCE
}

pub fn create_bitmap_rep_for_bounds(bounds: NSRect) -> Result<*mut AnyObject, AppError> {
    let width = bounds.size.width.ceil().max(1.0) as isize;
    let height = bounds.size.height.ceil().max(1.0) as isize;
    unsafe {
        let bitmap: *mut AnyObject = msg_send![class!(NSBitmapImageRep), alloc];
        let color_space = nsstring_from_str("NSCalibratedRGBColorSpace");
        let bitmap: *mut AnyObject = msg_send![
            bitmap,
            initWithBitmapDataPlanes: std::ptr::null_mut::<*mut u8>()
            pixelsWide: width
            pixelsHigh: height
            bitsPerSample: 8isize
            samplesPerPixel: 4isize
            hasAlpha: true
            isPlanar: false
            colorSpaceName: color_space
            bytesPerRow: 0isize
            bitsPerPixel: 0isize
        ];
        let () = msg_send![color_space, release];

        if bitmap.is_null() {
            return Err(AppError::RenderFailed(
                "failed to allocate fixed-size bitmap image rep".to_string(),
            ));
        }

        Ok(bitmap)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    use super::{
        create_bitmap_rep_for_bounds, generate_diff_png, BITMAP_IMAGE_FILE_TYPE_PNG,
        PIXEL_CHANNEL_TOLERANCE,
    };
    use crate::objc_helpers::nsstring_from_str;

    /// テストごとに独立したディレクトリを使う（cargo test は並列実行のため共有すると干渉する）。
    /// 名前には連番も混ぜ、`test_name` をコピペで重複させても並列テスト同士が
    /// ディレクトリを共有しないようにする。
    struct TempPngDir(PathBuf);

    impl TempPngDir {
        fn new(test_name: &str) -> Self {
            static DIR_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let seq = DIR_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "cliip-show-png-test-{}-{seq}-{test_name}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }

        fn path(&self, file_name: &str) -> String {
            self.0.join(file_name).to_string_lossy().into_owned()
        }
    }

    impl Drop for TempPngDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// ピクセル関数から PNG を書き出す。`create_fixture_image` は Safety 契約が
    /// メインスレッド前提なので使わず、`write_window_png` と同型の NSBitmapImageRep
    /// 直書きにする。テストスレッドから呼んでもメインスレッド制約に触れない。
    ///
    /// alpha は 255 固定で使うこと。alpha < 255 のピクセルは PNG の premultiplied alpha 変換で
    /// ロード時にチャンネル値がずれ、期待値が非決定になる。
    unsafe fn write_test_png(
        path: &str,
        width: usize,
        height: usize,
        pixel: impl Fn(usize, usize) -> [u8; 4],
    ) {
        let rep = create_bitmap_rep_for_bounds(NSRect {
            origin: NSPoint { x: 0.0, y: 0.0 },
            size: NSSize {
                width: width as f64,
                height: height as f64,
            },
        })
        .expect("create bitmap rep");

        let data: *mut u8 = msg_send![rep, bitmapData];
        assert!(!data.is_null());
        let bytes_per_row: usize = msg_send![rep, bytesPerRow];
        for y in 0..height {
            for x in 0..width {
                let offset = y * bytes_per_row + x * 4;
                let color = pixel(x, y);
                std::slice::from_raw_parts_mut(data.add(offset), 4).copy_from_slice(&color);
            }
        }

        let properties: *mut AnyObject = msg_send![objc2::class!(NSDictionary), dictionary];
        let png: *mut AnyObject = msg_send![
            rep,
            representationUsingType: BITMAP_IMAGE_FILE_TYPE_PNG
            properties: properties
        ];
        assert!(!png.is_null());
        let path_ns = nsstring_from_str(path);
        let success: bool = msg_send![png, writeToFile: path_ns atomically: true];
        let () = msg_send![path_ns, release];
        let () = msg_send![rep, release];
        assert!(success, "failed to write test PNG: {path}");
    }

    /// PNG から 1 ピクセルを読み戻す。diff PNG の成果物検証用。
    unsafe fn read_png_pixel(path: &str, x: usize, y: usize) -> [u8; 4] {
        let path_ns = nsstring_from_str(path);
        let rep: *mut AnyObject = msg_send![
            objc2::class!(NSBitmapImageRep),
            imageRepWithContentsOfFile: path_ns
        ];
        let () = msg_send![path_ns, release];
        assert!(!rep.is_null(), "failed to load PNG: {path}");
        let _: *mut AnyObject = msg_send![rep, retain];

        let data: *mut u8 = msg_send![rep, bitmapData];
        assert!(!data.is_null());
        let bytes_per_row: usize = msg_send![rep, bytesPerRow];
        let mut pixel = [0u8; 4];
        pixel.copy_from_slice(std::slice::from_raw_parts(
            data.add(y * bytes_per_row + x * 4),
            4,
        ));
        let () = msg_send![rep, release];
        pixel
    }

    const BASE_COLOR: [u8; 4] = [100, 100, 100, 255];

    #[test]
    fn identical_images_have_zero_diff_pixels() {
        let temp = TempPngDir::new("identical");
        let baseline = temp.path("baseline.png");
        let current = temp.path("current.png");
        unsafe {
            write_test_png(&baseline, 8, 6, |_, _| BASE_COLOR);
            write_test_png(&current, 8, 6, |_, _| BASE_COLOR);
        }

        let diff = temp.path("diff.png");
        let summary = generate_diff_png(&baseline, &current, &diff).expect("diff");
        assert_eq!(summary.diff_pixels, 0);
        assert_eq!(summary.total_pixels, 8 * 6);
        // VRT スクリプトが消費する成果物なので、書き出しの成功まで見る
        assert!(std::path::Path::new(&diff).is_file());
    }

    #[test]
    fn a_single_changed_pixel_is_counted_once() {
        let temp = TempPngDir::new("one-pixel");
        let baseline = temp.path("baseline.png");
        let current = temp.path("current.png");
        unsafe {
            write_test_png(&baseline, 8, 6, |_, _| BASE_COLOR);
            write_test_png(&current, 8, 6, |x, y| {
                if x == 3 && y == 2 {
                    [200, 0, 0, 255]
                } else {
                    BASE_COLOR
                }
            });
        }

        let diff = temp.path("diff.png");
        let summary = generate_diff_png(&baseline, &current, &diff).expect("diff");
        assert_eq!(summary.diff_pixels, 1);

        // diff PNG 上で、変えた座標だけが赤くマークされること（x/y 転置や stride ミスの検出）。
        // アルファ 255 未満のピクセルは premultiplied alpha の丸めで読み戻し値が数段階ずれるため、
        // 正確な値ではなく「赤優位」と「グレー（R=G=B）」で判定する
        unsafe {
            let marked = read_png_pixel(&diff, 3, 2);
            assert!(
                marked[0] >= 100 && marked[1] == 0 && marked[2] == 0,
                "changed pixel not marked red: {marked:?}"
            );
            let unmarked = read_png_pixel(&diff, 0, 0);
            assert!(
                unmarked[0] == unmarked[1] && unmarked[1] == unmarked[2],
                "unchanged pixel not gray: {unmarked:?}"
            );
        }
    }

    /// チャンネル差がちょうど許容値のときは同一扱い、許容値 + 1 で差分扱いになること
    /// （`PIXEL_CHANNEL_TOLERANCE` の off-by-one 検出）。
    #[test]
    fn channel_tolerance_boundary_splits_same_and_diff() {
        let temp = TempPngDir::new("tolerance");
        let baseline = temp.path("baseline.png");
        let within = temp.path("within.png");
        let beyond = temp.path("beyond.png");
        let delta_within = BASE_COLOR[0] + PIXEL_CHANNEL_TOLERANCE;
        let delta_beyond = BASE_COLOR[0] + PIXEL_CHANNEL_TOLERANCE + 1;
        unsafe {
            write_test_png(&baseline, 8, 6, |_, _| BASE_COLOR);
            write_test_png(&within, 8, 6, |_, _| [delta_within, 100, 100, 255]);
            write_test_png(&beyond, 8, 6, |_, _| [delta_beyond, 100, 100, 255]);
        }

        let summary =
            generate_diff_png(&baseline, &within, &temp.path("diff-within.png")).expect("diff");
        assert_eq!(summary.diff_pixels, 0);

        let summary =
            generate_diff_png(&baseline, &beyond, &temp.path("diff-beyond.png")).expect("diff");
        assert_eq!(summary.diff_pixels, 8 * 6);
    }

    #[test]
    fn size_mismatch_is_an_error() {
        let temp = TempPngDir::new("size-mismatch");
        let baseline = temp.path("baseline.png");
        let current = temp.path("current.png");
        unsafe {
            write_test_png(&baseline, 8, 6, |_, _| BASE_COLOR);
            write_test_png(&current, 8, 7, |_, _| BASE_COLOR);
        }

        let error = generate_diff_png(&baseline, &current, &temp.path("diff.png"))
            .expect_err("must fail on size mismatch");
        assert!(error.to_string().contains("size mismatch"), "{error}");
    }

    #[test]
    fn missing_baseline_is_an_error() {
        let temp = TempPngDir::new("missing-baseline");
        let current = temp.path("current.png");
        unsafe {
            write_test_png(&current, 8, 6, |_, _| BASE_COLOR);
        }

        let error = generate_diff_png(&temp.path("no-such.png"), &current, &temp.path("diff.png"))
            .expect_err("must fail on missing baseline");
        assert!(error.to_string().contains("baseline"), "{error}");
    }
}
