#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

usage() {
  cat <<'EOF'
Usage: ./scripts/local_check.sh [options]

Options:
  --position <top|center|bottom>                      HUD position (optional; default uses app config default)
  --scale <0.5-2.0>                                   HUD scale (optional; default uses app config default)
  --color <default|yellow|blue|green|red|purple>      HUD background color (optional; default uses app config default)
  --text <TEXT>                                        Clipboard text to copy after startup
  --config-path <PATH>                                 Temp config path (default: /tmp/cliip-show-local-check.toml)
  --no-stop-installed                                  Leave the installed cliip-show running (HUD may appear twice)
  --no-build                                           Skip `cargo build`
  --no-copy                                            Do not auto-copy test text
  -h, --help                                           Show this help

Environment overrides:
  LOCAL_CHECK_POSITION
  LOCAL_CHECK_SCALE
  LOCAL_CHECK_COLOR
  LOCAL_CHECK_TEXT
  LOCAL_CHECK_CONFIG_PATH
EOF
}

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "local_check requires macOS" >&2
  exit 1
fi

for required in cargo pbcopy; do
  if ! command -v "$required" >/dev/null 2>&1; then
    echo "required command not found: $required" >&2
    exit 1
  fi
done

POSITION=""
SCALE=""
COLOR=""
POSITION_EXPLICIT=false
SCALE_EXPLICIT=false
COLOR_EXPLICIT=false
TEXT="${LOCAL_CHECK_TEXT:-}"
TEXT_EXPLICIT=false
CONFIG_PATH="${LOCAL_CHECK_CONFIG_PATH:-/tmp/cliip-show-local-check.toml}"
STOP_INSTALLED=true
DO_BUILD=true
AUTO_COPY=true

if [[ -n "${LOCAL_CHECK_POSITION:-}" ]]; then
  POSITION="${LOCAL_CHECK_POSITION}"
  POSITION_EXPLICIT=true
fi
if [[ -n "${LOCAL_CHECK_SCALE:-}" ]]; then
  SCALE="${LOCAL_CHECK_SCALE}"
  SCALE_EXPLICIT=true
fi
if [[ -n "${LOCAL_CHECK_COLOR:-}" ]]; then
  COLOR="${LOCAL_CHECK_COLOR}"
  COLOR_EXPLICIT=true
fi

while [[ $# -gt 0 ]]; do
  case "$1" in
    --position)
      [[ $# -ge 2 ]] || { echo "missing value for --position" >&2; exit 2; }
      POSITION="$2"
      POSITION_EXPLICIT=true
      shift 2
      ;;
    --scale)
      [[ $# -ge 2 ]] || { echo "missing value for --scale" >&2; exit 2; }
      SCALE="$2"
      SCALE_EXPLICIT=true
      shift 2
      ;;
    --color)
      [[ $# -ge 2 ]] || { echo "missing value for --color" >&2; exit 2; }
      COLOR="$2"
      COLOR_EXPLICIT=true
      shift 2
      ;;
    --text)
      [[ $# -ge 2 ]] || { echo "missing value for --text" >&2; exit 2; }
      TEXT="$2"
      TEXT_EXPLICIT=true
      shift 2
      ;;
    --config-path)
      [[ $# -ge 2 ]] || { echo "missing value for --config-path" >&2; exit 2; }
      CONFIG_PATH="$2"
      shift 2
      ;;
    --no-stop-installed)
      STOP_INSTALLED=false
      shift
      ;;
    --no-build)
      DO_BUILD=false
      shift
      ;;
    --no-copy)
      AUTO_COPY=false
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if $POSITION_EXPLICIT; then
  case "$POSITION" in
    top|center|bottom) ;;
    *)
      echo "invalid --position: $POSITION (allowed: top, center, bottom)" >&2
      exit 2
      ;;
  esac
fi

if $COLOR_EXPLICIT; then
  case "$COLOR" in
    default|yellow|blue|green|red|purple) ;;
    *)
      echo "invalid --color: $COLOR (allowed: default, yellow, blue, green, red, purple)" >&2
      exit 2
      ;;
  esac
fi

if $SCALE_EXPLICIT; then
  if [[ ! "$SCALE" =~ ^[0-9]+([.][0-9]+)?$ ]] || ! awk -v s="$SCALE" 'BEGIN { exit !(s >= 0.5 && s <= 2.0) }'; then
    echo "invalid --scale: $SCALE (allowed range: 0.5 - 2.0)" >&2
    exit 2
  fi
fi

if ! $TEXT_EXPLICIT; then
  if $POSITION_EXPLICIT || $SCALE_EXPLICIT || $COLOR_EXPLICIT; then
    local_position="${POSITION:-app-default}"
    local_scale="${SCALE:-app-default}"
    local_color="${COLOR:-app-default}"
    TEXT="local check: ${local_position}/${local_scale}/${local_color}"
  else
    TEXT="local check: default settings"
  fi
fi

