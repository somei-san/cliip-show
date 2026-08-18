use unicode_segmentation::UnicodeSegmentation;

use crate::error::AppError;
use crate::i18n::{self, Lang, Msg};

use super::types::{AppConfigFile, ConfigKey, HudBackgroundColor, HudPosition, LanguageSetting};
use super::{
    MAX_HUD_BACKGROUND_OPACITY, MAX_HUD_DURATION_SECS, MAX_HUD_FADE_DURATION_SECS,
    MAX_HUD_IMAGE_MAX_HEIGHT, MAX_HUD_SCALE, MAX_POLL_INTERVAL_SECS, MAX_TRUNCATE_MAX_LINES,
    MAX_TRUNCATE_MAX_WIDTH, MIN_HUD_BACKGROUND_OPACITY, MIN_HUD_DURATION_SECS,
    MIN_HUD_FADE_DURATION_SECS, MIN_HUD_IMAGE_MAX_HEIGHT, MIN_HUD_SCALE, MIN_POLL_INTERVAL_SECS,
    MIN_TRUNCATE_MAX_LINES, MIN_TRUNCATE_MAX_WIDTH,
};

pub fn parse_f64_value(value: f64, default: f64, min: f64, max: f64) -> f64 {
    if !value.is_finite() {
        return default;
    }
    value.clamp(min, max)
}

/// `usize` は NaN/Infinity を持たないため `default` パラメータは不要。
/// 範囲外の値は `min`/`max` にクランプして返す。
pub fn parse_usize_value(value: usize, min: usize, max: usize) -> usize {
    value.clamp(min, max)
}

pub fn parse_hud_position(raw: &str) -> Option<HudPosition> {
    let normalized = raw.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "top" => Some(HudPosition::Top),
        "center" => Some(HudPosition::Center),
        "bottom" => Some(HudPosition::Bottom),
        _ => None,
    }
}

pub fn parse_hud_background_color(raw: &str) -> Option<HudBackgroundColor> {
    let normalized = raw.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "default" => Some(HudBackgroundColor::Default),
        "yellow" => Some(HudBackgroundColor::Yellow),
        "blue" => Some(HudBackgroundColor::Blue),
        "green" => Some(HudBackgroundColor::Green),
        "red" => Some(HudBackgroundColor::Red),
        "purple" => Some(HudBackgroundColor::Purple),
        _ => None,
    }
}

pub fn parse_language(raw: &str) -> Option<LanguageSetting> {
    let normalized = raw.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "auto" => Some(LanguageSetting::Auto),
        "ja" => Some(LanguageSetting::Ja),
        "en" => Some(LanguageSetting::En),
        _ => None,
    }
}

/// 絵文字を含むコードポイント範囲（Unicode Emoji Data の主要ブロック + 異体字セレクタ・ZWJ・
/// キーキャップ）。絵文字クラスタの判定にのみ使う。
/// 国旗の地域指示記号（U+1F1E6..=U+1F1FF）は U+1F000..=U+1FAFF に包含されるため別枠で持たない。
fn is_emoji_codepoint(c: char) -> bool {
    matches!(c as u32,
        0x00A9 | 0x00AE | 0x203C | 0x2049 | 0x2122 | 0x2139
        | 0x2194..=0x21AA | 0x231A..=0x231B | 0x2328 | 0x23CF | 0x23E9..=0x23FA
        | 0x24C2 | 0x25AA..=0x25FE | 0x2600..=0x27BF | 0x2934..=0x2935
        | 0x2B00..=0x2BFF | 0x3030 | 0x303D | 0x3297 | 0x3299
        | 0x1F000..=0x1FAFF
        | 0xFE0F | 0x20E3 | 0x200D
    )
}

/// `hud_emoji` の入力値を判定し、妥当なら `None`、不正なら理由の `Msg` を返す。
/// 空文字（trim 後）は「アイコンなし」として妥当扱いする。
pub fn hud_emoji_validation_error(raw: &str) -> Option<Msg> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut graphemes = trimmed.graphemes(true);
    let first = graphemes.next().expect("trimmed is non-empty");
    if graphemes.next().is_some() {
        return Some(Msg::EmojiTooLong);
    }
    if first.chars().any(is_emoji_codepoint) {
        None
    } else {
        Some(Msg::EmojiNotEmoji)
    }
}

