//! 設定ウィンドウを組み立て、ビューツリーを直接検査する。見るのはコントロールの存在と
//! tag・action・target・配置で、見た目は対象外。
//!
//! `NSWindow` はメインスレッド以外で生成すると AppKit が例外を投げるため、libtest の
//! ワーカースレッドでは動かせない。`harness = false`（`Cargo.toml`）で自前の `main` から
//! メインスレッド上で実行する。そのため `cargo test <フィルタ>` のフィルタは効かず、
//! フィルタに一致しない実行でもこのバイナリは毎回ウィンドウを組み立てる。

use std::collections::HashMap;

use objc2::runtime::{AnyClass, AnyObject, Sel};
use objc2::{class, msg_send, sel};
use objc2_foundation::NSRect;

use cliip_show::app::get_delegate_class;
use cliip_show::config::ConfigKey;
use cliip_show::i18n::Lang;
use cliip_show::settings_window::{
    build_settings_window, config_key_to_tag, tag_to_config_key, InitialToggles, SettingsControls,
};

/// `ConfigKey` ごとに、値の変更を受け付けるコントロールを `SettingsControls` から引く。
/// ステッパー行はフィールドとステッパーの両方が同じ tag で `settingChanged:` を送るため
/// 2 つ返す。`ConfigKey` を増やしたときは、ここの match が網羅性で落ちるので対応する
/// コントロールを足すこと。
fn controls_for(controls: &SettingsControls, key: ConfigKey) -> Vec<*mut AnyObject> {
    match key {
        ConfigKey::PollIntervalSecs => vec![controls.poll_interval_slider],
        ConfigKey::HudDurationSecs => vec![controls.hud_duration_slider],
        ConfigKey::HudFadeDurationSecs => vec![controls.hud_fade_duration_slider],
        ConfigKey::MaxCharsPerLine => vec![
            controls.max_chars_per_line_field,
            controls.max_chars_per_line_stepper,
        ],
        ConfigKey::MaxLines => vec![controls.max_lines_field, controls.max_lines_stepper],
        ConfigKey::HudPosition => vec![controls.hud_position_popup],
        ConfigKey::HudScale => vec![controls.hud_scale_slider],
        ConfigKey::HudBackgroundColor => vec![controls.hud_background_color_popup],
        ConfigKey::HudBackgroundOpacity => vec![controls.hud_background_opacity_slider],
        ConfigKey::HudEmoji => vec![controls.hud_emoji_field],
        ConfigKey::HudImageMaxHeight => vec![
            controls.hud_image_max_height_field,
            controls.hud_image_max_height_stepper,
        ],
        ConfigKey::Language => vec![controls.language_popup],
        ConfigKey::StartAtLogin => vec![controls.login_item_toggle],
        ConfigKey::ShowMenuBarIcon => vec![controls.show_menu_bar_icon_toggle],
    }
}

/// `ConfigKey` ごとに、コントロールが送るべき action。下書き（`SettingsControls::draft`）に
/// 乗る項目と言語は `settingChanged:` が tag で振り分ける。自動起動とメニューバーアイコン
/// 表示は専用の action を持ち、取り違えると別の設定を書き換えるためここで固定する。
fn expected_action(key: ConfigKey) -> Sel {
    match key {
        ConfigKey::StartAtLogin => sel!(toggleLoginItem:),
        ConfigKey::ShowMenuBarIcon => sel!(toggleMenuBarIcon:),
        ConfigKey::PollIntervalSecs
        | ConfigKey::HudDurationSecs
        | ConfigKey::HudFadeDurationSecs
        | ConfigKey::MaxCharsPerLine
        | ConfigKey::MaxLines
        | ConfigKey::HudPosition
        | ConfigKey::HudScale
        | ConfigKey::HudBackgroundColor
        | ConfigKey::HudBackgroundOpacity
        | ConfigKey::HudEmoji
        | ConfigKey::HudImageMaxHeight
        | ConfigKey::Language => sel!(settingChanged:),
    }
}

unsafe fn subviews(view: *mut AnyObject) -> Vec<*mut AnyObject> {
    let array: *mut AnyObject = msg_send![view, subviews];
    let count: usize = msg_send![array, count];
    (0..count)
        .map(|i| msg_send![array, objectAtIndex: i])
        .collect()
}

unsafe fn is_kind_of(object: *mut AnyObject, class: &AnyClass) -> bool {
    msg_send![object, isKindOfClass: class]
}

unsafe fn action_of(control: *mut AnyObject) -> Option<Sel> {
    msg_send![control, action]
}

unsafe fn frame_of(view: *mut AnyObject) -> NSRect {
    msg_send![view, frame]
}

unsafe fn string_value(control: *mut AnyObject) -> String {
    let ns: *mut AnyObject = msg_send![control, stringValue];
    let utf8: *const std::os::raw::c_char = msg_send![ns, UTF8String];
    std::ffi::CStr::from_ptr(utf8)
        .to_string_lossy()
        .into_owned()
}

unsafe fn static_text_fields(document_view: *mut AnyObject) -> Vec<*mut AnyObject> {
    subviews(document_view)
        .into_iter()
        .filter(|view| {
            if !is_kind_of(*view, class!(NSTextField)) {
                return false;
            }
            let editable: bool = msg_send![*view, isEditable];
            !editable
        })
        .collect()
}

fn rect_contains(outer: NSRect, inner: NSRect) -> bool {
    inner.origin.x >= outer.origin.x
        && inner.origin.y >= outer.origin.y
        && inner.origin.x + inner.size.width <= outer.origin.x + outer.size.width
        && inner.origin.y + inner.size.height <= outer.origin.y + outer.size.height
}

