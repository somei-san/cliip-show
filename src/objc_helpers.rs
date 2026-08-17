use std::ffi::{c_char, c_void, CStr};
use std::ptr;

use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_foundation::NSSize;

const UTF8_ENCODING: usize = 4;

/// `&str` から NSString を生成して返す。
///
/// # Safety
/// 返されたポインタは `alloc/init` で手動管理されるため、
/// 呼び出し側が `msg_send![ptr, release]` で解放する責任を持つ。
pub unsafe fn nsstring_from_str(value: &str) -> *mut AnyObject {
    let ns_string: *mut AnyObject = msg_send![class!(NSString), alloc];
    msg_send![
        ns_string,
        initWithBytes: value.as_ptr() as *const c_void
        length: value.len()
        encoding: UTF8_ENCODING
    ]
}

/// NSString ポインタを Rust の `String` に変換して返す。`value` が null の場合は `None`。
///
/// # Safety
/// `value` は有効な NSString インスタンスか null ポインタであること。
pub unsafe fn nsstring_to_string(value: *mut AnyObject) -> Option<String> {
    if value.is_null() {
        return None;
    }

    let utf8_ptr: *const c_char = msg_send![value, UTF8String];
    if utf8_ptr.is_null() {
        return Some(String::new());
    }

    Some(CStr::from_ptr(utf8_ptr).to_string_lossy().into_owned())
}

/// PNG のバイト列から正方形のテンプレート画像を作る。読み込みに失敗したら null を返す。
///
/// `setTemplate: true` にすると描画色は無視され、アルファだけがマスクとして使われる。
/// macOS が背景の濃淡や選択状態に応じて白黒を決めるので、ライト/ダークに自動追従する。
///
/// # Safety
/// - AppKit のメインスレッドから呼ぶこと。
/// - 返されたポインタは `alloc/init` で手動管理されるため、
///   呼び出し側が `msg_send![ptr, release]` で解放する責任を持つ。
pub unsafe fn template_image_from_png(png: &[u8], size: f64) -> *mut AnyObject {
    // *const u8 のままだと ObjC 側で char* と解釈され、`const void *` を期待する
    // dataWithBytes:length: と型が食い違う
    let bytes = png.as_ptr() as *const c_void;
    let data: *mut AnyObject = msg_send![
        class!(NSData),
        dataWithBytes: bytes
        length: png.len()
    ];
    let image: *mut AnyObject = msg_send![class!(NSImage), alloc];
    let image: *mut AnyObject = msg_send![image, initWithData: data];
    if image.is_null() {
        return ptr::null_mut();
    }

    // 素材は実寸より大きい。表示サイズを指定すると Retina でも元の解像度から縮小される。
    let () = msg_send![
        image,
        setSize: NSSize {
            width: size,
            height: size,
        }
    ];
    let () = msg_send![image, setTemplate: true];
    image
}
