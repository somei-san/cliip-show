#!/usr/bin/env bash
# `Cliip Show.app` を組み立てる。cask で配る成果物と、ローカルで動作確認する .app は
# どちらもこのスクリプトが作る。
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

APP_NAME="Cliip Show"
BIN_NAME="cliip-show"
OUT_DIR="target/bundle"
UNIVERSAL=1

usage() {
  cat <<'USAGE' >&2
Usage: build_app_bundle.sh [--out <dir>] [--host-only]
  --out <dir>   .app の出力先ディレクトリ（既定: target/bundle）
  --host-only   実行中のマシンの CPU 向けだけをビルドする（既定は universal binary）
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --out)
      [[ $# -ge 2 ]] || { usage; exit 1; }
      OUT_DIR="$2"
      shift 2
      ;;
    --host-only)
      UNIVERSAL=0
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "不明なオプション: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "このスクリプトは macOS でのみ動作します" >&2
  exit 1
fi

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
if [[ -z "$VERSION" ]]; then
  echo "Cargo.toml からバージョンを読み取れません" >&2
  exit 1
fi

ICNS="assets/AppIcon.icns"
if [[ ! -f "$ICNS" ]]; then
  echo "アイコンがありません: ${ICNS}（./scripts/build_app_icon.sh で生成します）" >&2
  exit 1
fi

if [[ "$UNIVERSAL" -eq 1 ]]; then
  TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)
else
  TARGETS=("$(rustc -vV | sed -n 's/^host: //p')")
fi

for TARGET in "${TARGETS[@]}"; do
  if command -v rustup >/dev/null 2>&1; then
    rustup target add "$TARGET" >/dev/null
  fi
  cargo build --release --target "$TARGET"
done

APP="$OUT_DIR/$APP_NAME.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

BINARIES=()
for TARGET in "${TARGETS[@]}"; do
  BINARIES+=("target/$TARGET/release/$BIN_NAME")
done
# lipo は入力が1つでも通るので、universal と host-only で分岐せずに済む
lipo -create -output "$APP/Contents/MacOS/$BIN_NAME" "${BINARIES[@]}"

sed "s/{{VERSION}}/$VERSION/g" packaging/macos/Info.plist.template > "$APP/Contents/Info.plist"
cp "$ICNS" "$APP/Contents/Resources/AppIcon.icns"

# lipo で結合したバイナリは署名が外れる。ad-hoc 署名を入れ直さないと起動できない
codesign --force --sign - "$APP"

echo "生成しました: ${APP}（バージョン ${VERSION} / ${TARGETS[*]}）"
