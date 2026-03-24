use super::io::{config_file_path, load_config_file, save_config_file};
use super::parse::{parse_config_key, set_config_value};
use super::settings::{
    apply_config_file, apply_env_overrides, default_display_settings, print_effective_settings,
    settings_to_config_file,
};
use super::types::ConfigKey;

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
            if key == ConfigKey::PollIntervalSecs {
                println!(
                    "hint: poll_interval_secs takes effect after restart: brew services restart cliip-show"
                );
            } else {
                println!("hint: change will be applied automatically (no restart needed)");
            }
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
