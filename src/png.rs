use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_foundation::NSRect;

use crate::config::display_settings;
use crate::error::AppError;
use crate::hud::{create_hud_window, layout_hud};
use crate::objc_helpers::nsstring_from_str;
use crate::text::truncate_text;

const BITMAP_IMAGE_FILE_TYPE_PNG: usize = 4;
const PIXEL_CHANNEL_TOLERANCE: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffSummary {
    pub diff_pixels: usize,
    pub total_pixels: usize,
}

pub fn render_hud_png(text: &str, output_path: &str) -> Result<(), AppError> {
    unsafe {
        let _app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let settings = display_settings();
        let (window, icon_label, label) = create_hud_window(settings.clone());
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
