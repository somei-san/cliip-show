#!/usr/bin/env bash
# assets/peanut-template.png から .app 用のアイコン assets/AppIcon.icns を作る。
# 素材の PNG は build_menu_icon.sh が SVG から生成する。
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "このスクリプトは macOS でのみ動作します" >&2
  exit 1
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

osascript -l JavaScript "$ROOT_DIR/scripts/build_app_icon.js" "$WORK/icon.png"

ICONSET="$WORK/AppIcon.iconset"
mkdir -p "$ICONSET"

# iconutil が要求する名前と辺の長さの対応
sips -z 16 16     "$WORK/icon.png" --out "$ICONSET/icon_16x16.png"      >/dev/null
sips -z 32 32     "$WORK/icon.png" --out "$ICONSET/icon_16x16@2x.png"   >/dev/null
sips -z 32 32     "$WORK/icon.png" --out "$ICONSET/icon_32x32.png"      >/dev/null
sips -z 64 64     "$WORK/icon.png" --out "$ICONSET/icon_32x32@2x.png"   >/dev/null
sips -z 128 128   "$WORK/icon.png" --out "$ICONSET/icon_128x128.png"    >/dev/null
sips -z 256 256   "$WORK/icon.png" --out "$ICONSET/icon_128x128@2x.png" >/dev/null
sips -z 256 256   "$WORK/icon.png" --out "$ICONSET/icon_256x256.png"    >/dev/null
sips -z 512 512   "$WORK/icon.png" --out "$ICONSET/icon_256x256@2x.png" >/dev/null
sips -z 512 512   "$WORK/icon.png" --out "$ICONSET/icon_512x512.png"    >/dev/null
cp "$WORK/icon.png" "$ICONSET/icon_512x512@2x.png"

iconutil --convert icns --output "$ROOT_DIR/assets/AppIcon.icns" "$ICONSET"

echo "生成しました: assets/AppIcon.icns"