fn overlaps_vertically(a: NSRect, b: NSRect) -> bool {
    a.origin.y < b.origin.y + b.size.height && b.origin.y < a.origin.y + a.size.height
}

unsafe fn check_every_config_key_is_wired(
    lang: Lang,
    controls: &SettingsControls,
    delegate: *mut AnyObject,
) {
    let delegate_class = get_delegate_class();
    for key in ConfigKey::ALL {
        let expected_tag = config_key_to_tag(key);
        let expected = expected_action(key);
        assert!(
            delegate_class.responds_to(expected),
            "{lang:?} {key:?}: デリゲートが action {expected} を実装していない"
        );
        for control in controls_for(controls, key) {
            assert!(!control.is_null(), "{lang:?} {key:?}: コントロールが null");

            let superview: *mut AnyObject = msg_send![control, superview];
            assert_eq!(
                superview, controls.document_view,
                "{lang:?} {key:?}: コントロールが documentView の子ではない"
            );

            let tag: isize = msg_send![control, tag];
            assert_eq!(
                tag, expected_tag,
                "{lang:?} {key:?}: tag が config_key_to_tag と一致しない"
            );

            assert_eq!(
                action_of(control),
                Some(expected),
                "{lang:?} {key:?}: action が期待と違う"
            );

            let target: *mut AnyObject = msg_send![control, target];
            assert_eq!(
                target, delegate,
                "{lang:?} {key:?}: target がデリゲートではない"
            );
        }
    }
}

/// `settingChanged:` を送るコントロールを documentView から拾い、tag ごとの個数が
/// `controls_for` と一致することを見る。`SettingsControls` に載っていないコントロールや、
/// `setTag:` を忘れて既定の tag 0（`PollIntervalSecs` と同じ値）のまま残ったコントロールは、
/// ここで余剰として現れる。
unsafe fn check_setting_changed_senders_match_config_keys(lang: Lang, controls: &SettingsControls) {
    let mut count_by_tag: HashMap<isize, usize> = HashMap::new();
    for view in subviews(controls.document_view) {
        if !is_kind_of(view, class!(NSControl)) {
            continue;
        }
        if action_of(view) != Some(sel!(settingChanged:)) {
            continue;
        }
        let tag: isize = msg_send![view, tag];
        *count_by_tag.entry(tag).or_insert(0) += 1;
    }

    for tag in count_by_tag.keys() {
        assert!(
            tag_to_config_key(*tag).is_some(),
            "{lang:?}: settingChanged: を送るコントロールの tag {tag} が ConfigKey に解決できない"
        );
    }
    for key in ConfigKey::ALL {
        if expected_action(key) != sel!(settingChanged:) {
            continue;
        }
        let expected = controls_for(controls, key).len();
        let actual = count_by_tag
            .get(&config_key_to_tag(key))
            .copied()
            .unwrap_or(0);
        assert_eq!(
            actual, expected,
            "{lang:?} {key:?}: settingChanged: を送るコントロールの個数が期待と違う"
        );
    }
}

unsafe fn check_every_subview_fits_in_document_view(lang: Lang, document_view: *mut AnyObject) {
    let bounds: NSRect = msg_send![document_view, bounds];
    for (index, view) in subviews(document_view).into_iter().enumerate() {
        let frame = frame_of(view);
        assert!(
            rect_contains(bounds, frame),
            "{lang:?}: documentView の {index} 番目の子 {view:?} が bounds {bounds:?} の外にある: \
             {frame:?}"
        );
    }
}

unsafe fn check_every_config_key_has_a_label(lang: Lang, controls: &SettingsControls) {
    let labels = static_text_fields(controls.document_view);
    for key in ConfigKey::ALL {
        let control = controls_for(controls, key)[0];
        let control_frame = frame_of(control);
        let row_labels: Vec<String> = labels
            .iter()
            .filter(|label| {
                let frame = frame_of(**label);
                overlaps_vertically(frame, control_frame)
                    && frame.origin.x + frame.size.width <= control_frame.origin.x
            })
            .map(|label| string_value(*label))
            .collect();
        assert!(
            !row_labels.is_empty(),
            "{lang:?} {key:?}: コントロールの左にラベルが無い"
        );
        assert!(
            row_labels.iter().all(|text| !text.is_empty()),
            "{lang:?} {key:?}: ラベルが空文字"
        );
    }
}

unsafe fn check_language(lang: Lang) {
    // action は一切発火させないので、AppState 未初期化の素の delegate でよい
    let delegate: *mut AnyObject = msg_send![get_delegate_class(), new];
    let controls = build_settings_window(
        &*delegate,
        lang,
        InitialToggles {
            start_at_login: false,
            show_menu_bar_icon: true,
        },
    );
    assert!(
        !controls.window.is_null(),
        "{lang:?}: 設定ウィンドウが生成されていない"
    );
    assert!(
        !controls.document_view.is_null(),
        "{lang:?}: documentView が生成されていない"
    );

    check_every_config_key_is_wired(lang, &controls, delegate);
    check_setting_changed_senders_match_config_keys(lang, &controls);
    check_every_subview_fits_in_document_view(lang, controls.document_view);
    check_every_config_key_has_a_label(lang, &controls);

    let () = msg_send![controls.window, close];
    let () = msg_send![delegate, release];
}

fn main() {
    unsafe {
        let _app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        for lang in [Lang::Ja, Lang::En] {
            check_language(lang);
        }
    }

    println!("settings_window_view_tree: ok");
}
