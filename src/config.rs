use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::AppError;

pub const POLL_INTERVAL_SECS: f64 = 0.3;
pub const HUD_DURATION_SECS: f64 = 1.0;
pub const DEFAULT_TRUNCATE_MAX_WIDTH: usize = 100;
pub const DEFAULT_TRUNCATE_MAX_LINES: usize = 5;
pub const DEFAULT_HUD_SCALE: f64 = 1.1;

pub const MIN_POLL_INTERVAL_SECS: f64 = 0.05;
pub const MAX_POLL_INTERVAL_SECS: f64 = 5.0;
pub const MIN_HUD_DURATION_SECS: f64 = 0.1;
pub const MAX_HUD_DURATION_SECS: f64 = 10.0;
pub const MIN_HUD_SCALE: f64 = 0.5;
pub const MAX_HUD_SCALE: f64 = 2.0;
pub const DEFAULT_HUD_FADE_DURATION_SECS: f64 = 0.3;
pub const MIN_HUD_FADE_DURATION_SECS: f64 = 0.0;
pub const MAX_HUD_FADE_DURATION_SECS: f64 = 2.0;
pub const MIN_TRUNCATE_MAX_WIDTH: usize = 1;
pub const MAX_TRUNCATE_MAX_WIDTH: usize = 500;
pub const MIN_TRUNCATE_MAX_LINES: usize = 1;
pub const MAX_TRUNCATE_MAX_LINES: usize = 20;

const DEFAULT_CONFIG_RELATIVE_PATH: &str = "Library/Application Support/cliip-show/config.toml";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum HudPosition {
    #[default]
    Top,
    Center,
    Bottom,
}

impl HudPosition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Center => "center",
            Self::Bottom => "bottom",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum HudBackgroundColor {
    #[default]
    Default,
    Yellow,
    Blue,
    Green,
    Red,
    Purple,
}

