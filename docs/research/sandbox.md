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
| Read and change | the worktree, the agent's change folders, the run's temp folder (on Linux also `/tmp`; the run's MCP config is in it), what a commit writes in the git folder (`worktree::commit_dirs`), its own CLI's folders (`~/.claude`, or CLAUDE_CONFIG_DIR; `~/.codex` or CODEX_HOME; `~/.gemini`; OpenCode's data, state and cache) but for their settings, the package caches (`~/.npm`, `~/.cache`, `~/.cargo/registry` and `git`, pnpm, Yarn, Bun, Go, Gradle, Maven) |
| Read only | everything else; the agent's read folders; the own CLI's settings, instructions, hooks, plugins, skills and MCP servers (`cli_settings`: `~/.claude.json`, `~/.claude/settings.json`, `~/.claude/plugins`, `~/.codex/config.toml`, `~/.gemini/settings.json`, `~/.config/opencode`, …), so a run can't leave a program there for your own sessions; from the hidden data folder the run's Gemini settings file and Hersir's socket |
| Hidden | Hersir's data folder (database, logs, MCP configs, other cards' worktrees), the other runs' temp folders, `SECRET_PLACES` (`~/.ssh`, `~/.gnupg`, `~/.aws`, `~/.azure`, `~/.kube`, `~/.docker`, `~/.config/gh`, `~/.netrc`, `~/.git-credentials`, keyrings, browser profiles, …), the other CLIs' folders; Linux: `$XDG_RUNTIME_DIR`, `/run/dbus`, Docker's, Podman's and containerd's sockets |
| Network | as today |

Sandboxed runs get `rerere.enabled=false`, `maintenance.auto=false` and `gc.auto=0` in their git settings: git's upkeep
after a commit would write the git folder's `packed-refs`, which the sandbox refuses (the commit itself works).

**A fresh install:** Linux needs `bwrap` (Fedora and Ubuntu desktops usually have it through Flatpak or GNOME; Arch,
Debian and servers need `bubblewrap`). Ubuntu 24.04 and newer allow it user namespaces through the package's AppArmor
profile. `./install.sh --check` names the command for the system; the check doesn't fail the install. macOS needs nothing.

**Windows:** no sandbox (AppContainer and restricted tokens would need every CLI to cope with a different user's view of
the disk; WSL2 is out of scope). Each run says "No sandbox on Windows".

**Each CLI's sign-in and settings:** a run gets its own CLI's folders, read and change but for their settings (read only),
and none of the others' (by kind; an Other CLI by its program's name, `opencode`). The environment was done in GA-266.
Not covered: the macOS keychain, which Seatbelt can't split by item (Claude Code reads its sign-in there through
`security`, which can read other items made by `security` too; `~/Library/Keychains` stays readable for that), a settings
file that isn't there yet (a run could make it), and a second account's folder of another CLI entry.

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

## 4. Agents' features in the sandbox (Jefsev, 26 Nov)

Checked on Linux (kernel 7.2, bubblewrap 0.11) with the built code (`sandbox::policy`, `Engine::wrap`, `run_env`), a
stand-in data folder, socket and runtime folder, and your home folder for the caches and browsers. The CLIs ran with a
scratch home and no keys, so nothing was spent and nothing written in `~/.claude`. `cargo run -p gizai-agents --example
sandbox_check` repeats the main checks anywhere; all pass here.

