# 🥜 cliip-show

[English](README.md)

![cliip-show HUDの表示イメージ](docs/assets/cliip-show-hud.gif)

コピーしたと思ったのにできてなかった 😡

ペーストしたら意図したコピー内容と違った 😡 😡

そんなことありませんか？

ありますよねぇ〜 🤓

てなわけで、

コピーされたプレーンテキストを画面中央に表示する、macOS向けの常駐アプリ`cliip-show`です 🥜

## 概要

- クリップボードの更新を監視し、コピー直後にHUD表示します
- テキストはそのまま、画像はサムネイルで表示します
- HUDは数秒で自動的にフェードアウトして消えます
- アプリはバックグラウンドで常駐して動作します

テキストと画像の両方を含むコピー（ブラウザや表計算アプリからのコピー）ではテキストを表示します。

メニューバーのアイコンから以下の操作ができます。
- 設定…: 設定ウィンドウを開く（詳細は下記「表示設定」参照）
- 一時停止: HUDの表示を止める（チェックで状態表示）。一時停止中にコピーした内容は再開後も表示しません
- cliip-show を終了

## 動作環境

- macOS（AppKitを使用）
- Homebrew（通常利用時のインストール手段）

## インストール手順 （Homebrew経由）

```bash
brew install somei-san/tap/cliip-show
cliip-show
```

`cliip-show` はターミナルを占有したまま起動します。これは初回だけの一時的な起動で、ターミナルを閉じるとアプリも終了します。起動すると自動起動の確認ダイアログが出るので、有効にすると次回ログインから自動で起動するようになります。

ログイン時の自動起動は設定ウィンドウの「ログイン時に自動起動」からいつでも切り替えられます（下記「表示設定」参照）。

## 表示設定

メニューバーの「設定…」から設定ウィンドウを開いて変更できます。`--config set`と同じ設定ファイルを読み書きします。

設定ウィンドウでの変更はその場では保存されず、下部のボタンで確定します。
- 保存: 変更をファイルに書き込み、HUDにも反映します（Enterキーでも実行できます）
- お試し表示: ファイルには保存せず、変更後の見た目をHUDで確認します。クリップボードの内容は使わず、押すたびに短文・長文・画像の固定サンプルを順に切り替えて表示します
- デフォルトに戻す: すべての項目を既定値に戻します。ファイルには保存しません

保存せずにウィンドウを閉じると、ファイルに保存済みの内容に戻ります。

「言語」と「ログイン時に自動起動」だけは上記の下書き・保存モデルに乗らず、変更した瞬間に反映します。「デフォルトに戻す」の対象外なのもこの2つだけです。「ログイン時に自動起動」はOSのログイン項目の設定なので、設定ファイルには保存しません。

### コマンドラインから

初期化と確認:

```bash
cliip-show --config init
cliip-show --config show
```

設定値を保存:

```bash
cliip-show --config set hud_duration_secs 2.5
cliip-show --config set hud_fade_duration_secs 0.5
cliip-show --config set max_lines 3
cliip-show --config set hud_position top
cliip-show --config set hud_scale 1.2
cliip-show --config set hud_background_color blue
cliip-show --config set hud_emoji 🍣
cliip-show --config set hud_image_max_height 120
```

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

> **設定の即時反映:** 変更は再起動なしで自動的に反映されます。

環境変数でも上書き可能です（設定ファイルより優先）。

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

設定ファイル:
- 既定パス: `~/Library/Application Support/cliip-show/config.toml`
- パス変更: `CLIIP_SHOW_CONFIG_PATH=/path/to/config.toml`

### リンク

- [開発ガイド](docs/development.md)
- [Homebrew Tap リポジトリ](https://github.com/somei-san/homebrew-tap)

## 解説

アプリ名は[Creepy Nutsのアルバム](https://ja.wikipedia.org/wiki/%E3%82%AF%E3%83%AA%E3%83%BC%E3%83%97%E3%83%BB%E3%82%B7%E3%83%A7%E3%83%BC)のもじりです