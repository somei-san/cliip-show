use serde::{Deserialize, Serialize};

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
pub enum LanguageSetting {
    #[default]
    Auto,
    Ja,
    En,
}

impl LanguageSetting {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Ja => "ja",
            Self::En => "en",
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

#[derive(Debug, Clone, PartialEq)]
pub struct DisplaySettings {
    pub poll_interval_secs: f64,
    pub hud_duration_secs: f64,
    pub hud_fade_duration_secs: f64,
    pub truncate_max_width: usize,
    pub truncate_max_lines: usize,
    pub hud_position: HudPosition,
    pub hud_scale: f64,
    pub hud_background_color: HudBackgroundColor,
    pub hud_background_opacity: f64,
    pub hud_emoji: String,
    pub hud_image_max_height: usize,
    pub language: LanguageSetting,
    pub show_menu_bar_icon: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfigFile {
    #[serde(default)]
    pub display: DisplayConfigFile,
    #[serde(default)]
    pub startup: StartupConfigFile,
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
    pub hud_background_opacity: Option<f64>,
    pub hud_emoji: Option<String>,
    pub hud_image_max_height: Option<usize>,
    pub language: Option<LanguageSetting>,
    pub show_menu_bar_icon: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StartupConfigFile {
    pub start_at_login: Option<bool>,
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
    HudBackgroundOpacity,
    HudEmoji,
    HudImageMaxHeight,
    Language,
    ShowMenuBarIcon,
    StartAtLogin,
}

impl ConfigKey {
    /// 設定ファイルに並ぶ順。キーを網羅して回りたい箇所はここを使う。
    pub const ALL: [ConfigKey; 14] = [
        ConfigKey::PollIntervalSecs,
        ConfigKey::HudDurationSecs,
        ConfigKey::HudFadeDurationSecs,
        ConfigKey::MaxCharsPerLine,
        ConfigKey::MaxLines,
        ConfigKey::HudPosition,
        ConfigKey::HudScale,
        ConfigKey::HudBackgroundColor,
        ConfigKey::HudBackgroundOpacity,
        ConfigKey::HudEmoji,
        ConfigKey::HudImageMaxHeight,
        ConfigKey::Language,
        ConfigKey::ShowMenuBarIcon,
        ConfigKey::StartAtLogin,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ConfigKey::PollIntervalSecs => "poll_interval_secs",
            ConfigKey::HudDurationSecs => "hud_duration_secs",
            ConfigKey::HudFadeDurationSecs => "hud_fade_duration_secs",
            ConfigKey::MaxCharsPerLine => "max_chars_per_line",
            ConfigKey::MaxLines => "max_lines",
            ConfigKey::HudPosition => "hud_position",
            ConfigKey::HudScale => "hud_scale",
            ConfigKey::HudBackgroundColor => "hud_background_color",
            ConfigKey::HudBackgroundOpacity => "hud_background_opacity",
            ConfigKey::HudEmoji => "hud_emoji",
            ConfigKey::HudImageMaxHeight => "hud_image_max_height",
            ConfigKey::Language => "language",
            ConfigKey::ShowMenuBarIcon => "show_menu_bar_icon",
            ConfigKey::StartAtLogin => "start_at_login",
        }
    }
}
