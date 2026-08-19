mod cli;
mod io;
mod parse;
pub(crate) mod settings;
pub(crate) mod types;

// 定数
pub const POLL_INTERVAL_SECS: f64 = 0.3;
pub const HUD_DURATION_SECS: f64 = 1.0;
pub const DEFAULT_TRUNCATE_MAX_WIDTH: usize = 100;
pub const DEFAULT_TRUNCATE_MAX_LINES: usize = 5;
pub const DEFAULT_HUD_SCALE: f64 = 1.1;
pub const DEFAULT_HUD_IMAGE_MAX_HEIGHT: usize = 160;
pub const DEFAULT_HUD_BACKGROUND_OPACITY: f64 = 0.9;

pub const MIN_POLL_INTERVAL_SECS: f64 = 0.05;
pub const MAX_POLL_INTERVAL_SECS: f64 = 5.0;
pub const MIN_HUD_DURATION_SECS: f64 = 0.1;
pub const MAX_HUD_DURATION_SECS: f64 = 10.0;
pub const MIN_HUD_SCALE: f64 = 0.5;
pub const MAX_HUD_SCALE: f64 = 2.0;
pub const MIN_HUD_BACKGROUND_OPACITY: f64 = 0.2;
pub const MAX_HUD_BACKGROUND_OPACITY: f64 = 1.0;
pub const DEFAULT_HUD_FADE_DURATION_SECS: f64 = 0.3;
pub const MIN_HUD_FADE_DURATION_SECS: f64 = 0.0;
pub const MAX_HUD_FADE_DURATION_SECS: f64 = 2.0;
pub const MIN_TRUNCATE_MAX_WIDTH: usize = 1;
pub const MAX_TRUNCATE_MAX_WIDTH: usize = 500;
pub const MIN_TRUNCATE_MAX_LINES: usize = 1;
pub const MAX_TRUNCATE_MAX_LINES: usize = 20;
// 上限 240 は、`DEFAULT_HUD_SCALE` のときに実際に効く高さ上限（HUD_MAX_HEIGHT から縦パディングを引いた値）に合わせている。
// hud_scale を上げれば実際の上限も上がる。
pub const MIN_HUD_IMAGE_MAX_HEIGHT: usize = 40;
pub const MAX_HUD_IMAGE_MAX_HEIGHT: usize = 240;

// Re-exports: 外部モジュールのインポートを一切変更しないようにする
pub use cli::show_config;
pub use io::{config_file_path, load_config_file, save_config_file};
pub use parse::{
    hud_emoji_validation_error, parse_f64_setting, parse_f64_value, parse_hud_background_color,
    parse_hud_emoji, parse_hud_position, parse_language, parse_usize_setting, parse_usize_value,
    set_config_value,
};
pub use settings::{
    apply_config_file, apply_env_overrides, default_display_settings, display_settings,
    merge_display_settings, print_effective_settings, settings_to_config_file,
    start_at_login_from_config,
};
pub use types::{
    AppConfigFile, ConfigKey, DisplayConfigFile, DisplaySettings, HudBackgroundColor, HudPosition,
    LanguageSetting, StartupConfigFile,
};
