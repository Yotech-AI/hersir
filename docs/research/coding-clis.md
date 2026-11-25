# Coding CLIs: OpenCode first, and a CLI profile so a new one needs no code (GA-365)

Research by the R&D Agent, 25 Nov 2026, on `main` at `945076a2`. Each claim is marked **[V]** when it was verified in Hersir's code or in a primary source (named next to it), or **[I]** when it is my judgement. Every web source was read on 2026-11-25; the list is in §8.

*(The summary, the CLI survey and the plan follow below; this commit has the project map and Hersir's needs first.)*

## 2. The project now

### 2.1 What it does

Hersir is a desktop app (Tauri 2, a React/TypeScript UI, Rust crates, local SQLite) where a team of agents works the cards of a board: each agent runs a coding CLI on the user's own machine, with the user's own logins, in a git worktree per card **[V]** (`README.md`, `CLAUDE.md`, `docs/features/cli.md`). The Chief (the Team Lead) chats with the user, checks the board on a heartbeat and answers the agents' questions first **[V]** (`docs/features/chief.md`).

### 2.2 How a run on a coding CLI works today

| Step | Where | What it does |
|---|---|---|
| The CLI list | `crates/gizai-core/src/clis.rs:23` (`Cli`), `:12` (`KINDS`) | A CLI is `{id, name, kind, command, env, args}`, kept as JSON in the `clis` setting. `kind` is `claude_code`, `codex`, `gemini` or `other`; `args` is only for `other` **[V]**. |
| Settings → Coding CLIs | `src/components/CliSettings.tsx` | Name, Kind, Program, Environment (`NAME=value` lines), and Arguments for Other (`{prompt}`, `{model}`) **[V]**. |
| Find installed CLIs | `src-tauri/src/clis.rs:104` (`KNOWN`) | Already knows OpenCode (`run -m {model} {prompt}`), Cursor Agent (`-p --force --output-format text --model {model} {prompt}`) and Crush (`run --quiet`), all as **Other**, read as plain text **[V]**. |
| What a run starts | `src-tauri/src/clis.rs:153` (`spec`) → `crates/gizai-agents/src/cli.rs:174` (`task_exec`) | One Rust function per kind builds the command line: `ClaudeArgs` (`claude.rs`), `codex_args` (`cli.rs:222`), `gemini_args` (`cli.rs:417`), `other_args` (`cli.rs:472`). The prompt goes in on stdin, which is then closed (`process.rs:332`) **[V]**. |
| The run's facts | `src-tauri/src/runs.rs:1438` (`TaskRun`) | Prompt, session id and resume, model and effort, permission mode, allowed commands (Claude Code rules like `Bash(git status:*)`), the agent's folders (read, or read and change), the run's temp folder, git settings for the agent's own push, the MCP config, the Web switches **[V]**. |
| Running it | `crates/gizai-agents/src/process.rs:311` (`spawn_exec`) | Its own process tree, the time and tool-call caps (a cap counts `ToolUse` events), the memory limits, ending a CLI that stays quiet after its result (GA-221) **[V]**. |
| Reading it | `cli.rs:557` (`Parser`) → `stream.rs:7` (`RunEvent`) | One parser per kind turns each stdout line into `RunEvent`s: `Init {session_id, model}`, `Text`, `ToolUse {name, summary}`, `ToolResult {is_error, preview}`, `Result {is_error, subtype, text, cost_usd, input_tokens, output_tokens, num_turns}`, `Refused {tool, input, reason}`, `McpServers`, `Limits`, `Note`, `Other` **[V]**. A log that isn't Claude Code's starts with `{"type":"gizai_cli","kind":…}` so it can be read again (`cli.rs:496`, `:534`) **[V]**. |
| The outcome | `crates/gizai-agents/src/outcome.rs:32` | The last `GIZAI_RESULT: {json}` line of the final text: this works on any CLI whose final message reaches Hersir as text. A plain-text CLI keeps the last 32 KiB for it (`cli.rs:865`) **[V]**. |
| Continue, nudges, the Chief's answers | `runs.rs:871`, `runs.rs:2062` (`nudge_for`), `runs.rs:1560` | Continue, the nudge after a run without a result line, and Ask the Chief first all resume the run's session. A CLI that names its own session (Codex) has it saved from its `Init` event. `Kind::Other` can't resume (`cli.rs:46`): Continue is refused with "can't continue a run" **[V]**. |
| MCP servers | `runs.rs:1337` | Only Claude Code gets the agent's MCP servers, through a per-run 0600 `--mcp-config` file with `--strict-mcp-config` (`mcp_run.rs`); any other CLI gets a note "MCP servers work on Claude Code for now" **[V]**. |
| Allowed commands and folders | `cli.rs:133`, `:153`, `:270` | Claude Code gets `--allowedTools` and deny rules for read-only folders; Codex gets no list (its sandbox, `approval_policy="never"`) and writable roots; Gemini gets `run_shell_command(<prefix>)` and `--include-directories` (no read-only folders); Other gets nothing **[V]**. |
| How this run works | `crates/gizai-agents/src/prompt.rs:454` | The rules at the end of every task prompt, written per kind **[V]**. |
| Built-in tools | `crates/gizai-agents/src/tool_catalog.rs:140` | A catalog per kind; Other has none **[V]**. |
| Subscription limits | `src-tauri/src/limits.rs:44` | Claude Code's `rate_limit_event`, Codex's session log (`$CODEX_HOME/sessions`); Gemini and Other: none **[V]**. |
| The Chief | `src-tauri/src/chat.rs:1532` (chat turn), `:1817` (board check), `:2024` (look at an agent's question) | Always Claude Code: `--restricted`, `--tools Read,Glob,Grep`, `--permission-prompts none`, `--append-system-prompt`, `--include-partial-messages`, and Hersir's own MCP server `gizai` through the `hersir-mcp` shim with a token per turn (`chat.rs:1509`, `crates/gizai-mcp`) **[V]**. |

**Tests and release.** Parsers and command lines are tested with fixed lines and fake CLI scripts (`crates/gizai-agents/tests/it/cli_test.rs`, `tests/fake-cli.sh`, `tests/fake-claude*.{sh,py}`, `tests/fake-gemini-env.sh`), run flows in `src-tauri/tests/it/cli_run_flow_test.rs`; CI builds and tests each pull request on Linux, macOS and Windows (`.github/workflows/ci.yml`); a release is a `vX.Y.Z` tag on `production` (`docs/RELEASING.md`) **[V]**. Tests never start a real CLI **[V]** (`CLAUDE.md`).

### 2.3 What this feature touches

- `crates/gizai-agents/src/cli.rs` (the `Kind` enum, the per-kind command lines and parsers), `stream.rs` (`RunEvent`), `prompt.rs` (`rules_section`, `may_sleep`), `tool_catalog.rs`, `mcp_run.rs`.
- `crates/gizai-core/src/clis.rs` (the saved `Cli`, `KINDS`), `crates/gizai-core/src/limits.rs`.
- `src-tauri/src/clis.rs` (`KNOWN`, `spec`), `src-tauri/src/runs.rs` (the start of a task run), `src-tauri/src/limits.rs`.
- `src/components/CliSettings.tsx`, `src/lib/clis.ts`, `src/components/AgentForm.tsx` (Runs on, Permissions, Tools).
- Not the Chief's chat (`chat.rs`, `chat_stream.rs`) in the first cards: see §3.2.

## 3. The feature

### 3.2 What Hersir needs from a CLI, per feature

"Must" means Hersir can't run an agent on the CLI without it; every other row has a fallback, which already exists for Other CLIs today.

| Hersir feature | What it needs from the CLI | Today | When a CLI can't |
|---|---|---|---|
| **Task runs** (must) | Runs headless with a prompt and exits; works in the folder Hersir starts it in (the worktree); never waits for a keyboard. The prompt on stdin or from a file is best: Linux caps one argument at 128 KiB, and a long card would not start (`claude.rs:68`) **[V]**. | All four kinds. Other puts `{prompt}` in an argument or on stdin. | Not supported: Settings says so. A CLI that only takes the prompt as an argument gets it there, and a prompt over 128 KiB fails with a plain error **[I]**. |
| **The outcome line** (must) | The agent's final message as text, whole. | All kinds; plain text keeps the last 32 KiB. | n/a: any CLI that prints its answer can do it. |
| **The Run panel** | Events for text, tool calls (name and input) and tool results. | Claude Code, Codex, Gemini. | Each output line is text (as Other now). The **tool-call cap can't count** (it counts `ToolUse`), so only the time cap stops the run: Settings and the run log should say so **[I]**. |
| **Refused tool calls** | An event for a tool call refused because nobody could approve it: the tool, what it asked for, and why. | Claude Code only (`cli.rs:601`). | The Run panel lists none and the run log says "this CLI doesn't report refusals", as for Codex and Gemini now (`docs/features/tools.md`) **[V]**. |
| **Usage** | Input tokens (cache included), output tokens, and cost when it knows it. | Claude Code (with cost), Codex and Gemini (tokens; cost unknown). | Tokens 0, cost unknown; Usage shows "unknown" as for Codex now **[V]** (`docs/features/cli.md`). |
| **Subscription limits** | The account's limit windows (used, resets), in its output or a log Hersir may read. | Claude Code, Codex. | "Hersir can't read its limits yet", as for Gemini now **[V]**. |
| **Resuming** (Continue, the nudge, Ask the Chief first, Done continue) | A session id (chosen by Hersir, or given in the output) and a headless resume of it. | Claude Code, Codex, Gemini. | Continue is refused ("can't continue a run: Run starts the card fresh") and the nudge is skipped (`runs.rs:871`, `:2072`) **[V]**. The Chief's answer could then start a fresh run with the answer in its prompt **[I]** (a later card). |
| **MCP servers** (the agent's servers, the browser) | A per-run MCP config that doesn't touch the user's own files, with only the run's servers (none of the user's or the repository's), stdio and HTTP with headers, and a way to allow or refuse a server's tools. | Claude Code only. | The run goes without them, with a note, as now **[V]**. |
| **Hersir's own MCP tools** (the `gizai` server) | The same as MCP servers, plus a stdio server started from a path (the `hersir-mcp` shim) with an environment (the token). | The Chief only, on Claude Code. Task runs don't get it. | n/a in task runs. |
| **Allowed commands** | Headless: a list of allowed shell commands by prefix; any other command refused at once (never a prompt that hangs); a refusal visible in the output. | Claude Code (`--allowedTools`), Gemini (`run_shell_command(…)`), Codex (its sandbox instead). | **Hersir must not pretend.** Either the CLI's own sandbox limits it (as Codex), or the agent's Permission mode must say "any command" (the user picks it, as Codex's "Full access"). A CLI with neither shows a warning in the agent form and the run log; the commands list is left out and the run says so **[I]**. |
| **Read-only folders and extra folders** | Extra folders the agent may read (and change), and a way to keep some read only. | Claude Code (both), Codex (writable roots; reads all), Gemini (read and change only). | Left out with a note, as Gemini's read folders now (`cli.rs:153`) **[V]**. |
| **The run's temp folder** | Writable as an extra folder, `TMPDIR` honoured. | Claude Code, Codex, Gemini. | It still gets `TMPDIR`, `TMP`, `TEMP` (every kind does, `cli.rs:176`) **[V]**. |
| **Web switches** | A switch per run for web search and fetching pages. | Claude Code, Codex (search), Gemini (fetch). | Off or on by the CLI's own settings; the agent form shows it disabled with why, as now (`tool_catalog.rs:233`) **[V]**. |
| **Model and effort** | A flag for each. | All (Other through `{model}`). | The CLI's default. |
| **A repository's own settings** (GA-263, GA-322) | A way to ignore the worktree's own config (MCP servers, plugins, hooks, custom commands, `.env`), or to know which files to look for. | Claude Code (`--setting-sources user`), Gemini (`.gemini/` holds the card), Codex (no project config). | Hold the card "blocked" when such a file is in the worktree, as Gemini's `.gemini/` now (`cli.rs:278`) **[V]**. |
| **A second account** | A variable that moves its config/credentials folder. | `CLAUDE_CONFIG_DIR`, `CODEX_HOME`. | One account only. |
| **The Chief's chat, board check and look at a question** | Streaming text while it writes, Hersir's `gizai` MCP server, built-in tools cut down to read-only ones, no permission prompts, an appended system prompt, a session resumed per turn. | Claude Code only (§2.2). | Stays on Claude Code. §3.4 says why and what a later card could do. |

### 3.3 A CLI profile

A **profile** is a JSON document that tells Hersir how to start one coding CLI, how to read what it writes, and which of Hersir's features it can take. Profiles **shipped with Hersir** are checked by the team against a real version of the CLI and compiled into the app; profiles **you add** in Settings → Coding CLIs are yours and are marked "not checked by Hersir" **[I]**.

A CLI in Settings then has a **profile** instead of one of the four fixed kinds: `{"kind": "profile", "profile": "opencode", …}`. Claude Code, Codex and Gemini stay the code they are now (see "What stays code" below); `other` stays as it is.

#### The sections of a profile

| Section | What it says | Hersir feature it serves |
|---|---|---|
| `id`, `name`, `programs`, `checked_with`, `version_args` | The profile's id; the name Settings shows; the program names Find installed CLIs looks for; the CLI version it was checked with; how to read the installed version (Settings warns when it differs in its major version). | Settings, Find installed CLIs |
| `sign_in`, `account_env` | One line on how to sign the CLI in (Hersir never does it); the variable that moves its logins to another folder, for a second account. | Settings → Environment hint |
| `start` | The arguments of a new run and of a resumed one, as an **array** (never a shell string); how the prompt goes in (`stdin`, `arg`, or `file`, which Hersir writes in the run's log folder); environment variables. Placeholders: `{prompt}`, `{prompt_file}`, `{model}`, `{effort}`, `{session}`, `{worktree}`, `{temp_dir}`, `{run_config}` (the per-run config file's path), `{run_config_json}` (its text). An inner array is an **optional group**, left out when a placeholder in it is empty (`["-m", "{model}"]`); `{"each": "folders_rw", "args": ["--add-dir", "{path}"]}` repeats per folder. | Task runs, model, effort, folders |
| `run_config` | A per-run config file in the CLI's own format (JSON or TOML), which Hersir writes (0600, in the run's log folder, removed when the run ends, as `mcp_run::write_config` does now) from the profile's `base` object plus what Hersir fills in: MCP servers, permission rules, folders. | MCP servers, allowed commands, folders |
| `session` | `"id": "caller"` (Hersir chooses a UUID and passes `{session}`) or `"reported"` with the JSON pointer of the event that names it. Without `resume` args the CLI can't continue. | Continue, the nudge, Ask the Chief first |
| `events` | How each output line becomes a `RunEvent` (below). `"format": "text"` keeps today's plain-text reading. | Run panel, outcome line, usage, refusals |
| `mcp` | Where in `run_config` the servers go, and a template for a `stdio` entry and an `http` entry in the CLI's own words; how a server's tools are allowed or refused; how the user's own servers are kept out (`strict`). | MCP servers, the browser |
| `permissions` | The permission modes the agent form offers (id, label, the text that explains it, risky or not, its **tier** from strict to loose for the Chief's GA-352 checks, `tools/write.rs:286`, and the `run_config` or arguments each sets); for the mode with a commands list, where the allowed commands go and in which **style** (below); the rule that refuses every other command. Whether the CLI keeps an agent to its list (`keeps_to_the_list`, `tools/write.rs:282`, now Claude Code only) is true only for a shipped profile whose check proved it. | Allowed commands, Permissions, the Chief's Approve |
| `folders` | How extra folders are given: read and change, and read only (or "can't"). | Folders, the run's temp folder |
| `web` | The arguments or `run_config` that switch web search and fetching pages on and off; or "can't". | Web switches |
| `guards` | Files or folders in the worktree that hold the card "blocked" before the run (as `.gemini/` now, `cli.rs:278`), with the message; variables set empty unless the CLI's own lines set them (as `BASH_ENV`, `ENV` for Gemini, `cli.rs:302`). | A repository's own settings (GA-263, GA-322) |
| `rules` | The words "How this run works" uses for this CLI: its write tool's name, and whether a refused call is reported. | The prompt's rules (`prompt.rs:454`) |
| `tools` | The CLI's built-in tools for the agent form's catalog (name, what it does, read only or not). Optional. | Tools per agent (`tool_catalog.rs`) |

A section left out means "this CLI can't", and Hersir does what §3.2 says for that feature: the agent form and the run log say so, as they do for Other CLIs now **[I]**.

#### Reading the events: a mapping, not a script

The `events` section is a list of rules, tried in order on each JSON line. A rule matches on JSON pointers (serde_json's `Value::pointer`, which `cli.rs` already uses) and says which `RunEvent` to emit with which fields:

```json
{ "match": { "/type": "tool_use", "/part/state/status": "completed" },
  "emit": "tool", "id": "/part/callID", "name": "/part/tool", "input": "/part/state/input",
  "output": "/part/state/output", "error": false }
```

The emits are a fixed list, each built on what the three parsers in `cli.rs` already do:

| Emit | Becomes | Like today's |
|---|---|---|
| `init` (`session`, `model`) | `RunEvent::Init`, once | Codex `thread.started`, Gemini `init` |
| `text` (`text`, `delta: true\|false`) | `RunEvent::Text`; deltas are joined until the next other event; the text after the last tool call is kept as the final message | Gemini `message` (deltas), Codex `agent_message` |
| `tool` (`id`, `name`, `input`, and when done `output`, `error`) | `RunEvent::ToolUse` once per id (so the tool-call cap counts each call once), then `RunEvent::ToolResult` when it has an output | Codex `item.started` / `item.completed` |
| `usage` (`input`, `output`, more pointers to add up, `cost`, `sum: true` to add over lines) | Kept until the result | Codex `turn.completed`, Gemini `result.stats` |
| `refused` (`tool`, `input`, `reason`) | `RunEvent::Refused` | Claude Code `permission_denied` |
| `error` (`message`) | Kept; becomes the result when the output ends without one | Codex and Gemini `error` |
| `result` (`text` or the final message, `error`) | `RunEvent::Result` with the usage kept | Codex `turn.completed`, Gemini `result` |
| `ignore` | nothing | |

A line no rule matches is `Other {raw_type}` and a line that isn't JSON is `Other {invalid}`, as now (`cli.rs:717`). When the output ends without a `result`, the engine gives one from the last error (failed) or the final message (done), as `Codex::finish` and `Gemini::finish` do now. There is no scripting language and no regular expression in a profile: a profile can't run code inside Hersir **[I]**.

A test can prove the vocabulary is enough without moving anything: profiles written for Codex and Gemini (in the tests only) must give the same events as `Codex::line` and `Gemini::line` on the fixture lines in `crates/gizai-agents/tests/it/cli_test.rs:285` and `:333` **[I]**.

A run log of a profile CLI starts with `{"type":"gizai_cli","kind":"profile","profile":"opencode","events":[…]}`: the header carries the event rules themselves, so a finished run's log reads again the way it ran even after the profile changes (today the header names only the kind, `cli.rs:496`) **[I]**.

#### Allowed commands: a few named styles

Hersir keeps an agent's allowed commands as Claude Code rules (`Bash(git status:*)`, `docs/features/tools.md`). A profile names the style its CLI takes, and Hersir translates with code it tests:

| Style | `Bash(git status:*)` becomes | For |
|---|---|---|
| `claude_rule` | `Bash(git status:*)` | CLIs that copied Claude Code's rules |
| `prefix_glob` | `git status *` (and `git status`) | OpenCode's `permission.bash` (§3.1.1) |
| `shell_call` | `shell(git status:*)` (the profile gives the tool name) | Copilot-like `--allow-tool` |
| `gemini_shell` | `run_shell_command(git status)` | Gemini, Qwen Code (`cli.rs:270`) |

A profile also says whether its CLI splits a command line on `&&`, `;`, `|` and `$(…)` before matching (so `git status && rm -rf x` is refused). Hersir can't check that from a profile: the shipped profile's check does, and a profile you add says "not checked by Hersir" next to its Allowed commands **[I]**.

#### What stays code

- **Running a CLI**: the process tree, the time and tool-call caps, the memory slice, ending a CLI that stays quiet after its result, closing stdin (`process.rs`). The outcome line (`outcome.rs`).
- **The engines**: the template renderer (arguments as an array, optional groups, `each`), the event engine above, the `run_config` writer, and the allowed-commands styles. They are the only things that read a profile.
- **Everything that guards your data**: the checks on the agent's folders (never `/`, your home folder, Hersir's data or folders with keys), the run's temp folder and `TMPDIR`, the git settings of the agent's own push, the MCP entries with secrets from your keychain (they go only in the per-run file, never in a profile), the guard checks (`guards` only names the files).
- **What one CLI needs that no other does**: Gemini's `.env` handling (GA-322, `cli.rs:287`), Codex's writable git folders (GA-263, `cli.rs:243`), reading Codex's subscription limits from its session log and Claude Code's `rate_limit_event` (`limits.rs:44`). So **Claude Code, Codex and Gemini stay code adapters**; moving them to profiles would win nothing and risk their security fixes **[I]**.
- **The Chief's chat** (§3.4).
- **An ACP client**, if Hersir takes option C (§4): the protocol is code; an ACP agent's profile is then only `start`, `guards` and `account_env`.
- **What the front end knows per kind today** (`src/lib/clis.ts`: `PERMISSIONS`, `FOLDERS_NOTE`, `EFFORTS_BY_KIND`, `RISKY`) moves to the backend: `list_clis` sends each CLI's modes, efforts and notes, built from its kind or its profile, so a profile needs no front-end change **[I]**.

#### Shipped and added profiles

- **Shipped:** `crates/gizai-agents/profiles/<id>.json`, compiled in with `include_str!`, each with the CLI version it was checked with and a fixture of that version's real output in the tests. A new CLI or a changed format comes with a Hersir release, and Hersir doesn't download profiles from anywhere **[I]**: Hersir sends nothing about you (`docs/features/cli.md:18`) **[V]**, and a profile is trusted input (it picks the program and its arguments).
- **Added by you:** Settings → Coding CLIs → Add a profile (paste or pick a JSON file). Hersir checks it against the format (unknown keys, a placeholder that doesn't exist, an emit that doesn't exist, a `start` without the prompt) and says in plain words what is wrong; it keeps it in a `cli_profiles` setting. It shows "not checked by Hersir", and its `keeps_to_the_list` is always false, so the Chief's changes to an agent on it wait for your Approve (GA-352) **[I]**.
- A profile you add with the id of a shipped one is refused: copy it under a new id (open question 3).

### 3.4 The Chief's chat stays on Claude Code (for now)

The chat turn is the most Claude Code-bound part of Hersir **[V]** (`src-tauri/src/chat.rs:1509`–`:1540`): `--restricted` and `--tools Read,Glob,Grep` (built-in tools cut down), `--permission-prompts none`, `--append-system-prompt`, `--include-partial-messages` for the text as it is written, the `gizai` MCP server through the `hersir-mcp` shim with a token per turn, and the GA-271/GA-289 logic that decides whether a tool result came from a trusted source (`Trusted::of`, `TurnTools::of`). `chat_problem` (`crates/gizai-core/src/clis.rs:155`) and `team.rs:353` refuse the chat on any other kind.

**Recommendation [I]:** keep the Chief on Claude Code in this work. "Any coding CLI" is about the agents that build; the Chief is one per company and already has Runs on for a second Claude Code account. A later card can move the chat to a profile once two things are checked on a real CLI: that its built-in tools can be cut down to read-only ones per run (OpenCode can, with a per-run agent whose `permission` denies edit and bash, §3.1.1), and that it streams text. ACP (§3.1.13) gives both streaming and MCP servers per session, so if Hersir gets an ACP client (option C, §4), the chat on an ACP agent is the natural later card. Open question 5.

## 5. The design

### 5.1 Screens

Two screens change; both keep the app's own pieces: the `.cli-list` rows and the `.cli-edit` form of `src/components/CliSettings.tsx`, `badge outline` (as `AgentCliTools.tsx:23`), `btn ghost sm`, `input`, `select`, `textarea mono`, `hint`, `warn` and `faint` (`src/styles/components.css`, `tokens.css`) **[V]**. No new component is needed **[I]**.

**Settings → Coding CLIs.** The Kind list gets the shipped profiles between Gemini and Other, and the profiles you added after a divider. Picking a profile fills Program from its `programs`, and the Environment hint from `account_env` and `sign_in`. Each row shows whether the profile is checked, and in one line what Hersir can and can't do with the CLI (the `can` list of `list_clis`, below):

```
Coding CLIs
 Codex        · Codex                 codex exec --json                     [Edit] [Remove]
 OpenCode     · OpenCode  [checked with 1.19]   opencode run --format json  [Edit] [Remove]
              Run panel, Continue, MCP servers, allowed commands · not: subscription limits
 My CLI       · my-cli    [not checked by Hersir]  my-cli exec --json       [Edit] [Remove]
              Run panel, usage · not: Continue, MCP servers, allowed commands
 [Add CLI] [Find installed CLIs] [Add a profile]

 Add a profile ─────────────────────────────────────────────────────────────────────────────
 | { "profile": 1, "id": "my-cli", … }                                                     |
 Paste a CLI profile (JSON). Hersir checks it, and shows what it can do before you save it.
 ✓ my-cli: Run panel, usage, cost · not: Continue, MCP servers, allowed commands   [Cancel] [Save profile]
```

**Agent form → Runs on, Permissions, Folders, Tools** (`src/components/AgentForm.tsx`). The permission modes, their texts, the risky ones, the efforts and the folders note come from `list_clis` (now `src/lib/clis.ts:9`–`:49` per kind). A profile CLI without allowed commands shows its modes with "Hersir can't keep this CLI to a list of commands", as Codex's sandbox modes are shown now; a profile you added shows "not checked by Hersir" next to Allowed commands. MCP servers and the browser are on only when the CLI `can.mcp` (now `mcp_not_on`, `crates/gizai-core/src/mcp_servers.rs:433`).

### 5.3 Data

- `clis` setting: `Cli` (`crates/gizai-core/src/clis.rs:23`, `src/types.ts:167`) gets `profile: String`, set when `kind` is `"profile"`; `KINDS` gets `"profile"`. Existing CLIs keep their kind: nothing to migrate. An **Other** CLI whose program is `opencode` keeps running as now; Settings offers "Use the OpenCode profile" on its row **[I]**.
- `cli_profiles` setting: the profiles you added, each as the JSON you saved. Shipped profiles aren't stored.
- A run's log header names the profile and carries its `events` rules (§3.3); the per-run config file sits next to the log (`<data dir>/runs/<run>.<id>.json`, 0600) and goes when the run ends, like the MCP config now (`runs.rs:1343`).

### 5.4 Commands and types (the contract)

Backend → frontend, word for word (Rust in `src-tauri/src/clis.rs` and `commands.rs`, TypeScript in `src/types.ts` and `src/api.ts`):

```ts
export type CliKind = "claude_code" | "codex" | "gemini" | "profile" | "other";
export type Cli = { id: string; name: string; kind: CliKind; command: string; env: string[]; args: string; profile: string };
export type CliMode = { id: string; label: string; text: string; risky: boolean; tier: number };
export type CliCan = { runPanel: boolean; resume: boolean; mcp: boolean; allowedCommands: boolean; readOnlyFolders: boolean;
  refusals: boolean; usage: boolean; cost: boolean; limits: boolean; webSearch: boolean; webFetch: boolean; chat: boolean };
export type CliCaps = { modes: CliMode[]; efforts: string[]; foldersNote: string; can: CliCan;
  /** Why not, per key of `can` that is false, in plain words. */ why: Partial<Record<keyof CliCan, string>> };
export type CliProfileInfo = { id: string; name: string; shipped: boolean; checkedWith: string | null; programs: string[];
  signIn: string | null; accountEnv: string | null; caps: CliCaps };
export type CliStatus = Cli & { path?: string | null; problem?: string | null; caps: CliCaps;
  /** The installed version when the profile names `version_args`; a warning when its major version differs. */
  version?: string | null; versionWarning?: string | null };

export const listCliProfiles = () => invoke<T.CliProfileInfo[]>("list_cli_profiles");
/** Checks and saves one profile you added; Err in plain words. Same id as a shipped profile: refused. */
export const addCliProfile = (json: string) => invoke<T.CliProfileInfo>("add_cli_profile", { json });
/** Refused while a CLI in Settings uses it. */
export const removeCliProfile = (id: string) => invoke<T.CliProfileInfo[]>("remove_cli_profile", { id });
```

`list_clis`, `save_clis` and `find_clis` keep their names; `CliStatus` gains `caps` (for every kind: Claude Code, Codex and Gemini fill it from today's constants), so the front end drops its per-kind tables **[I]**.

### 5.5 Security and privacy

- **A profile can't run code inside Hersir** (no scripts, no expressions; §3.3). It does pick the program and its arguments, as Other's Arguments do now: only you add one, and shipped ones are reviewed and tested **[I]**.
- **Secrets** go only in the per-run config file (0600, removed at the end), filled from your keychain by `mcp_servers::for_run` as now; never in a profile, the run log or its header **[V]** (`mcp_run.rs:27`, `runs.rs:1340`).
- **A repository's own settings** never start a program outside the agent's allowed commands (`docs/features/tools.md:128`) **[V]**: each profile names the files that would (for OpenCode §3.1.1) as `guards`, and the card goes on hold "blocked" when the worktree has one, as for `.gemini` now **[I]**.
- **Allowed commands** are only claimed (`keeps_to_the_list`) for a shipped profile whose check proved that a chained or substituted command (`git status && rm -rf x`, `$(…)`) is refused; otherwise the Chief's changes to the agent wait for Approve (GA-352) **[I]**.
- **No new network traffic**: profiles come with releases (§3.3).
- **The sandbox** for every CLI stays GA-265; a profile can name the CLI's own sandbox flags in a permission mode, as Codex's modes do now.

### 5.6 Tests

- The engine: unit tests per emit with fixture lines; each shipped profile has a fixture of its CLI's real output (recorded once by the builder, with the version), and a test that it gives the expected events, usage and result **[I]**.
- The proof of the vocabulary: test-only profiles for Codex and Gemini give the same events as `Codex::line` and `Gemini::line` on the fixtures in `cli_test.rs:285` and `:333` **[I]**.
- The command line and the per-run config: like `cli_test.rs:62`–`:146` now (arguments, environment, temp folder, git settings, optional groups, `each`), plus the MCP entries and permission rules in the config.
- A run flow with a fake CLI script (as `tests/fake-cli.sh` and `src-tauri/tests/it/cli_run_flow_test.rs`): start, Continue with the session, the guard that holds a card **[V]** (these files exist). Tests never start a real CLI (`CLAUDE.md`) **[V]**.
- A manual check by the builder against the real CLI, written in the pull request and in a code comment, as `prompt.rs:426` does for Claude Code 2.1.289 **[V]**: an allowed command runs, another is refused, a chain with a refused part is refused, the run resumes, the MCP server answers.

### 5.7 Left out

The Chief's chat (§3.4); subscription limits for profile CLIs (no CLI but Claude Code and Codex reports them, §3.1); the sandbox (GA-265); Hersir as an MCP server for other AI apps (GA-347); downloading profiles; moving Claude Code, Codex or Gemini to profiles.
