# A sandbox for every coding CLI (GA-265)

The plan for GA-265, with what was built and what is left. The audit's finding (GA-247 #2): the default allowed
commands (`npm`, `npx`, `node`, `composer`, `php`, `cargo`, `pytest`, `./vendor/bin/*`) run code that can read and change
anything your account can. Jefsev, 25 Nov 2026: build it now, for every coding CLI.

## 1. The sandbox

**Engine: Hersir's own, in Rust (`crates/gizai-agents/src/sandbox.rs`), wrapped around the CLI where Hersir starts it
(`process::spawn_exec`):** `[systemd-run --user --scope …] bwrap … -- <cli> <args>` on Linux, `sandbox-exec -p <profile>
-- <cli> <args>` on macOS. The whole process tree is inside, whatever the CLI's own sandbox does.

- **Linux: bubblewrap.** A mount namespace in which `/` is read only, hidden places are empty folders (or `/dev/null`),
  and writable places are bound back; a PID namespace with its own `/proc`, so a run sees no other process and can't
  reach Hersir's environment or files through `/proc/<pid>`. Needs the `bubblewrap` package and user namespaces.
- **macOS: Seatbelt** (`/usr/bin/sandbox-exec`, part of macOS) with a profile Hersir writes per run: everything allowed
  but writing, then each place, deepest last (in Seatbelt the last matching rule wins). Apple Events and Launch Services
  (`osascript`, `open`) are refused, as they would run things outside.
- **Not Anthropic's sandbox-runtime:** it needs Node.js and `socat` at run time, and its network proxy, its main extra,
  isn't wanted (the network stays as it is).
- **Not Landlock alone:** it only allows (to hide `~/.ssh` in a readable home it would have to list every other
  folder), and on today's kernels it doesn't stop connecting to your session's Unix sockets: `systemd-run --user` over
  D-Bus would run anything outside the sandbox.

**What a run may touch** (`sandbox::policy`):

| | |
| --- | --- |
| Read and change | the worktree, the agent's change folders, the run's temp folder (on Linux also `/tmp`), what a commit writes in the git folder (`worktree::commit_dirs`), its own CLI's folders (`~/.claude` + `~/.claude.json`, or CLAUDE_CONFIG_DIR; `~/.codex` or CODEX_HOME; `~/.gemini`; OpenCode's four), the package caches (`~/.npm`, `~/.cache`, `~/.cargo/registry` and `git`, pnpm, Yarn, Bun, Go, Gradle, Maven) |
| Read only | everything else; the agent's read folders; from the hidden data folder the run's MCP config, Gemini settings file and Hersir's socket |
| Hidden | Hersir's data folder (database, logs, MCP configs, other cards' worktrees), the other runs' temp folders, `SECRET_PLACES` (`~/.ssh`, `~/.gnupg`, `~/.aws`, `~/.azure`, `~/.kube`, `~/.docker`, `~/.config/gh`, `~/.netrc`, `~/.git-credentials`, keyrings, browser profiles, …), the other CLIs' folders; Linux: `$XDG_RUNTIME_DIR`, `/run/dbus`, Docker's, Podman's and containerd's sockets |
| Network | as today |

Sandboxed runs get `rerere.enabled=false`, `maintenance.auto=false` and `gc.auto=0` in their git settings: git's upkeep
after a commit would write the git folder's `packed-refs`, which the sandbox refuses (the commit itself works).

**A fresh install:** Linux needs `bwrap` (Fedora and Ubuntu desktops usually have it through Flatpak or GNOME; Arch,
Debian and servers need `bubblewrap`). Ubuntu 24.04 and newer allow it user namespaces through the package's AppArmor
profile. `./install.sh --check` names the command for the system; the check doesn't fail the install. macOS needs nothing.

**Windows:** no sandbox (AppContainer and restricted tokens would need every CLI to cope with a different user's view of
the disk; WSL2 is out of scope). Each run says "No sandbox on Windows".

**Each CLI's sign-in and settings:** a run gets its own CLI's folders, read and change, and none of the others' (by kind;
an Other CLI by its program's name, `opencode`). The environment was done in GA-266. Not covered: the macOS keychain,
which Seatbelt can't split by item (Claude Code reads its sign-in there through `security`, which can read other items
made by `security` too), and a second account's folder of another CLI entry.

**When the sandbox can't start:** the run doesn't start, and the card goes on hold "blocked" with what to install on that
system (`sandbox::missing_bubblewrap`, `bubblewrap_fails`). **Settings → Coding CLIs → Run agents without a sandbox on
this computer** (off by default) starts runs without it, and each says so.

**The CLIs' own sandboxes:** Codex 0.163's `workspace-write` (itself bubblewrap on Linux) works nested inside Hersir's,
checked on this computer with `codex sandbox` inside bwrap, so it stays. Claude Code and Gemini run in Hersir's alone.

## 2. Narrower default commands

Not built: with the sandbox, `npm:*` can no longer reach Hersir's data or your keys, which was the reason. Narrowing
`npm:*` to `npm test:*`, `npm run …:*` and `npm ci:*` would break `npm install <package>` (adding a dependency), `npm ls`,
`npm view`, `npx <tool>` (`npx tsc`, `npx vitest`), `composer require` and `composer dump-autoload`, and `cargo add`,
`cargo fmt` and `cargo clippy` for `cargo:*`. A later card can narrow them where no one needs the rest.

## 3. Tests that prove it

The pure parts on every system: `policy` (what is hidden, writable and read only, links followed, the own CLI's folders
by kind and by program name), `bwrap_args` (order: a deeper place last; hidden folders made read only; no
`--new-session`), `seatbelt_profile`, `install_bubblewrap` per `/etc/os-release`, and the setting's save and load. On
Linux with bwrap, behind `#[cfg(target_os = "linux")]` and skipped without it: a fake Claude Code, Codex and Gemini run
in the sandbox (`AppState::sandbox` on) whose `node -e` can't read a fake data folder's database, `~/.ssh/id_ed25519` (a
fake `HOME`) or another card's worktree, can't write outside its places, and can commit. A run with the switch on, and on
a system without bwrap (`PATH` without it), the card on hold with the sentence.

A smoke check of the built code on Linux (bubblewrap 0.11, kernel 7.2) did that by hand: every read above refused, writes
outside refused, `git commit` in the worktree worked, two processes visible.

## Open: pushing and pull requests

The sandbox hides `~/.ssh` and gh's login, so in a run an agent's own `git push` (over SSH with your keys, or HTTPS with
gh) and the QA Agent's `gh pr create` can't sign in. Hersir still pushes the branch after each run. Proposal, a card of
its own: Hersir relays them. The run's git gets a `GIT_SSH_COMMAND` (and gh a helper) that asks Hersir over its socket,
with the run's token; Hersir checks it is the project's own repository and branch, and pushes, or opens the pull
request, outside the sandbox with your keys.
