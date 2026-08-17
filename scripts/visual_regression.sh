#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "visual regression requires macOS" >&2
  exit 1
fi

UPDATE=false
if [[ "${1:-}" == "--update" ]]; then
  UPDATE=true
elif [[ $# -gt 0 ]]; then
  echo "Usage: $0 [--update]" >&2
  exit 2
fi

BASELINE_DIR="$ROOT_DIR/tests/visual/baseline"
ARTIFACT_DIR="$ROOT_DIR/tests/visual/artifacts"
mkdir -p "$BASELINE_DIR" "$ARTIFACT_DIR"
rm -f "$ARTIFACT_DIR"/*.png

cargo build >/dev/null
BIN="$ROOT_DIR/target/debug/cliip-show"
MAX_DIFF_PERMILLE="${MAX_DIFF_PERMILLE:-120}" # 120/1000 = 12%
# 設定ウィンドウのペインは HUD より差分の面積比が小さい（ja/en の文言全取り替えでも
# 実測 40〜65‰）ため、HUD 用の許容値では文言の退行を検出できない。別枠で厳しくする
SETTINGS_MAX_DIFF_PERMILLE="${SETTINGS_MAX_DIFF_PERMILLE:-30}"
VRT_CONFIG_PATH="$ARTIFACT_DIR/vrt-config.toml"
rm -f "$VRT_CONFIG_PATH"

failed=0

render_and_compare() {
  local id="$1"
  local mode_flag="$2"
  local content_flag="$3"
  local content_value="$4"
  shift 4

  local current="$ARTIFACT_DIR/${id}.current.png"
  local baseline="$BASELINE_DIR/${id}.png"
  local diff="$ARTIFACT_DIR/${id}.diff.png"

  local -a cmd=(
    env
    -u CLIIP_SHOW_CONFIG_PATH
    -u CLIIP_SHOW_POLL_INTERVAL_SECS
    -u CLIIP_SHOW_HUD_DURATION_SECS
    -u CLIIP_SHOW_MAX_CHARS_PER_LINE
    -u CLIIP_SHOW_MAX_LINES
    -u CLIIP_SHOW_HUD_POSITION
    -u CLIIP_SHOW_HUD_SCALE
    -u CLIIP_SHOW_HUD_BACKGROUND_COLOR
    -u CLIIP_SHOW_HUD_IMAGE_MAX_HEIGHT
    -u CLIIP_SHOW_HUD_EMOJI
    -u CLIIP_SHOW_LANGUAGE
    "CLIIP_SHOW_CONFIG_PATH=$VRT_CONFIG_PATH"
  )
  if [[ $# -gt 0 ]]; then
    local override
    for override in "$@"; do
      if [[ ! "$override" =~ ^[A-Za-z_][A-Za-z0-9_]*=.*$ ]]; then
        echo "invalid env override for $id: $override (expected KEY=VALUE)" >&2
        exit 2
      fi
      cmd+=("$override")
    done
  fi
  cmd+=("$BIN" "$mode_flag" "$content_flag" "$content_value" --output "$current")
  "${cmd[@]}"

  if $UPDATE; then
    cp "$current" "$baseline"
    rm -f "$diff"
    echo "updated: $baseline"
    return
  fi

  if [[ ! -f "$baseline" ]]; then
    echo "missing baseline: $baseline (run ./scripts/visual_regression.sh --update once)" >&2
    failed=1
    return
  fi

  if diff_output=$("$BIN" --diff-png --baseline "$baseline" --current "$current" --output "$diff" 2>&1); then
    diff_pixels="$(echo "$diff_output" | sed -n 's/.*diff_pixels=\([0-9][0-9]*\).*/\1/p' | tail -n1)"
    total_pixels="$(echo "$diff_output" | sed -n 's/.*total_pixels=\([0-9][0-9]*\).*/\1/p' | tail -n1)"
    if [[ -z "$diff_pixels" || -z "$total_pixels" ]]; then
      echo "ng: $id" >&2
      echo "  baseline: $baseline" >&2
      echo "  current : $current" >&2
      echo "  diff    : failed to parse diff output" >&2
      echo "  reason  : $diff_output" >&2
      failed=1
      return
    fi
  else
    echo "ng: $id" >&2
    echo "  baseline: $baseline" >&2
    echo "  current : $current" >&2
    echo "  diff    : failed to generate" >&2
    if [[ -n "$diff_output" ]]; then
      echo "  reason  : $diff_output" >&2
    fi
    failed=1
    return
  fi

  local tolerance="${CASE_MAX_DIFF_PERMILLE:-$MAX_DIFF_PERMILLE}"
  if [[ "$diff_pixels" -eq 0 ]]; then
    rm -f "$diff"
    echo "ok: $id"
  else
    if (( diff_pixels * 1000 <= total_pixels * tolerance )); then
      echo "ok: $id (within tolerance ${diff_pixels}/${total_pixels}, max=${tolerance}/1000)"
      rm -f "$diff"
    else
      echo "ng: $id" >&2
      echo "  baseline: $baseline" >&2
      echo "  current : $current" >&2
      echo "  diff    : $diff" >&2
      echo "  pixels  : ${diff_pixels}/${total_pixels} (max=${tolerance}/1000)" >&2
      failed=1
    fi
  fi
}

run_case() {
  local id="$1"
  local text="$2"
  shift 2
  render_and_compare "$id" --render-hud-png --text "$text" "$@"
}

# サイズは <W>x<H>。バイナリ側が決定的な市松模様を生成するのでフィクスチャ画像ファイルは不要
run_image_case() {
  local id="$1"
  local size="$2"
  shift 2
  render_and_compare "$id" --render-hud-png --image-fixture "$size" "$@"
}

# 設定ウィンドウのペイン（settings | support）。タブバーは contentView の外なので写らない。
# 言語は必ず CLIIP_SHOW_LANGUAGE で固定して呼ぶこと（auto は実行マシンの言語に依存する）
run_settings_case() {
  local id="$1"
  local pane="$2"
  shift 2
  CASE_MAX_DIFF_PERMILLE="$SETTINGS_MAX_DIFF_PERMILLE" \
    render_and_compare "$id" --render-settings-png --pane "$pane" "$@"
}

run_case \
  "ascii_short" \
  "hello clipboard"

run_case \
  "ascii_long" \
  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

run_case \
  "wide_text" \
  "日本語のコピー内容です"

run_case \
  "multiline" \
  $'line1\nline2\nline3'

# Settings profile: max_lines=2
run_case \
  "setting_max_lines_2_multiline" \
  $'line1\nline2\nline3\nline4' \
  "CLIIP_SHOW_MAX_LINES=2"

# Settings profile: max_chars_per_line=24
run_case \
  "setting_max_chars_24_ascii_long" \
  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" \
  "CLIIP_SHOW_MAX_CHARS_PER_LINE=24"

# Settings profile: max_chars_per_line=16 and max_lines=2
run_case \
  "setting_compact_text_block" \
  $'abcdefghijklmnopqrstuvwxyz\nabcdefghijklmnopqrstuvwxyz\nabcdefghijklmnopqrstuvwxyz' \
  "CLIIP_SHOW_MAX_CHARS_PER_LINE=16" \
  "CLIIP_SHOW_MAX_LINES=2"

# Settings profile: hud_position=top (position is validated by unit tests;
# render-hud-png snapshots only HUD content, not screen coordinates)
run_case \
  "setting_hud_position_top" \
  "hello clipboard" \
  "CLIIP_SHOW_HUD_POSITION=top"

# Settings profile: hud_scale variants
run_case \
  "setting_hud_scale_08" \
  "hello clipboard" \
  "CLIIP_SHOW_HUD_SCALE=0.8"

run_case \
  "setting_hud_scale_15" \
  "hello clipboard" \
  "CLIIP_SHOW_HUD_SCALE=1.5"

run_case \
  "setting_hud_scale_20" \
  "hello clipboard" \
  "CLIIP_SHOW_HUD_SCALE=2.0"

# Settings profile: hud_background_color variants
run_case \
  "setting_hud_background_color_default" \
  "hello clipboard" \
  "CLIIP_SHOW_HUD_BACKGROUND_COLOR=default"

run_case \
  "setting_hud_background_color_yellow" \
  "hello clipboard" \
  "CLIIP_SHOW_HUD_BACKGROUND_COLOR=yellow"

run_case \
  "setting_hud_background_color_blue" \
  "hello clipboard" \
  "CLIIP_SHOW_HUD_BACKGROUND_COLOR=blue"

run_case \
  "setting_hud_background_color_green" \
  "hello clipboard" \
  "CLIIP_SHOW_HUD_BACKGROUND_COLOR=green"

run_case \
  "setting_hud_background_color_red" \
  "hello clipboard" \
  "CLIIP_SHOW_HUD_BACKGROUND_COLOR=red"

run_case \
  "setting_hud_background_color_purple" \
  "hello clipboard" \
  "CLIIP_SHOW_HUD_BACKGROUND_COLOR=purple"

# Layout stress: single line exceeding default max_chars_per_line=100 (truncated with ellipsis)
run_case \
  "over_max_chars" \
  "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz"

# Layout stress: multiple lines each exceeding max_chars_per_line, plus line count overflow
run_case \
  "over_max_chars_and_lines" \
  $'abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz0123456789\nabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz0123456789\nabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz0123456789\nabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz0123456789\nabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz0123456789\nabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz0123456789'

# Layout stress: no-space text exceeding max_chars_per_line (char-wrap + truncation)
run_case \
  "no_space_over_max_chars" \
  "aVeryLongIdentifierNameThatDefinitelyExceedsOneHundredCharactersLimitSetByTheDefaultConfigurationValue"

# Layout stress: single character (minimum HUD size)
run_case \
  "single_char" \
  "x"

# Layout stress: long text without any spaces (char-wrap must kick in, not clip)
run_case \
  "no_space_url" \
  "https://example.com/very/long/path/with/many/segments/that/goes/on/and/on/without/spaces"

# Layout stress: long camelCase token without spaces (code-like, no word boundaries)
run_case \
  "no_space_code" \
  "aVeryLongVariableNameThatExceedsTheMaxWidthOfTheHudDisplayWithoutAnySpacesAtAll"

# Layout stress: many short lines (width at minimum, height grows tall)
run_case \
  "many_short_lines" \
  $'a\nb\nc\nd\ne'

# Layout stress: line count overflow beyond default max_lines (truncation + ellipsis)
run_case \
  "many_lines_overflow" \
  $'line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8'

# Layout stress: long CJK without spaces (full-width char-wrap)
run_case \
  "cjk_long_no_space" \
  "あいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほまみむめも"

# Layout stress: mixed CJK and ASCII across multiple lines (uneven line widths)
run_case \
  "mixed_cjk_ascii_multiline" \
  $'Hello 世界\n日本語 text\nこんにちは World'

# Image: landscape thumbnail (height-bound)
run_image_case \
  "image_landscape" \
  "320x180"

# Image: portrait thumbnail (height-bound, HUD width stays at minimum)
run_image_case \
  "image_portrait" \
  "180x320"

# Image: smaller than the thumbnail box (must not be upscaled)
run_image_case \
  "image_tiny" \
  "16x16"

# Image: far larger than the thumbnail box (height clamp)
run_image_case \
  "image_over_max_height" \
  "4000x3000"

# Settings profile: hud_image_max_height=80
run_image_case \
  "setting_hud_image_max_height_80" \
  "320x180" \
  "CLIIP_SHOW_HUD_IMAGE_MAX_HEIGHT=80"

# Settings profile: hud_scale=2.0 with an image (thumbnail box must scale with the HUD)
run_image_case \
  "setting_hud_scale_20_image" \
  "320x180" \
  "CLIIP_SHOW_HUD_SCALE=2.0"

# Settings profile: hud_background_color with an image (background layer behind the thumbnail)
run_image_case \
  "setting_hud_background_color_green_image" \
  "320x180" \
  "CLIIP_SHOW_HUD_BACKGROUND_COLOR=green"

# Settings profile: hud_emoji="" (no icon, text) — icon column and its gap are removed
run_case \
  "no_emoji_short_text" \
  "hello clipboard" \
  "CLIIP_SHOW_HUD_EMOJI="

# Settings profile: hud_emoji="" (no icon, image) — icon column and its gap are removed
run_image_case \
  "no_emoji_image" \
  "320x180" \
  "CLIIP_SHOW_HUD_EMOJI="

# Settings window panes (row layout, control placement, localized labels)
run_settings_case \
  "settings_pane_ja" \
  "settings" \
  "CLIIP_SHOW_LANGUAGE=ja"

run_settings_case \
  "settings_pane_en" \
  "settings" \
  "CLIIP_SHOW_LANGUAGE=en"

run_settings_case \
  "support_pane_ja" \
  "support" \
  "CLIIP_SHOW_LANGUAGE=ja"

run_settings_case \
  "support_pane_en" \
  "support" \
  "CLIIP_SHOW_LANGUAGE=en"

if $UPDATE; then
  echo "visual regression baseline updated"
  exit 0
fi

if [[ "$failed" -ne 0 ]]; then
  exit 1
fi

echo "visual regression passed"
