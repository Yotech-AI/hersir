# Packaging

Hersir's packages for package managers, made from the ready-made builds of its releases. Only a signed release reaches them: [`scripts/packages.sh`](../scripts/packages.sh) checks the release's signature, fills its version, sha256s and commit into the files here, and publishes them. Jeffrey runs it on their own computer, right after signing a release ([docs/RELEASING.md](../docs/RELEASING.md), step 6); never CI and never an agent.

| Folder | Package | Install | Published to |
|---|---|---|---|
| [`aur/hersir-bin`](aur/hersir-bin) | `hersir-bin` on the AUR, for Arch Linux and Omarchy | `yay -S hersir-bin` | `ssh://aur@aur.archlinux.org/hersir-bin.git` |
| [`homebrew`](homebrew) | the formula `hersir` in the Homebrew tap `yotech-ai/tap`, for Macs with Apple silicon | `brew trust yotech-ai/tap`, then `brew install yotech-ai/tap/hersir` | `https://github.com/Yotech-AI/homebrew-tap.git`, its `main` |

## aur/hersir-bin

`PKGBUILD` and `.SRCINFO`, as the AUR keeps them, for one release: the one they name. For each signed release, `scripts/packages.sh vX.Y.Z` writes them with that release's values (`pkgver`, `pkgrel`, `_commit` and the sums) into a clone of the AUR's `hersir-bin`, with [`LICENSE`](LICENSE) beside them, shows the change and pushes it after `y`. The files here stay as they are.

What the package takes, and from where:

- **The programs:** the release's `hersir-X.Y.Z-linux-x86_64.tar.gz`, with its sha256 from the release's `SHA256SUMS`, which the release's signature covers. `hersir` and `hersir-mcp` go into `/usr/lib/hersir` exactly as the release built them (no strip).
- **The mark:** `/usr/lib/hersir/hersir-package.txt`, which says `aur`. With it, Hersir doesn't update itself, and Settings → Updates says to update it with `yay -Syu`.
- **The launcher and the desktop entry:** `install.sh`'s own, from `install.sh` at the release's commit: `/usr/lib/hersir/hersir-launch`, the `hersir` command in `/usr/bin` that links to it, and `/usr/share/applications/hersir.desktop`. The `PKGBUILD` takes `install.sh`'s two here-documents and lets bash expand them with `/usr/lib/hersir` for `$LIB`, as when `install.sh` runs.
- **The icons and NOTICE:** from the release's commit, in the sizes `install.sh` installs, under `/usr/share/icons/hicolor`, and NOTICE in `/usr/share/licenses/hersir-bin`.
- The commit's files come from `raw.githubusercontent.com` by the commit's id, never from a branch. `scripts/packages.sh` takes their sha256s from git at that commit, which it fetches by the id the signature names.
- **depends:** the libraries the two programs link (WebKitGTK 4.1, GTK 3, libsoup 3 and the rest), libayatana-appindicator for the tray icon, hicolor-icon-theme, git, curl and bash. Claude Code, Codex and Gemini stay the user's own installs.
- Removing the package removes only its files: the data folder, `~/.local/share/hersir`, stays.

To change the package, edit `PKGBUILD` here, then make its `.SRCINFO` again on Arch Linux with `makepkg --printsrcinfo > .SRCINFO`. CI's Arch Linux job (`.github/workflows/ci.yml`, the arch job) builds it with `makepkg` from the release it names, runs `namcap`, installs it and compares the launcher, the desktop entry and the icons with the ones `install.sh` makes. It publishes nothing.

## homebrew

`hersir.rb`, the formula, for one release: the one it names; and `README.md`, the tap's README. For each signed release, `scripts/packages.sh vX.Y.Z` writes the formula with that release's values (its `url` and `sha256`) into a clone of `Yotech-AI/homebrew-tap` as `Formula/hersir.rb`, with the README and [`LICENSE`](LICENSE) beside it, shows the change and pushes it to `main` after `y`. Nothing else in the tap changes, so it can hold other formulae. The files here stay as they are. The same release with another formula becomes the formula's next `revision`, so `brew upgrade` installs it again; a new README alone doesn't.

What the formula installs, and from where:

- **The build:** the release's `hersir-X.Y.Z-macos-arm64.tar.gz`, with its sha256 from the release's `SHA256SUMS`, which the release's signature covers. Homebrew reads the version from its address. A formula, never a cask: Homebrew puts the quarantine mark on what a cask downloads, never on a formula's, and the formula neither sets nor removes it.
- **Hersir.app:** the build's own, in the formula's prefix (`$(brew --prefix)/Cellar/hersir/X.Y.Z/Hersir.app`, which `$(brew --prefix)/opt/hersir/Hersir.app` always leads to), with `hersir-mcp` copied into its `Contents/MacOS` and the app signed ad hoc again (`codesign --force --deep --sign -`), as `install.sh` does.
- **The commands:** `hersir`, a script that starts the app's own program (not a link, as macOS tells a program the path of the link it started from), and `hersir-mcp`, a link to the one in the app, in Homebrew's `bin`.
- **The mark:** none is written. A Hersir that runs from Homebrew's Cellar doesn't update itself, and Settings → Updates says to update it with `brew update && brew upgrade hersir`. A file inside Hersir.app would break its signature.
- The caveats say how to link Hersir.app into `~/Applications`, once, which a formula may not do itself. `brew test` runs `hersir --version`.
- Uninstalling removes only the formula's files: the data folder, `~/Library/Application Support/Hersir`, stays.

To change the formula, edit `hersir.rb` here. CI's Homebrew job (`.github/workflows/ci.yml`, the homebrew job) makes a test build of the pull request's commit, packed as Release builds packs a release's, installs the formula for it from a local tap, and runs `brew test`, `brew audit`, `codesign --verify` on the installed app and a check that nothing the formula installed has the quarantine mark. It publishes nothing.

## Licence

The files in this folder are under the BSD Zero Clause License (0BSD, [`LICENSE`](LICENSE)), as the AUR encourages for package files, so any package repository can carry them as they are. Hersir itself is under the Apache License 2.0 ([`LICENSE`](../LICENSE) at the repository's root).
