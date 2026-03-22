use std::fmt::Write as _;

use crate::config::handle_config_command;
use crate::png::{generate_diff_png, render_hud_png};

pub fn handle_cli_flags() -> bool {
    let mut args = std::env::args();
    let _program = args.next();
    let Some(flag) = args.next() else {
        return false;
    };

    match flag.as_str() {
        "--version" | "-V" | "-v" => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            true
        }
        "--help" | "-h" => {
            let mut help = String::new();
            let _ = writeln!(help, "cliip-show {}", env!("CARGO_PKG_VERSION"));
            let _ = writeln!(help, "clipboard HUD resident app for macOS");
            let _ = writeln!(help);
            let _ = writeln!(help, "Options:");
            let _ = writeln!(help, "  -h, --help       Print help");
            let _ = writeln!(help, "  -v, -V, --version    Print version");
            let _ = writeln!(
                help,
                "  --render-hud-png --text <TEXT> --output <PATH>    Render HUD snapshot PNG and exit"
            );
            let _ = writeln!(
                help,
                "  --diff-png --baseline <PATH> --current <PATH> --output <PATH>    Generate visual diff PNG and exit"
            );
            let _ = writeln!(
                help,
                "  --config <path|show|init|set ...>    Manage persistent settings file"
            );
            let _ = writeln!(help);
            let _ = writeln!(help, "Config commands (persistent settings):");
            let _ = writeln!(help, "  cliip-show --config init");
            let _ = writeln!(help, "  cliip-show --config init --force");
            let _ = writeln!(help, "  cliip-show --config show");
            let _ = writeln!(help, "  cliip-show --config set hud_duration_secs 2.5");
            let _ = writeln!(help, "  cliip-show --config set max_lines 3");
            let _ = writeln!(help, "  cliip-show --config set hud_position top");
            let _ = writeln!(help, "  cliip-show --config set hud_scale 1.2");
            let _ = writeln!(help, "  cliip-show --config set hud_background_color blue");
            let _ = writeln!(help, "  cliip-show --config set hud_emoji 🍣");
            let _ = writeln!(help);
            let _ = writeln!(help, "Config keys:");
            let _ = writeln!(
                help,
                "  poll_interval_secs      default=0.3 (0.05 - 5.0)  ※ restart required"
            );
            let _ = writeln!(help, "  hud_duration_secs       default=1.0 (0.1 - 10.0)");
            let _ = writeln!(help, "  hud_fade_duration_secs  default=0.3 (0.0 - 2.0)");
            let _ = writeln!(help, "  max_chars_per_line      default=100 (1 - 500)");
            let _ = writeln!(help, "  max_lines               default=5 (1 - 20)");
            let _ = writeln!(
                help,
                "  hud_position            default=top (top|center|bottom)"
            );
            let _ = writeln!(help, "  hud_scale               default=1.1 (0.5 - 2.0)");
            let _ = writeln!(
                help,
                "  hud_background_color    default=default (default|yellow|blue|green|red|purple)"
            );
            let _ = writeln!(
                help,
                "  hud_emoji               default=📋 (任意の文字・絵文字)"
            );
            let _ = writeln!(help);
            let _ = writeln!(
                help,
                "Note: config changes are hot-reloaded automatically (no restart needed),"
            );
            let _ = writeln!(
                help,
                "      except poll_interval_secs which requires a service restart."
            );
            let _ = writeln!(help);
            let _ = writeln!(help, "For Homebrew service:");
            let _ = writeln!(help, "  brew services restart cliip-show");
            let _ = writeln!(help);
            let _ = writeln!(help, "Persistent config file:");
            let _ = writeln!(
                help,
                "  default: ~/Library/Application Support/cliip-show/config.toml"
            );
            let _ = writeln!(help, "  override path via: CLIIP_SHOW_CONFIG_PATH");
            let _ = writeln!(help);
            let _ = writeln!(help, "Display settings via env vars (override file):");
            let _ = writeln!(
                help,
                "  CLIIP_SHOW_POLL_INTERVAL_SECS   Poll interval seconds (0.05 - 5.0)"
            );
            let _ = writeln!(
                help,
                "  CLIIP_SHOW_HUD_DURATION_SECS    HUD visible seconds (0.1 - 10.0)"
            );
            let _ = writeln!(
                help,
                "  CLIIP_SHOW_MAX_CHARS_PER_LINE   Max chars per line (1 - 500)"
            );
            let _ = writeln!(
                help,
                "  CLIIP_SHOW_MAX_LINES            Max lines in HUD (1 - 20)"
            );
            let _ = writeln!(
                help,
                "  CLIIP_SHOW_HUD_POSITION         HUD position (top|center|bottom)"
            );
            let _ = writeln!(
                help,
                "  CLIIP_SHOW_HUD_SCALE            HUD scale (0.5 - 2.0)"
            );
            let _ = writeln!(
                help,
                "  CLIIP_SHOW_HUD_BACKGROUND_COLOR HUD background color (default|yellow|blue|green|red|purple)"
            );
            let _ = writeln!(
                help,
                "  CLIIP_SHOW_HUD_EMOJI            HUD icon emoji (default: 📋)"
            );
            print!("{help}");
            true
        }
        "--config" => handle_config_command(&mut args),
        "--render-hud-png" => {
            let mut text: Option<String> = None;
            let mut output_path: Option<String> = None;

            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--text" => {
                        let Some(value) = args.next() else {
                            eprintln!("Missing value for --text");
                            std::process::exit(2);
                        };
                        text = Some(value);
                    }
                    "--output" => {
                        let Some(value) = args.next() else {
                            eprintln!("Missing value for --output");
                            std::process::exit(2);
                        };
                        output_path = Some(value);
                    }
                    unknown => {
                        eprintln!("Unknown option for --render-hud-png: {unknown}");
                        std::process::exit(2);
                    }
                }
            }

            let text = text.unwrap_or_else(|| "Clipboard text".to_string());
            let Some(output_path) = output_path else {
                eprintln!("--output is required for --render-hud-png");
                std::process::exit(2);
            };

            if let Err(error) = render_hud_png(&text, &output_path) {
                eprintln!("{error}");
                std::process::exit(1);
            }
            true
        }
        "--diff-png" => {
            let mut baseline_path: Option<String> = None;
            let mut current_path: Option<String> = None;
            let mut output_path: Option<String> = None;

            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--baseline" => {
                        let Some(value) = args.next() else {
                            eprintln!("Missing value for --baseline");
                            std::process::exit(2);
                        };
                        baseline_path = Some(value);
                    }
                    "--current" => {
                        let Some(value) = args.next() else {
                            eprintln!("Missing value for --current");
                            std::process::exit(2);
                        };
                        current_path = Some(value);
                    }
                    "--output" => {
                        let Some(value) = args.next() else {
                            eprintln!("Missing value for --output");
                            std::process::exit(2);
                        };
                        output_path = Some(value);
                    }
                    unknown => {
                        eprintln!("Unknown option for --diff-png: {unknown}");
                        std::process::exit(2);
                    }
                }
            }

            let Some(baseline_path) = baseline_path else {
                eprintln!("--baseline is required for --diff-png");
                std::process::exit(2);
            };
            let Some(current_path) = current_path else {
                eprintln!("--current is required for --diff-png");
                std::process::exit(2);
            };
            let Some(output_path) = output_path else {
                eprintln!("--output is required for --diff-png");
                std::process::exit(2);
            };

            match generate_diff_png(&baseline_path, &current_path, &output_path) {
                Ok(summary) => {
                    println!(
                        "diff_pixels={} total_pixels={}",
                        summary.diff_pixels, summary.total_pixels
                    );
                }
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            }
            true
        }
        unknown => {
            eprintln!("Unknown option: {unknown}");
            eprintln!("Use --help to see available options.");
            std::process::exit(2);
        }
    }
}
