# Packaging

Hersir's packages for package managers, made from the ready-made builds of its releases. Only a signed release reaches them: [`scripts/packages.sh`](../scripts/packages.sh) checks the release's signature, fills its version, sha256s and commit into the files here, and publishes them. Jeffrey runs it on their own computer, right after signing a release ([docs/RELEASING.md](../docs/RELEASING.md), step 6); never CI and never an agent.

| Folder | Package | Install | Published to |
|---|---|---|---|
| [`aur/hersir-bin`](aur/hersir-bin) | `hersir-bin` on the AUR, for Arch Linux and Omarchy | `yay -S hersir-bin` | `ssh://aur@aur.archlinux.org/hersir-bin.git` |
| [`scoop`](scoop) | `hersir` in the Scoop bucket `Yotech-AI/scoop-bucket`, for Windows | `scoop bucket add hersir https://github.com/Yotech-AI/scoop-bucket`, then `scoop install hersir/hersir` | `https://github.com/Yotech-AI/scoop-bucket.git` (`main`) |

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

## scoop

The bucket `Yotech-AI/scoop-bucket` as it is for one release, the one `hersir.json` names: `hersir.json` goes into the bucket as `bucket/hersir.json`, with `scripts/hersir-shortcut.ps1`, `README.md` (the bucket's own README, for people who install Hersir with Scoop) and [`LICENSE`](LICENSE) beside it. For each signed release, `scripts/packages.sh vX.Y.Z` writes the release's version, the zip's address and its sha256 into `hersir.json`, puts the four files into a clone of the bucket (only them: what else is there goes), shows the change and pushes it to `main` after `y`, with your own GitHub login, as for Hersir itself. The files here stay as they are.

What the manifest takes, and what it does:

- **The programs:** the release's `hersir-X.Y.Z-windows-x86_64.zip`, with its sha256 from the release's `SHA256SUMS`, which the release's signature covers. `extract_dir` takes `hersir.exe` and `hersir-mcp.exe` out of the zip's `target\release`, exactly as the release built them, and `bin` puts both on the PATH, as the commands `hersir` and `hersir-mcp`.
- **No `checkver` or `autoupdate`:** only `scripts/packages.sh` changes the version, after the signature. Nothing of Scoop's own, such as its Excavator, may write a version into the bucket.
- **The mark:** `post_install` writes `hersir-package.txt`, which says `scoop`, beside the programs. With it, Hersir doesn't update itself, and Settings → Updates says to update it with `scoop update; scoop update hersir`.
- **The Start menu shortcut:** `post_install` then runs the bucket's `scripts/hersir-shortcut.ps1`, which makes `Start Menu\Programs\Scoop Apps\Hersir.lnk` with `install.ps1`'s own C#: it starts `hersir.exe` in Scoop's `current` folder, so it outlives updates, and carries Hersir's AppUserModelID, `dev.hersir.app`, without which Windows shows none of Hersir's notifications. Scoop's own `shortcuts` can't carry one, and `scoop reset` would make them again without it. `pre_uninstall` removes the shortcut; on an update, Scoop runs the old version's `pre_uninstall`, then the new version's `post_install`.
- **No `persist`:** Hersir's data is in `%APPDATA%\Hersir`, which Scoop never touches: `scoop uninstall hersir` keeps it, also with `--purge`.
- **The Visual C++ runtime:** `hersir.exe` carries it already (tauri-build links it in), and `install.ps1` builds `hersir-mcp.exe` with a static C runtime (`-C target-feature=+crt-static`), so neither needs `VCRUNTIME140.dll`. The bucket takes releases built that way: the first release after 0.18.1, not 0.18.1 itself or an earlier one.

To change the package, edit the files here. CI's Scoop job (`.github/workflows/ci.yml`, the scoop job) builds Hersir with `install.ps1 -BuildOnly` as Release builds does, checks with `dumpbin /dependents` that neither program needs `VCRUNTIME140.dll`, packs the zip as Release builds packs it, and installs it with Scoop from a local bucket of these files, filled in for that zip. Then `hersir --version` must say the version, `hersir-package.txt` must say `scoop`, the shortcut must carry `dev.hersir.app` and start `hersir.exe` in `current`, and the programs must have no `Zone.Identifier` stream; after an update, the shortcut and the mark must still be there; `scoop uninstall hersir` must keep the data folder as it was. It publishes nothing.

## Licence

The files in this folder are under the BSD Zero Clause License (0BSD, [`LICENSE`](LICENSE)), as the AUR encourages for package files, so any package repository can carry them as they are. Hersir itself is under the Apache License 2.0 ([`LICENSE`](../LICENSE) at the repository's root).
