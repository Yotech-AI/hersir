# Hersir through Homebrew, the AUR and Scoop for 1.0, from the release's ready-made builds (research, 2026-11-10)

Status: research only, no code. Written by the R&D Agent for GA-272 on 2026-11-10, against `main` at `edab8f2` (Hersir 0.16.0; the card names `11eb959`, and nothing the packages touch changed between the two: `git diff --stat 11eb959 edab8f2` is GA-258's Memory and Chief files only). Line numbers are main's.
Legend: **[V]** = verified today, in the code (file and line) or in the primary source named (link and date in §8). **[I]** = my judgement, or a fact I could not check in a primary source (the reason is given).
Limits: this run may not run brew, makepkg, scoop, curl or a build, so the cask, the PKGBUILD and the manifest below are **drafts, not built or tested**. §5.9 says how each card tests them. GA-260 (the signature), GA-252 and GA-253 (the one line and Update from the builds) are not on main yet and have no pull request (`gh pr list --state all --search "GA-260 in:title"`: none): where this design leans on GA-260, it follows the card's description of it.
This document follows `docs/research/packaging.md` (GA-165, 2026-10-13), which planned the AUR and a tap **built from source**, before releases had ready-made builds. Its from-source plan and its build-time `GIZAI_PACKAGE` setting no longer fit: §2.4 says what carries over.

---

## 1. Summary

- **Homebrew: a formula, not a cask.** Homebrew quarantines every cask download, and has had no `--no-quarantine` since July 2026. On macOS 15 and 26 an ad hoc signed, unnotarized Hersir.app would then need System Settings → Privacy & Security → Open Anyway at the first start, and most likely again after every upgrade: Homebrew carries an approval over only to an app that meets the old one's designated requirement, which an ad hoc signature doesn't. A formula that installs the ready-made app gets no mark [I, checked on a Mac in the card], so Hersir opens as it does from the one line; you link it into `~/Applications` once. No Linux Homebrew.
- **AUR: `hersir-bin`** (the AUR requires the `-bin` suffix) from the Linux archive, in `/usr/lib/hersir`, with install.sh's launcher and desktop entry, and the icons from the release's signed commit. It is Jefsev's account, and the key stays on Jefsev's PC.
- **Scoop: a `Yotech-AI/scoop-bucket`** with `hersir.json`. Scoop sets no mark of the web, so SmartScreen stays quiet; Smart App Control, where it is on, blocks unsigned programs however they are installed. The manifest makes its own Start menu shortcut with Hersir's app id, for toasts, and `hersir-mcp.exe` should be built with the VC runtime inside it.
- **One gate:** `scripts/packages.sh vX.Y.Z`, run by Jefsev right after signing. It checks the signature with `ssh-keygen -Y verify` against `release-signers`, takes the sha256s only from that signed `SHA256SUMS`, and publishes all three. No publishing key goes to CI.
- **Hersir's own Update stays off** through a mark the package writes (`hersir-package.txt`, or Homebrew's folder on macOS): GA-199 and GA-200, rewritten. Uninstalling keeps your data everywhere.
- **Cards (§6):** GA-199, GA-200 and the AUR card at the same time, then Homebrew and Scoop. **Later:** WinGet maybe, Flatpak no, `.deb`/`.rpm` on releases first. **Open questions (§7):** 10, with Q1 (formula or cask) the main one.

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

---

## 4. Options

All three take the release's ready-made builds and need no certificate or Apple account. They differ in who writes a new version into the packages, and so in where the publishing keys sit.

### Option A (recommended): one script after the signature, on Jefsev's PC; the packaging kept in this repository

- **What it takes:**
  - the packaging files in this repository: `packaging/aur/`, `packaging/homebrew/` and `packaging/scoop/` (§5.3-5.5), with tests that keep them in step with install.sh and install.ps1;
  - `scripts/packages.sh vX.Y.Z` (§5.6), which checks the signature, writes the three files from the signed `SHA256SUMS`, and publishes: it pushes the AUR package with Jefsev's AUR key, and the formula and the manifest to the tap and the bucket with Jefsev's own git login;
  - after that, CI installs each package as people do (§5.9);
  - the marker and the texts in Hersir (GA-199, GA-200).
- **Libraries:** none new in Hersir. The tools are makepkg and namcap (Arch, GPL), Homebrew (BSD-2-Clause) and Scoop (Unlicense or MIT) [I: licences not read today], all well kept.
- **Keys:**
  - the AUR key and Jefsev's GitHub login stay on Jefsev's PC, next to GA-260's signing key;
  - CI only reads.
- **Risks:**
  - The packages follow only when Jefsev runs the script. It is one command right after signing, which Jefsev does anyway, and a later run catches up.
  - The script must run on Arch (makepkg). Jefsev's PC is Omarchy. [I]
- **Size:** GA-199 S-M, GA-200 S, AUR M, Homebrew M, Scoop M.

### Option B: each package repository updates itself in CI, after its own signature check

- **What it takes:**
  - The tap and the bucket get a workflow, started by hand or every few hours, that reads the latest release, checks its signature against a `release-signers` copy of their own, and commits the new version with their own `GITHUB_TOKEN`. That token can write only to that repository.
  - The AUR needs a workflow holding the AUR private key (KSXGitHub/github-actions-deploy-aur).
- **Risks:**
  - The AUR key in a GitHub secret can push every AUR package of Jefsev's account (§3.3), which is what GA-260 avoids for its signing key.
  - A commit pushed with `GITHUB_TOKEN` starts no other workflow ("a new workflow will not run", GitHub docs), so the test install must run in the same workflow, before it commits. [V]
  - Scheduled workflows in a public repository stop after 60 days without activity. [V]
  - The signature check would live in three places.
- **When:** for the tap and the bucket, if running the script by hand becomes a chore. Never for the AUR key without a new decision.

### Option C: build from source in the packages (GA-165's plan)

- Superseded: every release now has tested ready-made builds, and building takes minutes on every install and update. Scoop would also need the C++ Build Tools first.
- **Rejected,** as are a `hersir-git` package and Homebrew bottles.

---

## 5. The design (Option A)

### 5.1 The screens (GA-200)

Settings → Updates for a Hersir from the AUR, while 1.0.1 is out and signed, and 1.0.0 runs:

