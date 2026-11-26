# Tools per agent

Each agent gets the commands its role allows, the folders you give it, and the tools you switch on for it.

## Off until you switch it on

The agent form → Tools has MCP servers, web search, fetching pages, a hidden browser for testing web pages, and the CLI's own built-in tools. Everything is off until you switch it on per agent, with a switch per server and per tool.

Hersir builds no tools of its own: each one is the CLI's own or comes from an MCP server. A web or built-in tool named in an agent's allowed commands is left out of its runs.

## MCP servers

Agents can use outside services through MCP servers, like Otus OS. Settings → MCP servers holds one list for all agents.

- Add a server by hand (a command, or an address), or **Import from Claude Code**, which reads the servers your Claude Code accounts already have: read only, with the values going into the keychain.
- **List tools** shows what each tool does, its parameters and its risk, before you switch anything on.
- A tool whose name isn't a plain tool name (letters, digits, `_` and `-`, at most 64 characters, not starting with `mcp__`) is never allowed: it shows as "left out: its name isn't a plain tool name".
- A server that asks for it gets **Sign in**: Hersir signs in as a device of its own, in your default browser.
- Environment values, headers and sign-in tokens stay in the OS keychain, never in Hersir's database.

## The web

**Web** has two switches: search the web, and fetch pages (any, or only the domains you list).

- Claude Code gets `WebSearch` and `WebFetch`.
- Codex gets its own web search.
- Gemini fetches pages; its web search is on by its own policy.

## A hidden browser

**Browser** is a hidden Chrome for testing web pages, through Chrome DevTools MCP, the built-in entry at the top of Settings → MCP servers.

- It always runs headless with a throwaway profile: never on your screen, never your own browser, profile or logins.
- It needs Node 20.19 or newer with npx, and Google Chrome or Chromium (never Brave). The form says what is missing.
- Hersir installs none of it: `npx` fetches the pinned server, and the server starts the browser.

MCP servers and the browser work on Claude Code. Codex and Gemini agents show them disabled, with why. A Gemini run starts no MCP server at all, not even one from your own Gemini settings.

## Built-in tools

**Built-in tools** lists what the agent's CLI offers: Hersir's catalog merged with what the CLI itself reports. For Claude Code that is its list from the agent's last run, or **Ask Claude Code again**, which starts it without a login. Each tool says what it allows and how risky it is; a few, like Read and Grep, are always on.

**Slash commands and skills**, in Built-in tools, lets a Claude Code agent's task runs use your slash commands and skills, like `/design`, plugins' too.

- Medium risk: they can steer the run, so switch it on only for an agent that needs one.
- On, its runs go without `--disable-slash-commands` and get `SlashCommand` and `Skill`. Hooks stay off, and settings still come only from your user settings.
- The Chief's chat never gets them, and Codex and Gemini agents show the switch disabled, with why.

## Allowed commands

A new agent starts with its role's allowed commands.

- Hersir's default list has the usual git, package manager and test commands, the read-only helpers agents use in pipes (`head`, `tail`, `wc`, `sort`, `uniq`, `cut`, `diff`, `grep`, `jq`, `pwd`, `which` and `tree`), and `sleep`, to wait between checks. The Chief gets that list.
- Builders (Backend, Frontend, Design and your own roles) also get `git push`, `git pull`, `git fetch`, a few read-only git commands, `node`, `echo` and `printf`.
- QA gets the builders' list plus `gh pr create`, `list`, `view` and `edit`.
- DevOps gets a list of its own for releases.

An agent without a list runs with Hersir's default list. An agent with its own list needs `Bash(sleep:*)` added to it to wait between checks.

Each entry is one rule: a tool name, optionally with one `( … )` after it, like `Bash(npm test:*)`. Claude Code reads rules separated by spaces or commas, so `Bash(ls:*) Bash(*)` on one line would be two rules, the second any command. Saving refuses such an entry: "`<entry>` isn't one rule: write each command as its own entry, like Bash(npm test:*)." One saved before Hersir checked this is left out of runs (the run log says so), and the agent form shows it with that sentence.

The Chief can't give an agent a shell, an interpreter or a program by its path, like `Bash(bash:*)`, `Bash(sh -c:*)`, `Bash(python3:*)`, `Bash(env:*)`, `Bash(sudo:*)` or `Bash(/bin/sh:*)`, unless the agent has that entry already or its role's list has it. Only you add one, in the agent form.

### Commands that run code

`npm`, `npx`, `node`, `php`, `composer`, `cargo`, `pytest` and `./vendor/bin/*` are on the lists, and they run code: the project's scripts, tests and builds. Claude Code only checks the command line, not what the command then does. So on Linux and macOS every run starts in Hersir's sandbox (below), which limits what that code can reach.

### The sandbox

Hersir starts each task run's CLI, Claude Code, Codex, Gemini or any other, in a sandbox, and so everything the CLI starts: its tools, the project's scripts and tests, a `node -e`.

