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

## インストール手順 （Homebrew経由）

```bash
brew trust somei-san/tap
brew install --cask somei-san/tap/cliip-show
open -a "Cliip Show"
```

- `brew trust` は初回のみ必要です。Homebrew 6 は信頼していない tap の cask を読み込みません
- Apple の Developer ID で署名していないため、quarantine 属性は cask が外します
- 初回起動時にログイン時の自動起動を確認されます。設定ウィンドウからいつでも変更できます

## リンク

- [開発ガイド](docs/development.md)
- [Homebrew Tap リポジトリ](https://github.com/somei-san/homebrew-tap)

## 支援

[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-support-FFDD00?logo=buymeacoffee&logoColor=black)](https://buymeacoffee.com/somei)

## 名前の由来

アプリ名は[Creepy Nutsのアルバム](https://ja.wikipedia.org/wiki/%E3%82%AF%E3%83%AA%E3%83%BC%E3%83%97%E3%83%BB%E3%82%B7%E3%83%A7%E3%83%BC)のもじりです
