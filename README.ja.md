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
- 表示位置・サイズ・色などはメニューバーの「設定…」から変更できます

## 動作環境

- macOS（AppKitを使用）
- Homebrew（通常利用時のインストール手段）

## インストール手順 （Homebrew経由）

```bash
brew install somei-san/tap/cliip-show
cliip-show
```

`cliip-show` はターミナルを占有したまま起動します。これは初回だけの一時的な起動で、ターミナルを閉じるとアプリも終了します。起動すると自動起動の確認ダイアログが出て、あとから設定ウィンドウの「ログイン時に自動起動」でも切り替えられます。

## リンク

- [開発ガイド](docs/development.md)
- [Homebrew Tap リポジトリ](https://github.com/somei-san/homebrew-tap)

## 支援

[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-support-FFDD00?logo=buymeacoffee&logoColor=black)](https://buymeacoffee.com/somei)

## 解説

アプリ名は[Creepy Nutsのアルバム](https://ja.wikipedia.org/wiki/%E3%82%AF%E3%83%AA%E3%83%BC%E3%83%97%E3%83%BB%E3%82%B7%E3%83%A7%E3%83%BC)のもじりです
