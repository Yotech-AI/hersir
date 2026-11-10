# Hersir through Homebrew, the AUR and Scoop for 1.0, from the release's ready-made builds (research, 2026-11-10)

Status: research only, no code. Written by the R&D Agent for GA-272 on 2026-11-10, against `main` at `edab8f2` (Hersir 0.16.0; the card names `11eb959`, and nothing the packages touch changed between the two: `git diff --stat 11eb959 edab8f2` is GA-258's Memory and Chief files only). Line numbers are main's.
Legend: **[V]** = verified today, in the code (file and line) or in the primary source named (link and date in §8). **[I]** = my judgement, or a fact I could not check in a primary source (the reason is given).
Limits: this run may not run brew, makepkg, scoop, curl or a build, so the cask, the PKGBUILD and the manifest below are **drafts, not built or tested**. §5.9 says how each card tests them. GA-260 (the signature), GA-252 and GA-253 (the one line and Update from the builds) are not on main yet and have no pull request (`gh pr list --state all --search "GA-260 in:title"`: none): where this design leans on GA-260, it follows the card's description of it.
This document follows `docs/research/packaging.md` (GA-165, 2026-10-13), which planned the AUR and a tap **built from source**, before releases had ready-made builds. Its from-source plan and its build-time `GIZAI_PACKAGE` setting no longer fit: §2.4 says what carries over.

---

## 1. Summary

(written last, see below)

---

## 2. The project now

### 2.1 What Hersir does for its users

Hersir is a desktop app where you talk to one Chief, which turns what you need into cards on a board, starts agents on them and puts only tested work in your Review (README.md:13). It all runs on your own computer, with your own git repositories, logins and coding CLIs (README.md:24). [V] It runs on Linux (x86_64), Macs with Apple silicon and Windows 11, and installs today with one line that builds a release from source (docs/install.md:7-27). An installed Hersir updates itself the same way (docs/RELEASING.md:175-201). [V]

### 2.2 How it is built, released, installed and updated

| Part | What it is | Where |
|---|---|---|
| The app | Tauri 2 (tauri-build 2.7.1): Rust in `src-tauri/` and `crates/`, React in `src/`. The program is `hersir`; the chat's helper is a second program, `hersir-mcp` (crate `gizai-mcp`). App id `dev.hersir.app`, version 0.16.0. | src-tauri/tauri.conf.json:3-6; Cargo.lock:4305-4306; crates/gizai-mcp/Cargo.toml:8-12 |
| Release builds (GA-251) | When a release is published: the release's own `install.sh --build-only` on Ubuntu 22.04 (x86_64) and macOS 15 (Apple silicon), `install.ps1 -BuildOnly` on Windows. Each archive holds exactly what `--skip-build` installs, at its build paths, and must install from a fresh checkout and say the version before it counts. Then the workflow's own token uploads `hersir-X.Y.Z-linux-x86_64.tar.gz`, `hersir-X.Y.Z-macos-arm64.tar.gz`, `hersir-X.Y.Z-windows-x86_64.zip` and `SHA256SUMS`. | .github/workflows/release-builds.yml:1-16, :89-102, :182-221, :230-265, :291-346 |
| What the archives hold | Linux: `target/release/hersir` and `hersir-mcp`, built against glibc 2.35. macOS: the same two and `target/release/bundle/macos/Hersir.app`, **without** `hersir-mcp` inside it (install.sh copies it in and signs the app ad hoc again). Windows: `hersir.exe` and `hersir-mcp.exe`. No icons, desktop entry or launcher: install.sh takes those from the source. | release-builds.yml:191-194, :212-215; install.sh:296-302, :401-420 |
| The signature (GA-260, planned) | `release-signers` in the repository; `hersir-X.Y.Z.sig` signs the line `hersir vX.Y.Z <commit>`, a newline and `SHA256SUMS`; checked with `ssh-keygen -Y verify`. Only Jefsev signs, on their own computer. | the card; not on main |
| Linux install (install.sh) | `<prefix>/lib/hersir/` holds `hersir`, `hersir-mcp` and `hersir-launch` (your login shell's PATH with `bash -lc`, and Mesa's EGL on a PC without NVIDIA). `<prefix>/bin/hersir` links to the launcher; the icon in five PNG sizes and the SVG under `hicolor`; `hersir.desktop` with `Exec=<lib>/hersir-launch` and `StartupWMClass=hersir`. The data is backed up with the new build first. | install.sh:271-284, :364-426 (launcher :375-386, icons :401-407, entry :409-420) |
| macOS install | `<prefix>/lib/hersir/Hersir.app` with `hersir-mcp` copied into `Contents/MacOS`, signed ad hoc again (`codesign --force --deep --sign -`); `lsregister -f`; `~/Applications/Hersir.app` links to it. | install.sh:289-361 (:296, :302, :331, :333-340) |
| Windows install | `%LOCALAPPDATA%\Programs\Hersir\` with `hersir.exe`, `hersir-mcp.exe` and the marker `hersir-installed.txt` (the version); a Start menu shortcut `Hersir.lnk` that carries the AppUserModelID `dev.hersir.app`. | install.ps1:2-3, :128, :366, :369-481 |
| `hersir-mcp` | Hersir looks for it next to its own program (the folder of `current_exe`), or at `HERSIR_MCP_BIN`. | src-tauri/src/mcp.rs:65-72 |
| Tray (Linux) | libayatana-appindicator is loaded with `dlopen` at start; without it Hersir runs with no tray icon. | src-tauri/src/lib.rs:459-466, :516-523 |
| Notifications (Windows) | Hersir sets its AppUserModelID at start; Windows shows its toasts only when a Start menu shortcut carries that id. | src-tauri/src/lib.rs:151-154, :584-592; src-tauri/src/notifications.rs:397-406 |
| Data | `~/.local/share/hersir`, `~/Library/Application Support/Hersir`, `%APPDATA%\Hersir` (or the gizai folder of an install from before 0.17.0); the window's storage in `~/.local/share/dev.hersir.app`, `~/Library/WebKit/dev.hersir.app`, `%LOCALAPPDATA%\dev.hersir.app`. The install scripts' `--uninstall` keeps it; only `--purge` deletes it. Hersir copies its database before it upgrades the schema. | docs/PLATFORMS.md:16, :20; docs/install.md:33, :164-168; crates/gizai-core/src/db.rs:219-226 |
| Who updates itself | `install_prefix`: on Linux and macOS a Hersir at `<prefix>/lib/hersir/hersir` (on macOS also the app's own program in `lib/hersir/Hersir.app`), on Windows a `hersir.exe` beside `hersir-installed.txt`. Anything else says why it doesn't, with a dev build's text. | crates/gizai-agents/src/update.rs:175-214; src-tauri/src/update.rs:219-229 |
| What the UI shows | `UpdateStatus` carries `installTo` or `cannotInstall`. Settings → Updates shows `cannotInstall` in a grey line where Update would be; the sidebar's notice "Update to X.Y.Z" then opens Settings, with `cannotInstall` as its tooltip. | src-tauri/src/update.rs:165-192, :231-253; src/types.ts:535-555; src/components/UpdateSettings.tsx:27, :37-38, :66-68; src/components/UpdateNotice.tsx:54-60; src/lib/update.ts:22-31 |
| The rules that stay | No Apple Developer account and no Windows certificate; nothing signed beyond the build's own ad hoc signature; never browser downloads, because "a browser marks what it downloads, and Gatekeeper and SmartScreen stop an unsigned app with that mark". | docs/PLATFORMS.md:8, :10, :103; docs/RELEASING.md:173; docs/install.md:129-136 |
| Tests and CI | `cargo test --workspace` and `npm test` on Linux, macOS and Windows for every pull request; Install and One-line install run the installers on six systems; Release builds' own tests in `scripts/release-builds.test.mjs`. | docs/PLATFORMS.md:104-109 |

[V] for every row except the signature's, which is the card's.

### 2.3 What the feature touches

| Area | Today | With packages |
|---|---|---|
| The updater (`crates/gizai-agents/src/update.rs`, `src-tauri/src/update.rs`) | `/usr/lib/hersir/hersir` counts as an install script's install into `/usr` (§3.2): Update would build, then fail to write `/usr/lib/hersir` | a package's Hersir never updates itself, and knows which command does (GA-199, §5.2) |
| Settings → Updates, the sidebar's notice | "Update to X.Y.Z", or a dev build's text | the package manager's command, with Copy (GA-200, §5.1) |
| The release (docs/RELEASING.md) | steps 1-4, then GA-260's signature | one more step after the signature: the packages follow (§5.6) |
| Hersir's own build on Windows | `hersir-mcp.exe` is a plain `cargo build`, so it may need the Visual C++ runtime (§3.4) | built with the runtime inside it |
| Install docs (docs/install.md, README.md) | the one line and a clone | "With a package manager" (§5.7) |
| New, outside this repository | none | the AUR package `hersir-bin`, the tap `Yotech-AI/homebrew-tap`, the bucket `Yotech-AI/scoop-bucket` |

### 2.4 What carries over from GA-165's research, and what doesn't

- **Doesn't fit any more:** building from source in the package (GA-165's `gizai` PKGBUILD and formula), and marking a package's Hersir at build time with `GIZAI_PACKAGE`. Every channel now takes the same archive that Release builds made once, so nothing can be set while building: a package must mark its Hersir **when it installs it** (§5.2). [V: one build per system, release-builds.yml:88-102]
- **Carries over:** the trap in the updater (a package in `/usr/lib/hersir` looks like an install script's install), the texts and the Copy button in Settings → Updates (GA-165 §5.1, now GA-200), the launcher and desktop entry copied from install.sh, the AUR's rules (no pull requests, pushes to `master`, `.SRCINFO`), Homebrew's tap naming, and the decision to leave Homebrew on Linux out.
- GA-199 and GA-200 are that research's PK1 and PK2. §6 rewrites both for markers instead of a build-time setting.

---

## 3. The feature

### 3.1 What it should do (from the card)

- For 1.0, developers get Hersir the way they get their other tools: Homebrew on Macs with Apple silicon, the AUR on Arch and Omarchy, Scoop on Windows, all from the ready-made builds of each GitHub release.
- No Apple Developer account, no Microsoft certificate, nothing notarized.
- A package's Hersir doesn't update itself (GA-199), and Settings → Updates gives the package's command (GA-200).
- A new release reaches a package only after Jefsev's signature checks out against `release-signers`, never from an unsigned release.
- Uninstalling leaves your data, unless you ask otherwise.

### 3.2 Q1: Homebrew on Macs with Apple silicon

**Does Homebrew put the quarantine mark on what it installs? On a cask's, always; on a formula's, no.**
- Every cask download is quarantined. `Cask::Download#fetch` calls `quarantine(downloaded_path)`, which calls `Quarantine.cask!` (`cask/download.rb`). `Quarantine.propagate` then copies the mark to every file unpacked from it (`cask/quarantine.rb`). The mark names "Homebrew Cask" as its agent (`extend/os/mac/cask/quarantine.rb`). [V brew source at `ed4988b`, 2026-11-10]
- There is no way out any more:
  - Homebrew 5.0.0 (2025-11-12) deprecated `--no-quarantine` and `--quarantine`, "as Homebrew does not wish to easily provide circumvention to macOS security features";
  - PR #23363, "Remove leftover code for `--no-quarantine`", was merged on 2026-07-30;
  - `HOMEBREW_CASK_OPTS` no longer lists the switch.
  [V brew.sh, GitHub, `env_config.rb`]
- Formulae get no mark. The quarantine code is the `Cask::Quarantine` module, called only from cask code, and Homebrew's security page says "Homebrew applies macOS quarantine attributes to Cask downloads". [V] That formula downloads stay unmarked is [I]: this run didn't read the formula download code, so the Homebrew card checks it on a Mac (§5.9).

**Does a quarantined, ad hoc signed, unnotarized Hersir.app then open without System Settings? No, on macOS 15 and 26.**
- Gatekeeper runs when a quarantined app is launched: "If you launch a quarantined app, the system invokes Gatekeeper" (Apple DTS, forum thread 706442). [V]
- Since macOS 15, Control-click no longer overrides it: users "need to visit System Settings > Privacy & Security" (Apple developer news, 2024-08-06). [V]
- There, Open Anyway "is available for about an hour after you try to open the app", then asks for your login password. The macOS 15 and 26 guides say the same. [V Mac User Guide 15 and 26] Apple silicon runs ad hoc signed code ("a simple ad-hoc signature is sufficient", Big Sur 11.0.1 release notes). [V] So Open Anyway should open Hersir. [I: no Apple page names ad hoc signing; a reader report on 15.1 says so]
- On macOS 26.6.1 with default settings, a quarantined ad hoc app "was directed to the Trash" from the first dialog (Howard Oakley, 2026-08-11). [I: a careful secondary source]
- **And again after every upgrade, most likely.**
  - Since 2026, `brew upgrade` carries your approval over to the new version only when the new app satisfies the old app's designated requirement (`cask/upgrade.rb`: `signing_identity_match`, `:signer_changed`; `extend/os/mac/cask/quarantine.rb`: `designated_requirement`). [V]
  - An ad hoc signature names no one, so its designated requirement is the code's own hash, which changes with every build. [I: Apple's TN3127 didn't render for this run's tools] Hersir already shows the same effect: "After an update, macOS may ask once whether Hersir may use what it saved in your keychain" (docs/install.md:136). [V]
  - Then brew warns "Homebrew couldn't inherit …'s quarantine approval so macOS may prompt", and Open Anyway comes back with each release. [V the text; I that it fires for Hersir]
