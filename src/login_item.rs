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

/// ログイン時に launchd が起動したことを示すフラグ。`plist_xml` が
/// `ProgramArguments` に書き込み、起動を知らせる HUD を出すかの判断に使う。
pub const LOGIN_FLAG: &str = "--login";

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

/// plist が実在しないパスを指していたら、今の実行ファイルへ向け直す。配布形態が
/// 変わったユーザー（Homebrew の formula から cask へ移った、`.app` を別の場所へ
/// 動かした）の自動起動が黙って止まるのを防ぐ。次回ログインから元に戻る。
///
/// plist を読めないときは自動起動が無効なだけなので、何もせず終える。
pub fn repair_stale_plist() -> Result<(), AppError> {
    let Some(path) = plist_path() else {
        return Ok(());
    };
    let Ok(xml) = fs::read_to_string(&path) else {
        return Ok(());
    };
    let current_exe = std::env::current_exe().map_err(|source| AppError::ConfigRead {
        path: path.display().to_string(),
        source,
    })?;
    // `current_exe()` はシンボリックリンクを解決しない。cask が張ったリンク経由で
    // 起動すると `.app` の中かどうかを判定できないため、実体のパスに直す。
    let current_exe = fs::canonicalize(&current_exe).unwrap_or(current_exe);

    if needs_repair(&xml, &current_exe, |p| p.exists()) {
        enable()?;
    }
    Ok(())
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

/// plist の `ProgramArguments` を並び順に返す。`plist_xml` が書いた書式だけを読み、
/// 読めなければ `None`。空の配列も `None` を返す（実行ファイルの無い plist は
/// このアプリが書いたものではない）。
pub fn program_arguments_in_plist(xml: &str) -> Option<Vec<String>> {
    let after_key = xml.split_once("<key>ProgramArguments</key>")?.1;
    let array = after_key.split_once("<array>")?.1;
    let mut rest = array.split_once("</array>")?.0;

    let mut arguments = Vec::new();
    while let Some((_, after_open)) = rest.split_once("<string>") {
        let (value, after_close) = after_open.split_once("</string>")?;
        arguments.push(unescape_xml(value.trim()));
        rest = after_close;
    }

    if arguments.is_empty() {
        return None;
    }
    Some(arguments)
}

/// plist の `ProgramArguments` の先頭に書かれた実行ファイルのパスを返す。
pub fn program_path_in_plist(xml: &str) -> Option<String> {
    program_arguments_in_plist(xml)?.into_iter().next()
}

/// `/Applications` 配下にインストールされた `.app` の中の実行ファイルか。
///
/// 親ディレクトリ名まで見るのは、`target/bundle` に組み立てた動作確認用の `.app` を
/// 弾くため。ビルドのたびに作り直されるパスを plist に書くと、次のビルドで自動起動が
/// 黙って止まる。cask の `--appdir` を既定から変えている場合は書き直しが効かないが、
/// 誤って壊すよりは何もしないほうを採る。
fn is_installed_app_bundle(exe: &Path) -> bool {
    exe.ancestors()
        .find(|dir| dir.extension().is_some_and(|ext| ext == "app"))
        .and_then(|app| app.parent())
        .is_some_and(|parent| {
            parent
                .file_name()
                .is_some_and(|name| name == "Applications")
        })
}

/// plist を書き直すかを決める。書き直すのは、今の実行ファイルがインストール済みの
/// `.app` の中にあり、かつ plist が指す実行ファイルが実在しないか `LOGIN_FLAG` を
/// 持たないとき。
///
/// 書式を読めない plist は他のツールが作ったものの可能性があるので触らない。判定は
/// `program_arguments_in_plist` が `Some` を返した中だけで行う。
///
/// `current_exe` はシンボリックリンクを解決してから渡すこと。
pub fn needs_repair(xml: &str, current_exe: &Path, exists: impl Fn(&Path) -> bool) -> bool {
    if !is_installed_app_bundle(current_exe) {
        return false;
    }
    program_arguments_in_plist(xml).is_some_and(|arguments| {
        let program_is_missing = arguments
            .first()
            .is_some_and(|program| !exists(Path::new(program)));
        program_is_missing || !arguments.iter().any(|argument| argument == LOGIN_FLAG)
    })
}

/// launchd がログイン時に起動したか。`plist_xml` が書いたフラグの有無で判別する。
pub fn started_at_login() -> bool {
    launched_with_login_flag(std::env::args())
}

fn launched_with_login_flag(arguments: impl Iterator<Item = String>) -> bool {
    arguments.skip(1).any(|argument| argument == LOGIN_FLAG)
}

// `escape_xml` の逆。`&amp;` を最後に戻さないと、`&amp;lt;` が `<` に化ける
fn unescape_xml(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
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
		<string>{LOGIN_FLAG}</string>
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
    use super::{launched_with_login_flag, LOGIN_FLAG, LOGIN_ITEM_LABEL};
    use super::{
        needs_repair, plist_xml, program_arguments_in_plist, program_path_in_plist,
        stable_executable_path,
    };
    use std::path::Path;

    const APP_EXE: &str = "/Applications/Cliip Show.app/Contents/MacOS/cliip-show";

    fn plist_pointing_at(executable: &str) -> String {
        plist_xml(Path::new(executable), Path::new("/tmp/cliip-show.log"))
    }

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
    fn program_path_is_read_back_from_the_plist() {
        let xml = plist_pointing_at("/opt/homebrew/bin/cliip-show");
        assert_eq!(
            program_path_in_plist(&xml).as_deref(),
            Some("/opt/homebrew/bin/cliip-show")
        );
    }

    // エスケープしたまま読み戻すと、実在するパスを「無い」と判定して毎回書き直してしまう
    #[test]
    fn program_path_round_trips_through_xml_escaping() {
        let executable = "/Applications/A & <B> \"C\".app/Contents/MacOS/cliip-show";
        let xml = plist_pointing_at(executable);
        assert_eq!(program_path_in_plist(&xml).as_deref(), Some(executable));
    }

    #[test]
    fn program_path_is_none_for_a_plist_without_program_arguments() {
        assert_eq!(program_path_in_plist("<plist><dict/></plist>"), None);
    }

    #[test]
    fn repair_is_needed_when_the_plist_points_at_a_missing_executable() {
        let xml = plist_pointing_at("/opt/homebrew/bin/cliip-show");
        assert!(needs_repair(&xml, Path::new(APP_EXE), |_| false));
    }

    #[test]
    fn repair_is_skipped_when_the_plist_still_points_at_an_existing_executable() {
        let xml = plist_pointing_at("/opt/homebrew/bin/cliip-show");
        assert!(!needs_repair(&xml, Path::new(APP_EXE), |_| true));
    }

    // 開発ビルドを起動しただけで plist が target/debug を指すようになるのを防ぐ
    #[test]
    fn repair_is_skipped_when_the_running_executable_is_not_in_an_app_bundle() {
        let xml = plist_pointing_at("/opt/homebrew/bin/cliip-show");
        let exe = Path::new("/Users/someone/repo/target/debug/cliip-show");
        assert!(!needs_repair(&xml, exe, |_| false));
    }

    // 動作確認用に組み立てた .app はビルドのたびに作り直されるので、plist に書かない
    #[test]
    fn repair_is_skipped_for_an_app_bundle_outside_applications() {
        let xml = plist_pointing_at("/opt/homebrew/bin/cliip-show");
        let exe =
            Path::new("/Users/someone/repo/target/bundle/Cliip Show.app/Contents/MacOS/cliip-show");
        assert!(!needs_repair(&xml, exe, |_| false));
    }

    #[test]
    fn repair_is_needed_for_an_app_bundle_in_the_home_applications_folder() {
        let xml = plist_pointing_at("/opt/homebrew/bin/cliip-show");
        let exe = Path::new("/Users/someone/Applications/Cliip Show.app/Contents/MacOS/cliip-show");
        assert!(needs_repair(&xml, exe, |_| false));
    }

    #[test]
    fn repair_is_skipped_for_a_plist_this_app_did_not_write() {
        assert!(!needs_repair(
            "<plist><dict/></plist>",
            Path::new(APP_EXE),
            |_| false
        ));
    }

    // フラグの有無だけで判断すると、書式を読めない plist が「フラグ無し」に見えて
    // 上書き対象になる。他のツールが置いた plist は実行ファイルが実在しても触らない
    #[test]
    fn repair_is_skipped_for_an_unreadable_plist_even_though_it_has_no_login_flag() {
        assert!(!needs_repair(
            "<plist><dict/></plist>",
            Path::new(APP_EXE),
            |_| true
        ));
    }

    // フラグを持たない plist のまま起動すると、ログインのたびに起動を知らせてしまう
    #[test]
    fn repair_is_needed_when_the_plist_has_no_login_flag() {
        let xml = plist_pointing_at("/opt/homebrew/bin/cliip-show")
            .replace(&format!("\t\t<string>{LOGIN_FLAG}</string>\n"), "");
        assert!(!xml.contains(LOGIN_FLAG));
        assert!(needs_repair(&xml, Path::new(APP_EXE), |_| true));
    }

    #[test]
    fn program_arguments_are_read_back_in_order() {
        let xml = plist_pointing_at("/opt/homebrew/bin/cliip-show");
        assert_eq!(
            program_arguments_in_plist(&xml),
            Some(vec![
                "/opt/homebrew/bin/cliip-show".to_string(),
                LOGIN_FLAG.to_string(),
            ])
        );
    }

    #[test]
    fn the_login_flag_is_detected_only_when_it_is_passed() {
        let args = |list: &[&str]| {
            list.iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .into_iter()
        };
        assert!(launched_with_login_flag(args(&[
            "/Applications/Cliip Show.app/Contents/MacOS/cliip-show",
            LOGIN_FLAG
        ])));
        assert!(!launched_with_login_flag(args(&[
            "/Applications/Cliip Show.app/Contents/MacOS/cliip-show"
        ])));
        // 実行ファイル名は判定に混ぜない
        assert!(!launched_with_login_flag(args(&[LOGIN_FLAG])));
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
