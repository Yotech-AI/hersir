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
