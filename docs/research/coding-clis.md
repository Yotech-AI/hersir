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

### 3.4 The Chief's chat stays on Claude Code (for now)

The chat turn is the most Claude Code-bound part of Hersir **[V]** (`src-tauri/src/chat.rs:1509`–`:1540`): `--restricted` and `--tools Read,Glob,Grep` (built-in tools cut down), `--permission-prompts none`, `--append-system-prompt`, `--include-partial-messages` for the text as it is written, the `gizai` MCP server through the `hersir-mcp` shim with a token per turn, and the GA-271/GA-289 logic that decides whether a tool result came from a trusted source (`Trusted::of`, `TurnTools::of`). `chat_problem` (`crates/gizai-core/src/clis.rs:155`) and `team.rs:353` refuse the chat on any other kind.

**Recommendation [I]:** keep the Chief on Claude Code in this work. "Any coding CLI" is about the agents that build; the Chief is one per company and already has Runs on for a second Claude Code account. A later card can move the chat to a profile once two things are checked on a real CLI: that its built-in tools can be cut down to read-only ones per run (OpenCode can, with a per-run agent whose `permission` denies edit and bash, §3.1.1), and that it streams text. ACP (§3.1.13) gives both streaming and MCP servers per session, so if Hersir gets an ACP client (option C, §4), the chat on an ACP agent is the natural later card. Open question 5.