- **Without the mark, Hersir opens as it does today.** Apple DTS: "Unix-y networking tools, like curl and scp, don't quarantine the files they download", and "Gatekeeper usually only kicks in [if] the app is quarantined". [V] On macOS 26.6.1 an unquarantined ad hoc app "launched normally, without any warning dialog" (Oakley). [I]

**What Homebrew's current rules say.**
- **The official tap.** `homebrew/cask` requires apps to "pass Homebrew's Gatekeeper checks and must not require System Integrity Protection or Gatekeeper to be disabled or bypassed" (Acceptable Casks). [V] Casks that failed were disabled from 2026-09-01 with the reason `fails_gatekeeper_check`, Chromium among them (formulae.brew.sh). [V] So Hersir could never be in `homebrew/cask`, only in our own tap.
- **Our own tap.** The signing audit runs only for official casks, or with `--signing` (`cask/audit.rb`: `return if !cask.tap&.official? && !signing?`), and nothing checks Gatekeeper at install. [V] A tap is trusted per tap or per item since 6.0.0 ("Non-official taps require explicit trust by default", Tap Trust). [V]
- **Removing the mark from our own cask is possible, but against Homebrew's direction.**
  - Homebrew 7.0.0 (2026-09-13) deprecated cask `*flight` Ruby blocks and formula `post_install` blocks in favour of declared steps. Official taps reject them; third-party taps get warnings "until 11 December 2027". [V]
  - The declared steps have no quarantine step. A `run` step runs inside Homebrew's sandbox, so whether `run "/usr/bin/xattr", args: ["-dr", "com.apple.quarantine", …]` even works there is untested. [V the steps; I the rest]
  - GoReleaser documents that very hook, warning that "Apple may disable this bypass method in future macOS versions without notice". [V]
  - **Not recommended:** it does what Homebrew removed `--no-quarantine` to stop.