# 常駐中のインストール済みインスタンスを止める。HUD が二重に出ると、どちらの見た目を
# 見ているのか分からなくなるため。止めた場合は終了時に元の起動状態へ戻す。
LOGIN_ITEM_LABEL="io.github.somei-san.cliip-show"
LOGIN_ITEM_PLIST="$HOME/Library/LaunchAgents/$LOGIN_ITEM_LABEL.plist"
LOGIN_ITEM_DOMAIN="gui/$(id -u)"
INSTALLED_BIN="$(command -v cliip-show 2>/dev/null || true)"
RESTORE_LOGIN_ITEM=false
RESTORE_INSTALLED_BIN=false

if $STOP_INSTALLED; then
  if launchctl print "$LOGIN_ITEM_DOMAIN/$LOGIN_ITEM_LABEL" >/dev/null 2>&1; then
    echo "[local_check] stopping login item: $LOGIN_ITEM_LABEL"
    launchctl bootout "$LOGIN_ITEM_DOMAIN/$LOGIN_ITEM_LABEL" >/dev/null 2>&1 || true
    RESTORE_LOGIN_ITEM=true
  elif [[ -n "$INSTALLED_BIN" ]] && pgrep -xf "$INSTALLED_BIN" >/dev/null 2>&1; then
    echo "[local_check] stopping running instance: $INSTALLED_BIN"
    pkill -xf "$INSTALLED_BIN" >/dev/null 2>&1 || true
    RESTORE_INSTALLED_BIN=true
  fi
fi

restore_installed() {
  if $RESTORE_LOGIN_ITEM && [[ -f "$LOGIN_ITEM_PLIST" ]]; then
    echo "[local_check] restoring login item: $LOGIN_ITEM_LABEL"
    launchctl bootstrap "$LOGIN_ITEM_DOMAIN" "$LOGIN_ITEM_PLIST" >/dev/null 2>&1 || true
  elif $RESTORE_INSTALLED_BIN && [[ -n "$INSTALLED_BIN" ]]; then
    echo "[local_check] restarting: $INSTALLED_BIN"
    "$INSTALLED_BIN" >/dev/null 2>&1 &
  fi
}

if $DO_BUILD; then
  echo "[local_check] cargo build"
  cargo build >/dev/null
fi

BIN="$ROOT_DIR/target/debug/cliip-show"
if [[ ! -x "$BIN" ]]; then
  echo "binary not found: $BIN (run cargo build or remove --no-build)" >&2
  exit 1
fi

echo "[local_check] config path: $CONFIG_PATH"
# 上書きは環境変数ではなく設定ファイルに書く。環境変数は設定ファイルより優先されるため、
# 設定ウィンドウで保存してもウィンドウを閉じた時点で環境変数の値に戻り、ウィンドウの
# 動作確認ができなくなる。書かなかった項目はアプリの既定値になる。
printf '[display]\n' > "$CONFIG_PATH"
if $POSITION_EXPLICIT; then
  printf 'hud_position = "%s"\n' "$POSITION" >> "$CONFIG_PATH"
fi
if $SCALE_EXPLICIT; then
  printf 'hud_scale = %s\n' "$SCALE" >> "$CONFIG_PATH"
fi
if $COLOR_EXPLICIT; then
  printf 'hud_background_color = "%s"\n' "$COLOR" >> "$CONFIG_PATH"
fi
if ! $POSITION_EXPLICIT && ! $SCALE_EXPLICIT && ! $COLOR_EXPLICIT; then
  echo "[local_check] using app default display settings (no overrides)"
fi
CLIIP_SHOW_CONFIG_PATH="$CONFIG_PATH" "$BIN" --config-show

APP_PID=""
# trap は TERM で発火したあと EXIT でも発火するため、復帰処理が二度走らないよう塞ぐ。
# 二度走るとインストール済みインスタンスが二重起動する。
CLEANUP_DONE=false
cleanup() {
  if $CLEANUP_DONE; then
    return
  fi
  CLEANUP_DONE=true
  if [[ -n "$APP_PID" ]]; then
    kill "$APP_PID" >/dev/null 2>&1 || true
  fi
  restore_installed
}
trap cleanup EXIT INT TERM

# 設定ウィンドウの自動起動トグルだけは $CONFIG_PATH ではなく $HOME の LaunchAgent を読み書きする。
# ここで触ると開発ビルドのパスを指す plist が書かれ、cargo clean でインストール済みアプリの
# 自動起動ごと壊れる。
echo "[local_check] warning: do not touch the login item toggle; it overwrites the installed app's LaunchAgent"
echo "[local_check] starting cliip-show (Ctrl+C to stop)"
CLIIP_SHOW_CONFIG_PATH="$CONFIG_PATH" "$BIN" &
APP_PID="$!"
sleep 1

if $AUTO_COPY; then
  printf '%s' "$TEXT" | pbcopy
  echo "[local_check] copied text to clipboard: $TEXT"
else
  echo "[local_check] auto-copy disabled (--no-copy)"
fi

wait "$APP_PID"
