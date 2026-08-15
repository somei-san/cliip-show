use std::fmt::Write as _;

use crate::config::show_config;
use crate::png::{generate_diff_png, render_hud_image_png, render_hud_png};

/// `--image-fixture` の `<W>x<H>` をパースする。
fn parse_image_fixture_size(raw: &str) -> Option<(usize, usize)> {
    let (width, height) = raw.trim().split_once(['x', 'X'])?;
    let width = width.trim().parse::<usize>().ok()?;
    let height = height.trim().parse::<usize>().ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    Some((width, height))
}

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
                "  --render-hud-png --image-fixture <W>x<H> --output <PATH>    Render image HUD snapshot PNG and exit"
            );
            let _ = writeln!(
                help,
                "  --diff-png --baseline <PATH> --current <PATH> --output <PATH>    Generate visual diff PNG and exit"
            );
            let _ = writeln!(
                help,
                "  --config-show    Print the current settings and exit"
            );
            let _ = writeln!(help);
            let _ = writeln!(
                help,
                "Settings are edited in the settings window (\"Settings…\" in the menu bar)."
            );
            let _ = writeln!(
                help,
                "Changes are hot-reloaded automatically (no restart needed)."
            );
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
                "  CLIIP_SHOW_HUD_FADE_DURATION_SECS  HUD fade seconds (0.0 - 2.0; 0.0 disables)"
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
            let _ = writeln!(
                help,
                "  CLIIP_SHOW_HUD_IMAGE_MAX_HEIGHT Max thumbnail height for copied images (40 - 240)"
            );
            let _ = writeln!(
                help,
                "  CLIIP_SHOW_LANGUAGE             UI language (auto|ja|en)"
            );
            print!("{help}");
            true
        }
        "--config-show" => show_config(&mut args),
        "--render-hud-png" => {
            let mut text: Option<String> = None;
            let mut image_fixture: Option<(usize, usize)> = None;
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
                    "--image-fixture" => {
                        let Some(value) = args.next() else {
                            eprintln!("Missing value for --image-fixture");
                            std::process::exit(2);
                        };
                        let Some(size) = parse_image_fixture_size(&value) else {
                            eprintln!("Invalid value for --image-fixture: {value} (expected <W>x<H>, e.g. 320x180)");
                            std::process::exit(2);
                        };
                        image_fixture = Some(size);
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

            if text.is_some() && image_fixture.is_some() {
                eprintln!("--text and --image-fixture are mutually exclusive");
                std::process::exit(2);
            }

            let Some(output_path) = output_path else {
                eprintln!("--output is required for --render-hud-png");
                std::process::exit(2);
            };

            let result = match image_fixture {
                Some((width, height)) => render_hud_image_png(width, height, &output_path),
                None => {
                    let text = text.unwrap_or_else(|| "Clipboard text".to_string());
                    render_hud_png(&text, &output_path)
                }
            };
            if let Err(error) = result {
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

#[cfg(test)]
mod tests {
    use super::parse_image_fixture_size;

    #[test]
    fn parse_image_fixture_size_accepts_valid_sizes() {
        assert_eq!(parse_image_fixture_size("320x180"), Some((320, 180)));
        assert_eq!(parse_image_fixture_size(" 16X16 "), Some((16, 16)));
    }

    #[test]
    fn parse_image_fixture_size_rejects_invalid_sizes() {
        assert_eq!(parse_image_fixture_size("320"), None);
        assert_eq!(parse_image_fixture_size("320x"), None);
        assert_eq!(parse_image_fixture_size("0x180"), None);
        assert_eq!(parse_image_fixture_size("320x0"), None);
        assert_eq!(parse_image_fixture_size("-1x10"), None);
        assert_eq!(parse_image_fixture_size("axb"), None);
    }
}
