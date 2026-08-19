use std::fs;
use std::path::{Path, PathBuf};

use crate::error::AppError;

use super::types::AppConfigFile;

const DEFAULT_CONFIG_RELATIVE_PATH: &str = "Library/Application Support/cliip-show/config.toml";

pub fn config_file_path() -> Result<PathBuf, AppError> {
    config_file_path_with(|name| std::env::var(name).ok())
}

/// 環境変数の読み取り元を差し替えられる本体。`std::env` はプロセスグローバルで
/// 並列実行のテストから安全に操作できないため、テストは lookup を注入して呼ぶ。
fn config_file_path_with(lookup: impl Fn(&str) -> Option<String>) -> Result<PathBuf, AppError> {
    if let Some(path) = lookup("CLIIP_SHOW_CONFIG_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    let home = lookup("HOME").ok_or_else(|| {
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

    use super::{config_file_path_with, load_config_file, save_config_file};
    use crate::config::settings::{
        apply_config_file, default_display_settings, merge_display_settings,
        settings_to_config_file, start_at_login_from_config,
    };
    use crate::config::types::{
        AppConfigFile, HudBackgroundColor, HudPosition, LanguageSetting, StartupConfigFile,
    };
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

    /// `[startup]` セクションも他のセクションと同じく保存 → 再読込で値が変わらないこと。
    /// `settings_to_config_file` は保存経路の中間表現で `start_at_login` を常に `Some(false)`
    /// に固定するため通さず、`AppConfigFile` を直接組んで `Some(true)` が保たれるかを見る。
    #[test]
    fn start_at_login_survives_a_save_load_cycle() {
        let temp = TempConfigDir::new("startup");
        let config = AppConfigFile {
            startup: StartupConfigFile {
                start_at_login: Some(true),
            },
            ..Default::default()
        };

        let config_path = temp.path("config.toml");
        save_config_file(&config_path, &config).expect("save config");
        let (loaded, existed) = load_config_file(&config_path).expect("load config");
        assert!(existed);
        assert_eq!(start_at_login_from_config(&loaded), Some(true));
    }

    /// `[startup]` セクション自体が無い設定ファイルは、`false` ではなく「未設定」
    /// （`None`）として読めること。plist の有無からの移行はこの区別に依存する。
    #[test]
    fn absent_startup_section_is_none() {
        let temp = TempConfigDir::new("no-startup-section");
        let config_path = temp.path("config.toml");
        fs::write(&config_path, "[display]\npoll_interval_secs = 0.5\n")
            .expect("write config without [startup]");

        let (loaded, existed) = load_config_file(&config_path).expect("load config");
        assert!(existed);
        assert_eq!(start_at_login_from_config(&loaded), None);
    }

    /// `save_settings`（設定ウィンドウの「保存」ボタン）が使う `merge_display_settings` を
    /// 経由して保存しても、既存の `start_at_login` を巻き戻さないこと。丸ごと上書きに
    /// 戻す変更が起きたらここで落ちる。
    #[test]
    fn merge_display_settings_preserves_existing_start_at_login() {
        let temp = TempConfigDir::new("preserve-startup");
        let config_path = temp.path("config.toml");

        let existing = AppConfigFile {
            startup: StartupConfigFile {
                start_at_login: Some(true),
            },
            ..Default::default()
        };
        save_config_file(&config_path, &existing).expect("save initial config");

        let mut draft_settings = default_display_settings();
        draft_settings.hud_scale = 1.7;
        let (config, _) = load_config_file(&config_path).expect("load config");
        let merged = merge_display_settings(config, draft_settings);
        save_config_file(&config_path, &merged).expect("save merged config");

        let (loaded, _) = load_config_file(&config_path).expect("reload config");
        assert_eq!(start_at_login_from_config(&loaded), Some(true));
        assert_eq!(
            apply_config_file(default_display_settings(), &loaded).hud_scale,
            1.7
        );
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

    /// `CLIIP_SHOW_CONFIG_PATH` があれば HOME より優先される。前後の空白は取り除かれる。
    /// VRT スクリプトはこの変数で開発者の実設定を隔離している。
    #[test]
    fn explicit_config_path_wins_over_home() {
        let path = config_file_path_with(|name| match name {
            "CLIIP_SHOW_CONFIG_PATH" => Some(" /tmp/custom.toml ".to_string()),
            "HOME" => Some("/Users/example".to_string()),
            _ => None,
        })
        .expect("resolve path");
        assert_eq!(path, PathBuf::from("/tmp/custom.toml"));
    }

    /// 空白のみの `CLIIP_SHOW_CONFIG_PATH` は未設定と同じ扱いで HOME 配下に落ちる。
    #[test]
    fn blank_config_path_falls_back_to_home() {
        let path = config_file_path_with(|name| match name {
            "CLIIP_SHOW_CONFIG_PATH" => Some("   ".to_string()),
            "HOME" => Some("/Users/example".to_string()),
            _ => None,
        })
        .expect("resolve path");
        assert_eq!(
            path,
            PathBuf::from("/Users/example/Library/Application Support/cliip-show/config.toml")
        );
    }

    /// HOME が引けない・空白のみのときはエラーになる（勝手なパスに書きにいかない）。
    #[test]
    fn missing_or_blank_home_is_an_error() {
        let error = config_file_path_with(|_| None).expect_err("must fail without HOME");
        assert!(matches!(error, AppError::ConfigResolve(_)), "{error:?}");

        let error = config_file_path_with(|name| match name {
            "HOME" => Some("  ".to_string()),
            _ => None,
        })
        .expect_err("must fail with blank HOME");
        assert!(matches!(error, AppError::ConfigResolve(_)), "{error:?}");
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