- **Formulae.** `homebrew/core` would not take Hersir as a formula: "Proprietary or platform-specific binary-only software belongs in a cask", and "A formula whose primary output is a native macOS `.app` bundle is not eligible" (Acceptable Formulae, written for `homebrew/core`). [V] A tap's own formulae aren't bound by those rules, and this run found no audit against them. [I]

**Which way works without an Apple account?**

| Way | Opens without System Settings? | What you get | Verdict |
|---|---|---|---|
| **A formula that installs the ready-made app** (`brew install yotech-ai/tap/hersir`) | **yes** [I: formulae aren't quarantined; checked on a Mac in the card] | Hersir.app in Homebrew's own folder (`$(brew --prefix)/opt/hersir/Hersir.app`) and the `hersir` command; you link the app into `~/Applications` once (formulae may not write there, GA-165 §3.2) | **recommended**: the same trust as the one-line install (a sha256 Jefsev's signature covers, no mark), within a tap's rules |
| A cask (`brew install --cask yotech-ai/tap/hersir`) | **no**: Open Anyway at the first start, and most likely after every upgrade | Hersir.app in `/Applications`, upgraded in place, so the Dock and Spotlight keep it | the standard form; choose it only if the Open Anyway step per release is acceptable (Q1 in §7) |
| A cask that removes the mark itself | yes, for now | as the cask | **no**: it circumvents what Homebrew stopped circumventing, on a deprecated or untested hook |
| Only the command-line programs (a formula with `hersir` and `hersir-mcp`, no app) | yes | no Dock icon, no Finder or Spotlight, and notifications come from Hersir's bundle id only once macOS knows Hersir.app (docs/PLATFORMS.md:50) | no: it is no longer a Mac app |

**Homebrew on Linux: no.**
- Casks now run on Linux ("`app_image` is Linux-only, `app` and `pkg` are macOS-only, and `binary` works on either", Cask Cookbook). [V]
- But a Linux cask can't declare the system's WebKitGTK, desktop entry or icons, and a formula would bring Homebrew's own WebKitGTK and Mesa stack (GA-165 §3.2). [V GA-165; I for casks]
- Arch and Omarchy get the AUR, and every Linux system has the one line.

**How a user updates it.** Plain `brew upgrade` upgrades formulae and casks. It runs `brew update` first only when the last one is older than `HOMEBREW_AUTO_UPDATE_SECS`, 86400 (24 hours) by default (`env_config.rb`). [V] So the command Settings shows is `brew update && brew upgrade hersir`: without `brew update`, a release from that day can be missed. [I]

**Uninstalling.** `brew uninstall hersir` removes the keg (Hersir.app and the command) and leaves your data. [I: brew removes only the keg] Remove the `~/Applications/Hersir.app` link yourself. A formula has no `zap`. A cask's `zap` "will never be performed by default, but only if the user uses `--zap`" (Cask Cookbook): see §5.4 for its list. [V]

### 3.3 Q2: the AUR, a `hersir-bin` package

**The name.**
- It must be `hersir-bin`. The AUR's rule is: "Packages that use prebuilt deliverables, when the sources are available, must use the -bin suffix" (AUR submission guidelines). [V]
- `hersir` is in none of Arch's own repositories (archlinux.org's search: 0 results). [V]
- The AUR's GitHub mirror has no branch that starts with `hersir`, while the same query for `yay` finds 31 (`api.github.com/repos/archlinux/aur/git/matching-refs/heads/…`). [V] So `hersir-bin` is very likely free [I: the AUR's own API and cgit answered this run's tools with their bot check]. Nothing reserves a name until its first push (GA-165).

**What it installs**, against install.sh's Linux install:

| install.sh (prefix `~/.local`) | `hersir-bin` | From |
|---|---|---|
| `lib/hersir/hersir`, `hersir-mcp` | `/usr/lib/hersir/hersir`, `hersir-mcp` (`hersir-mcp` beside `hersir`: mcp.rs:65-72) | the Linux archive, checked against the signed `SHA256SUMS` |
| `lib/hersir/hersir-launch` | `/usr/lib/hersir/hersir-launch`: install.sh's launcher (install.sh:375-386) for `/usr/lib/hersir` | a file kept with the PKGBUILD |
| none | `/usr/lib/hersir/hersir-package.txt`, holding `aur` (GA-199, §5.2) | written by `package()` |
| `bin/hersir` → the launcher | `/usr/bin/hersir` → the launcher | `package()` |
| `share/applications/hersir.desktop` | `/usr/share/applications/hersir.desktop`, `Exec=/usr/lib/hersir/hersir-launch` | a file kept with the PKGBUILD |
| five PNG sizes and the SVG in `share/icons/hicolor` | the same files in `/usr/share/icons/hicolor` | the release's source at its signed commit (`src-tauri/icons/`) |
| none | `/usr/share/licenses/hersir-bin/NOTICE` (Apache-2.0 §4(d); the licence text itself is in Arch's `licenses` package) | the release's source at its signed commit |

Why `/usr/lib/hersir` and not `/opt/hersir`: Arch keeps `/opt` for "Large self-contained packages" and `/usr/lib/pkg` for a package's own modules (Arch package guidelines), and Hersir's two programs and launcher are small. [V] The folder is what install.sh uses under its prefix, so the launcher and desktop entry stay copies of install.sh's with one path changed. A test in this repository keeps them equal (§5.3).

**The trap, again.** Hersir 0.16.0 and 0.17.0 take `/usr/lib/hersir/hersir` for an install script's install into `/usr` (update.rs:198-201). So Update would offer itself, build for minutes, then fail to write `/usr/lib/hersir`. The first `hersir-bin` must therefore carry a release with GA-199 in it. [V by reading the code]

**Dependencies.**
- Arch lists the libraries a program links itself, so namcap on the built package decides the final list (card AUR, §6).
- What the program needs:
  - WebKitGTK 4.1 and what it brings: `webkit2gtk-4.1` depends on `cairo`, `gdk-pixbuf2`, `glib2`, `glibc`, `gtk3`, `libgcc`, `libsoup3` and `pango`;
  - `git` (worktrees, the Editor) and `curl` (the release check, update.rs:136-147) at run time;
  - `libayatana-appindicator` for the tray, which Hersir loads with `dlopen`, so the package keeps it in `depends`, as GA-165's Q7 recommended;
  - `hicolor-icon-theme` for the icon folders.
- The versions today are `webkit2gtk-4.1` 2.54.2, `gtk3` 3.24.52, `libayatana-appindicator` 0.6.1, `libsoup3` 3.8.1, `glibc` 2.44 and `libgcc` 16.2.1 (archlinux.org, 2026-11-10). [V] `gcc-libs` is now a bundle of split libraries, and `libgcc` is the one a Rust program needs. [V]
- The archive is built against Ubuntu 22.04's glibc 2.35, older than Arch's 2.44, so it runs there (release-builds.yml:89-91). [V] The rest of its ABI is WebKitGTK 4.1's, which Arch carries. [I]
- **Not** `libsoup` (2.x) or `desktop-file-utils`, which Tauri's own AUR example lists: Arch's WebKitGTK 4.1 uses `libsoup3`, and pacman's hooks update the desktop and icon caches. [V Tauri's AUR page; V archlinux.org]

**Who owns it.**
- Jefsev's own AUR account maintains `hersir-bin`. Its SSH key stays on Jefsev's PC.
- The AUR has no pull requests: it takes pushes to `master` only, with a `.SRCINFO` made by `makepkg --printsrcinfo`. [V AUR submission guidelines]
- An account may hold several public keys. [V] Each key carries the account's whole push right: aurweb looks a key up to one user, then lets that user push to any package base they maintain or co-maintain, and create new ones (`aurweb/git/auth.py`, `aurweb/git/serve.py`). [V source; I conclusion]
- So a key in CI could push every AUR package Jefsev ever maintains, not just `hersir-bin`. A second account for a bot would narrow that, but Arch's Code of Conduct asks for "one account only" (worded for the forums) [V], so that is a grey area. [I]
- The packaging files get a licence of their own: the AUR encourages 0BSD. [V]

**How each release updates it.** Three ways, with where the key sits:

| Way | Key | What the key can do | Verdict |
|---|---|---|---|
| **By hand, right after Jefsev signs** (recommended): one command, `scripts/packages.sh vX.Y.Z` (§5.6), checks the signature, writes the PKGBUILD and `.SRCINFO`, builds the package as a check and pushes | on Jefsev's PC, in `~/.ssh`, offered by Jefsev's ssh-agent | push to Jefsev's AUR packages; it never leaves the PC | Jefsev is at the PC to sign anyway, so this adds one command and no new secret |
| The DevOps Agent, in a card after the signature | the same key, through Jefsev's ssh-agent, while the agent runs on Jefsev's PC | the same | possible later with the same script, once its run may use ssh to aur.archlinux.org. It adds a card per release and waits for Jefsev's signature anyway |
| CI, for example KSXGitHub/github-actions-deploy-aur, whose `ssh_private_key` input is "Your private key with access to AUR package" [V] | a GitHub secret, at best an environment secret behind Jefsev's approval (available for public repositories on GitHub Free [V]) | push every AUR package of the account; the AUR adds that "Automated PKGBUILD updates are used at your own risk and any malfunctioning accounts and their packages may be removed without prior notice" [V] | not now: it puts a publishing key in GitHub, which GA-260 keeps the signing key out of |

**How a user updates it.** `yay -Syu`, or `yay` alone, which is "Alias to `yay -Syu`" (yay's README). [V] `pacman -Syu` alone doesn't update AUR packages (ArchWiki, AUR helpers). [V] Omarchy comes with yay. [I]

**Uninstalling.** `sudo pacman -Rns hersir-bin`. pacman removes the package's own files, and "will not remove configurations that the application itself creates (for example "dotfiles" in the home directory)" (ArchWiki, pacman). [V] So `~/.local/share/hersir` and the window's storage (`~/.local/share/dev.hersir.app`) stay.

### 3.4 Q3: Scoop on Windows, a bucket with `hersir.json`

**The bucket and the name.**
- A bucket is any public git repository with the manifests in `bucket/`. Users add it with `scoop bucket add <name> <url>` and install with `scoop install <name>/<app>` (BucketTemplate's README). [V]
- `hersir` is in neither Main nor Extras: `bucket/hersir.json` answers 404 in both. [V] Recommended: `Yotech-AI/scoop-bucket`, added as `hersir`.

**The manifest** (draft in §5.5):
- `url` and `hash` for the zip, under `architecture.64bit`. Scoop's hash is SHA256 by default, written by our script from the signed `SHA256SUMS`.
- `extract_dir: "target/release"`, so `hersir.exe` and `hersir-mcp.exe` land side by side in the app's folder (mcp.rs:65-72).
- `bin: "hersir.exe"` for the `hersir` command.
- `checkver` for GitHub releases, and `autoupdate` with `"hash": {"url": "$baseurl/SHA256SUMS"}`. Scoop's built-in pattern reads a `<hash>  <file name>` line, so no regex is needed (`lib/autoupdate.ps1`). [V wiki and source; I that our line matches]

**The Start menu shortcut needs more than Scoop's `shortcuts`.**
- Scoop makes its shortcuts in `Start Menu\Programs\Scoop Apps` with `WScript.Shell`, pointing into the `current` folder, and sets no AppUserModelID (`lib/shortcuts.ps1`, `lib/install.ps1`). [V]
- Hersir's toasts need a Start menu shortcut that carries `dev.hersir.app` (src-tauri/src/notifications.rs:397-400). [V]
- So the manifest makes the shortcut itself:
  - `post_install` runs a small script kept in the bucket, which install.ps1's shortcut code (install.ps1:369-481) is ported into;
  - `pre_uninstall` removes it.
- Scoop runs the old version's `pre_uninstall` on every update before it installs the new one (`libexec/scoop-update.ps1`), so the shortcut is made again each time, still pointing at `current\hersir.exe`. [V the order; I the rest]
- The same `post_install` writes the marker `hersir-package.txt` (`scoop`) beside `hersir.exe`.

**Does Scoop's download mark the files as from the internet? No.**
- Scoop downloads with .NET's `HttpWebRequest`, or with aria2 when it is installed (`lib/download.ps1`). Neither `Zone.Identifier` nor `Unblock-File` appears in its download, install, shortcut or uninstall code. [V for those four files; a search of the whole repository was not possible]
- Microsoft says browsers add the mark, and that "Other methods of downloading files may not mark the files as coming from the Internet Zone", naming `curl.exe` and `Invoke-WebRequest` (about_Execution_Policies). [V]
- SmartScreen "checks the reputation of downloaded files before allowing them to run". [V] With no mark, it shouldn't stop `hersir.exe`. [I: Microsoft doesn't spell out that the check keys on the mark; the card checks on a Windows PC]
- **One exception that no channel avoids: Smart App Control.**
  - On a Windows 11 PC that has it on, "Smart App Control will block execution of unsigned files unless the file has a positive reputation. Smart App Control signature checks apply to all executable files, not just those downloaded from the Internet" (Microsoft Learn). [V]
  - So it blocks Hersir from Scoop, from the zip and from install.ps1's own build alike, and "There is currently no way to bypass Smart App Control protection for individual apps". [V]
  - Only a signature fixes that, which the card rules out. The docs say so (§5.7).

**The Visual C++ runtime.**
- Rust on Windows links the C runtime dynamically unless built with `crt-static` (Rust reference, Linkage). [V]
- `hersir.exe` gets the runtime inside it: tauri-build links the VC runtime statically unless told not to (`STATIC_VCRUNTIME`, `static_vcruntime::metabuild()` in tauri-build's `lib.rs` on its `dev` branch). [V dev; I for 2.7.1, the version in Cargo.lock:4305-4306]
- `hersir-mcp.exe` is a plain `cargo build -p gizai-mcp`, which no tauri-build touches (crates/gizai-mcp/Cargo.toml has no build script; install.ps1:283-285). [V] So it most likely needs `vcruntime140.dll`, which a fresh Windows doesn't have. [I: check with `dumpbin /dependents`]
- **Recommended:** build `hersir-mcp.exe` with the VC runtime inside it, the way tauri-build builds `hersir.exe`, rather than make the package install the runtime.
  - Extras' `vcredist2022` runs Microsoft's installers. [V]
  - A `depends` on `extras/vcredist2022` only works for users who added Extras under the name `extras` (`lib/depends.ps1`). [V]

**How a user updates it.**
- Quit Hersir first: `scoop update hersir` skips a program that runs ("Running process detected, skip updating", `libexec/scoop-update.ps1`). [V]
- `scoop update <app>` refreshes the buckets only when the last refresh is 3 hours old or more (`is_scoop_outdated`, `lib/core.ps1`). [V]
- So the command is `scoop update; scoop update hersir`.

**Uninstalling.**
- `scoop uninstall hersir` removes the app's folders, its shims, its shortcuts and the `current` junction (`libexec/scoop-uninstall.ps1`). [V]
- `-p` ("Remove all persistent data") deletes only Scoop's own persist folder for the app, which this manifest doesn't use. Scoop's code never touches `%APPDATA%`. [V] So `%APPDATA%\Hersir` stays, even with `-p`.

### 3.5 Q4: for each manager

**Updates, with Hersir's own Update off.** The archives are the same for every channel, so a package marks its Hersir when it installs it (§5.2): a one-word file `hersir-package.txt` beside the program on Linux and Windows. On macOS the mark is Homebrew's own folder: a formula's app runs from `…/Cellar/hersir/`, and a cask leaves `…/Caskroom/hersir`. Changing the app's files would break its signature's seal. Hersir then never offers Update, keeps checking GitHub, and shows the manager's command (GA-199, GA-200):

| | The command Settings shows | While Hersir runs |
|---|---|---|
| AUR | `yay -Syu` | pacman replaces the files; restart Hersir to use the new version. Until then a new chat answer starts the new `hersir-mcp` beside the old Hersir [I] |
| Homebrew | `brew update && brew upgrade hersir` (§3.2: `brew upgrade` refreshes the tap only once a day by itself) | brew puts the new keg in place and removes the old one; restart Hersir. Until then, a new chat answer can't find `hersir-mcp` beside the old program [I] |
| Scoop | `scoop update; scoop update hersir` | Scoop skips a program that runs ("Running process detected, skip updating", `libexec/scoop-update.ps1`), and `scoop update <app>` refreshes the buckets only when they are 3 hours old or more (`is_scoop_outdated` in `lib/core.ps1`). [V] So: quit Hersir, run both, start it again |

**Only signed releases reach a package.**
- The version, the address and the sha256 are written by one script, `scripts/packages.sh vX.Y.Z`, on Jefsev's PC right after the signature (§5.6). It first checks `hersir-X.Y.Z.sig` with `ssh-keygen -Y verify` against `release-signers` in its own checkout of `main`, never against the release's own copy. On a wrong or missing signature it stops before it writes anything.
- The sha256s come from that signed `SHA256SUMS`, and nothing else: the AUR's `updpkgsums` must arrive at the same number for the archive, or the script stops.
- No manager's own automation publishes anything: no Excavator in the bucket, no `brew bump-cask-pr` or autobump in the tap, no CI with the AUR key. Their `checkver`, `autoupdate` and `livecheck` fields stay in the files, as the managers' tools expect, but only the script writes new versions.
- `ssh-keygen -Y verify` takes the message on standard input and signals a good signature from an allowed signer with exit status 0 (ssh-keygen(1)). It has been in OpenSSH since 8.1 (2019), so Arch's, macOS's and the CI runners' OpenSSH all have it. [V man.openbsd.org, openssh.org release notes, runner-images]

**Uninstalling, and your data.**

| | Remove | Your data folder | Also left |
|---|---|---|---|
| AUR | `sudo pacman -Rns hersir-bin` | stays: pacman never removes what an app made in your home folder [V] | the window's storage, `~/.local/share/dev.hersir.app` |
| Homebrew | `brew uninstall hersir`, and `rm ~/Applications/Hersir.app` (the link you made) | stays | `~/Library/WebKit/dev.hersir.app`. A formula has no `zap`; with a cask, `brew uninstall --zap --cask hersir` would move what its `zap` lists to the Trash, and only then (Cask Cookbook) [V]. §5.4 lists what a cask's `zap` would hold, if Q1 picks the cask |
| Scoop | `scoop uninstall hersir` | stays: `%APPDATA%\Hersir` isn't Scoop's; `-p` purges only Scoop's own `persist` folder, which the manifest doesn't use [I: §3.4] | `%LOCALAPPDATA%\dev.hersir.app` |

None of the three backs your data up before it replaces Hersir, as the install scripts do (install.sh:271-284). Hersir itself copies its database before it upgrades the schema on the first start of a new version (crates/gizai-core/src/db.rs:219-226), so a package update that changes the schema is still covered. [V]

**What Jefsev must make or own** (all **by hand**, once, before the first package):

| What | Why | Recommendation |
|---|---|---|
| An AUR account, with an SSH key made on your PC (`ssh-keygen -t ed25519 -f ~/.ssh/aur`) and its public key in My Account | the AUR takes pushes only from a maintainer's key | your own account; the key with a passphrase, only on your PC; add `Host aur.archlinux.org` with that key to `~/.ssh/config` |
| The repository `Yotech-AI/homebrew-tap`, public, with a `README.md` | `brew tap yotech-ai/tap` reads `github.com/Yotech-AI/homebrew-tap` | yes; protect `main` against force pushes; no Actions secrets |
| The repository `Yotech-AI/scoop-bucket`, public | `scoop bucket add hersir https://github.com/Yotech-AI/scoop-bucket` | yes, the same settings |
| Push rights to both, for your own git and gh login | the script pushes the new cask and manifest as you | you have them as the organisation's owner [I] |
| GA-260's signing key and `release-signers` | the gate | as GA-260 sets them up; the script reads the signer identity and namespace GA-260 documents |
| A licence for the packaging files | the AUR asks for one [V] | 0BSD, in the AUR repository, the tap and the bucket (Q5) |

### 3.6 Q5: later, in a few lines each

**WinGet.**
- **What it takes.** A manifest of `InstallerType: zip` with `NestedInstallerType: portable`, listing `hersir.exe` and `hersir-mcp.exe` (only `portable` takes more than one nested file). It points at the release's zip, which must come from "the ISV's release location". [V winget-pkgs schema 1.12.0, Microsoft's policies]
- **How it gets in.** A pull request to `microsoft/winget-pkgs`, scanned "through several antivirus programs", then a moderator. No signing requirement is written anywhere. [V Learn; I that unsigned is fine]
- **The mark of the web.** winget marks every download Internet zone first, then re-marks a file from a trusted source such as winget-pkgs as zone 2 after its hash check (`DownloadFlow.cpp`). So SmartScreen probably stays quiet. [V source; I effect]
- **The costs:**
  - a portable app gets no Start menu shortcut ("No shortcuts or icons will be created"), so Hersir's toasts wouldn't show without one Hersir makes itself [V spec #182];
  - an unsigned program that starts other programs risks a "PUA" flag in the scans [V Learn: genuine tools "appear as PUA"];
  - updates need a classic token with `public_repo` for Komac or winget-releaser, which a script would run only after the signature check [V their READMEs].
- **Size:** M. Worth it after Scoop, if Windows users ask.

**Flatpak and Flathub.** A poor fit.
- Flathub builds "entirely from source", without network, and refuses apps that "rely on host components" for their core.
- Running the user's git and coding CLIs on the host needs `flatpak-spawn --host` (`--talk-name=org.freedesktop.Flatpak`, "commands to run outside the sandbox"). That leaves almost no sandbox, and Flathub's reviewers judge it.
- Flathub also asks for any AI-generated material to be disclosed, and "Flathub manifests must not contain AI-generated or AI-assisted content".
- [V Flathub requirements and linter pages] **Not recommended.**

**A Debian or RPM repository.**
- **The packages.** Tauri's bundler makes a `.deb` (tauri.conf.json:24-28 already lists `deb`, with `libayatana-appindicator3-1`) and an `.rpm`, and Tauri says to build on the oldest system you support, as Release builds does on 22.04. [V Tauri's Debian and RPM pages]
- **A repository needs:**
  - a GPG repository key, a second publishing key besides GA-260's, on Jefsev's PC;
  - hosting: GitHub Pages, R2, or Cloudsmith's open-source tier;
  - a keyring that users install with `signed-by`, never into apt's global keyring (Debian wiki: "MUST NOT be placed in /etc/apt/trusted.gpg.d").
- [V Debian wiki; I Cloudsmith's terms, read only as a search snippet] The openSUSE Build Service, Copr and a PPA rebuild from source on their own builders. [I]
- **Size:** L.
- **A smaller first step:** attach the `.deb` and `.rpm` to each release, as the archives are. Linux has no download mark to fear, and the signed `SHA256SUMS` would cover them. People wouldn't get updates through apt or dnf, though: Hersir's own Update would have to leave them alone, as for a package (§5.2).
