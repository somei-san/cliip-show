use super::io::{config_file_path, load_config_file};
use super::settings::{
    apply_config_file, apply_env_overrides, default_display_settings, print_effective_settings,
};

/// `--config-show` の処理。設定の変更手段は設定ウィンドウに一本化しているため、
/// CLI からは現在値の確認だけできる。不具合の切り分けに使う。
pub fn show_config<I: Iterator<Item = String>>(args: &mut I) -> bool {
    if args.next().is_some() {
        eprintln!("Usage: cliip-show --config-show");
        std::process::exit(2);
    }

    let path = match config_file_path() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

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
        if let Some(value) = config.display.language {
            println!("language = {}", value.as_str());
        }
    } else {
        println!("config_file = not_found");
    }
    println!("[effective]");
    let effective = apply_env_overrides(apply_config_file(default_display_settings(), &config));
    print_effective_settings(effective);
    true
}