```
Updates
Hersir came from the AUR, so yay updates it, not Hersir itself. Hersir still asks GitHub for new
releases and tells you when one is out.

Version        ⊕ Hersir 1.0.0. Version 1.0.1 is out (checked 2 minutes ago).
               [↗ Release 1.0.1 on GitHub]
               ▸ What's new in 1.0.1

Update         ⊖ Hersir came from the AUR, so it doesn't update itself. Update it with yay -Syu, then restart Hersir.
               ┌──────────┐
               │ yay -Syu │  [⧉ Copy]
               └──────────┘
               The AUR package hersir-bin follows each release once it is signed, usually the same day.

New releases   [x] Check for new releases automatically
               [⟳ Check now]
```

The sidebar, above Company: `⊕ Hersir 1.0.1 is out`, a link that opens Settings, with the Update line as its tooltip.

| Where | AUR (`aur`) | Homebrew (`homebrew`) | Scoop (`scoop`) | Any other word (`package`) |
|---|---|---|---|---|
| The section's text (UpdateSettings.tsx:27) | Hersir came from the AUR, so yay updates it, not Hersir itself. Hersir still asks GitHub for new releases and tells you when one is out. | Hersir came from Homebrew, so brew updates it, not Hersir itself. (the same second sentence) | Hersir came from Scoop, so scoop updates it, not Hersir itself. (the same) | Hersir came from a package, so its package manager updates it, not Hersir itself. (the same) |
| The Update line (`cannotInstall`) | Hersir came from the AUR, so it doesn't update itself. Update it with yay -Syu, then restart Hersir. | Hersir came from Homebrew, so it doesn't update itself. Update it with brew update && brew upgrade hersir, then restart Hersir. | Hersir came from Scoop, so it doesn't update itself. Quit Hersir, update it with scoop update; scoop update hersir, then start it again. | Hersir came from a package, so it doesn't update itself. Update it with the package manager that installed it, then restart Hersir. |
| The command, in mono with Copy (`updateCommand`) | `yay -Syu` | `brew update && brew upgrade hersir` | `scoop update; scoop update hersir` | none |
| The hint under it | The AUR package hersir-bin follows each release once it is signed, usually the same day. | The formula in yotech-ai/tap follows each release once it is signed, usually the same day. | The manifest in the hersir bucket follows each release once it is signed, usually the same day. | none |
| The sidebar's notice (src/lib/update.ts:30) | Hersir 1.0.1 is out | the same | the same | the same |

- **How it fits the design system:** nothing new is drawn.
  - The texts use the section's `text` and the `Field` with its `hint` (src/components/Form.tsx).
  - The Update line is the `gh-line skipped` row with `CircleMinus` that `cannotInstall` uses today (UpdateSettings.tsx:66-67).
  - The command is in `mono`, with a `btn ghost sm` Copy button and lucide's `Copy` icon, as Copy ID on a card.
  - No gold: nothing needs you.
  - So no design card (GA-165 §5.1). [I]
- With nothing newer out, a package's Hersir shows only the Version line, as today (UpdateSettings.tsx:37). [V]

### 5.2 The mark, and the updater (GA-199)

A package's Hersir must never update itself, and must know which manager updates it. The archives are shared, so the mark is made at install time:

| Where Hersir runs | The mark | Who makes it |
|---|---|---|
| Linux, `/usr/lib/hersir/hersir` | `hersir-package.txt` beside the program, holding `aur` | the PKGBUILD's `package()` |
| Windows, `…\scoop\apps\hersir\current\hersir.exe` | `hersir-package.txt` beside the program, holding `scoop` | the manifest's `post_install` |
| macOS, a formula's app | the resolved program is in a `…/Cellar/hersir/<version>/Hersir.app` | Homebrew's own layout: nothing to write |
| macOS, a cask's app (only if Q1 picks the cask) | `<prefix>/Caskroom/hersir` exists for `/opt/homebrew` or `/usr/local`, and the app isn't an install script's | Homebrew's own layout |

- **Why not a file inside Hersir.app:** the app's ad hoc signature seals its files. A formula could still sign it again after adding the file, but Homebrew's own path is mark enough. [I]
- **Why not path rules on Linux and Windows:** `/usr/lib/…` and `…\scoop\apps\…` say less than a word the package writes, and a `.deb` or WinGet later just writes its own word. [I]
- **The mark wins over `install_prefix`**, so `/usr/lib/hersir/hersir` no longer counts as an install into `/usr` (update.rs:198-201).

What changes, file by file:

1. `crates/gizai-agents/src/update.rs`:
   - `pub fn package(exe: &Path) -> Option<String>`: the mark's word for the program at `exe`. It is lowercase `[a-z0-9-]`, at most 32 characters; any other content gives `"package"`, and a missing or empty file gives None. On macOS it also returns `"homebrew"` for the Cellar and Caskroom rules above.
   - `pub fn package_update_command(word: &str) -> Option<&'static str>`.
   - `parse_release` also keeps the release's asset names (GitHub's `assets[].name`), so a package's Hersir can tell a signed release (`hersir-X.Y.Z.sig` attached) from one still waiting for Jefsev's signature. GA-253 may add that list first.
2. `src-tauri/src/update.rs`:
   - `Inner` gets `package: Option<String>`, filled from `up::package(exe)` at start, with `Updates::set_package` for tests, as `set_exe` is.
   - The `install_prefix` wrapper (:219-229) answers a package first, with the exact texts of §5.1. So `status`, `check`, `start` and the restart refuse a package's install as they refuse a dev build's today.
   - `UpdateStatus` gets `package` and `update_command` (the contract in §6).
   - For a package, `available` counts a release only once it has its signature.
3. The UI (GA-200): `src/types.ts`, `UpdateSettings.tsx:27`, `:37-38`, `:66-68`, `UpdateNotice.tsx:54-60`, `src/lib/update.ts:22-31`.
4. Docs: docs/RELEASING.md:200 ("Only an installed Hersir updates itself") and docs/PLATFORMS.md:96 get one sentence: "A Hersir from a package (`hersir-package.txt` beside it, or Homebrew's folder on macOS) never updates itself: Settings → Updates gives its package manager's command."

The existing tests of the trap stay true without a mark (crates/gizai-agents/tests/it/update_test.rs:201-245). [V]

### 5.3 The AUR package (draft, untested)

In this repository, `packaging/aur/hersir-bin/`:

`PKGBUILD` (the script fills in `pkgver`, `_commit` and the archive's sum from the signed `SHA256SUMS`, and the small files' sums with `updpkgsums`):

