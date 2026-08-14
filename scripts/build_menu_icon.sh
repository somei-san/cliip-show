#!/usr/bin/env bash
# assets/peanut-template.svg から、メニューバーアイコン用の
# assets/peanut-template.png を生成する。
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "このスクリプトは macOS でのみ動作します" >&2
  exit 1
fi

SVG="$ROOT_DIR/assets/peanut-template.svg"
PNG="$ROOT_DIR/assets/peanut-template.png"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

if [[ ! -f "$SVG" ]]; then
  echo "素材が見つかりません: $SVG" >&2
  exit 1
fi

# 塗りが中間色のままだとアルファが最大値に届かず、アイコンが薄く描画される
sed -E 's/#[0-9A-Fa-f]{6}/#000000/g' "$SVG" > "$WORK/icon.svg"

qlmanage -t -s 512 -o "$WORK" "$WORK/icon.svg" >/dev/null 2>&1

RENDERED="$WORK/icon.svg.png"
if [[ ! -f "$RENDERED" ]]; then
  echo "SVG の描画に失敗しました" >&2
  exit 1
fi

osascript -l JavaScript "$ROOT_DIR/scripts/build_menu_icon.js" "$RENDERED" "$PNG"

echo "生成しました: $PNG"
