# 🥜 Cliip Show

[日本語](README.ja.md)

![Cliip Show HUD demo](docs/assets/cliip-show-hud.gif)

You thought you copied it, but you didn't 😡

You pasted, and it was not what you copied 😡 😡

Sound familiar?

Of course it does 🤓

So here it is: Cliip Show, a macOS menu bar app that shows what you just copied 🥜

## What it does

- Watches the clipboard and shows a HUD right after you copy
- Text is shown as-is, images as a thumbnail
- The HUD fades out on its own after a few seconds
- Runs in the background as a menu bar app
- Position, size, colour and the rest are changed from "Settings…" in the menu bar

## Install (Homebrew)

```bash
brew trust somei-san/tap
brew install --cask somei-san/tap/cliip-show
open -a "Cliip Show"
```

- `brew trust` is needed once; Homebrew 6 does not load casks from taps you have not trusted
- Cliip Show is not signed with an Apple Developer ID, so the cask clears the quarantine attribute for you
- On first launch it asks whether to start at login; you can change that any time in the settings window

## Links

- [Development guide](docs/development.md)
- [Homebrew tap repository](https://github.com/somei-san/homebrew-tap)

## Support

[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-support-FFDD00?logo=buymeacoffee&logoColor=black)](https://buymeacoffee.com/somei)

## About the name

The name plays on [an album by Creepy Nuts](https://en.wikipedia.org/wiki/Creepy_Nuts).