```bash
# Maintainer: <Jefsev's name> <address at domain dot tld>
# hersir-bin: Hersir from the ready-made Linux build of a release, checked against the release's SHA256SUMS, which
# Jefsev's signature covers. Made by scripts/packages.sh from packaging/aur/hersir-bin in Yotech-AI/hersir.
pkgname=hersir-bin
pkgver=1.0.0
pkgrel=1
_commit=0000000000000000000000000000000000000000 # the release's commit, from its signed line
pkgdesc='Talk to one Chief that runs a team of AI coding agents through your board'
arch=('x86_64')
url='https://hersir.dev'
license=('Apache-2.0')
depends=('webkit2gtk-4.1' 'gtk3' 'libsoup3' 'glib2' 'cairo' 'pango' 'gdk-pixbuf2' 'glibc' 'libgcc'
         'libayatana-appindicator' 'hicolor-icon-theme' 'git' 'curl')
optdepends=('github-cli: review cards as pull requests on GitHub')
provides=('hersir')
conflicts=('hersir')
# The programs come stripped from the release's build (Cargo.toml's strip = true).
options=('!strip' '!debug')
_raw="https://raw.githubusercontent.com/Yotech-AI/hersir/$_commit"
source=("hersir-$pkgver-linux-x86_64.tar.gz::https://github.com/Yotech-AI/hersir/releases/download/v$pkgver/hersir-$pkgver-linux-x86_64.tar.gz"
        "hersir-$pkgver-32.png::$_raw/src-tauri/icons/32x32.png"
        "hersir-$pkgver-64.png::$_raw/src-tauri/icons/64x64.png"
        "hersir-$pkgver-128.png::$_raw/src-tauri/icons/128x128.png"
        "hersir-$pkgver-256.png::$_raw/src-tauri/icons/128x128@2x.png"
        "hersir-$pkgver-512.png::$_raw/src-tauri/icons/icon.png"
        "hersir-$pkgver.svg::$_raw/src-tauri/icons/gizai.svg"
        "hersir-$pkgver-NOTICE::$_raw/NOTICE"
        'hersir-launch'
        'hersir.desktop')
sha256sums=('<the Linux archive's line in the signed SHA256SUMS>'
            '…') # one per source, by updpkgsums

package() {
  local lib=/usr/lib/hersir size
  install -Dm755 target/release/hersir "$pkgdir$lib/hersir"
  install -Dm755 target/release/hersir-mcp "$pkgdir$lib/hersir-mcp"
  install -Dm755 hersir-launch "$pkgdir$lib/hersir-launch"
  # GA-199: Hersir from the AUR never updates itself; Settings → Updates gives yay's command.
  printf 'aur\n' > hersir-package.txt
  install -Dm644 hersir-package.txt "$pkgdir$lib/hersir-package.txt"
  install -dm755 "$pkgdir/usr/bin"
  ln -s "$lib/hersir-launch" "$pkgdir/usr/bin/hersir"
  install -Dm644 hersir.desktop "$pkgdir/usr/share/applications/hersir.desktop"
  for size in 32 64 128 256 512; do
    install -Dm644 "hersir-$pkgver-$size.png" "$pkgdir/usr/share/icons/hicolor/${size}x${size}/apps/hersir.png"
  done
  install -Dm644 "hersir-$pkgver.svg" "$pkgdir/usr/share/icons/hicolor/scalable/apps/hersir.svg"
  install -Dm644 "hersir-$pkgver-NOTICE" "$pkgdir/usr/share/licenses/$pkgname/NOTICE"
}
```

- `hersir-launch` is install.sh's Linux launcher (install.sh:375-386) with `/usr/lib/hersir`.
- `hersir.desktop` is install.sh's entry (install.sh:409-420) with `Exec=/usr/lib/hersir/hersir-launch`.
- A test in `scripts/` compares both with install.sh, so a release that changes them changes the package too. That answers GA-165's Q8 (two copies) with a test, not a reminder.

Notes:
- **Small files from the signed commit.**
  - The icons and NOTICE come from `raw.githubusercontent.com` at the release's commit, which the signed line names, so their content is the release's. The icons are 0.8 to 11 KB each (src-tauri/icons). [V]
  - The saved names carry `pkgver`, so makepkg's download cache never mixes versions. [I]
  - If GA-252 puts the icons into the Linux archive, the PKGBUILD takes them from there instead.
- **`.SRCINFO`** comes from `makepkg --printsrcinfo` in the script. The AUR repository also gets a `LICENSE` (0BSD, Q5).
- **Arch's own pacman hooks** refresh the desktop and icon caches: no `.install` file. [V GA-165, from archlinux.org's file lists]

### 5.4 The Homebrew formula (draft, untested)

The tap `Yotech-AI/homebrew-tap` holds `Formula/hersir.rb`, a `README.md` (the two commands, the `~/Applications` link, the update command, why it is a formula) and a `LICENSE` (0BSD). The template lives in `packaging/homebrew/hersir.rb` here.

```ruby
# Hersir from the ready-made macOS build of a release, checked against the release's SHA256SUMS, which Jefsev's
# signature covers. Made by scripts/packages.sh from packaging/homebrew/hersir.rb in Yotech-AI/hersir.
# A formula, not a cask: Homebrew quarantines every cask, and Hersir isn't notarized (docs/research/packages.md §3.2).
class Hersir < Formula
  desc "Talk to one Chief that runs a team of AI coding agents through your board"
  homepage "https://hersir.dev"
  url "https://github.com/Yotech-AI/hersir/releases/download/v1.0.0/hersir-1.0.0-macos-arm64.tar.gz"
  sha256 "<the macOS archive's line in the signed SHA256SUMS>"
  license "Apache-2.0"

  livecheck do
    url :stable
    strategy :github_latest
  end

  depends_on arch: :arm64
  depends_on :macos

  def install
    app = buildpath/"target/release/bundle/macos/Hersir.app"
    # hersir-mcp inside the app, beside hersir, where Hersir looks for it; then the app signed ad hoc again, so the
    # seal covers it too: what install.sh does (install.sh:296-302). An ad hoc signature names no one.
    (app/"Contents/MacOS").install "target/release/hersir-mcp"
    system "/usr/bin/codesign", "--force", "--deep", "--sign", "-", app
    prefix.install app
    # The hersir command runs the app's own program (not a link: Hersir looks for hersir-mcp beside the path it ran as).
    bin.write_exec_script prefix/"Hersir.app/Contents/MacOS/hersir"
  end

  def caveats
    <<~EOS
      To open Hersir from Finder and keep it in the Dock, link it into Applications in your home folder, once:
        mkdir -p ~/Applications && ln -sfn #{opt_prefix}/Hersir.app ~/Applications/Hersir.app
      Hersir doesn't update itself when Homebrew installed it. Update it with:
        brew update && brew upgrade hersir
      then restart Hersir. Your data stays in ~/Library/Application Support/Hersir.
    EOS
  end

  test do
    assert_equal "hersir #{version}", shell_output("#{bin}/hersir --version").strip
    assert_predicate prefix/"Hersir.app/Contents/MacOS/hersir-mcp", :executable?
  end
end
```

