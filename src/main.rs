fn main() {
    if cliip_show::cli::handle_cli_flags() {
        return;
    }

    unsafe {
        use objc2::runtime::AnyObject;
        use objc2::{class, msg_send};

        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        if app.is_null() {
            eprintln!("fatal: NSApplication の初期化に失敗しました");
            std::process::exit(1);
        }
        let _: bool = msg_send![app, setActivationPolicy: 1isize];

        let delegate_class = cliip_show::app::get_delegate_class();
        let delegate: *mut AnyObject = msg_send![delegate_class, new];
        let () = msg_send![app, setDelegate: delegate];
        let () = msg_send![app, run];
    }
}
