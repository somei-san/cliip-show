use super::io::{config_file_path, load_config_file};
use super::parse::{
    parse_f64_config_value, parse_f64_setting, parse_hud_background_color_setting, parse_hud_emoji,
    parse_hud_position_setting, parse_language_setting, parse_usize_config_value,
    parse_usize_setting,
};
use super::types::HudBackgroundColor;
use super::types::{
    AppConfigFile, ConfigKey, DisplayConfigFile, DisplaySettings, HudPosition, LanguageSetting,
    StartupConfigFile,
};
use super::{
    DEFAULT_HUD_BACKGROUND_OPACITY, DEFAULT_HUD_FADE_DURATION_SECS, DEFAULT_HUD_IMAGE_MAX_HEIGHT,
    DEFAULT_HUD_SCALE, DEFAULT_TRUNCATE_MAX_LINES, DEFAULT_TRUNCATE_MAX_WIDTH, HUD_DURATION_SECS,
    MAX_HUD_BACKGROUND_OPACITY, MAX_HUD_DURATION_SECS, MAX_HUD_FADE_DURATION_SECS,
    MAX_HUD_IMAGE_MAX_HEIGHT, MAX_HUD_SCALE, MAX_POLL_INTERVAL_SECS, MAX_TRUNCATE_MAX_LINES,
    MAX_TRUNCATE_MAX_WIDTH, MIN_HUD_BACKGROUND_OPACITY, MIN_HUD_DURATION_SECS,
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
        hud_background_opacity: DEFAULT_HUD_BACKGROUND_OPACITY,
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
    if let Some(value) = config.display.hud_background_opacity {
        settings.hud_background_opacity = parse_f64_config_value(
            value,
            settings.hud_background_opacity,
            MIN_HUD_BACKGROUND_OPACITY,
            MAX_HUD_BACKGROUND_OPACITY,
            "hud_background_opacity",
        );
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
    apply_env_overrides_with(base, |name| std::env::var(name).ok())
}

/// 環境変数の読み取り元を差し替えられる本体。`std::env` はプロセスグローバルで
/// 並列実行のテストから安全に操作できないため、テストは lookup を注入して呼ぶ。
fn apply_env_overrides_with(
    base: DisplaySettings,
    lookup: impl Fn(&str) -> Option<String>,
) -> DisplaySettings {
    // trim はここで行う（lookup は生の値を返す契約）
    let read_env_option = |name: &str| lookup(name).map(|raw| raw.trim().to_string());
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
    if let Some(value) = read_env_option("CLIIP_SHOW_HUD_BACKGROUND_OPACITY") {
        settings.hud_background_opacity = parse_f64_setting(
            &value,
            settings.hud_background_opacity,
            MIN_HUD_BACKGROUND_OPACITY,
            MAX_HUD_BACKGROUND_OPACITY,
        );
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
        ConfigKey::HudBackgroundOpacity => display.hud_background_opacity.map(|v| v.to_string()),
        ConfigKey::HudEmoji => display.hud_emoji.clone(),
        ConfigKey::HudImageMaxHeight => display.hud_image_max_height.map(|v| v.to_string()),
        ConfigKey::Language => display.language.map(|v| v.as_str().to_string()),
        ConfigKey::StartAtLogin => config.startup.start_at_login.map(|v| v.to_string()),
    }
}

pub fn print_effective_settings(settings: DisplaySettings, start_at_login: bool) {
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
    println!(
        "hud_background_opacity = {}",
        settings.hud_background_opacity
    );
    println!("hud_emoji = {}", settings.hud_emoji);
    println!("hud_image_max_height = {}", settings.hud_image_max_height);
    println!("language = {}", settings.language.as_str());
    println!("start_at_login = {start_at_login}");
}

/// 設定ファイルに保存済みの自動起動の値。未設定（`[startup]` 節が無い、または
/// `start_at_login` キーが無い）と、明示的に `false` が保存されている状態を
/// 呼び出し側が区別できるよう `Option` のまま返す。
pub fn start_at_login_from_config(config: &AppConfigFile) -> Option<bool> {
    config.startup.start_at_login
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
            hud_background_opacity: Some(settings.hud_background_opacity),
            hud_emoji: Some(settings.hud_emoji.clone()),
            hud_image_max_height: Some(settings.hud_image_max_height),
            language: Some(settings.language),
        },
        // `DisplaySettings` は自動起動の値を持たないため、ここは常に未設定
        // （`StartupConfigFile::default()`）。`Some(false)` を返すと、この関数の
        // 戻り値がそのまま保存された場合に自動起動が黙って無効化されてしまう。
        // 未設定なら plist の有無から復元できるため被害が出ない。
        startup: StartupConfigFile::default(),
    }
}

