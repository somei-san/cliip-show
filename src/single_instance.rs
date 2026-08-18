//! 二重起動の抑止。
//!
//! GUI からの起動（Spotlight・Finder・`open`）は Launch Services が同じバンドルの
//! 二重起動を止めるため、ここで相手にするのはバンドル内の実行ファイルを直接叩いた
//! 経路（cask が張る `cliip-show` の symlink・LaunchAgent・スクリプト）だけ。
//!
//! 既に動いているインスタンスを見つけたら、前面化を頼んで自分は終了する。頼み事は
//! `NSDistributedNotificationCenter` で送り、受けた側は設定ウィンドウを開く。

use objc2::runtime::AnyObject;
use objc2::{class, msg_send, sel};
use objc2_foundation::NSString;

/// 前面化を頼む通知の名前。通知センターはシステム全体で共有なので bundle id で名前空間を切る。
pub const ACTIVATE_NOTIFICATION: &str = "io.github.somei-san.cliip-show.activate";

/// 自分以外の pid を返す。
///
/// 列挙には自分自身も含まれる。バンドルから起動していれば自分が載る前に呼ばれることも
/// あるので、載っていないケースも通す。
fn other_pid(pids: &[i32], own_pid: i32) -> Option<i32> {
    pids.iter().copied().find(|pid| *pid != own_pid)
}

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

/// 同じ bundle id で動いているプロセスの pid を集める。
///
/// バンドルの外から起動した開発ビルドは bundle id を持たないため空になり、
/// 二重起動の抑止も前面化も働かない。インストール済みのアプリを開発ビルドが
/// 止めてしまわないよう、これは意図した挙動。
///
/// # Safety
/// AppKit のメインスレッドから呼ぶこと。
unsafe fn running_pids() -> Vec<i32> {
    let identifier = bundle_identifier();
    if identifier.is_null() {
        return Vec::new();
    }

    let apps: *mut AnyObject = msg_send![
        class!(NSRunningApplication),
        runningApplicationsWithBundleIdentifier: identifier
    ];
    if apps.is_null() {
        return Vec::new();
    }

    let count: usize = msg_send![apps, count];
    (0..count)
        .map(|index| {
            let app: *mut AnyObject = msg_send![apps, objectAtIndex: index];
            let pid: i32 = msg_send![app, processIdentifier];
            pid
        })
        .collect()
}

/// 既に動いているインスタンスがあれば前面化を頼み、`true` を返す。
/// 呼び出し側は `true` なら起動をやめる。
///
/// # Safety
/// AppKit のメインスレッドから呼ぶこと。
pub unsafe fn activate_existing_instance() -> bool {
    let own_pid = std::process::id() as i32;
    let Some(pid) = other_pid(&running_pids(), own_pid) else {
        return false;
    };

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

    eprintln!("cliip-show is already running (pid {pid})");
    true
}

/// 他のインスタンスからの前面化依頼を受け取れるようにする。
/// 依頼は `openSettings:` に流し、設定ウィンドウを開いて前面に出す。
///
/// 開発ビルドは購読しない。検出側で対象外にしているのと揃えて、`cargo run` 中に
/// インストール済みのアプリを起動しても開発ビルドの設定ウィンドウが開かないようにする。
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
    use super::{other_pid, ACTIVATE_NOTIFICATION};

    #[test]
    fn no_other_instance_when_the_list_holds_only_this_process() {
        assert_eq!(other_pid(&[42], 42), None);
    }

    #[test]
    fn no_other_instance_when_the_list_is_empty() {
        assert_eq!(other_pid(&[], 42), None);
    }

    // 自分が列挙に載る前に呼ばれることがある
    #[test]
    fn another_instance_is_found_when_this_process_is_not_listed_yet() {
        assert_eq!(other_pid(&[7], 42), Some(7));
    }

    #[test]
    fn another_instance_is_found_next_to_this_process() {
        assert_eq!(other_pid(&[42, 7], 42), Some(7));
    }

    // システム全体の通知センターを使うため、名前が衝突すると他アプリの通知を拾う
    #[test]
    fn the_notification_name_is_namespaced_by_the_bundle_identifier() {
        assert!(ACTIVATE_NOTIFICATION.starts_with(crate::login_item::LOGIN_ITEM_LABEL));
    }
}
