//! ログイン時の自動起動を管理する。Homebrew の service ブロック（launchd）には委譲せず、
//! アプリ自身が `~/Library/LaunchAgents` に LaunchAgent の plist を書き出し `launchctl` で
//! 出し入れする。管理主体を1つに絞ることで二重起動を避ける。

use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::AppError;

pub const LOGIN_ITEM_LABEL: &str = "io.github.somei-san.cliip-show";

/// Homebrew の `service do` ブロックが生成していた LaunchAgent。残っていると
/// アプリ自身の LaunchAgent と二重起動するため、存在確認にのみ使う。
const HOMEBREW_LOGIN_ITEM_LABEL: &str = "homebrew.mxcl.cliip-show";

fn home_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let trimmed = home.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

fn launch_agents_dir() -> Option<PathBuf> {
    home_dir().map(|home| home.join("Library/LaunchAgents"))
}

pub fn plist_path() -> Option<PathBuf> {
    launch_agents_dir().map(|dir| dir.join(format!("{LOGIN_ITEM_LABEL}.plist")))
}

pub fn homebrew_agent_path() -> Option<PathBuf> {
    launch_agents_dir().map(|dir| dir.join(format!("{HOMEBREW_LOGIN_ITEM_LABEL}.plist")))
}

pub fn is_enabled() -> bool {
    plist_path().is_some_and(|path| path.exists())
}

fn current_uid() -> Result<u32, AppError> {
    let home = home_dir().ok_or_else(|| {
        AppError::ConfigResolve("failed to resolve HOME for uid lookup".to_string())
    })?;
    fs::metadata(&home)
        .map(|metadata| metadata.uid())
        .map_err(|source| AppError::ConfigRead {
            path: home.display().to_string(),
            source,
        })
}

