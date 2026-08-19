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

/// 設定ファイルの値を正としつつ、未設定なら plist の有無を実効値として使う
/// （`start_at_login_from_config` が区別する「未設定」を、呼び出し側が使える1つの
/// bool へ解決する）。
pub fn resolved_start_at_login(config: &crate::config::AppConfigFile) -> bool {
    crate::config::start_at_login_from_config(config).unwrap_or_else(is_enabled)
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

/// 書き直しが要る plist（条件は `needs_repair`）を、今の実行ファイルへ向けて書き直す。
/// 配布形態が変わったユーザー（Homebrew の formula から cask へ移った、`.app` を別の
/// 場所へ動かした）の自動起動が黙って止まるのを防ぐ。次回ログインから効く。
///
/// `enable` が plist を丸ごと生成し直すため、手で足したキーは残らない。plist の管理は
/// アプリに寄せる方針（モジュールの説明を参照）に沿う。
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

/// 設定ファイルの `start_at_login` を正として、LaunchAgent の plist をその値に合わせる。
/// `[startup]` 節を持たない設定ファイルで plist が実在するときだけ、`true` を書き戻す
/// （plist が外から消えても、次の起動でここが設定から復元できるようにするため）。
/// plist が無ければ書き戻さない。設定ファイルを持たない環境で自動起動を使っていない
/// 利用者に、書き戻しのためだけの設定ファイルを作らせないため。
///
/// 設定ファイルの読み書きに失敗したらエラーを返す。呼び出し側（`main`）は警告に
/// 留めて起動を続ける。
pub fn sync_plist_with_config() -> Result<(), AppError> {
    let config_path = crate::config::config_file_path()?;
    let (mut config, _) = crate::config::load_config_file(&config_path)?;

    let plist_exists = is_enabled();
    let desired = match crate::config::start_at_login_from_config(&config) {
        Some(value) => value,
        None => {
            if let Some(value) = start_at_login_to_persist(plist_exists) {
                config.startup.start_at_login = Some(value);
                crate::config::save_config_file(&config_path, &config)?;
            }
            plist_exists
        }
    };

    apply_desired_state(desired)
}

/// 設定の希望値へ plist を合わせる。`login_item_action` で行動（`Enable`/`Repair`/`Disable`/
/// 何もしない）を決めて実行する。`sync_plist_with_config` と、設定ファイルの手編集を拾う
/// ファイル監視の再読み込みの両方から使う。
pub fn apply_desired_state(desired_enabled: bool) -> Result<(), AppError> {
    let plist_exists = is_enabled();
    let action = login_item_action(
        desired_enabled,
        plist_exists,
        current_exe_is_installed_app_bundle(),
    );
    if desired_enabled && action == LoginItemAction::None {
        // 開発ビルドのパスを plist に焼き付けると、次のビルドで自動起動が黙って止まる。
        // 書き出しを見送ったことは利用者から見えないので、ここで知らせる。
        eprintln!(
            "warning: start_at_login is enabled but the running executable is not an installed .app; skipping plist creation"
        );
    }
    match action {
        LoginItemAction::Enable => enable(),
        LoginItemAction::Repair => repair_stale_plist(),
        LoginItemAction::Disable => disable(),
        LoginItemAction::None => Ok(()),
    }
}

/// 現在の実行ファイルが、インストール済みの `.app` の中にあるか。
/// `current_exe()` に失敗したときは安全側に倒し `false` とする。
fn current_exe_is_installed_app_bundle() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    // `current_exe()` はシンボリックリンクを解決しない。cask が張ったリンク経由で
    // 起動すると `.app` の中かどうかを判定できないため、実体のパスに直す。
    let exe = fs::canonicalize(&exe).unwrap_or(exe);
    is_installed_app_bundle(&exe)
}

/// 設定に値が無いときの書き戻し判断（副作用なし）。plist があるときだけ `true` を書き戻す。
/// plist が無いときに書き戻さないのは、自動起動を使っていない利用者に、書き戻しのためだけの
/// 設定ファイルを作らせないため。
fn start_at_login_to_persist(plist_exists: bool) -> Option<bool> {
    plist_exists.then_some(true)
}

/// 設定の希望値と plist の実在から、次に取る行動を決める（副作用なし）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoginItemAction {
    Enable,
    Repair,
    Disable,
    None,
}

