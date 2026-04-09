use crate::error::AppError;

use super::types::{AppConfigFile, ConfigKey, HudBackgroundColor, HudPosition};
use super::{
    MAX_HUD_DURATION_SECS, MAX_HUD_FADE_DURATION_SECS, MAX_HUD_SCALE, MAX_POLL_INTERVAL_SECS,
    MAX_TRUNCATE_MAX_LINES, MAX_TRUNCATE_MAX_WIDTH, MIN_HUD_DURATION_SECS,
    MIN_HUD_FADE_DURATION_SECS, MIN_HUD_SCALE, MIN_POLL_INTERVAL_SECS, MIN_TRUNCATE_MAX_LINES,
    MIN_TRUNCATE_MAX_WIDTH,
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

pub fn parse_hud_emoji(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
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

pub fn parse_config_key(raw: &str) -> Option<ConfigKey> {
    match raw {
        "poll_interval_secs" | "poll-interval-secs" => Some(ConfigKey::PollIntervalSecs),
        "hud_duration_secs" | "hud-duration-secs" => Some(ConfigKey::HudDurationSecs),
        "hud_fade_duration_secs" | "hud-fade-duration-secs" => Some(ConfigKey::HudFadeDurationSecs),
        "max_chars_per_line" | "max-chars-per-line" => Some(ConfigKey::MaxCharsPerLine),
        "max_lines" | "max-lines" => Some(ConfigKey::MaxLines),
        "hud_position" | "hud-position" => Some(ConfigKey::HudPosition),
        "hud_scale" | "hud-scale" => Some(ConfigKey::HudScale),
        "hud_background_color" | "hud-background-color" => Some(ConfigKey::HudBackgroundColor),
        "hud_emoji" | "hud-emoji" => Some(ConfigKey::HudEmoji),
        _ => None,
    }
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
        ConfigKey::HudEmoji => {
            let raw = value.trim();
            if raw.is_empty() {
                return Err(AppError::InvalidValue {
                    key: "hud_emoji",
                    message: "must not be empty".to_string(),
                });
            }
            config.display.hud_emoji = Some(raw.to_string());
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
        HUD_DURATION_SECS, MAX_HUD_DURATION_SECS, MAX_TRUNCATE_MAX_WIDTH, MIN_HUD_SCALE,
        MIN_POLL_INTERVAL_SECS, POLL_INTERVAL_SECS,
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
    fn parse_config_key_accepts_aliases() {
        assert_eq!(
            parse_config_key("poll_interval_secs"),
            Some(ConfigKey::PollIntervalSecs)
        );
        assert_eq!(
            parse_config_key("poll-interval-secs"),
            Some(ConfigKey::PollIntervalSecs)
        );
        assert_eq!(
            parse_config_key("hud_position"),
            Some(ConfigKey::HudPosition)
        );
        assert_eq!(parse_config_key("hud-scale"), Some(ConfigKey::HudScale));
        assert_eq!(parse_config_key("hud_emoji"), Some(ConfigKey::HudEmoji));
        assert_eq!(parse_config_key("hud-emoji"), Some(ConfigKey::HudEmoji));
        assert_eq!(parse_config_key("hub_background_color"), None);
        assert_eq!(parse_config_key("hub-background-color"), None);
        assert_eq!(parse_config_key("unknown"), None);
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

        let err = set_config_value(&mut config, ConfigKey::HudEmoji, "  ")
            .expect_err("reject empty emoji");
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
    fn parse_hud_emoji_trims_and_rejects_empty() {
        assert!(parse_hud_emoji("🥜").is_some());
        assert_eq!(parse_hud_emoji("  🎯  ").unwrap(), "🎯");
        assert!(parse_hud_emoji("").is_none());
        assert!(parse_hud_emoji("   ").is_none());
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
