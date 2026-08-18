use std::fmt::Write as _;

use crate::config::show_config;
use crate::png::{
    generate_diff_png, render_hud_image_png, render_hud_png, render_settings_png, SettingsPane,
};

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

/// `--render-hud-png` で HUD に載せる内容。
#[derive(Debug, PartialEq, Eq)]
enum RenderContent {
    Text(String),
    ImageFixture { width: usize, height: usize },
}

#[derive(Debug, PartialEq, Eq)]
struct RenderHudArgs {
    content: RenderContent,
    output_path: String,
}

/// `--render-hud-png` に続くオプションを解析する。Err はそのまま stderr に出す
/// usage エラーメッセージ（呼び出し元が exit code 2 で終了する）。
fn parse_render_hud_args(args: impl Iterator<Item = String>) -> Result<RenderHudArgs, String> {
    let mut args = args;
    let mut text: Option<String> = None;
    let mut image_fixture: Option<(usize, usize)> = None;
    let mut output_path: Option<String> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--text" => {
                let Some(value) = args.next() else {
                    return Err("Missing value for --text".to_string());
                };
                text = Some(value);
            }
            "--image-fixture" => {
                let Some(value) = args.next() else {
                    return Err("Missing value for --image-fixture".to_string());
                };
                let Some(size) = parse_image_fixture_size(&value) else {
                    return Err(format!(
                        "Invalid value for --image-fixture: {value} (expected <W>x<H>, e.g. 320x180)"
                    ));
                };
                image_fixture = Some(size);
            }
            "--output" => {
                let Some(value) = args.next() else {
                    return Err("Missing value for --output".to_string());
                };
                output_path = Some(value);
            }
            unknown => {
                return Err(format!("Unknown option for --render-hud-png: {unknown}"));
            }
        }
    }

    if text.is_some() && image_fixture.is_some() {
        return Err("--text and --image-fixture are mutually exclusive".to_string());
    }

    let Some(output_path) = output_path else {
        return Err("--output is required for --render-hud-png".to_string());
    };

    let content = match image_fixture {
        Some((width, height)) => RenderContent::ImageFixture { width, height },
        None => RenderContent::Text(text.unwrap_or_else(|| "Clipboard text".to_string())),
    };
    Ok(RenderHudArgs {
        content,
        output_path,
    })
}

#[derive(Debug, PartialEq, Eq)]
struct SettingsPngArgs {
    pane: SettingsPane,
    output_path: String,
}

/// `--render-settings-png` に続くオプションを解析する。Err の扱いは `parse_render_hud_args` と同じ。
fn parse_settings_png_args(args: impl Iterator<Item = String>) -> Result<SettingsPngArgs, String> {
    let mut args = args;
    let mut pane: Option<SettingsPane> = None;
    let mut output_path: Option<String> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--pane" => {
                let Some(value) = args.next() else {
                    return Err("Missing value for --pane".to_string());
                };
                pane = Some(match value.as_str() {
                    "settings" => SettingsPane::Settings,
                    "support" => SettingsPane::Support,
                    unknown => {
                        return Err(format!(
                            "Invalid value for --pane: {unknown} (expected settings or support)"
                        ));
                    }
                });
            }
            "--output" => {
                let Some(value) = args.next() else {
                    return Err("Missing value for --output".to_string());
                };
                output_path = Some(value);
            }
            unknown => {
                return Err(format!(
                    "Unknown option for --render-settings-png: {unknown}"
                ));
            }
        }
    }

    let Some(pane) = pane else {
        return Err("--pane is required for --render-settings-png".to_string());
    };
    let Some(output_path) = output_path else {
        return Err("--output is required for --render-settings-png".to_string());
    };

    Ok(SettingsPngArgs { pane, output_path })
}

#[derive(Debug, PartialEq, Eq)]
struct DiffPngArgs {
    baseline_path: String,
    current_path: String,
    output_path: String,
}

