use super::io::{config_file_path, load_config_file};
use super::parse::{
    parse_f64_config_value, parse_f64_setting, parse_hud_background_color_setting, parse_hud_emoji,
    parse_hud_position_setting, parse_language_setting, parse_usize_config_value,
    parse_usize_setting,
};
use super::types::HudBackgroundColor;
use super::types::{
    AppConfigFile, ConfigKey, DisplayConfigFile, DisplaySettings, HudPosition, LanguageSetting,
};
use super::{
    DEFAULT_HUD_FADE_DURATION_SECS, DEFAULT_HUD_IMAGE_MAX_HEIGHT, DEFAULT_HUD_SCALE,
    DEFAULT_TRUNCATE_MAX_LINES, DEFAULT_TRUNCATE_MAX_WIDTH, HUD_DURATION_SECS,
    MAX_HUD_DURATION_SECS, MAX_HUD_FADE_DURATION_SECS, MAX_HUD_IMAGE_MAX_HEIGHT, MAX_HUD_SCALE,
    MAX_POLL_INTERVAL_SECS, MAX_TRUNCATE_MAX_LINES, MAX_TRUNCATE_MAX_WIDTH, MIN_HUD_DURATION_SECS,
    MIN_HUD_FADE_DURATION_SECS, MIN_HUD_IMAGE_MAX_HEIGHT, MIN_HUD_SCALE, MIN_POLL_INTERVAL_SECS,
    MIN_TRUNCATE_MAX_LINES, MIN_TRUNCATE_MAX_WIDTH, POLL_INTERVAL_SECS,
};

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
        hud_image_max_height: DEFAULT_HUD_IMAGE_MAX_HEIGHT,
        language: LanguageSetting::default(),
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
    if let Some(value) = config.display.hud_image_max_height {
        settings.hud_image_max_height = parse_usize_config_value(
            value,
            MIN_HUD_IMAGE_MAX_HEIGHT,
            MAX_HUD_IMAGE_MAX_HEIGHT,
            "hud_image_max_height",
        );
    }
    if let Some(value) = config.display.language {
        settings.language = value;
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
    if let Some(value) = read_env_option("CLIIP_SHOW_HUD_IMAGE_MAX_HEIGHT") {
        settings.hud_image_max_height = parse_usize_setting(
            &value,
            settings.hud_image_max_height,
            MIN_HUD_IMAGE_MAX_HEIGHT,
            MAX_HUD_IMAGE_MAX_HEIGHT,
        );
    }
    if let Some(value) = read_env_option("CLIIP_SHOW_LANGUAGE") {
        settings.language = parse_language_setting(&value, settings.language);
    }
    settings
}

/// 設定ファイルに保存済みの値だけを出力する。キーごとの分岐を `ConfigKey` の網羅 match に
/// 寄せているので、キーを足したときの追随漏れはコンパイルエラーになる。
pub fn print_saved_settings(config: &AppConfigFile) {
    for key in ConfigKey::ALL {
        if let Some(value) = saved_value(config, key) {
            println!("{} = {}", key.as_str(), value);
        }
    }
}

fn saved_value(config: &AppConfigFile, key: ConfigKey) -> Option<String> {
    let display = &config.display;
    match key {
        ConfigKey::PollIntervalSecs => display.poll_interval_secs.map(|v| v.to_string()),
        ConfigKey::HudDurationSecs => display.hud_duration_secs.map(|v| v.to_string()),
        ConfigKey::HudFadeDurationSecs => display.hud_fade_duration_secs.map(|v| v.to_string()),
        ConfigKey::MaxCharsPerLine => display.max_chars_per_line.map(|v| v.to_string()),
        ConfigKey::MaxLines => display.max_lines.map(|v| v.to_string()),
        ConfigKey::HudPosition => display.hud_position.map(|v| v.as_str().to_string()),
        ConfigKey::HudScale => display.hud_scale.map(|v| v.to_string()),
        ConfigKey::HudBackgroundColor => {
            display.hud_background_color.map(|v| v.as_str().to_string())
        }
        ConfigKey::HudEmoji => display.hud_emoji.clone(),
        ConfigKey::HudImageMaxHeight => display.hud_image_max_height.map(|v| v.to_string()),
        ConfigKey::Language => display.language.map(|v| v.as_str().to_string()),
    }
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
    println!("hud_image_max_height = {}", settings.hud_image_max_height);
    println!("language = {}", settings.language.as_str());
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
            hud_image_max_height: Some(settings.hud_image_max_height),
            language: Some(settings.language),
        },
    }
}

fn read_env_option(name: &str) -> Option<String> {
    let Ok(raw) = std::env::var(name) else {
        return None;
    };
    Some(raw.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::{default_display_settings, saved_value, settings_to_config_file};
    use crate::config::ConfigKey;

    /// キーを足したときに `[saved]` の出力から漏れないことを守る。
    #[test]
    fn saved_value_covers_every_config_key() {
        let config = settings_to_config_file(default_display_settings());
        for key in ConfigKey::ALL {
            assert!(
                saved_value(&config, key).is_some(),
                "{} が [saved] に出ない",
                key.as_str()
            );
        }
    }

    /// `as_str` が設定ファイルのキー名からずれると、`[saved]` が設定ファイルに無い名前を出す。
    #[test]
    fn config_key_names_match_the_config_file() {
        let config = settings_to_config_file(default_display_settings());
        let serialized = toml::to_string_pretty(&config).expect("serialize config");
        for key in ConfigKey::ALL {
            assert!(
                serialized.contains(&format!("{} = ", key.as_str())),
                "{} が設定ファイルのキー名と一致しない",
                key.as_str()
            );
        }
    }
}
