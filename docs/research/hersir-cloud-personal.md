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

**Recommendation** [I]: one computer per workspace is **the Chief's computer**, chosen in Settings. The Chief runs there and only there: its chat answers, its board checks, its approvals. Every other computer writes to it through Hersir Cloud with **commands**: small messages sealed with the account's command key and signed by the computer that wrote them, which the server stores and forwards but can't read, add or change. What the Chief does comes back to every computer through sync, since its threads, messages, queue and pending actions are synced rows with one writer. While it writes, its answer streams live through Reverb.

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
- **Approve on Laptop** sends `pending.approve { id }` (or `pending.approve_all { ids }`, `pending.discard { id }`). Desktop runs it through the same path as its own button (`src-tauri/src/pending.rs:114, 136, 152`), whose `pending::claim` (`crates/gizai-core/src/pending.rs:155`) runs a call once: an approval from a second computer, or a second click, gets the usual refusal. Laptop shows "Approving on Desktop…" until the synced row changes; the note says where it was decided ("Approved on Laptop: …").
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

- **Sign-in**: the same account, in the browser (password and authenticator), as a public client of Passport with an `https` redirect on the PWA's own origin.
- **The key, by pairing** (recommended): Settings → Hersir Cloud → *Add a phone* shows a QR code holding a one-time pairing secret; the phone makes its own keys (X25519 and Ed25519, kept as non-extractable WebCrypto keys in IndexedDB), sends its public keys through the server tagged with the secret, and the computer seals the key to the phone and signs its join to the roster. The server can't step in between without the secret. **Or** typing the recovery code on the phone, as a computer does.
- **Which key**: the decision says a device that holds the account key; recommended instead is a **chat key** derived from AK, so a phone holds what the chat needs and nothing else: it can write commands and read the chat, not the backups or the sync log (open question 17).
- **What the phone reads**: it doesn't apply the sync log. The Chief's computer also posts each finished message and waiting call to a chat feed sealed with the chat key, kept 30 days on the server; the phone reads that feed and the live answers, and writes commands like any computer.
- **Notifications**: Web Push with no content ("The Chief answered", "The Chief waits for your OK"), so nothing readable passes through Apple's or Google's push services.
- **The web's weak point**: the page's code comes from a server, so a hacked server could serve code that takes the key. Serve the PWA as static files from their own origin, built from a public repository, with a strict Content Security Policy; that is the PWA card's work, not this card's.

<!-- Section 3.9 onwards follows. -->