| | In the sandbox |
| --- | --- |
| Read and change | The card's worktree, the agent's folders, the run's temp folder (also `/tmp` on Linux), what a commit writes in the repository's git folder, the run's own CLI's sign-in and sessions (`~/.claude`, `~/.codex`, `~/.gemini`, OpenCode's), and the package caches (`~/.npm`, `~/.cache`, Cargo's registry, …) |
| Read only | The run's own CLI's settings, instructions, hooks, plugins and MCP servers (`~/.claude.json`, `~/.claude/settings.json`, `~/.codex/config.toml`, `~/.gemini/settings.json`, …), so a run can't leave a program there for your own sessions |
| Read only | The rest of your computer |
| Hidden | Hersir's data folder (its database, the runs' logs and MCP configs, the other cards' worktrees), the other runs' temp folders, your keys and logins (`~/.ssh`, `~/.gnupg`, `~/.aws`, `~/.config/gh`, `~/.netrc`, `~/.git-credentials`, the browsers' profiles, …), the other CLIs' folders, and on Linux your session's sockets (`$XDG_RUNTIME_DIR`: systemd, D-Bus, the keyring) and Docker's |
| Network | As it is |

Hersir's socket comes through, read only, and the run's MCP config is in its temp folder until the CLI has read it. A run sees no other process on Linux.

The agents' features work in it: Hersir's socket, remote MCP servers (Figma's, Trello's), servers a run starts with `npx`, `uvx` or a program of its own, a server on 127.0.0.1 (Figma's desktop app), the hidden browser, web search and fetch, and each CLI's own sign-in. Chrome's crash reports and uv's tools go in the run's temp folder. `cargo run -p gizai-agents --example sandbox_check` shows what the sandbox allows on your computer.

- **Linux:** bubblewrap (`bwrap`). Install the `bubblewrap` package (`./install.sh --check` names the command for your system). It needs user namespaces, which are on by default.
- **macOS:** Seatbelt, which comes with macOS.
- **Windows:** no sandbox. Each run says "No sandbox on Windows".
- **When the sandbox can't start** (Linux without bubblewrap, say), the run doesn't start: the card goes on hold "blocked" with what to install. **Settings → Coding CLIs → Run agents without a sandbox on this computer** (off by default) starts runs without it; each such run says so.
- **The CLIs' own sandboxes:** Codex keeps its `workspace-write` sandbox inside Hersir's. Claude Code and Gemini have only Hersir's.

What the sandbox doesn't cover:
- **Pushing and pull requests:** your SSH keys and gh's login are hidden, so an agent's own `git push` and `gh pr create` can't sign in in the sandbox. Hersir pushes a card's branch after its run, as before.
- **The network** is open: what the run can read it can send anywhere.
- **The package caches** (`~/.npm`, `~/.cache`, Cargo's registry, …) are shared: a package a run changes in one can reach your own later builds, and other projects' runs.
- **A window:** a run has no display, so a browser or app it starts must be headless. Hersir's browser always is.
- **X11:** on an X server that listens on an abstract socket (Xorg does unless told not to), a run can still reach it, and with it your other X11 programs.
- **Its own CLI's folder** is writable but for its settings, and a settings file that isn't there yet can be made.
- **macOS:** the keychain stays reachable (Claude Code signs in from it), and macOS's sandbox isn't tested on every version.
- **Places made after the run started**, and keys or logins kept elsewhere than the places above.
- **Chat turns** (the Chief) aren't in the sandbox; they get only Hersir's own tools.

## Folders

Besides its worktree, an agent can have folders of its own, each set to read or to read and change (agent form → Permissions). They limit the agent's file tools, and in the sandbox the commands it runs too. `/`, your home folder, Hersir's data and folders with keys are refused.

## The rules of a run

Agent runs are headless: nobody is there to approve anything while one runs, so a command or tool call that needs approval is refused.

### A temp folder per card

Each card's runs get a short temp folder outside its worktree, Hersir's data folder and any git repository: `/var/tmp/gizai-<uid>/<card>-<hash>`, like `/var/tmp/gizai-1000/DSO-30-1a2b3c4d` (on Windows `%TEMP%\gizai\<card>-<hash>`). Before every run Hersir makes it, empty, and sets `TMPDIR`, `TMP` and `TEMP` to it for every CLI. So `mktemp`, PHP's `sys_get_temp_dir()`, Node's `os.tmpdir()`, Python's `tempfile` and the agents' browser write there.

- It is short: on Linux and macOS at most 50 characters for any card name, so a Unix socket in it fits. Chromium makes its socket at `$TMPDIR/org.chromium.Chromium.XXXXXX/SingletonSocket`, and a socket path must be shorter than 108 bytes (104 on macOS): in the worktree, the agents' browser couldn't start.
- The hash comes from the worktree's path, so worktrees with the same name in different data folders get different folders.
- It is in `/var/tmp`, on disk: `/tmp` is in memory on many systems (Arch, Fedora), and files there would count toward the runs' memory limit.
- Claude Code gets it with `--add-dir`, Codex as a writable root and Gemini with `--include-directories`, so the agent may read and change it.
- Hersir makes `/var/tmp/gizai-<uid>` with mode 0700. One that is already there is used only when it is a real folder (not a link), yours and closed to others. Otherwise, or when the folder can't be made, the run uses the worktree's `.gizai-tmp/` instead, and its log says why.
- The folder is emptied when the run ends, also after Stop or a limit, and removed with the card's worktree. Links in it are removed, never followed.

The worktree's own `.gizai-tmp/` stays for the screenshots and the preview:

- Hersir adds `/.gizai-tmp/` once to the repository's `.git/info/exclude`, which your checkout and every worktree share. So the folder never shows in `git status`, and `.gitignore` is left alone.
- It is emptied when the run ends, also after Stop or a limit, and goes with the worktree.

### How this run works

Every task prompt, new or continued, ends with "How this run works": nobody can approve anything, the commands the agent may run (its allowed list), the shell rules of its CLI, the temp folder's path, and that Hersir pushes the branch when the run ends.

For Claude Code in acceptEdits mode (the default), checked against Claude Code 2.1.289:

- the command lines and file tools it allows reach only the worktree, the agent's folders and the run's temp folder: `ls /tmp`, `ls ..`, a redirect to `/tmp` and the Write tool on `/tmp` are refused, also for an allowed command. That is the command line only: a command that runs code, like `npm test`, runs the project's scripts, which can read and change anything your user can ([Commands that run code](#commands-that-run-code));
- `$(…)`, backticks, variables such as `$TMPDIR` and heredocs with an unquoted delimiter (`<<EOF`) are refused;
- pipes, `2>&1`, `&&` and `;` between allowed commands work, and so does a redirect to a file in the worktree.

Codex and Gemini hear only what holds for them. Codex asks for nothing and its sandbox blocks what it doesn't allow; Gemini hears its allowed commands.

### Waiting in a run

Ending its message ends an agent's run, and nothing wakes it up later. So "How this run works" also says how to wait for something outside the run, like a CI run, a release or deploy workflow or a pull request's checks.

- Check it in the foreground about once a minute: one check, then `sleep 45`, in one command, and again.
- When it won't be done before the run's limit, end with the result line and say what is left.
- `sleep` is named only for an agent that may run it.

Checked against Claude Code 2.1.289: a command that starts with a sleep of more than 20 seconds is blocked, and most shell loops are refused. A command that runs more than 2 minutes (or its own timeout, at most 10) is moved to the background, and a background command is stopped when the message ends.

### Refused in this run

Claude Code reports every tool call it refused, with the reason. Hersir saves them on the run:

- the Run panel lists them under "Refused in this run";
- Show output marks each one where it happened;
- the Chief's `get_task` and `get_agent` return them for each run.

Codex and Gemini don't report refusals, so their runs list none.

## A repository's own settings

A repository's own files and git settings never start a program outside an agent's allowed commands or its sandbox.

- **Hersir's own git runs no hooks.** When Hersir commits (Commit… on your card), stashes, fetches, pushes or makes and removes a worktree, git runs none of the repository's hooks and no `core.fsmonitor` program. Hersir changes no setting in the repository: your own `git commit` there still runs its hooks.
- **Codex writes only what a commit writes.** Besides the worktree, its folders and the temp folder, a Codex run may write `objects`, `refs`, `logs` and its own `worktrees/<name>` in the repository's git folder. Never the git folder's `config`, `hooks` or `info`, or another worktree's folder. Its agent can still `git add`, `git commit`, `git push` and `git fetch`.
- **No Gemini run in a worktree with a `.gemini` folder.** Gemini would load the settings in it, which can start MCP servers, hooks and other programs. The card goes on hold, blocked, with what to do: remove the folder from the branch, or run the card on Claude Code or Codex.
- **Gemini gets no extensions and no MCP server but its own.** Not even one from your own Gemini settings.

[docs/agent-tools.md](../agent-tools.md#a-repositorys-own-settings) has the details.

## Safety

Once a chat used an outside tool (an MCP server, the web, the browser), for the rest of that chat the Chief's calls that start runs, change agents or columns, save to memory, merge, comment, change a card's text or a project's repository, folder, branch or goal wait under its answer for your Approve, each with a preview. See [Approve with one click](chief.md#approve-with-one-click).

In the Chief's form, each MCP server, the browser and Web have **After the Chief uses it:** with **Ask for my OK** (the default) or **Allow**. A source on Allow doesn't mark the chat, so what the Chief does after using it runs without your OK. See [Sources you trust](chief.md#sources-you-trust).

## Read more

- [docs/agent-tools.md](../agent-tools.md): MCP servers, signing in, the web, the browser, built-in tools and every safety rule, in full.
- [Bring your own CLI](cli.md): what each coding CLI gets.
- [docs/PLATFORMS.md](../PLATFORMS.md): how the browser and the agents' folders differ on Linux, macOS and Windows.