fn run_launchctl(args: &[String]) -> Result<(), AppError> {
    let command = format!("launchctl {}", args.join(" "));
    let output = Command::new("launchctl")
        .args(args)
        .output()
        .map_err(|source| AppError::CommandFailed {
            command: command.clone(),
            message: source.to_string(),
        })?;
    if output.status.success() {
        return Ok(());
    }
    Err(AppError::CommandFailed {
        command,
        message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

/// plist を `~/Library/LaunchAgents` に書き出す。`RunAtLoad=true` により、
/// 次回ログイン時に launchd が自動で読み込んで起動する。ここで `launchctl bootstrap`
/// を呼ぶと手動起動中のアプリと二重に立ち上がるため、書き出しだけで終える。
pub fn enable() -> Result<(), AppError> {
    let path = plist_path().ok_or_else(|| {
        AppError::ConfigResolve("failed to resolve HOME for LaunchAgent path".to_string())
    })?;
    let home = home_dir().ok_or_else(|| {
        AppError::ConfigResolve("failed to resolve HOME for LaunchAgent path".to_string())
    })?;

    let current_exe = std::env::current_exe().map_err(|source| AppError::ConfigWrite {
        path: path.display().to_string(),
        source,
    })?;
    let executable = stable_executable_path(&current_exe, |p| p.exists());
    let log_path = home.join("Library/Logs/cliip-show.log");
    let xml = plist_xml(&executable, &log_path);

    let parent = path.parent().ok_or_else(|| {
        AppError::ConfigResolve(format!(
            "failed to determine parent directory for {}",
            path.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(|source| AppError::ConfigWrite {
        path: parent.display().to_string(),
        source,
    })?;
    fs::write(&path, xml).map_err(|source| AppError::ConfigWrite {
        path: path.display().to_string(),
        source,
    })
}

/// `launchctl bootout` で読み込みを解除し、plist を削除する。
///
/// `bootout` の失敗は警告に留めて plist の削除まで進める。エージェントが読み込まれていない
/// ときも `bootout` は失敗するため、ここで中断すると plist が残り、`is_enabled` が有効を
/// 返し続けてチェックを外せなくなる。
pub fn disable() -> Result<(), AppError> {
    let path = plist_path().ok_or_else(|| {
        AppError::ConfigResolve("failed to resolve HOME for LaunchAgent path".to_string())
    })?;
    let uid = current_uid()?;

    if let Err(error) = run_launchctl(&[
        "bootout".to_string(),
        format!("gui/{uid}/{LOGIN_ITEM_LABEL}"),
    ]) {
        eprintln!("warning: {error}");
    }

    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(AppError::ConfigWrite {
            path: path.display().to_string(),
            source,
        }),
    }
}

/// 自動起動確認ダイアログを抑止したかどうかのマーカーファイルのパス。
/// 設定ファイルと同じディレクトリに置く（`CLIIP_SHOW_CONFIG_PATH` で配置先が変わるため決め打ちしない）。
fn prompted_marker_path() -> Option<PathBuf> {
    let config_path = crate::config::config_file_path().ok()?;
    let parent = config_path.parent()?;
    Some(parent.join(".login-item-prompted"))
}

pub fn has_prompted() -> bool {
    prompted_marker_path().is_some_and(|path| path.exists())
}

pub fn mark_prompted() -> Result<(), AppError> {
    let path = prompted_marker_path().ok_or_else(|| {
        AppError::ConfigResolve(
            "failed to resolve config directory for prompted marker".to_string(),
        )
    })?;
    let parent = path.parent().ok_or_else(|| {
        AppError::ConfigResolve(format!(
            "failed to determine parent directory for {}",
            path.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(|source| AppError::ConfigWrite {
        path: parent.display().to_string(),
        source,
    })?;
    fs::write(&path, "").map_err(|source| AppError::ConfigWrite {
        path: path.display().to_string(),
        source,
    })
}

/// Homebrew 経由の `cargo install` では `current_exe()` がシンボリックリンクを解決し、
/// `.../Cellar/cliip-show/<version>/bin/cliip-show` のようなバージョン付きパスを返す。
/// これを plist に書くと `brew upgrade` でパスごと消え、自動起動が黙って壊れるため、
/// バージョンに依存しない `opt` パスへ読み替える。読み替え先が存在しなければ元のパスのまま。
pub fn stable_executable_path(exe: &Path, exists: impl Fn(&Path) -> bool) -> PathBuf {
    match cellar_to_opt_path(exe) {
        Some(opt_path) if exists(&opt_path) => opt_path,
        _ => exe.to_path_buf(),
    }
}

/// `.../Cellar/<name>/<version>/bin/cliip-show` → `.../opt/<name>/bin/cliip-show`
fn cellar_to_opt_path(exe: &Path) -> Option<PathBuf> {
    let components: Vec<_> = exe.components().collect();
    let cellar_index = components.iter().position(|c| c.as_os_str() == "Cellar")?;
    let name = components.get(cellar_index + 1)?;
    // Cellar / <name> / <version> / 以降（bin/cliip-show 等）
    let rest = components.get(cellar_index + 3..)?;
    if rest.is_empty() {
        return None;
    }

    let mut opt_path: PathBuf = components[..cellar_index].iter().collect();
    opt_path.push("opt");
    opt_path.push(name.as_os_str());
    for component in rest {
        opt_path.push(component.as_os_str());
    }
    Some(opt_path)
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// LaunchAgent の plist を組み立てる。`KeepAlive.SuccessfulExit=false` により、メニューの
/// 「終了」（0 終了）では再起動させず、クラッシュ時のみ復帰させる。
pub fn plist_xml(executable: &Path, log_path: &Path) -> String {
    let executable = escape_xml(&executable.display().to_string());
    let log_path = escape_xml(&log_path.display().to_string());
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{LOGIN_ITEM_LABEL}</string>
	<key>ProgramArguments</key>
	<array>
		<string>{executable}</string>
	</array>
	<key>RunAtLoad</key>
	<true/>
	<key>KeepAlive</key>
	<dict>
		<key>SuccessfulExit</key>
		<false/>
	</dict>
	<key>StandardOutPath</key>
	<string>{log_path}</string>
	<key>StandardErrorPath</key>
	<string>{log_path}</string>
</dict>
</plist>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::{plist_xml, stable_executable_path, LOGIN_ITEM_LABEL};
    use std::path::Path;

    #[test]
    fn cellar_path_converts_to_opt_path_when_it_exists() {
        let exe = Path::new("/opt/homebrew/Cellar/cliip-show/0.2.0/bin/cliip-show");
        let opt = Path::new("/opt/homebrew/opt/cliip-show/bin/cliip-show");
        let result = stable_executable_path(exe, |p| p == opt);
        assert_eq!(result, opt);
    }

    #[test]
    fn cellar_path_falls_back_to_original_when_opt_path_is_missing() {
        let exe = Path::new("/opt/homebrew/Cellar/cliip-show/0.2.0/bin/cliip-show");
        let result = stable_executable_path(exe, |_| false);
        assert_eq!(result, exe);
    }

    #[test]
    fn non_cellar_path_passes_through_unchanged() {
        let exe = Path::new("/usr/local/bin/cliip-show");
        let result = stable_executable_path(exe, |_| true);
        assert_eq!(result, exe);
    }

    #[test]
    fn plist_xml_contains_expected_keys() {
        let xml = plist_xml(
            Path::new("/opt/homebrew/opt/cliip-show/bin/cliip-show"),
            Path::new("/Users/someone/Library/Logs/cliip-show.log"),
        );
        assert!(xml.contains(&format!("<string>{LOGIN_ITEM_LABEL}</string>")));
        assert!(xml.contains("<string>/opt/homebrew/opt/cliip-show/bin/cliip-show</string>"));
        assert!(xml.contains("<key>RunAtLoad</key>"));
        assert!(xml.contains("<true/>"));
        assert!(xml.contains("<key>SuccessfulExit</key>"));
        assert!(xml.contains("<false/>"));
        assert!(xml.contains("<string>/Users/someone/Library/Logs/cliip-show.log</string>"));
    }

    #[test]
    fn plist_xml_escapes_special_characters_in_paths() {
        let xml = plist_xml(
            Path::new("/opt/A & B/cliip-show"),
            Path::new("/tmp/log.log"),
        );
        assert!(xml.contains("A &amp; B"));
        assert!(!xml.contains("A & B"));
    }
}
