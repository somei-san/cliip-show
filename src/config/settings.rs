use super::io::{config_file_path, load_config_file};
use super::parse::{
    parse_f64_config_value, parse_f64_setting, parse_hud_background_color_setting, parse_hud_emoji,
    parse_hud_position_setting, parse_usize_config_value, parse_usize_setting,
};
use super::types::{AppConfigFile, DisplayConfigFile, DisplaySettings, HudPosition};
use super::{
    DEFAULT_HUD_FADE_DURATION_SECS, DEFAULT_HUD_SCALE, DEFAULT_TRUNCATE_MAX_LINES,
    DEFAULT_TRUNCATE_MAX_WIDTH, HUD_DURATION_SECS, MAX_HUD_DURATION_SECS,
    MAX_HUD_FADE_DURATION_SECS, MAX_HUD_SCALE, MAX_POLL_INTERVAL_SECS, MAX_TRUNCATE_MAX_LINES,
    MAX_TRUNCATE_MAX_WIDTH, MIN_HUD_DURATION_SECS, MIN_HUD_FADE_DURATION_SECS, MIN_HUD_SCALE,
    MIN_POLL_INTERVAL_SECS, MIN_TRUNCATE_MAX_LINES, MIN_TRUNCATE_MAX_WIDTH, POLL_INTERVAL_SECS,
};
use super::types::HudBackgroundColor;

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
        hud_emoji: "📋".to_string(),
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
            hud_emoji: Some(settings.hud_emoji.clone()),
        },
    }
}

fn read_env_option(name: &str) -> Option<String> {
    let Ok(raw) = std::env::var(name) else {
        return None;
    };
    Some(raw.trim().to_string())
}