/// `installed_app_bundle` は `is_installed_app_bundle` の結果（実行ファイルがインストール済み
/// `.app` の中にあるか）。plist が無い状態で新規に書き出す（`Enable`）のは、インストール済み
/// `.app` から実行しているときだけに限る。開発ビルドや `cargo run` の実行ファイルパスを
/// 焼き付けると、次のビルドで自動起動が黙って壊れるため。
fn login_item_action(
    desired_enabled: bool,
    plist_exists: bool,
    installed_app_bundle: bool,
) -> LoginItemAction {
    match (desired_enabled, plist_exists) {
        (true, false) if installed_app_bundle => LoginItemAction::Enable,
        (true, false) => LoginItemAction::None,
        (true, true) => LoginItemAction::Repair,
        (false, true) => LoginItemAction::Disable,
        (false, false) => LoginItemAction::None,
    }
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

/// 自動起動の値だけを設定ファイルへ書く。書き込む前に現在のファイルを読み直すのは、
/// 外部エディタで変更された他のキーを巻き戻さないため。
pub fn save_start_at_login(path: &Path, value: bool) -> Result<(), AppError> {
    let (mut config, _) = crate::config::load_config_file(path)?;
    config.startup.start_at_login = Some(value);
    crate::config::save_config_file(path, &config)
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

/// plist を書き直すかを決める。書き直すのは、plist が指す実行ファイルが実在しないか
/// `LOGIN_FLAG` を持たないとき。
///
/// ただし今の実行ファイルを指していない plist は、インストール済みの `.app` から
/// 起動したときしか書き直さない。開発ビルドのパスを焼き付けないため。
///
/// 書式を読めない plist は他のツールが作ったものの可能性があるので触らない。判定は
/// `program_arguments_in_plist` が `Some` を返した中だけで行う。
///
/// `current_exe` はシンボリックリンクを解決してから渡すこと。
pub fn needs_repair(xml: &str, current_exe: &Path, exists: impl Fn(&Path) -> bool) -> bool {
    program_arguments_in_plist(xml).is_some_and(|arguments| {
        let program = arguments.first().map(Path::new);
        if program != Some(current_exe) && !is_installed_app_bundle(current_exe) {
            return false;
        }
        program.is_some_and(|program| !exists(program))
            || !arguments.iter().any(|argument| argument == LOGIN_FLAG)
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
    use super::{login_item_action, start_at_login_to_persist, LoginItemAction};
    use super::{needs_repair, plist_xml, program_arguments_in_plist, stable_executable_path};
    use std::path::Path;

    const APP_EXE: &str = "/Applications/Cliip Show.app/Contents/MacOS/cliip-show";

    fn plist_pointing_at(executable: &str) -> String {
        plist_xml(Path::new(executable), Path::new("/tmp/cliip-show.log"))
    }

    /// `LOGIN_FLAG` を書くようになる前の plist。
    fn plist_without_login_flag(executable: &str) -> String {
        let xml = plist_pointing_at(executable)
            .replace(&format!("\t\t<string>{LOGIN_FLAG}</string>\n"), "");
        assert!(!xml.contains(LOGIN_FLAG), "フラグを外せていない");
        xml
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

    // エスケープしたまま読み戻すと、実在するパスを「無い」と判定して毎回書き直してしまう
    #[test]
    fn program_arguments_round_trip_through_xml_escaping() {
        let executable = "/Applications/A & <B> \"C\".app/Contents/MacOS/cliip-show";
        let xml = plist_pointing_at(executable);
        assert_eq!(
            program_arguments_in_plist(&xml).as_deref(),
            Some([executable.to_string(), LOGIN_FLAG.to_string()].as_slice())
        );
    }

    #[test]
    fn program_arguments_are_none_for_a_plist_without_program_arguments() {
        assert_eq!(program_arguments_in_plist("<plist><dict/></plist>"), None);
    }

    // 中身の無い配列と閉じていない <string> は、いずれも書式を読めない plist として扱う。
    // ここが Some を返すと、他のツールが置いた plist を書き直してしまう
    #[test]
    fn program_arguments_are_none_for_a_malformed_array() {
        let empty = "<key>ProgramArguments</key><array></array>";
        assert_eq!(program_arguments_in_plist(empty), None);
        assert!(!needs_repair(empty, Path::new(APP_EXE), |_| false));

        let unterminated = "<key>ProgramArguments</key><array><string>/bin/x</array>";
        assert_eq!(program_arguments_in_plist(unterminated), None);
        assert!(!needs_repair(unterminated, Path::new(APP_EXE), |_| false));
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

    // 自分自身を指す plist の書き直しはパスを変えないので、`.app` の外から起動しても
    // フラグだけ足せる。ここを塞ぐと、`.app` に入らない構成では毎ログイン HUD が出る
    #[test]
    fn repair_is_needed_outside_a_bundle_when_the_plist_already_points_at_the_running_executable() {
        let exe = "/Users/someone/repo/target/debug/cliip-show";
        let xml = plist_without_login_flag(exe);
        assert!(needs_repair(&xml, Path::new(exe), |_| true));
    }

    // フラグを持たない plist のまま起動すると、ログインのたびに起動を知らせてしまう
    #[test]
    fn repair_is_needed_when_the_plist_has_no_login_flag() {
        let xml = plist_without_login_flag("/opt/homebrew/bin/cliip-show");
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

    #[test]
    fn login_item_action_enables_when_desired_missing_and_installed() {
        assert_eq!(
            login_item_action(true, false, true),
            LoginItemAction::Enable
        );
    }

    // インストール済み .app の外（`cargo run`・開発ビルド・Homebrew formula 時代のパス等）
    // から実行しているときに新規作成すると、次のビルド・アンインストールでパスが消えて
    // 自動起動が黙って壊れる。plist を書かず何もしない。
    #[test]
    fn login_item_action_does_nothing_when_desired_missing_and_not_installed() {
        assert_eq!(login_item_action(true, false, false), LoginItemAction::None);
    }

    #[test]
    fn login_item_action_repairs_when_desired_and_present() {
        assert_eq!(
            login_item_action(true, true, false),
            LoginItemAction::Repair
        );
        assert_eq!(login_item_action(true, true, true), LoginItemAction::Repair);
    }

    #[test]
    fn login_item_action_disables_when_not_desired_but_present() {
        assert_eq!(
            login_item_action(false, true, false),
            LoginItemAction::Disable
        );
        assert_eq!(
            login_item_action(false, true, true),
            LoginItemAction::Disable
        );
    }

    #[test]
    fn login_item_action_does_nothing_when_not_desired_and_absent() {
        assert_eq!(
            login_item_action(false, false, false),
            LoginItemAction::None
        );
        assert_eq!(login_item_action(false, false, true), LoginItemAction::None);
    }

    #[test]
    fn start_at_login_to_persist_writes_back_true_when_plist_exists() {
        assert_eq!(start_at_login_to_persist(true), Some(true));
    }

    #[test]
    fn start_at_login_to_persist_writes_nothing_when_plist_is_absent() {
        assert_eq!(start_at_login_to_persist(false), None);
    }
}
