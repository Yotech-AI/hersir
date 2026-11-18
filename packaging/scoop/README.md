# Hersir for Scoop

[Hersir](https://hersir.dev) on Windows with [Scoop](https://scoop.sh): clients, projects and tasks, worked on by local AI agents. This bucket has one app, `hersir`: the ready-made Windows build (x86_64) of Hersir's latest signed release, the one Hersir's one-line install installs.

## Install

```powershell
scoop bucket add hersir https://github.com/Yotech-AI/scoop-bucket
scoop install hersir/hersir
```

What it installs, for you alone:

- `hersir.exe` and `hersir-mcp.exe` (the helper the Chief's chat uses), exactly as the release built them, in `~\scoop\apps\hersir\current`, and the commands `hersir` and `hersir-mcp`. `hersir` starts Hersir, and `hersir --version`, `--help` and `--backup` print in PowerShell, cmd and Git Bash: the command is a small PowerShell script beside the programs, `hersir.ps1`, as Scoop's own shim would show nothing of what Hersir, a window app, prints.
- `hersir-package.txt` beside them, which says `scoop`. With it, Hersir doesn't update itself: Scoop does.
- Hersir in the Start menu (Scoop Apps → Hersir), which starts Hersir from Scoop's `current` folder and carries Hersir's app ID, `dev.hersir.app`, so Windows shows Hersir's notifications.

Hersir also needs Git for Windows, WebView2 (Windows 11 has it) and Claude Code or Codex, installed and logged in: [Hersir's install guide](https://github.com/Yotech-AI/hersir/blob/production/docs/install.md#what-every-system-needs) says more. Neither program needs the Visual C++ Redistributable.

Use one way to install Hersir, not two: to move from the one-line install to Scoop, first remove the one line's install with its `-Uninstall`, which keeps your data.

## Update

Quit Hersir first (Quit Hersir completely, in its tray menu): Scoop doesn't update a program that runs. Then:

```powershell
scoop update; scoop update hersir
```

`scoop update` gets the bucket's latest manifest first: `scoop update hersir` alone does that only every three hours. Hersir still asks GitHub for new releases and tells you when one is out, and Settings → Updates shows this command. The bucket gets a release only once it is signed, usually on the same day.

## Uninstall

```powershell
scoop uninstall hersir
```

It removes the programs, the commands and the Start menu shortcut. Your data stays in `%APPDATA%\Hersir`, also with `scoop uninstall hersir --purge`, as Scoop keeps none of it.

## SmartScreen and Smart App Control

Hersir has no Microsoft signature yet. Scoop doesn't mark what it downloads as coming from the internet (no `Zone.Identifier` stream), so SmartScreen shouldn't stop Hersir. Smart App Control does: on a Windows 11 PC that has it on, Windows blocks programs without a signature, however they were installed, and it has no exception for one app.

## How this bucket is made

`bucket/hersir.json` comes from Hersir's own repository, [`packaging/scoop`](https://github.com/Yotech-AI/hersir/tree/main/packaging/scoop), with `scripts/hersir-shortcut.ps1` (the Start menu shortcut), this README and the licence. For each release, Jeffrey runs `scripts/packages.sh` there on their own computer, right after signing the release. It checks the release's signature, takes the zip's sha256 from the release's signed `SHA256SUMS`, fills in the version, the address and the sha256, and pushes the files here. An unsigned release never reaches this bucket. The manifest has no `checkver` or `autoupdate`, so nothing else changes its version. Changes and issues go to [Yotech-AI/hersir](https://github.com/Yotech-AI/hersir).

## Licence

The files in this bucket are under the BSD Zero Clause License (0BSD, [`LICENSE`](https://github.com/Yotech-AI/scoop-bucket/blob/main/LICENSE)). Hersir itself is under the Apache License 2.0.
