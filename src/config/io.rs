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
