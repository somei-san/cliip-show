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

When a copy carries both text and an image (browsers and spreadsheets usually do), the text is shown.

The menu bar icon gives you:
- Settings…: opens the settings window
- Pause: stops the HUD, with a checkmark while it is on. Anything copied while paused stays unshown after you resume
- About Cliip Show: shows the version and a link to the repository
- Quit Cliip Show

## Requirements

- macOS (built on AppKit)
- Homebrew (how you normally install it)

## Install (Homebrew)

```bash
brew install somei-san/tap/cliip-show
cliip-show
```

`cliip-show` holds the terminal while it runs — a one-off launch that ends when you close the terminal. On startup it asks whether to start at login, and you can toggle that any time under "Start at login" in the settings window.

## Settings

Open the settings window from "Settings…" in the menu bar. It is the only way to change settings, and it has two tabs, "Settings" and "Support".

Changes apply when you press Save, and closing the window without saving discards them. "Language" and "Start at login" are the exceptions: both apply the moment you change them, and "Restore Defaults" leaves them alone.

Print what is currently in effect:

```bash
cliip-show --config-show
```

### Links

- [Development guide](docs/development.md)
- [Homebrew tap repository](https://github.com/somei-san/homebrew-tap)

## Support

[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-support-FFDD00?logo=buymeacoffee&logoColor=black)](https://buymeacoffee.com/somei)

The same link is in the settings window under the "Support" tab.

## About the name

The name plays on [an album by Creepy Nuts](https://en.wikipedia.org/wiki/Creepy_Nuts).
