# 🥜 Cliip Show

[English](README.md)

![Cliip Show HUDの表示イメージ](docs/assets/cliip-show-hud.gif)

コピーしたと思ったのにできてなかった 😡

ペーストしたら意図したコピー内容と違った 😡 😡

そんなことありませんか？

ありますよねぇ〜 🤓

てなわけで、

コピーされたプレーンテキストを画面中央に表示する、macOS向けの常駐アプリ Cliip Show です 🥜

## 概要

- クリップボードの更新を監視し、コピー直後にHUD表示します
- テキストはそのまま、画像はサムネイルで表示します
- HUDは数秒で自動的にフェードアウトして消えます
- アプリはバックグラウンドで常駐して動作します

テキストと画像の両方を含むコピー（ブラウザや表計算アプリからのコピー）ではテキストを表示します。

メニューバーのアイコンから以下の操作ができます。
- 設定…: 設定ウィンドウを開く
- 一時停止: HUDの表示を止める（チェックで状態表示）。一時停止中にコピーした内容は再開後も表示しません
- Cliip Show について: バージョンとリポジトリへのリンクを表示します
- Cliip Show を終了

## 動作環境

- macOS（AppKitを使用）
- Homebrew（通常利用時のインストール手段）

## インストール手順 （Homebrew経由）

```bash
brew install somei-san/tap/cliip-show
cliip-show
```

`cliip-show` はターミナルを占有したまま起動します。これは初回だけの一時的な起動で、ターミナルを閉じるとアプリも終了します。起動すると自動起動の確認ダイアログが出て、あとから設定ウィンドウの「ログイン時に自動起動」でも切り替えられます。

## 表示設定

メニューバーの「設定…」から設定ウィンドウを開いて変更します。設定を変更する手段はこれだけで、ウィンドウは「設定」「寄付」の2つのタブに分かれています。

変更は「保存」を押すと反映され、保存せずにウィンドウを閉じると破棄されます。「言語」と「ログイン時に自動起動」だけは例外で、変更した瞬間に反映され、「デフォルトに戻す」の対象外です。

現在の設定値を表示する:

```bash
cliip-show --config-show
```

### リンク

- [開発ガイド](docs/development.md)
- [Homebrew Tap リポジトリ](https://github.com/somei-san/homebrew-tap)

## 支援

[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-support-FFDD00?logo=buymeacoffee&logoColor=black)](https://buymeacoffee.com/somei)

同じリンクは設定ウィンドウの「寄付」タブからも開けます。

## 解説

アプリ名は[Creepy Nutsのアルバム](https://ja.wikipedia.org/wiki/%E3%82%AF%E3%83%AA%E3%83%BC%E3%83%97%E3%83%BB%E3%82%B7%E3%83%A7%E3%83%BC)のもじりです