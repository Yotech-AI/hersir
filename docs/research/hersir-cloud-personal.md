# Hersir Cloud for one person: Cloud Backup, Hersir on another computer, the PWA, and Multiple devices

Research for GA-67 by the R&D Agent. Round 1: 9 November 2026. Round 2: 17 November 2026, rewritten to Jefsev's twelve decisions of 17 November ([section 0](#0-jefsevs-decisions-17-november-2026)). **Round 3: 20 November 2026**, for Jefsev's two requirements of 20 November ([section 0.1](#01-jefsevs-requirements-20-november-2026)): the whole Hersir in a PWA, and Hersir on the MacBook with the cards on the PC. No code was changed. It turns `docs/PAID-FEATURES.md` (Phase 1 and Phase 2) into build cards.

- **[V]** = verified: in this repository (a file and line), or on a primary source (listed in [Sources](#9-sources) with the date read). **[I]** = my judgement.
- **Replaced** marks an earlier recommendation that a later decision or requirement replaced. It stays only where it helps to see why.
- Round 3's code citations are checked against main at `723097f` (Hersir 0.19.0, 19 Nov 2026) with `git show` and `git grep`; Round 2's against `99ce4e3` (0.18.0), and those Round 3 relies on were checked again at `723097f`. This branch could not merge main in Round 2 (`git merge` was refused), so the files in the branch's own tree are older than some lines cited.
- **Where to start reading Round 3**: the requirements and Jefsev's choice of Remote (0.1), the summary (1), hosts and their projects (3.11), what a remote device may do (3.12), security and pairing (3.13), the PWA (3.14), the relay (3.15), plans (3.16), the options (4), the design (5.8), the cards (6) and the questions (7).

## 0. Jefsev's decisions (17 November 2026)

These close Round 1's twelve open questions (section 7). Where they differ from Round 1 or from the card's first brief (the 9 Oct choices, 'no home-made login', a merchant of record, the plan's Rust server), they count.

| # | Decision | Closes Round 1's | Where in this document |
|---|---|---|---|
| 1 | **End-to-end encryption** for Cloud Backup and for sync. | Q1 | 3.2 |
| 2 | **A recovery code only**, no passphrase. The app warns clearly to keep it: lose the code and every computer, and the data is gone. | Q2 | 3.2, 5.1 |
| 3 | **SQLite with Litestream** on the server, not Postgres. | Q3 | 3.3 |
| 4 | **Our own Laravel app**, not Rust, no Zitadel: a new private repository; a Laravel Forge server in the EU; Fortify for sign-up, sign-in, password reset, email verification and 2FA; the desktop signs in through the browser with `oauth.rs` against Passport; each computer gets its own token, which the account page can withdraw; Cashier with Stripe; backups and Litestream's copies on Cloudflare R2 in its EU jurisdiction. | Q3, Q4 | 3.3, 3.4 |
| 5 | **Email and password, plus 2FA with an authenticator app** (TOTP, with Fortify's recovery codes), required for every account. No magic links, no SMS or email codes. | Q5 | 3.4 |
| 6 | **Agents and the Chief run only on a computer's CLI.** No agent starts by itself while its computer is offline: card claims need the cloud. | Q6 | 3.7 |
| 7 | **Sell Cloud Backup first**, then sync. | Q7 | 3.10 |
| 8 | **Keep every backup 7 days**, then one a day up to 90 days. | Q8 | 3.6 |
| 9 | **Payments through Stripe** (the S.L. has it), not Paddle: Cashier, Stripe Checkout and Stripe Tax. | Q9 | 3.5 |
| 10 | **`cloud.hersir.dev`** for the account pages and the API, for now. The DNS is on Cloudflare. | Q10 | 3.3 |
| 11 | **The server's code in a private repository**; the app stays open source. | Q11 | 3.3 |
| 12 | **The Chief's chat is writable on every computer**, and later in a PWA. | Q12 | 3.8 |

**What Round 2 replaced** (Round 1's recommendation → the decision): Rust with axum → Laravel (4); Zitadel Cloud with passkeys, Google and GitHub → Fortify with email, password and required TOTP (4, 5); Hersir Cloud's own device token after a Zitadel sign-in → a Passport token per computer (4); Paddle as merchant of record → Stripe direct with Stripe Tax (9); Hetzner Object Storage → Cloudflare R2, EU jurisdiction (4); `api.hersir.dev`, `account.hersir.dev`, `login.hersir.dev` → `cloud.hersir.dev` (10); the Chief's chat read-only on other computers → writable (12).

**What still holds from Round 1**: the account key and recovery code (now without storing the code on computers, 3.2); format 1's manifest with chunked, encrypted, keyed-hash uploads (3.6); the trigger-written `sync_log`, the hybrid logical clock, last writer wins per field and card numbers settled in the server's order (3.7); cloud claims before a run starts (3.7); the legal list (3.9, updated for selling directly).

## 0.1 Jefsev's requirements (20 November 2026)

Added in chat on 20 November, on top of the decisions above, which still count. Where Round 1 or Round 2 put the phone chat or agents across devices out of scope, these count: both are in scope, up to their build cards.

| # | Requirement | Where in this document |
|---|---|---|
| R1 | **The PWA is the whole Hersir, not only the chat**: the Kanban board, the cards (comments, files, runs), the Chief's chat with its Approve and Discard buttons, and the Inbox, on the phone and in any browser, through Hersir Cloud. | 3.14, 5.8, cards 10 to 12 |
| R2 | **Hersir on the MacBook, with the cards on the PC**: the project folders, the agents with their CLIs and the runs stay on the PC. On the MacBook, Jefsev sees and does everything on the board: the cards, the Chief's chat and its approvals, and Run, Stop and Continue. The PC does the work. | 3.11, 3.12, 5.8, cards 7 to 9 |

**Jefsev chose Remote** (chat, 20 November, through the Master Chief), so this round designs Remote and drops what it makes unneeded, instead of comparing it with Round 2's Sync:

| # | Jefsev's choice (20 Nov) | Where |
|---|---|---|
| C1 | **A project lives on the device it was created on**: its folder, cards, agents, runs and data stay there. The same repository added on another device is a different project. | 3.11 |
| C2 | **The MacBook's app and the PWA work with it through Remote**: they send its host's own commands through Hersir Cloud, end-to-end encrypted, and the host answers. The host must stay on; while it is off, its projects say so on the other devices. | 3.11, 3.12 |
| C3 | **"Sync between devices" now means that every device shows the projects of all your devices**, each answered by its own host. Nothing is copied. | 3.11 |
| C4 | **Round 2's sync engine** (`sync_log`, the clock, last writer wins per field, card numbers in the server's order, card claims) **isn't needed**: say what of it still is, and mark the rest replaced. Cloud Backup stays as designed. | 3.7 |
| C5 | **Projects in the cloud**, so a device can work with a project while its host is off, come later, after the paid features: out of scope, but not made harder. | 3.11 |

**Jefsev's answers on top of it** (chat, 20 November, through the Master Chief):

| # | Answer | Where |
|---|---|---|
| A1 | **A repository that is already a project on another of your devices becomes a separate project**, and Hersir says so when it is added, so you know. | 3.11, *The same repository on two devices* |
| A2 | **While a host is off, the other devices still show its projects, grayed out, with an icon that says the device is offline.** A click on one is designed here. | 3.11, *A host that is off*; 5.8 |
| A3 | **No copies at all**, not even a read-only one for while a host is off: the host stays on. (Round 3's first draft offered one as a question; it is dropped.) | 3.11 |
| A4 | **Running projects in the cloud stays out of scope for a while.** | 3.11 |

**What this replaces in Round 2** [I]: the sync engine and its server side (3.7: `sync_log`, `sync_clock`, `cloud_link`, the HLC, last writer wins, numbering in the server's order, home computers and cloud claims, the server's `workspaces`, `txs`, `file_sets` and `claims`); the Chief's chat by sealed commands (3.8: commands, outbox, roster, the Chief's computer), because with Remote the screens call the host's own chat and approval commands; and Round 2's cards 6 to 11. The PWA gets its own device key by pairing and never the account key (3.13, replacing question 17).

## 1. Summary

1. **Remote, as Jefsev chose** (0.1): each project lives on its **host**, the computer it was made on. Every other device (the MacBook's Hersir, the PWA) shows **the projects of all your hosts**, one host per workspace in a rail at the window's left edge, and sends each host its own commands through Hersir Cloud, end-to-end encrypted; the host answers from its own database and runs. Nothing is copied, not even a read-only copy (A3). A host that is off stays in the rail, grayed out with an offline icon; a click on it, or on one of its projects, shows since when it is off and what to do, never stale data (A2, 3.11). A repository that is already a project on another of your hosts can be added again, and Hersir says it becomes a separate project (A1). Each host keeps its own Chief, agents, clients, people and memory.
2. **The screens stay as they are** [V]: every screen talks to the backend through `invoke` and events in `src/api.ts`, which `npm run browser` already swaps for a fake (`src/browser/main.ts`). On the MacBook, Hersir's own Rust side forwards each command to the host of the workspace on screen (through Tauri's app-level invoke handler), so its screens, dialogs and file drops work unchanged; the PWA swaps the IPC as the browser mode does. About **21 to 29 agent-days** for both requirements (cards 7 to 12), on top of the account server; the MacBook can be used first (cards 2, 7, 8 and 9, about 9 to 12 agent-days on the critical path), then the phone (cards 10 to 12). A command takes about 0.1 to 0.2 s instead of 1 ms. Of the 224 commands, 129 work remotely as they are, 8 need a change (files in and out, a binary answer, and the project and settings forms), and 87 stay on the host (the terminal, the editor, folders, the keychain, the updater, and everything that widens what an agent may run); the run and chat streams are forwarded (3.12).
3. **Security** (3.13): a device is paired from a QR code (or a long code) shown on the PC, holds its own key pair (in the keychain, or a non-extractable WebCrypto key in the PWA), and talks to the PC in a Noise session the server can't read. Removing a device, from the account page or on the PC, ends it at once. A web app's code comes from a server, so the PWA is built in this public repository, served from its own origin (`app.hersir.dev`), deployed only from a signed release, and bounded by the PC's allowlist; the PC shows every connected device and has a switch that turns remote use off.
4. **The relay** (3.15) is Laravel Reverb's client events on one private channel per host and device, inside the Round 2 Laravel app; files and big answers go sealed through R2. About €0 extra at 10 and 100 customers and about €7 a month at 1,000. **Web Push** for the Chief's questions, holds and approvals works on iOS only for a PWA added to the Home Screen (3.14).
5. **Plans** (3.16): Hersir on your other devices and the PWA go into Personal (€5 a month, €48 a year) with Cloud Backup, which is still what is sold first (decision 7). Using a host from its own screen stays free.
6. **Round 2 still holds** for the server (Laravel 13 at `cloud.hersir.dev`, Fortify with TOTP required, Passport with `oauth.rs` as it is, Cashier, SQLite with Litestream, R2 EU; with the relay about €22, €31 and €148 a month at 10, 100 and 1,000 customers, 3.3), payments (Stripe direct; against Managed Payments for now, 3.5) and Cloud Backup (3.6). Round 2's sync engine and chat commands are replaced (3.7, 3.8), and with them decision 6's cloud claims: a host's agents keep working while it is offline, as today; twelve cards in two repositories (section 6).
7. **Open questions** 13 to 27 (section 7): VERI*FACTU invoicing by 1 January 2027 still leads; new ones ask whether a host may keep itself awake while remote use is on, where the PWA is served, and what a paired device may do.

## 2. The project now

### 2.1 What Hersir does for its users

Hersir is a desktop app (Linux, macOS, Windows) for a developer or a small agency: clients, projects and task cards on a board, worked on by local AI coding agents (Claude Code, Codex and other CLIs) in git worktrees, with a chat with **the Chief** (the agent that looks after your board, until 0.17 called the Team Lead), an editor, terminals and screenshots of what agents built. It is free, Apache-2.0, and works without an account or network (`README.md:11-13, 159`, `docs/PAID-FEATURES.md:30-40`) [V]. Since 0.16.0 it makes local snapshots to a folder you choose and restores them, also on a new computer (GA-216 to GA-219, `docs/backup-format.md`) [V]. Main is at 0.18.0 (`src-tauri/tauri.conf.json:5`) [V].

### 2.2 How it is built

| Part | What it does | Where |
|---|---|---|
| Core | SQLite store, domain, workflow, the Chief's chat and pending actions, backups and restore; never imports Tauri | `crates/gizai-core` (rusqlite 0.41 bundled, `rusqlite_migration`; schema version 22, `db.rs:12`) |
| Agents | CLIs, worktrees, processes, the OAuth client, the keychain, release signatures | `crates/gizai-agents` (`ureq` 3 with rustls, `keyring` 3, `ssh-key` with ed25519) |
| MCP | the MCP server the agents and the Chief use | `crates/gizai-mcp` |
| App | the Tauri 2 adapter: commands, schedulers (backups, the board check), events | `src-tauri/src` |
| UI | React and TypeScript; talks to the app only through `src/api.ts` | `src/` |
| Design system | tokens and the generator | `design/tokens.json`, `design/README.md` |

- **Data** [V]: one SQLite file in the data folder: `HERSIR_DATA_DIR`, else `GIZAI_DATA_DIR`, else `~/.local/share/hersir` on Linux, `~/Library/Application Support/Hersir` on macOS, `%APPDATA%\Hersir` on Windows, with the older `gizai` folders still found (`src-tauri/src/lib.rs:159-190`). WAL mode, one writer connection and a few read-only ones (`crates/gizai-core/src/db.rs:1-16`). Stored files sit beside it by SHA-256 (`files/<xx>/<sha256>`), run logs in `runs/`, worktrees in `worktrees/`, the Chief's code copies in `code/`.
- **Secrets** [V]: never in SQLite (`0001_init.sql:378-380`). They go to the OS keychain under the service `hersir` (`crates/gizai-agents/src/secrets.rs:14`), with values saved under `gizai` before 0.17.0 still found (`:17`), through a `Keychain` trait (`:23`) with an OS, a file (`HERSIR_FAKE_KEYCHAIN`, for headless tests, `:249`) and a memory implementation. The 1.0 rename Round 1 waited for is done.
- **Writes** [V]: every write is one IMMEDIATE transaction through `Db::write` (`db.rs:314`), and the code calls `w.insert`, `w.update` or `w.delete` (`db.rs:352-361`) to append a row to `changes` (`db.rs:364-377`).
- **Tests, build and release** [V]: `cargo test --workspace`, `npm test`, `npm run build`, and `scripts/ui-test.sh` headless (`CLAUDE.md`); CI on Linux, macOS and Windows (`.github/workflows/ci.yml`); a release is a `vX.Y.Z` tag on `production`, signed (`crates/gizai-agents/src/release_sig.rs:1-12`), which every installed Hersir offers as an update (`docs/RELEASING.md`).

### 2.3 What the three features touch

- **The snapshot and restore of 0.16.0.** Format 1 (`docs/backup-format.md`): a manifest per snapshot, the database copied with `VACUUM INTO` and compressed whole with zstd, each stored file once by hash. Core: `crates/gizai-core/src/backups.rs` (1,191 lines) and `restore.rs` (757); app: `src-tauri/src/backups.rs` and `restore.rs`; UI: `src/components/BackupSettings.tsx` in Settings → General (`src/pages/SettingsPage.tsx:101`) [V]. The format was written for the cloud: "Hersir Cloud will write its backups in the same format … so one restore reads both" (`docs/backup-format.md:3`; its section *For Hersir Cloud*, lines 198-206) [V].
- **The restore's rules that sync must keep** [V]: a computer keeps its own `devices` row (`docs/backup-format.md:172-174`); this computer's settings stay its own (`crates/gizai-core/src/scope.rs:12-52`), and "Sync … will send the shared settings only" (`scope.rs:6-9`); `last_restore` says which snapshot a computer's data starts from (`scope.rs:57-59`).
- **`devices`** (`0001_init.sql:381-384`): `id` (UUIDv7), `name`, `is_self` [V].
- **The OAuth client** (`crates/gizai-agents/src/oauth.rs:1-13`) [V]: `discover` reads the protected resource metadata (RFC 9728) and then the authorization server's metadata (RFC 8414, else OpenID discovery) (`:169-250`); `begin` binds 127.0.0.1 on a free port, registers Hersir (RFC 7591) **only when no client id is given** (`:293-302`), and builds the address with PKCE S256, a random state and the resource (RFC 8707) (`:311-316`); `finish` takes the answer and exchanges the code (`:362`); `TokenStore` keeps tokens in the keychain, renews them one caller at a time, and withdraws them on sign-out (RFC 7009). Built for MCP servers; it is the desktop sign-in decision 4 asks for.
- **Claims** [V]: a run claims its card in the same write that creates the run (`tasks.claimed_by_run_id`, `lease_expires_at`: `crates/gizai-core/src/runs.rs:86-119`), the dispatcher skips claimed cards (`workflow.rs:189-190`), and a run's end releases its claim (`runs.rs:253`). All local.
- **The Chief's chat** [V]: threads and messages (`0001_init.sql:342-359`); each thread's CLI session (`chat_threads.session_id`, `session_cli`: a session lives in one computer's CLI account, `0012_chat_runs_on.sql`, `0003_chat.sql`); messages sent while the Chief answers wait in `chat_queue` (`0003_chat.sql`); the calls that wait for your OK are `pending_actions` (`0017_pending_actions.sql`), approved by `src-tauri/src/pending.rs:114` (`approve`), `:136` (`approve_all`) and `:152` (`discard`), with `crates/gizai-core/src/pending.rs:155` (`claim`) making sure one is run once; a chat that used a tool from outside Hersir stays marked (`0022_outside_marks.sql`). The board check starts in `src-tauri/src/chat.rs:1611` (`start_check`); agents ask the Chief first through `crates/gizai-core/src/questions.rs`. What a user sees: `docs/features/chief.md`.
- **Settings** [V]: tabs General, Appearance, Notifications, Agents and runs, MCP servers, GitHub and Bitbucket (`src/pages/SettingsPage.tsx:34-36`, `src/router.ts:16`).

### 2.4 What the code says that the plan doesn't

Round 1's findings, checked again on main; item 7 is new. They change the plan mostly for Multiple devices and the chat.

1. **`changes` holds summaries for the activity feed, not values sync can replay** [V]. A new card logs only `{"identifier", "title"}` (`crates/gizai-core/src/tasks.rs:321`). A move logs the column *names* (`tasks.rs:519`). Labels log their names (`tasks.rs:544`). A run's insert logs a summary (`runs.rs:119`). A field edit logs the patch's new values without the old ones (`tasks.rs:458`), although the schema's comment says "old and new per field" (`0001_init.sql:397`).
2. **Some writes log nothing** [V]: settings (`settings.rs:23-26`), a card's labels (deleted and inserted again on every change, `tasks.rs:535-537`), a column's agents (`columns.rs:128-133`), the project's next task number (`tasks.rs:299-301`), the claim (`runs.rs:113`) and runs' status (`runs.rs:159, 212, 241`).
3. **The clock isn't a hybrid logical clock yet** [V]. `hlc` is the wall clock's milliseconds, a counter that starts at 0 at each start and wraps at 10,000, and the first 8 characters of the device id (`db.rs:365-367`). It never moves past a clock received from another device, and its device part is the id's creation time (UUIDv7), which two devices can share (`docs/backup-format.md:206`).
4. **Task numbers collide offline** [V]. A card's number comes from `projects.next_task_number` (`tasks.rs:299-302`) and identifiers are unique per organisation (`0001_init.sql:265`): two computers offline both make `EUQR-13`.
5. **Some columns of shared rows belong to one computer** [V]: `repos.local_path`, `agent_configs.folders_json` and `default_cwd`, `projects.folder_path`, `docs.mirror_path`, `chat_threads.session_id` and `session_cli`, the claim columns, and `runs.cwd`, `worktree_path` and `pid` (`docs/backup-format.md:205`).
6. **Snapshots are plain and compressed whole** [V] (`docs/backup-format.md:5`, `:74`): right for a folder; for the cloud they need encryption, and a whole-file zstd stream changes nearly everywhere when one page changes.
7. **The Chief works on one computer by design** [V]. Its chat resumes a CLI session that lives in that computer's CLI account (`0003_chat.sql`), its approvals run the waiting call "as the Chief" in that computer's app (`src-tauri/src/pending.rs:114`), and its board check reads that computer's code copies (`docs/features/chief.md`, *It reads your code*). So a message or an approval from another computer must travel to the Chief's computer and run there: it can't be applied as a synced row (3.8).

### 2.5 What remote use touches (Round 3, main at `723097f`, Hersir 0.19.0)

- **One file talks to Tauri** [V]. `src/api.ts:1` says "The only file that talks to Tauri. Swap this file to change the shell." Every command goes through one `invoke` (`src/api.ts:11`), which is Tauri's own `invoke` through the perf counter (`src/lib/perf.ts:5, 28`) and the shared-reads layer (`src/lib/share.ts`); every event through `listen` (`src/lib/perf.ts:6, 43`). The only `Channel` stream is the terminal's output (`terminalAttach`, `src/api.ts:362`).
- **224 commands** [V]: `#[tauri::command]` in `src-tauri/src`: 176 in `commands.rs`, 21 in `editor.rs`, 8 in `terminal.rs`, 4 in `logs.rs`, 3 each in `lib.rs` and `changes.rs`, 2 each in `unsaved.rs`, `tour.rs` and `screenshots.rs`, 1 each in `paste_guard.rs`, `pasted.rs` and `glass.rs`; registered in one `generate_handler!` (`src-tauri/src/lib.rs:818`). Every one takes only `State<'_, AppState>` and/or `AppHandle` besides its arguments (210 and 68 uses; no `Window` or `Webview` parameter), so Rust code that holds the `AppHandle` can call any of them with `app.state()` [V the signatures; I the use].
- **Eleven events** [V], each emitted in one place: `rows-changed` (two places), `runs-changed`, `run-events`, `chat-changed`, `chat-events`, `backups-changed`, `restore-changed`, `update-changed`, `editor-updated`, `terminals-changed`, `quit-requested`. `run-events` and `chat-events` come in batches, at most one every 50 ms per run or chat (`src/api.ts:246-251, 485-488`).
- **The browser mode** [V]: `npm run browser` (GA-185) serves `browser.html`, whose `src/browser/main.ts:19-20` calls Tauri's `mockIPC(…, { shouldMockEvents: true })` before React starts, and `src/browser/mock.ts` answers each command from fixtures and emits `rows-changed` itself (`mock.ts:4, 75`). So every screen already runs in a plain browser with another backend behind `invoke`. The app's build has only `index.html` (`vite.config.ts`).
- **Imports from `@tauri-apps` outside `src/api.ts`** (main code, tests left out) [V]: `@tauri-apps/plugin-opener`'s `openUrl` in nine files (`BitbucketSettings.tsx:5`, `GithubSettings.tsx:5`, `MarkdownView.tsx:4`, `PullPanel.tsx:7`, `UpdateSettings.tsx:5`, `memory/NoteView.tsx:8`, `lib/terminals.ts:18`, `OnboardingPage.tsx:10`, `ProjectPage.tsx:1`); `@tauri-apps/plugin-dialog`'s `open` or `save` in five (`BackupSettings.tsx:11`, `EditorFoldersSetting.tsx:6`, `FileDrop.tsx:4`, `ProjectForm.tsx:2`, `TeamPage.tsx:2`); the webview's drag-and-drop event in `lib/useDropZone.ts:4` (HTML5 drop "never fires in the webview", `:1-2`) and its type in `lib/drop.ts:1`; window effects in `lib/glass.ts:13`; `emit` in `selftest.ts:4`; and `perf.ts` itself.
- **Files are host paths** [V]: `addFiles(ownerType, ownerId, paths)` (`src/api.ts:140`), `sendChat(…, files: string[])` with "paths of files added to the message" (`:450-453`), `checkFiles(paths)` (`:455`), `pasteClipboardImage` saving under `<data dir>/pasted/` (`:456-458`); `openFile(id)` opens a stored file on the computer (`:143`); `screenshotBytes` answers raw bytes (`:270`, `tauri::ipc::Response` in `screenshots.rs:235`).
- **Settings and forms that run things on the computer** [V]: `Settings.claudeBin`, `ghBin`, `editorRoots` and `dataDir` (`src/types.ts:386-405`); a project's `repoPath`, `worktreeCopy`, `worktreeInstall`, `worktreeSetup` and `previewCommand`, the last "set only by a person, in the app" (`types.ts:44-60`); an agent's `permissionMode`, `allowedTools`, `folders`, MCP tools and CLI tools (`types.ts:142-147, 174-192`); MCP servers and coding CLIs (`src/api.ts:192-218`).
- **Hersir keeps running with its window closed** [V]: closing the main window hides it (`src-tauri/src/lib.rs:896-905`), and quitting is the tray's or Settings' *Quit*. So a PC can answer remote devices with its window closed, as long as it is on and awake.
- **Desktop notifications** [V]: four kinds with their switches (hold, waiting, the Chief asks, the Chief answered), worked out from the Inbox every few seconds and shown through one sink, `AppState::desktop` (`src-tauri/src/notifications.rs:1-17, 41-52`; `lib.rs:97`). Web Push can hang off the same sink.
- **Release signatures** [V]: every release has `hersir-X.Y.Z.sig` over `hersir vX.Y.Z <commit>` and the release's `SHA256SUMS`, made on Jeffrey's computer and checked by Hersir against built-in keys (`crates/gizai-agents/src/release_sig.rs:1-12`). The PWA's files can be published and signed the same way.

## 3. The feature

### 3.1 What it should do

- **Cloud Backup**: "Encrypted backups of your data, kept off your computer. Restore them on a new one." Every backup kept 7 days, then one a day up to 90 (decision 8); a 10 GB quota; restore on a clean computer; a lapsed plan stops uploads, not restores (`docs/PAID-FEATURES.md:104-124`, P7).
- **Multiple devices**: "Your clients, projects and cards in sync on each of your computers." Every computer keeps a full local copy that works offline; agents run on their home computer, which claims a card in the cloud before its agent starts it; no agent starts by itself offline (decision 6).
- **The Chief's chat on every computer** (decision 12): write to the Chief from any computer; it answers on its own computer; the answer and its Approve and Discard show everywhere; a message to an offline Chief's computer waits and says so. Later a PWA on a phone is one more device. Building the phone chat is out of scope; designing for it is in.
- Both paid features are in the Personal plan (€5 a month or €48 a year, excl. VAT). The app stays free and complete without an account; Hersir sells hosted services only.
- Out of scope (Round 2): building the phone chat, agents across devices or on servers, teams and SSO.
- **Round 3** (0.1): **Hersir on your other devices**: the MacBook's Hersir and the PWA show the projects of all your hosts and do everything on their boards (cards, comments, files, runs with Run, Stop and Continue, the Chief's chat with Approve and Discard, the Inbox), sent to each project's host and answered there; a host that is off says so. The phone chat is now in scope as part of the PWA. Still out of scope: projects that run in the cloud (C5), teams and SSO.

### 3.2 Encryption and keys (decisions 1 and 2)

End-to-end for both features: the server stores and orders ciphertext; your computers encrypt, decrypt and merge. Round 1's reasons stand [I]: your customers' clients stay unreadable to YoTech and to anyone who breaks into Hersir Cloud; the server never needs Hersir's schema, so an app release with a migration needs no server release; and a breach of ciphertext whose keys only the customer holds is, in the main, a matter for the breach record, not for telling the people in it (GDPR Art. 34(3)(a); table below).

#### The keys

| Key | What it is | Where it lives |
|---|---|---|
| Account key (AK) | 256 random bits, made by the first computer | each signed-in computer's keychain (`cloud/<account>/key`, service `hersir`); never on the server as it is |
| Derived keys | from AK with HKDF-SHA256: the **data key** (backups and the sync log), the **id key** (object names), the **command key** (the chat's commands and live answers), the **key id** (16 bytes, on the server, to tell a wrong recovery code from a right one) | computed when needed |
| Recovery code | 128 random bits, shown as 26 characters in groups (`HC-7K2Q-…`) with a check character | on paper or in your password manager. The server keeps AK sealed with a key derived from it. **New in Round 2: no computer stores it** |
| Computer keys (new) | per computer: an Ed25519 signing key and an X25519 key | that computer's keychain (`cloud/<account>/device`); the public halves on the server and in the roster |

- **No passphrase** (decision 2). A key sealed by a passphrase sits on the server, where a weak passphrase can be guessed offline after a breach; a random 128-bit code can't, and needs no Argon2. This changes the plan's "Argon2id from a passphrase" (`PAID-FEATURES.md:83`).
- **A new computer gets the key** by signing in and typing the recovery code (v1). Later: *Approve on your other computer* (the other computer seals AK to the new one's X25519 key after you compare a short code), which also serves the PWA (3.8).
- **The recovery code is never stored on a computer** [I, new]. Round 1 let a signed-in computer *show* the code, which means keeping it in the keychain, where a stolen laptop would give it away. Instead, a signed-in computer offers **Make a new recovery code**: AK is sealed again under the new code, and the old code stops working. Nothing is encrypted again.
- **The warning** (decision 2), at setup and before every *Make a new recovery code*, in plain words: "Hersir encrypts your data on this computer before it leaves. Only your computers and this recovery code can open it. If you lose this code and every computer signed in to Hersir Cloud, your cloud backups can't be opened by anyone, Hersir included." Setup asks you to type the code's last group back before the first upload, as Apple asks for a recovery key before Advanced Data Protection (S-E2).
- **What is lost when the key is lost**: only when every computer and the recovery code are gone. Then the cloud copies (backups and the sync log) can't be read by anyone; the data on your computers is untouched. The account itself (sign-in, plan) is not lost: it is protected by password and TOTP, not by the key (3.4).
- **The roster** [I, new]: each computer keeps a list of the account's computers and their public keys, built from join records sealed with AK and signed by the joining computer. A computer that joined with the recovery code also signs its join with a key derived from the code, whose public half the roster holds (it changes with *Make a new recovery code*), so every computer can check it, and a thief who has only AK (from a stolen laptop) can't add a computer; a removal, signed by any computer on the roster, is final. The Chief's computer uses it to know who wrote a command (3.8).
- **Ciphers** [V versions, S-E15 to S-E18]: XChaCha20-Poly1305 (`chacha20poly1305` 0.11, Apache-2.0 or MIT), HKDF and HMAC with SHA-256 (`hkdf`, `hmac` and `sha2`, already in Hersir's `Cargo.lock`), Ed25519 (`ed25519-dalek`, BSD-3, already in the lock through `ssh-key`) and X25519 (`x25519-dalek`, BSD-3). Each object is at most a 4 MiB chunk sealed whole, with a random 24-byte nonce and its id as associated data. Not `age` (beta, scrypt passphrases).

#### Removing a computer, and whether the key must change (decision 12)

- **Remove** (on the account page or in Settings on another computer) withdraws that computer's token at once: its next call gets 401 and it says "This computer was removed from Hersir Cloud on the account page." [I]. It also puts a removal on the roster, so from then on the Chief's computer refuses every command signed by it, even if the server forwarded one.
- **The account key doesn't have to change for writing**: the removed computer has no token, and its signature is refused; it can't add itself back without the recovery code.
- **It must change for reading** when the computer is lost, stolen or out of your hands without being wiped: it still holds AK, so with a copy of Hersir Cloud's storage (a breach, a court order) it could read what you back up and sync from now on. *Change the account key* makes a new AK (a new key epoch), seals it to each remaining computer's X25519 key and under a new recovery code, and the next backup uploads in full under the new key; older backups stay readable under the old key until they age out. Recommended for a later card (open question 18): a stolen laptop has your local data anyway, and reading Hersir Cloud's copies would need a breach as well.

#### GDPR exposure [Round 1, still holds]

| After a breach of the server | End-to-end (decided) | Readable by the server (weighed) |
|---|---|---|
| What leaks | ciphertext, sizes, times, computer names, account and billing data | your customers' clients: names, contacts, IBANs, notes |
| Your customer (the controller) | records it; telling the people in it is not needed when the data is unintelligible (Art. 34(3)(a)) | assesses within 72 hours, very likely tells their authority and perhaps their clients (Art. 33, 34) |
| YoTech (processor) | tells customers without undue delay (Art. 33(2)) | the same, plus damages claims (Art. 82) |
| A court order | ciphertext only | plaintext |

Whether ciphertext is personal data *for YoTech* is arguable since C-413/23 P (4 Sep 2025, S-E19), but YoTech holds account data, IP addresses and metadata anyway: **treat YoTech as a processor and offer a DPA** [I].

### 3.3 The server: a Laravel app at `cloud.hersir.dev` (decisions 3, 4, 10 and 11)

**What it is** [I, versions V S-L*]: one Laravel 13 app, `hersir-cloud`, in a new private repository (`Yotech-AI/hersir-cloud`), on one Forge-managed Hetzner Cloud server in Germany (Falkenstein or Nuremberg). It serves the account pages and the API at `cloud.hersir.dev`, and Reverb's websockets beside them. Its own data is one SQLite file in WAL mode, streamed by Litestream to an R2 bucket; the customers' encrypted chunks go straight from their computers to a second R2 bucket, both in R2's EU jurisdiction. The server holds no customer content in plaintext: accounts, computers, plans, the object and snapshot index, the encrypted sync log, claims and the commands waiting for a Chief's computer.

**Replaced**: Round 1's Rust (axum) binary behind Caddy, Hetzner Object Storage and `api.hersir.dev`. They stay below as weighed alternatives.

#### The packages

| Package | Version on 17 Nov 2026 | Licence | What it does here |
|---|---|---|---|
| `laravel/framework` | 13.43 (13.x since 17 Mar 2026; PHP 8.3 to 8.5) [V S-L1] | MIT | the app |
| Laravel's Livewire starter kit | for Laravel 13 [V S-L3] | MIT | the account pages; "All starter kits use Laravel Fortify", with 2FA on by default |
| `laravel/fortify` | current [V S-L2] | MIT | sign-up, sign-in, password reset, email verification, TOTP with recovery codes, password confirmation, rate limits |
| `laravel/passport` | 13.12 on `league/oauth2-server` ^9.2 [V S-L4] | MIT | the desktop's browser sign-in, and each computer's token |
| `laravel/cashier` (Stripe) | 16.11, Stripe API `2026-08-26.dahlia` [V S-L6] | MIT | Checkout for subscriptions, Stripe Tax, the billing portal, webhooks |
| `laravel/reverb` | current [V S-L7] | MIT | websockets on the Pusher protocol: the doorbell for sync, commands and live answers |
| `league/flysystem-aws-s3-v3` | current [I] | MIT | R2 through Laravel's `s3` disk; `temporaryUploadUrl` is "only supported by the s3 and local drivers" [V S-L9] |
| Pest | current [I] | MIT | tests |
| Litestream (on the server, not in the repository) | 0.5.18, 3 Nov 2026 [V S-L10] | Apache-2.0 | streams the SQLite file to R2 |

Why Livewire for the account pages [I]: a handful of pages (sign-up, 2FA, the account page, billing), all server-rendered, with no build of their own to keep in step with Hersir's React app; they take Hersir's colours and type from `design/tokens.json`.

#### The data (SQLite, Laravel migrations)

```
users                 Fortify: name, email, password, email_verified_at, two_factor_secret,
                      two_factor_recovery_codes, two_factor_confirmed_at; Cashier: stripe_id, pm_type, pm_last_four
subscriptions, subscription_items                       Cashier's own
oauth_clients, oauth_access_tokens, oauth_refresh_tokens, oauth_auth_codes, oauth_device_codes   Passport's own
devices(id PK = the app's devices.id, user_id, name, platform, app_version, token_id → oauth_access_tokens.id,
        sign_pub, box_pub, created_at, last_seen_at, revoked_at, lost_at)
account_keys(user_id PK, key_id, epoch, sealed_by_code BLOB, roster BLOB, updated_at)   -- AK sealed by the recovery code; the sealed roster
comps(user_id, plan, valid_until, note)                 -- plans given by hand (your own use before Stripe is live)
objects(user_id, id, bytes, created_at, registered_at, PRIMARY KEY(user_id, id))
snapshots(id, user_id, name, made_at, device_id, reason, schema_version, app_version, bytes, manifest_id,
          deleted_at, UNIQUE(user_id, name))
snapshot_objects(snapshot_id, object_id)
stripe_events(id PK, type, received_at)                 -- each webhook handled once
-- Multiple devices (card 7)
workspaces(id PK = the app's orgs.id, user_id, min_schema, chief_device_id, created_at)
txs(workspace_id, seq, id UNIQUE, device_id, bytes, body BLOB, created_at, PRIMARY KEY(workspace_id, seq))
file_sets(workspace_id, object_id, PRIMARY KEY(workspace_id, object_id))   -- the blobs synced rows use (ids only)
claims(workspace_id, task_id, device_id, run_id, expires_at, PRIMARY KEY(workspace_id, task_id))
-- The Chief's chat (card 7, used by card 10)
commands(id PK, workspace_id, to_device_id, from_device_id, body BLOB, created_at, expires_at, delivered_at)
```

R2 keys: `o/<user id>/<object id>` in the bucket `hersir-cloud-objects`; each object is `HC1` (3 bytes) ‖ key epoch (1 byte) ‖ a 24-byte nonce ‖ ciphertext and tag.

#### Presigned uploads to R2 [V S-L11 to S-L13; I design]

1. The app asks which chunks are missing (`POST /api/v1/objects/missing`), then for upload URLs for those (`POST /api/v1/objects/upload`, with each one's size).
2. The server checks the plan and the quota, records each object as pending, and answers with a presigned `PUT` per object from `Storage::disk('r2')->temporaryUploadUrl(...)`, valid 10 minutes (R2 allows 1 second to 7 days).
3. The app uploads straight to `https://<account id>.eu.r2.cloudflarestorage.com` (presigned URLs don't work on R2 custom domains), so the bytes never pass through the Laravel server.
4. **R2 doesn't document signing Content-Length**, so a presigned URL can't cap the size. The server checks each object's real size with a `HEAD` when the snapshot is registered (`POST /api/v1/snapshots`), refuses objects over 4 MiB plus overhead, and a sweep deletes pending objects not registered within 24 hours. The quota counts the real sizes.
5. The app never gets a delete URL: only the server deletes, by retention, the quota sweep or account deletion. Two R2 API tokens: one for `hersir-cloud-objects` (the app server), one for `hersir-cloud-db` (Litestream).

#### Keeping backups (decision 8)

- A scheduled job, `snapshots:prune`, runs daily: per account it keeps **every snapshot for 7 days**, then **the newest of each day (UTC) up to 90 days**, and marks the rest deleted. The server decides, so a computer that is off can't keep old snapshots alive [I].
- `objects:collect` then deletes objects that no kept snapshot and no workspace's `file_sets` use, after a day's grace, in batches (R2 `DeleteObjects`, free operations) [V S-L11 price; I design].
- A snapshot deleted by hand (Settings or the account page) is kept 7 more days before its objects go, so a mistake, or someone who took over the account, can't erase your history at once [I].
- **The quota** (10 GB = 10,000,000,000 bytes) counts each object once, across every kept snapshot and the synced files. Over it, `POST /api/v1/objects/upload` answers 413 with "Hersir Cloud is full: 10.0 of 10 GB. Remove old backups or files you no longer need."

#### Litestream and the server's own backups [V S-L10, S-L14; I]

- **SQLite settings**: WAL (Litestream needs it), `busy_timeout` 5 seconds, `synchronous = NORMAL`, foreign keys on, and transactions that take the write lock when they start (IMMEDIATE), so two PHP workers writing at once wait instead of failing. Check Laravel 13's SQLite connection options for the last one when the card is built.
- **The file** lives outside the deploy folders: `/home/forge/data/hersir-cloud.sqlite`, so zero-downtime deploys never touch it.
- **Litestream** runs as a systemd service (`/etc/litestream.yml`; installed with a Forge recipe), replicating to `s3://hersir-cloud-db/hersir-cloud` at `https://<account id>.eu.r2.cloudflarestorage.com`, region `auto`, with snapshots kept 7 days (`snapshot.retention: 168h`; the default is 24 hours), so the database can go back to any second of the last week (`litestream restore -timestamp …`).
- **A server restore may lose the last second or so.** Computers notice when the server's sequence for their workspace went back, and push again what they sent after it (they keep their own transactions, 3.7) [I].
- **Nightly**: a copy made with `VACUUM INTO`, compressed, to a third bucket, `hersir-cloud-db-nightly`, with an R2 bucket lock of 30 days, so a leaked key can't delete those copies (R2 has no S3 Object Lock; it has bucket locks by prefix and age) [V S-L13].
- **Hetzner's server backups** (7 daily images, 20% of the server's price) on top [V Round 1 S-S5].
- **One company holds every copy** (Cloudflare) once Hetzner's images are left aside: open question 15 asks for a copy at a second company before the first customer.
- **A restore drill**, monthly and before launch: `litestream restore -o drill.sqlite` on a throwaway server, `PRAGMA integrity_check`, row counts, written down in the repository's `RUNBOOK.md`.
- **Secrets**: `.env` in Forge (APP_KEY, Passport's key pair, the entitlement signing key, Stripe's keys and webhook secret, the R2 keys), and each one also in your password manager. Losing Passport's private key signs every computer out; losing the entitlement key needs a Hersir release with a new public key, so keep both.

#### Deploys with Forge [V S-L15 to S-L17; I]

- **Forge** manages the server: a Hetzner Cloud server is a supported provider; the site `cloud.hersir.dev` deploys from `Yotech-AI/hersir-cloud` with **zero-downtime deployments** (every plan); Let's Encrypt; the scheduler's cron; daemons for the queue worker and Litestream; and Reverb through Forge's *Laravel Reverb* switch, which sets its hostname, port and maximum connections and installs an event loop for more than 1,000 connections (`stream_select` is limited to 1,024 open files).
- **The deploy**: GitHub Actions runs the tests on each pull request; on main, it calls Forge's deploy hook. The deploy script: `composer install --no-dev -o`, `php artisan migrate --force`, `php artisan optimize`, `php artisan reverb:restart`, `php artisan queue:restart`. Migrations only add (a column is dropped one release after nothing reads it), so the previous release keeps working during the switch, and a rollback is Forge's previous release.
- **Forge's plans**: Hobby $12 a month (one external server), Growth $19, Business $39; database backups and, by its docs, server monitoring are on Business only, and its database backups don't list SQLite, so Litestream stays. **Hobby** is enough for one server; Growth when a standby server comes (at about 1,000 customers).
- **Laravel Holdings (US) has root access to the server through Forge**: it goes on the sub-processor list (3.9).

#### Monitoring [V S-L17, S-L18; I]

- **Errors and slow requests**: Laravel Nightwatch, Free (300,000 events a month), which has data centres in the US and the EU; confirm the EU region when the account is made, else Sentry's EU region.
- **Jobs**: a healthchecks.io check (free up to 20) pinged by `snapshots:prune`, `objects:collect`, the nightly copy, and a minute-by-minute job that checks Litestream's last replication is under 5 minutes old; Forge's heartbeats do the same where the plan has them.
- **Uptime**: an external check of Laravel's `/up` every minute, with a public status page (`status.hersir.dev`) [I].
- **Disk, memory, CPU**: Forge's monitoring where the plan has it, else a healthchecks.io check from a cron with `df`.
- **No customer content in logs**: there is none to log; logs hold ids, sizes and times. IP addresses are kept 30 days for abuse and security, as the privacy policy says.

#### Cost a month (EUR, excl. VAT) [V prices S-L11, S-L16, S-L19; I sums]

Assumptions: $1 = €0.90 (1.1127 USD per EUR, 11 Nov 2026); per customer 3 GB stored on R2 and about 3,000 uploads (R2 class A) a month; Litestream's own uploads about 2.6 million a month at 1,000 customers; one CX23 (2 vCPU, 4 GB) up to a few hundred customers; at 1,000 a CX43 (8 vCPU, 16 GB), a standby CX23, Forge Growth and Nightwatch Pro; transactional email from an EU sender.

| | 10 customers | 100 customers | 1,000 customers |
|---|---|---|---|
| Forge | €10.80 (Hobby) | €10.80 | €17.10 (Growth) |
| Hetzner server + its backups (+20%) | €7.20 (CX23) | €7.20 | €19.80 (CX43) + €7.20 (standby CX23) |
| R2 storage ($0.015 a GB-month, 10 GB free) | €0.20 | €3.90 | €40.40 |
| R2 operations (class A $4.50 a million, 1 million free) | €0 | €0 | €18.60 |
| Email (an EU sender) | €0 (free tier) | €5 | €15 |
| Monitoring (Nightwatch, healthchecks.io) | €0 | €0 | €18 (Nightwatch Pro) |
| A second company's copy of the server's database (question 15) | €4 | €4 | €4 |
| **Total** | **about €22** | **about €31** | **about €141** |
| **Round 3: the relay** (3.15: Reverb's own CX23 at 1,000, R2 blobs) | €0 | €0 | about €7 |
| **Total with the relay** | **about €22** | **about €31** | **about €148** |
| Revenue at €5 a month, before Stripe's fees | €50 | €500 | €5,000 |

If every customer filled the 10 GB quota, 1,000 customers would store about 15 TB with history: about €200 a month on R2 instead of €40. **Replaced**: Round 1's €14 / €14 / €71 with Rust on Hetzner storage and no Forge; Forge costs about €11 to €17 a month, and R2 storage costs about twice Hetzner's at scale. Not counted: the IPv4 address, the domain, Stripe's fees (3.5) and your time.

#### Weighed alternatives (replaced by decisions 3 and 4)

| Set-up | For | Against |
|---|---|---|
| Round 1: one Rust (axum) binary on Hetzner, Hetzner Object Storage | one EU company; cheapest storage; the app's own language | a sign-in service to buy (Zitadel), and every account page by hand; you know Laravel better |
| Cloudflare Workers, a Durable Object per workspace, R2 and D1, all in the EU jurisdiction | no server to run; cheapest at 10 to 100 customers | TypeScript; Durable Object ids logged outside the jurisdiction; lock-in |
| Laravel Cloud (managed, EU regions) [I] | no server to patch | its disks don't keep a SQLite file across deploys, so it means Postgres (decision 3 says SQLite) |
| Postgres instead of SQLite | a second app server later | a database server to run, patch and back up; not needed for one server (decision 3) |

### 3.4 Sign-in: Fortify, required 2FA, and Passport with `oauth.rs` (decisions 4 and 5)

**Replaced**: Round 1's Zitadel Cloud with passkeys, Google, GitHub and a password, and its 'no home-made login'. Fortify is Laravel's own, maintained sign-in, not code of ours; decision 4 makes it the sign-in service.

#### On the web (Fortify, in the starter kit) [V S-L2, S-L3; I flow]

1. **Sign up**: name, email, password (Laravel's `Password::defaults()`: at least 12 characters, checked against known leaks with `uncompromised()`) [I rules].
2. **Verify the email** (a link).
3. **Set up 2FA, required** (decision 5): scan the QR code in an authenticator app, type a code to confirm it (Fortify's `confirm` option sets `two_factor_confirmed_at`), save the 8 recovery codes. Nothing else opens until this is done.
4. **Sign in**: email and password, then a code from the app or one recovery code (each works once). Fortify's rate limiters `login` and `two-factor` (5 tries a minute per email and address).
5. **Password reset** by an emailed link; 2FA stays on, so a stolen mailbox alone doesn't open the account.
6. **No** magic links, SMS or email codes (decision 5). Fortify now offers passkeys too [V S-L2]; leave them off.

**Requiring 2FA**: Fortify doesn't force it [V S-L2], so one middleware, `RequireTwoFactor`, sends every signed-in user without `two_factor_confirmed_at` to the 2FA setup page. It covers every web route, **including Passport's `/oauth/authorize`**, so the desktop's sign-in can't skip it; only the setup pages, email verification and sign-out are left out. A test lists the routes and fails on one that isn't covered [I].

**Lost the authenticator and the recovery codes**: you write to support; YoTech resets 2FA after checking the email address and a detail of the last Stripe payment, and emails the account. A reset opens the account, never the data: that still needs a computer or the recovery code [I].

#### The desktop signs in through the browser (Passport, with `oauth.rs` as it is) [V S-L4, S-L5; I design]

1. **One public client** in Passport, *Hersir desktop*, made with `passport:client --public`, redirect `http://127.0.0.1/callback`. Passport hands the client's redirect to `league/oauth2-server`'s `RedirectUriValidator`, which matches 127.0.0.1 and `[::1]` "excluding port", so the random port `oauth.rs` binds (`oauth.rs:299-301`) is accepted (RFC 8252 §7.3). `localhost` doesn't get that treatment, and `oauth.rs` doesn't use it. The client id is built into Hersir: a public client's id isn't a secret (RFC 8252 §8.4).
2. **Discovery**: Passport publishes no RFC 8414 metadata and no RFC 9728 document, and Laravel MCP's `Mcp::oauthRoutes()` would add open client registration and an `mcp:use` scope Hersir Cloud doesn't want. So Hersir Cloud serves two small JSON documents itself:
   - `/.well-known/oauth-protected-resource` (and the same at `/.well-known/oauth-protected-resource/api`) → `{ "resource": "https://cloud.hersir.dev/api", "authorization_servers": ["https://cloud.hersir.dev"], "scopes_supported": ["device:register"], "resource_name": "Hersir Cloud" }`;
   - `/.well-known/oauth-authorization-server` → `{ "issuer": "https://cloud.hersir.dev", "authorization_endpoint": "https://cloud.hersir.dev/oauth/authorize", "token_endpoint": "https://cloud.hersir.dev/oauth/token", "code_challenge_methods_supported": ["S256"], "response_types_supported": ["code"], "grant_types_supported": ["authorization_code", "refresh_token"] }`.
   `oauth.rs::discover("https://cloud.hersir.dev/api", None)` reads the first (with the URL's path, then without: `oauth.rs:177-181`), takes its resource, its first authorization server and its `scopes_supported` as the scope to ask for (`:194-213`), then the second, and checks that S256 is offered (`:228-231`); `begin(&d, Some(HERSIR_CLOUD_CLIENT_ID))` skips registration (`:293-302`). **No change to `oauth.rs` is needed** [V the code paths on main; I that it works end to end, which cards 2 and 4 test].
3. **Two details to test in card 2**: `league/oauth2-server` also accepts the weaker `plain` PKCE method, so a middleware refuses an authorize request without `code_challenge_method=S256`; and `oauth.rs` sends `resource` (RFC 8707), which the league's grant doesn't read: it should be ignored (not verified).
4. **The exchange: a token per computer.** The browser's token has only the scope `device:register`. The app sends it once to `POST /api/v1/devices` with the computer's id, name, platform, Hersir version and public keys. The server records the computer, makes it a Passport personal access token named `device:<id>` with the scope `cloud` (valid a year; the app renews it in its last 30 days with `POST /api/v1/devices/me/token`), revokes the browser's token and its refresh token, and answers with the new token. The app keeps it in the keychain (`cloud/<account>/token`). One package (Passport) does both the browser sign-in and the tokens per computer, so Sanctum isn't needed [I].
5. **Withdrawing a computer**: the account page and Settings → Hersir Cloud on another computer list the computers (name, platform, Hersir version, last seen, added) with **Remove**, which revokes its token (`$token->revoke()`, checked on every request), sets `devices.revoked_at`, and adds the removal to the roster (3.2). That computer's next call gets 401 and it says "This computer was removed from Hersir Cloud on the account page." **Sign out** on a computer does the same for itself, and also removes the account key and its own keys from its keychain.
6. **What `oauth.rs` doesn't get**: revocation (RFC 7009). Passport has no revocation endpoint; `TokenStore` withdraws tokens "where the server offers that", and the exchange revokes the browser's token on the server anyway [V S-L4].

Weighed: Passport's device-code grant (RFC 8628) needs no listener, but `oauth.rs` already has one; Sanctum tokens would need a second package beside Passport [I].

### 3.5 Payments: Stripe with Cashier (decision 9)

**Recommendation** [I]: sell directly through the S.L.'s own Stripe account: Cashier 16 with Stripe Checkout for the subscription, Stripe Billing for renewals, Stripe Tax (Basic, 0.5%) to work out and collect VAT, Stripe's customer portal for cards, cancelling and invoices. **Not Managed Payments, for now**: it costs about 3.5 points more of every sale, puts Link, LLC's name on your customers' statements and receipts, and takes away VAT-number collection for business buyers, while what it saves (registrations outside the EU) matters little to an EU-first launch. **Replaced**: Round 1's Paddle.

#### Can a Spanish S.L. use Stripe Managed Payments today? Yes, at 3.5% on top [V S-P8 to S-P16, read 2026-11-17]

| | What Stripe says |
|---|---|
| Who can use it | a business based in a supported location; the list includes ES and every other EU country, GB, CH, NO, US, CA, AU, HK, JP and SG. "Stripe determines access to Managed Payments based on an eligibility review." The sign-up page says *Start now*; no page calls it a preview, and none says "generally available" either |
| What it sells | "fully automated digital product[s]": software, "electronically supplied business and web services"; each product needs an eligible tax code, such as `txcd_10103000` (SaaS, personal use) |
| What it costs | "3,5 % por transacción exitosa de Managed Payments además de las tarifas de Payments", on "the full transaction amount, including any indirect taxes"; Stripe Tax is included, "billing fees apply separately" (0.7%) |
| Who sells to your customer | "Sold through Link, LLC, the legal entity providing the Merchant of Record service" (LemonSqueezy LLC on invoices before 6 April 2026); statements show `LINK.COM* <your descriptor>`; receipts and tax invoices come from Link; you get a monthly self-billed invoice |
| How it is built | only Stripe Checkout and Payment Links (`managed_payments[enabled]` on the Checkout Session, API `2025-03-31.basil` or later); `automatic_tax`, `tax_id_collection` and some other options must be left out; no subscriptions made outside Checkout; "Existing subscriptions aren't eligible"; the customer portal still works |
| Refunds and payouts | "Stripe can issue refunds within 60 days", and may refund without you after 48 hours without an answer; funds "within 5-7 business days" |
| With Cashier | not in Cashier's docs; Cashier 16 passes Checkout options through, so `->checkout(['managed_payments' => ['enabled' => true]])` without `Cashier::calculateTaxes()` should work, untested [I from the source] |

Not found: how Managed Payments treats an EU business buyer's VAT number (reverse charge) once `tax_id_collection` is left out.

#### What it costs: Stripe direct, Managed Payments and Paddle [V fees S-P9, S-P11, S-P17; I sums]

Assumptions: a standard EEA card in EUR; every percentage taken from the amount with VAT (Stripe says so for Managed Payments, Paddle's example shows it; assumed for the card, Billing and Tax fees); Billing 0.7% in both Stripe columns; Stripe Tax 0.5% only in the first (Managed Payments includes it); Paddle's $0.50 = €0.45. Disputes and refunds left out. Each cell: **fees / what YoTech keeps / fees as a share of the price without VAT.**

| A consumer in | Stripe direct, Billing and Tax | Managed Payments | Paddle (replaced) |
|---|---|---|---|
| Spain, monthly (pays €6.05) | €0.41 / €4.59 / 8.3% | €0.59 / €4.41 / 11.9% | €0.75 / €4.25 / 15.0% |
| Spain, yearly (pays €58.08) | €1.82 / €46.18 / 3.8% | €3.56 / €44.44 / 7.4% | €3.35 / €44.65 / 7.0% |
| Germany, monthly (pays €5.95) | €0.41 / €4.59 / 8.2% | €0.59 / €4.41 / 11.8% | €0.75 / €4.25 / 14.9% |
| Germany, yearly (pays €57.12) | €1.79 / €46.21 / 3.7% | €3.51 / €44.49 / 7.3% | €3.31 / €44.69 / 6.9% |

Stripe direct leaves YoTech about €0.18 more a month and €1.74 more a year per customer than Managed Payments; it doesn't count the cost of filing VAT yourself (below). Push the yearly plan: its fees are half the monthly one's.

#### Selling directly: what the S.L. registers and files, and from when [V S-T1 to S-T10 unless marked; research, not tax advice]

| What | Form | From when |
|---|---|---|
| Spanish VAT, 21%, on Spanish consumers, and on consumers in other EU countries while under the €10,000 threshold | Modelo 303 each quarter, 390 each year | now (the S.L. files them already) |
| **The €10,000 threshold** (LIVA art. 73): the S.L.'s total sales of electronic services (and distance sales of goods) to consumers in other EU countries, without VAT, in the previous or the current calendar year. Below it, Spanish VAT applies; above it, the customer's country's VAT | (Stripe Tax's *small seller* option applies Spanish VAT and tracks the €10,000) | it counts all of the S.L.'s such sales, not only Hersir's [I] |
| **OSS, Union scheme**: the customer's country's VAT on EU consumers | Modelo 035 to register; Modelo 369 each quarter, also with nothing to declare, paid in the following month (by 30 April, 31 July, 31 October, 31 January) | from the sale that crosses €10,000 [Commission guidance, search result only], or earlier by choice (binding for at least two calendar years); the scheme applies from the next quarter, or from the first sale when the tax office is told by the 10th of the following month |
| ROI (the intra-EU VAT number) for business customers in other EU countries, who reverse-charge ("inversión del sujeto pasivo" on the invoice); check their VAT numbers in VIES | Modelo 036 (box 582) | before the first sale to an EU business; no answer in 3 months counts as a refusal |
| Intra-EU services summary | Modelo 349, each quarter while under €50,000 | from the quarter of the first sale to an EU business |
| **VERI*FACTU** invoicing software (RD 1007/2023): every invoice the S.L. issues, also when another party issues them for it | compliant software, or the SII (whose users are exempt) | **before 1 January 2027** for companies that pay corporate tax, so an S.L.; Stripe doesn't claim its invoices comply and points to Invopop. Open question 14 |
| Records and evidence | the customer's location: two pieces that agree, or one from a third party (the card's country) while these sales stay under €100,000 a year; kept 10 years | from the first sale |
| **The UK**: 20% VAT, no threshold for a business not established there | register "within 30 days of the date of making the supply"; returns through Making Tax Digital software | from the first UK consumer |
| Switzerland | registration and a Swiss tax representative, once the S.L.'s **worldwide** turnover is at least CHF 100,000 | from then, on the first Swiss consumer |
| Norway (VOEC) NOK 50,000; Canada C$30,000 (and Québec C$30,000); Australia A$75,000 [search results]; the US by state, mostly $100,000 [search results]; Japan ¥10 million [search result] | local registration | at each threshold |
| South Korea, India, Turkey, Saudi Arabia, the UAE, Serbia and others [not verified] | local registration | from the first consumer |

**What Stripe Tax does** [V S-T8 to S-T10]: works out and collects VAT in Checkout and Billing; collects business customers' VAT numbers and applies the reverse charge; with *small seller* it charges Spanish VAT and tells you when EU sales pass €10,000; its monitoring "highlights potential registration obligations, but it's up to you to confirm" (alerts start once the account had USD 10,000 the year before); exports per country, including for an OSS return; stores the location evidence, although it decides on one address, so the two-piece rule stays yours. **What it doesn't do**: register you, file or pay. Stripe files itself only in the US, for Ireland's non-Union OSS and Canada; in the EU it uses partners (Taxually, Marosa; Tax Complete from €80 a month with 2 registrations and 4 filings a year).

**What stays with the S.L.'s gestor**: filing 303, 390, 349, 035 and 369; the ROI; whether to opt into the OSS at once or start as a small seller; the VERI*FACTU route (Stripe with Invopop, or the SII); the invoices' wording (a simplified invoice is allowed up to €400 with VAT, so for consumers; a full invoice with the reverse-charge line for businesses); the location-evidence policy; the UK registration if the UK is sold to; the Swiss check against the S.L.'s turnover.

**Where to sell at launch** [I]: the EU (consumers and businesses) and businesses anywhere; the UK's consumers once you register there; other countries' consumers, from zero-threshold countries especially, not until the gestor agrees (open question 13). Stripe Checkout asks for the billing country; the account page can refuse a country before Checkout opens.

#### How a plan reaches the app: signed entitlements [Round 1, adapted to Cashier]

```
account page ── Subscribe ──► Stripe Checkout (automatic tax, VAT numbers, prices excl. VAT)
                                   │ webhooks: customer.subscription.created / updated / deleted,
                                   │           invoice.paid, invoice.payment_failed
                                   ▼
Cashier (/stripe/webhook, signature checked; stripe_events: each event once) → subscriptions
   → a listener on Cashier's WebhookHandled event works out the account's plan
   → Reverb: `entitlement.changed` to the account's computers
                                   │
GET /api/v1/me/entitlements  →  { payload, sig }, Ed25519, the public key built into Hersir
                                   │
the app: plan badge, storage bar, "renew" notice: display only
every upload and sync call: the server checks the plan itself
```

- **The plan from Cashier** [I]: `subscribed('default')` → `active`; `pastDue()` (Stripe retrying a failed renewal) → `past_due`, uploads go on; cancelled but paid to the period's end (`onGracePeriod()`) → `active` until `valid_until`; ended → `lapsed`, with `read_only_until` 60 days later (P7). A row in `comps` (a plan given by hand with `php artisan hersir:comp`) counts as `active`: that is how you use Cloud Backup before Stripe is live.
- **The entitlement**: `{ v: 1, account, plan, status, features, quota_bytes, used_bytes, valid_until, read_only_until, issued_at }` and an Ed25519 signature (`ed25519-dalek`, already in Hersir's lock). The app never locks or deletes local data over it (`PAID-FEATURES.md:81`).
- **Lapse** (P7): uploads stop when the plan ends; the list and restore work, read-only, for 60 days; emails when it lapses, after 30 days and a week before the end; then the objects are deleted.

### 3.6 Cloud Backup in detail [Round 1, still holds; storage now R2]

**Recommendation** [I]: keep format 1's manifest and restore, and change only how the bytes travel and sit in the cloud: cut them into chunks, compress and encrypt each chunk on the computer, name each by a keyed hash, and upload only what the server lacks. A cloud restore downloads into a local format-1 folder and hands over to GA-218's restore.

#### What goes up, and how

```
the database: copied with SQLite's online backup API (page for page, page numbers kept),
              then cut into fixed 256 KiB chunks (64 pages of 4 KiB)
the blobs the copy names: a blob up to 4 MiB is one chunk; a bigger one is cut by content (FastCDC)
                    │
each chunk:  id = HMAC-SHA256(id key, its bytes)        never the plain SHA-256
             body = XChaCha20-Poly1305(data key, nonce, zstd(chunk)), with the id as associated data
                    │
POST /api/v1/objects/missing  →  POST /api/v1/objects/upload  →  PUT each to its presigned R2 URL
                    │
the manifest: format 1's JSON, plus the ordered chunk ids of the database and of each blob,
encrypted the same way, then registered (POST /api/v1/snapshots) with every chunk id it uses
```

- **A page-for-page copy, not `VACUUM INTO`** [V S-B1; I]: `VACUUM INTO` (format 1's copy) renumbers pages, so chunks of it would barely repeat from one day to the next; the backup API keeps each page where it is, so a day's upload is the chunks holding changed pages, plus new blobs. Each chunk is compressed on its own, as restic does (S-B2). The card measures chunk sizes on a month of real use before it settles. A page-for-page copy keeps free pages, which may hold deleted text; they are encrypted and age out with the snapshot.
- **R2 states no minimum billable object size** [V S-L11], so chunks needn't be packed (Round 1 worried about Hetzner's 64 KB minimum).
- **A keyed hash as the name**: a plain SHA-256 would tell the server which known files you have. The same file still has the same name across your snapshots and computers, which is all dedup needs.
- **Each chunk its own object**: an upload that stops resumes at the next missing chunk; no S3 multipart; the server counts and deletes objects without reading them.
- **What the server sees**: object ids, sizes, times, the snapshot list (name, time, computer, reason, schema version, total size) and which objects a snapshot uses. Not one byte of content.

#### When it goes up

| When | What | Why |
|---|---|---|
| Daily, when something changed since the last cloud snapshot | reason `daily` | nothing changed means nothing to do |
| On quit, when On quit is on | reason `quit`, as format 1 | the end of a working day |
| Back up now | reason `manual` | before something risky |
| Before a restore | reason `before-restore`, as format 1 | what a restore already does |

Not on every change: with Multiple devices every change goes to the cloud anyway. The cloud backup runs beside the local one, never instead of it.

#### History and restore

- **Kept** (decision 8, by the server, 3.3): every snapshot 7 days, then the newest of each day up to 90 days.
- **Restore on this computer or a new one**: the list comes from the server (newest first, with the computer that made it); Restore downloads the manifest and its chunks into `<data folder>/restore/cloud/`, checks every hash, and from there it is GA-218's restore as it is (`docs/backup-format.md:127-196`): the checks, today's data saved first, the swap at the next start, the device rule, this computer's settings.
- **A new computer**: install Hersir; *Restore from Hersir Cloud* on the first screen or in Settings: sign in in the browser (password and the authenticator's code), type the recovery code, pick a backup, restart.

#### The quota and a lapsed plan

- **10 GB counts what is stored**: each object once, across kept snapshots and synced files (3.3).
- **A lapsed plan**: uploads stop; restore works for 60 days; then deletion after the emails (3.5). Local data is never touched.

### 3.7 Multiple devices in detail [Round 1, still holds, with decision 6 and Reverb]

**Replaced in Round 3** (Jefsev's choice C4): a project lives on its host and is never copied (3.11), so nothing of a board is merged between computers. What this section designed is not built:

| Round 2's piece | Round 3 |
|---|---|
| `sync_log` with its triggers, `sync_clock`, `cloud_link` (migration `0023_sync.sql`) | **replaced**: nothing is copied |
| the hybrid logical clock (changes to `db.rs:365-367`) | **replaced**: one writer per project, its host. `changes.hlc` stays what it is, a sort key for the feed (2.4, 3) |
| last writer wins per field, deletes as fields, document conflicts, deferred foreign keys | **replaced**: concurrent writes from the host's own window and from remote devices meet in the host's one writer connection (`Db::write`, `db.rs:314`), as two of its own screens do today |
| card numbers settled in the server's order | **replaced**: numbers come from the host's `projects.next_task_number` as today (`tasks.rs:372` on `723097f`); they can't collide, since one host numbers its projects |
| home computers (`agent_configs.home_device_id`) and cloud card claims | **replaced**: an agent lives on its host and its dispatcher starts its runs; the local claim (`runs.rs:86-119`) is enough. Decision 6's "claims need the cloud" falls away: **a host's agents keep working while it is offline**, as today |
| files by keyed hash for sync, the server's `file_sets` | **replaced**; files from another device are uploaded to the host for one command (3.12) |
| what never syncs, the schema gate between computers, two-device sync tests | **replaced**; a remote device and its host can run different Hersir versions: the session says which, and a command the host doesn't know is refused with "Update Hersir on PC" (3.13) |
| the server's `workspaces`, `txs`, `file_sets`, `claims`, `commands` tables and their calls | **replaced**: the relay (3.15) has `pairings`, `push_subscriptions` and `relay_blobs` instead |
| **Still needed**: Reverb, its presence channels and a Pusher-protocol client on `tungstenite` in Rust (the doorbell) | now the relay's transport (3.15, card 8) |
| **Still needed**: the computer's own keys (Ed25519 and X25519 in the keychain) and its Passport token (3.2, 3.4) | used for pairing and the remote session (3.13) |
| **Still needed**: `changes` as the activity feed | each host's feed, shown remotely as is (`task_activity`) |

Cloud Backup (3.6) stays as designed: each host backs up its own database and files.

**Kept below as Round 2 wrote it**, for the record and for a later "projects in the cloud" (C5), which could reuse the clock and the log if a cloud copy ever needs to merge with a host's.

**Recommendation** [I]: build sync on a new, complete log beside `changes`, written by triggers, with a real hybrid logical clock, last writer wins per field, and a server that only stores and orders encrypted transactions: Actual Budget's model with Linear's server sequence numbers (S-B4, S-B5). Keep `changes` as the activity feed it is, and carry its rows along, so another computer's feed shows the same lines. No sync engine can be adopted instead: PowerSync, ElectricSQL, Turso Sync and SQLite Sync all need a server that reads the rows, which end-to-end encryption rules out (S-B5).

#### The `changes` log as the code has it now [V]

| Column | What it holds now | Enough for sync? |
|---|---|---|
| `seq` | local order | yes, as the order to push in |
| `id` | UUIDv7 of the change | yes |
| `hlc` | `<ms>-<counter 0000-9999>-<8 chars of the device>`, made at write time (`db.rs:365-367`) | no: not an HLC (2.4, 3) |
| `device_id`, `actor_id`, `run_id` | who and where | yes |
| `table_name`, `row_id`, `op` | the row | only for single-key tables (`task_labels` has a two-column key) |
| `diff_json` | a summary for the feed | no (2.4, 1) |
| `schema_version` | `SCHEMA_VERSION` at write time | yes |
| `pushed_at` | NULL; nothing sets it | the right idea |

So **triggers write a sync log**: on each synced table an `AFTER INSERT/UPDATE/DELETE` trigger, generated from one list in code, appends the changed columns' new values to `sync_log`, in the same transaction as the write. Nothing can bypass it, the existing write code (about 200 statements) stays as it is, and `changes` keeps its job. `Db::write` registers SQL functions on the writer connection: `hersir_hlc()` gives the next clock value and `hersir_capture()` says 0 while remote changes are applied or a migration runs.

#### The new local tables (migration `0023_sync.sql`; main is at schema 22)

```sql
CREATE TABLE sync_log (            -- local and remote transactions' row changes
  seq INTEGER PRIMARY KEY,         -- local order
  tx_id TEXT NOT NULL,             -- one Db::write = one transaction, pushed as one
  hlc TEXT NOT NULL,               -- '<13-digit ms>-<6-digit counter>-<full device id>': sorts as text
  device_id TEXT NOT NULL,
  table_name TEXT NOT NULL,
  row_key TEXT NOT NULL,           -- the id, or a JSON array for a two-column key
  op TEXT NOT NULL CHECK (op IN ('upsert','delete')),
  cols_json TEXT,                  -- new values of the changed columns
  schema_version INTEGER NOT NULL,
  server_seq INTEGER               -- NULL until the server has it
) STRICT;
CREATE TABLE sync_clock (          -- the last write that won, per field
  table_name TEXT NOT NULL, row_key TEXT NOT NULL, col TEXT NOT NULL, hlc TEXT NOT NULL,
  PRIMARY KEY (table_name, row_key, col)
) STRICT;
CREATE TABLE cloud_link (          -- one row: this computer's link to Hersir Cloud; never synced, never restored over
  account_id TEXT NOT NULL, workspace_id TEXT NOT NULL,      -- workspace = orgs.id
  pulled_to INTEGER NOT NULL DEFAULT 0,                       -- the last server_seq applied here
  hlc_state TEXT                                              -- the clock's last value, so it never goes back
) STRICT;
```

#### The clock

A hybrid logical clock (Kulkarni and others, 2014, S-B3): a local write takes `l = max(l, now)`, with the counter + 1 when `l` didn't move and 0 when it did; a received change takes `l = max(l, l_remote, now)`. Four changes to `db.rs:365-367`: the receive rule; the last value kept (`cloud_link.hlc_state`), so a restart or a clock set back never goes behind; a change more than a minute ahead of the server's time refused (as Actual Budget does); and ties broken by the whole device id (`docs/backup-format.md:206`). The server's sequence number decides the order of delivery; the HLC decides which write wins.

#### Applying another computer's changes, and conflicts

Pull returns transactions in the server's order. For each, in one write with `hersir_capture()` off:

1. **Seen already** (its `tx_id` is in `sync_log`): skip. A retry, a duplicate or an echo does nothing.
2. **Schema**: one from a newer schema stops the pull there: "Your laptop uses a newer Hersir. Update this computer to go on syncing." Local changes keep queuing. One from an older schema is applied (migrations that only add are safe; any other needs a translation in sync's code, enforced by a test when `SCHEMA_VERSION` changes).
3. **Per field, last writer wins**, by HLC against `sync_clock`. Two computers editing different fields of one card both keep their edit.
4. **Deletes** are `deleted_at` values like any other field, so an edit doesn't bring a deleted card back, and an explicit restore does. Rows removed for real (a card's labels, a column's agents, `task_builds_on`) sync as row deletes.
5. **Counters take the maximum**: `next_task_number` never goes down.
6. **Colliding task numbers**: a card's UUID is its identity; its number is settled in the server's order. The card the server got first keeps `EUQR-13`; a later one takes the next number free. Every computer applies the same transactions in the same order, so each works out the same answer (S-B9). A card made offline shows its number as "not synced yet" until then; links survive (they hold the UUID). The same rule serves project keys, label names and actor handles.
7. **A document's text**: when both computers changed `docs.body_md` since they last agreed, the newer wins and the other is kept as a version "from Laptop, kept after a conflict".
8. **Foreign keys**: each batch runs with `PRAGMA defer_foreign_keys = ON`.
9. **The server went back** (restored from Litestream, 3.3): a computer whose `pulled_to` is past the server's last sequence pushes again its own transactions after it [I, new].

#### Files

Files sync by keyed hash, as backups store them: a push uploads the blobs the server lacks before the transaction that names them, and updates the workspace's `file_sets` (ids only), so the server's garbage collection keeps them; a pull fetches a blob when it is opened, and the rest in the background.

#### What never syncs

| What | Why |
|---|---|
| `runs`, `run_refusals`, run logs, worktrees, the Chief's code copies | a run lives on the computer that ran it |
| `api_tokens` | this computer's MCP tokens |
| `doc_links`, `search` | derived: rebuilt from what syncs |
| `devices` | the server's list and the roster replace it |
| this computer's settings (`scope::THIS_COMPUTER`) | `scope.rs:6-9` already says so |
| the columns of 2.4, 5 (local paths, CLI sessions, claims) | true on one computer only |
| `sync_log`, `sync_clock`, `cloud_link`, the keychain | the machinery itself, and secrets |

**Changed in Round 2**: `chat_queue` and `pending_actions` now sync (Round 1 kept them local), with one writer, the Chief's computer, so every computer sees what waits and can approve it (3.8). Everything else syncs: organisations, people and agents (without their folders), teams and columns, clients and contacts, projects and repositories (without local paths), cards, labels, comments, docs and memory notes, files, the Chief's threads and messages, and the shared settings. A test lists every table and fails on one that is in neither list.

#### Agents: a card runs on one computer, and claims need the cloud (decision 6)

- **Each agent has a home computer** (a synced column `agent_configs.home_device_id`): the computer it was made on, or, when sync is first switched on, that computer. Its form gets *Runs on*. Only the home computer's dispatcher starts its runs.
- **A claim in the cloud before each start**: `POST /api/v1/workspaces/{ws}/claims` with the card, the run and a 10-minute lease, renewed every 3 minutes while the run lives and released when it ends. The first claim wins; the other gets 409 with who holds it, and its board shows "Running on Desktop". A computer that dies leaves a lease that runs out.
- **Offline** (decision 6): on a synced workspace no agent starts by itself, because the claim can't be made; the band says "Hersir Cloud can't be reached: agents start again when it can." Run by hand still works after a warning ("Another computer could start this card too"), since you chose it [I].
- **The Chief** runs only on the Chief's computer, the board check included (3.8). The board check needs the cloud too, as it starts agents.
- **Agents ask the Chief first** (`questions.rs`) only on the Chief's computer in v1: an agent on another computer that ends with a question asks you, and the card's hold shows on every computer (open question 19).

#### How changes and chat messages reach the other computers quickly: Reverb as a doorbell [V S-L7, S-L8; I design]

- **Reverb rings, HTTPS carries.** Each signed-in Hersir keeps one websocket to Reverb (Pusher protocol, `wss://cloud.hersir.dev`), subscribed to `private-account.<user id>` and `presence-workspace.<ws>` (authorised by `/broadcasting/auth` with the computer's Passport token on the `api` guard). The server sends small events: `sync.tx { seq }`, `command { to }`, `command.status { id, status }`, `claim.changed { task_id }`, `entitlement.changed`, and `chat.live { body }`, the Chief's answer while it is written, sealed with the command key and never stored. The computer then fetches what it needs with normal calls.
- **Presence** says which computers are online right now, so a message to an offline Chief's computer can say so at once (3.8).
- **Without the socket** (blocked, down, reconnecting): poll every 30 seconds while Hersir is open, and catch up once on every reconnect, so nothing depends on the doorbell.
- **Not PHP long polling**: each waiting request holds a PHP worker for its whole wait, so a few hundred open Hersirs would need a few hundred workers; Reverb holds thousands of connections in one process (with an event loop past 1,000).
- **The Rust side**: no maintained Pusher client crate exists (`pusher` 0.4.5 is from 2022 and only covers the HTTP API; `pusher-rs` 0.1.3 is barely used) [V S-L8]. The Pusher protocol needs about 300 lines on `tungstenite` (MIT or Apache-2.0): connect, `pusher:connection_established`, authorise a channel over HTTPS, `pusher:subscribe`, ping and pong, reconnect with back-off [I].
- **Weighed**: polling only (option B in section 4): no websocket client to write, but a message to the Chief would take up to the polling interval to arrive, and 1,000 customers polling every 5 seconds would be a few hundred requests a second on one server.

#### How to test two devices, with fault injection [Round 1, still holds]

1. **In the core, without a network** (`crates/gizai-core/tests/it/sync_test.rs`): two or three in-memory databases and a fake server; a property test (`proptest`) makes random edits on each, then delivers with drops, duplicates, reorders, a partition, a crash after the push but before the answer, one clock an hour behind, and a server that goes back a few transactions. Checks: all replicas equal after delivery, nothing acknowledged lost, identifiers unique, an older schema stops instead of applying.
2. **Against the real server**: the Laravel app run locally (`php artisan serve` and `reverb:start`, with an S3 fake for R2), two Hersirs with their own data folders (`HERSIR_DATA_DIR`) and fake keychains (`HERSIR_FAKE_KEYCHAIN`), driven through their commands; `toxiproxy` between them for latency and cuts.
3. **In real use**: your desktop and laptop, with sync's log visible in Settings until it has run a few weeks without surprises.

### 3.8 The Chief's chat on every computer (decision 12)

**Replaced in Round 3 for v1**: with Remote, the MacBook and the PWA show the PC's own Chat page and call its own chat and approval commands (`sendChat`, `approvePendingAction` and the rest, 3.11), so decision 12 is met without the commands, outbox and roster below, and Round 2's cards 10 and 11 go. An approval from the phone runs on the PC through `src-tauri/src/pending.rs:114` with its `pending::claim`, as a click there does. The design below stays for Sync, where two computers hold copies and the Chief's computer is one of them.

**Recommendation** [I, Round 2]: one computer per workspace is **the Chief's computer**, chosen in Settings. The Chief runs there and only there: its chat answers, its board checks, its approvals. Every other computer writes to it through Hersir Cloud with **commands**: small messages sealed with the account's command key and signed by the computer that wrote them, which the server stores and forwards but can't read, add or change. What the Chief does comes back to every computer through sync, since its threads, messages, queue and pending actions are synced rows with one writer. While it writes, its answer streams live through Reverb.

#### How a message reaches the Chief's computer, and the answer comes back

```
Laptop (you type)                          Hersir Cloud                         Desktop (the Chief's computer)
  chat.send {id, thread, text, files}
  sealed (command key) + signed (Laptop) ─► commands: to=Desktop ── Reverb `command` ──► fetches, checks seal,
  outbox: "Sent to Desktop"                 (stored ≤ 7 days)                         signature, roster, id, seq
                                                                                      → chat::enqueue / send, as if typed
                                                                                        there, by you, "from Laptop"
                              ◄── Reverb `chat.live` (sealed, not stored) ──────────── the answer, as it is written
  shows the answer as it comes
                              ◄── sync: chat_messages, pending_actions, chat_queue ─── the answer and its waiting calls
  the outbox row goes when the synced message with the same id arrives
```

- **The message's id is the command's id**, and Desktop stores your message under it, so Laptop knows when its message has arrived and drops its outbox row.
- **Files** go first, as synced files do (by keyed hash), and the command names them.
- **Desktop treats it as typed there**, through the same code (`src-tauri/src/chat.rs:655` `send_with_files`; the queue when the Chief is answering, `crates/gizai-core/src/chat.rs:747`), with every rule as it is: the outside-tool mark, the limits, Runs on. The message notes the computer it came from ("from Laptop").
- **Other commands**: `chat.stop`, `chat.send_now` (release a held queue), `chat.queue_edit`, `chat.queue_remove`, `chat.set_cli` (Runs on), `chat.new` (a new thread). Pins and titles are plain synced fields any computer may change.
- **Only Desktop writes** the Chief's threads' messages, queue and pending actions; on another computer those writes become commands. A test checks that no other path writes them on a computer that isn't the Chief's.

#### Approvals from any computer

- `pending_actions` sync, so **Waiting for your OK**, with each call's preview, **Approve**, **Discard** and **Approve all**, shows on every computer, and the Inbox's Approval item too.
- **Approve on Laptop** sends `pending.approve { pending_id }` (or `pending.approve_all { pending_ids }`, `pending.discard { pending_id }`; 5.4). Desktop runs it through the same path as its own button (`src-tauri/src/pending.rs:114, 136, 152`), whose `pending::claim` (`crates/gizai-core/src/pending.rs:155`) runs a call once: an approval from a second computer, or a second click, gets the usual refusal. Laptop shows "Approving on Desktop…" until the synced row changes; the note says where it was decided ("Approved on Laptop: …").
- **An approval waits at most 24 hours** for an offline Desktop; after that it expires with a note ("Your OK from Laptop expired: Desktop was off. Approve again.") so a stale approval never runs days later (open question 20) [I].

#### While the Chief's computer is off

- Laptop knows at once from Reverb's presence (or Desktop's `last_seen_at`). Under your message: "Waiting for Desktop: it has been offline since 09:12. The Chief answers when Desktop is back." The text box stays open; more messages queue behind it.
- The command waits on the server for up to **7 days**; then it expires and the message says "Not delivered: Desktop was off for 7 days. Send again" with a button. When Desktop comes back, it fetches its commands in order and answers them as it would a queue.
- The Inbox, the board and the cards work on Laptop as always; only what the Chief does waits.

#### Choosing the Chief's computer

- **Settings → Hersir Cloud → The Chief's computer**: a choice of the account's computers, shared by sync (`chief_device`, a shared setting) and recorded on the server (`workspaces.chief_device_id`). The first computer to turn sync on is the Chief's computer.
- **Switching**: the new computer starts the Chief's chats with a hand-over of the conversation, as switching Runs on does today (`crates/gizai-core/src/chat.rs:655` `handover`), because the old CLI sessions live on the old computer. A command that reaches a computer that is no longer the Chief's is answered with `command.status: moved`, and the sender sends it again to the new one.
- **One Chief at a time**: the board check takes a cloud claim (`chief:board`, the same lease as a card's) before it starts, so two computers never check the board at once during a switch.

#### Who can write, and why the server can't (decision 12)

A command is accepted by the Chief's computer only when all of these hold [I]:

1. **It opens with the command key** (XChaCha20-Poly1305, derived from AK): only a computer that holds the account key could have sealed it. The server, which never has AK, can't add a command or change one; a changed byte fails the tag.
2. **Its signature is from a computer on the roster** (Ed25519, 3.2), and that computer wasn't removed. So Desktop knows which computer wrote it, and a removed computer is refused even if the server forwarded its command.
3. **Its id is new and its counter is higher** than the last one from that computer, and it is addressed to this computer: the server can't replay an old approval or deliver one twice. It can only hold commands back or drop them, which shows as "Waiting for Desktop".
4. **It is under the current key epoch** (after a key change, 3.2).

The server sees who sends to whom, when, and the size; never the text.

#### What removing a computer does

Remove (account page or Settings) withdraws its token at once, so it can't reach Hersir Cloud; adds its removal to the roster, so the Chief's computer refuses anything it signed, also commands already waiting; and deletes the commands it sent that wait. **The account key doesn't have to change for the chat**: without a token and with its signature refused, the computer can't write, and without the recovery code it can't join again. Change the key when the computer is lost or stolen, so it can't read what you send from now on (3.2; open question 18).

#### A later PWA on a phone: just another device

**Replaced in Round 3** by 3.12 and 3.13: the PWA is the whole Hersir, as a screen of the PC; it gets its own device key by pairing and never the account key or a chat key.

- **Sign-in**: the same account, in the browser (password and authenticator), as a public client of Passport with an `https` redirect on the PWA's own origin.
- **The key, by pairing** (recommended): Settings → Hersir Cloud → *Add a phone* shows a QR code holding a one-time pairing secret; the phone makes its own keys (X25519 and Ed25519, kept as non-extractable WebCrypto keys in IndexedDB), sends its public keys through the server tagged with the secret, and the computer seals the key to the phone and signs its join to the roster. The server can't step in between without the secret. **Or** typing the recovery code on the phone, as a computer does.
- **Which key**: the decision says a device that holds the account key; recommended instead is a **chat key** derived from AK, so a phone holds what the chat needs and nothing else: it can write commands and read the chat, not the backups or the sync log (open question 17).
- **What the phone reads**: it doesn't apply the sync log. The Chief's computer also posts each finished message and waiting call to a chat feed sealed with the chat key, kept 30 days on the server; the phone reads that feed and the live answers, and writes commands like any computer.
- **Notifications**: Web Push with no content ("The Chief answered", "The Chief waits for your OK"), so nothing readable passes through Apple's or Google's push services.
- **The web's weak point**: the page's code comes from a server, so a hacked server could serve code that takes the key. Serve the PWA as static files from their own origin, built from a public repository, with a strict Content Security Policy; that is the PWA card's work, not this card's.

### 3.9 Legal [Round 1, updated for selling directly and the new sub-processors]

**Recommendation** [I]: do the template work now (rows 1 to 7, with the European Commission's and the AEPD's material), then one fixed-fee job for a Spanish consumer and technology lawyer: the terms of service and of sale in a business and a consumer version, a review of the DPA, and the consumer rules of the other EU countries you sell to. Don't charge before the terms, the checkout's consent texts and the VAT set-up are signed off. **Changed by decision 9**: the S.L. is now the seller to every customer, so the consumer rules (row 8), the invoices (3.5) and the refunds are YoTech's, not a merchant of record's. This is research, not legal advice (Round 1's legal sources, section 9).

YoTech's two roles: **controller** for account, billing, support and website data; **processor** for the data customers back up and sync. Using that data for its own purposes would make YoTech its controller (LOPDGDD art. 33.2).

| # | What | What it needs | Template or lawyer |
|---|---|---|---|
| 1 | Legal notice (LSSI-CE art. 10) | NEXO IV GRUPO, S.L., its address, email, Registro Mercantil entry and NIF, on the website and the account pages; before an order the steps, the language and storable terms (art. 27); a confirmation within 24 hours (art. 28) | template |
| 2 | Privacy policy (GDPR art. 13) | purposes, legal bases, recipients (below), retention (IP addresses 30 days, invoices as tax law says), rights, the AEPD | template, a quick review |
| 3 | Records of processing (art. 30) | as controller and as processor (the AEPD's Gestiona RGPD) | template |
| 4 | DPO | not needed (LOPDGDD art. 34.1, GDPR art. 37); a memo says why | in-house |
| 5 | DPA offered to customers (art. 28) | the Commission's standard clauses (Decision 2021/915), sub-processors by general authorisation with notice and a right to object; deletion or return at the end (LOPDGDD art. 33.3-4) | template, lawyer review |
| 6 | **Sub-processors** (changed) | Hetzner Online GmbH (Germany): the server; **Cloudflare, Inc. (US)**: R2 in the EU jurisdiction, as a processor under its DPA v6.4, with the EU-US Data Privacy Framework and standard clauses, 30 days' notice of its own new sub-processors [V S-L12]; **Laravel Holdings Inc. (US)**: Forge, which has root access to the server, and Nightwatch if used (check its DPA); the EU email sender. Stripe (Stripe Payments Europe, Ireland) for payments, partly a controller of its own. **Two US companies now**, where Round 1's set-up had none: say so, with the transfer basis, in the list and the privacy policy. What they store or reach of customers' data is ciphertext. **Round 3**: Cloudflare also serves the PWA's static files (`app.hersir.dev`, no personal data in them) and carries the relay's sealed blobs in R2; Apple's and Google's push services carry sealed pushes (an endpoint and a time, no readable content); the relay itself (Reverb) runs on the Hetzner server | in-house |
| 7 | Breach procedure | AEPD within 72 hours as controller, every breach recorded; each customer without undue delay as processor; with end-to-end encryption the content stays unreadable | template |
| 8 | **Consumers** (TRLGDCU), now the S.L.'s own | the total price with VAT per period (art. 97.1 e), so the pricing page shows consumers the price with VAT; 14 days to withdraw (art. 102); starting at once needs the consumer's express request and a confirmation on a durable medium (98.7-98.8), and a withdrawal then pays for the days used (108.4); a withdrawal function in the account (Directive 2023/2673, from 19 June 2026; its Spanish transposition still missing in June [I]); refunds by Stripe, from the account page; conformity and free export of the consumer's content (119 ter). Selling to consumers in another EU country can bring that country's mandatory rules too (Rome I art. 6), such as Germany's cancellation button [I] | lawyer, with the checkout's texts |
| 9 | Terms of service and of sale | transparent and accepted; for consumers no exclusion of liability for death or injury or of conformity remedies (art. 86); for businesses a cap (12 months' fees) and no indirect loss [I]; renewal, cancellation at the period's end, refunds; the lapse (read-only for 60 days, deletion after notice); the recovery code: losing it with every computer loses the cloud copies; the Data Act's switching terms | lawyer |
| 10 | Data Act, chapter VI | at most 2 months' notice to switch, at least 30 days to take the data out; a restore gives the cloud copy back decrypted, and local data is always complete: say so in the terms | in-house, in the terms |
| 11 | Digital Services Act | a hosting service: points of contact, terms on content, a notice form, statements of reasons; a small company is spared the transparency reports | in-house |
| 12 | NIS2, accessibility | NIS2 covers medium and large entities; the EAA spares micro-enterprises that provide services; a file note | in-house |
| 13 | Invoicing and VAT | VERI*FACTU by 1 January 2027, the OSS, the ROI (3.5) | the gestor |

### 3.10 The order, and what you do by hand (Round 3)

**What you can use first** [I] (Round 3; card numbers of section 6):

1. **The base**: card 2, the `hersir-cloud` app's first card (accounts with required 2FA, computers and their tokens, deployed on Forge), with a plan given by hand (`php artisan hersir:comp`). Card 1 (the design) and card 8 (the host and the MacBook's forwarding, against a local fake relay) start at the same time.
2. **Hersir on the MacBook** (requirement R2): cards 7 (the relay on the server), 8 and 9 (the screens on the desktop: the hosts' rail, pairing, the host's band and switch). About 9 to 12 agent-days on the critical path, and the first thing you can use. The PC must be on, awake and running Hersir (question 21).
3. **The phone** (requirement R1): cards 10 (the phone layouts' design), 11 (the PWA: its web build, client, pairing, install and Web Push) and 12 (the phone layouts). Card 11 can start once card 8's contract is merged; about 7 to 10 more agent-days on the critical path.
4. **Cloud Backup** (decision 7: still what is sold first): cards 3 (encrypted storage on the server), 4 (the app) and 5 (Settings → Hersir Cloud), at the same time as 2 to 3 above: they share no files with Remote but `src-tauri/src/lib.rs`'s command list and the Hersir Cloud tab, which cards 5 and 9 split by section.
5. **Before the first sale**: card 6 (Stripe and the account page), the legal pack (3.9, with the relay, Web Push and Cloudflare's static hosting added), the VERI*FACTU route and the OSS choice with the gestor (3.5), the website's pricing and legal pages. Then sell Personal with Cloud Backup, Hersir on your other devices and the phone (3.16).
6. **Later, after the paid features** (C5, out of scope): projects that run in the cloud, as one more host (3.11).

**What you must do by hand** (no agent can: accounts, money, keys, signatures, DNS):

| # | Where | What |
|---|---|---|
| 0 | **The first commit of `hersir-cloud`** (new in Round 3) | The repository exists and is empty. On your computer (PHP 8.3 or newer, Composer, Node, the Laravel installer): `laravel new hersir-cloud --livewire --pest --database=sqlite --npm --git --branch=main`, then in it `git remote add origin git@github.com:Yotech-AI/hersir-cloud.git` and `git push -u origin main`. That is the Livewire starter kit (Livewire 4, Flux UI, Fortify with 2FA on by default; single-file components, the installer's default), Pest, and SQLite (the installer's default) [V S-R9, read 2026-11-20; `--branch` and `--git` from the installer's source]. Leave out `--workos`, `--teams` and `--github` (the repository exists). Keep Boost, the installer's default, if you want its guidelines for the agents. Push it as the installer made it, so card 2's pull request shows only Hersir Cloud's own code. Then link the folder to project HC in Hersir. |
| 1 | GitHub | ~~a private repository~~ (done: `Yotech-AI/hersir-cloud`, project HC); give Forge access to it; a repository secret with Forge's deploy hook for the Actions workflow. **New**: in this (the app's) repository, an environment `pwa` with you as its required reviewer, holding the Cloudflare token that deploys the PWA (card 11) |
| 2 | Hetzner | an account for NEXO IV GRUPO, S.L., its DPA (AVV) accepted in the console; a project and an API token for Forge; check that a CX23 can be ordered in Falkenstein or Nuremberg (the price pages showed "currently unavailable", perhaps only when signed out) |
| 3 | Laravel Forge | Hobby ($12 a month) for the S.L.; connect Hetzner; create the server (Germany, PHP 8.4; no database server, the app uses SQLite); the site `cloud.hersir.dev` from the repository with zero-downtime deploys; Let's Encrypt; the *Laravel Reverb* switch; daemons for the queue and Litestream (a recipe installs Litestream and its systemd unit); the scheduler; the `.env`; the firewall (22, 80, 443); accept Laravel's terms and DPA |
| 4 | Cloudflare R2 | turn R2 on in the Cloudflare account that holds `hersir.dev`; three buckets **in the EU jurisdiction** (it can't be changed later): `hersir-cloud-objects`, `hersir-cloud-db`, `hersir-cloud-db-nightly` (a 30-day bucket lock on the last); two API tokens, each limited to its buckets, into Forge's `.env`; Cloudflare's DPA is part of its self-serve terms |
| 5 | Cloudflare DNS | `cloud.hersir.dev`: A and AAAA records to the server, **DNS only** (grey cloud), so TLS ends on the server and Reverb's websockets go straight to it; later `status.hersir.dev`; SPF, DKIM and DMARC for the email sender |
| 6 | An EU email sender | an account, the domain verified, its key into `.env` (verification, password reset, lapse emails) |
| 7 | Stripe (the S.L.'s account) | a sandbox first; the product *Hersir Personal* with two prices (€5 a month, €48 a year, EUR, tax *exclusive*) and the tax code `txcd_10103000`; Stripe Tax: the head office in Spain, the Spanish registration, *small seller* or OSS as the gestor decides; tax IDs collected; billing address required in Checkout; the customer portal (cancel at the period's end, cards, invoices); Billing's retries and emails; the invoice numbering; the webhook to `https://cloud.hersir.dev/stripe/webhook` (Cashier's `cashier:webhook` makes it) and its secret; restricted API keys into `.env`. Not Managed Payments (3.5) |
| 8 | Keys | make the Ed25519 key pair that signs entitlements, offline; the private key into `.env` and your password manager; the public key goes into Hersir's code with card 4; after `passport:keys` on the server, Passport's key pair into the password manager too. **New**: the VAPID key pair for Web Push (`php artisan webpush:vapid`, card 7) into `.env` and your password manager; losing it means every phone asks for notifications again |
| 9 | Monitoring | Nightwatch with its EU region (else Sentry EU); healthchecks.io checks; an uptime check and the status page |
| 10 | A second copy of the server's database at another company (question 15) | for example a Hetzner Storage Box, with an SSH key for the nightly copy |
| 11 | Your gestor | small seller or OSS from the start; the ROI (Modelo 036) and Modelo 349; the VERI*FACTU route for Stripe's invoices (Invopop, or the SII); the invoices' wording; the location-evidence policy; the UK registration if you sell there; the Swiss check |
| 12 | A lawyer | the terms of service and of sale (business and consumer), the checkout's consent and withdrawal texts, the DPA review, other EU countries' consumer rules |
| 13 | The website (`gizai-website`) | the pricing page (consumers see the price with VAT), the legal pages (notice, privacy, terms, DPA, sub-processors), links to `cloud.hersir.dev` and `app.hersir.dev` |
| 14 | **Cloudflare for the PWA** (new) | a Workers project `hersir-web` with static assets only (no Worker code), its custom domain `app.hersir.dev` (proxied, orange cloud: this one is served by Cloudflare), and an API token limited to deploying that project, saved in the `pwa` environment of row 1 (question 22) |
| 15 | **Your PC** (new) | in the OS's power settings, no sleep while plugged in (or Hersir's *Keep this computer awake while remote use is on*, question 21); Hersir set to start at login; remote use switched on in Settings → Hersir Cloud |
| 16 | **Each phone and the MacBook** (new) | pair them once with each host (a QR code on the PC, or the long code on the MacBook); on iPhone, *Add to Home Screen* before allowing notifications (3.14) |
| 17 | **Releases** (new) | each release also gets `hersir-web-X.Y.Z.zip`, listed in `SHA256SUMS` and so covered by the signature you already make with `scripts/sign-release.sh`; you approve the `pwa` environment's deploy after signing |

**No longer needed** (replaced): Zitadel, Paddle and its domain review, Hetzner Object Storage, a Scaleway or Backblaze bucket, `api.`, `account.` and `login.` records. Round 3: nothing for sync (no workspaces to set up, no Chief's computer to choose).

### 3.11 Hosts and their projects (C1 to C5, A1 to A4)

**The model** [I]: every Hersir on a computer is a **host** of the projects made on it, with everything that hangs off them: its database, its folders and worktrees, its agents and their CLIs, its runs, its Chief. A **device** is any screen you use: the host's own window, another computer's Hersir (the MacBook) or the PWA. A device shows the projects of every host it is paired with, and sends each host's commands to that host. Nothing is copied between hosts (C3, A3). The MacBook is both: a host of its own projects, if it has any, and a device of the PC.

#### How a device finds your hosts and their projects

1. **The account's computers**: `GET /api/v1/devices` (card 2) lists them, now with `kind` (`desktop` or `web`) and `remote_on` (a host says so when its switch is on, 3.13); Reverb's presence channel `presence-account.<user>` says which are online right now (3.15).
2. **Pairing makes a host usable** (3.13): a device works only with the hosts it is paired with, and a host answers only devices it has paired. Two desktop Hersirs pair **both ways at once**, so the PC shows the MacBook's projects as the MacBook shows the PC's. A computer on the account that isn't paired yet shows under *Add a computer* in the rail, with **Pair**.
3. **The directory**: when a device connects to a host, it asks the host for its name and its projects (`remote_directory`, 5.8: each project's id, key, name, colour, team and repository link) and keeps that list, and only that list, until the next connection: in the MacBook's data folder (a this-computer table, never backed up as the host's data) or in the PWA's IndexedDB. It is refreshed when the host's `rows-changed` names `projects`. This is what lets a device show an offline host's projects (A2) and warn about a repository that is already a project elsewhere (A1). It holds no cards, comments, chats or runs: no copy of the data (A3).

#### How a screen shows the projects of two hosts: one workspace per host, in a rail

```
 ┌────┐ ┌──────────────────┐ ┌──────────────────────────────────────────────┐
 │ PC │ │ PC               │ │ EUQR · Board                                 │
 │ ●  │ │ Inbox        3   │ │ To do        In progress    Review           │
 │    │ │ Chat             │ │ ┌────────┐  ┌────────┐                       │
 │ Mac│ │ Projects         │ │ │EUQR-12 │  │EUQR-9  │                       │
 │    │ │  EUQR            │ │ └────────┘  └────────┘                       │
 │ ⊘  │ │  GA              │ │                                              │
 │Lap │ │ …                │ │                                              │
 │    │ │                  │ │                                              │
 │ +  │ │ PC · remote      │ │                                              │
 └────┘ └──────────────────┘ └──────────────────────────────────────────────┘
  rail     sidebar island          page island
```

- **The rail** sits on the window's background, left of the sidebar island, as the top bar does above the page (`design/README.md`, *Islands on a background*): 44 px wide (`size-topbar`), one 32 px tile per host with its initials or icon, its name under it in `t-xs`, the selected one on `canvas-hover`, a badge with its Inbox count (on `island`, as the Inbox count on the background already is), a 6 px dot in `live` while it is connected, and `+` (*Add a computer*) at the bottom. **This computer** is the first tile on a desktop that has projects of its own; the PWA has no tile of its own.
- **Choosing a tile switches the whole window** to that host: the sidebar, the board, the chats, the Inbox, the search, the Usage page and Settings are the host's own, as if you sat at it; the sidebar's footer says "PC · remote". Hersir's routes (`src/router.ts`) don't change: each host's pages keep their own `#/…` address, with the host kept beside the route (`hersir.host` in the URL's query, so a link to a card opens on the right host) [I].
- **With one host and no projects of its own** (the PWA, or a MacBook only used for the PC), the rail can be hidden; it shows as soon as a second host is paired [I].
- **Why not one merged board** [I]: each host has its own teams, columns, agents, labels, Chief, clients and people (below), so a merged board would need Round 2's sync to agree on them. One workspace per host shows plainly where each project lives and which computer does its work (C1).

#### A host that is off (A2)

- **In the rail**: the tile grayed out (the faint text colour, no badge) with a small **offline icon** (lucide's `MonitorOff`, the same icon everywhere a host is off) and the tooltip "PC is offline since 09:12". Its projects stay listed in its sidebar, grayed out, each with the same icon, from the directory.
- **A click on the tile, or on one of its projects**, opens the host's workspace on an **offline page** in the page island, never stale data (A3):

```
  ⊘  PC is offline
     Since today 09:12. EUQR, GA and its other projects are on PC, and Hersir keeps no copies of them,
     so they open again when PC is on, awake and running Hersir.

     EUQR · GA · HC                                    (the projects, grayed out; the clicked one marked)

     [Check again]   [ ] Tell me when PC is back
```

- **The other states of that page**: *Connecting to PC…* (with `LoaderCircle`, at most 10 s, then offline); *Remote use is off on PC* ("Turn it on in Hersir on PC: Settings → Hersir Cloud → Use from other devices"); *PC runs an older Hersir* ("Update Hersir on PC to use it from here": the host's version is below the device's minimum for the session, 3.13); *This device was removed from PC* ("Pair it again from PC's Settings → Hersir Cloud").
- **A host that goes off while you work** (the socket closes, or presence says it left): the page you are on stays drawn but grayed, with a band at its top in `needs` gold, "PC went offline at 14:02. What you see may be out of date; changes can't be sent.", and every button that would write is disabled. A write that was on its way fails with "PC went offline before it answered. Check on PC whether it was done." [I]
- **Writes are never queued** for a host that is off (question 20): a Run, an approval or a comment that waited and ran hours later would surprise you.
- **Tell me when PC is back**: a desktop notification, or a Web Push to the phone, once presence shows the host again (card 7 sends `device.online` to the account's channel).

#### The same repository on two devices (A1)

- **Where it shows**: in the project form (*New project*, and *Folder* on an existing project, `ProjectForm.tsx`), right after a folder is picked and `check_repo` has read its remote (`src/api.ts:297-298`), the form compares the remote's link (`github` or `bitbucket`, normalised to `host/owner/repo`, lowercase, without `.git`) with the `repoUrl` of every project in the directories this device holds, this computer's own projects left out. On a match, an inline note under the *Folder* field, in the field's hint style with the offline-or-host icon, not an error: creating stays allowed.
- **Its text**: "**This repository is already the project EUQR on PC.** Adding it here makes a separate project on this Mac, with its own cards, agents and runs; nothing is shared with EUQR on PC. To work on EUQR, open PC in the rail." with the buttons **Open EUQR on PC** and the form's own **Create project**. When that host is off: "…is already the project EUQR on PC (offline since 09:12)…". For several matches, one line per host.
- **The Chief making a project** (its `create_project` tool, `src-tauri/src/tools/mod.rs:202, 452`) gets the same sentence in its tool's answer when the project's folder has a matching remote, so it can tell you [I].
- **The new project is separate on purpose** (C1): its own key (the same key may be used on both hosts, since keys are unique per host's organisation, `0001_init.sql:147`), its own numbering, cards, worktrees and branches. Two hosts pushing to one repository can still meet in git (the same branch name from two `GA-12`s on different hosts); the card branches carry the host's project key, and a later card could add the host's short name to branch names if that happens in practice [I].

#### The Chief, agents, clients, people and memory when two devices hold projects [I]

| What | With two hosts |
|---|---|
| **The Chief** | one per host. Each Chief chats, checks the board and approves on its own host, for its own projects only; the chat you open in a host's workspace is that host's Chief. Its board check never reaches another host. (Round 2's "Chief's computer" setting goes.) |
| **Agents** | belong to their host and run there with its CLIs, folders and keychain; an agent's form, tools and folders are changed on its host only (3.12). The same team on two hosts is two teams: Team → *Export template* on one host and *Import* on the other (GA-226) copies its setup. |
| **Agents' questions** | an agent asks its own host's Chief first, then you, in that host's Inbox; the rail's badge shows each host's Inbox count. Round 2's question 19 is settled by this. |
| **Clients and people** | per host. A client with projects on both hosts exists twice, edited separately; "you" are a person on each host. No link between them in v1. |
| **Memory** | per host, with its agents. |
| **Cards building on cards** | only within a host: a card on PC can't build on one on the MacBook. |
| **Settings, notifications, Usage, Cloud Backup** | per host. Each host notifies its own paired devices (3.14) and backs up its own data (3.6). |

#### Projects in the cloud, later (C5, A4) [I]

Out of scope for a while. Remote keeps the door open: a host is anything that holds projects and answers the protocol of 3.13, identified by its device id, with no assumption of a screen. A Hersir running on a server later would be one more tile in the rail, paired like any computer. Keep the remote module apart from the window code (it already is: the commands take only `State` and `AppHandle`, 2.5) and keep the host-only list in one place (5.8), so a headless host can use both.

### 3.12 What a remote device may do

**The rule** [I]: a remote device may do on a host what you could do there from the board, the cards, the chat and the Inbox; it may not reach the host's own files, programs or accounts, nor widen what the host's agents may run. **Run, Stop and Continue** and **the Chief's chat with Approve and Discard** work from the MacBook and the PWA alike: they are the host's own commands (`startRun`, `stopRun`, `continueRun`, `continueAfterRunForMe`, `src/api.ts:233-238`; `sendChat`, `approvePendingAction`, `approvePendingActions`, `discardPendingAction`, `:452, 478-483`), run on the host as if clicked there, with every rule in force (an approval runs once, through `src-tauri/src/pending.rs:114` and its `pending::claim`).

Of the 224 commands (2.5) [V the list on `723097f`; I the grouping]:

| Group | Commands | Remotely |
|---|---|---|
| Clients, contacts, people | `list_clients`, `get_client`, `save_client`, `archive_client`, `list_contacts`, `save_contact`, `remove_contact`, `list_users`, `add_user` (9) | **as they are** |
| Projects | `list_projects`, `get_project`, `project_team_move` (3) | **as they are** |
| Cards | `list_tasks`, `get_task`, `create_task`, `update_task`, `move_task`, `set_task_labels`, `get_builds_on`, `add_builds_on`, `remove_builds_on`, `list_archived_tasks`, `archive_task`, `restore_task`, `move_tasks`, `archive_tasks`, `restore_tasks`, `task_activity`, `list_comments`, `add_comment`, `item_id`, `suggest_agent`, `card_changes`, `card_file_diff`, `run_changes`, `list_screenshots` (24) | **as they are** |
| Teams, columns, labels | `list_teams`, `get_team`, `add_team`, `rename_state`, `add_state`, `set_column`, `add_column_agent`, `remove_column_agent`, `column_removal`, `remove_state`, `list_labels`, `save_label`, `remove_label`, `add_branch`, `remove_branch` (15) | **as they are** (an agent on a column starts runs within its own rights; it doesn't widen them) |
| Docs and Memory | `list_docs`, `get_doc`, `create_doc`, `save_doc`, `rename_doc`, `doc_versions`, `doc_version_body`, `agent_notes`, `memory_notes`, `memory_create`, `memory_move`, `memory_search`, `memory_recent`, `memory_enabled`, `set_memory_enabled`, `ask_lead_enabled`, `set_ask_lead_enabled` (17) | **as they are** |
| **Runs** | `start_run`, `stop_run`, `continue_run`, `continue_after_run_for_me`, `list_runs`, `run_events`, `run_commits`, `live_runs` (8) | **as they are**; `run-events` and `runs-changed` forwarded |
| Pull requests and git hosts (reads) | `open_pull_request`, `check_pull_request`, `github_status`, `github_check`, `bitbucket_status`, `bitbucket_check` (6) | **as they are** (the push and the pull request are the host's, with its `gh` or token) |
| **The Chief's chat and approvals** | `list_chat_threads`, `get_chat_thread`, `pinned_chats`, `pin_chat`, `unpin_chat`, `rename_chat`, `search_chat_threads`, `chat_messages`, `chat_queue`, `edit_queued_chat`, `remove_queued_chat`, `send_chat_queue`, `set_chat_cli`, `answer_chat_on`, `chat_clis`, `stop_chat`, `chat_live`, `chat_agent`, `dismiss_chat`, `pending_actions`, `approve_pending_action`, `approve_pending_actions`, `discard_pending_action` (23) | **as they are**; `chat-events` and `chat-changed` forwarded |
| Agents (reads, and pausing) | `get_agent`, `agent_stats`, `agent_runs`, `agent_next_task`, `set_agent_status`, `list_clis`, `claude_models`, `role_template`, `role_tools`, `agent_mcp`, `agent_cli_tools`, `list_mcp_servers`, `browser_entry` (13) | **as they are** (pausing narrows) |
| Usage and status | `usage_summary`, `subscription_limits`, `get_settings`, `backup_status`, `back_up_now`, `update_status`, `get_logs_info`, `app_info`, `restore_status` (9) | **as they are** |
| Files: list and remove | `list_files`, `remove_file` (2) | **as they are** |
| **Files in** | `add_files`, `send_chat` (its `files`), `check_files`, `paste_clipboard_image` (4) | **need a change**: the device uploads the bytes first (3.15) and passes `remote:<upload id>/<name>` tokens; the host refuses any other path from a remote device ("Files from another device must be uploaded"). The MacBook's forwarding does this for its own paths, so its file picker, drops and pasted images work unchanged; the PWA's pickers give File objects, which its client uploads |
| **Files out** | `open_file`, `screenshot_bytes` (2) | **need a change**: the host answers with a sealed blob through R2 (3.15) instead of opening the file on its own screen; the device opens or shows it itself |
| **Two forms** | `save_project`, `save_settings` (2) | **need a change**: the host refuses, from a remote device, a change to a project's `repoPath`, `worktreeCopy`, `worktreeInstall`, `worktreeSetup`, `previewCommand` or `leadMayMerge` (they run programs on the host, or are "set only by a person, in the app", `src/types.ts:44-60`), and to the settings' `claudeBin`, `ghBin`, `dataDir`, `editorRoots`, `pushOver`, `previewPortFrom`/`To` and the memory limits; the rest of both forms works. Each refusal names the field: "The project's folder and setup commands can only be changed on PC." |
| Terminal | `terminal_*` (8) | **host only** |
| Editor and by-hand cards | `editor_*`, `open_in_app`, `start_by_hand`, `hand_card`, `hand_commit`, `hand_push`, `hand_over` (21) | **host only** (they work in the host's folders) |
| Folders and worktrees | `check_repo`, `check_agent_folders`, `list_old_worktrees`, `remove_old_worktrees`, `remove_old_worktree_anyway`, `open_logs_folder` (6) | **host only** (`check_repo` runs on the device's own computer when it adds a project of its own) |
| **What widens what an agent may run** | `add_agent`, `update_agent`, `save_agent_mcp`, `save_agent_cli_tools`, `ask_cli_tools`, `save_clis`, `find_clis`, `detect_claude`, `detect_gh`, `save_mcp_server`, `remove_mcp_server`, `list_mcp_tools`, `scan_claude_code_mcp`, `import_mcp_servers`, `mcp_sign_in`, `mcp_sign_out`, `save_browser_entry`, `list_browser_tools`, `check_team_export`, `save_team_template`, `team_templates`, `preview_team_template`, `import_team_template` (23) | **host only** in v1. `update_agent` changes permissions, tools, folders and the CLI as well as the name; a later card could let a device change the fields that don't widen (name, instructions, model, effort, runs at once) |
| Keychain and accounts | `github_login`, `github_login_wait`, `github_login_cancel`, `bitbucket_save_login`, `bitbucket_remove_login` (5) | **host only** |
| The updater | `check_for_updates`, `set_update_auto_check`, `start_update`, `stop_update`, `restart_gizai` (5) | **host only** |
| Backups and restore | `save_backup_settings`, `list_backups`, `start_restore`, `cancel_restore`, `restart_to_restore`, `dismiss_restore_result` (6) | **host only** |
| The app and its window | `exit_app`, `selftest_report`, `selftest_panic`, `selftest_paste`, `tour_still`, `tour_steps`, `set_unsaved_edits`, `close_answer`, `glass_support`, `onboarding_pending`, `finish_onboarding`, `skip_onboarding`, `dismiss_unexpected_end` (13) | **host only**; on the MacBook these run on the MacBook itself, for its own window |

Totals: 129 as they are, 8 with a change, 87 on the host only. A command a host doesn't know (a newer device, an older host) is refused with "Update Hersir on PC to do this from here." **The events**: `rows-changed`, `runs-changed`, `run-events`, `chat-changed`, `chat-events`, `backups-changed`, `restore-changed` and `update-changed` are forwarded; `editor-updated`, `terminals-changed` and `quit-requested` are not.

**On screen** [I]: host-only places stay visible but say where they work: the Editor and Terminal pages show "The Editor works on PC itself" with nothing else; in Settings, the tabs and fields above are read-only with "Change this on PC"; the agent form opens read-only with the same line. The screens that import from `@tauri-apps` directly (2.5) need nothing on the MacBook (its plugins are its own) and a shim in the PWA (3.14): `openUrl` opens a new tab; the dialog's `open` gives picked files (the PWA's `FileDrop` gets an `<input type="file">`); `useDropZone.ts` listens to HTML5 drops in the PWA, where they do fire; `glass.ts` answers "none"; the template and folder dialogs are on host-only screens.

### 3.13 Security and pairing

A remote device can make a host's agents run code (a card and Run are enough). So the design keeps four promises [I]: only devices you paired on the host itself can send it commands; Hersir Cloud can neither read nor forge them; a device is withdrawn at once; and even a fully paired device can't widen what the host's agents may do.

#### Pairing a device (replaces question 17's design)

1. **On the host**: Settings → Hersir Cloud → *Use from other devices* → **Add a device**. The host asks Hersir Cloud for a pairing slot (`POST /api/v1/pairings` → `{ pairing_id, expires_at }`, 5 minutes) and makes a random 128-bit **pairing secret**, which never goes to the server. It shows a QR code holding `https://app.hersir.dev/pair#p=<pairing_id>&h=<host device id>&s=<secret, base64url>`: the part after `#` never leaves the phone in a request. For a MacBook without a camera it shows the same as a code to type: `HP-` and 26 characters in groups (the pairing id's short form and the secret, with a check character).
2. **On the device**: the phone's camera opens the link (the PWA signs in first if needed, password and authenticator, 3.4); the MacBook's *Add a computer* takes the typed code. The device has its own keys: the MacBook its computer keys from Round 2 (Ed25519 and X25519, in its keychain, 3.2); the PWA makes an X25519 and an Ed25519 key pair with WebCrypto, **not extractable**, kept in IndexedDB (both are in WebCrypto in Safari 17, Chrome 133 and 137, Firefox 129 and 130 [V S-R11]). It sends `POST /api/v1/pairings/{id}/join` with its name, kind, public keys and a MAC: HMAC-SHA256 under a key derived from the secret over the pairing id, both device ids and the public keys.
3. **The host checks the MAC** (it arrives through Reverb's `pairing.joined` on the host's own channel), then asks: "**Add iPhone (Safari) to PC?** It will see PC's projects and can run agents, approve the Chief's calls and add cards and comments. It can't use the terminal, the editor or PC's folders, or change what agents may run." with **What it may do**: *Everything above* (default) or *Board and chat only: no Run, Stop, Continue or approvals* (question 23), and **Add** / **Cancel**.
4. **Add** stores the device on the host (`remote_devices`, 5.8) and confirms with its own MAC over the host's public keys (`POST …/confirm`); the device checks it and keeps the host's public keys pinned. Two desktops pair both ways in the same exchange (3.11). The server records only that the two devices are paired (for Reverb's channel rules) and never sees the secret, so it can't put its own keys in between: a changed key fails a MAC.
5. **A device pairs with each host it uses** (question 25): two QR codes for two hosts. A host vouching for a device to your other hosts is a later convenience.

**The PWA's key, replacing question 17**: the PWA holds its own device key pair, made in the browser and registered on each host by pairing. It never holds the account key, a chat key or any data key, so it can't read Cloud Backup's copies; it reads only what a host answers it, while it is connected.

#### The session: Noise, end to end

Each time a device connects to a host, they run the Noise protocol's **KK** handshake (both sides already know the other's static X25519 key from pairing) as `Noise_KK_25519_AESGCM_SHA256`, then send everything in Noise's transport messages (AES-256-GCM with counters, so a replayed, reordered or changed message fails) [I: the Noise Protocol Framework's pattern; the `snow` crate on the host and the MacBook, and about 300 lines of TypeScript on WebCrypto in the PWA, checked against the published Noise test vectors]. Ed25519 signs nothing in the session; it signs the pairing records and the push key (3.14). The session starts with the versions: a host refuses a device whose session version it doesn't know, and says which Hersir it runs.

#### Withdrawing a device, at once

- **On the host** (Settings → Hersir Cloud → *Your devices* → **Remove**) or **on the account page** (*Your computers and devices* → **Remove**): the server revokes the device's Passport token and its pairings at once (its next call gets 401; Reverb refuses its channel); the host, told by `device.removed` on the account's channel or by its own click, deletes the device's keys and ends its sessions mid-stream. Either one alone already stops it.
- **The account's 2FA** is required for every device (decision 5): the PWA signs in with password and authenticator like the account pages; a new device and a removal each send an email.
- **The account key doesn't need to change** for Remote: a removed device had none (the PWA) or only what Round 2's 3.2 already covers (a computer with Cloud Backup).

#### What the server still sees (metadata)

Which devices you have and their names and kinds; which are paired with which host; when each is online; how many messages and bytes pass between a device and a host, and when; the sizes of uploaded files; Web Push endpoints and when a push is sent. Never a command, an answer, a project's name, a card, a chat or a file's content; the directory of 3.11 travels inside the session.

#### The PWA's code: where it is built and served, and how a changed web app is kept from using its keys

A web app's code comes from a server each time: whoever controls the origin that serves the PWA can serve code that uses its keys. Non-extractable keys can't be copied out, but any script on the origin can still use them [V S-R11, MDN on `extractable`; I]. No browser lets a page pin its code: the origin can always replace the service worker, and the browser checks for a new one on navigation and at least every 24 hours [V S-R7]. Subresource Integrity covers files the HTML names, not the HTML itself [V S-R8]. WEBCAT (Freedom of the Press Foundation) is an alpha Firefox extension, and Meta's Code Verify is a desktop-browser extension; Chrome's Isolated Web Apps are signed bundles installed by enterprise tools; none of them helps an iPhone today [V S-R8]. So the design limits and shows the risk instead of claiming to remove it [I]:

1. **Built in this repository**, which is public: `npm run build:web` makes the PWA from the same `src/`, so its code is reviewable like the app's.
2. **Served from its own origin, `app.hersir.dev`**, not from `cloud.hersir.dev`: a hole in the Laravel app (forms, payments, webhooks) can't serve the PWA's code or reach its IndexedDB, and the Forge server never holds it. Served as static files by Cloudflare (Workers static assets, free, up to 20,000 files of 25 MiB [V S-R13]; question 22), with no Worker code.
3. **Deployed only from a signed release**: the release workflow builds `hersir-web-X.Y.Z.zip`, puts it in `SHA256SUMS`, which `scripts/sign-release.sh` signs as for the app (`release_sig.rs:1-12`), and the `pwa` environment deploys it only after you approve. Anyone can compare what `app.hersir.dev` serves with the signed list; each host checks it daily against its built-in release keys and warns on Settings → Hersir Cloud when they differ ("app.hersir.dev serves files that aren't in Hersir 1.2.0's signed release. Remove your phones until this is explained.") [I].
4. **A strict Content Security Policy** (scripts only from the origin, no `eval`, `connect-src` only `cloud.hersir.dev` and the R2 endpoint), so injected script can't load more or send keys elsewhere.
5. **The host's allowlist bounds the damage** (3.12): a changed PWA can do what you can do from a phone, never widen an agent, read a host's files or open its terminal; *Board and chat only* devices can't run anything.
6. **The MacBook doesn't have this risk**: its code is the installed, signed Hersir, not served by anyone.

**Confirmation on the host for risky commands**: not by default, since you want Run and approvals from the phone; *Board and chat only* (question 23) is the per-device brake instead [I].

#### What the host shows, and its switch

- **The switch**: Settings → Hersir Cloud → **Use from other devices** (a this-computer setting, `remote_on`, **off by default**). Off: the host leaves the relay, refuses sessions and tells its devices "Remote use is off on PC". The tray menu has it too: *Remote use: on · Turn off*.
- **While a device is connected**: a line at the bottom of the host's sidebar island, in `live` teal: "iPhone and MacBook are connected · Disconnect", and a dot on the tray icon. *Disconnect* ends their sessions until they reconnect; *Remove* is in Settings.
- **What a device did**: *Your devices* lists each device with when it was last connected and its last 50 writes (the command, the card or chat, when), kept in a local table (`remote_log`, 30 days) [I]. The activity feed on each card already says what changed; Remote adds "from iPhone" to the writes a device made.

### 3.14 The PWA

#### Installing it

- **iPhone and iPad**: open `app.hersir.dev` in Safari, Share → **Add to Home Screen**. Since iOS 26, "by default, every website added to the Home Screen opens as a web app", and Safari has "zero requirements for installability" [V S-R4]; the manifest still gives the name, icons, colours and `display: standalone`.
- **Android**: Chrome offers *Install app* for a site served over HTTPS with a manifest that has a name, 192 and 512 px icons, a `start_url` and `display: standalone` [V S-R5]; the PWA ships a service worker anyway (for push and the app shell).
- **Any desktop browser**: the same address works without installing; it is how you'd use a host from someone else's computer (sign in, pair, and remove the device afterwards).

#### Web Push for the Chief's questions, holds and approvals

- **iOS**: Web Push works only for a web app on the Home Screen (since iOS 16.4), the permission must be asked from a tap, and every push must show a notification (`userVisibleOnly`); pushes that show none get the subscription revoked [V S-R1]. Android Chrome and desktop browsers push without installing [V S-R5]. So the PWA asks for notifications from a button on its Settings, after *Add to Home Screen* on iOS, and its service worker always shows one.
- **What triggers one**: the host's own four notification kinds (a card on hold, a card waiting for you, the Chief asks, the Chief answered; `notifications.rs:41-52`) and the Chief's calls waiting for your OK, under each device's own switches. The host sends them through the same sink as its desktop notifications (`AppState::desktop`), to each paired PWA device that has push on **and isn't connected right now** (a connected one gets the event in the session).
- **Sealed, so the server can't read them** (question 27): the host seals `{ kind, title, body, url }` (at most 2 KB) with a **push key** that the host and the device derived at pairing (X25519 between their keys, then HKDF), and posts it to `POST /api/v1/push`; the server sends it as the Web Push payload (RFC 8291 caps it near 4 KB [V S-R5]) with `minishlink/web-push` 11 (MIT, July 2026) through `laravel-notification-channels/webpush` 13 (MIT, September 2026; VAPID) [V S-R12]. The service worker opens it with the device's key from IndexedDB and shows "Hersir · PC: The Chief waits for your OK" with the call's line; a tap opens the card or chat. If it can't open it (a re-paired device), it shows "Hersir: something on PC needs you". Apple's Declarative Web Push (iOS 18.4) would show a push without a service worker, but needs a plain payload, so it isn't used [V S-R2].
- **Limits**: one push per kind per host every 30 seconds per device, at most 100 a day; the server keeps the endpoint, the device and the last push time.

#### Storage and offline

- The PWA stores little: its two keys, its token, the hosts' directories (3.11) and the app shell. No data (A3). Safari gives an origin up to 60% of the disk in a browser app and the same to a Home Screen web app; its 7-day cap on script-writable storage doesn't apply to a Home Screen web app, and `navigator.storage.persist()` is granted by heuristics such as being on the Home Screen [V S-R3]. In a Safari tab that isn't installed, keys unused for 7 days can go: the PWA then asks to pair again. The PWA asks for persistent storage after pairing.
- **Offline** (no network): the app shell opens from the service worker and says "No connection"; hosts show as unreachable, as in 3.11.
- A host's answers are held only in memory while a page shows them.

#### The screens that need a phone layout (design card 10, built in card 12)

The phone (under 600 px wide) gets its own layouts for the four places of requirement R1; the rest shows a plain list or "Open this on a computer" [I]:

| Place | Phone layout |
|---|---|
| **The hosts and the sidebar** | the rail becomes a host switcher at the top of a slide-in menu; the sidebar's items in that menu; a bottom bar with Board, Inbox, Chat and a host chip |
| **The board** | one column at a time, full width, with the columns as tabs you swipe between (counts in the tabs); a card's actions (move, Run) from a long press or its page, not drag and drop |
| **A card** | one column: title and state, then tabs Details, Comments, Runs, Files; the run's output full screen with Stop and Continue in a bottom bar; screenshots full width; Properties in a sheet |
| **The Chat** | the threads list, then one thread full screen; the text box above the keyboard with files from the camera or photos; waiting calls with Approve and Discard as full-width buttons |
| **The Inbox** | a list of what waits, each opening its card or chat, with the host's name on each row when there are several hosts |

They use the same tokens and components as the desktop (`design/tokens.json`), at a phone's sizes; touch targets of at least 44 px; `@media (hover: none)` already shows hover-only buttons (`src/styles/app.css:477, 493, 501, 569`).

### 3.15 The relay on the server

**What it is** [I, Reverb's behaviour V S-R6]: Reverb, which Round 2 already runs beside the Laravel app (3.3), relays the session's messages as **client events**: a device and a host both subscribe to one private channel per pair, `private-remote.<host id>.<device id>`, and each sends `client-m` events, which Reverb hands to the other subscribers inside its own process, without the Laravel app's code; Reverb accepts client events from channel members by default (`accept_client_events_from: members`) and never echoes one to its sender. The Laravel app only authorises the channel (`routes/channels.php`: the caller's token is one of the two devices, both are the same account's, they are paired, neither is removed, and the host has remote use on), and rings `device.online`, `device.removed` and `pairing.joined` on `private-account.<user>`.

| Setting (`config/reverb.php`) | Default [V S-R6] | Hersir Cloud |
|---|---|---|
| `max_message_size` | 10,000 bytes | 65,536, so a frame carries up to 48 KiB of sealed data in base64 |
| `rate_limiting` | off (60 a minute when on) | on: 1,200 messages a minute per connection, without cutting it off |
| `max_connections` | none | 5,000 per server, with the `ext-uv` event loop (needed past 1,000) and `nofile` raised as the docs say |
| `activity_timeout`, `ping_interval` | 30 s, 60 s | as they are; presence shows a host gone within about a minute |

**Messages** (5.8 has the formats): a client event's `data` must be a JSON object [V S-R6], so each frame is `{ "s": <session>, "n": <counter>, "p": <part>, "of": <parts>, "d": "<base64 of a Noise message>" }`. A message over 48 KiB is split into parts; one over 1 MiB goes through R2.

**Files and big answers through R2**: the sender asks `POST /api/v1/relay/blobs` for an upload URL (presigned `PUT`, 10 minutes, as backups do, 3.3) and gets a blob id; it uploads the bytes sealed with the session (in 4 MiB chunks, each sealed whole), and sends the blob id and size in the session; the receiver asks `POST /api/v1/relay/blobs/download` for its URL, downloads, opens and checks it. Blobs are deleted after 24 hours by an R2 lifecycle rule and as soon as the receiver says it has them. **Limit: 100 MB per file** (question 24), 2 GB of blobs a day per account; R2 can't cap a presigned upload's size, so the server checks it with a `HEAD` before the receiver gets its URL, as backups do (3.3). Over it: "Files from another device can be up to 100 MB."

**Load per connected device** [I, estimates]: each host keeps one websocket open while remote use is on, idle most of the time (a ping a minute). A device on a board makes a burst of 10 to 30 small calls (a few KB each, compressed); watching a run streams its events at most every 250 ms (the host batches `run-events` and `chat-events` for remote devices at 250 ms instead of 50 ms), about 2 to 20 KB a second. A screenshot or a file goes through R2, not Reverb.

**How fast a command is** [I]: from Spain, a round trip to a server in Germany is a few tens of milliseconds, so a command goes device → server → host → server → device in about 100 to 200 ms, plus the host's own time (1 to 20 ms for most reads). A screen's first load makes its calls at once, so it draws in about 0.2 to 0.4 s; a run's output follows a quarter to half a second behind. The first connection's handshake adds one round trip.

**Costs** (EUR a month, excl. VAT) [I, on Round 2's prices S-L11, S-L16, S-L19]: assume 70% of customers keep a host on with remote use, 1.5 devices each connected 2 hours a day, 30 MB a day relayed and 20 files a day through R2 per active customer.

| | 10 customers | 100 customers | 1,000 customers |
|---|---|---|---|
| Connections at the busiest time | about 15 | about 100 | about 1,000 |
| Reverb's memory and CPU | on the CX23 already there | on the CX23 | its own CX23 (€6, no backups needed) beside the CX43 |
| Traffic (about 1 GB, 10 GB, 0.9 TB a month) | within Hetzner's included traffic | within it | within it (Hetzner's EU cloud servers include 20 TB [I, from S-L19]) |
| R2 for blobs (short-lived; class A and B operations) | €0 (free tier) | €0 | about €1 |
| Web Push (Apple's and Google's push services) | €0 | €0 | €0 |
| **Extra for the relay** | **€0** | **€0** | **about €7** |
| **Hersir Cloud in total** (3.3's table plus the relay) | about €22 | about €31 | about €148 |

### 3.16 Plans

**Recommended** [I]: one plan, **Personal at €5 a month or €48 a year (excl. VAT)**, holding Cloud Backup **and** Hersir on your other devices with the PWA (question 26). The website's line for Multiple devices becomes "Your projects on all your computers and your phone, worked by the computer that holds them." Reasons: the relay costs about €0.007 per customer a month at 1,000 customers, so it needs no price of its own; one plan keeps Stripe, the entitlement (`features: ["backup", "remote"]`, 5.4) and the website simple; and "my board on my phone" is what makes people pay, which Jefsev said of the chat (decision 12). **Without a plan**: everything on a host's own screen stays free (P1); pairing works, but a device's sessions start only while the account's plan is active, since the relay is the hosted service. A lapsed plan: devices say "Hersir Cloud's plan ended on 9 Dec 2026: use PC on PC itself, or renew", and the hosts go on working as before.

## 4. Options

### 4.1 How to build Remote (Round 3)

Jefsev chose Remote (0.1), so the options are ways to build it, not Remote against Sync [I; sizes in agent-days, like `PAID-FEATURES.md`, for cards 7 to 12 on top of cards 1 and 2].

| | **A. The host's own commands, forwarded** (recommended) | B. A remote API of its own | C. Round 2's Sync with a PWA that holds a copy (replaced by Jefsev's choice) |
|---|---|---|---|
| What it is | the MacBook's Hersir forwards every command of the workspace on screen to its host, from Tauri's app-level invoke handler; the PWA swaps the IPC as `npm run browser` does; the host runs an allowlist of its own Tauri commands (3.12) | a smaller set of remote calls made for the phone (board, card, chat, Inbox), with new screens for them in the PWA and on the MacBook | every device keeps a copy and merges changes; runs and the Chief's chat go to the computer that holds the project |
| Screens | **the same screens everywhere**; phone layouts by CSS and a few phone components | a second set of screens to build and keep in step with the app | the same screens, on a local copy |
| What changes in the app | a forwarding handler on the desktop (`src-tauri/src/remote/`), the host's dispatcher and its allowlist, file uploads and downloads, a PWA entry beside `browser.html`, a rail | the same host side plus new remote endpoints and their screens | Round 2's cards 6 to 11, plus the sync engine in the browser (WebAssembly or TypeScript) |
| A new command in the app | works remotely when added to the allowlist (one line) | needs a remote endpoint and a screen | needs sync to cover its tables |
| While a host is off | its projects grayed out (A2) | the same | everything readable and writable on the copy |
| Risks | Tauri's `Invoke` is "explicitly NOT stable" [V S-R10], so the forwarding handler pins Tauri's minor version and has a test; a remote device gets the host's full board (bounded by 3.12) | two UIs drift apart; slower to reach "the whole Hersir" (R1) | merging and its bugs; a database in a phone's browser; the copies Jefsev doesn't want (A3) |
| Size | **about 21 to 29** | about 35 to 45 | about 45 to 65 |

**Recommended: A.** It gives "the whole Hersir" (R1) on the phone and the MacBook from the screens that already exist, and the host's allowlist is one list to review.

**Libraries for A** [V where marked]: on the host and the MacBook, `snow` (Noise; Apache-2.0 or MIT [I, check its version when card 8 starts]), `tungstenite` (MIT or Apache-2.0) with Round 2's ~300-line Pusher-protocol client, and the RustCrypto crates already in the lock (`hkdf`, `hmac`, `sha2`, `ed25519-dalek`; `x25519-dalek` new, BSD-3); in the PWA, WebCrypto only (X25519, Ed25519, HKDF, AES-GCM [V S-R11]) and `pusher-js` (MIT [I]) or a small client of our own for Reverb; on the server, Reverb and `laravel-notification-channels/webpush` 13 with `minishlink/web-push` 11 (both MIT [V S-R12]).

### 4.2 The stack (Round 2)

The decisions fix the stack; what is left to choose is how the computers hear about news, and the decisions themselves are compared with Round 1's choice [I; sizes in agent-days, like `PAID-FEATURES.md`]. **Round 3**: A's Reverb is now the relay too (3.15); B's polling can't carry a remote session.

| | **A. One Forge server: Laravel, SQLite with Litestream, Reverb as the doorbell** (recommended) | B. The same Laravel app, polling only | C. Round 1's option A: a Rust server on Hetzner, Zitadel, Paddle (replaced) |
|---|---|---|---|
| What it is | 3.3 to 3.8 | no websockets: every open Hersir asks every 5 to 30 seconds | one Rust binary, Hetzner Object Storage, a bought sign-in service, a merchant of record |
| News reaches another computer in | about a second | up to the polling interval | about a second (long polling in Rust is cheap) |
| Load at 1,000 customers | thousands of idle sockets in one Reverb process (with an event loop), a fetch per change | a few hundred requests a second at 5 seconds; at 30 seconds a slow chat | small |
| Extra work | a Pusher-protocol client in Rust (about a day); Reverb on Forge is a switch | none | the account pages by hand, Zitadel's set-up, Paddle's review |
| Libraries | Laravel 13, Fortify, Passport 13, Cashier 16, Reverb (all MIT); Litestream (Apache-2.0); app: `chacha20poly1305`, `hkdf`, `hmac`, `x25519-dalek`, `ed25519-dalek`, `fastcdc`, `tungstenite` (MIT or Apache-2.0; the two dalek crates BSD-3) | the same without Reverb and `tungstenite` | `axum`, `rusqlite`, `rusty-s3`, `jsonwebtoken` |
| Risks | a websocket that some networks block (polling covers it); Reverb past 1,000 connections needs its event loop; two US sub-processors | a slow chat, or a busy server | a sign-in service at $100 a month before the first customer; not the stack you know |
| Monthly cost at 10 / 100 / 1,000 customers | about €22 / €31 / €141 | about the same, more CPU at 1,000 | about €14 / €14 / €71 plus Zitadel Pro ($100) |
| Size, all eleven cards | **about 40 to 60 agent-days** | about 1 day less | about 30 to 40, for eight cards without the chat |

**Recommended: A.** It is the decisions as Jefsev made them, it keeps the chat as quick as a local one, and polling remains its fallback, so B costs nothing to keep in reserve.

## 5. The design (option A)

Sections 5.1 to 5.7 are Round 2's design of Cloud Backup, the server and sign-in, which still holds; their parts for Multiple devices and the Chief's chat by commands are **replaced** by Remote (marked where they are). **Section 5.8 designs Remote and the PWA.**

### 5.1 The screens

**On the desktop**: a new Settings tab, **Hersir Cloud** (`#/settings/cloud`, after GitHub and Bitbucket in `SettingsPage.tsx:34-36` and `router.ts:16`), and *Restore from Hersir Cloud* on the first screen of a new install (`src/pages/OnboardingPage.tsx`). The local Backups section stays in General as it is.

Signed out:

```
Hersir Cloud
  Encrypted backups of your data, kept off your computer. Restore them on a new one.
  Personal: €5 a month or €48 a year, excl. VAT. Hersir can't read what it stores.
  [Sign in]  [Create an account ↗]                Signing in opens your browser.
```

After the first sign-in, before anything is uploaded (once per account; decision 2's warning):

```
Save your recovery code
  Hersir encrypts your data on this computer before it leaves. Only your computers and this
  recovery code can open it. If you lose this code and every computer signed in to Hersir Cloud,
  your cloud backups can't be opened by anyone, Hersir included. Hersir won't show it again.

      HC-7K2Q-M4XD-9TRP-B6WN-3HFJ-Q8YC          (mono)
  [Copy]  [Save as a file…]
  Type its last group to show you have it:  [____]                 [Turn on Cloud Backup]
```

Signed in:

```
Hersir Cloud
  Account        jeffrey@… · Personal, renews 9 Nov 2027               [Account page ↗]  [Sign out]
  Storage        ▓▓▓▓░░░░░░░░░░░░  1.4 of 10 GB
  Recovery code  Made on 9 Nov 2026                                    [Make a new recovery code]

Cloud Backup
  Back up        (•) Daily   ( ) Only when I ask       [x] Also when I quit Hersir completely
  Last backup    Today 09:02 · 38 MB sent · next tomorrow 09:02       [Back up now]
  Backups        Today 09:02       Daily · this computer      1.2 GB  [Restore]
                 Yesterday 18:40   On quit · Laptop           1.2 GB  [Restore]

Use from other devices                                                  (Round 3: 5.8)
  [x] Use PC's projects from my other computers and my phone
  Your devices   MacBook · connected now · Everything      [Remove…]
                 iPhone (Safari) · 2 hours ago · Everything [Remove…]
  [Add a device]
```

*Remove…* asks: "Remove MacBook? It is disconnected and signed out of Hersir Cloud at once, and can no longer use PC. Is it lost or stolen?" with **Remove** and, once the key change exists (question 18), **Remove and change the key**. (**Replaced**: Round 2's *Your computers* with the sync switch and *The Chief's computer*.)

A new computer (the first screen, or Settings → Hersir Cloud while signed out):

```
Restore from Hersir Cloud
  1  Sign in              [Sign in]   opens your browser (password, then your authenticator's code)
  2  Your recovery code   [HC-____-____-____-____-____-____]
  3  Pick a backup        Today 09:02 · Desktop · 1.2 GB      [Restore]
     Hersir restarts to finish, as a restore from a folder does.
```

**Replaced in Round 3** (the two sketches below): with Remote, the Chat page in a host's workspace is that host's own Chat page, so nothing in it changes; a host that is off shows 3.11's offline page instead.

**The Chief's chat on a computer that isn't the Chief's** (Round 2; the Chat page as it is; only the lines under your message and above the text box are new):

```
  You · from this computer                                            09:14
  Can you split GA-310 into a backend and a frontend card?
  Sent to Desktop · waiting for the Chief                      (then: "Desktop is answering…")

  The Chief                                                            09:14
  I split it in two: GA-311 (backend) and GA-312 (frontend) …         (streams in as it is written)
  Waiting for your OK:  update_task GA-310 …        [Approve]  [Discard]

  (Desktop off)  Waiting for Desktop: offline since 09:12. The Chief answers when Desktop is back.
  [ Message the Chief…                                                                     ]
  The Chief works on Desktop. Your messages go there.
```

**Replaced in Round 3**: elsewhere for Multiple devices: "Running on Laptop" on a card claimed by another computer; "not synced yet" beside a number made offline; *Runs on* in the agent form; a band when Hersir Cloud can't be reached ("Agents start again when it can") or another computer runs a newer Hersir ("Update Hersir to go on syncing").

**On the web** (`cloud.hersir.dev`, the Livewire starter kit with Hersir's colours and type): *Create an account* (name, email, password) → *Check your email* → *Set up two-factor authentication* (QR code, a code to confirm, the 8 recovery codes to save; required) → *Your account*: plan and renewal with **Subscribe** (Stripe Checkout: monthly or yearly) or **Manage billing** (Stripe's portal), storage used, the backups list, your computers with **Remove**, two-factor and recovery codes, password, **Withdraw from the contract** (consumers, in their first 14 days) and **Delete account** (password and code; the cloud copies go after 7 days). A sign-in from the desktop shows *Signed in. You can close this tab and go back to Hersir.*

**How it fits the design system** (`design/README.md`) [V components; I use]: `FormSection` and `Field` as every Settings section; buttons `btn`, `btn primary`, `btn ghost`, `btn sm`; the radios and check of `BackupSettings.tsx`; its `backup-list` rows for the cloud list and the computers; its `ask` dialog for Restore and Remove; status lines with the step and `LoaderCircle`; `ok-banner` and `error-banner`. The recovery code and sizes in mono. Blue `accent` for actions; gold `needs` only for "Save your recovery code", "Waiting for your OK" and "Update Hersir", which need you; teal `live` for the Chief's answer while it streams, as on the Chief's own computer; `danger` for failures. Sentence case, buttons that say what happens. Each state in dark and light.

### 5.2 The data

**On the computer** [I]:

- **This computer's settings** (added to `scope::THIS_COMPUTER`, so a restore keeps them): `cloud_account` (`{ id, email }`), `cloud_backup` (`{ on, every: "daily"|"manual", onQuit }`), `cloud_last_backup`, `cloud_last_error`.
- **Keychain** (service `hersir`): `cloud/<account>/token` (this computer's Passport token), `cloud/<account>/key` (the account key, with its epoch), `cloud/<account>/device` (this computer's Ed25519 and X25519 secret keys). Never the recovery code.
- **Replaced in Round 3**: the shared setting `chief_device`, migration `0023_sync.sql` (`sync_log`, `sync_clock`, `cloud_link`, `agent_configs.home_device_id`) and migration `0024_cloud_chat.sql` (`cloud_roster`, `cloud_outbox`, `cloud_commands_seen`). Remote's local tables are in 5.8.

**On the server**: 3.3's tables.

### 5.3 What changes in which files

**The app** (this repository, open source):

| Where | What | Card |
|---|---|---|
| new `crates/gizai-cloud-client` (no Tauri) | `api.rs` (HTTP with `ureq`, as `oauth.rs`); `keys.rs` (AK, HKDF, the recovery code with its check character, sealing, the computer's keys); `chunks.rs` (page chunks, FastCDC, zstd, seal and open); `backup.rs`; `restore.rs` (download into a format-1 folder); `entitlement.rs` (Ed25519 check). Round 3: `doorbell.rs` (the Pusher-protocol client), `pairing.rs` and `noise.rs` (the session) move to card 8 (5.8) | 4, 8 |
| `crates/gizai-agents/src/oauth.rs` | nothing: `discover` and `begin` with a fixed client id already fit (3.4) | none |
| `crates/gizai-core/src/backups.rs` | a page-for-page copy (rusqlite's `backup` feature) beside `VACUUM INTO`; the manifest stays format 1 | 4 |
| `crates/gizai-core/src/scope.rs` | the cloud settings as this computer's | 4 |
| `src-tauri/src/cloud.rs` (new), `lib.rs` | the commands, the schedule (on the ticks of `backups.rs`), On quit, the event `cloud-changed` | 4 |
| `src/api.ts`, `src/types.ts`, `src/router.ts` | the commands and types; `cloud` in `SETTINGS_TABS` | 5 |
| `src/components/CloudSettings.tsx` (new), `src/pages/SettingsPage.tsx`, `src/pages/OnboardingPage.tsx` | the tab and *Restore from Hersir Cloud* | 5 |
| **Replaced in Round 3** | `crates/gizai-core/src/sync/` and `0023_sync.sql`; `src-tauri/src/sync.rs`; `src-tauri/src/chief_remote.rs` and `0024_cloud_chat.sql`; "Running on Laptop", "not synced yet", *Runs on* and the chat's outbox lines. Remote's files are in 5.8 | none |

**The server** (`Yotech-AI/hersir-cloud`, private; a Laravel 13 app from the Livewire starter kit):

| Where | What | Card |
|---|---|---|
| `app/Http/Middleware/RequireTwoFactor.php`, `RequirePkceS256.php` | 2FA before anything; S256 only | 2 |
| `routes/web.php` (the well-known documents, the account pages), `routes/api.php` (`/api/v1`), `routes/channels.php` | 5.4's and 5.8's routes | 2, 3, 6, 7 |
| `app/Http/Controllers/Api/{Device,Entitlement}Controller.php` | computers and devices, the plan | 2 |
| `app/Http/Controllers/Api/{Key,Object,Snapshot}Controller.php` | Cloud Backup's API | 3 |
| `app/Http/Controllers/Api/{Pairing,RelayBlob,Push}Controller.php`, `app/Events/*`, `config/reverb.php` | the relay (5.8) | 7 |
| `app/Models/*`, `database/migrations/*` | 3.3's tables and 5.8's | 2, 3, 6, 7 |
| `app/Console/Commands/Comp.php` | plans by hand | 2 |
| `app/Console/Commands/{PruneSnapshots,CollectObjects,NightlyCopy}.php`, `routes/console.php` | retention, garbage collection, the nightly copy | 3 |
| `app/Listeners/StripeWebhook.php`, `app/Support/Entitlements.php`, `resources/views/livewire/account/*` | the plan from Cashier, the signed entitlement, the account page, emails | 6 |
| `deploy/litestream.yml`, `deploy/forge-deploy.sh`, `RUNBOOK.md`, `PROTOCOL.md` | Litestream's config, the deploy script, the restore drill, the published protocol (P4) | 2 (7 adds the relay to `PROTOCOL.md`) |
| **Replaced in Round 3** | `{Workspace,Tx,Claim,Command,Live}Controller.php` (sync, claims, commands) | none |
| `tests/` (Pest) | per card | all |

### 5.4 Contracts

#### The server's API (cards 2, 3 and 6; used by cards 4, 5 and 8; card numbers of Round 3)

JSON over HTTPS at `https://cloud.hersir.dev/api/v1`. `Authorization: Bearer <this computer's Passport token>` (scope `cloud`), except `POST /devices`, which takes the browser's token (scope `device:register`). Errors: `{ "error": "<code>", "message": "<a sentence for people>" }` with 401 (`signed_out`, `removed`), 402 (`plan_required`, `read_only`), 403 (`not_yours`), 404 (`not_found`), 409 (`claimed`, `exists`, `epoch`), 413 (`quota`), 422 (`invalid`, `missing_objects`), 426 (`update_hersir`), 429 (`slow_down`). Times are Unix milliseconds; ids are the app's UUIDv7 strings; object ids are 64 hex characters; `body` and sealed blobs are base64.

| Call | Body → answer | Card |
|---|---|---|
| `POST /devices` | `{ device_id, name, platform, app_version, sign_pub, box_pub }` → 201 `{ account: { id, email }, device_token, expires_at }` (the browser's token is revoked) | 2 |
| `POST /devices/me/token` | `{}` → `{ device_token, expires_at }` (the old one is revoked) | 2 |
| `GET /devices` | → `[{ id, name, platform, app_version, kind: "desktop"\|"web", remote_on, created_at, last_seen_at, online, this, sign_pub, box_pub, revoked_at }]` (`kind` and `remote_on` new in Round 3) | 2 |
| `PUT /devices/me` | `{ name?, app_version?, remote_on? }` → 200 (new in Round 3: a host says its switch is on or off) | 2 |
| `DELETE /devices/{id}` (`me` to sign out) | `{ lost: boolean }` → 204 | 2 |
| `GET /me/entitlements` | → `{ payload, sig }`; payload `{ v: 1, account, plan: "personal"\|"none", status: "active"\|"past_due"\|"lapsed"\|"none", features: ["backup","remote"], quota_bytes, used_bytes, valid_until, read_only_until, issued_at }` (Round 3: `remote` in place of `sync`) | 2 (comps), 6 (Stripe) |
| `GET /keys` | → `{ key_id, epoch, sealed_by_code }` or 404 | 3 |
| `PUT /keys` | `{ key_id, epoch, sealed_by_code, if_epoch }` → 201 (first) or 200; 409 `epoch` when `if_epoch` isn't the current one | 3 |
| `POST /objects/missing` | `{ ids: [hex64] }` (at most 1,000) → `{ missing: [hex64] }` | 3 |
| `POST /objects/upload` | `{ objects: [{ id, bytes }] }` (at most 4,194,368 bytes each) → `{ urls: [{ id, url, headers, expires_at }] }`; 402, 413 | 3 |
| `POST /objects/download` | `{ ids }` → `{ urls: [{ id, url, expires_at }] }` (also while read-only) | 3 |
| `POST /snapshots` | `{ name, made_at, device_id, reason: "daily"\|"manual"\|"quit"\|"before-restore", schema_version, app_version, manifest_id, object_ids, bytes }` → 201; 422 `missing_objects` with `{ ids }`; 413 | 3 |
| `GET /snapshots` | → `[{ name, made_at, device_id, device_name, reason, schema_version, app_version, bytes, manifest_id, deleted_at }]`, newest first | 3 |
| `DELETE /snapshots/{name}`, `POST /snapshots/{name}/undelete` | → 204 (kept 7 more days); → 200 | 3 |
| **Replaced in Round 3** | Round 2's card 7 calls: `/workspaces/{ws}` with its `txs`, `files`, `claims`, `chief`, `commands`, `commands/ack`, `commands/sent` and `live`; and `PUT /keys/roster` (the roster served the chat's commands). The relay's calls are in 5.8 | none |

**Reverb**: Round 3's channels and events are in 5.8 (`private-account.<user id>`, `presence-account.<user id>`, `private-remote.<host>.<device>`); Round 2's `presence-workspace.<ws>` with `sync.tx`, `command`, `command.status`, `claim.changed`, `chief.changed` and `chat.live` is **replaced**.

**The webhook** (card 6): `POST /stripe/webhook`, Cashier's, with `customer.subscription.created`, `customer.subscription.updated`, `customer.subscription.deleted`, `invoice.paid`, `invoice.payment_failed`, `checkout.session.completed`.

#### A command (Round 2; **replaced in Round 3** by the Noise session of 5.8)

```
body = "HCC1" ‖ epoch (1 byte) ‖ nonce (24) ‖ XChaCha20-Poly1305(command key, nonce, plaintext,
       associated data = id ‖ workspace ‖ to) ‖ Ed25519 signature (64) by the sender's computer key over all before it

plaintext (JSON):
{ "v": 1, "id": "<UUIDv7>", "from": "<device id>", "to": "<device id>", "seq": 42, "at": 1763380000000,
  "kind": "chat.send" | "chat.new" | "chat.stop" | "chat.send_now" | "chat.queue_edit" | "chat.queue_remove"
        | "chat.set_cli" | "pending.approve" | "pending.approve_all" | "pending.discard",
  "args": {…} }

args: chat.send { thread_id, text, files: [{ id, name, bytes, mime }] }   (the message is stored under the command's id)
      chat.new { thread_id, agent_id, text, files }      chat.stop { thread_id }      chat.send_now { thread_id }
      chat.queue_edit { queued_id, text }                chat.queue_remove { queued_id }
      chat.set_cli { thread_id, cli: string | null }     pending.approve { pending_id }
      pending.approve_all { pending_ids }                pending.discard { pending_id }
lifetime: 24 hours for pending.*, 7 days for the rest
a live answer (POST …/live): "HCL1" ‖ epoch ‖ nonce ‖ sealed { thread_id, events: ChatEvent[] } ‖ signature
```

#### Hersir's commands for the Hersir Cloud tab (cards 4 and 5), in `src/api.ts` with the Tauri command in the comment

```ts
type CloudPlan = { name: "personal" | "none"; status: "active" | "past_due" | "lapsed" | "none";
  validUntil: number | null; readOnlyUntil: number | null; quotaBytes: number; usedBytes: number };
type CloudStep = "copying" | "chunking" | "uploading" | "registering" | "downloading" | "checking";
type CloudStatus = {
  signedIn: boolean; email: string | null; signingIn: boolean;
  plan: CloudPlan | null;                       // from the signed entitlement; null while signed out
  key: "missing" | "here" | "needed";           // missing: none made yet; needed: the account has one, this computer doesn't
  recoveryMadeAt: number | null;
  backup: { on: boolean; every: "daily" | "manual"; onQuit: boolean;
            running: { step: CloudStep; done: number; total: number } | null;
            last: { at: number; name: string; sentBytes: number } | null; next: number | null;
            lastError: { at: number; message: string } | null };
};
type CloudBackup = { name: string; madeAt: number; device: { id: string; name: string; this: boolean };
  reason: "daily" | "manual" | "quit" | "before-restore"; bytes: number; restorable: boolean; problem: string | null };
type CloudComputer = { id: string; name: string; platform: string; appVersion: string; lastSeenAt: number | null;
  online: boolean; this: boolean; chief: boolean };

cloudStatus(): Promise<CloudStatus>                                   // cloud_status
cloudSignIn(): Promise<CloudStatus>                                   // cloud_sign_in: opens the browser, waits up to 10 min
cloudCancelSignIn(): Promise<void>                                    // cloud_cancel_sign_in
cloudSignOut(): Promise<CloudStatus>                                  // cloud_sign_out
cloudMakeKey(): Promise<{ recoveryCode: string }>                     // cloud_make_key: the first computer only
cloudConfirmRecovery(lastGroup: string): Promise<CloudStatus>         // cloud_confirm_recovery: turns Cloud Backup on
cloudEnterRecovery(code: string): Promise<CloudStatus>                // cloud_enter_recovery: a computer whose key is "needed"
cloudNewRecovery(): Promise<{ recoveryCode: string }>                 // cloud_new_recovery: the old code stops working
cloudSaveSettings(s: { on?: boolean; every?: "daily" | "manual"; onQuit?: boolean }): Promise<CloudStatus>  // cloud_save_settings
cloudBackUpNow(): Promise<CloudStatus>                                // cloud_back_up_now
cloudListBackups(): Promise<CloudBackup[]>                            // cloud_list_backups
cloudStartRestore(name: string): Promise<RestoreStatus>               // cloud_start_restore: then GA-218's restore
cloudComputers(): Promise<CloudComputer[]>                            // cloud_computers
cloudRemoveComputer(id: string, lost: boolean): Promise<CloudComputer[]>   // cloud_remove_computer
cloudOpenAccount(): Promise<void>                                     // cloud_open_account: the account page in the browser
onCloudChanged(f: (s: CloudStatus) => void): Promise<() => void>      // event "cloud-changed"
```

Refusals are sentences: "Sign in to Hersir Cloud first.", "That recovery code doesn't open this account's backups.", "Hersir Cloud is full: 10.0 of 10 GB.", "Your plan ended on 9 Dec 2026: backups can be restored until 7 Feb 2027.".

#### Hersir's commands for Multiple devices (Round 2; **replaced in Round 3** by 5.8's)

```ts
type SyncStatus = { on: boolean; lastSyncAt: number | null; toSend: number; live: boolean;   // live: the doorbell is connected
  problem: null | { kind: "offline" | "update_hersir" | "plan" | "error"; message: string };
  chief: { deviceId: string; deviceName: string; online: boolean; here: boolean } | null };
syncStatus(): Promise<SyncStatus>                       // sync_status
syncSet(on: boolean): Promise<SyncStatus>               // sync_set
chiefSetComputer(deviceId: string): Promise<SyncStatus> // chief_set_computer
onSyncChanged(f: (s: SyncStatus) => void)               // event "sync-changed"
// on a card (types.ts Task): runningOn: { deviceId: string; deviceName: string } | null; numberSynced: boolean
// on an agent (AgentConfig): homeDeviceId: string | null   (saved with the agent's form)
```

#### The Chief's chat on another computer (Round 2; **replaced in Round 3**: the chat's commands go to the host as they are)

The chat's commands keep their names and arguments (`src/api.ts:440-467` on main): `sendChat`, `sendChatQueue`, `editQueuedChat`, `removeQueuedChat`, `setChatCli`, `stopChat`, `approvePendingAction`, `approvePendingActions`, `discardPendingAction`. On the Chief's computer they work as today; on another one they send a command and resolve once it is stored on the server (or refuse with a sentence: "Sign in to Hersir Cloud to write to the Chief from this computer."). The live answer from the Chief's computer arrives as the same `chat-events` event the Chat page already listens to (`onChatEvent`, `src/api.ts:472`). New:

```ts
type OutgoingStatus = "sending" | "waiting" | "delivered" | "done" | "moved" | "refused" | "expired";
type Outgoing = { id: string; threadId: string;
  kind: "message" | "new_chat" | "stop" | "send_now" | "queue_edit" | "queue_remove" | "runs_on" | "approve" | "approve_all" | "discard";
  text: string | null; status: OutgoingStatus; to: { deviceId: string; deviceName: string; online: boolean; lastSeenAt: number | null };
  at: number; expiresAt: number; note: string | null };      // note: "Waiting for Desktop: offline since 09:12."
chatOutbox(threadId: string): Promise<Outgoing[]>           // chat_outbox
resendOutgoing(id: string): Promise<Outgoing>               // resend_outgoing: an expired or moved one, again
cancelOutgoing(id: string): Promise<void>                   // cancel_outgoing: one still waiting on the server
onOutgoing(f: (o: Outgoing) => void)                        // event "chat-outgoing"
// on a message (types.ts ChatMessage): fromDevice: { id: string; name: string } | null
// on a pending action (PendingAction): decidedOn: { id: string; name: string } | null
```

### 5.5 Security and privacy

- **Keys** only in the keychain, wiped from memory after use (`zeroize`), never in errors or logs (as `oauth.rs` keeps tokens out of them). The recovery code is never stored.
- **The server** sees ciphertext, keyed object names, sizes, times, computer names and who sends commands to whom; never content, never a command's text.
- **Tokens**: one Passport token per computer, revocable, checked on every request; the browser's token is single-use. Presigned URLs last 10 minutes; computers never get delete URLs.
- **The account**: 2FA required on every route, the desktop's sign-in included; S256 only; Fortify's rate limits; an email on every new computer, on a removal and on a 2FA reset.
- **Remote use** (Round 3, replacing Round 2's sealed commands): 3.13 and 5.8's *Security and privacy*.
- **The server itself**: SSH keys only, Forge's firewall, unattended security updates, `.env` outside the repository, the R2 tokens limited to their buckets; Cloudflare in DNS only for `cloud.hersir.dev`.
- **A stolen laptop with a running Hersir has the key**: remove it, and change the key once that exists (question 18).

### 5.6 How it is tested

- **The client crate**: test vectors for HKDF, the recovery code and the command format; seal and open; a tampered byte, a replayed id, a lower counter, a removed computer and an old epoch are refused; restore(backup(db)) equals db; an unchanged database uploads no new chunks; an upload cut halfway resumes; the entitlement signature (good, bad, expired). A fake server on 127.0.0.1 and the memory keychain.
- **The app**: the commands under `scripts/ui-test.sh` with `HERSIR_FAKE_KEYCHAIN` and `HERSIR_CLOUD_URL` pointing at the Laravel app run locally (`php artisan serve`, `reverb:start`, an S3 fake); never a real account in tests.
- **The server** (Pest): every route with its errors; a computer can't reach another account's objects, snapshots, workspaces or commands; 2FA required on every web route, `/oauth/authorize` included (a test lists the routes); the loopback redirect on a random port; Cashier webhooks from Stripe's fixtures (signature, retries, out of order); retention and garbage collection with a fake clock; the quota with real sizes; the lapse dates; a restore drill from Litestream in CI.
- **Remote and the PWA** (Round 3, replacing Round 2's sync and chat tests): 5.8's *How it is tested*.

### 5.7 What is left out

Changing the account key (question 18); approval of a new computer's Cloud Backup key from another one (the recovery code does it in v1); teams and SSO; passkeys; server-side search or a web view of the data; a self-hosted server; sharing with clients. Round 3: the phone and the MacBook are now in (5.8, which lists what Remote leaves out); sync and agents across devices are replaced by Remote.

### 5.8 Remote and the PWA (Round 3)

#### The screens

**On a desktop** (cards 1 and 9), in Hersir's islands (`design/README.md`, `design/islands/README.md`), with existing components (`FormSection`, `Field`, `btn`, `ask` dialogs, `ok-banner`, `error-banner`, lucide icons):

- **The hosts' rail** (3.11's sketch): tiles on the background left of the sidebar island; selected tile on `canvas-hover`; Inbox badge on `island`; `live` dot while connected; a host that is off grayed with `MonitorOff`; `+` opens *Add a computer*.
- **The offline page** and the other host states (3.11, *A host that is off*), in the page island; the band "PC went offline at 14:02…" in `needs` gold at the top of a page left open.
- **The repository note** in `ProjectForm.tsx` under *Folder* (3.11, A1), in the field's hint style with `MonitorSmartphone`, and **Open EUQR on PC**.
- **Host-only places**: "The Editor works on PC itself." on the Editor and Terminal pages; "Change this on PC" on read-only Settings tabs and the agent form.
- **Settings → Hersir Cloud → Use from other devices** (on a host), after Cloud Backup:

```
Use from other devices
  [x] Use PC's projects from my other computers and my phone
      They send PC's own commands through Hersir Cloud, end to end encrypted. PC must stay on.
  [x] Keep PC awake while this is on                        (question 21)
  Your devices
    MacBook · desktop · connected now · Everything                   [Last actions]  [Remove…]
    iPhone (Safari) · phone · 2 hours ago · Board and chat only      [Last actions]  [Remove…]
  [Add a device]
```

- **Add a device** (an `ask` dialog on the host): the QR code (200 px, on white in both themes), the code to type in mono (`HP-7K2Q-…`), "Open Hersir on your other computer and choose *Add a computer*, or scan this with your phone's camera. It works for 5 minutes."; then, when a device joins, the question of 3.13 step 3 with *What it may do* and **Add** / **Cancel**.
- **Add a computer** (on a device; the rail's `+`): the account's computers that aren't paired yet ("PC · online · not paired"), each with **Pair**, which asks for the code shown on that computer; and the line "To add your phone, open app.hersir.dev on it."
- **While devices are connected** (on the host): a line at the bottom of the sidebar island in `live` teal, "MacBook and iPhone are connected · Disconnect".

**In the PWA** (cards 10 to 12): *Sign in* (email, password, authenticator code, as the account pages); *Pair with a computer* (open from the QR link, or "Scan the code in Hersir on your computer: Settings → Hersir Cloud → Use from other devices → Add a device"); *Add to Home Screen* (iOS, with the Share icon) and *Allow notifications* in its Settings; then the phone layouts of 3.14.

#### The data

**On a host** (card 8, migration `0023_remote.sql`, this computer's tables, never sent anywhere; main is at schema 22 [V `db.rs:12` on `723097f`, check again when built]):

```sql
CREATE TABLE remote_devices (        -- devices paired with this host
  device_id TEXT PRIMARY KEY,        -- the device's Hersir Cloud id
  name TEXT NOT NULL, kind TEXT NOT NULL CHECK (kind IN ('desktop','web')),
  box_pub BLOB NOT NULL, sign_pub BLOB NOT NULL,      -- pinned at pairing
  scope TEXT NOT NULL CHECK (scope IN ('full','board')),
  push_on INTEGER NOT NULL DEFAULT 0,
  paired_at INTEGER NOT NULL, last_seen_at INTEGER, removed_at INTEGER
) STRICT;
CREATE TABLE remote_log (            -- the writes devices made here, 30 days
  id INTEGER PRIMARY KEY, device_id TEXT NOT NULL, at INTEGER NOT NULL,
  cmd TEXT NOT NULL, target TEXT                       -- a card's identifier or a chat's title
) STRICT;
CREATE TABLE remote_hosts (          -- on a device that is a desktop: the hosts it is paired with
  host_id TEXT PRIMARY KEY, name TEXT NOT NULL,
  box_pub BLOB NOT NULL, sign_pub BLOB NOT NULL, paired_at INTEGER NOT NULL,
  directory_json TEXT, directory_at INTEGER           -- 3.11's directory: projects only, no data
) STRICT;
```

Settings (this computer's, added to `scope::THIS_COMPUTER`): `remote_on` (false), `remote_keep_awake` (true on a desktop without a battery), `remote_last_host`. Keychain: nothing new (the computer keys of 3.2). **In the PWA**: IndexedDB `hersir` with `keys` (the two non-extractable `CryptoKey` pairs), `token`, `hosts` (as `remote_hosts`), `push` (the subscription and each host's push key).

**On the server** (card 7, migrations; `devices` gets `kind` and `remote_on`):

```
pairings(id PK, user_id, host_id, device_id NULL, join_body JSON NULL, confirm_body JSON NULL,
         created_at, expires_at, confirmed_at NULL)                -- deleted 1 day after expiry
device_pairs(host_id, device_id, user_id, created_at, PRIMARY KEY(host_id, device_id))   -- for channel rules
push_subscriptions(device_id PK, endpoint, p256dh, auth, created_at, last_push_at, pushes_today)
relay_blobs(id PK, user_id, from_device_id, to_device_id, bytes, created_at, received_at NULL)  -- R2 key b/<user>/<id>
```

#### What changes in which files

**The app** (this repository):

| Where | What | Card |
|---|---|---|
| `src-tauri/src/remote/mod.rs` (new), `lib.rs` | the switch, the relay connection (Round 2's Pusher-protocol client on `tungstenite`, `remote/doorbell.rs`), sessions per host and per device, keep-awake, the tray item | 8 |
| `src-tauri/src/remote/pairing.rs`, `noise.rs` | pairing with its MACs; `Noise_KK_25519_AESGCM_SHA256` with `snow`; push keys | 8 |
| `src-tauri/src/remote/dispatch.rs` | **the allowlist**: one `remote_commands!` list that names each command of 3.12's "as they are" and "need a change" groups with its arguments, calls the same function as `generate_handler!` (`lib.rs:818`) with `app.state()` and `app.clone()`, and checks the two forms' fields; anything not on it is refused | 8 |
| `src-tauri/src/remote/files.rs` | uploads into `<data dir>/remote-uploads/<id>/` (deleted after 24 h), `remote:` tokens, blobs out (`open_file`, `screenshot_bytes`, answers over 1 MiB) | 8 |
| `src-tauri/src/remote/events.rs` | `listen_any` on the eight forwarded events [V S-R10], `run-events` and `chat-events` batched at 250 ms per device | 8 |
| `src-tauri/src/remote/forward.rs`, `lib.rs` | **the desktop as a device**: the app's `invoke_handler` becomes a closure that runs `generate_handler!`'s handler for this computer's workspace and the window's own commands, and for another host's workspace sends `invoke.message.command()` and its payload to that host and answers with `invoke.resolver` [V the API, S-R10; I the use]; local paths in `add_files`, `send_chat`, `check_files` uploaded first; `open_file` and `screenshot_bytes` answered from blobs; the host's events emitted to the window under their own names | 8 |
| `src-tauri/src/notifications.rs` | the `Desktop` sink also sends sealed pushes to paired PWA devices not connected | 8 |
| `src-tauri/src/remote/check_web.rs` | the daily check of `app.hersir.dev` against the signed release (3.13) | 11 |
| `crates/gizai-core/migrations/0023_remote.sql`, `scope.rs` | the tables and settings above | 8 |
| `src/api.ts`, `src/types.ts` | the commands and events below | 9 |
| `src/components/HostRail.tsx`, `HostOffline.tsx`, `RemoteSettings.tsx`, `PairDialog.tsx`, `AddComputer.tsx` (new); `App.tsx`, `Sidebar.tsx`, `SettingsPage.tsx`, `ProjectForm.tsx`, `EditorPage.tsx`, `TerminalPage.tsx`, `AgentForm.tsx` | the rail, the offline page and band, the repository note, pairing on both sides, host-only notes | 9 |
| `web.html`, `src/web/main.ts`, `src/web/client.ts`, `src/web/noise.ts`, `src/web/shims.ts`, `src/web/sw.ts`, `public/manifest.webmanifest`, `vite.config.ts` (`--mode web`), `package.json` (`build:web`) | the PWA: `mockIPC` (as `src/browser/main.ts`) answered by the client; plugin shims; pairing; push; app shell | 11 |
| `src/lib/useDropZone.ts` | HTML5 drops when not in Tauri | 11 |
| `.github/workflows/release-builds.yml` | `hersir-web-X.Y.Z.zip` in `SHA256SUMS`; a `deploy-web` job in the `pwa` environment | 11 |
| `src/styles/app.css`, `src/components/phone/*` | the phone layouts of 3.14 | 12 |

**The server** (`Yotech-AI/hersir-cloud`): `app/Http/Controllers/Api/{Pairing,RelayBlob,Push}Controller.php`, `routes/channels.php`, `app/Events/{DeviceOnline,DeviceRemoved,PairingJoined,PairingConfirmed}.php`, `config/reverb.php`, the four migrations, `app/Console/Commands/PruneRelay.php`, the R2 lifecycle rule for `b/`, `PROTOCOL.md`'s relay section (card 7).

#### Contracts

**The server's relay API** (card 7; same rules as 5.4: `/api/v1`, a device's Passport token, errors as there, times in ms, ids UUIDv7, blobs base64):

| Call | Body → answer |
|---|---|
| `POST /pairings` | (host) `{}` → 201 `{ pairing_id, expires_at }` (5 minutes; 402 `plan_required` without an active plan) |
| `POST /pairings/{id}/join` | (device) `{ device_id, name, kind: "desktop"\|"web", box_pub, sign_pub, mac }` → 202; the host hears `pairing.joined { pairing_id }`; 404 when expired or used |
| `GET /pairings/{id}` | (host) → `{ device_id, name, kind, box_pub, sign_pub, mac }` |
| `POST /pairings/{id}/confirm` | (host) `{ host_box_pub, host_sign_pub, mac, both_ways: boolean }` → 200; makes `device_pairs` (both ways when asked); the device hears `pairing.confirmed { pairing_id }` |
| `GET /pairings/{id}/confirmation` | (device) → `{ host_id, host_box_pub, host_sign_pub, mac }` |
| `DELETE /pairings/{id}` | → 204 (cancel) |
| `GET /pairs` | → `[{ host_id, device_id, created_at }]` (this device's) |
| `DELETE /pairs/{host_id}/{device_id}` | → 204; both hear `device.removed { id }` for the device, and Reverb refuses their channel from then on |
| `POST /relay/blobs` | `{ to, bytes }` (at most 104,857,600 plus 1% sealing) → 201 `{ blob_id, url, headers, expires_at }`; 413 `file_too_large`, 429 `slow_down` past 2 GB a day |
| `POST /relay/blobs/{id}/sent` | `{}` → 200 after a `HEAD` checks the size; the receiver may now download |
| `POST /relay/blobs/{id}/download` | (receiver) → `{ url, bytes, expires_at }` |
| `DELETE /relay/blobs/{id}` | (receiver) → 204 |
| `PUT /devices/me/push` | (PWA) `{ endpoint, keys: { p256dh, auth } }` → 204; `DELETE` → 204 |
| `POST /push` | (host) `{ to, body }` (`body` at most 2,048 bytes, sealed) → 202, or 409 `no_subscription`; at most one per 30 s per kind is the host's job, 100 a day the server's |

**Reverb** (Pusher protocol at `wss://cloud.hersir.dev/app/<key>`, authorised at `POST /broadcasting/auth` with the device's token):

- `private-account.<user id>`: `device.online { id }`, `device.offline { id }`, `device.removed { id }`, `pairing.joined { pairing_id }`, `pairing.confirmed { pairing_id }`, `entitlement.changed {}`.
- `presence-account.<user id>`: members `{ device_id, name, kind, remote_on }`.
- `private-remote.<host id>.<device id>`: client events only, `client-m` with `data` = `{ "s": "<session id>", "n": <counter>, "p": <part, from 0>, "of": <parts>, "d": "<base64>" }`, `d` at most 49,152 bytes before base64; the channel is allowed when the caller is that host or that device, both are the account's, `device_pairs` has them, neither is removed, and the host's `remote_on` is true.

**The session** (`PROTOCOL.md`; cards 8 and 11 implement it, each with the other's test vectors):

```
handshake   Noise_KK_25519_AESGCM_SHA256, prologue "hersir-remote/1" ‖ host_id ‖ device_id;
            the device starts. Payloads: device → { "v": 1, "app": "hersir-desktop 1.2.0" | "hersir-web 1.2.0",
            "scope_seen": "full" | "board" }; host → { "v": 1, "app": "hersir-desktop 1.2.0", "name": "PC",
            "scope": "full" | "board", "min_device": "1.2.0" }
messages    after the handshake, each a Noise transport message whose plaintext is JSON, deflated when over 1 KiB
            ({"z":1} first byte flag), at most 1 MiB; larger answers go as a blob
device → host
  { "t": "call", "id": 17, "cmd": "list_tasks", "args": { "filter": {} } }        args as the app's invoke sends them
  { "t": "watch", "events": ["rows-changed","runs-changed","run-events","chat-changed","chat-events",
                             "backups-changed","restore-changed","update-changed"] }
  { "t": "bye" }
host → device
  { "t": "ok", "id": 17, "v": <the command's answer> }
  { "t": "err", "id": 17, "message": "<the command's own sentence>", "kind": "refused" | "host_only" | "unknown" | "failed" }
  { "t": "blob", "id": 17, "blob_id": "<uuid>", "bytes": 123456, "sha256": "<hex>", "mime": "image/png" }
  { "t": "event", "name": "rows-changed", "payload": { "table": "tasks" } }
  { "t": "bye", "why": "removed" | "remote_off" | "quit" }
host-side commands a device may call besides 3.12's
  remote_directory {} → { "host": { "id", "name", "app_version" },
                          "projects": [{ "id", "key", "name", "color", "teamId", "repoUrl" }] }
  remote_take_upload { "blobId", "name", "bytes", "sha256" } → { "path": "remote:<upload id>/<name>" }
  remote_file { "fileId" } → a "blob" answer (what open_file opens on a host)
blobs       sealed with the session's export key (HKDF(handshake hash, "hersir-blob" ‖ blob_id)) in 4 MiB chunks
push body   "HP1" ‖ nonce (12) ‖ AES-256-GCM(push key, { "kind": "hold"|"waiting"|"leadAsks"|"leadAnswered"|"approval",
            "title", "body", "url": "#/task/<id>" | "#/chat/<id>", "host": "<host id>" })
            push key = HKDF-SHA256(X25519(host box, device box), "hersir-push-1" ‖ host_id ‖ device_id)
```

**Hersir's commands for the remote screens** (card 8 builds them, card 9 uses them), in `src/api.ts` with the Tauri command in the comment:

```ts
type RemoteScope = "full" | "board";
type RemoteDevice = { id: string; name: string; kind: "desktop" | "web"; scope: RemoteScope;
  connected: boolean; lastSeenAt: number | null; pairedAt: number; push: boolean };
type RemoteStatus = { on: boolean; keepAwake: boolean; signedIn: boolean; plan: boolean;
  devices: RemoteDevice[];                       // paired with this host
  problem: null | { kind: "signed_out" | "plan" | "offline" | "web_mismatch"; message: string } };
type Pairing = { pairingId: string; qr: string; code: string; expiresAt: number;   // qr: the https://app.hersir.dev/pair#… link
  joined: null | { deviceId: string; name: string; kind: "desktop" | "web" } };
type RemoteAction = { at: number; cmd: string; target: string | null };
type HostState = "this" | "connected" | "connecting" | "offline" | "remote_off" | "old_version" | "removed";
type Host = { id: string; name: string; state: HostState; offlineSince: number | null; inbox: number;
  projects: { id: string; key: string; name: string; color: string | null; repoUrl: string | null }[];   // the directory
  message: string | null };                      // the offline page's sentence
type RepoElsewhere = { hostId: string; hostName: string; hostState: HostState; offlineSince: number | null;
  projectId: string; key: string; name: string };

remoteStatus(): Promise<RemoteStatus>                                    // remote_status
remoteSet(s: { on?: boolean; keepAwake?: boolean }): Promise<RemoteStatus>   // remote_set
remotePairStart(): Promise<Pairing>                                      // remote_pair_start (host)
remotePairAnswer(pairingId: string, add: boolean, scope: RemoteScope): Promise<RemoteStatus>  // remote_pair_answer
remotePairCancel(pairingId: string): Promise<void>                       // remote_pair_cancel
remoteRemoveDevice(id: string): Promise<RemoteStatus>                    // remote_remove_device
remoteDisconnectAll(): Promise<void>                                     // remote_disconnect_all
remoteDeviceLog(id: string): Promise<RemoteAction[]>                     // remote_device_log
hosts(): Promise<Host[]>                                                 // hosts: this computer first
hostPair(code: string): Promise<Host[]>                                  // host_pair (device; the HP- code)
hostSelect(hostId: string): Promise<void>                                // host_select: the window's workspace
hostCheckAgain(hostId: string): Promise<Host>                            // host_check_again
hostNotifyWhenBack(hostId: string, on: boolean): Promise<void>          // host_notify_when_back
repoElsewhere(repoUrl: string): Promise<RepoElsewhere[]>                 // repo_elsewhere (A1), from the directories
onRemoteChanged(f: (s: RemoteStatus) => void)                            // event "remote-changed"
onHostsChanged(f: (h: Host[]) => void)                                   // event "hosts-changed"
```

Refusals are sentences: "Turn on *Use from other devices* first.", "That code has expired: show a new one on PC.", "This device was removed from PC.", "PC is offline.", "The project's folder and setup commands can only be changed on PC.", "Files from another device can be up to 100 MB.".

**The PWA's client** (card 11, `src/web/client.ts`), so card 12 and later screens can rely on it: `connect(hostId)`, `call(hostId, cmd, args)`, `on(hostId, event, f)`, `upload(hostId, file: File): Promise<string>` (the `remote:` path), `blob(hostId, answer): Promise<Blob>`; `mockIPC` routes every `invoke` of the page to `call` for the selected host and answers the plugin commands itself (`plugin:opener|open_url` → `window.open`; `plugin:dialog|open` → a file input whose files become `upload:` tokens the client uploads before the call; `plugin:clipboard-manager|read_text` → `navigator.clipboard.readText`; `plugin:window|*` → nothing).

#### Security and privacy

3.13, plus: the host checks every incoming call's command against the allowlist and the device's scope before parsing its arguments; a session ends after 12 hours or when the device's token is revoked; keys are wiped from memory after use (`zeroize`) on the host and the MacBook; the PWA's keys are non-extractable; nothing a device sends is logged on the host except `remote_log`'s command names and targets; the server's logs hold ids and sizes only.

#### How it is tested

- **The session**: Noise test vectors on both sides; a device and a host in one test with an in-memory relay: a changed byte, a replayed or reordered frame, a removed device, an expired pairing and a wrong MAC all fail; the PWA's client against the Rust host through a local Reverb.
- **The allowlist**: a test lists every `#[tauri::command]` and fails on one that is in neither the allowlist nor the host-only list (as Round 2's sync list test would have); each of 3.12's refusals has a test, `save_project` and `save_settings` field by field.
- **Two Hersirs** with their own data folders (`HERSIR_DATA_DIR`) and fake keychains (`HERSIR_FAKE_KEYCHAIN`) against the server run locally (`php artisan serve`, `reverb:start`, an S3 fake): pair, open a card, Run, Stop, Continue, approve a call, add a file from the device, open a screenshot, remove the device mid-run, switch remote off, quit the host (the device's offline page shows), and A1's note.
- **The PWA**: `scripts/ui-test.sh`-style runs in Chromium and WebKit (Playwright) against the same local set-up; iOS by hand on your iPhone (install, push, pairing by camera).
- **The server** (Pest): every relay call and its errors; channel authorisation (another account's channel, an unpaired pair, a removed device, remote off); blob size checks; push limits; pairing expiry.

#### What is left out

Projects in the cloud (A4); any copy of a host's data on a device (A3); queued writes for a host that is off; a host vouching for a device to your other hosts (question 25); changing an agent's non-widening fields remotely; the terminal and the editor remotely; native iOS and Android apps; sharing a host with another person (teams).

## 6. Proposed cards

Twelve cards in four features, each one pull request in one repository: **the app** (this repository, open source) or **`Yotech-AI/hersir-cloud`** (private, project HC). Sizes in agent-days [I]. Card numbers are Round 3's; how they map to Round 2's is at the end of this section.

**At the same time**: cards 1, 2, 4 and 8 can start together (4 builds against a fake server, 8 against a local fake relay; both against 5.4's and 5.8's contracts). Then 3 and 7 (after 2; both in the server repository, in different files, so they can run together), 5 (after 1 and 4), 6 (after 2), 9 (after 1 and 8), 10 (after 1), 11 (after 8, and 7 for its end-to-end test), 12 (after 10 and 11). **The MacBook's path**: 2, 7, 8, 9. **The phone's**: 10, 11, 12.

### The account and Cloud Backup (cards 1 to 6)

**1. Design: Hersir Cloud on the desktop and the web, and Hersir on your other computers**
- Repository: the app (`design/hersir-cloud.md` and its screens in Figma, as `design/islands/` does). Column: Design. Role: design. Builds on: none. Size: 3 to 4.
- Builds: every state of 5.1 that still holds and of 5.8's desktop screens. The Hersir Cloud tab: signed out, signing in, the recovery code and its warning, signed in, a backup running, a failure, the quota full, a lapsed plan, *Make a new recovery code*. *Restore from Hersir Cloud* (first screen and Settings). *Use from other devices*: off, on with no devices, devices connected and not, *Add a device* with the QR code and code, a device joining and the question of 3.13 step 3, *Last actions*, *Remove…*. On a device: the hosts' rail (one host, two hosts, a host connecting, offline, remote off, an old version, removed), the offline page (3.11), the band of a host that went offline, *Add a computer*, the repository note in the project form (A1), "The Editor works on PC itself" and "Change this on PC". The web pages: create an account, check your email, set up two-factor authentication, the account page (with *Your computers and devices*), the desktop sign-in's done page.
- Acceptance:
  - [ ] every state drawn, in dark and light, with the final texts; decision 2's warning and 3.11's offline and repository texts word for word
  - [ ] only existing components and tokens (5.1, 5.8); the rail on the background, as the islands' rules say; gold only where you are needed
  - [ ] each screen names its component and the commands of 5.4 and 5.8 it shows
  - [ ] the web pages say which starter-kit views they change and which of Hersir's tokens they take

**2. Hersir Cloud server, first card: the app, accounts with required 2FA, computers and their tokens**
- Repository: `Yotech-AI/hersir-cloud`. Column: To do. Role: backend. Builds on: **your first commit** (3.10, row 0: `laravel new hersir-cloud --livewire --pest --database=sqlite --npm --git --branch=main`, pushed to `git@github.com:Yotech-AI/hersir-cloud.git` as the installer made it) and your set-up (3.10 rows 1 to 6 and 8). Size: 3 to 4.
- Builds: on the starter kit's commit, Fortify with `RequireTwoFactor` on every web route, Passport's included; Passport 13 with the public client *Hersir desktop*, S256 only, the two well-known documents of 3.4, the exchange (`POST /devices`), a token per computer, Remove; the calls marked card 2 in 5.4 (`/devices` with `kind` and `remote_on`, `PUT /devices/me`, entitlements from `comps`); `hersir:comp`; SQLite in WAL mode with Litestream's config, the Forge deploy script and `RUNBOOK.md` with the restore drill; healthchecks pings; a minimal account page with *Your computers and devices* and Remove; `PROTOCOL.md` started.
- Acceptance:
  - [ ] the pull request starts from the installer's commit and changes only Hersir Cloud's own code
  - [ ] every call marked card 2 in 5.4, with its errors, covered by Pest tests
  - [ ] the sign-in `oauth.rs` makes works: a redirect to 127.0.0.1 on a random port with S256 is accepted; `plain` is refused; `resource` is ignored
  - [ ] no web route, `/oauth/authorize` included, opens without confirmed 2FA (a test lists the routes); a removed computer gets 401 at once
  - [ ] deployed by Forge at `cloud.hersir.dev`; one restore drill from Litestream done and written down

**3. Hersir Cloud server: keys and encrypted backup storage**
- Repository: `Yotech-AI/hersir-cloud`. Column: To do. Role: backend. Builds on: card 2. Size: 3 to 4.
- Builds: the calls marked card 3 in 5.4 (keys, objects with presigned R2 URLs and size checks, snapshots); retention (decision 8) and garbage collection; the quota; the nightly copy to the locked bucket.
- Acceptance:
  - [ ] every call marked card 3 in 5.4, with its errors, covered by Pest tests against an S3 fake
  - [ ] a computer can't reach another account's objects or snapshots
  - [ ] retention keeps every snapshot 7 days and the newest of each day to 90; garbage collection never deletes an object a kept snapshot uses (fake-clock tests)
  - [ ] objects over 4 MiB plus overhead are refused when a snapshot is registered

**4. Cloud Backup in the app: sign in, encrypt, upload and restore**
- Repository: the app. Column: To do. Role: backend. Builds on: cards 2 and 3's contract (can be built at the same time, against a fake server). Size: 5 to 7.
- Builds: the crate `gizai-cloud-client` (`api`, `keys` with the computer's keys, `chunks`, `backup`, `restore`, `entitlement`) and `src-tauri/src/cloud.rs`; sign-in through `oauth.rs` with the built-in client id, then the exchange; the account key, the recovery code (never stored) and the key id; the page-for-page copy, chunks, sealing and upload; daily, On quit and Back up now; *Restore from Hersir Cloud* into a format-1 folder and on through GA-218's restore; the Hersir Cloud tab's commands and event of 5.4; the entitlement's public key.
- Acceptance:
  - [ ] a backup made on one computer restores on a clean one (scratch data folders), with the device rule and this computer's settings kept
  - [ ] the second daily backup of an unchanged database uploads nothing but the manifest
  - [ ] a wrong recovery code is refused with a sentence; nothing unencrypted leaves the computer (a test reads what the fake server got); the recovery code is in no keychain entry, file or log
  - [ ] a lapsed plan stops uploads and still restores
  - [ ] `cargo test --workspace`, `npm test`, `npm run build` pass on Linux, macOS and Windows CI

**5. Settings → Hersir Cloud**
- Repository: the app. Column: To do. Role: frontend. Builds on: cards 1 and 4. Size: 3 to 4.
- Builds: `CloudSettings.tsx`, the tab, *Restore from Hersir Cloud* on the first screen, every Cloud Backup state of card 1's design, on the commands of 5.4. (*Use from other devices* in the same tab is card 9's.)
- Acceptance:
  - [ ] each state of card 1 shows from the matching `CloudStatus` and `CloudComputer` (component tests)
  - [ ] the recovery code and its warning can't be skipped before the first backup
  - [ ] restore shows GA-218's steps and Restart now; `scripts/ui-test.sh` covers sign-in to restore against the local server

**6. Stripe with Cashier, and the account page**
- Repository: `Yotech-AI/hersir-cloud`. Column: To do. Role: backend. Builds on: cards 2 and 3 (and card 1's web pages). Size: 3 to 4. Needed before the first customer, not for your own use.
- Builds: Cashier 16 with Checkout (monthly and yearly, `Cashier::calculateTaxes()`, VAT numbers, the billing country), the customer portal, the webhook with `stripe_events`, the plan worked out from Cashier into the signed entitlement (`features: ["backup","remote"]`) with `entitlement.changed`; the account page of 5.1 (plan, Subscribe, Manage billing, storage, backups, computers and devices, 2FA, password, Withdraw, Delete account); the lapse emails and the deletion after 60 days; the countries question 13 decides, refused before Checkout opens.
- Acceptance:
  - [ ] a sandbox subscription turns the plan on in the app within a minute; a cancelled one stays active to the period's end, then lapses
  - [ ] webhooks are checked by signature and handled once each, in any order (Stripe's fixtures)
  - [ ] in the sandbox, Stripe Tax charges Spanish VAT to a Spanish consumer and reverse-charges an EU business with a valid VAT number
  - [ ] Withdraw and Delete account work, with their emails

### Hersir on your other computers: Remote (cards 7 to 9)

**7. Hersir Cloud server: the relay, pairing and Web Push**
- Repository: `Yotech-AI/hersir-cloud`. Column: To do. Role: backend. Builds on: card 2. Size: 3 to 4.
- Builds: Reverb on Forge (the *Laravel Reverb* switch, `ext-uv`, `nofile` raised) with 3.15's settings; 5.8's channels and events with their rules in `routes/channels.php`; presence; the relay API of 5.8 (pairings, pairs, relay blobs with size checks and the R2 lifecycle rule, push subscriptions, `POST /push` with `laravel-notification-channels/webpush` and VAPID keys); `device.online`, `device.offline`, `device.removed`; `relay:prune`; the relay's section of `PROTOCOL.md` (5.8's session, word for word).
- Acceptance:
  - [ ] every relay call of 5.8, with its errors, covered by Pest tests
  - [ ] a client can join `private-remote.<host>.<device>` only as that host or device, when they are paired, the account's, not removed, and the host's `remote_on` is true; client events pass between the two, and Reverb never stores them
  - [ ] removing a device revokes its token and pairs at once, and both sides hear `device.removed`
  - [ ] a blob over 100 MB is refused before the receiver gets a URL; blobs go after 24 hours
  - [ ] a test push reaches a browser's subscription (Playwright with Chromium); the server never sees a push's plaintext (it is sealed by the sender)

**8. The remote host and the desktop's forwarding**
- Repository: the app. Column: To do. Role: backend. Builds on: card 2's contract and card 7's (5.8; can start at once against a local fake relay). Size: 6 to 8.
- Builds: `src-tauri/src/remote/` of 5.8: the switch and keep-awake; the Pusher-protocol client on `tungstenite`; pairing with its MACs (host and device, both ways for two desktops); `Noise_KK_25519_AESGCM_SHA256` with `snow`; the allowlist `remote_commands!` with 3.12's groups, the two forms' field checks and the device's scope; uploads, `remote:` paths, blobs out and `remote_file`; `remote_directory`; event forwarding (eight events, `run-events` and `chat-events` at 250 ms); pushes through the `Desktop` sink; the desktop as a device (`forward.rs`: the app-level invoke handler routes the selected host's commands, uploads local paths, answers from blobs, emits the host's events); `remote_log`; migration `0023_remote.sql`; the commands and events of 5.8 for the screens; `PROTOCOL.md`'s test vectors.
- Acceptance:
  - [ ] two Hersirs with their own data folders and fake keychains, against the server run locally: pair both ways; on the device, list a host's cards, Run, Stop, Continue, comment, approve and discard a waiting call, add a file and open a screenshot; each happens on the host as if clicked there
  - [ ] a command not on the allowlist, a host-only command, a widening field of `save_project` or `save_settings`, a path that isn't `remote:`, and Run from a *Board and chat only* device are refused with their sentences (a test per case)
  - [ ] a test lists every `#[tauri::command]` and fails on one in neither list
  - [ ] a changed byte, a replayed frame, a removed device, remote switched off and an expired pairing end the session and run nothing (tests with a relay that tampers)
  - [ ] the Tauri version is pinned to its minor version, and a test covers the forwarding handler (Tauri's `Invoke` is unstable, S-R10)
  - [ ] `cargo test --workspace`, `npm test` and `npm run build` pass on Linux, macOS and Windows CI

**9. Hersir on your other computers, on screen: the hosts' rail, pairing and the host's controls**
- Repository: the app. Column: To do. Role: frontend. Builds on: cards 1 and 8. Size: 3 to 4.
- Builds: 5.8's desktop screens on its commands: `HostRail.tsx` and the window switching by host (`hersir.host` beside the route); `HostOffline.tsx` with every host state and the band of a host that went offline (A2); *Add a computer* and pairing by code; *Use from other devices* in Settings → Hersir Cloud with *Add a device* (QR and code), the join question with *What it may do*, *Last actions* and *Remove…*; the connected line on the host's sidebar; the repository note in `ProjectForm.tsx` from `repoElsewhere` (A1); the host-only notes on the Editor, Terminal, Settings and the agent form.
- Acceptance:
  - [ ] each state of card 1's design shows from `RemoteStatus`, `Host` and `RepoElsewhere` (component tests)
  - [ ] a host that is off keeps its tile and projects, grayed with the offline icon, and a click shows the offline page, never its data
  - [ ] adding a folder whose repository is a project on another host shows the note with that host and project, and still creates the project
  - [ ] `scripts/ui-test.sh` covers pairing two Hersirs, a Run from the device, and the host quitting while the device shows its board

### The PWA (cards 10 to 12)

**10. Design: Hersir on the phone**
- Repository: the app (`design/hersir-phone.md` and its frames in Figma). Column: Design. Role: design. Builds on: card 1. Size: 2 to 3.
- Builds: 3.14's phone layouts (the host switcher and menu, the bottom bar, the board one column at a time, a card with its tabs and a run's output with Stop and Continue, the chat with Approve and Discard, the Inbox) and the PWA's own screens (sign in, pair with a computer, *Add to Home Screen* on iOS, allow notifications, no connection, a host that is off), at 375 and 430 px, in dark and light; the notifications' texts for each kind.
- Acceptance:
  - [ ] every layout and state drawn with the final texts, from `design/tokens.json`, with touch targets of 44 px
  - [ ] each frame names the desktop component it adapts and what changes, so card 12 can build it in CSS and a few phone components

**11. The PWA: its web build, client, pairing, install and Web Push**
- Repository: the app. Column: To do. Role: frontend. Builds on: card 8's contract (5.8), and card 7 deployed for its end-to-end test. Size: 4 to 6.
- Builds: `npm run build:web` (`web.html`, `src/web/`) on the screens as they are; the client of 5.8 (Noise over WebCrypto with the published test vectors, Reverb, uploads and blobs) behind `mockIPC`; the plugin shims; HTML5 drops in `useDropZone.ts`; sign-in to Hersir Cloud (Passport with PKCE and an `https://app.hersir.dev` redirect) and the device's registration; pairing from the QR link; the manifest, icons and service worker (app shell; push: open the sealed body, always show a notification); persistent storage; the strict CSP; the release workflow's `hersir-web-X.Y.Z.zip` in `SHA256SUMS` and its deploy in the `pwa` environment; the host's daily check of `app.hersir.dev` against the signed release.
- Acceptance:
  - [ ] in Chromium and WebKit (Playwright) against the local set-up: sign in, pair, open a host's board, Run, approve a call, upload a photo to a card, open a screenshot
  - [ ] the device's keys are non-extractable `CryptoKey`s; nothing readable goes to the server (a test reads what the fake relay got)
  - [ ] a push sent while the PWA is closed shows the host's text, and a tap opens the card
  - [ ] the deploy runs only from a signed release with your approval; a changed file on `app.hersir.dev` makes the host warn
  - [ ] installed on your iPhone from the Home Screen, with notifications (by hand)

**12. The PWA's phone layouts: the board, a card, the chat and the Inbox**
- Repository: the app. Column: To do. Role: frontend. Builds on: cards 10 and 11. Size: 3 to 4.
- Builds: card 10's layouts under 600 px in `app.css` and `src/components/phone/*`: the host switcher and menu, the bottom bar, the board's column tabs, the card's tabs and its run's output, the chat with its waiting calls, the Inbox; "Open this on a computer" elsewhere.
- Acceptance:
  - [ ] each place of card 10 matches its frames at 375 and 430 px (screenshots in the pull request)
  - [ ] the desktop layouts don't change (the existing UI tests pass)
  - [ ] Run, Stop, Continue, Approve and Discard can be done with a thumb, without drag and drop

### Replaced: Round 2's Multiple devices and chat cards

| Round 2's card | Round 3 |
|---|---|
| 1 Design (with Multiple devices and the chat) | **1**, with remote screens in place of sync and chat states |
| 2 Server: accounts, computers, keys, backup storage | **2** (accounts, computers) and **3** (keys, storage) |
| 3 Cloud Backup in the app | **4** |
| 4 Settings → Hersir Cloud | **5** |
| 5 Stripe and the account page | **6** |
| 6 Sync engine in the core | **replaced** (C4) |
| 7 Server: sync, claims, commands, Reverb | **replaced**; Reverb goes to **7**, the relay |
| 8 Sync in the app, with claims | **replaced**; its Pusher-protocol client goes to **8** |
| 9 Multiple devices on screen | **replaced** by **9** |
| 10 The Chief's chat from another computer (commands) | **replaced** by **8**: the host's own chat commands, forwarded |
| 11 The Chief's chat on screen | **replaced** by **9** and **12** |
| new | **7**, **10**, **11**, **12** |

Their full text is in this document at commit `a65f211`.

## 7. Open questions for Jefsev

### Round 1's questions 1 to 12: closed by the decisions of 17 November

| # | Round 1's question | Closed by |
|---|---|---|
| 1 | End-to-end for both? | decision 1: yes |
| 2 | A recovery code only? | decision 2: yes, with a clear warning |
| 3 | Hetzner, Rust, SQLite with Litestream? | decisions 3 and 4: SQLite with Litestream, in a Laravel app on Forge, R2 EU |
| 4 | Zitadel Cloud? | decision 4: no; Fortify and Passport |
| 5 | No magic link at launch? | decision 5: email, password and TOTP required; no magic links, SMS or email codes |
| 6 | No automatic starts offline? | decision 6: yes; claims need the cloud |
| 7 | Sell with Cloud Backup first? | decision 7: yes |
| 8 | 7 days, then daily to 90? | decision 8: yes |
| 9 | Paddle? | decision 9: no; Stripe with Cashier, Checkout and Tax |
| 10 | The domain? | decision 10: `cloud.hersir.dev`, for now |
| 11 | A private server repository? | decision 11: yes |
| 12 | The Chief's chat read-only elsewhere? | decision 12: no; writable on every computer, and later in a PWA |

### Open questions 13 to 27 (Round 2's that still stand, redone where Round 3 changed them, and Round 3's new ones)

Jefsev hasn't answered 13 to 20 yet; 13 to 16 and 18 stand as Round 2 asked them, 17, 19 and 20 are redone for Remote, and 21 to 27 are new. Round 3's first draft had a question on a read-only copy for while a host is off; Jefsev answered it (A3: no copies), so it is gone.

13. **Where to sell at launch.** Consumers in the EU and businesses anywhere; UK consumers from the day the S.L. registers for UK VAT; other countries' consumers not yet, since some tax a foreign seller from the first sale? *Recommended: yes*, with the account page refusing other consumer countries before Checkout opens. If you'd rather sell everywhere at once, Managed Payments only for those countries is possible per Checkout Session (untested).
14. **VERI*FACTU by 1 January 2027.** Stripe's invoices are the S.L.'s invoices, and Stripe doesn't claim they comply. *Recommended*: ask your gestor now which route (Stripe with Invopop, or the SII); the S.L.'s other invoices need it from the same day anyway. Card 6 waits for the answer only for its invoice settings.
15. **A copy of the server's database outside Cloudflare.** Every copy (Litestream, the nightly one) is at Cloudflare. *Recommended: yes*, a nightly copy to a Hetzner Storage Box (a few euros a month), before the first customer.
16. **Small seller or the OSS from the start.** Spanish VAT on EU consumers until €10,000 a year, then the OSS; or the OSS at once (binding for two years). *Recommended*: small seller at launch, which Stripe Tax supports, unless the S.L. already sells electronic services to EU consumers in another business (they count towards the €10,000); the gestor decides.
17. **The PWA's key** (redone). With Remote, the PWA holds **its own device key pair**, made in the browser as non-extractable WebCrypto keys and registered on each host by pairing from a QR code; it never holds the account key or a chat key, and can't read Cloud Backup's copies (3.13). *Recommended: yes.*
18. **Changing the account key** when a lost or stolen computer is removed: v1 or later? It now concerns Cloud Backup only. *Recommended: later*: removal already stops the computer from reaching Hersir Cloud and your hosts, a stolen laptop holds its own data anyway, and reading the cloud copies would also need a breach of Hersir Cloud.
19. **A Chief per host** (redone; Round 2 asked about agents' questions on another computer). Each host keeps its own Chief, which chats, checks the board and answers its agents' questions for that host's projects only; its chat shows in that host's workspace on every device (3.11). *Recommended: yes*; one Chief across hosts would need to reach every host's projects and is a later idea.
20. **Writes to a host that is off** (redone; Round 2 asked how long a command waits). *Recommended: refused at once, never queued*: "PC is offline". A Run or an approval that waited and ran hours later would surprise you, and Jefsev's answer A3 keeps nothing on the devices.
21. **Keeping a host awake.** Remote works only while the host is on and awake. Hersir could keep the computer from sleeping while *Use from other devices* is on (the OS's "keep awake" request, as video players make). *Recommended: yes, on by default on a computer without a battery and off on a laptop*, with the switch in Settings → Hersir Cloud; waking a sleeping computer through the internet is out of scope.
22. **Where the PWA is served.** *Recommended: `app.hersir.dev`, its own origin, as static files on Cloudflare (Workers static assets, free), deployed only from a signed release with your approval* (3.13). Alternatives: GitHub Pages (also static and auditable; Microsoft as a new party) or the Forge server (one company less, but a hole in the Laravel app could then serve the PWA's code).
23. **What a paired device may do.** At pairing you choose *Everything a remote device may do* (3.12) or *Board and chat only* (no Run, Stop, Continue or approvals). *Recommended: Everything by default*, since you want Run and approvals from the phone, with *Board and chat only* for a device you trust less; no confirmation on the host per command.
24. **The size of a file from another device.** *Recommended: 100 MB per file* through R2 (screenshots, photos, logs, small archives); the desktop's own limit of 1 GB stays for files added on the host itself.
25. **Pairing a device with each host.** *Recommended: yes, one QR code per host in v1*; a host vouching for a new device to your other hosts can come later if you have more than two.
26. **Plans.** *Recommended: Hersir on your other devices and the PWA in Personal (€5 a month, €48 a year), with Cloud Backup*; using a host from its own screen stays free; no separate plan (3.16).
27. **What a push says.** *Recommended: the host's own text, sealed so that only your phone can read it* ("PC: the Chief waits for your OK: update_task GA-310"); the alternative, a fixed text by kind ("Hersir: something needs you"), shows less on the lock screen but tells the server the kind.

## 8. Changes proposed to `docs/PAID-FEATURES.md` (not made)

1. **Line 18 and P1**: Personal holds Cloud Backup and **Hersir on your other devices**: the MacBook's Hersir and the PWA (the whole Hersir: board, cards, runs, the Chief's chat with its approvals, the Inbox) working with each project on its host (Round 3); using a host from its own screen stays free.
2. **Lines 65-77 (How it fits together)**: `cloud.hersir.dev`, a Laravel 13 app (Fortify, Passport, Cashier, Reverb; SQLite with Litestream; R2 EU), instead of `api.hersir.dev (Rust, axum)`; **the relay** is Reverb's client events between a host and its paired devices, end to end encrypted (Noise), with files through R2; the PWA at `app.hersir.dev`, built from this repository.
3. **Line 80**: the keychain (service `hersir`) holds this computer's Passport token, the account key and the computer's own keys; never the recovery code.
4. **Lines 82-87 (encryption)**: end-to-end for backups and sync, decided; a random account key with a recovery code only, no passphrase (decision 2); objects named by a keyed hash; computer keys and a roster; why (3.2).
5. **Line 108**: the server is a Laravel 13 app in the private repository `hersir-cloud`, on a Forge-managed Hetzner server in Germany; SQLite with Litestream (decision 3); R2 in its EU jurisdiction for the objects and the database's copies; Cloudflare in DNS only.
6. **Line 110**: email and password with TOTP required (Fortify; decision 5); no magic links, passkeys, Google or GitHub; the desktop signs in through `oauth.rs` against Passport, then gets a token per computer.
7. **Lines 111-115**: computers with their public keys, removed at once; backups as page-for-page chunks; every backup kept 7 days, then one a day to 90 (decision 8); billing webhooks from Stripe through Cashier.
8. **Line 120**: no "back up on change": daily, on quit and by hand.
9. **Lines 126-140 (Phase 2)**, rewritten for Round 3: **"Multiple devices" is Remote**: a project lives on the device it was made on (its host); every device shows the projects of all your hosts, one workspace per host in a rail, each answered by its host; nothing is copied; a host that is off shows its projects grayed out; a repository added on a second host becomes a separate project, with a note; a Chief, agents, clients, people and memory per host; pairing per device and host, with a QR code; what a remote device may do (3.12). **Replaced**: the trigger-written `sync_log`, the HLC, numbers settled in the server's order, home computers and cloud claims; decision 6's "claims need the cloud" falls away, since a host's agents only ever run on it.
10. **A new phase after Phase 2**: the PWA (installable on iOS and Android, Web Push for holds, questions and approvals) with its phone layouts. **Later, out of scope for a while**: projects that run in the cloud (A4), as one more host.
11. **Line 167 and P2**: Stripe direct with Billing and Tax (decision 9); Managed Payments weighed (3.5% more, sold through Link, LLC; open to a Spanish S.L.); Paddle replaced.
12. **Line 171 and P3**: done: NEXO IV GRUPO, S.L. (Spain); the VAT duties of 3.5 (small seller or OSS, ROI, 349, VERI*FACTU by 1 January 2027, the UK from the first consumer).
13. **Lines 172-178**: the Spanish legal list of 3.9, with Cloudflare (R2 and now the PWA's static hosting) and Laravel as US sub-processors, and Apple's and Google's push services named in the privacy policy (they carry sealed pushes only).
14. **Line 187**: the cost table of 3.3 with the relay of 3.15 (about €22, €31 and €148 a month at 10, 100 and 1,000 customers), Forge included.
15. **Decisions table**: P2 and P3 as above; add P10 (the Laravel stack, decision 4), P11 (sign-in with TOTP required, decision 5), P12 (agents run only on their host, decision 6 as Round 3 leaves it), P13 (retention, decision 8), P14 (`cloud.hersir.dev`, decision 10), P15 (the Chief's chat on every device, decision 12, met by Remote), P16 (Remote: projects live on their host, nothing is copied, C1 to C5 and A1 to A4), P17 (the PWA's own device key by pairing, question 17).
16. **Phases 1 and 2's sizes**: the twelve cards of section 6, about 41 to 56 agent-days, of which Remote and the PWA (cards 7 to 12) about 21 to 29 (Round 2's eleven were 40 to 60, with sync).
17. **Line 84 (end-to-end by default)**: add the remote session: Noise KK between a device and its host, so the server relays what it can't read; the PWA's code risk and its limits (3.13).

## 9. Sources

Round 2's sources were read on **2026-11-17** by this card's research (the R&D Agent and its helpers); Round 1's, read on **2026-11-09**, are kept for what still holds. *Search result only* where the page itself wasn't opened.

**Round 3: the PWA, Remote and the relay** (read 2026-11-20 by this card's research and its helpers; quotes kept short)
- S-R1 WebKit, Web Push for web apps on iOS and iPadOS (16 Feb 2023): https://webkit.org/blog/13878/web-push-for-web-apps-on-ios-and-ipados/ ; Meet Web Push (7 Jun 2022, `userVisibleOnly`, revoked subscriptions): https://webkit.org/blog/12945/meet-web-push/
- S-R2 WebKit, Meet Declarative Web Push (27 Mar 2025; iOS 18.4, no service worker needed): https://webkit.org/blog/16535/meet-declarative-web-push/
- S-R3 WebKit, Updates to storage policy (10 Aug 2023; 60% per origin, Home Screen web apps the same, `persist()` by heuristics): https://webkit.org/blog/14403/updates-to-storage-policy/ ; Tracking prevention (the 7-day cap and the Home Screen exemption; undated): https://webkit.org/tracking-prevention/
- S-R4 WebKit features in Safari 26.0 (15 Sep 2025; "every website added to the Home Screen opens as a web app"): https://webkit.org/blog/17333/webkit-features-in-safari-26-0/ ; Safari 27.0 (17 Sep 2026; read in part, no Home Screen or push change seen): https://webkit.org/blog/18325/webkit-features-for-safari-27-0/
- S-R5 web.dev, install criteria: https://web.dev/articles/install-criteria ; push FAQ and overview: https://web.dev/articles/push-notifications-faq , https://web.dev/articles/push-notifications-overview ; RFC 8291 (4,096-octet payloads): https://www.rfc-editor.org/rfc/rfc8291 ; RFC 8030: https://www.rfc-editor.org/rfc/rfc8030
- S-R6 Laravel Reverb: https://laravel.com/docs/13.x/reverb ; its source on `main`: https://github.com/laravel/reverb/blob/main/config/reverb.php , `src/Protocols/Pusher/ClientEvent.php`, `Server.php`, `Channels/Channel.php`, `src/Application.php`, `src/Servers/Reverb/Connection.php` (client events from members by default, never echoed to the sender, `data` an object; `max_message_size` 10,000; rate limiting off by default; `max_connections` unset); Pusher's client events: https://pusher.com/docs/channels/using_channels/events/#triggering-client-events
- S-R7 Service Workers (Editor's Draft, 17 Nov 2026; the 24-hour stale rule, soft update): https://w3c.github.io/ServiceWorker/ ; web.dev, the service worker lifecycle: https://web.dev/articles/service-worker-lifecycle
- S-R8 WEBCAT (alpha, Firefox extension): https://github.com/freedomofpress/webcat ; Meta's Code Verify (10 Mar 2022): https://engineering.fb.com/2022/03/10/security/code-verify/ ; Cloudflare's part: https://blog.cloudflare.com/cloudflare-verifies-code-whatsapp-web-serves-users/ ; Isolated Web Apps (explainer): https://github.com/WICG/isolated-web-apps ; MDN, Subresource Integrity: https://developer.mozilla.org/en-US/docs/Web/Security/Defenses/Subresource_Integrity ; Chrome's status page for IWAs didn't render (not verified)
- S-R9 Laravel 13 installation and starter kits ("All starter kits use Laravel Fortify", 2FA on by default, Livewire 4 with Flux UI, SQLite by default): https://laravel.com/docs/13.x/installation , https://laravel.com/docs/13.x/starter-kits ; the installer's flags (`master`): https://github.com/laravel/installer/blob/master/src/NewCommand.php
- S-R10 Tauri 2.12.7 on docs.rs (18 Nov 2026): `Listener::listen_any` https://docs.rs/tauri/latest/tauri/trait.Listener.html ; `Builder::invoke_handler` https://docs.rs/tauri/latest/tauri/struct.Builder.html ; `ipc::Invoke` ("explicitly NOT stable"), `InvokeMessage::command`/`payload`, `InvokeResolver::resolve`/`reject` https://docs.rs/tauri/latest/tauri/ipc/struct.Invoke.html ; `Manager::state` https://docs.rs/tauri/latest/tauri/trait.Manager.html ; `Webview::on_message` with `InvokeRequest` ("NOT part of the public stable API") https://docs.rs/tauri/latest/tauri/webview/struct.InvokeRequest.html ; mocks (`shouldMockEvents`): https://v2.tauri.app/reference/javascript/api/namespacemocks/ ; camelCase arguments: https://v2.tauri.app/develop/calling-rust/ . Not verified: whether `plugin:` commands bypass the app's invoke handler (card 8 checks it first)
- S-R11 MDN browser-compat data for SubtleCrypto (Ed25519: Chrome 137, Firefox 129, Safari 17; X25519: Chrome 133, Firefox 130, Safari 17): https://raw.githubusercontent.com/mdn/browser-compat-data/main/api/SubtleCrypto.json ; MDN `CryptoKey` (`extractable`): https://developer.mozilla.org/en-US/docs/Web/API/CryptoKey ; `sign`, `deriveBits`. Not verified: storing non-extractable keys in IndexedDB and using them from a service worker (the Web Crypto spec, to check in card 11)
- S-R12 `minishlink/web-push` 11.0.0 (MIT, 23 Jul 2026, PHP 8.2+): https://packagist.org/packages/minishlink/web-push ; `laravel-notification-channels/webpush` 13.0.1 (MIT, 8 Sep 2026; Laravel ^12 or ^13.13; VAPID): https://packagist.org/packages/laravel-notification-channels/webpush
- S-R13 Cloudflare Workers static assets (free, 20,000 files, 25 MiB each; SPA routing) and Pages: https://developers.cloudflare.com/workers/static-assets/ , https://developers.cloudflare.com/workers/static-assets/billing-and-limitations/ , https://developers.cloudflare.com/pages/
- Not verified this round: Apple's DMA page no longer mentions Home Screen web apps in the EU (https://developer.apple.com/support/dma-and-apps-in-the-eu/); the Noise Protocol Framework and the `snow` crate were not re-read (card 8 checks the version); round-trip times from Spain to Germany are my estimate (a WonderNetwork search gave no Madrid to Germany figures).

**Laravel, Forge, Reverb, R2, Litestream, Hetzner** (2026-11-17)
- S-L1 Laravel 13 release notes: https://laravel.com/docs/releases ; laravel/framework: https://packagist.org/packages/laravel/framework
- S-L2 Fortify: https://laravel.com/docs/13.x/fortify ; its config stub: https://github.com/laravel/fortify/blob/1.x/stubs/fortify.php ; requiring 2FA with a middleware (a Laracasts thread, search result only)
- S-L3 Starter kits: https://laravel.com/docs/13.x/starter-kits
- S-L4 Passport: https://laravel.com/docs/master/passport ; releases: https://github.com/laravel/passport/releases ; its routes: https://github.com/laravel/passport/blob/13.x/routes/web.php
- S-L5 league/oauth2-server (`RedirectUriValidator`, `AuthCodeGrant`, `AbstractGrant`): https://github.com/thephpleague/oauth2-server/tree/master/src ; Laravel MCP's OAuth routes: https://laravel.com/docs/master/mcp , https://github.com/laravel/mcp
- S-L6 Cashier (Stripe): https://laravel.com/docs/13.x/billing ; releases: https://github.com/laravel/cashier-stripe/releases ; source: https://github.com/laravel/cashier-stripe/tree/16.x/src
- S-L7 Reverb: https://laravel.com/docs/13.x/reverb , https://github.com/laravel/reverb ; broadcasting: https://laravel.com/docs/13.x/broadcasting
- S-L8 Pusher crates on crates.io: https://crates.io/api/v1/crates?q=pusher
- S-L9 Laravel's filesystem (`temporaryUploadUrl`): https://laravel.com/docs/13.x/filesystem ; databases (SQLite): https://laravel.com/docs/13.x/database
- S-L10 Litestream: https://github.com/benbjohnson/litestream/releases , https://litestream.io/guides/s3-compatible/ , https://litestream.io/reference/restore/ , https://litestream.io/reference/config/ , https://litestream.io/guides/systemd/
- S-L11 R2 pricing: https://developers.cloudflare.com/r2/pricing/
- S-L12 R2 data location: https://developers.cloudflare.com/r2/reference/data-location/ ; Cloudflare's customer DPA (v6.4): https://www.cloudflare.com/cloudflare-customer-dpa/ ; its privacy policy: https://www.cloudflare.com/privacypolicy/
- S-L13 R2 presigned URLs: https://developers.cloudflare.com/r2/api/s3/presigned-urls/ ; its S3 API: https://developers.cloudflare.com/r2/api/s3/api/ ; bucket locks: https://developers.cloudflare.com/r2/buckets/bucket-locks/ ; lifecycles: https://developers.cloudflare.com/r2/buckets/object-lifecycles/
- S-L14 Hetzner backups (2026-11-09): https://docs.hetzner.com/cloud/servers/backups-snapshots/overview/
- S-L15 Forge's server providers: https://laravel.com/forge/docs/server-providers.md ; sites and Reverb: https://laravel.com/forge/docs/sites/laravel.md
- S-L16 Forge pricing: https://laravel.com/forge/pricing ; database backups: https://laravel.com/forge/docs/resources/database-backups.md ; monitoring: https://laravel.com/forge/docs/servers/monitoring.md ; heartbeats: https://laravel.com/forge/docs/resources/scheduler.md
- S-L17 Nightwatch pricing: https://laravel.com/nightwatch/pricing
- S-L18 (2026-11-09) healthchecks.io pricing: https://healthchecks.io/pricing/ ; Sentry pricing: https://sentry.io/pricing/
- S-L19 Hetzner Cloud prices: https://www.hetzner.com/cloud/cost-optimized/ , https://www.hetzner.com/cloud/regular-performance/

**Payments** (2026-11-17)
- S-P8 Managed Payments eligibility: https://docs.stripe.com/payments/managed-payments/eligibility
- S-P9 Stripe's prices in Spain: https://stripe.com/es/pricing , https://stripe.com/en-es/pricing
- S-P10 Managed Payments: https://stripe.com/en-es/managed-payments , https://docs.stripe.com/payments/managed-payments
- S-P11 How it works: https://docs.stripe.com/payments/managed-payments/how-it-works ; its pricing: https://support.stripe.com/questions/managed-payments-pricing
- S-P12 "Sold through Link, LLC": https://support.stripe.com/questions/why-is-sold-through-link-llc-the-legal-entity-on-my-invoice-for-stripe-managed-payments-fees
- S-P13 Checkout changes: https://docs.stripe.com/payments/managed-payments/update-checkout ; the changelog: https://docs.stripe.com/changelog/dahlia/2026-04-22/managed-payments
- S-P14 Payouts: https://support.stripe.com/questions/understand-managed-payments-payout-speed ; invoices: https://support.stripe.com/questions/which-invoices-will-i-receive-for-managed-payments-transactions ; tax compliance: https://docs.stripe.com/payments/managed-payments/tax-compliance
- S-P15 Stripe Tax pricing: https://stripe.com/en-es/tax/pricing
- S-P16 Stripe Tax filing: https://docs.stripe.com/tax/filing
- S-P17 Paddle: https://www.paddle.com/pricing ; https://www.paddle.com/help/manage/reporting/how-are-my-earnings-calculated ; https://www.paddle.com/help/manage/risk-prevention/understanding-chargebacks-with-paddle
- The USD rate (ECB, 11 Nov 2026): search result only

**Tax** (2026-11-17)
- S-T1 LIVA (Ley 37/1992), art. 73: https://www.boe.es/buscar/act.php?id=BOE-A-1992-28740
- S-T2 AEAT, VAT on electronic services: https://sede.agenciatributaria.gob.es/Sede/iva/iva-comercio-electronico/iva-telecomunicaciones-servicios-electronicos.html ; AEAT's VAT manual 2025, *régimen de la Unión*
- S-T3 The Commission's OSS pages: https://vat-one-stop-shop.ec.europa.eu/one-stop-shop/register-oss_en , https://vat-one-stop-shop.ec.europa.eu/one-stop-shop/declare-and-pay-oss_en ; the OSS Guide (July 2026 revision)
- S-T4 Implementing Regulation 282/2011, Art. 24b (the UK's copy of the EU text; EUR-Lex didn't load): https://www.legislation.gov.uk/eur/2011/282/article/24b
- S-T5 Your Europe, cross-border VAT: https://europa.eu/youreurope/business/taxation/vat/cross-border-vat/index_en.htm ; AEAT on the ROI: https://sede.agenciatributaria.gob.es/Sede/iva/iva-operaciones-comercio-exterior/identificacion-realizar-operaciones-otros-empresarios-ue.html ; Modelo 349 (AEAT leaflet, section 5.9)
- S-T6 RD 1619/2012 (invoicing): https://www.boe.es/buscar/act.php?id=BOE-A-2012-14696 ; RD 1007/2023 (VERI*FACTU): https://www.boe.es/buscar/act.php?id=BOE-A-2023-24840 ; AEAT's VERI*FACTU FAQ; Stripe on VERI*FACTU: https://stripe.com/resources/more/what-is-verifactu-spain
- S-T7 HMRC VAT Notice 700/1: https://www.gov.uk/guidance/who-should-register-for-vat-vat-notice-7001 ; the Swiss ESTV: https://www.estv.admin.ch ; Canada's GST/HST for the digital economy: https://www.canada.ca ; Norway, Australia, the US, Japan and Singapore: search results only; the zero-threshold countries: not verified
- S-T8 Stripe Tax monitoring: https://docs.stripe.com/tax/monitoring
- S-T9 Stripe Tax in the EU: https://docs.stripe.com/tax/supported-countries/european-union
- S-T10 Stripe Tax filing: https://docs.stripe.com/tax/filing

**Round 1's sources this document still cites** (2026-11-09; the full list is in section 9 of this document at commit `496f976`)
- S-E2 Apple, Advanced Data Protection: https://support.apple.com/en-us/102651
- S-E15 to S-E18 chacha20poly1305: https://crates.io/api/v1/crates/chacha20poly1305 , https://github.com/RustCrypto/AEADs/tree/master/chacha20poly1305 ; age: https://crates.io/api/v1/crates/age ; RFC 9106: https://www.rfc-editor.org/rfc/rfc9106.html
- S-E19 Court of Justice press release 107/25 (C-413/23 P): https://curia.europa.eu/site/upload/docs/application/pdf/2025-09/cp250107en.pdf
- S-B1 sqlite3_rsync: https://sqlite.org/rsync.html ; S-B2 restic's design: https://raw.githubusercontent.com/restic/restic/master/doc/design.rst
- S-B3 the HLC paper: https://cse.buffalo.edu/tech-reports/2014-04.pdf ; S-B4 Actual Budget: https://archive.jlongster.com/using-crdts-in-the-wild
- S-B5 Linear's sync engine: https://linear.app/now/scaling-the-linear-sync-engine ; S-B9: https://github.com/wzhudev/reverse-linear-sync-engine
- RFC 8252 (OAuth for native apps): https://www.rfc-editor.org/rfc/rfc8252.html
- The legal texts behind 3.9 (LSSI-CE, LOPDGDD, TRLGDCU on the BOE; the AEPD's tools; the Commission's standard clauses; the Data Act, DSA and NIS2 pages)

**In this repository, Round 3** (main at `723097f`, Hersir 0.19.0, read 2026-11-20 with `git show` and `git grep`): `src/api.ts`, `src/types.ts`, `src/router.ts`, `src/lib/{perf,share,useDropZone,drop,glass,terminals}.ts`, `src/browser/{main,mock}.ts`, `src/components/{FileDrop,UpdateSettings,Sidebar}.tsx`, `src/App.tsx`, `src/styles/app.css`, `vite.config.ts`, `package.json`, `src-tauri/src/{lib,commands,notifications,screenshots,terminal}.rs` and every `#[tauri::command]` in `src-tauri/src`, `src-tauri/src/tools/mod.rs`, `crates/gizai-core/src/{db,tasks,runs}.rs`, `crates/gizai-core/migrations/0001_init.sql`, `crates/gizai-agents/src/release_sig.rs`, `.github/workflows/release-builds.yml`, `design/README.md`, `design/islands/README.md`.

**In this repository, Round 2** (main at `99ce4e3`, read 2026-11-17): `docs/PAID-FEATURES.md`, `docs/backup-format.md`, `docs/features/chief.md`, `docs/agent-tools.md`, `README.md`, `crates/gizai-core/src/{db,tasks,runs,scope,settings,columns,workflow,chat,pending,questions}.rs`, `crates/gizai-core/migrations/{0001_init,0003_chat,0012_chat_runs_on,0017_pending_actions,0022_outside_marks}.sql`, `crates/gizai-agents/src/{oauth,secrets,release_sig}.rs`, `src-tauri/src/{lib,chat,pending}.rs`, `src/api.ts`, `src/router.ts`, `src/pages/SettingsPage.tsx`, `src-tauri/tauri.conf.json`, `Cargo.lock`.
