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

#[derive(Debug, Clone)]
pub struct DisplaySettings {
    pub poll_interval_secs: f64,
    pub hud_duration_secs: f64,
    pub hud_fade_duration_secs: f64,
    pub truncate_max_width: usize,
    pub truncate_max_lines: usize,
    pub hud_position: HudPosition,
    pub hud_scale: f64,
    pub hud_background_color: HudBackgroundColor,
    pub hud_emoji: String,
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
