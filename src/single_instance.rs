//! 二重起動の抑止。
//!
//! GUI からの起動（Spotlight・Finder・`open`）は Launch Services が同じバンドルの
//! 二重起動を止めるため、ここで相手にするのはバンドル内の実行ファイルを直接叩いた
//! 経路（cask が張る `cliip-show` の symlink・LaunchAgent・スクリプト）だけ。
//!
//! 起動できるのはロックファイルの排他ロックを取れたプロセスだけ。取れなかった側は
//! 前面化を頼んで終了する。頼み事は `NSDistributedNotificationCenter` で送り、
//! 受けた側は設定ウィンドウを開く。

use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use objc2::runtime::AnyObject;
use objc2::{class, msg_send, sel};
use objc2_foundation::NSString;

/// 前面化を頼む通知の名前。通知センターはシステム全体で共有なので bundle id で名前空間を切る。
pub const ACTIVATE_NOTIFICATION: &str = "io.github.somei-san.cliip-show.activate";

const LOCK_FILE_NAME: &str = ".instance-lock";

extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
}

/// `LOCK_EX | LOCK_NB`。待たずに取れなければ失敗させる。
const LOCK_EXCLUSIVE_NONBLOCKING: i32 = 2 | 4;

/// 自分の bundle id。バンドルの外から起動した開発ビルドでは null になる。
///
/// # Safety
/// AppKit のメインスレッドから呼ぶこと。
unsafe fn bundle_identifier() -> *mut AnyObject {
    let bundle: *mut AnyObject = msg_send![class!(NSBundle), mainBundle];
    if bundle.is_null() {
        return std::ptr::null_mut();
    }
    msg_send![bundle, bundleIdentifier]
}

/// ロックファイルは設定ファイルと同じディレクトリに置く
/// （`CLIIP_SHOW_CONFIG_PATH` で配置先が変わるため決め打ちしない）。
fn lock_path() -> Option<PathBuf> {
    let config_path = crate::config::config_file_path().ok()?;
    Some(config_path.parent()?.join(LOCK_FILE_NAME))
}

/// 排他ロックを取れたら `true`。取れなければ他のインスタンスが動いている。
///
/// `flock` はプロセスが死ねばカーネルが外すので、クラッシュしてもロックが残らない。
/// ロックの取得を open と同時に済ませられるため、ログイン時に launchd が 2 つの
/// エージェントを同時に立ち上げても、勝つのは必ず 1 つだけになる。
///
/// パスを決められない・ファイルを開けないときは `true` を返す。二重起動の抑止より
/// アプリが起動することを優先する。
fn acquire_lock() -> bool {
    lock_path().is_none_or(|path| acquire_lock_at(&path))
}

fn acquire_lock_at(path: &Path) -> bool {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(file) = OpenOptions::new().create(true).append(true).open(path) else {
        return true;
    };

    // SAFETY: 開いたばかりの有効な fd を渡している
    if unsafe { flock(file.as_raw_fd(), LOCK_EXCLUSIVE_NONBLOCKING) } != 0 {
        return false;
    }

    // ファイルを閉じるとロックも外れる。プロセスが終わるまで開いたままにする
    std::mem::forget(file);
    true
}

/// 既に動いているインスタンスがあれば前面化を頼み、`true` を返す。
/// 呼び出し側は `true` なら起動をやめる。
///
/// バンドルの外から起動した開発ビルドはロックを取りに行かない。`cargo run` が
/// インストール済みのアプリと同じロックを奪い合って落ちるのを防ぐため。
///
/// # Safety
/// AppKit のメインスレッドから呼ぶこと。
pub unsafe fn activate_existing_instance() -> bool {
    if bundle_identifier().is_null() {
        return false;
    }
    if acquire_lock() {
        return false;
    }

    let center: *mut AnyObject = msg_send![class!(NSDistributedNotificationCenter), defaultCenter];
    let name = NSString::from_str(ACTIVATE_NOTIFICATION);
    // 直後に終了するため、配送を待たずに投げると受け取ってもらえない
    let () = msg_send![
        center,
        postNotificationName: &*name
        object: std::ptr::null_mut::<AnyObject>()
        userInfo: std::ptr::null_mut::<AnyObject>()
        deliverImmediately: true
    ];

    eprintln!("cliip-show is already running");
    true
}

/// 他のインスタンスからの前面化依頼を受け取れるようにする。
/// 依頼は `openSettings:` に流し、設定ウィンドウを開いて前面に出す。
///
/// 依頼はロックを取れなかった側が投げるので、ロックを取った直後から受け取れないと
/// 取りこぼす。`applicationDidFinishLaunching:` を待たず、`run` の前に登録すること。
///
/// 開発ビルドは購読しない。ロックを取りに行かないのと揃える。
///
/// # Safety
/// AppKit のメインスレッドから、AppDelegate を渡して呼ぶこと。
pub unsafe fn observe_activation_requests(delegate: *mut AnyObject) {
    if bundle_identifier().is_null() {
        return;
    }

    let center: *mut AnyObject = msg_send![class!(NSDistributedNotificationCenter), defaultCenter];
    let name = NSString::from_str(ACTIVATE_NOTIFICATION);
    let () = msg_send![
        center,
        addObserver: delegate
        selector: sel!(openSettings:)
        name: &*name
        object: std::ptr::null_mut::<AnyObject>()
    ];
}

#[cfg(test)]
mod tests {
    use super::{acquire_lock_at, lock_path, ACTIVATE_NOTIFICATION, LOCK_FILE_NAME};
    use std::path::PathBuf;

    fn scratch_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("cliip-show-test-{name}"))
    }

    // システム全体の通知センターを使うため、名前が衝突すると他アプリの通知を拾う
    #[test]
    fn the_notification_name_is_namespaced_by_the_bundle_identifier() {
        assert!(ACTIVATE_NOTIFICATION.starts_with(crate::login_item::LOGIN_ITEM_LABEL));
    }

    // 設定ファイルと同じディレクトリに置く。cask の zap で設定ごと消える位置
    #[test]
    fn the_lock_sits_next_to_the_config_file() {
        let lock = lock_path().expect("lock path");
        let config = crate::config::config_file_path().expect("config path");
        assert_eq!(lock.parent(), config.parent());
        assert_eq!(lock.file_name().unwrap(), LOCK_FILE_NAME);
    }

    // flock は同じプロセスでも別の fd 同士なら衝突する。2 つ目のインスタンスが
    // 起動をやめる判断は、この衝突だけを根拠にしている
    #[test]
    fn the_second_lock_on_the_same_file_is_refused() {
        let path = scratch_path("second-lock");
        let _ = std::fs::remove_file(&path);
        assert!(acquire_lock_at(&path));
        assert!(!acquire_lock_at(&path));
    }

    // ロックの置き場所を作れないときは、抑止よりアプリが起動することを優先する
    #[test]
    fn an_unusable_lock_path_does_not_stop_the_app() {
        let path = scratch_path("unusable/../unusable-file/lock");
        assert!(acquire_lock_at(&path));
    }
}