/// trim 後が空、または単一の絵文字書記素クラスタのときだけ `Some` を返す。
/// 空文字は「アイコンなし」を表す `Some(String::new())`。複数文字・非絵文字は `None`。
pub fn parse_hud_emoji(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    match hud_emoji_validation_error(raw) {
        None => Some(trimmed.to_string()),
        Some(_) => None,
    }
}

pub fn parse_f64_setting(raw: &str, default: f64, min: f64, max: f64) -> f64 {
    let Ok(value) = raw.parse::<f64>() else {
        return default;
    };
    if !value.is_finite() {
        return default;
    }
    value.clamp(min, max)
}

pub fn parse_usize_setting(raw: &str, default: usize, min: usize, max: usize) -> usize {
    let Ok(value) = raw.parse::<usize>() else {
        return default;
    };
    value.clamp(min, max)
}

/// `set_config_value` 内で f64 フィールドをパース・バリデーション・クランプする共通処理。
/// 成功時は `(clamped_value, clamp_warning_message)` を返す。
fn parse_and_clamp_f64(
    raw: &str,
    key: &'static str,
    min: f64,
    max: f64,
) -> Result<(f64, Option<String>), AppError> {
    let trimmed = raw.trim();
    let parsed = trimmed.parse::<f64>().map_err(|_| AppError::InvalidValue {
        key,
        message: format!("invalid f64: {trimmed}"),
    })?;
    if !parsed.is_finite() {
        return Err(AppError::InvalidValue {
            key,
            message: format!("value must be finite, got: {trimmed}"),
        });
    }
    let clamped = parsed.clamp(min, max);
    let warn = if parsed < min || parsed > max {
        Some(format!(
            "{key} was clamped from {parsed} to {clamped} (allowed range: {min}..={max})"
        ))
    } else {
        None
    };
    Ok((clamped, warn))
}

pub(crate) fn parse_f64_config_value(
    value: f64,
    default: f64,
    min: f64,
    max: f64,
    key: &str,
) -> f64 {
    if !value.is_finite() {
        eprintln!("warning: {key}: value must be finite, got {value}; using default {default}");
        return default;
    }
    let clamped = value.clamp(min, max);
    if clamped != value {
        eprintln!(
            "warning: {key} was clamped from {value} to {clamped} (allowed range: {min}..={max})"
        );
    }
    clamped
}

pub(crate) fn parse_usize_config_value(value: usize, min: usize, max: usize, key: &str) -> usize {
    let clamped = value.clamp(min, max);
    if clamped != value {
        eprintln!(
            "warning: {key} was clamped from {value} to {clamped} (allowed range: {min}..={max})"
        );
    }
    clamped
}

pub(crate) fn parse_hud_position_setting(raw: &str, default: HudPosition) -> HudPosition {
    parse_hud_position(raw).unwrap_or(default)
}

pub(crate) fn parse_hud_background_color_setting(
    raw: &str,
    default: HudBackgroundColor,
) -> HudBackgroundColor {
    parse_hud_background_color(raw).unwrap_or(default)
}

pub(crate) fn parse_language_setting(raw: &str, default: LanguageSetting) -> LanguageSetting {
    parse_language(raw).unwrap_or(default)
}

