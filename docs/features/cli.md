# Bring your own CLI

Each agent runs on the coding CLI you pick, like Claude Code, Codex or Gemini, on your own machine with your own logins and accounts.

## Pick a CLI per agent

- **Claude Code, Codex, Gemini**, or any other coding CLI, whose output Hersir reads as text.
- **A second account** of one, with its own environment (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`).
- **Settings → Coding CLIs** adds them, or finds the ones installed. Each agent picks its CLI under Runs on, in its form.

With only Codex installed, Hersir adds it under Settings → Coding CLIs and runs every agent but the Chief on it.

## On your own machine

- Everything runs on your computer, with your own git repositories and your own logins.
- Agents run on the coding CLIs you installed, with your own logins and accounts.
- Hersir needs no account of its own.
- Your data stays in one folder on your computer, and Hersir sends nothing about you: see [Your data](../install.md#your-data) and [No telemetry](../install.md#no-telemetry).

Hersir never sells Claude usage: agents keep running on your own Claude Code login or API key. On Windows, Hersir runs the CLIs installed on Windows itself, never inside WSL ([Install](../install.md#windows)).

## Your keys stay yours

- MCP sign-ins and values, and your Bitbucket login, stay in your system's keychain.
- Hersir never stores a GitHub token or password: pushes go over SSH with your keys, or over HTTPS with gh's login.
- An agent's folders never include `/`, your home folder, Hersir's data or folders with keys ([Folders](tools.md#folders)).
- **A run gets no other program's keys.** Hersir's environment may hold keys for other programs. A run gets none of the variables named `*_API_KEY`, `*_TOKEN`, `*SECRET*`, `*PASSWORD*` or `AWS_*`, except the ones its own CLI signs in with and the ones its lines in Settings → Coding CLIs set:
  - Claude Code: `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CODE_OAUTH_TOKEN`, `CLAUDE_CODE_OAUTH_REFRESH_TOKEN`, `CLAUDE_TRUSTED_DEVICE_TOKEN`, `ANTHROPIC_FOUNDRY_API_KEY`, `ANTHROPIC_AWS_API_KEY` and `AWS_BEARER_TOKEN_BEDROCK`, and every `AWS_` one when `CLAUDE_CODE_USE_BEDROCK` is on (Amazon Bedrock).
  - Codex: `OPENAI_API_KEY`, `CODEX_API_KEY` and `CODEX_ACCESS_TOKEN`.
  - Gemini: `GEMINI_API_KEY`, `GOOGLE_API_KEY` and `GOOGLE_CLOUD_ACCESS_TOKEN`.
  - Another CLI: none. Give it its key in its lines in Settings → Coding CLIs.

  A run that needs another key (a private package registry's token, a Codex model provider's `env_key`) gets it the same way: add the line to its CLI in Settings → Coding CLIs.
- **A run's MCP config is gone once its CLI has read it.** The file with the MCP servers' secrets, and the token of a chat answer, is deleted as soon as Claude Code's first line says its servers started, and in any case when the run ends or fails to start.
- **A chat answer's token works only for that answer.** Hersir's tools for the Chief take a turn's token only from that turn's own Claude Code and what it started: another run that reads the token is refused ([PLATFORMS.md](../PLATFORMS.md) says how each system checks it).

## The Chief needs Claude Code

The Chief's chat runs on Claude Code. Runs on, under the chat's text box, moves a chat to another Claude Code account: see [The chat](chief.md#the-chat).

## What each CLI gets

- **Claude Code** gets everything: the Chief's chat, MCP servers and the browser, and slash commands and skills.
- **Codex** has its own web search, and its sandbox blocks what it doesn't allow.
- **Gemini** fetches pages, and searches the web by its own policy.
- **Other CLIs** run with their output read as text.

Memory works the same on every CLI. Which tools each CLI can use: [Tools per agent](tools.md).

## Usage

Usage (in Company) shows the agents' input tokens (cache included), output tokens and API cost for today, 7 days, 30 days or this month. It shows them in total with a bar per day, per agent and per project, with the Chief's chat on its own line.

- The Projects list shows each project's API cost this month.
- API cost is what the tokens would cost at API prices, not a bill.
- A CLI that reports no cost (Codex, Gemini) shows its tokens and an unknown cost.

## Subscription limits

Usage → Subscription, the tab the page opens on, has a block per coding CLI in Settings, so two accounts of one CLI show apart.

- A Claude Code account shows its session, weekly and Fable limits; a Codex account its plan's two windows (usually 5 hours and a week).
- Each shows how much is used, when it resets and when the number was read, amber from 80% and red when reached.
- Each lists the agents on it and the chats that run there, so you see whom to move to another account.
- Gemini and other CLIs say Hersir can't read their limits yet.

The numbers are the newest the CLI itself reported in a run or chat turn: Claude Code in its stream, Codex in its session log (`$CODEX_HOME/sessions`), and a run that hit a limit. Hersir never reads their logins or asks Anthropic or OpenAI.

## Read more

- [Install](../install.md): what each system needs, your data, no telemetry and updates.
- [Tools per agent](tools.md): what each agent may use, per CLI.
- [docs/PLATFORMS.md](../PLATFORMS.md#reaching-the-coding-clis): how Hersir finds the coding CLIs on Linux, macOS and Windows.
- [docs/PAID-FEATURES.md](../PAID-FEATURES.md): what Hersir will sell, and why it never sells Claude usage.