Notes:
- **No build:** `url` is the release asset, and nothing is compiled. `depends_on arch: :arm64` follows Hersir's Apple silicon decision (docs/PLATFORMS.md:9). [V]
- **No `post_install`:** Homebrew 7.0.0 deprecated it. The first start from Finder registers the app with LaunchServices, which then names its notifications. [I]
- **Two things only a Mac shows** (the card's check list):
  - Does a Dock item kept from the `~/Applications` link still start Hersir after `brew upgrade` replaces the keg?
  - Does Spotlight find the link? (GA-165 §3.2 left both open.)
  If the Dock breaks, the caveat says to start Hersir from `~/Applications` or with `hersir`.
- **If Q1 picks the cask instead:** the same archive, with these stanzas:
  - `app "target/release/bundle/macos/Hersir.app"`;
  - `command_wrapper "hersir", executable: "#{appdir}/Hersir.app/Contents/MacOS/hersir"`;
  - `preflight_steps` that copy `hersir-mcp` into the staged app, and a sign-again step;
  - `depends_on arch: :arm64`;
  - `zap trash: ["~/Library/Application Support/Hersir", "~/Library/WebKit/dev.hersir.app", "~/Library/Caches/dev.hersir.app"]`;
  - caveats that explain Open Anyway.

  The declared steps have no code-signing step [V Cask Cookbook]. So the cask needs Release builds to put `hersir-mcp` into the signed app first (a change to release-builds.yml:190-192), or a `run` step whose sandbox allows `codesign`. [I]

### 5.5 The Scoop manifest (draft, untested)

The bucket `Yotech-AI/scoop-bucket` holds `bucket/hersir.json`, `scripts/hersir-shortcut.ps1` (install.ps1's shortcut code, ported), a `README.md` and a `LICENSE` (0BSD). The templates live in `packaging/scoop/` here.

```json
{
    "version": "1.0.0",
    "description": "Talk to one Chief that runs a team of AI coding agents through your board.",
    "homepage": "https://hersir.dev",
    "license": "Apache-2.0",
    "notes": [
        "Hersir doesn't update itself when Scoop installed it: quit Hersir, then run scoop update; scoop update hersir.",
        "Your data stays in %APPDATA%\\Hersir, also after scoop uninstall hersir."
    ],
    "architecture": {
        "64bit": {
            "url": "https://github.com/Yotech-AI/hersir/releases/download/v1.0.0/hersir-1.0.0-windows-x86_64.zip",
            "hash": "<the Windows zip's line in the signed SHA256SUMS>"
        }
    },
    "extract_dir": "target/release",
    "bin": "hersir.exe",
    "post_install": [
        "Set-Content -LiteralPath \"$dir\\hersir-package.txt\" -Value 'scoop' -Encoding Ascii",
        "& \"$bucketsdir\\$bucket\\scripts\\hersir-shortcut.ps1\" -Target \"$dir\\hersir.exe\""
    ],
    "pre_uninstall": "& \"$bucketsdir\\$bucket\\scripts\\hersir-shortcut.ps1\" -Remove",
    "checkver": { "github": "https://github.com/Yotech-AI/hersir" },
    "autoupdate": {
        "architecture": {
            "64bit": {
                "url": "https://github.com/Yotech-AI/hersir/releases/download/v$version/hersir-$version-windows-x86_64.zip",
                "hash": { "url": "$baseurl/SHA256SUMS" }
            }
        }
    }
}
```

Notes:
- `$dir` in `post_install` is the `current` path, so the shortcut and the marker survive updates (`lib/install.ps1`: `link_current` before the hooks). [V]
- Whether `$bucket` names the user's own name for the bucket in a hook is [I]: the card checks it, or falls back to `$bucketsdir\hersir`.
- **The shortcut:** `Start Menu\Programs\Scoop Apps\Hersir.lnk`, with the AppUserModelID `dev.hersir.app`, so toasts show.
- **No `persist`:** Hersir's data is in `%APPDATA%\Hersir`.

### 5.6 The packages step, after the signature

`scripts/packages.sh vX.Y.Z [aur] [homebrew] [scoop]` (all three when none is named), run by Jefsev in a clean checkout of `main` on their own PC:

1. **The release.** `gh release view vX.Y.Z` must show a published release, not a draft or pre-release, with `SHA256SUMS` and `hersir-X.Y.Z.sig` among its assets. The script downloads both into a scratch folder (`gh release download`).
2. **The signature.** The commit comes from `git ls-remote origin refs/tags/vX.Y.Z^{}`. Then:

   ```sh
   { printf 'hersir %s %s\n' "$tag" "$commit"; cat SHA256SUMS; } |
     ssh-keygen -Y verify -f release-signers -I "$signer" -n "$namespace" -s "hersir-$version.sig"
   ```

   - `release-signers` is this checkout's, which must equal `origin/main`'s (`git diff --quiet origin/main -- release-signers`). It is never the release's own copy.
   - The signer and namespace are the ones GA-260 documents.
   - Any exit status but 0 stops the script before it writes anything.
3. **The sums.** Each archive's sha256 is its line in that `SHA256SUMS`.
4. **AUR:**
   - clone `ssh://aur@aur.archlinux.org/hersir-bin.git` into the scratch folder and copy `packaging/aur/hersir-bin/` over it;
   - set `pkgver`, `pkgrel=1`, `_commit` and the archive's sum, then run `updpkgsums` for the small files;
   - stop if the archive's sum changed;
   - `makepkg --printsrcinfo > .SRCINFO`, `makepkg --cleanbuild` and `namcap` as a check (nothing is installed);
   - `git commit -m "Update to X.Y.Z"`, `git push`.
5. **Homebrew:** clone `Yotech-AI/homebrew-tap`, write `Formula/hersir.rb` from the template with the version and the macOS sum, commit `hersir X.Y.Z`, push.
6. **Scoop:** the same with `bucket/hersir.json` and the Windows sum.
7. **The check:** it prints the three addresses and starts the Packages workflows' "installed as people do" jobs (§5.9) with `gh workflow run`.

For docs/RELEASING.md, a step after GA-260's signature:

> **The packages follow, once the release is signed.** In a clean checkout of `main` on your own PC: `scripts/packages.sh vX.Y.Z`. It checks `hersir-X.Y.Z.sig` against `release-signers` and stops if it doesn't check out. Then it writes the version and the sha256s from that signed `SHA256SUMS` into the AUR's `hersir-bin`, the tap's formula and the bucket's manifest, and pushes the three: the AUR with your AUR key, the tap and the bucket with your git login. Then the Packages workflow installs each one as people do, and must be green. An unsigned release never reaches a package. If a push fails, the release still stands: run the script again.

And to "Never" (docs/RELEASING.md:168-173): never let CI, Excavator, `brew bump-formula-pr` or any `autoupdate` publish a package version, and never put the AUR key into GitHub.

### 5.7 The install docs

- `docs/install.md` gets "With a package manager" after "In one line":
  - the three commands: `yay -S hersir-bin`; `brew install yotech-ai/tap/hersir` and the `~/Applications` link; `scoop bucket add hersir https://github.com/Yotech-AI/scoop-bucket` and `scoop install hersir/hersir`;
  - how each updates and uninstalls, that the data stays;
  - "use one way to install Hersir, not two": remove the one line's install first, which keeps your data.
- "Not signed" (docs/install.md:129-136) gets Smart App Control: with it on, Windows blocks Hersir however it was installed.
- `README.md` gets one line under Install.
- The packages' own READMEs repeat what is theirs.

### 5.8 Security and privacy

- **What gets installed** is exactly the archive Release builds made and tested. Its sha256 is pinned in each package and taken from the `SHA256SUMS` that Jefsev signed. The PKGBUILD's small files come from the commit named in the signed line.
- **Who can publish:**
  - the AUR key (all of Jefsev's AUR packages) and Jefsev's GitHub login (the tap, the bucket) stay on Jefsev's PC, as GA-260's signing key does;
  - CI holds no publishing secret;
  - no manager's own automation runs.
- **Release assets can be replaced** by any token with `contents: write` (`gh release upload --clobber`). [V GitHub docs] The signature is what makes that harmless: a replaced archive no longer matches the signed sum, and the script refuses it.
  - GitHub's immutable releases (GA 2025-10-28) would lock the assets. But "assets can't be added" after publishing, which breaks today's order: builds uploaded after publishing (GA-251), then the signature (GA-260). [V] So not now (Q9).
- **Nothing new is sent.** A package's Hersir checks GitHub's latest release as before (docs/install.md:187-189).
- **Gatekeeper and SmartScreen:** none of the recommended channels marks the files, as with the one line.
  - The formula gets no quarantine [I, checked in the card].
  - Scoop sets no mark of the web [V for its download code].
  - A user's Smart App Control still blocks unsigned programs everywhere. [V]
- **Data** stays on every uninstall (§3.5).

### 5.9 How it is tested

- **GA-199:**
  - unit tests of `package()`: AUR mark, Scoop mark, Cellar path, Caskroom, no mark, a mark with junk;
  - a flow test in the style of `a_gizai_that_isnt_installed_never_installs_anything` (src-tauri/tests/it/update_flow_test.rs:456): with `set_package(Some("aur"))` and an exe at `<scratch>/usr/lib/hersir/hersir`, `installTo` is null, `cannotInstall` is the AUR text, `start_update` fails with it, and nothing appears in `data/update`;
  - a release without its `.sig` isn't `available` for a package's Hersir, and is for an install script's.
- **GA-200:**
  - `src/lib/update.test.ts`, `UpdateNotice.test.tsx` and a new `UpdateSettings.test.tsx` for each text and Copy;
  - `scripts/ui-test.sh`;
  - a screenshot from `cage` with a fake package status.
- **AUR:**
  - `scripts/packages.sh` against a fake release, with a test signing key and a test `release-signers` in a scratch folder: a good signature writes the files; a bad one, a missing one or a changed `SHA256SUMS` writes nothing;
  - on Jefsev's PC, `makepkg` of the real package as a normal user (never `-i`), `namcap`, and the file list against §3.3;
  - after publishing, Packages' Arch job: an `archlinux` container installs `hersir-bin` from the AUR as a build user, runs `hersir --version`, and checks the marker and the desktop entry.
- **Homebrew:**
  - Packages' `macos-15` job: the formula from a local tap (`brew tap-new`), `brew install`, `brew test`, `brew audit --strict`, `codesign --verify --deep --strict` on the app, `hersir --version`;
  - **no `com.apple.quarantine` on the installed app** (`xattr -p com.apple.quarantine` fails), which settles §3.2's [I];
  - then on Jefsev's Mac: install, link, open from Finder with no prompt, keep in the Dock, upgrade at the next release.
- **Scoop:**
  - Packages' Windows job: `scoop install` of the local manifest, `hersir --version`, the marker, the shortcut's AppUserModelID (read back as install.yml reads install.ps1's), no `Zone.Identifier` on `hersir.exe`, `scoop uninstall` keeping `%APPDATA%\Hersir`;
  - `dumpbin /dependents hersir-mcp.exe` without `VCRUNTIME140.dll`, in Release builds' Windows job;
  - then on Jefsev's Windows 11 PC: Start menu, a toast, an update while Hersir runs (skipped), then after quitting.

### 5.10 What is left out

- Hersir noticing that its package installed a newer version and offering Restart (GA-165's Q6): later, as its own card.
- A cask, unless Q1 picks it; Homebrew on Linux; `hersir-git`; bottles.
- WinGet, Flatpak, and Debian or RPM repositories (§3.6).
- CI publishing the packages (Option B).
- The website's install page (another repository).

---

## 6. Proposed cards

Order: **GA-199, GA-200 and AUR at the same time** → a release with GA-199 and GA-200 in it (the usual release card) and GA-260's first signed release → **Homebrew and Scoop at the same time** → Jefsev runs `scripts/packages.sh` for the next signed release. No card needs a design first (§5.1). Every card's pull request is in `Yotech-AI/hersir`. The tap, the bucket and the AUR get their files from the script, so they need no pull requests.

### 1. GA-199 (rewritten): a Hersir from a package doesn't update itself, and says which command does

- **Column:** To do. **Role:** backend. **Builds on:** none.
- **Builds:** §5.2:
  - `package()` and `package_update_command()` in `gizai_agents::update`;
  - the release's asset names, and a signed release for a package;
  - the package answer first in `install_prefix`, with §5.1's texts word for word;
  - `package` and `updateCommand` in `UpdateStatus`;
  - the docs sentence.
- **Acceptance:**
  - [ ] With `hersir-package.txt` holding `aur` beside the program, `installTo` is null, `cannotInstall` is the AUR text, and `start_update` fails with it, fetching and building nothing (a flow test).
  - [ ] The same for `scoop`, and on macOS for an app under `…/Cellar/hersir/` (unit tests of the path rules on every system). Any other word gives the `package` text and no command.
  - [ ] A package's Hersir shows a release as out only once its `hersir-X.Y.Z.sig` is attached. An install script's Hersir is unchanged.
  - [ ] Without a mark nothing changes: every existing update test passes.
  - [ ] `cargo test --workspace`, `npm test` and `npm run build` pass.

### 2. GA-200 (rewritten): Settings → Updates shows a package's update command

- **Column:** To do. **Role:** frontend. **Builds on:** GA-199's contract below, built at the same time.
- **Builds:** §5.1:
  - the section's text per package;
  - the Update line, the command in mono with Copy, the hint;
  - the sidebar's "Hersir X.Y.Z is out", which opens Settings.
- **Acceptance:**
  - [ ] For `aur`, `homebrew`, `scoop` and `package`, every text of §5.1 shows word for word. Copy copies `updateCommand`.
  - [ ] Without `package`, Settings and the notice look exactly as today.
  - [ ] Tests in `src/lib/update.test.ts`, `UpdateNotice.test.tsx` and a new `UpdateSettings.test.tsx`.
  - [ ] `npm test`, `npm run build` and `scripts/ui-test.sh` pass. The summary has a screenshot of Settings → Updates for the AUR from `cage`.

**Contract GA-199 → GA-200.** No new command: `update_status`, `check_for_updates`, `start_update` and the others stay as they are. `UpdateStatus` gets two fields.

```rust
    /// The package manager this Hersir came from: "aur", "homebrew", "scoop", or "package" for a mark Hersir doesn't
    /// know; None for an install script's Hersir or a dev build. A package's Hersir never updates itself.
    pub package: Option<String>,
    /// The command that updates it: "yay -Syu", "brew update && brew upgrade hersir", "scoop update; scoop update hersir";
    /// None for "package" and for a Hersir that isn't a package's.
    pub update_command: Option<String>,
```

```ts
  /** The package manager this Hersir came from ("aur", "homebrew", "scoop" or "package"), or null. */
  package?: string | null;
  /** The command that updates a package's Hersir, or null. */
  updateCommand?: string | null;
```

For a package's Hersir, `installTo` is null and `cannotInstall` is §5.1's Update line for its package, word for word.

### 3. Packages (AUR): `hersir-bin` from the release's Linux build, and `scripts/packages.sh` after the signature

- **Column:** To do. **Role:** backend (the DevOps Agent can take it). **Builds on:** GA-260 (a signed release to test against).
- **Before publishing, it needs:** a release with GA-199 in it, and **Jefsev's AUR account (by hand)**.
- **Builds:**
  - `packaging/aur/hersir-bin/` (§5.3), with its tests against install.sh;
  - `scripts/packages.sh` with the signature gate and the AUR part (§5.6), laid out so Homebrew and Scoop add a file each (`scripts/packages/homebrew.sh`, `scoop.sh`);
  - Packages' Arch job;
  - the AUR part of docs/install.md and the step in docs/RELEASING.md.
- **Acceptance:**
  - [ ] The script refuses a missing `.sig`, a bad signature, a draft or pre-release, and a changed `SHA256SUMS`, writing nothing (tests with a test key).
  - [ ] Against the latest signed release, `makepkg` builds `hersir-bin` as a normal user with nothing installed; namcap's findings are fixed or explained; the files match §3.3's table.
  - [ ] The launcher and desktop entry tests fail when install.sh's change and the package's don't.
  - [ ] The first publish is Jefsev's: the summary gives the exact `scripts/packages.sh vX.Y.Z aur` line in `run_for_me`. After it, https://aur.archlinux.org/packages/hersir-bin shows the version, and Packages' Arch job is green.

### 4. Packages (Homebrew): the tap `Yotech-AI/homebrew-tap` with a `hersir` formula

- **Column:** To do. **Role:** backend. **Builds on:** card 3. It runs at the same time as card 5.
- **Before publishing, it needs:** **the tap repository (Jefsev, by hand)**, and Q1's answer.
- **Builds:**
  - `packaging/homebrew/` (§5.4: the formula, the tap's README and LICENSE);
  - `scripts/packages/homebrew.sh`;
  - Packages' `macos-15` job;
  - the Homebrew part of the docs.
- **Acceptance:**
  - [ ] On `macos-15`, the formula installs the latest signed release from a local tap; `brew test`, `brew audit --strict` and `codesign --verify --deep --strict` pass; `hersir --version` says the release; Hersir.app has `hersir-mcp` inside.
  - [ ] The installed app has no `com.apple.quarantine`.
  - [ ] The caveats show the link line and `brew update && brew upgrade hersir`.
  - [ ] The summary has the check list for Jefsev's Mac: open from Finder without a prompt, the Dock and Spotlight, an upgrade at the next release.
  - [ ] After Jefsev's first `scripts/packages.sh vX.Y.Z homebrew`, `brew install yotech-ai/tap/hersir` works as the docs say.

### 5. Packages (Scoop): the bucket `Yotech-AI/scoop-bucket` with `hersir.json`, and `hersir-mcp.exe` without the VC runtime DLL

- **Column:** To do. **Role:** backend. **Builds on:** card 3. It runs at the same time as card 4.
- **Before publishing, it needs:** **the bucket repository (Jefsev, by hand)**, and a release with this card's build change in it.
- **Builds:**
  - `packaging/scoop/` (§5.5: the manifest, `scripts/hersir-shortcut.ps1` ported from install.ps1, README, LICENSE);
  - `scripts/packages/scoop.sh`;
  - `hersir-mcp.exe` built with the VC runtime inside it, as tauri-build does for `hersir.exe`;
  - Packages' Windows job;
  - the Scoop part of the docs, with Smart App Control.
- **Acceptance:**
  - [ ] `dumpbin /dependents` of Release builds' `hersir-mcp.exe` lists no `VCRUNTIME140.dll`. install.ps1's build still passes Install.
  - [ ] On Windows, the manifest installs the latest signed release:
    - `hersir --version` says it;
    - `hersir-package.txt` says `scoop`;
    - the Start menu shortcut carries `dev.hersir.app` and points into `current`;
    - `hersir.exe` has no `Zone.Identifier`;
    - `scoop uninstall hersir` removes the shortcut and keeps `%APPDATA%\Hersir`.
  - [ ] An update keeps the shortcut and the marker.
  - [ ] After Jefsev's first `scripts/packages.sh vX.Y.Z scoop`, the docs' two commands install Hersir.

---

## 7. Open questions for Jefsev

1. **Homebrew: a formula or a cask?**
   - A cask is quarantined by Homebrew, with no way to turn that off since July 2026. Hersir would then need Open Anyway in System Settings at the first start, and most likely after every upgrade (§3.2).
   - A formula isn't quarantined and opens like the one-line install, but Hersir.app stays in Homebrew's folder and you link it into `~/Applications` once.
   - *Recommendation:* the formula; never a cask that strips the mark.
2. **Who runs the package step after the signature?**
   - *Recommendation:* you, with `scripts/packages.sh vX.Y.Z`, right after signing, so the AUR key stays on your PC (Option A).
   - The DevOps Agent can run the same script later.
   - CI with the AUR key: no.
3. **The AUR account (by hand):** your own account, and the name and address for the PKGBUILD's public Maintainer line? *Recommendation:* your own account, the key only on your PC. Make it soon: nothing reserves `hersir-bin` until its first push.
4. **Create `Yotech-AI/homebrew-tap` and `Yotech-AI/scoop-bucket` (by hand):** public, `main` protected against force pushes, no Actions secrets. *Recommendation:* yes, once cards 4 and 5 are merged.
5. **0BSD for the packaging files** in the AUR repository, the tap and the bucket, as the AUR encourages? *Recommendation:* yes.
6. **Should a package's Hersir announce a release only once it is signed** (§5.2)? *Recommendation:* yes, in GA-199: otherwise Settings says "1.0.1 is out" hours before `yay` can find it.
7. **`libayatana-appindicator` in `depends`** (the tray just works on Omarchy) or `optdepends` (Hersir runs without it)? *Recommendation:* `depends`.
8. **WinGet, Flatpak, and a Debian or RPM repository?**
   - WinGet: later, if Windows users ask.
   - Flatpak: no.
   - Debian or RPM: first attach the `.deb` and `.rpm` to releases; a repository later.
   - *Recommendation:* as said for each.
9. **GitHub's immutable releases?** They'd lock the assets, but today's flow adds the builds and the signature after publishing. *Recommendation:* not now. Look again if releases move to draft, build, sign, then publish.
10. **If Q1 picks the cask: should `brew uninstall --zap` move your data folder to the Trash,** like install.sh's `--purge`? *Recommendation:* yes, as the Trash keeps it recoverable.

---

## 8. Sources

All read on 2026-11-10, most through this run's helpers. Homebrew's source was read at commit `ed4988b`.

| Source | Link |
|---|---|
| Homebrew 5.0.0 (2025-11-12), 6.0.0 (2026-06-11), 7.0.0 (2026-09-13) | https://brew.sh/2025/11/12/homebrew-5.0.0, https://brew.sh/2026/06/11/homebrew-6.0.0/, https://brew.sh/2026/09/13/homebrew-7.0.0/ |
| Homebrew PR #23363, "Remove leftover code for `--no-quarantine`" (merged 2026-07-30) | https://github.com/Homebrew/brew/pull/23363 |
| Homebrew's source: `cask/download.rb`, `cask/quarantine.rb`, `extend/os/mac/cask/quarantine.rb`, `cask/upgrade.rb`, `cask/audit.rb`, `cask/installer.rb`, `deprecate_disable.rb`, `env_config.rb`, `cmd/upgrade.rb` | https://github.com/Homebrew/brew/tree/HEAD/Library/Homebrew |
| Homebrew docs: Acceptable Casks, Acceptable Formulae, Cask Cookbook, How to Create and Maintain a Tap, Tap Trust, Homebrew Security and Supply Chain | https://docs.brew.sh/Acceptable-Casks, https://docs.brew.sh/Acceptable-Formulae, https://docs.brew.sh/Cask-Cookbook, https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap, https://docs.brew.sh/Tap-Trust, https://docs.brew.sh/Homebrew-Security-and-Supply-Chain |
| formulae.brew.sh: `hersir` (formula and cask: 404), Chromium's cask (`fails_gatekeeper_check`, 2026-09-01) | https://formulae.brew.sh/api/formula/hersir.json, https://formulae.brew.sh/api/cask/hersir.json, https://formulae.brew.sh/api/cask/chromium.json |
| GoReleaser, Homebrew Casks (the quarantine hook and its warning) | https://goreleaser.com/customization/publish/homebrew_casks/ |
| Apple developer news, "Updates to runtime protection in macOS Sequoia" (2024-08-06) | https://developer.apple.com/news/?id=saqachfa |
| Apple Support, "Safely open apps on your Mac" (2026-05-27); Mac User Guide 15 and 26, "Open a Mac app from an unknown developer" | https://support.apple.com/en-us/102445, https://support.apple.com/guide/mac-help/open-a-mac-app-from-an-unknown-developer-mh40616/15.0/mac/15.0, https://support.apple.com/guide/mac-help/open-a-mac-app-from-an-unknown-developer-mh40616/26/mac/26 |
| Apple Platform Security, "Gatekeeper and runtime protection" (2024-12-19) | https://support.apple.com/guide/security/gatekeeper-and-runtime-protection-sec5599b66df/web |
| Apple DTS forum answers: threads 706442, 666452, 767774, 694784, 706379 | https://developer.apple.com/forums/thread/706442 (and /666452, /767774, /694784, /706379) |
| macOS Big Sur 11.0.1 Universal Apps release notes (ad hoc signatures on Apple silicon) | https://developer.apple.com/documentation/macos-release-notes/macos-big-sur-11_0_1-universal-apps-release-notes |
| Howard Oakley, eclecticlight.co (secondary): 2024-10-01, 2026-08-06, 2026-08-11 | https://eclecticlight.co/2024/10/01/living-without-notarization/, https://eclecticlight.co/2026/08/06/can-you-disable-gatekeeper-and-xprotect/, https://eclecticlight.co/2026/08/11/how-can-you-run-code-that-hasnt-been-notarised/ |
| ArchWiki through the Kiwix copy of its April 2026 dump (wiki.archlinux.org answered with its bot check): AUR submission guidelines, Arch package guidelines, PKGBUILD, Desktop entries, AUR helpers, Pacman | https://wikipedia.jakami.de/content/archlinux_en_all_maxi_2026-04/AUR_submission_guidelines (and the same path with the other page names) |
| Arch Code of Conduct | https://terms.archlinux.org/docs/code-of-conduct/ |
| aurweb's source: `aurweb/git/auth.py`, `aurweb/git/serve.py` (GitHub copy; gitlab.archlinux.org answered with its bot check) | https://github.com/archlinux/aurweb |
| The AUR's GitHub mirror: no `hersir` branches, 31 for `yay` | https://api.github.com/repos/archlinux/aur/git/matching-refs/heads/hersir |
| archlinux.org's package search (hersir: 0; webkit2gtk-4.1, gtk3, libayatana-appindicator, git, curl, glibc, gcc-libs, libgcc, hicolor-icon-theme, cairo, pango, gdk-pixbuf2, glib2, libsoup3, dbus) | https://archlinux.org/packages/search/json/?name=hersir |
| KSXGitHub/github-actions-deploy-aur (README, build.sh); aurpublish (README) | https://github.com/KSXGitHub/github-actions-deploy-aur, https://github.com/eli-schwartz/aurpublish |
| Tauri 2: AUR, Debian, RPM, Flatpak pages; tauri-build's `lib.rs` on `dev` (static VC runtime) | https://v2.tauri.app/distribute/aur/, https://v2.tauri.app/distribute/debian/, https://v2.tauri.app/distribute/rpm/, https://v2.tauri.app/distribute/flatpak/, https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-build/src/lib.rs |
| yay's README | https://raw.githubusercontent.com/Jguer/yay/next/README.md |
| Scoop wiki: App Manifests, App Manifest Autoupdate | https://github.com/ScoopInstaller/Scoop/wiki/App-Manifests, https://github.com/ScoopInstaller/Scoop/wiki/App-Manifest-Autoupdate |
| Scoop's source: `lib/download.ps1`, `lib/install.ps1`, `lib/shortcuts.ps1`, `lib/autoupdate.ps1`, `lib/depends.ps1`, `lib/core.ps1`, `libexec/scoop-update.ps1`, `libexec/scoop-uninstall.ps1` | https://github.com/ScoopInstaller/Scoop |
| Scoop's BucketTemplate (README, Excavator), GithubActions, Main and Extras (`hersir.json`: 404; `vcredist2022.json`) | https://github.com/ScoopInstaller/BucketTemplate, https://github.com/ScoopInstaller/GithubActions, https://github.com/ScoopInstaller/Main, https://github.com/ScoopInstaller/Extras |
| Microsoft Learn: about_Execution_Policies (7.6, 2026-08-31), Microsoft Defender SmartScreen (2026-04-23), SmartScreen reputation for app developers (2026-05-04), IAttachmentExecute | https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_execution_policies, https://learn.microsoft.com/en-us/windows/security/operating-system-security/virus-and-threat-protection/microsoft-defender-smartscreen/, https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation |
| Microsoft Support, "What is Smart App Control?" | https://support.microsoft.com/en-us/topic/what-is-smart-app-control-285ea03d-fa88-4d56-882e-6698afdb7003 |
| The Rust reference, Linkage (static and dynamic C runtimes) | https://doc.rust-lang.org/reference/linkage.html |
| WinGet: manifest docs, schema 1.12.0 `installer.md`, repository and policies pages, settings, spec #182, `DownloadFlow.cpp`; Komac, winget-releaser, winget-create READMEs | https://learn.microsoft.com/en-us/windows/package-manager/package/manifest, https://github.com/microsoft/winget-pkgs/tree/master/doc/manifest/schema/1.12.0, https://learn.microsoft.com/en-us/windows/package-manager/package/repository, https://learn.microsoft.com/en-us/windows/package-manager/package/windows-package-manager-policies, https://github.com/microsoft/winget-cli, https://github.com/russellbanks/Komac, https://github.com/vedantmgoyal9/winget-releaser |
| Flathub requirements and linter; Flatpak sandbox permissions | https://docs.flathub.org/docs/for-app-authors/requirements, https://docs.flathub.org/docs/for-app-authors/linter, https://docs.flatpak.org/en/latest/sandbox-permissions.html |
| Debian wiki, DebianRepository/UseThirdParty (2025-11-02); openSUSE Build Service package formats; Cloudsmith's open-source policy (search snippet only) | https://wiki.debian.org/DebianRepository/UseThirdParty, https://openbuildservice.org/help/manuals/obs-user-guide/cha-obs-package-formats, https://help.cloudsmith.io/docs/open-source-hosting-policy |
| ssh-keygen(1); OpenSSH 8.1 and 8.2 release notes; runner images (ubuntu-24.04, macos-15, windows-2025); Git for Windows release notes | https://man.openbsd.org/ssh-keygen, https://www.openssh.org/txt/release-8.1, https://www.openssh.org/txt/release-8.2, https://github.com/actions/runner-images, https://raw.githubusercontent.com/git-for-windows/build-extra/main/ReleaseNotes.md |
| GitHub docs: events that trigger workflows (release, schedule), triggering a workflow (GITHUB_TOKEN), workflow and repository dispatch, environments, rulesets; the immutable releases changelog (2025-10-28) | https://docs.github.com/en/actions/writing-workflows/choosing-when-your-workflow-runs/events-that-trigger-workflows, https://docs.github.com/en/actions/writing-workflows/choosing-when-your-workflow-runs/triggering-a-workflow, https://docs.github.com/en/actions/managing-workflow-runs-and-deployments/managing-deployments/managing-environments-for-deployment, https://github.blog/changelog/2025-10-28-immutable-releases-are-now-generally-available/ |

**Not read, so marked [I] where used:**
- Homebrew's formula download and bottle code: are formulae ever quarantined?
- Whether a `run` step in Homebrew's sandbox can run `xattr` or `codesign`.
- Apple's TN3127: ad hoc designated requirements.
- Microsoft's statement that SmartScreen keys on the mark of the web.
- A search of Scoop's whole repository for `Zone.Identifier`.
- tauri-build 2.7.1 itself, rather than `dev`.
- What `$bucket` holds in a Scoop hook.
- The AUR's own API, for the name.
- The Dock and Spotlight with a linked keg app.
