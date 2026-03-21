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
./scripts/local_check.sh --no-stop-brew --no-build
```

このスクリプトは検証用configを `/tmp/cliip-show-local-check.toml` に作成し、`Ctrl+C` で終了できます。

## `.app` 化して動作確認

ローカルで `.app` として起動確認したい場合のみ実行してください。  
通常は Homebrew 経由での利用を想定しています。

```bash
cargo install cargo-bundle
cargo bundle --release
open target/release/bundle/osx/cliip-show.app
```

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
