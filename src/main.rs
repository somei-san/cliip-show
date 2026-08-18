fn main() {
    if cliip_show::cli::handle_cli_flags() {
        return;
    }

    unsafe {
        use objc2::runtime::AnyObject;
        use objc2::{class, msg_send};

        // すぐ終える側が LaunchAgent の plist を書き換えないよう、修復より先に見る
        if cliip_show::single_instance::activate_existing_instance() {
            return;
        }

        if let Err(error) = cliip_show::login_item::repair_stale_plist() {
            eprintln!("warning: {error}");
        }

        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        if app.is_null() {
            eprintln!("fatal: failed to initialize NSApplication");
            std::process::exit(1);
        }
        let _: bool = msg_send![app, setActivationPolicy: 1isize];

        let delegate_class = cliip_show::app::get_delegate_class();
        let delegate: *mut AnyObject = msg_send![delegate_class, new];
        let () = msg_send![app, setDelegate: delegate];
        // 2 つ目のインスタンスは起動をやめる直前に依頼を投げる。run より前に購読して
        // おかないと、ロックを取ってから購読するまでの間に来た依頼を取りこぼす
        cliip_show::single_instance::observe_activation_requests(delegate);
        let () = msg_send![app, run];
    }
}