/// `--diff-png` に続くオプションを解析する。Err の扱いは `parse_render_hud_args` と同じ。
fn parse_diff_png_args(args: impl Iterator<Item = String>) -> Result<DiffPngArgs, String> {
    let mut args = args;
    let mut baseline_path: Option<String> = None;
    let mut current_path: Option<String> = None;
    let mut output_path: Option<String> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--baseline" => {
                let Some(value) = args.next() else {
                    return Err("Missing value for --baseline".to_string());
                };
                baseline_path = Some(value);
            }
            "--current" => {
                let Some(value) = args.next() else {
                    return Err("Missing value for --current".to_string());
                };
                current_path = Some(value);
            }
            "--output" => {
                let Some(value) = args.next() else {
                    return Err("Missing value for --output".to_string());
                };
                output_path = Some(value);
            }
            unknown => {
                return Err(format!("Unknown option for --diff-png: {unknown}"));
            }
        }
    }

    let Some(baseline_path) = baseline_path else {
        return Err("--baseline is required for --diff-png".to_string());
    };
    let Some(current_path) = current_path else {
        return Err("--current is required for --diff-png".to_string());
    };
    let Some(output_path) = output_path else {
        return Err("--output is required for --diff-png".to_string());
    };

    Ok(DiffPngArgs {
        baseline_path,
        current_path,
        output_path,
    })
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
                "  --render-settings-png --pane <settings|support> --output <PATH>    Render settings window pane PNG and exit"
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
                "  CLIIP_SHOW_HUD_BACKGROUND_OPACITY  HUD background opacity (0.2 - 1.0)"
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
            let parsed = match parse_render_hud_args(args) {
                Ok(parsed) => parsed,
                Err(message) => {
                    eprintln!("{message}");
                    std::process::exit(2);
                }
            };

            let result = match parsed.content {
                RenderContent::ImageFixture { width, height } => {
                    render_hud_image_png(width, height, &parsed.output_path)
                }
                RenderContent::Text(text) => render_hud_png(&text, &parsed.output_path),
            };
            if let Err(error) = result {
                eprintln!("{error}");
                std::process::exit(1);
            }
            true
        }
        "--render-settings-png" => {
            let parsed = match parse_settings_png_args(args) {
                Ok(parsed) => parsed,
                Err(message) => {
                    eprintln!("{message}");
                    std::process::exit(2);
                }
            };

            if let Err(error) = render_settings_png(parsed.pane, &parsed.output_path) {
                eprintln!("{error}");
                std::process::exit(1);
            }
            true
        }
        "--diff-png" => {
            let parsed = match parse_diff_png_args(args) {
                Ok(parsed) => parsed,
                Err(message) => {
                    eprintln!("{message}");
                    std::process::exit(2);
                }
            };

            match generate_diff_png(
                &parsed.baseline_path,
                &parsed.current_path,
                &parsed.output_path,
            ) {
                Ok(summary) => {
                    // VRT スクリプトがこの stdout 形式をパースしている（変えるならスクリプトも同時に）
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
    use super::{
        parse_diff_png_args, parse_image_fixture_size, parse_render_hud_args,
        parse_settings_png_args, DiffPngArgs, RenderContent, RenderHudArgs, SettingsPane,
        SettingsPngArgs,
    };

    fn args(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn render_args_accept_text_and_output() {
        let parsed = parse_render_hud_args(args(&["--text", "hello", "--output", "/tmp/out.png"]));
        assert_eq!(
            parsed,
            Ok(RenderHudArgs {
                content: RenderContent::Text("hello".to_string()),
                output_path: "/tmp/out.png".to_string(),
            })
        );
    }

    #[test]
    fn render_args_accept_image_fixture() {
        let parsed = parse_render_hud_args(args(&[
            "--image-fixture",
            "320x180",
            "--output",
            "/tmp/o.png",
        ]));
        assert_eq!(
            parsed,
            Ok(RenderHudArgs {
                content: RenderContent::ImageFixture {
                    width: 320,
                    height: 180
                },
                output_path: "/tmp/o.png".to_string(),
            })
        );
    }

    /// `--text` 省略時はプレースホルダ文言で描画する（既定値も外部挙動なので固定する）。
    #[test]
    fn render_args_default_to_placeholder_text() {
        let parsed = parse_render_hud_args(args(&["--output", "/tmp/out.png"]));
        assert_eq!(
            parsed.map(|p| p.content),
            Ok(RenderContent::Text("Clipboard text".to_string()))
        );
    }

    /// `--output` も欠けた状態で両指定した場合、必須チェックより排他チェックが先に報告されること
    /// （エラーの判定順序の固定）。
    #[test]
    fn render_args_reject_text_with_image_fixture() {
        let parsed =
            parse_render_hud_args(args(&["--text", "hello", "--image-fixture", "320x180"]));
        assert_eq!(
            parsed,
            Err("--text and --image-fixture are mutually exclusive".to_string())
        );
    }

    /// ループ内のエラー（unknown option）は排他チェックより先に報告されること。
    #[test]
    fn render_args_report_loop_errors_before_exclusivity() {
        let parsed = parse_render_hud_args(args(&[
            "--text",
            "hello",
            "--image-fixture",
            "320x180",
            "--bogus",
        ]));
        assert_eq!(
            parsed,
            Err("Unknown option for --render-hud-png: --bogus".to_string())
        );
    }

    #[test]
    fn render_args_require_output() {
        let parsed = parse_render_hud_args(args(&["--text", "hello"]));
        assert_eq!(
            parsed,
            Err("--output is required for --render-hud-png".to_string())
        );
    }

    #[test]
    fn render_args_reject_unknown_options_and_missing_values() {
        assert_eq!(
            parse_render_hud_args(args(&["--textt", "hello"])),
            Err("Unknown option for --render-hud-png: --textt".to_string())
        );
        assert_eq!(
            parse_render_hud_args(args(&["--text"])),
            Err("Missing value for --text".to_string())
        );
        assert_eq!(
            parse_render_hud_args(args(&["--image-fixture"])),
            Err("Missing value for --image-fixture".to_string())
        );
        assert_eq!(
            parse_render_hud_args(args(&["--output"])),
            Err("Missing value for --output".to_string())
        );
        assert_eq!(
            parse_render_hud_args(args(&["--image-fixture", "320"])),
            Err(
                "Invalid value for --image-fixture: 320 (expected <W>x<H>, e.g. 320x180)"
                    .to_string()
            )
        );
    }

    /// オプション直後の値はフラグ風の文字列でもそのまま値として取り込む（現行契約の固定）。
    /// `--text --output /x.png` は text="--output" になり、続く /x.png が unknown 扱いになる。
    #[test]
    fn render_args_take_the_next_token_verbatim_as_a_value() {
        let parsed = parse_render_hud_args(args(&["--text", "--output", "/tmp/x.png"]));
        assert_eq!(
            parsed,
            Err("Unknown option for --render-hud-png: /tmp/x.png".to_string())
        );
    }

    #[test]
    fn diff_args_accept_all_three_paths() {
        let parsed = parse_diff_png_args(args(&[
            "--baseline",
            "/tmp/b.png",
            "--current",
            "/tmp/c.png",
            "--output",
            "/tmp/d.png",
        ]));
        assert_eq!(
            parsed,
            Ok(DiffPngArgs {
                baseline_path: "/tmp/b.png".to_string(),
                current_path: "/tmp/c.png".to_string(),
                output_path: "/tmp/d.png".to_string(),
            })
        );
    }

    /// 3 つの必須オプションはどれが欠けてもエラーになり、欠けたものが名指しされること。
    /// 全部欠けたときは baseline → current → output の順で先頭のものが報告される。
    #[test]
    fn diff_args_require_each_path() {
        assert_eq!(
            parse_diff_png_args(args(&[])),
            Err("--baseline is required for --diff-png".to_string())
        );
        assert_eq!(
            parse_diff_png_args(args(&["--current", "/tmp/c.png", "--output", "/tmp/d.png"])),
            Err("--baseline is required for --diff-png".to_string())
        );
        assert_eq!(
            parse_diff_png_args(args(&[
                "--baseline",
                "/tmp/b.png",
                "--output",
                "/tmp/d.png"
            ])),
            Err("--current is required for --diff-png".to_string())
        );
        assert_eq!(
            parse_diff_png_args(args(&[
                "--baseline",
                "/tmp/b.png",
                "--current",
                "/tmp/c.png"
            ])),
            Err("--output is required for --diff-png".to_string())
        );
    }

    #[test]
    fn diff_args_reject_unknown_options() {
        assert_eq!(
            parse_diff_png_args(args(&["--base", "/tmp/b.png"])),
            Err("Unknown option for --diff-png: --base".to_string())
        );
    }

    #[test]
    fn diff_args_reject_missing_values() {
        assert_eq!(
            parse_diff_png_args(args(&["--baseline"])),
            Err("Missing value for --baseline".to_string())
        );
        assert_eq!(
            parse_diff_png_args(args(&["--current"])),
            Err("Missing value for --current".to_string())
        );
        assert_eq!(
            parse_diff_png_args(args(&["--output"])),
            Err("Missing value for --output".to_string())
        );
    }

    #[test]
    fn settings_args_accept_pane_and_output() {
        assert_eq!(
            parse_settings_png_args(args(&["--pane", "support", "--output", "/tmp/s.png"])),
            Ok(SettingsPngArgs {
                pane: SettingsPane::Support,
                output_path: "/tmp/s.png".to_string(),
            })
        );
        assert_eq!(
            parse_settings_png_args(args(&["--pane", "settings", "--output", "/tmp/s.png"]))
                .map(|parsed| parsed.pane),
            Ok(SettingsPane::Settings)
        );
    }

    #[test]
    fn settings_args_reject_invalid_or_missing_options() {
        assert_eq!(
            parse_settings_png_args(args(&["--pane", "about", "--output", "/tmp/s.png"])),
            Err("Invalid value for --pane: about (expected settings or support)".to_string())
        );
        assert_eq!(
            parse_settings_png_args(args(&["--output", "/tmp/s.png"])),
            Err("--pane is required for --render-settings-png".to_string())
        );
        assert_eq!(
            parse_settings_png_args(args(&["--pane", "settings"])),
            Err("--output is required for --render-settings-png".to_string())
        );
        assert_eq!(
            parse_settings_png_args(args(&["--panes", "settings"])),
            Err("Unknown option for --render-settings-png: --panes".to_string())
        );
    }

    #[test]
    fn settings_args_reject_missing_values() {
        assert_eq!(
            parse_settings_png_args(args(&["--pane"])),
            Err("Missing value for --pane".to_string())
        );
        assert_eq!(
            parse_settings_png_args(args(&["--output"])),
            Err("Missing value for --output".to_string())
        );
    }

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
