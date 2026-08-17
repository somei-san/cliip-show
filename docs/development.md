# 開発手順

## 前提

- macOS
- Rust toolchain

## 開発起動

```bash
cargo run
```

## ローカル操作確認（ワンコマンド）

```bash
./scripts/local_check.sh
```

引数なし実行では、アプリのデフォルト表示設定（`hud_position=top`, `hud_scale=1.1`, `hud_background_color=default`）で確認します。

主なオプション:

```bash
./scripts/local_check.sh --position bottom --scale 1.5 --color red
./scripts/local_check.sh --no-stop-installed --no-build
```

このスクリプトは検証用configを `/tmp/cliip-show-local-check.toml` に作成し、`Ctrl+C` で終了できます。

## 設定ファイルと環境変数

現在の設定値を表示する:

```bash
cliip-show --config-show
```

設定ファイル:
- 既定パス: `~/Library/Application Support/cliip-show/config.toml`
- パス変更: `CLIIP_SHOW_CONFIG_PATH=/path/to/config.toml`

ファイルへの変更は再起動なしで反映されます。

設定キー:
- `poll_interval_secs`（既定値: `0.3`、`0.05` - `5.0`）
- `hud_duration_secs`（既定値: `1.0`、`0.1` - `10.0`）
- `hud_fade_duration_secs`（既定値: `0.3`、`0.0` - `2.0`、`0.0` でフェードなし）
- `max_chars_per_line`（既定値: `100`、`1` - `500`）
- `max_lines`（既定値: `5`、`1` - `20`）
- `hud_position`（既定値: `top`、`top` / `center` / `bottom`）
- `hud_scale`（既定値: `1.1`、`0.5` - `2.0`）
- `hud_background_color`（既定値: `default`、`default` / `yellow` / `blue` / `green` / `red` / `purple`）
- `hud_emoji`（既定値: `📋`、任意の絵文字。空でアイコンなし）
- `hud_image_max_height`（既定値: `160`、`40` - `240`）画像サムネイルの高さ上限（px）。実際の上限は `hud_scale` 倍され、元画像より大きくは表示しません
- `language`（既定値: `auto`、`auto` / `ja` / `en`）メニューバーと設定ウィンドウの表示言語。`auto` は macOS の優先言語に従います

環境変数は設定ファイルより優先します。キー名を大文字にして `CLIIP_SHOW_` を付けた名前です。

```bash
CLIIP_SHOW_HUD_DURATION_SECS=2.5 \
CLIIP_SHOW_HUD_FADE_DURATION_SECS=0.5 \
CLIIP_SHOW_MAX_LINES=3 \
CLIIP_SHOW_HUD_POSITION=top \
CLIIP_SHOW_HUD_SCALE=1.2 \
CLIIP_SHOW_HUD_BACKGROUND_COLOR=blue \
CLIIP_SHOW_HUD_EMOJI=🍣 \
CLIIP_SHOW_HUD_IMAGE_MAX_HEIGHT=120 \
CLIIP_SHOW_LANGUAGE=en \
cargo run
```

## `.app` 化して動作確認

ローカルで `.app` として起動確認したい場合のみ実行してください。  
通常は Homebrew 経由での利用を想定しています。

```bash
cargo install cargo-bundle
cargo bundle --release
open target/release/bundle/osx/cliip-show.app
```

## アイコンの素材を更新する

`assets/*-template.svg` を差し替えたときは、同名の PNG を作り直します。この PNG を `src/menu.rs`（メニューバー常駐アイコン）と `src/settings_window.rs`（設定ウィンドウのタブ）が `include_bytes!` で埋め込み、テンプレート画像として描画します。素材を増やすときも `-template.svg` で終わる名前で `assets/` に置きます。

`scripts/build_menu_icon.sh` を実行すると、`assets/` にあるすべての SVG から PNG を生成します。

```bash
./scripts/build_menu_icon.sh
```

スクリプトは次の3点を行います。

- 塗りを純黒に置換してから描画する。元の塗りが中間色のままだとアルファが最大値に届かず、アイコンが薄く表示されます
- 白背景で描かれたものを、輝度からアルファへ変換する。テンプレート画像はアルファだけを使うため、これで素材になります
- 図形の外接矩形が canvas の中心に来るよう平行移動する。SVG の canvas と図形の位置関係はそのままだと偏っていることがあり、表示したときに上下左右にずれて見えます

出力後、不透明ピクセル数と外接矩形の中心を表示します。中心が (0.5, 0.5) から離れている場合や不透明ピクセルが 0 の場合は生成に失敗しています。

## ビジュアルリグレッションテスト

HUDの描画結果をPNGで比較します。

### 実行方法

```bash
# 初回または意図的なUI変更時にベースラインを更新
./scripts/visual_regression.sh --update

# 通常の差分チェック
./scripts/visual_regression.sh
```

このスクリプトは以下の観点を比較します。

- デフォルト設定での表示
- 設定プロファイルごとの表示（例: `max_lines=2`, `max_chars_per_line=24`）

### 生成物

- `tests/visual/baseline/*.png`: 比較基準となるベースライン画像
- `tests/visual/artifacts/*.current.png`: 現在の描画結果
- `tests/visual/artifacts/*.diff.png`: 差分を赤で強調した画像（差分がある場合）

### 判定ルール

- 判定はピクセル差分率で行います
- 既定の許容値は `MAX_DIFF_PERMILLE=120`（12%）です
- 必要に応じて環境変数で調整できます

```bash
MAX_DIFF_PERMILLE=80 ./scripts/visual_regression.sh
```

### 運用ルール

- 通常のPRでは `./scripts/visual_regression.sh` のみ実行
- 意図したUI変更を入れたPRのみ `./scripts/visual_regression.sh --update` を実行
- CI失敗時は `visual-regression-artifacts` の diff 画像を確認

## Homebrewで公開する手順

タグを push すると GitHub Actions が自動で Release 作成と Homebrew tap 更新を行います。

### 1. リリーススクリプトを実行する

```bash
./scripts/release.sh 0.1.4
```

バージョンを引数で指定すると、`Cargo.toml` のバージョン更新 → コミット・push → タグ作成・push を一括で行います。

引数なしで実行すると、現在の `Cargo.toml` のバージョンでタグを作成します（事前に手動でバージョンを更新済みの場合）。

```bash
./scripts/release.sh
```

### 2. 自動実行される内容

GitHub Actions（`.github/workflows/release.yml`）が以下を自動実行します。

1. tarball の SHA256 を算出
2. GitHub Release を作成（リリースノート自動生成）
3. [Homebrew Tap リポジトリ](https://github.com/somei-san/homebrew-tap)の Formula を更新

進捗は [Actions](https://github.com/somei-san/cliip-show/actions) で確認できます。

### セットアップ（初回のみ）

リポジトリの Settings → Secrets and variables → Actions に以下のシークレットを登録してください。

| シークレット名 | 用途 |
|---|---|
| `HOMEBREW_TAP_TOKEN` | `somei-san/homebrew-tap` への書き込み権限を持つ Fine-grained PAT |

### ユーザーのインストール手順

[TapリポジトリのREADME参照](https://github.com/somei-san/homebrew-tap/blob/main/README.md)

### 手動での Formula 生成（参考）

CD パイプラインを使わずに手動で Formula を生成する場合は以下を実行します。

```bash
./scripts/homebrew/generate_formula.sh somei-san 0.2.0 ./Formula/cliip-show.rb
```
