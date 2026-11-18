# Yotech-AI/homebrew-tap

Homebrew's way to [Hersir](https://hersir.dev) on a Mac with Apple silicon (M1 and later): you talk to one Chief, which turns what you need into cards on a board, starts AI coding agents on them and puts only tested work in your Review. It all runs on your own computer, with your own git repositories, logins and coding CLIs. Hersir's source, releases and docs: [Yotech-AI/hersir](https://github.com/Yotech-AI/hersir).

## Install

```sh
brew install yotech-ai/tap/hersir
```

Then link Hersir into Applications in your home folder, once, so you open it from Finder, Spotlight and the Dock (Homebrew may not put it there itself):

```sh
mkdir -p ~/Applications && ln -sfn "$(brew --prefix)/opt/hersir/Hersir.app" ~/Applications/Hersir.app
```

Or start it from a terminal: `hersir`.

- It is the release's ready-made build for macOS, the one Hersir's one-line install installs: `Hersir.app`, with `hersir-mcp`, the helper the Chief's chat starts, inside it, and the `hersir` and `hersir-mcp` commands.
- A release reaches this tap only once it is signed, usually the same day. The formula pins the build's sha256 from the release's `SHA256SUMS`, which the release's signature covers, so Homebrew installs that build and no other.
- Hersir.app is signed ad hoc, not by Apple. Homebrew doesn't put the quarantine mark on what a formula downloads, so macOS opens Hersir without asking, as after the one-line install.
- Hersir needs git, which comes with Apple's Command Line Tools (`xcode-select --install`) or Homebrew (`brew install git`), and Claude Code or Codex, logged in: those stay your own installs. What else each system needs: [docs/install.md](https://github.com/Yotech-AI/hersir/blob/main/docs/install.md).
- Use one way to install Hersir, not both: to move from the one line to Homebrew, first remove the one line's install with its `--uninstall`, which keeps your data.

## Update

```sh
brew update && brew upgrade hersir
```

Then restart Hersir. `brew upgrade` on its own looks at the tap only once a day, so a release from today needs `brew update` first. Hersir doesn't update itself when Homebrew installed it: Settings → Updates gives this command.

## Uninstall

```sh
brew uninstall hersir
rm ~/Applications/Hersir.app
```

The second line removes the link you made. Your data stays in `~/Library/Application Support/Hersir`, and the window's own settings in `~/Library/WebKit/dev.hersir.app`: install Hersir again, in any way, and it has them back.

## Why a formula, not a cask

Homebrew puts the quarantine mark on everything a cask downloads, and has no way to leave it off since July 2026. macOS then stops an app that Apple hasn't notarized, such as Hersir, until you allow it in System Settings → Privacy & Security → Open Anyway, at the first start and most likely after every upgrade. A formula's download gets no such mark. The formula never sets or removes it either.

## This tap

`Formula/hersir.rb` is made by `scripts/packages.sh` in [Yotech-AI/hersir](https://github.com/Yotech-AI/hersir), from [`packaging/homebrew`](https://github.com/Yotech-AI/hersir/tree/main/packaging/homebrew) there, for each signed release: changes to it go there, not here. Nothing else publishes a version, here or in CI.

The files in this repository are under the BSD Zero Clause License (0BSD, [`LICENSE`](LICENSE)). Hersir itself is under the Apache License 2.0.
