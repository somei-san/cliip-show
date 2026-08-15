# cliip-show 開発ガイド

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

### 運用ルール
- 通常の変更: 上記3つがすべて通ることを確認してからPRを出す
- 意図したUI変更（HUD外観の変更など）: VRTのベースラインを更新する
  ```bash
  ./scripts/visual_regression.sh --update
  ```
- `Cargo.toml` の `edition` を変えたら `.claude/settings.json` の rustfmt フック（`--edition` を直書き）も同期する。ズレるとフックの整形結果を CI の `cargo fmt --check` が拒否する

## モジュール構成

| ファイル | 役割 |
|---|---|
| `src/main.rs` | エントリポイント（`fn main` のみ） |
| `src/lib.rs` | モジュール宣言 |
| `src/cli.rs` | CLIフラグの処理（`--help`, `--config`, `--render-hud-png` など） |
| `src/config/` | 設定（`types.rs` 型 / `parse.rs` パース / `io.rs` 読み書き / `settings.rs` 項目定義 / `cli.rs` `--config` サブコマンド） |
| `src/hud.rs` | HUDウィンドウ生成・レイアウト計算・描画 |
| `src/app.rs` | AppDelegate・クリップボード監視・フェードアニメーション |
| `src/menu.rs` | メニューバー常駐アイコンとメインメニューの構築 |
| `src/settings_window.rs` | 設定ウィンドウのUI |
| `src/login_item.rs` | LaunchAgent の plist 書き出し・解除（自動起動） |
| `src/text.rs` | テキスト切り詰め処理 |
| `src/png.rs` | PNG生成・差分計算（VRT用） |
| `src/objc_helpers.rs` | NSString変換ユーティリティ |
| `src/error.rs` | `AppError` 型定義 |

## リリース

`v*` タグの push で `release.yml` が GitHub Release の作成と [somei-san/homebrew-tap](https://github.com/somei-san/homebrew-tap) の Formula 更新まで行う。

Formula は `packaging/homebrew/cliip-show.rb.template` から自動生成されるが、tap の README は手書きなので追随しない。起動方法・設定コマンド・スクリーンショットを変えたら tap の README も更新すること。
