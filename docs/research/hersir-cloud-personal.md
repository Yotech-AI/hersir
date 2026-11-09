# Hersir Cloud for one person: Cloud Backup and Multiple devices

Research for GA-67, 9 November 2026, by the R&D Agent. No code was changed. It turns `docs/PAID-FEATURES.md` (Phase 1 and Phase 2) into build cards.

**[V]** = verified: in this repository (a file and line), or on a primary source read on 2026-11-09 (listed in [Sources](#9-sources)). **[I]** = my judgement.

> Draft 1: the project, the code findings, and the backup and sync design. The outside research (encryption, server, sign-in, payments, legal) is being added.

## 1. Summary

(Written last.)

## 2. The project now

### 2.1 What Hersir does for its users

Hersir is a desktop app (Linux, macOS, Windows) for a developer or a small agency: clients, projects and task cards on a board, worked on by local AI coding agents (Claude Code, Codex and other CLIs) in git worktrees, with a Team Lead chat, an editor, terminals and screenshots of what agents built. It is free, Apache-2.0, and works without an account or network (`README.md`, `docs/PAID-FEATURES.md:7`) [V]. Since 0.16.0 it makes local snapshots to a folder you choose and restores them, also on a new computer (GA-216 to GA-219, `docs/backup-format.md`) [V].

### 2.2 How it is built

| Part | What it does | Where |
|---|---|---|
| Core | SQLite store, domain, workflow, backups and restore; never imports Tauri | `crates/gizai-core` (rusqlite 0.40 bundled, `rusqlite_migration`; schema version 21, `db.rs:12`) |
| Agents | CLIs, worktrees, processes, the OAuth client, the keychain | `crates/gizai-agents` (`ureq` 3 with rustls, `keyring` 3) |
| MCP | the MCP server the agents use | `crates/gizai-mcp` |
| App | the Tauri 2 adapter: commands, schedulers, events | `src-tauri/src` |
| UI | React and TypeScript; talks to the app only through `src/api.ts` | `src/` |
| Design system | tokens and the generator | `design/tokens.json`, `design/README.md` |

- **Data** [V]: one SQLite file, `gizai.db`, in the data folder (`~/.local/share/gizai` on Linux, `~/Library/Application Support/Gizai` on macOS, `%APPDATA%\Gizai` on Windows: `src-tauri/src/lib.rs:161-177`). WAL mode, one writer connection and four read-only ones (`crates/gizai-core/src/db.rs:1-16`). Stored files sit beside it by SHA-256 (`files/<xx>/<sha256>`), run logs in `runs/`, worktrees in `worktrees/`.
- **Secrets** [V]: never in SQLite (`0001_init.sql:378-379`). They go to the OS keychain under the service `gizai` (`crates/gizai-agents/src/secrets.rs:12`), through a `Keychain` trait with an OS, a file (`GIZAI_FAKE_KEYCHAIN`, for headless tests) and a memory implementation (`secrets.rs:19-27`). The 1.0 cards 'Hersir everywhere (1 of 2)' and '(2 of 2)' rename the data folder and the keychain service to hersir for new installs, so the cloud's keychain entries must use whatever `secrets::SERVICE` is then.
- **Writes** [V]: every write is one IMMEDIATE transaction through `Db::write`, and the code calls `w.insert`, `w.update` or `w.delete` to append a row to `changes` (`db.rs:313-376`).
- **Tests, build and release** [V]: `cargo test --workspace`, `npm test`, `npm run build`, and `scripts/ui-test.sh` headless in `cage` (`CLAUDE.md`); CI builds and tests each pull request on Linux, macOS and Windows (`.github/workflows/ci.yml`); a release is a `vX.Y.Z` tag on `production`, which every installed Hersir offers as an update (`docs/RELEASING.md`).

### 2.3 What the two features touch

- **The snapshot and restore of 0.16.0.** Format 1 (`docs/backup-format.md`): a manifest per snapshot, the database copied with `VACUUM INTO` and compressed whole with zstd, and each stored file once by hash. Core: `crates/gizai-core/src/backups.rs` (1,191 lines) and `restore.rs` (757); app: `src-tauri/src/backups.rs` (the schedule: 2 minutes after start, then hourly by the wall clock; On quit) and `src-tauri/src/restore.rs`; UI: `src/components/BackupSettings.tsx` in Settings → General (`src/pages/SettingsPage.tsx:93`) [V]. The format was written for the cloud: "Hersir Cloud will write its backups in the same format, so one restore reads both" (`docs/backup-format.md:3`, and its section *For Hersir Cloud*, lines 198-206) [V].
- **The restore's rules that sync must keep** [V]: a computer keeps its own `devices` row (*the device rule*, `docs/backup-format.md:172-174`); this computer's settings stay its own (`crates/gizai-core/src/scope.rs:12-51`), and "sync will send the shared settings only" (`scope.rs:6-9`); `last_restore` says which snapshot a computer's data starts from (`scope.rs:56-58`).
- **`devices`** (`0001_init.sql:381-384`): `id` (UUIDv7), `name`, `is_self` [V].
- **The OAuth client** (`crates/gizai-agents/src/oauth.rs:1-13`) [V]: authorization code with PKCE S256 and a random state, the answer on a listener on 127.0.0.1, RFC 8414 discovery, RFC 7591 registration as a public client, RFC 8707 resource indicators, RFC 7009 revocation, and a `TokenStore` in the keychain that renews one server's tokens one caller at a time. It was built for MCP servers, and it is exactly the desktop sign-in the plan asks for.
- **Claims** [V]: a run claims its card in the same write that creates the run (`tasks.claimed_by_run_id`, `lease_expires_at`: `crates/gizai-core/src/runs.rs:86-114`), and the dispatcher skips claimed cards (`workflow.rs:189-190`). Both are local.
- **Settings** [V]: tabs General, Appearance, Notifications, Agents and runs, MCP servers, GitHub and Bitbucket (`SettingsPage.tsx:33-36`).

### 2.4 What the code says that the plan doesn't

These change the plan, mostly for Multiple devices.

1. **`changes` holds summaries for the activity feed, not values sync can replay** [V]. A new card logs only `{"identifier", "title"}` (`crates/gizai-core/src/tasks.rs:304`), not its project, column, description or order. A move logs the column *names*, `{"column": [from, to]}`, not `state_id` or `sort_key` (`tasks.rs:496`). Labels log their names (`tasks.rs:521`). A run's insert logs `{task_id, agent, role, trigger}` (`runs.rs:115`). A field edit logs the patch's new values without the old ones (`tasks.rs:435`), although the schema's comment says "old and new per field" (`0001_init.sql:396`).
2. **Some writes log nothing** [V]: settings (`settings.rs:23-26`), a card's labels (deleted and inserted again on every change, `tasks.rs:512-515`), a column's agents (the same, `columns.rs:128-133`), the project's next task number (`tasks.rs:282-284`), the claim (`runs.rs:113`) and runs' status (`runs.rs:159, 212, 241`). The code has about 200 write statements across about 40 files.
3. **The clock isn't a hybrid logical clock yet** [V]. `hlc` is the wall clock's milliseconds, a counter that starts at 0 at each start and wraps at 10,000, and the first 8 characters of the device id (`db.rs:364-366`). It never moves past a clock received from another device, so a computer whose clock is behind writes changes that look older than they are. Its device part is the id's creation time (UUIDv7), which two devices can share (`docs/backup-format.md:206`).
4. **Task numbers collide offline** [V]. A card's number comes from `projects.next_task_number` (`tasks.rs:282-285`) and identifiers are unique per organisation (`0001_init.sql:265`): two computers offline both make `EUQR-13`, and applying the second insert fails the constraint.
5. **Some columns of shared rows belong to one computer** [V]: `repos.local_path`, `agent_configs.folders_json` and `default_cwd`, `projects.folder_path`, `docs.mirror_path`, `chat_threads.session_id` and `session_cli` (a CLI session lives in that computer's CLI account), the claim columns, and `runs.cwd`, `worktree_path` and `pid` (`docs/backup-format.md:205`, `0001_init.sql`, `0012_chat_runs_on.sql:4-5`).
6. **Snapshots are plain and compressed whole** [V] (`docs/backup-format.md:5`, `:74`). Right for a folder; for the cloud they need encryption, and a whole-file zstd stream changes nearly everywhere when one page changes, so 90 daily copies would not share storage.

## 3. The feature

### 3.1 What it should do (from the card)

- **Cloud Backup**: "Encrypted backups of your data, kept off your computer. Restore them on a new one." Daily snapshots kept 90 days, a 10 GB quota, restore on a clean computer; a lapsed plan stops uploads, not restores (`docs/PAID-FEATURES.md:104-124`).
- **Multiple devices**: "Your clients, projects and cards in sync on each of your computers." Every computer keeps a full local copy that works offline; agents run on the computer they were set up on, and a computer claims a card in the cloud before its agent starts it (the first claim wins).
- Both are in the Personal plan (€5 a month or €48 a year, excl. VAT). The app stays free and complete without an account; Hersir sells hosted services only, never Claude or Codex usage.
- Out of scope: the phone chat, agents across devices or on servers, teams and SSO.

(The answers to questions 1 to 8 follow.)
