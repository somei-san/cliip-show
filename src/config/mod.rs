mod cli;
mod io;
mod parse;
pub(crate) mod settings;
mod types;

// 定数
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

// Re-exports: 外部モジュールのインポートを一切変更しないようにする
pub use cli::handle_config_command;
pub use io::{config_file_path, load_config_file, save_config_file};
pub use parse::{
    parse_config_key, parse_f64_setting, parse_f64_value, parse_hud_background_color,
    parse_hud_emoji, parse_hud_position, parse_usize_setting, parse_usize_value, set_config_value,
};
pub use settings::{
    apply_config_file, apply_env_overrides, default_display_settings, display_settings,
    print_effective_settings, settings_to_config_file,
};
pub use types::{
    AppConfigFile, ConfigKey, DisplayConfigFile, DisplaySettings, HudBackgroundColor, HudPosition,
};