impl HudBackgroundColor {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Yellow => "yellow",
            Self::Blue => "blue",
            Self::Green => "green",
            Self::Red => "red",
            Self::Purple => "purple",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DisplaySettings {
    pub poll_interval_secs: f64,
    pub hud_duration_secs: f64,
    pub hud_fade_duration_secs: f64,
    pub truncate_max_width: usize,
    pub truncate_max_lines: usize,
    pub hud_position: HudPosition,
    pub hud_scale: f64,
    pub hud_background_color: HudBackgroundColor,
    pub hud_emoji: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfigFile {
    #[serde(default)]
    pub display: DisplayConfigFile,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DisplayConfigFile {
    pub poll_interval_secs: Option<f64>,
    pub hud_duration_secs: Option<f64>,
    pub hud_fade_duration_secs: Option<f64>,
    pub max_chars_per_line: Option<usize>,
    pub max_lines: Option<usize>,
    pub hud_position: Option<HudPosition>,
    pub hud_scale: Option<f64>,
    pub hud_background_color: Option<HudBackgroundColor>,
    pub hud_emoji: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigKey {
    PollIntervalSecs,
    HudDurationSecs,
    HudFadeDurationSecs,
    MaxCharsPerLine,
    MaxLines,
    HudPosition,
    HudScale,
    HudBackgroundColor,
    HudEmoji,
}

pub fn default_display_settings() -> DisplaySettings {
    DisplaySettings {
        poll_interval_secs: POLL_INTERVAL_SECS,
        hud_duration_secs: HUD_DURATION_SECS,
        hud_fade_duration_secs: DEFAULT_HUD_FADE_DURATION_SECS,
        truncate_max_width: DEFAULT_TRUNCATE_MAX_WIDTH,
        truncate_max_lines: DEFAULT_TRUNCATE_MAX_LINES,
        hud_position: HudPosition::Top,
        hud_scale: DEFAULT_HUD_SCALE,
        hud_background_color: HudBackgroundColor::default(),
        hud_emoji: "🥜",
    }
}

pub fn display_settings() -> DisplaySettings {
    let mut settings = default_display_settings();
    match config_file_path() {
        Ok(config_path) => match load_config_file(&config_path) {
            Ok((config, _)) => {
                settings = apply_config_file(settings, &config);
            }
            Err(error) => {
                eprintln!("warning: {error}");
            }
        },
        Err(error) => {
            eprintln!("warning: {error}");
        }
    }
    apply_env_overrides(settings)
}

pub fn apply_config_file(base: DisplaySettings, config: &AppConfigFile) -> DisplaySettings {
    let mut settings = base;
    if let Some(value) = config.display.poll_interval_secs {
        settings.poll_interval_secs = parse_f64_config_value(
            value,
            settings.poll_interval_secs,
            MIN_POLL_INTERVAL_SECS,
            MAX_POLL_INTERVAL_SECS,
            "poll_interval_secs",
        );
    }
    if let Some(value) = config.display.hud_duration_secs {
        settings.hud_duration_secs = parse_f64_config_value(
            value,
            settings.hud_duration_secs,
            MIN_HUD_DURATION_SECS,
            MAX_HUD_DURATION_SECS,
            "hud_duration_secs",
        );
    }
    if let Some(value) = config.display.hud_fade_duration_secs {
        settings.hud_fade_duration_secs = parse_f64_config_value(
            value,
            settings.hud_fade_duration_secs,
            MIN_HUD_FADE_DURATION_SECS,
            MAX_HUD_FADE_DURATION_SECS,
            "hud_fade_duration_secs",
        );
    }
    if let Some(value) = config.display.max_chars_per_line {
        settings.truncate_max_width = parse_usize_config_value(
            value,
            MIN_TRUNCATE_MAX_WIDTH,
            MAX_TRUNCATE_MAX_WIDTH,
            "max_chars_per_line",
        );
    }
    if let Some(value) = config.display.max_lines {
        settings.truncate_max_lines = parse_usize_config_value(
            value,
            MIN_TRUNCATE_MAX_LINES,
            MAX_TRUNCATE_MAX_LINES,
            "max_lines",
        );
    }
    if let Some(value) = config.display.hud_position {
        settings.hud_position = value;
    }
    if let Some(value) = config.display.hud_scale {
        settings.hud_scale = parse_f64_config_value(
            value,
            settings.hud_scale,
            MIN_HUD_SCALE,
            MAX_HUD_SCALE,
            "hud_scale",
        );
    }
    if let Some(value) = config.display.hud_background_color {
        settings.hud_background_color = value;
    }
    if let Some(value) = &config.display.hud_emoji {
        settings.hud_emoji = parse_hud_emoji(value).unwrap_or(settings.hud_emoji);
    }
    settings
}

pub fn apply_env_overrides(base: DisplaySettings) -> DisplaySettings {
    let mut settings = base;
    if let Some(value) = read_env_option("CLIIP_SHOW_POLL_INTERVAL_SECS") {
        settings.poll_interval_secs = parse_f64_setting(
            &value,
            settings.poll_interval_secs,
            MIN_POLL_INTERVAL_SECS,
            MAX_POLL_INTERVAL_SECS,
        );
    }
    if let Some(value) = read_env_option("CLIIP_SHOW_HUD_DURATION_SECS") {
        settings.hud_duration_secs = parse_f64_setting(
            &value,
            settings.hud_duration_secs,
            MIN_HUD_DURATION_SECS,
            MAX_HUD_DURATION_SECS,
        );
    }
    if let Some(value) = read_env_option("CLIIP_SHOW_HUD_FADE_DURATION_SECS") {
        settings.hud_fade_duration_secs = parse_f64_setting(
            &value,
            settings.hud_fade_duration_secs,
            MIN_HUD_FADE_DURATION_SECS,
            MAX_HUD_FADE_DURATION_SECS,
        );
    }
    if let Some(value) = read_env_option("CLIIP_SHOW_MAX_CHARS_PER_LINE") {
        settings.truncate_max_width = parse_usize_setting(
            &value,
            settings.truncate_max_width,
            MIN_TRUNCATE_MAX_WIDTH,
            MAX_TRUNCATE_MAX_WIDTH,
        );
    }
    if let Some(value) = read_env_option("CLIIP_SHOW_MAX_LINES") {
        settings.truncate_max_lines = parse_usize_setting(
            &value,
            settings.truncate_max_lines,
            MIN_TRUNCATE_MAX_LINES,
            MAX_TRUNCATE_MAX_LINES,
        );
    }
    if let Some(value) = read_env_option("CLIIP_SHOW_HUD_POSITION") {
        settings.hud_position = parse_hud_position_setting(&value, settings.hud_position);
    }
    if let Some(value) = read_env_option("CLIIP_SHOW_HUD_SCALE") {
        settings.hud_scale =
            parse_f64_setting(&value, settings.hud_scale, MIN_HUD_SCALE, MAX_HUD_SCALE);
    }
    if let Some(value) = read_env_option("CLIIP_SHOW_HUD_BACKGROUND_COLOR") {
        settings.hud_background_color =
            parse_hud_background_color_setting(&value, settings.hud_background_color);
    }
    if let Some(value) = read_env_option("CLIIP_SHOW_HUD_EMOJI") {
        settings.hud_emoji = parse_hud_emoji(&value).unwrap_or(settings.hud_emoji);
    }
    settings
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

fn parse_hud_position_setting(raw: &str, default: HudPosition) -> HudPosition {
    parse_hud_position(raw).unwrap_or(default)
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

fn parse_hud_background_color_setting(
    raw: &str,
    default: HudBackgroundColor,
) -> HudBackgroundColor {
    parse_hud_background_color(raw).unwrap_or(default)
}

pub fn parse_hud_emoji(raw: &str) -> Option<&'static str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(Box::leak(trimmed.to_string().into_boxed_str()))
}

fn read_env_option(name: &str) -> Option<String> {
    let Ok(raw) = std::env::var(name) else {
        return None;
    };
    Some(raw.trim().to_string())
}

pub fn parse_f64_value(value: f64, default: f64, min: f64, max: f64) -> f64 {
    if !value.is_finite() {
        return default;
    }
    value.clamp(min, max)
}

pub fn parse_usize_value(value: usize, min: usize, max: usize) -> usize {
    value.clamp(min, max)
}

fn parse_f64_config_value(value: f64, default: f64, min: f64, max: f64, key: &str) -> f64 {
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

fn parse_usize_config_value(value: usize, min: usize, max: usize, key: &str) -> usize {
    let clamped = value.clamp(min, max);
    if clamped != value {
        eprintln!(
            "warning: {key} was clamped from {value} to {clamped} (allowed range: {min}..={max})"
        );
    }
    clamped
}

pub fn config_file_path() -> Result<PathBuf, AppError> {
    if let Ok(path) = std::env::var("CLIIP_SHOW_CONFIG_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    let home = std::env::var("HOME").map_err(|_| {
        AppError::ConfigResolve("failed to resolve HOME for config path".to_string())
    })?;
    let trimmed = home.trim();
    if trimmed.is_empty() {
        return Err(AppError::ConfigResolve(
            "failed to resolve HOME for config path".to_string(),
        ));
    }
    Ok(PathBuf::from(trimmed).join(DEFAULT_CONFIG_RELATIVE_PATH))
}

pub fn load_config_file(path: &Path) -> Result<(AppConfigFile, bool), AppError> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok((AppConfigFile::default(), false));
        }
        Err(err) => {
            return Err(AppError::ConfigRead {
                path: path.display().to_string(),
                source: err,
            });
        }
    };
    toml::from_str::<AppConfigFile>(&content)
        .map(|config| (config, true))
        .map_err(|err| AppError::ConfigParse {
            path: path.display().to_string(),
            message: err.to_string(),
        })
}

pub fn save_config_file(path: &Path, config: &AppConfigFile) -> Result<(), AppError> {
    let parent = path.parent().ok_or_else(|| {
        AppError::ConfigResolve(format!(
            "failed to determine parent directory for config file {}",
            path.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(|err| AppError::ConfigWrite {
        path: parent.display().to_string(),
        source: err,
    })?;

    let content =
        toml::to_string_pretty(config).map_err(|err| AppError::ConfigEncode(err.to_string()))?;
    fs::write(path, content).map_err(|err| AppError::ConfigWrite {
        path: path.display().to_string(),
        source: err,
    })?;
    Ok(())
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

pub fn set_config_value(
    config: &mut AppConfigFile,
    key: ConfigKey,
    value: &str,
) -> Result<Option<String>, AppError> {
    match key {
        ConfigKey::PollIntervalSecs => {
            let raw = value.trim();
            let parsed = raw.parse::<f64>().map_err(|_| AppError::InvalidValue {
                key: "poll_interval_secs",
                message: format!("invalid f64: {raw}"),
            })?;
            if !parsed.is_finite() {
                return Err(AppError::InvalidValue {
                    key: "poll_interval_secs",
                    message: format!("value must be finite, got: {raw}"),
                });
            }
            let clamped = parsed.clamp(MIN_POLL_INTERVAL_SECS, MAX_POLL_INTERVAL_SECS);
            config.display.poll_interval_secs = Some(clamped);
            if parsed < MIN_POLL_INTERVAL_SECS || parsed > MAX_POLL_INTERVAL_SECS {
                return Ok(Some(format!(
                    "poll_interval_secs was clamped from {parsed} to {clamped} (allowed range: {MIN_POLL_INTERVAL_SECS}..={MAX_POLL_INTERVAL_SECS})"
                )));
            }
        }
        ConfigKey::HudDurationSecs => {
            let raw = value.trim();
            let parsed = raw.parse::<f64>().map_err(|_| AppError::InvalidValue {
                key: "hud_duration_secs",
                message: format!("invalid f64: {raw}"),
            })?;
            if !parsed.is_finite() {
                return Err(AppError::InvalidValue {
                    key: "hud_duration_secs",
                    message: format!("value must be finite, got: {raw}"),
                });
            }
            let clamped = parsed.clamp(MIN_HUD_DURATION_SECS, MAX_HUD_DURATION_SECS);
            config.display.hud_duration_secs = Some(clamped);
            if parsed < MIN_HUD_DURATION_SECS || parsed > MAX_HUD_DURATION_SECS {
                return Ok(Some(format!(
                    "hud_duration_secs was clamped from {parsed} to {clamped} (allowed range: {MIN_HUD_DURATION_SECS}..={MAX_HUD_DURATION_SECS})"
                )));
            }
        }
        ConfigKey::HudFadeDurationSecs => {
            let raw = value.trim();
            let parsed = raw.parse::<f64>().map_err(|_| AppError::InvalidValue {
                key: "hud_fade_duration_secs",
                message: format!("invalid f64: {raw}"),
            })?;
            if !parsed.is_finite() {
                return Err(AppError::InvalidValue {
                    key: "hud_fade_duration_secs",
                    message: format!("value must be finite, got: {raw}"),
                });
            }
            let clamped = parsed.clamp(MIN_HUD_FADE_DURATION_SECS, MAX_HUD_FADE_DURATION_SECS);
            config.display.hud_fade_duration_secs = Some(clamped);
            if parsed < MIN_HUD_FADE_DURATION_SECS || parsed > MAX_HUD_FADE_DURATION_SECS {
                return Ok(Some(format!(
                    "hud_fade_duration_secs was clamped from {parsed} to {clamped} (allowed range: {MIN_HUD_FADE_DURATION_SECS}..={MAX_HUD_FADE_DURATION_SECS})"
                )));
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
            if parsed < MIN_TRUNCATE_MAX_WIDTH || parsed > MAX_TRUNCATE_MAX_WIDTH {
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
            if parsed < MIN_TRUNCATE_MAX_LINES || parsed > MAX_TRUNCATE_MAX_LINES {
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
            let raw = value.trim();
            let parsed = raw.parse::<f64>().map_err(|_| AppError::InvalidValue {
                key: "hud_scale",
                message: format!("invalid f64: {raw}"),
            })?;
            if !parsed.is_finite() {
                return Err(AppError::InvalidValue {
                    key: "hud_scale",
                    message: format!("value must be finite, got: {raw}"),
                });
            }
            let clamped = parsed.clamp(MIN_HUD_SCALE, MAX_HUD_SCALE);
            config.display.hud_scale = Some(clamped);
            if parsed < MIN_HUD_SCALE || parsed > MAX_HUD_SCALE {
                return Ok(Some(format!(
                    "hud_scale was clamped from {parsed} to {clamped} (allowed range: {MIN_HUD_SCALE}..={MAX_HUD_SCALE})"
                )));
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

pub fn print_effective_settings(settings: DisplaySettings) {
    println!("poll_interval_secs = {}", settings.poll_interval_secs);
    println!("hud_duration_secs = {}", settings.hud_duration_secs);
    println!(
        "hud_fade_duration_secs = {}",
        settings.hud_fade_duration_secs
    );
    println!("max_chars_per_line = {}", settings.truncate_max_width);
    println!("max_lines = {}", settings.truncate_max_lines);
    println!("hud_position = {}", settings.hud_position.as_str());
    println!("hud_scale = {}", settings.hud_scale);
    println!(
        "hud_background_color = {}",
        settings.hud_background_color.as_str()
    );
    println!("hud_emoji = {}", settings.hud_emoji);
}

pub fn settings_to_config_file(settings: DisplaySettings) -> AppConfigFile {
    AppConfigFile {
        display: DisplayConfigFile {
            poll_interval_secs: Some(settings.poll_interval_secs),
            hud_duration_secs: Some(settings.hud_duration_secs),
            hud_fade_duration_secs: Some(settings.hud_fade_duration_secs),
            max_chars_per_line: Some(settings.truncate_max_width),
            max_lines: Some(settings.truncate_max_lines),
            hud_position: Some(settings.hud_position),
            hud_scale: Some(settings.hud_scale),
            hud_background_color: Some(settings.hud_background_color),
            hud_emoji: Some(settings.hud_emoji.to_string()),
        },
    }
}

pub fn handle_config_command<I: Iterator<Item = String>>(args: &mut I) -> bool {
    let path = match config_file_path() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let Some(cmd) = args.next() else {
        eprintln!("Usage: cliip-show --config <path|show|init|set>");
        std::process::exit(2);
    };

    match cmd.as_str() {
        "path" => {
            if args.next().is_some() {
                eprintln!("Usage: cliip-show --config path");
                std::process::exit(2);
            }
            println!("{}", path.display());
            true
        }
        "show" => {
            if args.next().is_some() {
                eprintln!("Usage: cliip-show --config show");
                std::process::exit(2);
            }
            println!("config_path = {}", path.display());
            let (config, loaded_from_file) = match load_config_file(&path) {
                Ok(result) => result,
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            };
            if loaded_from_file {
                println!("config_file = exists");
                println!("[saved]");
                if let Some(value) = config.display.poll_interval_secs {
                    println!("poll_interval_secs = {}", value);
                }
                if let Some(value) = config.display.hud_duration_secs {
                    println!("hud_duration_secs = {}", value);
                }
                if let Some(value) = config.display.hud_fade_duration_secs {
                    println!("hud_fade_duration_secs = {}", value);
                }
                if let Some(value) = config.display.max_chars_per_line {
                    println!("max_chars_per_line = {}", value);
                }
                if let Some(value) = config.display.max_lines {
                    println!("max_lines = {}", value);
                }
                if let Some(value) = config.display.hud_position {
                    println!("hud_position = {}", value.as_str());
                }
                if let Some(value) = config.display.hud_scale {
                    println!("hud_scale = {}", value);
                }
                if let Some(value) = config.display.hud_background_color {
                    println!("hud_background_color = {}", value.as_str());
                }
                if let Some(value) = &config.display.hud_emoji {
                    println!("hud_emoji = {}", value);
                }
            } else {
                println!("config_file = not_found");
            }
            println!("[effective]");
            let effective =
                apply_env_overrides(apply_config_file(default_display_settings(), &config));
            print_effective_settings(effective);
            true
        }
        "init" => {
            let mut force = false;
            if let Some(arg) = args.next() {
                if arg == "--force" {
                    force = true;
                    if args.next().is_some() {
                        eprintln!("Usage: cliip-show --config init [--force]");
                        std::process::exit(2);
                    }
                } else {
                    eprintln!("Usage: cliip-show --config init [--force]");
                    std::process::exit(2);
                }
            }

            if !force && path.exists() {
                eprintln!(
                    "config file already exists: {} (use --force to overwrite)",
                    path.display()
                );
                std::process::exit(2);
            }

            let config = settings_to_config_file(default_display_settings());
            if let Err(error) = save_config_file(&path, &config) {
                eprintln!("{error}");
                std::process::exit(1);
            }
            println!("initialized config: {}", path.display());
            true
        }
        "set" => {
            let Some(key_raw) = args.next() else {
                eprintln!("Usage: cliip-show --config set <key> <value>");
                eprintln!(
                    "Available keys: poll_interval_secs, hud_duration_secs, hud_fade_duration_secs, max_chars_per_line, max_lines, hud_position, hud_scale, hud_background_color, hud_emoji"
                );
                std::process::exit(2);
            };
            let Some(value_raw) = args.next() else {
                eprintln!("Usage: cliip-show --config set <key> <value>");
                std::process::exit(2);
            };
            if args.next().is_some() {
                eprintln!("Usage: cliip-show --config set <key> <value>");
                std::process::exit(2);
            }
            let Some(key) = parse_config_key(key_raw.trim()) else {
                eprintln!(
                    "Unknown key: {key_raw}. Available keys: poll_interval_secs, hud_duration_secs, hud_fade_duration_secs, max_chars_per_line, max_lines, hud_position, hud_scale, hud_background_color, hud_emoji"
                );
                std::process::exit(2);
            };

            let mut config = match load_config_file(&path) {
                Ok((config, _)) => config,
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            };

            let warning = match set_config_value(&mut config, key, value_raw.trim()) {
                Ok(warning) => warning,
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(2);
                }
            };
            if let Err(error) = save_config_file(&path, &config) {
                eprintln!("{error}");
                std::process::exit(1);
            }
            if let Some(warning) = warning {
                eprintln!("warning: {warning}");
            }
            println!("updated config: {}", path.display());
            println!(
                "hint: restart the service to apply changes: brew services restart cliip-show"
            );
            println!("[effective]");
            let effective =
                apply_env_overrides(apply_config_file(default_display_settings(), &config));
            print_effective_settings(effective);
            true
        }
        unknown => {
            eprintln!("Unknown --config command: {unknown}");
            eprintln!("Usage: cliip-show --config <path|show|init|set>");
            std::process::exit(2);
        }
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

#[cfg(test)]
mod tests {
    use super::{
        apply_config_file, default_display_settings, parse_config_key, parse_f64_setting,
        parse_f64_value, parse_hud_background_color, parse_hud_emoji, parse_hud_position,
        parse_usize_setting, set_config_value, AppConfigFile, ConfigKey, DisplayConfigFile,
        HudBackgroundColor, HudPosition, HUD_DURATION_SECS, MAX_HUD_DURATION_SECS,
        MAX_TRUNCATE_MAX_WIDTH, MIN_HUD_SCALE, MIN_POLL_INTERVAL_SECS, POLL_INTERVAL_SECS,
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
