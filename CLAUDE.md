# cliip-show 開発ガイド

macOS 専用（objc2 / AppKit 依存で `cargo test` も macOS でのみ通る）。
開発起動・`.app` 化・メニューアイコン生成・VRT の生成物と判定ルール・Homebrew 公開手順は `docs/development.md` にある。

## テスト

コードを変更したあとは **UT と VRT と lint** を必ず確認すること。いずれも CI のゲートになっている。

```bash
# UT（ユニットテスト）
cargo test

# VRT（ビジュアルリグレッションテスト）
./scripts/visual_regression.sh

# lint（CI と同じ条件）
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

HUD を実機で見るなら `./scripts/local_check.sh`（オプションは `docs/development.md`）。

### 運用ルール
- 通常の変更: 上記3つがすべて通ることを確認してからPRを出す
- 意図したUI変更（HUD外観の変更など）: VRTのベースラインを更新する
  ```bash
  ./scripts/visual_regression.sh --update
  ```
- VRT が撮るのは HUD の contentView だけ。設定ウィンドウとメニューバーは対象外
- VRT のケースは `CLIIP_SHOW_*` 環境変数で設定を上書きして作る（`scripts/visual_regression.sh` の `run_case`）
- `Cargo.toml` の `edition` を変えたら `.claude/settings.json` の rustfmt フック（`--edition` を直書き）も同期する。ズレるとフックの整形結果を CI の `cargo fmt --check` が拒否する

## ドキュメント

`README.md`（英語）と `README.ja.md`（日本語）は同じ内容・同じ構成を保つ。片方だけ直すと、もう一方を見た読み手が古い情報を掴む。見出しの並び、設定キーの表、コマンド例まで対応させること。

UI 文言を README に書くときは `src/i18n.rs` の文言テーブルと一致させる。英語版に載せるラベルはテーブルの英語表記そのままにする。

## モジュール構成

| ファイル | 役割 |
|---|---|
| `src/main.rs` | エントリポイント（`fn main` のみ） |
| `src/lib.rs` | モジュール宣言 |
| `src/cli.rs` | CLIフラグの処理（`--help`, `--config-show`, `--render-hud-png` など） |
| `src/config/` | 設定（`types.rs` 型 / `parse.rs` パース / `io.rs` 読み書き / `settings.rs` 項目定義 / `cli.rs` `--config-show` の出力） |
| `src/hud.rs` | HUDウィンドウ生成・レイアウト計算・描画 |
| `src/app/` | AppDelegate・クリップボード監視・フェードアニメーション（`mod.rs` デリゲート配線・AppState / `panels.rs` About・支援ページ・自動起動プロンプト / `hud_show.rs` HUD表示とフェード / `config_reload.rs` 設定ファイルの再読み込み） |
| `src/menu.rs` | メニューバー常駐アイコンとメインメニューの構築 |
| `src/settings_window/` | 設定ウィンドウのUI（`mod.rs` 型定義・再エクスポート / `rows.rs` コントロール生成・配置 / `build.rs` ウィンドウ組み立て / `sync.rs` AppState との同期・操作） |
| `src/login_item.rs` | LaunchAgent の plist 書き出し・解除（自動起動） |
| `src/text.rs` | テキスト切り詰め処理 |
| `src/png.rs` | PNG生成・差分計算（VRT用） |
| `src/i18n.rs` | UI文言の言語ごとの表記と、表示言語の解決 |
| `src/objc_helpers.rs` | NSString変換とテンプレート画像生成のユーティリティ |
| `src/error.rs` | `AppError` 型定義 |

## リリース

`./scripts/release.sh <version>` でバージョン更新から `v*` タグの push までを行う。タグを起点に `release.yml` が GitHub Release の作成と [somei-san/homebrew-tap](https://github.com/somei-san/homebrew-tap) の Formula 更新まで実行する。

Formula は `packaging/homebrew/cliip-show.rb.template` から自動生成されるが、tap の README は手書きなので追随しない。起動方法・設定コマンド・スクリーンショットを変えたら tap の README も更新すること。