/// 読み込み済みの設定ファイルへ `settings` の `display` セクションだけを差し替える
/// （load-merge-save の merge 部分）。`display` 以外のセクション（`startup` 等、下書きの
/// 対象外のキー）は `config` の値をそのまま残す。設定ウィンドウの「保存」ボタン
/// （`save_settings`）から使う。
pub fn merge_display_settings(
    mut config: AppConfigFile,
    settings: DisplaySettings,
) -> AppConfigFile {
    config.display = settings_to_config_file(settings).display;
    config
}

#[cfg(test)]
mod tests {
    use super::{
        apply_config_file, apply_env_overrides_with, default_display_settings, saved_value,
        settings_to_config_file,
    };
    use crate::config::types::{HudPosition, LanguageSetting};
    use crate::config::{
        AppConfigFile, ConfigKey, MAX_HUD_FADE_DURATION_SECS, MAX_HUD_IMAGE_MAX_HEIGHT,
        MAX_HUD_SCALE, MAX_TRUNCATE_MAX_LINES, MIN_HUD_SCALE, MIN_POLL_INTERVAL_SECS,
    };

    /// 固定の名前→値ペアを lookup として返す（`std::env` を触らないための注入）。
    fn env_from(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_string())
        }
    }

    /// キーを足したときに `[saved]` の出力から漏れないことを守る。`settings_to_config_file`
    /// は `start_at_login` を常に未設定で返すため、`ConfigKey::StartAtLogin` 分だけ
    /// 値を持たせて確認する。
    #[test]
    fn saved_value_covers_every_config_key() {
        let mut config = settings_to_config_file(default_display_settings());
        config.startup.start_at_login = Some(true);
        for key in ConfigKey::ALL {
            assert!(
                saved_value(&config, key).is_some(),
                "{} が [saved] に出ない",
                key.as_str()
            );
        }
    }

    /// 設定ファイルの範囲外の値は落とさずクランプして採用する（手編集した値で起動不能にしない）。
    /// 下限側は parse.rs の `apply_config_file_clamps_out_of_range_values` が張っているので、
    /// ここでは上限側と、そちらに無い fade / image_max_height を押さえる。
    #[test]
    fn apply_config_file_clamps_values_above_max() {
        let mut config = AppConfigFile::default();
        config.display.hud_scale = Some(99.0);
        config.display.max_lines = Some(999);
        config.display.hud_fade_duration_secs = Some(99.0);
        config.display.hud_image_max_height = Some(9999);

        let settings = apply_config_file(default_display_settings(), &config);
        assert_eq!(settings.hud_scale, MAX_HUD_SCALE);
        assert_eq!(settings.truncate_max_lines, MAX_TRUNCATE_MAX_LINES);
        assert_eq!(settings.hud_fade_duration_secs, MAX_HUD_FADE_DURATION_SECS);
        assert_eq!(settings.hud_image_max_height, MAX_HUD_IMAGE_MAX_HEIGHT);
    }

    /// 範囲の端の値はクランプに巻き込まれずそのまま通ること（境界の off-by-one 検出）。
    #[test]
    fn apply_config_file_keeps_boundary_values_unchanged() {
        let mut config = AppConfigFile::default();
        config.display.hud_scale = Some(MIN_HUD_SCALE);
        config.display.poll_interval_secs = Some(MIN_POLL_INTERVAL_SECS);
        config.display.max_lines = Some(MAX_TRUNCATE_MAX_LINES);

        let settings = apply_config_file(default_display_settings(), &config);
        assert_eq!(settings.hud_scale, MIN_HUD_SCALE);
        assert_eq!(settings.poll_interval_secs, MIN_POLL_INTERVAL_SECS);
        assert_eq!(settings.truncate_max_lines, MAX_TRUNCATE_MAX_LINES);
    }

    /// 設定ファイルに無いキーはデフォルト値のまま残ること。
    #[test]
    fn apply_config_file_keeps_defaults_for_absent_keys() {
        let mut config = AppConfigFile::default();
        config.display.hud_scale = Some(1.5);

        let defaults = default_display_settings();
        let settings = apply_config_file(defaults.clone(), &config);
        assert_eq!(settings.hud_scale, 1.5);
        assert_eq!(settings.poll_interval_secs, defaults.poll_interval_secs);
        assert_eq!(settings.hud_emoji, defaults.hud_emoji);
        assert_eq!(settings.language, defaults.language);
    }

    /// 環境変数は設定ファイルの値より優先される（docs/development.md に明記している外部仕様）。
    #[test]
    fn env_overrides_take_precedence_over_the_config_file() {
        let mut config = AppConfigFile::default();
        config.display.hud_scale = Some(1.5);
        config.display.hud_position = Some(HudPosition::Center);
        let from_file = apply_config_file(default_display_settings(), &config);

        let settings = apply_env_overrides_with(
            from_file,
            env_from(&[
                ("CLIIP_SHOW_HUD_SCALE", "0.8"),
                ("CLIIP_SHOW_HUD_POSITION", "bottom"),
                ("CLIIP_SHOW_LANGUAGE", "ja"),
            ]),
        );
        assert_eq!(settings.hud_scale, 0.8);
        assert_eq!(settings.hud_position, HudPosition::Bottom);
        assert_eq!(settings.language, LanguageSetting::Ja);
    }

    /// パースできない環境変数は無視して元の値を使う（変な値で起動不能にしない）。
    #[test]
    fn invalid_env_values_fall_back_to_the_base_value() {
        let base = default_display_settings();
        let settings = apply_env_overrides_with(
            base.clone(),
            env_from(&[
                ("CLIIP_SHOW_HUD_SCALE", "abc"),
                ("CLIIP_SHOW_HUD_POSITION", "middle"),
                ("CLIIP_SHOW_HUD_EMOJI", "📋🍣"),
                ("CLIIP_SHOW_MAX_LINES", "-1"),
            ]),
        );
        assert_eq!(settings, base);
    }

    /// 範囲外の環境変数は落とさずクランプして採用する。
    #[test]
    fn out_of_range_env_values_are_clamped() {
        let settings = apply_env_overrides_with(
            default_display_settings(),
            env_from(&[
                ("CLIIP_SHOW_HUD_SCALE", "99"),
                ("CLIIP_SHOW_POLL_INTERVAL_SECS", "0.0001"),
            ]),
        );
        assert_eq!(settings.hud_scale, MAX_HUD_SCALE);
        assert_eq!(settings.poll_interval_secs, MIN_POLL_INTERVAL_SECS);
    }

    /// シェルの引用符由来の前後空白は取り除いてから解釈する。
    #[test]
    fn env_values_are_trimmed_before_parsing() {
        let settings = apply_env_overrides_with(
            default_display_settings(),
            env_from(&[
                ("CLIIP_SHOW_HUD_SCALE", " 1.5 "),
                ("CLIIP_SHOW_HUD_POSITION", " bottom "),
            ]),
        );
        assert_eq!(settings.hud_scale, 1.5);
        assert_eq!(settings.hud_position, HudPosition::Bottom);
    }

    /// `DisplaySettings` に属する全キーが `CLIIP_SHOW_<キー名大文字>` の環境変数で
    /// 上書きできること。production 側の変数名リテラルの書き換え・追随漏れをここで落とす
    /// （命名規約は docs/development.md の環境変数の節）。
    /// `start_at_login` は `DisplaySettings` を持たず環境変数の上書き対象外のため、
    /// このテストでは飛ばす。
    #[test]
    fn every_config_key_is_reachable_via_env() {
        for key in ConfigKey::ALL {
            // デフォルトと異なる有効値。デフォルトと同じ値だと空振りでも通ってしまう
            let value = match key {
                ConfigKey::PollIntervalSecs => "0.5",
                ConfigKey::HudDurationSecs => "2.0",
                ConfigKey::HudFadeDurationSecs => "1.0",
                ConfigKey::MaxCharsPerLine => "50",
                ConfigKey::MaxLines => "3",
                ConfigKey::HudPosition => "bottom",
                ConfigKey::HudScale => "1.5",
                ConfigKey::HudBackgroundColor => "blue",
                ConfigKey::HudBackgroundOpacity => "0.5",
                ConfigKey::HudEmoji => "🍺",
                ConfigKey::HudImageMaxHeight => "80",
                ConfigKey::Language => "ja",
                ConfigKey::StartAtLogin => continue,
            };
            let env_name = format!("CLIIP_SHOW_{}", key.as_str().to_uppercase());
            let settings = apply_env_overrides_with(default_display_settings(), |name| {
                (name == env_name).then(|| value.to_string())
            });
            assert_ne!(
                settings,
                default_display_settings(),
                "{env_name} が設定に反映されない"
            );
        }
    }

    /// `CLIIP_SHOW_HUD_EMOJI=""`（アイコンなし）は「未設定」ではなく空指定として効くこと。
    /// VRT の no_emoji ケースはこの挙動に依存している。
    #[test]
    fn empty_env_emoji_disables_the_icon() {
        let settings = apply_env_overrides_with(
            default_display_settings(),
            env_from(&[("CLIIP_SHOW_HUD_EMOJI", "")]),
        );
        assert_eq!(settings.hud_emoji, "");
    }

    /// `as_str` が設定ファイルのキー名からずれると、`[saved]` が設定ファイルに無い名前を出す。
    /// `start_at_login` は未設定だと TOML に出力されないため、`Some(true)` を持たせて確認する。
    #[test]
    fn config_key_names_match_the_config_file() {
        let mut config = settings_to_config_file(default_display_settings());
        config.startup.start_at_login = Some(true);
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
