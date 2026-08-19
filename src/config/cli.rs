use super::io::{config_file_path, load_config_file};
use super::settings::{
    apply_config_file, apply_env_overrides, default_display_settings, print_effective_settings,
    print_saved_settings,
};

/// `--config-show` の処理。設定の変更手段は設定ウィンドウに一本化しているため、
/// CLI からは現在値の確認だけできる。不具合の切り分けに使う。
pub fn show_config<I: Iterator<Item = String>>(args: &mut I) -> bool {
    if args.next().is_some() {
        eprintln!("Usage: cliip-show --config-show");
        std::process::exit(2);
    }

    let path = match config_file_path() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    println!("config_path = {}", path.display());
    let (config, loaded_from_file) = match load_config_file(&path) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    if loaded_from_file {
        println!("config_file = exists");
        println!("[saved]");
        print_saved_settings(&config);
    } else {
        println!("config_file = not_found");
    }
    println!("[effective]");
    let effective = apply_env_overrides(apply_config_file(default_display_settings(), &config));
    let start_at_login = crate::login_item::resolved_start_at_login(&config);
    print_effective_settings(effective, start_at_login);
    true
}
