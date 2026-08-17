use std::fs;
use std::path::{Path, PathBuf};

use crate::error::AppError;

use super::types::AppConfigFile;

const DEFAULT_CONFIG_RELATIVE_PATH: &str = "Library/Application Support/cliip-show/config.toml";

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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{load_config_file, save_config_file};
    use crate::config::settings::{
        apply_config_file, default_display_settings, settings_to_config_file,
    };
    use crate::config::types::{HudBackgroundColor, HudPosition, LanguageSetting};
    use crate::error::AppError;

    /// テストごとに独立したディレクトリを使う（cargo test は並列実行のため共有すると干渉する）。
    /// Drop でディレクトリごと削除する。名前には連番も混ぜ、`test_name` をコピペで重複させても
    /// 並列テスト同士がディレクトリを共有しないようにする。
    struct TempConfigDir(PathBuf);

    impl TempConfigDir {
        fn new(test_name: &str) -> Self {
            static DIR_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let seq = DIR_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "cliip-show-io-test-{}-{seq}-{test_name}",
                std::process::id()
            ));
            fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }

        fn path(&self, file_name: &str) -> PathBuf {
            self.0.join(file_name)
        }
    }

    impl Drop for TempConfigDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// GUI 保存の経路そのもの: 設定 → TOML 保存 → 再読込 → 適用の一周で全キーの値が
    /// 変わらないこと。ここが崩れると「保存したのに次回起動で値が戻る」になる。
    #[test]
    fn saved_settings_survive_a_save_load_cycle() {
        let temp = TempConfigDir::new("roundtrip");
        // 全キーをデフォルト以外の値にする（デフォルトのままだと適用側の取りこぼしを検出できない）
        let mut settings = default_display_settings();
        settings.poll_interval_secs = 0.5;
        settings.hud_duration_secs = 2.5;
        settings.hud_fade_duration_secs = 1.0;
        settings.truncate_max_width = 42;
        settings.truncate_max_lines = 7;
        settings.hud_position = HudPosition::Bottom;
        settings.hud_scale = 1.5;
        settings.hud_background_color = HudBackgroundColor::Green;
        settings.hud_emoji = "🍺".to_string();
        settings.hud_image_max_height = 200;
        settings.language = LanguageSetting::Ja;

        let config_path = temp.path("config.toml");
        save_config_file(&config_path, &settings_to_config_file(settings.clone()))
            .expect("save config");
        let (loaded, existed) = load_config_file(&config_path).expect("load config");
        assert!(existed);

        let reloaded = apply_config_file(default_display_settings(), &loaded);
        assert_eq!(reloaded, settings);
    }

    /// hud_emoji = ""（アイコンなし）は正式な設定値。空文字が「未設定」に化けて
    /// デフォルトの 📋 に戻らないこと。
    #[test]
    fn empty_hud_emoji_survives_a_save_load_cycle() {
        let temp = TempConfigDir::new("empty-emoji");
        let mut settings = default_display_settings();
        settings.hud_emoji = String::new();

        let config_path = temp.path("config.toml");
        save_config_file(&config_path, &settings_to_config_file(settings)).expect("save config");
        let (loaded, _) = load_config_file(&config_path).expect("load config");

        let reloaded = apply_config_file(default_display_settings(), &loaded);
        assert_eq!(reloaded.hud_emoji, "");
    }

    /// 初回起動（設定ファイルなし）はエラーではなくデフォルト設定で立ち上がる。
    #[test]
    fn missing_file_yields_defaults_without_error() {
        let temp = TempConfigDir::new("missing");
        let (config, existed) = load_config_file(&temp.path("no-such-config.toml")).expect("load");
        assert!(!existed);
        assert!(config.display.poll_interval_secs.is_none());
        assert!(config.display.hud_emoji.is_none());
    }

    #[test]
    fn broken_toml_yields_a_parse_error() {
        let temp = TempConfigDir::new("broken");
        let config_path = temp.path("config.toml");
        fs::write(&config_path, "display = {").expect("write broken toml");

        let error = load_config_file(&config_path).expect_err("must fail");
        assert!(matches!(error, AppError::ConfigParse { .. }), "{error:?}");
    }

    /// 保存先の親ディレクトリが無くても作って書き込める（初回保存の経路）。
    #[test]
    fn save_creates_missing_parent_directories() {
        let temp = TempConfigDir::new("mkdir");
        let config_path = temp.path("nested/dir/config.toml");
        save_config_file(
            &config_path,
            &settings_to_config_file(default_display_settings()),
        )
        .expect("save config");
        assert!(config_path.is_file());
    }
}
