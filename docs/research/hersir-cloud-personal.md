# Hersir Cloud for one person: Cloud Backup, Multiple devices and the Chief's chat on every computer

Research for GA-67 by the R&D Agent. Round 1: 9 November 2026. **Round 2: 17 November 2026**, rewritten to Jefsev's twelve decisions of 17 November ([section 0](#0-jefsevs-decisions-17-november-2026)). No code was changed. It turns `docs/PAID-FEATURES.md` (Phase 1 and Phase 2) into build cards.

- **[V]** = verified: in this repository (a file and line), or on a primary source (listed in [Sources](#9-sources) with the date read). **[I]** = my judgement.
- **Replaced** marks a Round 1 recommendation that a decision of 17 November replaced. It stays only where it helps to see why.
- Code citations are checked against main at `99ce4e3` (Hersir 0.18.0, 17 Nov 2026). This branch could not merge main in this run (`git merge` was refused), so the files in the branch's own tree are older than some lines cited.

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

## 1. Summary

1. **Order** (decision 7): Cloud Backup for your own computers first, with a plan given by hand (cards 1 to 4); Stripe, the account page and the legal pack before the first sale (card 5); then Multiple devices (cards 6 to 9); then the Chief's chat on every computer (cards 10 and 11). Eleven cards, two repositories, about 38 to 50 agent-days.
2. **The server** is one Laravel 12 app, `hersir-cloud` (private), at `cloud.hersir.dev` on a Forge-managed Hetzner server in Germany: Fortify with TOTP required, Passport, Cashier, Reverb, SQLite in WAL mode streamed by Litestream to R2, and the customers' encrypted chunks uploaded straight to R2 (EU jurisdiction) with presigned URLs. About €30 a month for 10 or 100 customers, about €105 at 1,000 (3.3).
3. **Sign-in**: `oauth.rs` works against Passport as it is, given a fixed client id (it then skips registration); it already finds the server through RFC 9728 and RFC 8414 documents, which Hersir Cloud publishes. The first token is exchanged once for the computer's own Passport token, which the account page withdraws (3.4).
4. **Payments**: Stripe direct with Billing and Tax costs about 8% of the monthly price and 4% of the yearly one; Managed Payments (open to a Spanish S.L., +3.5%, sold through Link, LLC) about 12% and 7%. **Recommended against Managed Payments for now.** Selling directly, the S.L. charges Spanish VAT to EU consumers until EU sales to them pass €10,000 a year, then files the OSS quarterly; the UK needs registration from the first UK consumer (3.5).
5. **The Chief's chat everywhere**: a message written on another computer goes as an encrypted, signed command through Hersir Cloud to the Chief's computer, which answers there; the answer and its Approve and Discard reach every computer through sync, and an approval from any computer runs once, on the Chief's computer. Reverb rings the doorbell; data goes over plain HTTPS (3.8).
6. **Still true in the code** (main at 0.18.0): `changes` can't drive sync as it is, so sync gets a trigger-written log beside it (2.4, 3.7).
7. **New open questions** (13 to 20, section 7): the first is to sell only in the EU and the UK at launch.

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

## 3. The feature

### 3.1 What it should do

- **Cloud Backup**: "Encrypted backups of your data, kept off your computer. Restore them on a new one." Every backup kept 7 days, then one a day up to 90 (decision 8); a 10 GB quota; restore on a clean computer; a lapsed plan stops uploads, not restores (`docs/PAID-FEATURES.md:104-124`, P7).
- **Multiple devices**: "Your clients, projects and cards in sync on each of your computers." Every computer keeps a full local copy that works offline; agents run on their home computer, which claims a card in the cloud before its agent starts it; no agent starts by itself offline (decision 6).
- **The Chief's chat on every computer** (decision 12): write to the Chief from any computer; it answers on its own computer; the answer and its Approve and Discard show everywhere; a message to an offline Chief's computer waits and says so. Later a PWA on a phone is one more device. Building the phone chat is out of scope; designing for it is in.
- Both paid features are in the Personal plan (€5 a month or €48 a year, excl. VAT). The app stays free and complete without an account; Hersir sells hosted services only.
- Out of scope: building the phone chat, agents across devices or on servers, teams and SSO.

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
- **The roster** [I, new]: each computer keeps a list of the account's computers and their public keys, built from join records sealed with AK and signed by the joining computer. A computer that joined with the recovery code adds a tag made with a key derived from the code, so a thief who has only AK (from a stolen laptop) can't add a computer; a removal, signed by any computer on the roster, is final. The Chief's computer uses it to know who wrote a command (3.8).
- **Ciphers** [V versions, S-E15 to S-E18]: XChaCha20-Poly1305 (`chacha20poly1305` 0.11, Apache-2.0 or MIT), HKDF and HMAC with SHA-256 (`hkdf`, `hmac` and `sha2`, already in Hersir's `Cargo.lock`), Ed25519 (`ed25519-dalek`, already in the lock through `ssh-key`) and X25519 (`x25519-dalek`, BSD-3). Each object is at most a 4 MiB chunk sealed whole, with a random 24-byte nonce and its id as associated data. Not `age` (beta, scrypt passphrases).

#### Removing a computer, and whether the key must change (decision 12)

- **Remove** (on the account page or in Settings on another computer) withdraws that computer's token at once: its next call gets 401 and it shows *Signed out on the account page* [I]. It also puts a removal on the roster, so from then on the Chief's computer refuses every command signed by it, even if the server forwarded one.
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
   - `/.well-known/oauth-protected-resource` → `{ "resource": "https://cloud.hersir.dev/api", "authorization_servers": ["https://cloud.hersir.dev"], "resource_name": "Hersir Cloud" }`;
   - `/.well-known/oauth-authorization-server` → `{ "issuer": "https://cloud.hersir.dev", "authorization_endpoint": ".../oauth/authorize", "token_endpoint": ".../oauth/token", "code_challenge_methods_supported": ["S256"], "response_types_supported": ["code"], "grant_types_supported": ["authorization_code"], "scopes_supported": ["device:register"] }`.
   `oauth.rs::discover("https://cloud.hersir.dev/api", None)` finds both (`oauth.rs:169-250`), and `begin(&d, Some(HERSIR_CLOUD_CLIENT_ID))` skips registration (`:293-302`). **No change to `oauth.rs` is needed** [V the code paths; I that it works end to end, which card 3 tests].
3. **Two details to test in card 2**: `league/oauth2-server` also accepts the weaker `plain` PKCE method, so a middleware refuses an authorize request without `code_challenge_method=S256`; and `oauth.rs` sends `resource` (RFC 8707), which the league's grant doesn't read: it should be ignored (not verified).
4. **The exchange: a token per computer.** The browser's token has only the scope `device:register`. The app sends it once to `POST /api/v1/devices` with the computer's id, name, platform, Hersir version and public keys. The server records the computer, makes it a Passport personal access token named `device:<id>` with the scope `cloud` (valid a year; the app renews it in its last 30 days with `POST /api/v1/devices/me/token`), revokes the browser's token and its refresh token, and answers with the new token. The app keeps it in the keychain (`cloud/<account>/token`). One package (Passport) does both the browser sign-in and the tokens per computer, so Sanctum isn't needed [I].
5. **Withdrawing a computer**: the account page and Settings → Hersir Cloud on another computer list the computers (name, platform, Hersir version, last seen, added) with **Remove**, which revokes its token (`$token->revoke()`, checked on every request), sets `devices.revoked_at`, and adds the removal to the roster (3.2). That computer's next call gets 401 and it says "This computer was removed from Hersir Cloud on the account page." **Sign out** on a computer does the same for itself, and also removes the account key and its own keys from its keychain.
6. **What `oauth.rs` doesn't get**: revocation (RFC 7009). Passport has no revocation endpoint; `TokenStore` withdraws tokens "where the server offers that", and the exchange revokes the browser's token on the server anyway [V S-L4].

Weighed: Passport's device-code grant (RFC 8628) needs no listener, but `oauth.rs` already has one; Sanctum tokens would need a second package beside Passport [I].

<!-- Section 3.5 onwards follows. -->
