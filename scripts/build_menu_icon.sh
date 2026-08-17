#!/usr/bin/env bash
# assets/*-template.svg から、テンプレート画像用の PNG を生成する。
# 生成物は src/menu.rs と src/settings_window/build.rs が include_bytes! で埋め込む。
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "このスクリプトは macOS でのみ動作します" >&2
  exit 1
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

shopt -s nullglob
SVGS=("$ROOT_DIR"/assets/*-template.svg)
if [[ ${#SVGS[@]} -eq 0 ]]; then
  echo "素材が見つかりません: assets/*-template.svg" >&2
  exit 1
fi

for SVG in "${SVGS[@]}"; do
  NAME="$(basename "$SVG" .svg)"
  PNG="$ROOT_DIR/assets/$NAME.png"

  # 塗りが中間色のままだとアルファが最大値に届かず、アイコンが薄く描画される
  sed -E 's/#[0-9A-Fa-f]{6}/#000000/g' "$SVG" > "$WORK/$NAME.svg"

  qlmanage -t -s 512 -o "$WORK" "$WORK/$NAME.svg" >/dev/null 2>&1

  RENDERED="$WORK/$NAME.svg.png"
  if [[ ! -f "$RENDERED" ]]; then
    echo "SVG の描画に失敗しました: $SVG" >&2
    exit 1
  fi

  osascript -l JavaScript "$ROOT_DIR/scripts/build_menu_icon.js" "$RENDERED" "$PNG"

  echo "生成しました: $PNG"
done