pub fn set_config_value(
    config: &mut AppConfigFile,
    key: ConfigKey,
    value: &str,
) -> Result<Option<String>, AppError> {
    match key {
        ConfigKey::PollIntervalSecs => {
            let (clamped, warn) = parse_and_clamp_f64(
                value,
                "poll_interval_secs",
                MIN_POLL_INTERVAL_SECS,
                MAX_POLL_INTERVAL_SECS,
            )?;
            config.display.poll_interval_secs = Some(clamped);
            if let Some(msg) = warn {
                return Ok(Some(msg));
            }
        }
        ConfigKey::HudDurationSecs => {
            let (clamped, warn) = parse_and_clamp_f64(
                value,
                "hud_duration_secs",
                MIN_HUD_DURATION_SECS,
                MAX_HUD_DURATION_SECS,
            )?;
            config.display.hud_duration_secs = Some(clamped);
            if let Some(msg) = warn {
                return Ok(Some(msg));
            }
        }
        ConfigKey::HudFadeDurationSecs => {
            let (clamped, warn) = parse_and_clamp_f64(
                value,
                "hud_fade_duration_secs",
                MIN_HUD_FADE_DURATION_SECS,
                MAX_HUD_FADE_DURATION_SECS,
            )?;
            config.display.hud_fade_duration_secs = Some(clamped);
            if let Some(msg) = warn {
                return Ok(Some(msg));
            }
        }
        ConfigKey::MaxCharsPerLine => {
            let raw = value.trim();
            let parsed = raw.parse::<usize>().map_err(|_| AppError::InvalidValue {
                key: "max_chars_per_line",
                message: format!("invalid integer: {raw}"),
            })?;
            let clamped = parse_usize_value(parsed, MIN_TRUNCATE_MAX_WIDTH, MAX_TRUNCATE_MAX_WIDTH);
            config.display.max_chars_per_line = Some(clamped);
            if !(MIN_TRUNCATE_MAX_WIDTH..=MAX_TRUNCATE_MAX_WIDTH).contains(&parsed) {
                return Ok(Some(format!(
                    "max_chars_per_line was clamped from {parsed} to {clamped} (allowed range: {MIN_TRUNCATE_MAX_WIDTH}..={MAX_TRUNCATE_MAX_WIDTH})"
                )));
            }
        }
        ConfigKey::MaxLines => {
            let raw = value.trim();
            let parsed = raw.parse::<usize>().map_err(|_| AppError::InvalidValue {
                key: "max_lines",
                message: format!("invalid integer: {raw}"),
            })?;
            let clamped = parse_usize_value(parsed, MIN_TRUNCATE_MAX_LINES, MAX_TRUNCATE_MAX_LINES);
            config.display.max_lines = Some(clamped);
            if !(MIN_TRUNCATE_MAX_LINES..=MAX_TRUNCATE_MAX_LINES).contains(&parsed) {
                return Ok(Some(format!(
                    "max_lines was clamped from {parsed} to {clamped} (allowed range: {MIN_TRUNCATE_MAX_LINES}..={MAX_TRUNCATE_MAX_LINES})"
                )));
            }
        }
        ConfigKey::HudPosition => {
            let raw = value.trim();
            let parsed = parse_hud_position(raw).ok_or_else(|| AppError::InvalidValue {
                key: "hud_position",
                message: format!("{raw} (allowed: top, center, bottom)"),
            })?;
            config.display.hud_position = Some(parsed);
        }
        ConfigKey::HudScale => {
            let (clamped, warn) =
                parse_and_clamp_f64(value, "hud_scale", MIN_HUD_SCALE, MAX_HUD_SCALE)?;
            config.display.hud_scale = Some(clamped);
            if let Some(msg) = warn {
                return Ok(Some(msg));
            }
        }
        ConfigKey::HudBackgroundColor => {
            let raw = value.trim();
            let parsed = parse_hud_background_color(raw).ok_or_else(|| AppError::InvalidValue {
                key: "hud_background_color",
                message: format!("{raw} (allowed: default, yellow, blue, green, red, purple)"),
            })?;
            config.display.hud_background_color = Some(parsed);
        }
        ConfigKey::HudBackgroundOpacity => {
            let (clamped, warn) = parse_and_clamp_f64(
                value,
                "hud_background_opacity",
                MIN_HUD_BACKGROUND_OPACITY,
                MAX_HUD_BACKGROUND_OPACITY,
            )?;
            config.display.hud_background_opacity = Some(clamped);
            if let Some(msg) = warn {
                return Ok(Some(msg));
            }
        }
        ConfigKey::HudEmoji => {
            let Some(emoji) = parse_hud_emoji(value) else {
                // CLI 経路は言語設定を持たないため英語固定で組み立てる
                let reason = hud_emoji_validation_error(value)
                    .map(|msg| i18n::text(Lang::En, msg))
                    .unwrap_or("must be a single emoji");
                return Err(AppError::InvalidValue {
                    key: "hud_emoji",
                    message: format!("{reason}, got: {}", value.trim()),
                });
            };
            config.display.hud_emoji = Some(emoji);
        }
        ConfigKey::Language => {
            let raw = value.trim();
            let parsed = parse_language(raw).ok_or_else(|| AppError::InvalidValue {
                key: "language",
                message: format!("{raw} (allowed: auto, ja, en)"),
            })?;
            config.display.language = Some(parsed);
        }
        ConfigKey::HudImageMaxHeight => {
            let raw = value.trim();
            let parsed = raw.parse::<usize>().map_err(|_| AppError::InvalidValue {
                key: "hud_image_max_height",
                message: format!("invalid integer: {raw}"),
            })?;
            let clamped =
                parse_usize_value(parsed, MIN_HUD_IMAGE_MAX_HEIGHT, MAX_HUD_IMAGE_MAX_HEIGHT);
            config.display.hud_image_max_height = Some(clamped);
            if !(MIN_HUD_IMAGE_MAX_HEIGHT..=MAX_HUD_IMAGE_MAX_HEIGHT).contains(&parsed) {
                return Ok(Some(format!(
                    "hud_image_max_height was clamped from {parsed} to {clamped} (allowed range: {MIN_HUD_IMAGE_MAX_HEIGHT}..={MAX_HUD_IMAGE_MAX_HEIGHT})"
                )));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::super::settings::apply_config_file;
    use super::super::settings::default_display_settings;
    use super::super::types::DisplayConfigFile;
    use super::*;
    use crate::config::{
        HUD_DURATION_SECS, MAX_HUD_DURATION_SECS, MAX_HUD_IMAGE_MAX_HEIGHT, MAX_TRUNCATE_MAX_WIDTH,
        MIN_HUD_SCALE, MIN_POLL_INTERVAL_SECS, POLL_INTERVAL_SECS,
    };

    #[test]
    fn parse_f64_setting_clamps_and_fallbacks() {
        assert_eq!(parse_f64_setting("0.01", 1.0, 0.1, 5.0), 0.1);
        assert_eq!(parse_f64_setting("8.0", 1.0, 0.1, 5.0), 5.0);
        assert_eq!(parse_f64_setting("1.5", 1.0, 0.1, 5.0), 1.5);
        assert_eq!(parse_f64_setting("abc", 1.0, 0.1, 5.0), 1.0);
    }

    #[test]
    fn parse_usize_setting_clamps_and_fallbacks() {
        assert_eq!(parse_usize_setting("0", 10, 1, 20), 1);
        assert_eq!(parse_usize_setting("100", 10, 1, 20), 20);
        assert_eq!(parse_usize_setting("5", 10, 1, 20), 5);
        assert_eq!(parse_usize_setting("abc", 10, 1, 20), 10);
    }

    #[test]
    fn set_config_value_clamps_values() {
        let mut config = AppConfigFile::default();
        let poll_warning = set_config_value(&mut config, ConfigKey::PollIntervalSecs, "0.01")
            .expect("set poll interval");
        let lines_warning =
            set_config_value(&mut config, ConfigKey::MaxLines, "999").expect("set max lines");

        assert_eq!(config.display.poll_interval_secs, Some(0.05));
        assert_eq!(config.display.max_lines, Some(20));
        assert!(poll_warning.is_some());
        assert!(lines_warning.is_some());
    }

    #[test]
    fn set_config_value_accepts_new_display_options() {
        let mut config = AppConfigFile::default();
        let position_warning =
            set_config_value(&mut config, ConfigKey::HudPosition, "bottom").expect("set position");
        let scale_warning =
            set_config_value(&mut config, ConfigKey::HudScale, "9.9").expect("set scale");
        let color_warning = set_config_value(&mut config, ConfigKey::HudBackgroundColor, "blue")
            .expect("set background color");

        assert_eq!(config.display.hud_position, Some(HudPosition::Bottom));
        assert_eq!(config.display.hud_scale, Some(2.0));
        assert_eq!(
            config.display.hud_background_color,
            Some(HudBackgroundColor::Blue)
        );
        assert!(position_warning.is_none());
        assert!(scale_warning.is_some());
        assert!(color_warning.is_none());
    }

    #[test]
    fn set_config_value_clamps_hud_image_max_height() {
        let mut config = AppConfigFile::default();
        let warning = set_config_value(&mut config, ConfigKey::HudImageMaxHeight, "9999")
            .expect("set hud image max height");
        assert_eq!(
            config.display.hud_image_max_height,
            Some(MAX_HUD_IMAGE_MAX_HEIGHT)
        );
        assert!(warning.is_some());

        let warning = set_config_value(&mut config, ConfigKey::HudImageMaxHeight, "120")
            .expect("set hud image max height");
        assert_eq!(config.display.hud_image_max_height, Some(120));
        assert!(warning.is_none());

        let err = set_config_value(&mut config, ConfigKey::HudImageMaxHeight, "tall")
            .expect_err("reject non-integer");
        assert!(err.to_string().contains("hud_image_max_height"));
    }

    #[test]
    fn set_config_value_rejects_non_finite_f64_values() {
        let mut config = AppConfigFile::default();
        let poll_err = set_config_value(&mut config, ConfigKey::PollIntervalSecs, "NaN")
            .expect_err("reject NaN");
        let duration_err = set_config_value(&mut config, ConfigKey::HudDurationSecs, "inf")
            .expect_err("reject inf");

        assert!(poll_err.to_string().contains("poll_interval_secs"));
        assert!(duration_err.to_string().contains("hud_duration_secs"));
        assert_eq!(config.display.poll_interval_secs, None);
        assert_eq!(config.display.hud_duration_secs, None);
    }

    #[test]
    fn set_config_value_accepts_hud_emoji() {
        let mut config = AppConfigFile::default();
        set_config_value(&mut config, ConfigKey::HudEmoji, "🍺").expect("set hud emoji");
        assert_eq!(config.display.hud_emoji, Some("🍺".to_string()));

        // 空欄は「アイコンなし」として妥当
        set_config_value(&mut config, ConfigKey::HudEmoji, "  ").expect("set empty hud emoji");
        assert_eq!(config.display.hud_emoji, Some(String::new()));

        let err = set_config_value(&mut config, ConfigKey::HudEmoji, "📋🍣")
            .expect_err("reject multiple graphemes");
        assert!(err.to_string().contains("hud_emoji"));
    }

    #[test]
    fn set_config_value_rejects_invalid_enum_values() {
        let mut config = AppConfigFile::default();
        let position_err = set_config_value(&mut config, ConfigKey::HudPosition, "middle")
            .expect_err("reject invalid position");
        let color_err = set_config_value(&mut config, ConfigKey::HudBackgroundColor, "orange")
            .expect_err("reject invalid color");

        assert!(position_err.to_string().contains("hud_position"));
        assert!(color_err.to_string().contains("hud_background_color"));
        assert_eq!(config.display.hud_position, None);
        assert_eq!(config.display.hud_background_color, None);
    }

    #[test]
    fn parse_f64_value_clamps_and_fallbacks_for_non_finite() {
        assert_eq!(parse_f64_value(0.01, 1.0, 0.1, 5.0), 0.1);
        assert_eq!(parse_f64_value(8.0, 1.0, 0.1, 5.0), 5.0);
        assert_eq!(parse_f64_value(1.5, 1.0, 0.1, 5.0), 1.5);
        assert_eq!(parse_f64_value(f64::NAN, 1.0, 0.1, 5.0), 1.0);
        assert_eq!(parse_f64_value(f64::INFINITY, 1.0, 0.1, 5.0), 1.0);
    }

    #[test]
    fn parse_hud_position_accepts_valid_values() {
        assert_eq!(parse_hud_position("top"), Some(HudPosition::Top));
        assert_eq!(parse_hud_position("center"), Some(HudPosition::Center));
        assert_eq!(parse_hud_position("bottom"), Some(HudPosition::Bottom));
        assert_eq!(parse_hud_position("  Top  "), Some(HudPosition::Top));
        assert_eq!(parse_hud_position("CENTER"), Some(HudPosition::Center));
        assert_eq!(parse_hud_position("invalid"), None);
    }

    #[test]
    fn parse_hud_background_color_accepts_valid_values() {
        assert_eq!(
            parse_hud_background_color("default"),
            Some(HudBackgroundColor::Default)
        );
        assert_eq!(
            parse_hud_background_color("yellow"),
            Some(HudBackgroundColor::Yellow)
        );
        assert_eq!(
            parse_hud_background_color("  Green  "),
            Some(HudBackgroundColor::Green)
        );
        assert_eq!(parse_hud_background_color("invalid"), None);
    }

    #[test]
    fn parse_hud_emoji_trims_and_accepts_empty_as_no_icon() {
        assert_eq!(parse_hud_emoji("📋").unwrap(), "📋");
        assert_eq!(parse_hud_emoji("🍣").unwrap(), "🍣");
        assert_eq!(parse_hud_emoji("  🎯  ").unwrap(), "🎯");
        assert_eq!(parse_hud_emoji("").unwrap(), "");
        assert_eq!(parse_hud_emoji("   ").unwrap(), "");
        assert!(parse_hud_emoji("📋🍣").is_none());
        assert!(parse_hud_emoji("a").is_none());
        assert!(parse_hud_emoji("あ").is_none());
        assert!(parse_hud_emoji("1").is_none());
        assert_eq!(parse_hud_emoji("🇯🇵").unwrap(), "🇯🇵");
        assert_eq!(parse_hud_emoji("1️⃣").unwrap(), "1️⃣");
    }

    #[test]
    fn hud_emoji_validation_error_reports_reason() {
        assert_eq!(hud_emoji_validation_error(""), None);
        assert_eq!(hud_emoji_validation_error("   "), None);
        assert_eq!(hud_emoji_validation_error("📋"), None);
        assert_eq!(hud_emoji_validation_error("🇯🇵"), None);
        assert_eq!(hud_emoji_validation_error("1️⃣"), None);
        assert_eq!(hud_emoji_validation_error("📋🍣"), Some(Msg::EmojiTooLong));
        assert_eq!(hud_emoji_validation_error("a"), Some(Msg::EmojiNotEmoji));
        assert_eq!(hud_emoji_validation_error("あ"), Some(Msg::EmojiNotEmoji));
    }

    #[test]
    fn parse_language_accepts_valid_values() {
        assert_eq!(parse_language("auto"), Some(LanguageSetting::Auto));
        assert_eq!(parse_language("ja"), Some(LanguageSetting::Ja));
        assert_eq!(parse_language("en"), Some(LanguageSetting::En));
        assert_eq!(parse_language("  JA  "), Some(LanguageSetting::Ja));
        assert_eq!(parse_language("invalid"), None);
    }

    #[test]
    fn set_config_value_accepts_language() {
        let mut config = AppConfigFile::default();
        let warning =
            set_config_value(&mut config, ConfigKey::Language, "ja").expect("set language");
        assert_eq!(config.display.language, Some(LanguageSetting::Ja));
        assert!(warning.is_none());

        let err = set_config_value(&mut config, ConfigKey::Language, "fr")
            .expect_err("reject invalid language");
        assert!(err.to_string().contains("language"));
    }

    #[test]
    fn apply_config_file_treats_empty_hud_emoji_as_no_icon() {
        let base = default_display_settings();
        assert_ne!(base.hud_emoji, "");
        let config = AppConfigFile {
            display: DisplayConfigFile {
                hud_emoji: Some(String::new()),
                ..Default::default()
            },
        };
        let settings = apply_config_file(base, &config);
        assert_eq!(settings.hud_emoji, "");
    }

    #[test]
    fn apply_config_file_clamps_out_of_range_values() {
        let base = default_display_settings();
        let config = AppConfigFile {
            display: DisplayConfigFile {
                poll_interval_secs: Some(0.001), // below MIN_POLL_INTERVAL_SECS
                hud_duration_secs: Some(100.0),  // above MAX_HUD_DURATION_SECS
                hud_scale: Some(0.1),            // below MIN_HUD_SCALE
                max_chars_per_line: Some(1000),  // above MAX_TRUNCATE_MAX_WIDTH
                ..Default::default()
            },
        };
        let settings = apply_config_file(base, &config);
        assert_eq!(settings.poll_interval_secs, MIN_POLL_INTERVAL_SECS);
        assert_eq!(settings.hud_duration_secs, MAX_HUD_DURATION_SECS);
        assert_eq!(settings.hud_scale, MIN_HUD_SCALE);
        assert_eq!(settings.truncate_max_width, MAX_TRUNCATE_MAX_WIDTH);
    }

    #[test]
    fn apply_config_file_uses_default_for_non_finite() {
        let base = default_display_settings();
        let config = AppConfigFile {
            display: DisplayConfigFile {
                poll_interval_secs: Some(f64::NAN),
                hud_duration_secs: Some(f64::INFINITY),
                ..Default::default()
            },
        };
        let settings = apply_config_file(base, &config);
        assert_eq!(settings.poll_interval_secs, POLL_INTERVAL_SECS);
        assert_eq!(settings.hud_duration_secs, HUD_DURATION_SECS);
    }
}