| # | Feature | In the sandbox | Fixed in this card |
| --- | --- | --- | --- |
| 1 | Hersir's socket | Reachable: it comes back read only in the hidden runtime folder. GA-266's check passes: the kernel gives Hersir the process's own pid, which is in the run's process group. Task runs don't get Hersir's MCP server yet (chat turns do, outside the sandbox). | — |
| 1 | The run's MCP config | Was still readable after Hersir deleted it (GA-266): a file brought in on its own keeps its content until the run ends. | In the run's temp folder when the run is sandboxed: gone in the sandbox once deleted. |
| 2 | Remote MCP servers (Figma's, Trello's) | `https://mcp.figma.com/mcp` and Trello's API answer (405, 404): names, TLS and the network work. The token in the run's MCP config reaches the CLI. | — |
| 3 | Local MCP servers | `npx` (server-sequential-thinking 2.2.0 answered `initialize`), a program of its own, and a server on 127.0.0.1 (Figma's desktop MCP) work. `uvx` failed: uv makes its tool folder in `~/.local/share/uv/tools`. | `UV_TOOL_DIR`, `UV_TOOL_BIN_DIR` in the run's temp folder (`run_env`); downloads stay in `~/.cache/uv`. |
| 4 | The browser | Chrome DevTools MCP 1.10.1 with Chromium 143 (Hersir's browser: `--headless --isolated`, its profile in TMPDIR) ended at the start: Chromium stops when it can't make its crash-report folder in the read-only `~/.config`. `/dev/shm` works (bubblewrap's own `/dev`), and so does Chromium's own sandbox, nested. Playwright's headless shell worked as it was. | `BREAKPAD_DUMP_LOCATION` in the run's temp folder: `new_page` and `take_screenshot` work. |
| 4 | A visible window | No display: Wayland's socket is in the hidden runtime folder, X11's `/tmp/.X11-unix` isn't in the run's `/tmp`, and X11's abstract socket was refused here. Hersir's browser is always headless (`browser::safe`), so its setting doesn't change; a server you add that opens a window (Playwright MCP without `--headless`) fails in a run. | — |
| 5 | Web search and fetch | The network works (2): Claude Code's WebSearch runs at Anthropic, WebFetch fetches from the run. | — |
| 5 | Each CLI's sign-in | Claude Code 2.1.325, Codex 0.163 and Gemini 0.65 each started in their sandbox and got to signing in (an init line and a $0 "not logged in" result; a 401 from OpenAI; Gemini asking for an auth method). With your home: `~/.claude/.credentials.json` and `~/.claude/projects` read and change, `~/.codex` and `~/.gemini` not there for a Claude Code run. Claude Code doesn't update `~/.claude.json` in the sandbox (it saves it through a new file in the home folder); the runs went on without it. | The own CLI's settings are read only (`cli_settings`); no CLI minded. |
| 5 | macOS: keychain and DNS | Not run here. The profile refuses only Apple Events and Launch Services, not the keychain (`com.apple.SecurityServer`) or DNS (`mDNSResponder`). But it hid `~/Library/Keychains`, where a program reads its keychain file itself: Claude Code couldn't have signed in. | `~/Library/Keychains` isn't hidden. `sandbox_check` checks the keychain, DNS and headless Chrome on macOS. |

To be checked:
- **macOS in CI.** This run couldn't push a change to `.github/workflows/ci.yml` (its GitHub token can't change
  workflows), and CI runs only on a pull request. The step, for the test job after `cargo build --workspace`, with
  `bubblewrap` added to the Linux packages:

  ```yaml
  - name: The sandbox for runs
    if: steps.version.outputs.only != 'true' && runner.os != 'Windows'
    continue-on-error: true
    env:
      HERSIR_CHECK_DIR: /tmp/hersir-sandbox-check
    run: cargo run -q -p gizai-agents --example sandbox_check
  ```

  Ubuntu 24.04 keeps user namespaces from programs without an AppArmor profile of their own. If bubblewrap can't start
  there, users of Ubuntu 24.04 hit the same, and its sentence says what to check.
- **A real run of each CLI** with its own sign-in. Its folders are bound in as they are, so it should sign in as before.
- **A gap found on the way:** on an X server that listens on an abstract socket (Xorg does unless told not to), a run can
  reach it through the shared network. It could then open windows and send keys to your other X11 programs. Linux 6.12's
  Landlock can refuse abstract sockets outside the sandbox: a later card, on top of bubblewrap.

What can't work inside: pushing and pull requests (your SSH keys and gh's login are hidden), a visible browser window,
and an MCP server that needs your session (D-Bus, the keyring, Docker) or a folder the sandbox hides.

### Proposal: keeping those features

1. **Hersir does it for the run, over its socket.** A run asks Hersir to push its card's branch, or to open or update
   that card's pull request (option 1a). Hersir checks the asking process is in the run's process tree (GA-266) and that
   it is the run's own card and the project's own repository, and does it with your keys, as it does after a run now.
   The DevOps Agent needs more (tags, releases, merges): until this covers those, its runs go without the sandbox,
   marked on the run.
2. **Settings → MCP servers, per server: "Runs outside the sandbox"**, off by default, for servers you add. Hersir starts
   that server itself, outside the sandbox, as it starts a run's preview, and ends it with the run. The run reaches it
   locally: a server that speaks HTTP on 127.0.0.1, with a token for that run in the run's MCP config (127.0.0.1 works in
   the sandbox, item 3); a stdio server through Hersir's socket, like Hersir's own shim. The run's log says
   "<server> runs outside the sandbox". This is for a visible Playwright browser, or a server that needs your keyring or
   Docker. Figma's desktop MCP is outside already: it is the Figma app. What such a server can do, the run can ask it to
   do. So it is your choice, per server, and the run says so.

## Open: pushing and pull requests

The sandbox hides `~/.ssh` and gh's login, so in a run an agent's own `git push` (over SSH with your keys, or HTTPS with
gh) and the QA Agent's `gh pr create` can't sign in. Hersir still pushes the branch after each run. Proposal, a card of
its own: Hersir relays them. The run's git gets a `GIT_SSH_COMMAND` (and gh a helper) that asks Hersir over its socket,
with the run's token; Hersir checks it is the project's own repository and branch, and pushes, or opens the pull
request, outside the sandbox with your keys.
