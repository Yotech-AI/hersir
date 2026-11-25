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

### 3.1 The CLIs

Each CLI below has the points of the card, in this order: **headless** (how it runs and takes the prompt), **output** (structured events), **resume**, **MCP**, **model**, **permissions**, **folders**, **sign-in**, **sandbox**, **repository config** (what it loads from the worktree), **ACP** (the Agent Client Protocol, §3.1.13), **limits**, and **licence and systems**. Sources are numbered as in §8; each was read on 2026-11-25. Pages were read through a fetch tool that summarises a page before quoting it, so **[V]** means a primary source said it, and a flag name was copied from the source **[I]**/**[V]** as marked.

#### 3.1.2 Codex (OpenAI): already in Hersir

- **Headless:** `codex exec`; the prompt as an argument, or on stdin with `-` or no prompt (stdin and an argument together: stdin is appended as a `<stdin>` block). Progress goes to stderr **[V]** [S20, S21].
- **Output:** `--json`, JSONL: `thread.started{thread_id}`, `turn.started`, `item.started|updated|completed{item}` with item types `agent_message`, `reasoning`, `command_execution{command, aggregated_output, exit_code, status: in_progress|completed|failed|declined}`, `file_change`, `mcp_tool_call`, `web_search`, `todo_list`, `collab_tool_call`, `error`; `turn.completed{usage: input_tokens, cached_input_tokens, cache_write_input_tokens, output_tokens, reasoning_output_tokens}`, `turn.failed`, `error`. No cost **[V]** [S22]. Hersir's parser (`cli.rs:687`) reads all of these but `declined` and `cached_input_tokens`: a `declined` command shows as a failed result, not as Refused **[V]**/**[I]**.
- **Resume:** `codex exec resume <id> [-]`; the id is `thread.started.thread_id` **[V]** [S21].
- **MCP:** `[mcp_servers.<id>]` in `config.toml` (stdio `command, args, env, cwd`; HTTP `url, http_headers, bearer_token_env_var`; `enabled_tools`, `disabled_tools`, `required`, per-tool `approval_mode`); `--ignore-user-config` skips `$CODEX_HOME/config.toml`; a project's `.codex/config.toml` loads only when trusted **[V]** [S23]. Per run with `-c mcp_servers.<id>.…` **[I]**.
- **Model:** `-m`; `model_reasoning_effort` (`low…xhigh|max|ultra` by model) **[V]** [S23].
- **Permissions:** `--sandbox read-only|workspace-write|danger-full-access` (`exec` defaults to read-only); `approval_policy` `on-request|never|granular`; `--full-auto` is gone (Hersir doesn't use it, `cli.rs:234`); `--dangerously-bypass-approvals-and-sandbox`; command rules are execpolicy `.rules` files, `--ignore-rules` skips them **[V]** [S21, S23, S24]. What `exec` does when a command needs approval with `never` is not documented **[V]** (checked [S24]).
- **Folders:** `-C`, `--add-dir`, `sandbox_workspace_write.writable_roots`; `.git`, `.codex`, `.agents`, `.aws` inside writable roots stay read only **[V]** [S21, S24].
- **Sign-in:** ChatGPT login, `codex login --with-api-key`, device code; `CODEX_API_KEY` per run; credentials in `$CODEX_HOME/auth.json` or the keyring (`cli_auth_credentials_store`); `CODEX_HOME` moves it all **[V]** [S25].
- **Sandbox:** Seatbelt (macOS), bubblewrap + seccomp (Linux), MXC (Windows) **[V]** [S24].
- **Repository config:** `AGENTS.md`; `.codex/config.toml` only when trusted; hooks behind `features.hooks` **[V]** [S23].
- **ACP:** through the adapter `codex-acp` (now `agentclientprotocol/codex-acp`, Apache-2.0), built on Codex's app server **[V]** [S26].
- **Limits:** `RateLimitSnapshot` is in Codex's protocol but not in `exec --json` **[V]** [S27, S22]; Hersir reads them from the session log (`limits.rs:51`) **[V]**.
- **Licence and systems:** Apache-2.0; 0.172.0 (npm, 25 Nov 2026); Linux, macOS, Windows (native) **[V]** [S20, npm].

#### 3.1.3 Gemini CLI (Google): already in Hersir

- **Headless:** `gemini -p "<prompt>"` (appended to stdin); exit codes 0, 1, 42 (input), 53 (turn limit) **[V]** [S30].
- **Output:** `-o stream-json`: `init{session_id, model}`, `message{role, content, delta?}`, `tool_use{tool_name, tool_id, parameters}`, `tool_result{tool_id, status, output?, error?}`, `error{severity, message}`, `result{status, stats{input_tokens, output_tokens, cached, …}}`. No cost **[V]** [S31]. Hersir's parser reads these (`cli.rs:786`) **[V]**.
- **Resume:** `--session-id <id>` (chosen by the caller) and `--resume <id>` **[V]** [S32], as Hersir uses them (`cli.rs:426`).
- **MCP:** `mcpServers` in settings (`command, args, env, cwd`, `url`, `httpUrl`, `headers`, `includeTools`, `excludeTools`); `--allowed-mcp-server-names`; `GEMINI_CLI_SYSTEM_SETTINGS_PATH` overrides all settings files **[V]** [S33].
- **Permissions:** `--approval-mode default|auto_edit|yolo|plan`. **`--allowed-tools` is deprecated** in favour of the policy engine: TOML `[[rule]]` with `toolName`, `commandPrefix`, `commandRegex`, `decision = allow|deny|ask_user`, `allowRedirection`, `allowCommandSubstitution`, loaded with `--policy <file>`. **Headless, `ask_user` counts as deny** (no hang). In an untrusted folder the mode is forced to `default`; `--skip-trust` trusts it for the session **[V]** [S32, S34]. Hersir uses `--allowed-tools` (`cli.rs:428`): a follow-up card should move to a per-run `--policy` file **[I]**.
- **Folders:** `--include-directories` (at most 5); no read-only folders **[V]** [S33].
- **Sign-in:** Google login, `GEMINI_API_KEY`, Vertex; `GEMINI_CLI_HOME` moves `~/.gemini` **[V]** [S35, S33]. The sign-in page says Gemini CLI was **replaced by Antigravity CLI on 18 June 2026 for free-tier and Google One users** **[V]** [S35].
- **Sandbox:** Seatbelt, Docker/Podman, gVisor, LXC, Windows native **[V]** [S36].
- **Repository config:** `GEMINI.md`, `.gemini/settings.json`, `.gemini/.env`, hooks, extensions **[V]** [S33]; Hersir's guards (`cli.rs:276`–`:360`).
- **ACP:** native `--acp` (`--experimental-acp` deprecated) **[V]** [S32].
- **Limits:** tokens only, no quota in the output **[V]** [S31].
- **Licence and systems:** Apache-2.0; 0.71.1 (npm, 24 Nov 2026); Linux, macOS, Windows **[V]** [S30, npm].

#### 3.1.4 Qwen Code (Alibaba, a Gemini CLI fork)

- **Headless:** `qwen -p "<prompt>"` or the prompt on stdin; budgets `--max-session-turns`, `--max-wall-time`, `--max-tool-calls` **[V]** [S40].
- **Output:** `-o stream-json` in **Claude Code's shape**: `system{subtype, session_id, model, tools, mcp_servers, permission_mode}`, `assistant{message{content[text|thinking|tool_use]}}`, `user{message{content[tool_result{is_error}]}}`, `result{subtype, is_error, num_turns, result, usage{input_tokens, output_tokens, cache_read_input_tokens, cache_creation_input_tokens}, permission_denials[{tool_name, tool_use_id, tool_input}]}`. No cost **[V]** [S41]. Hersir's Claude Code reader (`stream.rs:83`) would read it nearly as is **[I]**.
- **Resume:** `--resume <id>`, `--continue`; the id is reported in `session_id` **[V]** [S40].
- **MCP:** settings like Gemini's (`QWEN_CODE_SYSTEM_SETTINGS_PATH`) **[V]** [S42]; the `mcpServers` fields not read (the page was cut) **[I]**.
- **Model:** `-m`; effort is the setting `model.reasoningEffort` **[V]** [S42].
- **Permissions:** `--approval-mode plan|default|auto-edit|auto|yolo`; refusals in `result.permission_denials` **[V]** [S40, S41]; the rule syntax not read **[I]**.
- **Folders:** `--include-directories` **[V]** [S42].
- **Sign-in:** Qwen OAuth needs a paid plan and a browser; API keys (`DASHSCOPE_API_KEY`, `OPENAI_API_KEY`, …) in `~/.qwen/settings.json` or `.env` **[V]** [S43]; no variable to move `~/.qwen` documented **[V]** (checked [S42], [S43]).
- **Repository config:** `.qwen/settings.json`, `.qwen/skills/`, `.qwen/.env` **[V]** [S42].
- **ACP:** `qwen serve` (HTTP + SSE, experimental) **[V]** [S44]; a stdio `--acp` flag not seen **[I]**.
- **Licence and systems:** Apache-2.0; 0.28.4 (npm, 22 Nov 2026); Node 22+, Linux, macOS, Windows **[V]** [S44, npm].

#### 3.1.5 Cursor CLI (`agent`, was `cursor-agent`)

- **Headless:** `agent -p "<prompt>" --output-format stream-json`; the prompt is an argument (stdin not documented); `--trust` trusts the workspace, headless only. Print mode "can't stop to ask you": what needs approval is rejected, no hang; it writes files in the workspace even without `--force` **[V]** [S80, S81, S82].
- **Output:** `stream-json` (NDJSON): `system/init {session_id, model, cwd, permissionMode}`, `assistant {message.content}`, `tool_call {subtype: started|completed, call_id, tool_call: {readToolCall|writeToolCall|function…}, result}`, `result {subtype, is_error, result, session_id, usage}`; `--stream-partial-output` for deltas. Claude Code's shape for `system`, `assistant` and `result`, Cursor's own for tool calls. No cost **[V]** [S81, S83].
- **Resume:** `--resume <chatId>`; `agent create-chat` returns a new id beforehand; `session_id` in every event **[V]** [S82, S81].
- **MCP:** `.cursor/mcp.json` (project) and `~/.cursor/mcp.json`; **no per-run flag** for the local CLI; `--approve-mcps` approves all servers headless; per-tool rules `Mcp(server:tool)` **[V]** [S84, S82, S85].
- **Model:** `--model <slug>` (effort is part of the slug), `--mode plan|ask` **[V]** [S82, S83].
- **Permissions:** `~/.cursor/cli-config.json` and `<project>/.cursor/cli.json`: `permissions.allow` / `deny` with `Shell(git:*)`, `Read(glob)`, `Write(glob)`, `Mcp(server:tool)`, `WebFetch(domain)`; deny wins. **`-f/--force` (alias `--yolo`): "Force allow commands unless explicitly denied"** **[V]** [S85, S82]. **Hersir's Find installed CLIs offers Cursor Agent as Other with `-p --force …` (`src-tauri/src/clis.rs:108`) **[V]**: an agent on it may run any command that isn't denied. The agent form should say so today **[I]**.**
- **Folders:** `--workspace <path>`, `--add-dir <path>`; read-only paths in `sandbox.json` (`additionalReadonlyPaths`) **[V]** [S82, S86].
- **Sign-in:** `agent login`, or `CURSOR_API_KEY` / `--api-key`; credentials in the keyring (Linux since 19 Nov 2026) or an owner-only file; no variable to move the config folder documented **[V]** [S87, S83].
- **Sandbox:** yes, `--sandbox enabled|disabled`, `sandbox.json` with `workspace_readwrite|workspace_readonly`, network allow lists **[V]** [S86].
- **Repository config:** `.cursor/cli.json`, `.cursor/mcp.json`, `.cursor/sandbox.json`, `AGENTS.md`, hooks; no switch to turn them off **[V]** [S82, S83].
- **ACP:** native, `agent acp` (`session/load`, `session/request_permission`; MCP servers from `mcp.json`, not from `session/new`) **[V]** [S88].
- **Limits:** token `usage` in the result; no cost **[V]** [S83].
- **Licence and systems:** proprietary; v2026.11.19; Linux, macOS, Windows (native and WSL); installs with a script into `~/.local/bin` and updates itself **[V]** [S80, S83].

#### 3.1.6 GitHub Copilot CLI

- **Headless:** `copilot -p "<prompt>"` or the prompt on stdin; exits when done; `--no-ask-user`. "Without any permission options, requests that need approval are denied" **[V]** [S90, S91].
- **Output:** `--output-format json` (JSONL); the CLI's event schema is **not documented**; the Copilot SDK's events (`assistant.message_delta`, `assistant.message`, `tool.execution_start {toolCallId, toolName, arguments}`, `tool.execution_complete {success, result, error}`, `assistant.usage {inputTokens, outputTokens, cost, cacheReadTokens, quotaSnapshots}`, `permission.completed`) are most likely the same **[V]** [S92]/**[I]**. A fixture from a real run is needed before a profile **[I]**.
- **Resume:** `--session-id <uuid>` makes a new session with **the caller's id**; `--resume=<id>` **[V]** [S90].
- **MCP:** `~/.copilot/mcp-config.json`; **`--additional-mcp-config=<json>` for this session only**, highest priority; entries `local|stdio|http|sse` with `headers` and a required `tools` allow list; workspace MCP is off in prompt mode; `--disable-mcp-server <name>` **[V]** [S90].
- **Model:** `--model`, `--effort low|medium|high|xhigh|max` **[V]** [S90].
- **Permissions:** `--allow-tool 'shell(git:*)'` (matches `git push`, not `gitea`), `--deny-tool`, `--allow-all-tools`, `--allow-all`/`--yolo`; deny wins even over `--allow-all` **[V]** [S90]. Refusals: `permission.completed {result.kind: "denied-by-rules"}` in the SDK **[V]** [S92]; in the CLI's JSONL **[I]**.
- **Folders:** current folder, `--add-dir`; no read-only folders **[V]** [S90].
- **Sign-in:** `/login`, or `COPILOT_GITHUB_TOKEN` > `GH_TOKEN` > `GITHUB_TOKEN`; the keychain; **`COPILOT_HOME`** moves `~/.copilot` **[V]** [S90, S93]. Needs a Copilot subscription **[V]** [S94].
- **Sandbox:** yes, an OS shell sandbox (`--sandbox`; Seatbelt on macOS, Windows sandbox) **[V]** [S90, S93].
- **Repository config:** `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.github/copilot-instructions.md` (off with `--no-custom-instructions`); repository hooks and extensions are off in prompt mode **[V]** [S90].
- **ACP:** native, `copilot --acp` (public preview) **[V]** [S90, S75].
- **Limits:** `assistant.usage.quotaSnapshots` and `totalPremiumRequests` (SDK) **[V]** [S92]: the only CLI besides Claude Code and Codex that reports its plan's quota **[I]**.
- **Licence and systems:** proprietary (GitHub's terms); 1.0.116 of 25 Nov 2026; native binaries for Linux, macOS, Windows **[V]** (npm).

#### 3.1.7 Amp (Sourcegraph)

- **Headless:** `amp -x "<prompt>"` or the prompt on stdin; prints its final message and exits **[V]** [S95].
- **Output:** `--stream-json` (with `-x`), which "tries to be compatible with Claude Code's format": `system/init {session_id, tools, mcp_servers}` (no model), `assistant {message.content[text|tool_use]}`, `user {tool_result{is_error}}`, `result {subtype, is_error, num_turns, session_id, result, usage, permission_denials}`. No cost **[V]** [S96].
- **Resume:** `amp threads continue <thread> -x "…" --stream-json`; the thread id is `session_id` **[V]** [S96].
- **MCP:** `amp.mcpServers` in settings; **`--mcp-config`** per run, highest precedence; `--settings-file <path>` replaces your settings file **[V]** [S95, S97, S98].
- **Model:** chosen by Amp (modes), `--fast` **[V]** [S95, S99].
- **Permissions:** **since 6 May 2026 Amp runs tools without asking** (`--dangerously-allow-all` is the default); the old rules come back as a plugin when `amp.permissions` is set, in `amp -x` too; refusals in `result.permission_denials` **[V]** [S99a, S96]. The rule syntax isn't on the current pages **[I]**.
- **Folders:** current folder only **[V]** [S97]. **Sign-in:** `amp login` or `AMP_API_KEY` **[V]** [S95].
- **Sandbox:** none locally **[V]** [S95]. **Repository config:** `.amp/settings.json`, `AGENTS.md`, skills **[V]** [S97].
- **ACP:** none from Sourcegraph; a third-party `amp-acp` **[V]** [S74].
- **Licence and systems:** proprietary; several releases a day; Linux, macOS, Windows through WSL **[V]** (npm, [S95]).

#### 3.1.8 Aider

- **Headless:** `aider --message "<prompt>"` or `--message-file <file>`, with `--yes-always`; it answers, then exits. Stdin isn't a prompt source **[V]** [S100, S101].
- **Output:** **no JSON** at all; plain text (`--no-pretty`, `--no-stream`) **[V]** [S100, S102].
- **Resume:** no session ids; `--restore-chat-history` with a `--chat-history-file` per card **[V]** [S100].
- **MCP:** none **[V]** (absent from [S100], [S103]).
- **Model:** `--model`, `--reasoning-effort`, `--thinking-tokens` **[V]** [S100].
- **Permissions:** only `--yes-always`; a prompt that needs an explicit yes is answered "n" (`io.py`), so the shell commands it suggests are refused **[V]** [S102]/**[I]**. No allow list; it auto-commits unless `--no-auto-commits` **[V]** [S100].
- **Folders:** `--file`, `--read <file>` (read only) **[V]** [S100].
- **Sign-in:** API keys only (`--api-key provider=KEY`, env vars, `.env`) **[V]** [S100].
- **Sandbox:** none **[I]**. **Repository config:** `.aider.conf.yml`, `.env`, `.aiderignore` **[V]** [S100]. **ACP:** none **[V]** [S103].
- **Licence and systems:** Apache-2.0, Python 3.10–3.12; **0.86.2 of 12 Feb 2026, one release in 15 months**: barely maintained **[V]** [S104]/**[I]**.

#### 3.1.9 Goose (now under the Agentic AI Foundation)

- **Headless:** `goose run -t "<text>"`, `-i <file>`, `-i -` (stdin), or `--recipe`; exits when done; `--max-turns` **[V]** [S110, S111].
- **Output:** `--output-format stream-json`: JSONL `{"type": "message", "message": {…}}`, `notification`, `error`, `complete {total_tokens, input_tokens, output_tokens, cache_read_input_tokens, cache_write_input_tokens, cost_usd}`. No session id or model in it; tool calls sit inside `message.content` (field names not verified) **[V]** [S112]/**[I]**.
- **Resume:** the caller names the session: `goose run -n <name> -t …`, then `goose run -n <name> -r` **[V]** [S111].
- **MCP ("extensions"):** per run `--with-extension "ENV=v cmd args"` (stdio), `--with-streamable-http-extension <url>` (no headers on the flag); `extensions:` in `config.yaml` (stdio, streamable HTTP with `headers`; no SSE) **[V]** [S110, S113].
- **Model:** `--provider`, `--model` **[V]** [S110].
- **Permissions:** `GOOSE_MODE` `auto` (default), `approve`, `smart_approve`. Headless: in `approve`/`smart_approve` a tool that needs approval cancels the run with an error; in any other mode it is **allowed** ("auto-allowing") **[V]** [S112]. No shell allow list documented **[V]** [S114].
- **Folders:** the working folder only **[I]**. **Sign-in:** API keys in env vars, the keyring or `secrets.yaml`; `GOOSE_PATH_ROOT` moves all config and data **[V]** [S114, S113].
- **Sandbox:** none (only `--container` for extensions) **[V]** [S110]. **Repository config:** `.goosehints`, `AGENTS.md` (`CONTEXT_FILE_NAMES`) **[V]** [S114].
- **ACP:** native, `goose acp` (stdio) **[V]** [S110].
- **Licence and systems:** Apache-2.0; v1.61.0 of 25 Nov 2026; repo moved to `aaif-goose/goose`; Linux, macOS, Windows **[V]** [S115].

#### 3.1.10 Crush (Charm)

- **Headless:** `crush run [prompt…]`; stdin is put before the arguments; exits when done **[V]** [S120].
- **Output:** **no JSON**: plain text to stdout **[V]** [S120].
- **Resume:** `--session <id>` or `--continue`; `run` doesn't print the id **[V]** [S120]/**[I]**.
- **MCP:** in `crushrc` (now a Bash-like file) or the old JSON config (`type stdio|http|sse`, `headers`, `disabled_tools`); no per-run flag **[V]** [S121, S122].
- **Model:** `-m provider/model`, `--reasoning-effort` **[V]** [S120].
- **Permissions:** **`crush run` approves every permission request of its session** (v0.102.0) **[V]** [S120, S123]. Lists exist in the config (`allowed_tools`, `allowed_commands`, `blocked_commands`) **[V]** [S122]. **Hersir's Find installed CLIs offers Crush as Other with `run --quiet` (`src-tauri/src/clis.rs:109`) **[V]**, so an agent on it runs any command: the agent form should say so today **[I]**.**
- **Folders:** `--cwd`, `--data-dir` **[V]** [S124]. **Sign-in:** provider env vars, `crush login`; `CRUSH_GLOBAL_CONFIG`, `CRUSH_GLOBAL_DATA` move its files **[V]** [S121].
- **Sandbox:** none **[V]** [S121]. **Repository config:** `.crushrc`, `crushrc`, many context files (`CLAUDE.md`, `GEMINI.md`, `AGENTS.md`, `.cursor/rules/`, …), skills folders **[V]** [S121, S122].
- **ACP:** not in the official repository **[V]** [S121].
- **Licence and systems:** **FSL-1.1-MIT** (source-available, MIT later); v0.102.0 of 19 Nov 2026; Linux, macOS, Windows, BSDs **[V]** (npm, [S121]).

#### 3.1.11 Factory Droid

- **Headless:** `droid exec [prompt]`, `-f <file>` or stdin; read-only by default; an action beyond its autonomy level "stops at that action, prints an error to stderr, and exits non-zero" **[V]** [S130].
- **Output:** `-o json` (one object `{type: "result", subtype, is_error, result, session_id, num_turns, usage{input_tokens, output_tokens, cache_read_input_tokens, cache_creation_input_tokens}, factory_credits}`), `stream-json` (`system`, `message`, `tool_call`, `tool_result`, `completion`), `stream-jsonrpc`, `acp` **[V]** [S130, S131]. Cost in Factory credits, not dollars **[V]**.
- **Resume:** `droid exec -s <session-id>`; `--fork <id>`; the id is in the output **[V]** [S130, S132]. Sessions sync to Factory's web app by default (`cloudSessionSync`) **[V]** [S133].
- **MCP:** `~/.factory/mcp.json` and the project's `.factory/mcp.json`; no per-run flag and no way to ignore them documented **[V]** [S134].
- **Model:** `-m`, `-r <effort>` **[V]** [S132].
- **Permissions:** `--auto low|medium|high`, `--skip-permissions-unsafe`; `permissionRules` (allow, ask, block) in settings **[V]** [S130, S133].
- **Folders:** `--cwd`; no extra folders documented **[V]** [S132]. **Sign-in:** `FACTORY_API_KEY`; no variable to move `~/.factory` documented **[V]** [S130, S132].
- **Sandbox:** a `sandbox` settings object (`filesystem`, `network`) **[V]** [S133]. **Repository config:** `.factory/settings.json`, `.factory/mcp.json`, `AGENTS.md` **[V]** [S133].
- **ACP:** native (`droid exec --output-format acp`) **[V]** [S135].
- **Licence and systems:** proprietary (npm `UNLICENSED`); v1.20.0 of 25 Nov 2026; Linux, macOS, Windows **[V]** (npm, [S132]).

#### 3.1.12 Other current CLIs, in short

Seven more are current and used (versions from npm, PyPI or their changelog, all read 2026-11-25). None copies Claude Code's stream-json, and all seven speak ACP **[V]** [S140–S146]/**[I]**.

| CLI (maker, licence, version) | Headless and prompt | Output | Resume | MCP per run | Approval when nobody can answer | Sign-in, second account | ACP |
|---|---|---|---|---|---|---|---|
| **Kilo CLI** (Kilo Code, MIT, 7.8.32, 20 Nov 2026), **a fork of OpenCode** that takes the same config | `kilo run [message]`, argument | `--format json` (events not documented; OpenCode's **[I]**) | `-s <id>`, `-c`, `--fork` | `KILO_CONFIG_CONTENT` **[I]** | **rejected, and the run exits 1**, unless `--auto`; rules `"bash": {"git *": "allow"}` | `kilo auth`; XDG folders **[I]** | `kilo acp` |
| **Cline CLI** (Cline, Apache-2.0, 3.0.75, 21 Nov 2026) | `cline "<prompt>"` or stdin | `--json` NDJSON, its own (`agent_event`, `run_result {usage, text}`) | `--id <id>` | `CLINE_MCP_SETTINGS_PATH` | **`--auto-approve` defaults to true** (everything runs); no command list | `cline auth`; `CLINE_DIR` | `--acp` |
| **Grok Build** (xAI, Apache-2.0, version not published) | `grok -p "<prompt>"` | `--output-format streaming-json` (not documented) | `-s <uuid>`, **chosen by the caller** | not documented | `--allow`, `--deny`, `--always-approve`; takes Claude Code's flag names (`--allowedTools`, `--append-system-prompt`) | browser, `XAI_API_KEY` | `grok agent stdio` |
| **Kimi Code** (Moonshot AI, MIT, 2.8.0, 25 Nov 2026; replaces the archived Kimi CLI) | `kimi -p "<prompt>"` | `--output-format stream-json` (OpenAI-like messages, no usage) | `-S <id>`, reported | `KIMI_CODE_HOME` (the whole home) | `--yolo`, `--auto`; `[[permission.rules]]` `Bash(rm -rf*)` | OAuth or key; `KIMI_CODE_HOME` | `kimi acp` |
| **Auggie** (Augment Code, proprietary, 1.2.3, 25 Nov 2026) | `auggie --print "<prompt>"`, stdin, `@file` | `--mode json` (fields not documented), `--show-cost` | `-c`, `--session <file>` | **`--mcp-config <json-or-path>`** | `--permission bash:ask-user` per run | `auggie auth login`; `--augment-cache-dir` | `--acp` |
| **Mistral Vibe** (Mistral, Apache-2.0, 3.1.5, 24 Nov 2026) | `vibe -p "<prompt>"`, stdin, `--prompt-file` | `--output streaming` (NDJSON) and `export.json` with tokens and **cost** | `--resume <id>`, reported | `VIBE_HOME`, `VIBE_<KEY>` | **denied, and the agent is told**; `--enabled-tools`; `outside_workdir = "deny"` | account or key in `~/.vibe/.env`; `VIBE_HOME` | `vibe-acp` |
| **Kiro CLI** (AWS, closed, 3.6.0, 24 Nov 2026; was Amazon Q Developer CLI) | `kiro-cli chat --no-interactive "<prompt>"` or stdin | `--output-format stream-json` (only errors documented) | `--resume-id <id>` | per agent (`--agent`) **[I]** | `--trust-tools <list>`, `--trust-all-tools`; denied otherwise | browser or device login, `KIRO_API_KEY`; `KIRO_HOME` | `kiro-cli acp` |

**Dropped:** the Python Kimi CLI (archived), Roo Code (archived May 2026), Plandex (no push since Oct 2025), iFlow CLI (no push since Mar 2026), the community "Grok CLI" (overtaken by Grok Build), Amazon Q Developer CLI (now Kiro) **[V]** [S147]. **Not checked further:** Continue's `cn`, Warp, OpenHands CLI, Letta Code, Qoder CLI, JetBrains Junie, and the **Pi coding agent** (`--mode json`, `--mode rpc`, ACP through `pi-acp`), which looks worth a later look **[I]** [S147, S75].

#### 3.1.13 The Agent Client Protocol (ACP): one protocol for many agents

ACP is to coding agents what LSP is to language servers: a client (an editor, or Hersir) starts the agent as a subprocess and talks JSON-RPC 2.0 with it over stdio **[V]** [S60, S61].

- **Who and what:** governed by Zed and JetBrains for now, with a foundation as the stated goal; every repository Apache-2.0 **[V]** [S62]. Protocol v1 is stable; v2 became a preview on 20 Nov 2026, and "v1-only agents and clients will be common for some time" **[V]** [S63]. Rust crate `agent-client-protocol` 5.6.1 (25 Nov 2026, Apache-2.0, features `process` and `stdio`) **[V]** [S64]; TypeScript SDK `@agentclientprotocol/sdk` 2.12.0 **[V]** (npm).
- **Transport:** newline-delimited JSON on the agent's stdout and stdin; the agent "MUST NOT write anything to its stdout that is not a valid ACP message" **[V]** [S65]. So the process stays open for the whole run, and Hersir writes to its stdin during the run: today's runner closes stdin after the prompt (`process.rs:332`) **[V]**.
- **A run:** `initialize` (capabilities: `loadSession`, `mcpCapabilities.http`, `sessionCapabilities.additionalDirectories`, `…systemPrompt`, `…resume`) → `session/new {cwd, mcpServers, additionalDirectories?, systemPrompt?}` (the **agent** chooses the session id) → `session/prompt {sessionId, prompt}` → `session/update` notifications → a `stopReason` (`end_turn`, `max_tokens`, `max_turn_requests`, `refusal`, `cancelled`) **[V]** [S66, S67, S68].
- **Events:** `agent_message_chunk`, `agent_thought_chunk`, `tool_call {toolCallId, title, kind: read|edit|delete|move|search|execute|think|fetch|other, status, rawInput}`, `tool_call_update {status: in_progress|completed|failed, content, rawOutput}`, `plan`, `usage_update {used, size, cost?}` (context size and a running cost, stable since 5 Jun 2026), and more **[V]** [S68, S69, S70]. Tokens per turn are only a draft (`unstable_end_turn_token_usage`) **[V]** [S71].
- **Resume:** `session/load` (replays the history) or `session/resume` (doesn't), each when the agent offers it **[V]** [S67].
- **MCP servers:** in `session/new`: stdio `{name, command, args, env}` always; `http` with headers when the agent offers it. No files to write **[V]** [S67].
- **Permissions:** the agent **may** ask with `session/request_permission {toolCall, options: allow_once|allow_always|reject_once|reject_always}`, and the client answers. The spec doesn't say which tools must ask; an agent in a loose mode, or with its own allow rules, never asks **[V]** [S69]. So Hersir can refuse what reaches it, but it can't make an agent ask; and `rawInput` (the command) has each agent's own shape **[I]**. Client-side `terminal/*` and `fs/*` exist, but agents don't have to use them, and v2 drops them **[V]** [S72].
- **What ACP doesn't give:** subscription limits ("a future RFD"), tokens per turn (draft), an allowed-commands list or sandbox policy, read-only folders, a turn or budget cap; sign-in is the agent's own **[V]** [S70, S72]/**[I]**.
- **Who speaks it:** the registry (`https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json`, 46 agents, each with `cmd`, `args`, `env` per platform) **[V]** [S73, S74]. Natively: Gemini (`--acp`), OpenCode (`opencode acp`), Goose (`goose acp`), Qwen Code (`--acp`), Kimi Code (`kimi acp`), Auggie (`--acp`), GitHub Copilot CLI (`--acp`, public preview), Cursor (`cursor-agent acp`), Kiro (`kiro-cli acp`), Cline (`--acp`), Factory Droid (`droid exec --output-format acp-daemon`), Junie, Mistral Vibe (`vibe-acp`). Through adapters: Claude Code (`@agentclientprotocol/claude-agent-acp`), Codex (`@agentclientprotocol/codex-acp`), Amp (a third-party `amp-acp`) **[V]** [S74, S75].
- **Headless clients exist:** `acpx` ("Headless CLI client for the Agent Client Protocol", MIT) **[V]** (npm); others seen only in search results **[I]**.

**What it means for Hersir [I]:** ACP is the one interface most current CLIs share, and it covers the Run panel, resume, MCP servers, extra folders and an appended system prompt the same way for all of them. It doesn't cover usage tokens, limits or allowed commands, and an ACP agent's permissions still have to be set in its own terms (its mode, its config) before Hersir's answers to `request_permission` mean anything. So ACP replaces the `events`, `session`, `mcp` and `folders` sections of a profile, not its `permissions` and `guards`.

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

## 4. Options

### A. A code adapter per CLI

One more `Kind` per CLI (`Kind::OpenCode`, `Kind::Cursor`, …), each with its command line and parser in `cli.rs`, its notes in `tool_catalog.rs`, `prompt.rs`, `mcp_servers.rs`, `limits.rs`, and its modes and texts in `src/lib/clis.ts`, as Codex and Gemini were added.

- **Takes:** per CLI about what Gemini took: its part of `cli.rs` (`gemini_args`, `gemini_env`, `Gemini` parser: about 300 lines), plus its notes in five files and the front end, and tests **[V]** (`cli.rs:276`–`:440`, `:786`–`:862`). No library.
- **Good:** each CLI exactly right, including its security quirks; nothing new to learn.
- **Risks:** every CLI and every change of a CLI's flags or events needs a Hersir release; the per-kind knowledge stays spread over the backend and `src/lib/clis.ts`; a user can't add a CLI beyond Other's plain text.
- **Size:** M per CLI (one card each).

### B. CLI profiles: data for the command line and a mapping for the events (recommended)

The design of §3.3: a profile engine in `gizai-agents`, the per-kind knowledge moved behind `list_clis`, and OpenCode as the first shipped profile.

- **Takes:** the profile types, the template renderer, the event engine and the `run_config` writer (about 1,000 to 1,500 lines of Rust with tests **[I]**); `list_clis` with `caps`; Settings and the agent form reading `caps`; the OpenCode profile. Libraries: none new (`serde`, `serde_json` with `Value::pointer`, `toml` if a profile writes TOML; all already in the workspace **[V]** `cargo tree` not needed: `cli.rs` and `codex_args` use them).
- **Good:** a new CLI with JSON output is a JSON file and its fixture; you can add one yourself; most current CLIs print JSONL (§3.1 overview): OpenCode, Cursor, Copilot, Amp, Goose, Droid, Qwen Code, Kimi, Auggie.
- **Risks:** a CLI whose output the vocabulary can't express needs a new emit (code) or stays Other; a CLI that changes its events breaks the profile until the next release (Settings warns on a new major version); a profile you add can claim more than its CLI keeps (it shows "not checked by Hersir", and the Chief's changes wait for Approve).
- **Size:** L for the engine and OpenCode (two cards), then S per shipped profile.

### C. An ACP client: one protocol for every agent that speaks it

Hersir talks ACP (§3.1.13) to the agent: `session/new` with the worktree, the run's MCP servers, the agent's folders and the run's rules as an appended system prompt; `session/prompt` with the card; `session/update` as `RunEvent`s; `session/request_permission` answered by Hersir; `session/load` or `session/resume` for Continue. A profile for an ACP agent is then only `start`, `permissions` (the agent's own mode and rules) and `guards`.

- **Takes:** the `agent-client-protocol` crate (5.6.1, Apache-2.0, Zed and JetBrains, released 25 Nov 2026 **[V]** [S64]); a runner that keeps stdin open and answers requests (today's closes it, `process.rs:332`); the mapping from `session/update` to `RunEvent`; answering permission requests from the agent's allowed commands, which needs each agent's `rawInput` shape **[I]**.
- **Good:** one client covers 13 native agents and three through adapters **[V]** [S74]; MCP servers, extra folders, resume and a system prompt without per-CLI files; a refused request is Hersir's own, so the Run panel lists it with Hersir's reason.
- **Risks:** an agent asks only for what its own mode makes it ask, so Hersir still sets each agent's permissions in its own terms; tokens per turn aren't stable in ACP yet (only context size and a running cost) **[V]** [S70, S71]; v2 is in preview and changes `session/load`, modes and permissions **[V]** [S63]; Claude Code and Codex need an extra npm adapter each; several agents mark their ACP mode preview (Copilot) **[V]** [S75].
- **Size:** L (two cards), then S per agent.

### Recommendation

**B now, with OpenCode first; C as the next feature, as a second protocol of the same profiles** **[I]**. B fits how Hersir already runs a CLI (one process per run, the prompt on stdin, JSONL on stdout), gives OpenCode every Hersir feature but subscription limits, and needs no new library. C is worth it once two or three ACP-only agents are wanted (Kiro, Cline, Junie, Auggie), and the profile format leaves room for it: `"protocol": "jsonl" | "text" | "acp"`. Open question 1.

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

## 8. Sources

Every source was read on **2026-11-25**. Versions and dates of npm packages come from `npm view <package> version time.modified license`, run on 2026-11-25 (opencode-ai 1.19.6, @openai/codex 0.172.0, @google/gemini-cli 0.71.1, @qwen-code/qwen-code 0.28.4, @github/copilot 1.0.116, @sourcegraph/amp, @charmland/crush 0.102.0, droid 1.20.0, @agentclientprotocol/sdk 2.12.0, acpx 0.22.0).

**Codex**
- [S20] https://github.com/openai/codex/releases/latest and https://raw.githubusercontent.com/openai/codex/main/README.md
- [S21] https://learn.chatgpt.com/docs/non-interactive-mode (from developers.openai.com/codex/noninteractive) and https://raw.githubusercontent.com/openai/codex/main/codex-rs/exec/src/cli.rs
- [S22] https://raw.githubusercontent.com/openai/codex/main/codex-rs/exec/src/exec_events.rs
- [S23] https://learn.chatgpt.com/docs/config-file/config-reference
- [S24] https://learn.chatgpt.com/docs/agent-approvals-security
- [S25] https://learn.chatgpt.com/docs/auth
- [S26] https://github.com/agentclientprotocol/codex-acp (and the archived github.com/zed-industries/codex-acp)
- [S27] https://raw.githubusercontent.com/openai/codex/main/codex-rs/protocol/src/protocol.rs

**Gemini CLI**
- [S30] https://raw.githubusercontent.com/google-gemini/gemini-cli/main/docs/cli/headless.md and https://github.com/google-gemini/gemini-cli/releases/latest
- [S31] https://raw.githubusercontent.com/google-gemini/gemini-cli/main/packages/core/src/output/types.ts
- [S32] https://raw.githubusercontent.com/google-gemini/gemini-cli/main/packages/cli/src/config/config.ts and https://raw.githubusercontent.com/google-gemini/gemini-cli/main/docs/cli/cli-reference.md
- [S33] https://raw.githubusercontent.com/google-gemini/gemini-cli/main/docs/reference/configuration.md
- [S34] https://raw.githubusercontent.com/google-gemini/gemini-cli/main/docs/reference/policy-engine.md
- [S35] https://geminicli.com/docs/get-started/authentication/
- [S36] https://raw.githubusercontent.com/google-gemini/gemini-cli/main/docs/cli/sandbox.md

**Qwen Code**
- [S40] https://raw.githubusercontent.com/QwenLM/qwen-code/main/docs/users/features/headless.md
- [S41] https://raw.githubusercontent.com/QwenLM/qwen-code/main/packages/cli/src/nonInteractive/types.ts
- [S42] https://raw.githubusercontent.com/QwenLM/qwen-code/main/docs/users/configuration/settings.md (the first 200 of 452 kB)
- [S43] https://raw.githubusercontent.com/QwenLM/qwen-code/main/docs/users/configuration/auth.md
- [S44] https://github.com/QwenLM/qwen-code and https://github.com/QwenLM/qwen-code/releases/latest

**The Agent Client Protocol**
- [S60] https://agentclientprotocol.com/overview/introduction
- [S61] https://agentclientprotocol.com/protocol/overview and https://agentclientprotocol.com/llms.txt
- [S62] https://agentclientprotocol.com/community/governance.md
- [S63] https://agentclientprotocol.com/announcements/acp-v2-preview.md and https://agentclientprotocol.com/protocol/v2/migration.md
- [S64] https://crates.io/api/v1/crates/agent-client-protocol and https://agentclientprotocol.com/libraries/rust.md
- [S65] https://agentclientprotocol.com/protocol/v1/transports.md
- [S66] https://agentclientprotocol.com/protocol/v1/initialization.md and https://agentclientprotocol.com/protocol/v1/authentication.md
- [S67] https://agentclientprotocol.com/protocol/v1/session-setup.md and https://agentclientprotocol.com/protocol/v1/session-list.md
- [S68] https://agentclientprotocol.com/protocol/v1/prompt-turn.md, https://agentclientprotocol.com/protocol/v1/session-modes.md and https://agentclientprotocol.com/protocol/v1/session-config-options.md
- [S69] https://agentclientprotocol.com/protocol/v1/tool-calls.md and https://agentclientprotocol.com/rfds/v2/permission-requests.md
- [S70] https://agentclientprotocol.com/rfds/session-usage.md and https://agentclientprotocol.com/announcements/session-usage-stabilized.md
- [S71] https://agentclientprotocol.com/rfds/end-turn-token-usage.md
- [S72] https://agentclientprotocol.com/protocol/v1/terminals.md, https://agentclientprotocol.com/protocol/v1/file-system.md and https://agentclientprotocol.com/rfds/v2/client-filesystem-terminal-capabilities.md
- [S73] https://agentclientprotocol.com/get-started/registry.md
- [S74] https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json
- [S75] https://agentclientprotocol.com/get-started/agents.md and https://zed.dev/docs/ai/external-agents
- [S76] https://github.com/agentclientprotocol/claude-agent-acp (`src/new-session-meta.ts`, `src/permissions/modes.ts`, `src/acp-client.ts`)

**Aider**
- [S100] https://aider.chat/docs/config/options.html
- [S101] https://aider.chat/docs/scripting.html
- [S102] https://raw.githubusercontent.com/Aider-AI/aider/main/aider/io.py
- [S103] https://github.com/Aider-AI/aider
- [S104] https://pypi.org/project/aider-chat/

**Goose**
- [S110] https://goose-docs.ai/docs/guides/goose-cli-commands
- [S111] https://goose-docs.ai/docs/guides/running-tasks
- [S112] https://raw.githubusercontent.com/aaif-goose/goose/main/crates/goose-cli/src/session/mod.rs
- [S113] https://goose-docs.ai/docs/guides/config-files
- [S114] https://goose-docs.ai/docs/guides/environment-variables
- [S115] https://github.com/aaif-goose/goose and https://github.com/aaif-goose/goose/releases

**Crush**
- [S120] https://raw.githubusercontent.com/charmbracelet/crush/main/internal/cmd/run.go
- [S121] https://github.com/charmbracelet/crush (README)
- [S122] https://raw.githubusercontent.com/charmbracelet/crush/main/internal/config/config.go
- [S123] https://github.com/charmbracelet/crush/releases
- [S124] https://raw.githubusercontent.com/charmbracelet/crush/main/internal/cmd/root.go

**Factory Droid**
- [S130] https://docs.factory.com/cli/droid-exec/overview (from docs.factory.ai)
- [S131] https://docs.factory.com/guides/building/droid-exec-tutorial
- [S132] https://docs.factory.com/reference/cli-reference
- [S133] https://docs.factory.com/cli/configuration/settings
- [S134] https://docs.factory.com/cli/configuration/mcp
- [S135] https://docs.factory.com/integrations/zed

**Cursor CLI**
- [S80] https://cursor.com/docs/cli/installation
- [S81] https://cursor.com/docs/cli/reference/output-format and https://cursor.com/docs/cli/headless
- [S82] https://cursor.com/docs/cli/reference/parameters
- [S83] https://cursor.com/docs/cli/changelog
- [S84] https://cursor.com/docs/cli/mcp and https://cursor.com/docs/context/mcp
- [S85] https://cursor.com/docs/cli/reference/permissions
- [S86] https://cursor.com/docs/reference/sandbox
- [S87] https://cursor.com/docs/cli/reference/authentication
- [S88] https://cursor.com/docs/cli/acp

**GitHub Copilot CLI**
- [S90] https://raw.githubusercontent.com/github/docs/main/content/copilot/reference/copilot-cli-reference/cli-command-reference.md
- [S91] https://raw.githubusercontent.com/github/docs/main/content/copilot/reference/copilot-cli-reference/cli-programmatic-reference.md
- [S92] https://docs.github.com/en/copilot/how-tos/copilot-sdk/features/streaming-events
- [S93] https://raw.githubusercontent.com/github/copilot-cli/main/changelog.md
- [S94] https://github.com/github/copilot-cli

**Amp**
- [S95] https://ampcode.com/docs/cli and https://ampcode.com/docs/cli/execute-mode
- [S96] https://ampcode.com/docs/cli/streaming-json
- [S97] https://ampcode.com/docs/cli/settings
- [S98] https://ampcode.com/docs/customize/mcp
- [S99] https://ampcode.com/manual
- [S99a] https://ampcode.com/news/neo

**Other CLIs**
- [S140] Kilo: https://kilo.ai/docs/cli, https://kilo.ai/docs/code-with-ai/platforms/cli-reference, https://api.github.com/repos/Kilo-Org/kilocode/releases?per_page=5
- [S141] Cline: https://docs.cline.bot/cline-cli/cli-reference, https://registry.npmjs.org/cline
- [S142] Grok Build: https://github.com/xai-org/grok-build, https://docs.x.ai/build/overview, https://docs.x.ai/build/cli/reference, https://docs.x.ai/build/cli/headless-scripting
- [S143] Kimi Code: https://github.com/MoonshotAI/kimi-code, https://moonshotai.github.io/kimi-code/en/reference/kimi-command, https://moonshotai.github.io/kimi-code/en/configuration/config-files, https://moonshotai.github.io/kimi-code/en/guides/getting-started
- [S144] Auggie: https://docs.augmentcode.com/cli/reference, https://registry.npmjs.org/-/v1/search?text=@augmentcode/auggie&size=3
- [S145] Mistral Vibe: https://github.com/mistralai/mistral-vibe, https://github.com/mistralai/mistral-vibe/blob/main/docs/programmatic-mode.md, https://docs.mistral.ai/vibe/code/cli/configuration, https://pypi.org/pypi/mistral-vibe/json
- [S146] Kiro: https://kiro.dev/docs/reference/cli-commands/, https://kiro.dev/docs/cli/headless/, https://kiro.dev/changelog/cli/
- [S147] Activity and stars: the GitHub search API (`api.github.com/search/repositories?q=repo:…`) for the dropped and unchecked candidates; npm registry search for monthly downloads
