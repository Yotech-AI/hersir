# Hersir Cloud for one person: Cloud Backup and Multiple devices

Research for GA-67, 9 November 2026, by the R&D Agent. No code was changed. It turns `docs/PAID-FEATURES.md` (Phase 1 and Phase 2) into build cards.

**[V]** = verified: in this repository (a file and line), or on a primary source read on 2026-11-09 (listed in [Sources](#9-sources)). **[I]** = my judgement.

## 1. Summary

1. **Order**: Cloud Backup first, on your own computers with a plan given by hand; Multiple devices next (its engine can be built at the same time); Personal goes on sale once Paddle and the legal pack are done. Eight cards, about 30 to 40 agent-days (section 6).
2. **Encryption: end-to-end for both** (recommended over the 9 Oct readable copy): a random account key in each computer's keychain plus a recovery code; the server holds ciphertext, never needs Hersir's schema, and a breach doesn't expose your customers' clients.
3. **The code's biggest surprise**: `changes` can't drive sync as it is. Its rows are feed summaries, some writes log nothing, and its clock isn't a real HLC. Sync gets a trigger-written log beside it, a real HLC, last writer wins per field, and card numbers settled in the server's order.
4. **Cloud Backup** keeps format 1's manifest and GA-218's restore; only the transport is new: a page-for-page copy in 256 KiB chunks, each compressed and encrypted, named by a keyed hash, uploaded straight to storage.
5. **Server**: one Hetzner server in Germany, Rust, SQLite with Litestream, Hetzner Object Storage: about €14 a month up to a few hundred customers, about €71 at 1,000.
6. **Sign-in**: Zitadel Cloud (EU region) through the OAuth client Hersir already has, then Hersir Cloud's own device tokens. **Payments**: Paddle (no OSS for the S.L.; fees 7% of the yearly price, 16% of the monthly).
7. **Legal**: templates for the notice, privacy policy, records, DPA (the Commission's clauses) and breach procedure; a Spanish lawyer for the terms and their fit with Paddle.
8. **Twelve open questions** (section 7); the first is to confirm end-to-end encryption.

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

### 3.2 Question 1: Encryption

**Recommendation: end-to-end for both Cloud Backup and Multiple devices** [I]. The server stores and orders ciphertext; your computers encrypt, decrypt and merge. Keep the rest of the 9 Oct choice: one workspace per customer in the EU, the cloud as the shared copy every computer syncs with, and a full local copy on every computer that works offline.

Why, now that the phone chat comes later [I]:

- **What the readable copy bought is out of scope now**: a phone or web view reading data through an API, snapshots made on the server, conflicts settled on the server.
- **It would hold your customers' clients in plaintext**: names, emails, phone numbers, IBANs and notes, on a server one person guards. Encryption at rest protects stolen disks and leaked backups, not a hacked app server, the operator or a court order. A readable sync would also undo the backups' encryption: the same data would sit in plaintext beside them.
- **An end-to-end server never needs Hersir's schema.** It doesn't migrate anything, so a Hersir release with a migration needs no server release. A readable master copy must apply every change with `gizai-core` and migrate every customer's database in step with each app release: release coupling a one-person team pays for every time.
- **GDPR**: a breach of ciphertext whose keys only the customer holds is, in the main, a matter for the breach record, not for telling the people in it (Art. 34(3)(a), below).
- **It is what the plan, the website and comparable tools say**: `PAID-FEATURES.md:82-87`; "Encrypted backups of your data"; end-to-end by default at Obsidian, Standard Notes, Joplin, Actual Budget and Ente (S-E1 to S-E9).

#### How the keys work

| Key | What it is | Where it lives |
|---|---|---|
| Account key (AK) | 256 random bits, made by the first computer | each signed-in computer's keychain (`cloud/<account>/key`); never on the server as it is |
| Data key, id key, key id | derived from AK with HKDF-SHA256 | computed when needed; the key id (16 bytes) is on the server, to tell a wrong code from a right one |
| Recovery code | 128 random bits, shown as 26 characters in groups (`HC-7K2Q-…`) | on paper or in your password manager; the server keeps AK sealed with a key derived from it |

- **A new computer gets the key** by signing in and typing the recovery code (v1). Later: *Approve on your other computer*, a short code and a key exchange (X25519) through the server, as Signal links devices [I]. Passkeys (WebAuthn PRF) could add one more way, once support is less patchy than in 2026: Bitwarden unlocks vaults with PRF, but only in Chromium browsers and not in Chrome profiles (S-E3; browser support from bug trackers, search results only).
- **No passphrase in v1.** A key sealed by a passphrase sits on the server, where a weak passphrase can be guessed offline after a breach; Argon2id slows that, it doesn't stop it. A random 128-bit code can't be guessed, and needs no Argon2 at all. 1Password and Ente add a random secret for this reason (S-E4, S-E8). This changes the plan's "Argon2id from a passphrase" (`PAID-FEATURES.md:83`).
- **What is lost when the key is lost**: only when every computer and the recovery code are gone. Then the cloud copies (backups and the sync log) can't be read by anyone, Hersir included; the data on your computers is untouched. While one computer still has the key, it can show a new recovery code (AK is sealed again; nothing is encrypted again). Setup says so plainly and asks you to type back part of the code before the first upload, as Apple asks for a recovery key before Advanced Data Protection (S-E2).
- **Ciphers** [V versions]: XChaCha20-Poly1305 (`chacha20poly1305` 0.11.0, Apache-2.0 or MIT; audited by NCC Group in 2020, before this version line), HKDF and HMAC with SHA-256 (`hkdf`, `hmac`, the `sha2` Hersir already has). Each object is at most a 4 MiB chunk sealed whole, with a random 24-byte nonce and its id as associated data, so no streaming construction is needed. Not `age` (0.12.1 says "[BETA]" and its passphrases use scrypt) (S-E15 to S-E18).

#### GDPR exposure

| After a breach of the server | End-to-end | Readable by the server |
|---|---|---|
| What leaks | ciphertext, sizes, times, device and account metadata | your customers' clients: names, contacts, IBANs, notes |
| Your customer (the controller) | records it; telling the people in it is not needed when the data is unintelligible (Art. 34(3)(a)) | assesses within 72 hours, very likely tells their authority, perhaps their own clients (Art. 33, 34) |
| YoTech | processor; tells customers without undue delay (Art. 33(2)) | the same, plus damages claims (Art. 82) |
| A court order | ciphertext only | plaintext |

- Whether ciphertext is personal data *for YoTech* is arguable since the Court of Justice's judgment of 4 Sep 2025 (C-413/23 P, EDPS v SRB): pseudonymised data "must not be regarded as constituting, in all cases and for every person, personal data" (S-E19). The EDPB's 2026 anonymisation draft follows it (S-E20). No court has applied it to encryption yet, and YoTech holds the account data, IP addresses and metadata anyway: **treat YoTech as a processor and sign a DPA either way** [I].
- The AEPD's breach guide says the same as Art. 34(3)(a): no notice to the people affected when encryption keeps the data unreadable, with the decision documented (S-E21, from search results; the PDF didn't open).

#### What a later phone chat would need

- **With end-to-end**: the phone gets the key (or a chat-only key) by scanning a code on a computer, as WhatsApp and Signal link devices; push notifications carry a wake-up or a small encrypted payload that the phone decrypts (an iOS Notification Service Extension; an Android data message) [I].
- **With a readable server**: the phone signs in and reads an API; but client names in push payloads would pass through Apple and Google, so you'd encrypt or leave them out anyway [I].
- End-to-end doesn't block it; it adds pairing and decryption on the phone. Since the phone chat talks to the Team Lead on its home computer, it can use a relay with its own key, without the phone ever syncing the whole database [I].

### 3.3 Question 2: The server

**Recommendation** [I]: one Hetzner Cloud server in Germany, a Rust (axum) API, SQLite on that server with Litestream streaming it to object storage, and Hetzner Object Storage for the customers' encrypted objects, uploaded straight from the app with presigned URLs. One EU company, one DPA, no transfer outside the EU. Keep the storage code plain S3, so Cloudflare R2 is a configuration change away.

- **SQLite instead of the plan's Postgres** (a change to `PAID-FEATURES.md:108`): with end-to-end encryption the server's own database holds only accounts, devices, plans, the object and snapshot index, the encrypted sync log and claims. On one server, SQLite in WAL mode does this with no database server to run, patch or secure, and Litestream (Apache-2.0) streams it to a bucket within a second, so a restore drill is one command. It is also the engine the app already uses. Move to Postgres when a second API server is needed, not before.
- **What the server is**: one binary, `hersir-cloud`, in a new private repository (P4: hosted and closed at first, with a published protocol, `PAID-FEATURES.md:205`). It serves `api.hersir.dev` (the plan's name, `PAID-FEATURES.md:69`) and the account pages; Caddy in front for TLS. Cloudflare keeps the DNS only (grey cloud): proxying would put a US company in the data path.

#### Hetzner today [V, S-S1 to S-S7]

| What | Price (excl. VAT) | Notes |
|---|---|---|
| CX23: 2 vCPU, 4 GB, 40 GB | €5.99 a month | 20 TB traffic included; enough until about 1,000 customers [I] |
| CX33: 4 vCPU, 8 GB, 80 GB / CX43: 8 vCPU, 16 GB, 160 GB | €8.99 / €16.49 | the next steps |
| Backups (7 daily images) | 20% of the server | a whole-machine copy, beside Litestream |
| Object Storage (FSN1, NBG1, HEL1) | €6.49 a month for 1 TB stored and 1 TB out; then about €6.35 a TB | S3 API, presigned URLs, lifecycle, object lock; S3 calls free; **a 64 KB minimum billable object**; Hetzner itself says it "is not ideal for all scenarios" |
| DPA (AVV) | in the customer account, by a checkbox | sub-processor list published; EU locations are processed in the EU |

Two risks: Hetzner raised cloud prices three times in 2026 and has limited new cloud servers since April (secondary sources; its own price-change page was 404 and every Create button said "currently unavailable" today) (S-S8). So: order the servers early, and keep Scaleway (France) as the fallback.

#### The alternatives

| Set-up | For | Against |
|---|---|---|
| **A. Hetzner server + Hetzner Object Storage** (recommended) | one EU company; cheapest at scale (about €6.35 a TB against $15 on R2); keeps Rust | you run a server: about 1 to 3 hours a month [I] |
| B. Hetzner server + Cloudflare R2 (EU jurisdiction) | R2 is mature, free egress, $0.015 a GB-month | a US sub-processor under the EU-US Data Privacy Framework (whose appeal, C-703/25 P, is pending) and SCCs (S-S10, S-S11) |
| C. Cloudflare Workers + one Durable Object per customer + R2, all with `jurisdiction: eu` | no server at all; cheapest at 10 to 100 customers; Durable Objects order a workspace's log by nature | US company; Durable Object ids are logged outside the jurisdiction; `workers-rs` "expect a few rough edges", D1 in Rust is alpha, so in practice TypeScript; lock-in (S-S12 to S-S16) |
| D. Scaleway (France) servers, storage and managed PostgreSQL | EU company with managed Postgres (about €11 a month) | its price pages returned 403 today (search snippets only) (S-S17) |

Cloudflare D1, Durable Objects and R2 all take a jurisdiction (`eu`) at creation, which can never change; location hints are "best effort" (S-S12 to S-S14).

#### Deploys, its own backups, monitoring [I, prices V]

- **Deploys**: GitHub Actions builds a static Linux binary on a tag; a script copies it to the server and restarts the systemd unit; the previous binary stays beside it for a rollback. Migrations of the server's database run at start, forward only, as in the app.
- **Its own backups**: Litestream replicates the server's SQLite to a bucket at a second provider (Scaleway or Backblaze B2), so one locked account can't take everything; Hetzner's 7 daily images on top. A monthly restore drill onto a throwaway server. The customers' objects stay in one location (they are backups already, and every computer keeps its data).
- **Monitoring**: healthchecks.io (free for 20 checks) for Litestream, the retention job and a minute-by-minute ping; an uptime check with a status page (Uptime Kuma on a second small server, or Better Stack's free tier); errors to email, or Sentry (from $26 a month, US company, a data-residency option) once there are paying customers. No customer data in logs: ids and sizes only (S-S18 to S-S20).

#### Cost a month (EUR, excl. VAT) [I, computed from the prices above and S-S15]

Assumptions: per customer 3 GB stored (1.5 × 0.2 GB database and 1.8 GB files) and 50 MB in the server's database; one CX23 up to a few hundred customers, a CX43 at 1,000 with a standby server and paid error tracking.

| Customers | A: Hetzner + its storage | B: Hetzner + R2 EU | C: Cloudflare only | Revenue (€5 a month) |
|---|---|---|---|---|
| 10 | about €14 | about €8 | about €5 | €50 |
| 100 | about €14 | about €11 | about €8 | €500 |
| 1,000 | about €71 | about €93 | about €78 | €5,000 |

If every customer filled the 10 GB quota, 1,000 customers would store about 15 TB: about €95 a month at Hetzner, about €193 at R2.

### 3.4 Question 3: Accounts and sign-in

**Recommendation** [I]: **Zitadel Cloud in its EU region** as the sign-in service, with passkeys, Google and GitHub, and email with a password as the fallback. Free while only you use it; Pro ($100 a month) with a custom domain such as `login.hersir.dev` before the first customer, because passkeys are bound to the domain they were made on. The desktop signs in with the OAuth client Hersir already has; then Hersir Cloud gives the computer its own device token, so a computer that syncs all day doesn't count as a sign-in every day. Fallback: Zitadel self-hosted, the same product.

#### The services that fit: EU data and a real OIDC provider for a desktop app [V unless marked, S-A1 to S-A12]

| Service (company) | EU data | Sign-in ways | Desktop client (PKCE, loopback, any port) | Price at 100 / 1,000 / 10,000 users a month |
|---|---|---|---|---|
| **Zitadel Cloud** (Switzerland) | EU region on every plan, Free included | passkeys; Google, GitHub and others; password; email codes as a second factor only, no magic link [I] | yes: OpenID certified; among loopback addresses the port is ignored (read in its source) | counted in *user-days*, and a token refresh counts as a day: Free 100 user-days; Pro $100 with 25,000 |
| Logto Cloud (Silverhand) | EU, US or AU, fixed at creation [I] | email code (passwordless), passkeys on Pro, social | native apps are public clients [V]; a changing port not tested [I] | Pro $24 with 50,000 access tokens, then $0.08 per 100 |
| Ory Network (US parent, German subsidiary) | "EU Data Storage" from the Production plan; it says PII, so other data may not be pinned | email code, passkeys, social | OpenID certified; any port on `127.0.0.1` [I] | about $64 / $146 / $1,070 |
| Auth0 (US, Okta) | EU tenant chosen at creation [I] | passwordless, passkeys, social | yes; port handling not checked | Free up to 25,000 users |
| Hanko Cloud (Germany) | Frankfurt only | passkeys, passcodes, social | **no**: not an OIDC provider | Free up to 10,000 |

Out for EU data or for a desktop client: Clerk and WorkOS (no data residency on their price pages), Stytch (US servers only), Descope (Frankfurt only on unpublished plans), Supabase Auth (redirects must match exactly, so a changing port is probably refused), Hanko (above). Zitadel is the only one here that is not a US company, is a certified OIDC provider, and keeps EU data on every plan.

#### Self-hosted, if the managed one goes away [V/I, S-A13 to S-A17]

| Project | Licence | What it needs | Fit |
|---|---|---|---|
| **Zitadel** | AGPL-3.0 (with Apache-2.0 and MIT parts) | Go, PostgreSQL 14 or later | the same server as the Cloud: moving needs no redesign |
| Rauthy | Apache-2.0 | Rust; embedded SQLite (Hiqlite) or Postgres; about 40 MB of memory | light, magic links and passkeys, Google and GitHub upstream, audited by Radically Open Security; but one part-time maintainer and pre-1.0 (0.37.2) |
| Keycloak | Apache-2.0 | Java, Postgres, 1 to 2 GB of memory | mature but heavy; passkeys since 26.4 |
| Ory Kratos + Hydra | Apache-2.0 | Go, Postgres, two services and your own login pages | the most glue |
| Logto OSS | MPL-2.0 | TypeScript, Postgres | the same product as Logto Cloud |

Upkeep, whichever: its database's backups, security releases within days, an EU email sender with SPF, DKIM and DMARC, and monitoring; a few hours a month for Zitadel or Rauthy [I].

#### The desktop's sign-in

1. Settings → Hersir Cloud → **Sign in** opens your browser on Zitadel's page, with the flow `oauth.rs` already does: authorization code, PKCE S256, a random state, the answer to `http://127.0.0.1:<random port>/callback` (RFC 8252 §7.3: the server "MUST allow any port"; `localhost` is "NOT RECOMMENDED", §8.3) [V S-A18]. Hersir is a public client registered once in Zitadel; its client id is in the app, without a secret (RFC 8252 §8.4-8.5).
2. The app sends the access token once to `POST /v1/devices`. The server checks it (issuer, audience, expiry, the key set from Zitadel's discovery; `jsonwebtoken` 11.1.0, MIT, behind a small axum extractor) [V S-A19], registers the computer (its `devices.id`) and answers with a **device token** of its own: a random secret stored hashed on the server and in the keychain on the computer.
3. Every later call uses short-lived access tokens the server signs from that device token. The account page lists the devices; *Remove* deletes a device's token, and that computer gets 401 on its next call and shows *Signed out on the account page*.

Why the device token [I]: Zitadel counts each day a user signs in *or refreshes a token*, so a computer syncing all day through Zitadel's tokens would cost a user-day every day (S-A3); revoking a computer is then Hersir Cloud's own list, as the plan wants (`PAID-FEATURES.md:111`); and moving to another sign-in service later doesn't touch signed-in computers. It is not a home-made login: passwords, passkeys and the Google and GitHub sign-ins stay Zitadel's.

Two things to test before the cards are built: whether Zitadel's hosted page takes an email code alone (if not, launch with passkeys, Google, GitHub and a password), and whether it accepts or ignores the `resource` parameter `oauth.rs` sends (RFC 8707) [I].

### 3.5 Question 4: Payments

**Recommendation: Paddle Billing** [I]. It takes a Spanish S.L. and this kind of product, leaves the S.L. one sale a month to Paddle with no VAT on it, pays out in EUR to a Spanish bank for free, and has the hosted customer portal, retries and webhooks the plan needs. Stripe Managed Payments is the plan B: cheaper on the monthly price, newer, and branded Link.

| | Paddle Billing [V S-P1 to S-P7] | Stripe Managed Payments [V S-P8 to S-P12] |
|---|---|---|
| A Spanish S.L. | yes: Spain isn't on the excluded list | yes: ES is listed, subject to review |
| Fees | 5% + $0.50 a transaction; "contact us for bespoke pricing" under $10; chargebacks up to €20; currency conversion at mid-market + 2% | 3.5% on top of Stripe's fees (EEA card 1.5% + €0.25), Billing 0.7%; €20 a dispute |
| Who sells to your customer | Paddle.com Market Ltd (England) for EU and other buyers; Paddle.com Inc. for US buyers; English law | "Sold through Link, LLC" (a US-style company, not an Irish one); customers see `LINK.COM*` on their statement and get receipts from Link |
| Payouts | monthly, by the 15th; EUR to a eurozone bank free; threshold from €100 | "up to four times faster" than monthly; Spain's schedule not checked |
| Your invoice | Paddle issues self-billing invoices to you | the Managed Payments fee is invoiced by the LLC, other Stripe fees by Stripe's Irish company |
| Limits | refunds within 14 days are Paddle's call; you don't refund directly | Checkout and Payment Links only; Stripe may refund without you after 48 hours of no answer |
| Before you can sell | a domain review: a live site with the product, prices, features, Terms naming the company, a refund policy and a privacy policy in its navigation | an eligibility review |

**What it costs, a Spanish consumer (21% VAT)** [V fees; I sums]:

| | Paddle monthly | Stripe monthly | Paddle yearly | Stripe yearly |
|---|---|---|---|---|
| The customer pays | €6.05 | €6.05 | €58.08 | €58.08 |
| Fees | €0.80 | €0.59 | €3.40 | €3.56 |
| YoTech receives | €4.20 | €4.41 | €44.60 | €44.44 |
| Fee as a share of the price without VAT | 16.1% | 11.9% | 7.1% | 7.4% |

So push the yearly plan (as the plan says), and ask Paddle for its pricing under $10.

**VAT and OSS for the S.L.** [I; a Spanish tax adviser must confirm]: Paddle (England, or the US) is your only customer, a business outside the EU, so your supply to it isn't subject to Spanish VAT (art. 69.Uno.1º LIVA) and goes in Modelo 303's box for operations not subject by place-of-supply rules, not in Modelo 349 (S-P13). Paddle is the seller to every end customer and files their VAT; Art. 9a of Implementing Regulation 282/2011 treats a platform that sets the terms and the charge as the supplier anyway (S-P14). So **no OSS registration**, and no ROI registration for the Paddle sales (the S.L. may need ROI for EU suppliers such as Hetzner).

#### How a plan reaches the app

```
Paddle checkout (on the account page) ── webhooks ──► hersir-cloud: subscriptions table
   subscription.created / updated / canceled / past_due, transaction.completed / payment_failed
                                                           │
                                     GET /v1/me/entitlements, signed with Ed25519
                                                           │
                         the app: plan badge, quota bar, "renew" notice (display only)
                         every backup and sync call: the server checks the plan itself
```

- The **account page** (`account.hersir.dev`, served by `hersir-cloud`): sign in with Zitadel; your plan and its renewal date; storage used; your computers with *Remove*; *Subscribe* (Paddle's overlay checkout, with your account id in its custom data); *Manage plan* (a Paddle customer-portal link made by Paddle's API). No payment screen in the app.
- **Webhooks** are checked with Paddle's signature header and handled once each (by event id); Paddle retries a failed renewal up to 7 times over 30 days, then the subscription ends, and the lapse rules of question 5 start (S-P4, S-P7).
- **The entitlement** is a small JSON document (`plan`, `status`, `features`, `quota_bytes`, `used_bytes`, `valid_until`, `read_only_until`, `issued_at`) and an Ed25519 signature; the public key is built into Hersir (`ed25519-dalek`). The app trusts it for display until `valid_until`; it never locks or deletes local data over it (`PAID-FEATURES.md:81`).
- A **comp plan** (an entitlement given by hand, from the server's admin command) lets you use Cloud Backup before Paddle is live.

### 3.6 Question 5: Cloud Backup in detail

**Recommendation** [I]: keep format 1's manifest and restore, and change only how the bytes travel and sit in the cloud: cut them into chunks, compress and encrypt each chunk on the computer, and name each by a keyed hash, so a daily backup uploads only what changed. A cloud restore downloads into a local format-1 folder and hands over to the restore Hersir already has.

#### What goes up, and how

```
the database: copied with SQLite's online backup API (page for page, page numbers kept),
              then cut into fixed 256 KiB chunks (64 pages of 4 KiB)
the blobs the copy names: a blob up to 4 MiB is one chunk; a bigger one is cut by content (FastCDC)
                    │
each chunk:  id = HMAC-SHA256(id key, its bytes)        never the plain SHA-256: see below
             body = XChaCha20-Poly1305(data key, nonce, zstd(chunk)), with the id as associated data
                    │
ask the server which ids it lacks  →  PUT only those, each to its own presigned URL
                    │
the manifest: format 1's JSON, plus the ordered chunk ids of the database and of each blob,
encrypted the same way, then registered with the list of every chunk id it uses
```

- **Why a page-for-page copy, not `VACUUM INTO`** [V S-B1; I]: `VACUUM INTO` (format 1's copy) writes the database afresh and renumbers its pages, so growth in one table moves every later page and changes the page numbers in every interior page after it; chunks of it would barely repeat from one day to the next. The backup API keeps each page where it is, so a day's upload is the 256 KiB chunks holding pages that changed, plus new blobs. Whole-file zstd (format 1's `.db.zst`) would change almost everywhere too: compress each chunk instead, as restic does (S-B2). The card measures chunk sizes (64 KiB, 256 KiB, FastCDC) on a month of real use before it settles. A page-for-page copy keeps free pages, which may hold deleted text; they are encrypted and go with the snapshot after 90 days.
- **Hetzner bills an object as at least 64 KB** (S-S6), and a compressed 256 KiB chunk is mostly above that; no packing of chunks into bigger files is needed, which keeps deletion simple [I].
- **Why a keyed hash as the name** [I]: a plain SHA-256 tells the server which known files you have (anyone can hash a public PDF and look for it). The HMAC key is derived from your account key, so the same file still has the same name across your own snapshots and computers, which is all dedup needs.
- **Why each chunk is its own object** [I]: an upload that stops resumes at the next missing chunk; nothing needs S3 multipart; and the server can count and delete objects without reading them. A presigned URL lets the app upload straight to object storage, so the bytes never pass through the API server.
- **What the server sees**: object ids, sizes, times, the snapshot list (name, time, computer, reason, schema version, total size) and which objects a snapshot uses. Not one byte of content.

#### When it goes up

| When | What | Why |
|---|---|---|
| Daily, when something changed since the last cloud snapshot | a snapshot, reason `daily` | the plan's daily snapshot; nothing changed means nothing to do |
| On quit, when On quit is on | reason `quit`, as format 1 | the end of a working day is the moment you'd want |
| Back up now | reason `manual` | before something risky |
| Before a restore | reason `before-restore`, as format 1 | already what a restore does |

Not "on every change": with Multiple devices every change goes to the cloud anyway, and before it a backup each few minutes would cost uploads for little [I]. The cloud backup runs beside the local one, never instead of it: a computer can have both (Settings keeps the folder's schedule as it is).

#### History and restore

- **Keep** (server side, so a computer that is off can't keep old snapshots alive): every snapshot for 7 days, then the newest of each day until 90 days, then none. Objects no snapshot uses go after a day's grace [I].
- **Restore on this computer or a new one**: the list comes from the server (newest first, with the computer that made it); Restore downloads the manifest and its chunks into a temporary format-1 folder (`<data folder>/restore/cloud/`), checks every hash, and from there it is the restore of GA-218 as it is (`docs/backup-format.md:127-196`): the checks, today's data saved first, staging, the swap at the next start, the device rule, this computer's settings.
- **A new computer**: install Hersir, then *Restore from Hersir Cloud* on the first screen or in Settings: sign in in the browser, type the recovery code (the key; see question 1), pick a snapshot, restart.

#### The quota and a lapsed plan

- **10 GB counts what is stored**: the compressed, encrypted bytes of every object that a kept snapshot uses, once each. The server knows each object's size, so it computes this exactly, and refuses a new upload over the quota with a clear message ("Hersir Cloud is full: 10.0 of 10 GB. Remove old backups or files you no longer need.") [I].
- **A lapsed plan** (P7, `docs/PAID-FEATURES.md:208`): uploads stop the day the plan ends (after the payment provider's retries); the list and restore keep working, read-only, for 60 days; emails go out when it lapses, after 30 days and a week before the end; then the objects are deleted. The app says when downloads end. Local data is never touched [V for the plan; I for the emails].

### 3.7 Question 6: Multiple devices in detail

**Recommendation** [I]: build sync on a new, complete log beside `changes`, written by triggers, with a real hybrid logical clock, last writer wins per field, and a server that only stores and orders encrypted transactions: Actual Budget's model (per-field changes, an HLC, a server that "is just passing around changes") with Linear's server sequence numbers (S-B4, S-B5). Keep `changes` as the activity feed and audit log it is, and carry its rows along, so another computer's feed shows the same lines.

No sync engine can be adopted instead: PowerSync, ElectricSQL, Turso Sync and SQLite Sync all need a server that reads the rows, which end-to-end encryption rules out, and each would replace Hersir's own log; cr-sqlite says "main may not be 100% stable" and has had no release since January 2024 (S-B5, `notes/E-sqlite-sync.md` §2) [V/I].

#### The `changes` log as the code has it now

| Column | What it holds now [V] | Enough for sync? |
|---|---|---|
| `seq` | local order | yes, as the local order to push in |
| `id` | UUIDv7 of the change | yes |
| `hlc` | `<ms>-<counter 0000-9999>-<8 chars of the device>`, made at write time (`db.rs:364-366`) | no: not an HLC (§2.4, 3) |
| `device_id`, `actor_id`, `run_id` | who and where | yes |
| `table_name`, `row_id`, `op` | the row | only for single-key tables (`task_labels` has a two-column key) |
| `diff_json` | a summary for the feed | no (§2.4, 1) |
| `schema_version` | `SCHEMA_VERSION` at write time | yes |
| `pushed_at` | NULL; nothing sets it yet | the right idea |

So sync can't read `changes` as it is. Two ways out:

- **(a) Make every write log full values.** About 200 statements in about 40 files to change, and every future write must remember to; the feed's lines would have to be rebuilt from values. Risky, and a missed write silently stops a field from syncing.
- **(b) Recommended: triggers write a sync log.** On each synced table an `AFTER INSERT/UPDATE/DELETE` trigger, generated from one list in code, appends the changed columns' new values to `sync_log`, in the same transaction as the write. Nothing can bypass it, the existing write code stays as it is, and `changes` keeps its job.

The two objections to triggers in `notes/E-sqlite-sync.md` (they can't know the actor, and they misfire while a merge is applied) have answers [I]: `Db::write` registers application-defined SQL functions on the writer connection (rusqlite's `functions` feature): `hersir_hlc()` gives the next clock value, `hersir_capture()` says 0 while remote changes are applied or a migration runs, and the transaction's actor comes from `Writer`.

#### The new local tables

```sql
CREATE TABLE sync_log (            -- local and remote transactions' row changes; never in a snapshot's restore as-is
  seq INTEGER PRIMARY KEY,         -- local order
  tx_id TEXT NOT NULL,             -- one Db::write = one transaction, pushed as one
  hlc TEXT NOT NULL,               -- '<13-digit ms>-<6-digit counter>-<full device id>': sorts as text
  device_id TEXT NOT NULL,
  table_name TEXT NOT NULL,
  row_key TEXT NOT NULL,           -- the id, or a JSON array for a two-column key
  op TEXT NOT NULL CHECK (op IN ('upsert','delete')),
  cols_json TEXT,                  -- {"title": "...", "state_id": "..."}: new values of the changed columns
  schema_version INTEGER NOT NULL,
  server_seq INTEGER               -- NULL until the server has it; set for every remote one
) STRICT;
CREATE TABLE sync_clock (          -- the last write that won, per field: what LWW compares against
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

A hybrid logical clock (Kulkarni and others, 2014) [V S-B3]: each value is (wall-clock ms `l`, counter `c`, device). A local write takes `l = max(l, now)`, with `c + 1` when `l` didn't move and 0 when it did; a received change takes `l = max(l, l_remote, now)` and continues the counter of whichever was largest. The paper proves that a change that happened after another always has a larger value. Four changes to today's `db.rs:364-366` [I]: the receive rule; the last value kept (`cloud_link.hlc_state`), so a restart or a clock set back never goes behind; a change whose clock is far in the future (more than a minute ahead of the server's time, as Actual Budget does, S-B4) refused; and ties broken by the *whole* device id, as `docs/backup-format.md:206` already asks. The feed's `changes.hlc` can take the same value. (`uhlc` 0.9.0, EPL-2.0 or Apache-2.0, exists, but the rule is twenty lines; Hersir's own keeps the text format.)

The server's sequence number decides the order of delivery; the HLC decides which write wins. With both, Actual Budget's Merkle tree isn't needed: a hash per range of sequence numbers is enough as a repair check (S-B4, S-B5) [I].

#### Applying another computer's changes, and conflicts

Pull returns transactions in the server's order. For each, in one write with `hersir_capture()` off:

1. **Seen already** (its `tx_id` is in `sync_log`): skip. So a retry, a duplicate or a computer's own echo does nothing.
2. **Schema**: a transaction from a newer schema stops the pull there, and Hersir asks to update: "Your laptop uses a newer Hersir. Update this computer to go on syncing." Local changes keep queuing meanwhile. One from an older schema is applied: migrations that only add tables or columns with defaults are safe, and any other migration must come with a translation in sync's code (a test fails when `SCHEMA_VERSION` changes without a note on its sync effect, as `scope.rs` does for settings).
3. **Per field, last writer wins**: a remote value replaces the local one when its HLC is greater than `sync_clock`'s for that field; then the clock takes it. Two computers editing different fields of one card both keep their edit.
4. **Deletes** are `deleted_at` values like any other field, so an edit to a deleted card doesn't bring it back and the card stays archived with the edit inside it; an explicit restore (a newer `deleted_at = NULL`) does. That is the plan's "tombstones win over edits" without a special case. Rows removed for real (a card's labels, a column's agents, `task_builds_on`) sync as row deletes, compared by HLC with the row's last insert.
5. **Counters take the maximum**: `projects.next_task_number` and `orgs.next_task_number` never go down.
6. **Colliding task numbers**: a card's UUID is its identity; its number is settled in the server's order. The card the server got first keeps `EUQR-13`; a later card with the same number takes the next number free among the cards the server already has. Every computer applies the same transactions in the same order, so each works out the same answer, with no extra change to send (S-B9). A card made offline shows its number as provisional ("not synced yet") until the server has it, and the creating computer's activity says "Renumbered EUQR-13 to EUQR-15: your laptop made EUQR-13 first". Links survive, since they hold the UUID; a branch already made keeps its old name. The same rule serves the other unique names: a project's number and key, a label's name, an actor's handle.
7. **A document's text** is the exception to last writer wins: when both computers changed `docs.body_md` since they last agreed, the newer text wins and the other one is kept as a version "from Laptop, kept after a conflict", so no text is lost. Version numbers that collide in `doc_versions` take the next one.
8. **Foreign keys**: the server's order is the order in which computers received each other's work, so a row always arrives after the rows it points at. Each batch still runs with `PRAGMA defer_foreign_keys = ON`.

#### Files

Files sync by keyed hash, as backups store them (question 5): a push uploads the blobs the server lacks before the transaction that names them; a pull fetches a blob when it is opened, and the rest in the background. A file not here yet shows as "On another computer: downloading…". The quota counts each blob once, for backups and sync together [I].

#### What never syncs

| What | Why |
|---|---|
| `runs`, `run_refusals`, run logs (`runs/`), worktrees | a run lives on the computer that ran it (the plan) |
| `chat_queue`, `pending_actions` | messages and actions waiting for this computer's Team Lead |
| `api_tokens` | this computer's MCP tokens |
| `doc_links`, `search` | derived: rebuilt from what syncs |
| `devices` | the server's device list replaces it |
| this computer's settings (`scope::this_computer`) | `scope.rs` already says so |
| the columns in §2.4, 5 (local paths, CLI sessions, claims) | true on one computer only |
| `sync_log`, `sync_clock`, `cloud_link`, the keychain | the machinery itself, and secrets |

Everything else syncs: organisations, people and agents (with their settings, not their folders), teams and columns, clients and contacts, projects and repositories (without local paths), cards, labels, comments, docs and memory notes, files, chats' threads and messages, and the shared settings. A test lists every table and fails on one that is in neither list, so a new migration can't forget [I].

#### Agents: a card runs on one computer

- **Each agent has a home computer** (a new synced column `agent_configs.home_device_id`): the computer it was made on, or, when sync is first switched on, the one switching it on. Its form gets *Runs on* (This computer, Laptop, …). Only the home computer's dispatcher starts its runs [I].
- **A claim in the cloud before each start**: `POST /v1/workspaces/:id/claims` with the card, the run and a lease of 10 minutes, renewed every 3 while the run lives and released when it ends. The first claim wins; the other computer gets 409 with who holds it, and its board shows the card as "Running on Desktop". A computer that dies leaves a lease that runs out [I].
- **Offline**: no automatic starts on a synced workspace, because another computer could take the same card; the band says why, and Run by hand still works after a warning. (Open question 6.)
- **The Team Lead's chat**: threads and messages sync, so you can read them anywhere; you chat on the Team Lead's home computer. Chatting from another computer is agents across devices: out of scope.

#### How to test two devices, with fault injection

1. **In the core, without a network** (`crates/gizai-core/tests/it/sync_test.rs`): two or three in-memory databases and a fake server (an ordered list of transactions). A property test (`proptest` 1.12.0, MIT or Apache-2.0) makes random edits on each, then delivers with faults from a seeded generator: drops, duplicates, reorders before the server, delays, a partition, a crash after the push but before the answer (the retry must change nothing), and one clock an hour behind. The checks: after everything is delivered all replicas are equal (a canonical dump of the synced tables), no change the server acknowledged is lost, identifiers are unique, and a replica on an older schema stops instead of applying.
   Also: applying a transaction twice equals applying it once; each computer's clock only goes up; no two live cards share a number; restoring a backup gives back the database it was made from; an unchanged database uploads no new chunks (S-B6).
2. **Against the real server**: the server run locally, and two Hersirs with their own data folders (`GIZAI_DATA_DIR`), driven through their commands, never the UI; `toxiproxy` (2.13.0, MIT) between them for latency and cuts, or `turmoil` (0.7.2, MIT) to simulate the network inside one test (S-B7, S-B8).
3. **In real use**: your desktop and laptop, with sync's log visible in Settings until it has run a few weeks without surprises.

### 3.8 Question 7: Legal

**Recommendation** [I]: do the template work now (items 1 to 7 below, with the European Commission's and the AEPD's material), then one fixed-fee job for a Spanish consumer and technology lawyer: the terms of service in a business and a consumer version, a review of the DPA, and how both fit Paddle's terms and its withdrawal flow. Don't charge before the terms and Paddle's checkout are signed off. This is research, not legal advice (S-L1 to S-L19).

YoTech's two roles [I]: **controller** for account, billing, support and website data; **processor** for the data customers back up and sync (their clients). Using that data for its own purposes would make YoTech its controller (LOPDGDD art. 33.2) [V S-L2].

| # | What | What it needs | Template or lawyer |
|---|---|---|---|
| 1 | Legal notice (LSSI-CE art. 10) | NEXO IV GRUPO, S.L., its registered address, email, Registro Mercantil entry and NIF, on the website, permanently and free; before an order the steps, the language and storable terms (art. 27); a confirmation within 24 hours (art. 28) [V S-L1] | template |
| 2 | Privacy policy (GDPR art. 13) | for account holders and visitors: purposes, legal bases, recipients (Paddle as a separate controller, Zitadel and Hetzner as processors), retention, rights, the right to complain to the AEPD [I]; the AEPD's Facilita EMPRENDE helps [V S-L4] | template, a quick review |
| 3 | Records of processing (art. 30; LOPDGDD art. 31) | one as controller, one as processor; the AEPD's Gestiona RGPD covers it [V S-L2, S-L4] | template |
| 4 | DPO | not needed: YoTech isn't on LOPDGDD art. 34.1's list, and art. 37's triggers aren't met; a short memo says so [V S-L2; I] | in-house |
| 5 | DPA offered to customers (art. 28) | the Commission's standard clauses between controllers and processors (Decision 2021/915), with Clause 7.7 option 2 (sub-processors by general authorisation, notice of changes, a right to object); at the end of the service, delete or return as the customer decides (LOPDGDD art. 33.3-4) [V S-L6, S-L2] | template, lawyer review |
| 6 | Sub-processors | a public list (Hetzner Online GmbH, Germany: hosting and storage; Zitadel, Switzerland: sign-in; the email sender) with 30 days' notice of changes; Paddle is a controller of its own for payments. All EU or adequate countries (Switzerland, the UK), so no US transfer in the recommended set-up [I] | in-house |
| 7 | Breach procedure | as controller: tell the AEPD within 72 hours through its electronic office, record every breach, notified or not [V S-L3]; as processor: tell each customer without undue delay, with a fixed number of hours in the DPA; the AEPD's Asesora Brecha and Comunica-Brecha RGPD tools [V S-L4]; with end-to-end encryption the content stays unreadable | template |
| 8 | Consumers (TRLGDCU) | the total price with VAT per billing period (art. 97.1 e); 14 days to withdraw (art. 102); starting at once needs the consumer's express request and a confirmation on a durable medium (98.7-98.8), and a withdrawal then pays for the days used (108.4); digital-service conformity, including free export of the consumer's content after the end (119 ter) and rules for changing the service (126) [V S-L7]. The *withdrawal button* of Directive 2023/2673 applies from 19 June 2026; no Spanish transposition was found (OCU said on 19 June 2026 it was still missing) [I S-L8, S-L9]. The EU's ODR link is no longer needed: the platform closed on 20 July 2025 [I S-L11] | Paddle covers the checkout, refunds and withdrawals (its buyer terms: 14 days, a full refund unless the buyer started using it and agreed to that, S-L10); YoTech's terms and pricing page; a lawyer checks the fit |
| 9 | Terms of service (Ley 7/1998) | transparent and accepted; for consumers no exclusion of liability for death or injury or of conformity remedies (TRLGDCU art. 86) [V S-L7]; for businesses a cap (for example 12 months' fees) and no indirect loss [I]; the lapse (read-only for 60 days, deletion after notice, export the whole time); the Data Act's switching terms | lawyer |
| 10 | Data Act, chapter VI | applies to SaaS since 12 Sep 2025, with no small-company exemption for new contracts; at most 2 months' notice to switch, a 30-day transition, at least 30 days to take the data out; switching charges at cost until 12 Jan 2027, then none [V S-L12; I details]. Hersir's local data is always complete, and a restore gives back the cloud copy decrypted: say so in the terms and on an export page | in-house, in the terms |
| 11 | Digital Services Act | a hosting service: points of contact, rules on content in the terms, a notice-and-action form, statements of reasons (arts. 11-12, 14, 16-18); a micro or small company is spared the transparency reports [V S-L14, S-L15; I articles] | in-house |
| 12 | NIS2, European Accessibility Act | NIS2 covers "as a rule, medium-sized and large entities" [V S-L16]; the EAA spares micro-enterprises that provide services [I S-L17]. A file note, reviewed yearly | in-house |

The price of a lawyer's pack in Spain: no published range was found [I].

### 3.9 Question 8: The order, the cards, and what you do by hand

**What you can use first** [I]:

1. **Cloud Backup for your own computers**, with a plan given by hand (a comp entitlement), before any payment exists. It reuses 0.16.0's snapshot and restore almost whole, and it is the safety net under sync: a sync bug that damages data is undone from a cloud backup.
2. **Multiple devices for your own computers.** The sync engine (card 6) needs no server and can be built at the same time as Cloud Backup; sync through the cloud follows.
3. **Before the first customer**: payments and the account page (card 5), the legal pack, Zitadel Pro with the custom domain. Then sell Personal with Cloud Backup, with sync "coming to the same plan" (the plan's own advice: sell backup first, `PAID-FEATURES.md:196`).

The cards are in [section 6](#6-proposed-cards); the contracts between them in [section 5.4](#54-contracts).

**What you must do by hand** (no agent can: accounts, money, keys, signatures):

| # | Where | What |
|---|---|---|
| 1 | GitHub | a private repository `Yotech-AI/hersir-cloud`, and a deploy key for it |
| 2 | Hetzner | an account for NEXO IV GRUPO, S.L.; accept the AVV (DPA) in the console; a project; a CX23 server in Falkenstein or Nuremberg (order early: new servers have been scarce); an Object Storage bucket for customers' objects; an SSH key; S3 credentials into the server's secrets, never into the repository |
| 3 | A second provider | a bucket at Scaleway or Backblaze B2 for the server's own Litestream replica |
| 4 | Cloudflare DNS | records for `api.`, `account.` and `login.` on the domain you choose (question 10), DNS only (grey cloud); SPF, DKIM and DMARC for the sign-in and notice emails |
| 5 | Zitadel Cloud | an EU-region instance; a project with two apps: *Hersir desktop* (native, PKCE, redirect `http://127.0.0.1/callback`) and *Hersir account* (web); Google and GitHub sign-in (their OAuth apps made at Google Cloud and GitHub); passkeys on; an SMTP sender; Pro and the custom domain `login.` before the first customer |
| 6 | Keys | make the Ed25519 key that signs entitlements and the server's token key, offline; the private keys into the server's secrets and your password manager; the entitlement public key goes into Hersir's code by card 3 |
| 7 | Monitoring | healthchecks.io checks; an uptime check and status page |
| 8 | Paddle | a sandbox and a live account; business and identity checks; the domain review (the website with prices, features, terms naming NEXO IV GRUPO, S.L., refund and privacy pages); the product with a monthly and a yearly price excl. tax; an API key, a client-side token, the webhook destination and its secret; payment recovery and portal settings; payouts in EUR to the Spanish IBAN; ask for pricing under $10 |
| 9 | Your asesor fiscal | confirm the VAT treatment of the sale to Paddle (art. 69 LIVA, the Modelo 303 box), Paddle's self-billing, that no OSS registration is needed, and ROI for EU suppliers |
| 10 | A lawyer | the terms (business and consumer), the DPA review, the fit with Paddle, the withdrawal button |
| 11 | The website (`gizai-website`) | the pricing page and the legal pages; the legal notice of item 1 in 3.8 |

## 4. Options

Three ways to build both features [I; sizes in agent-days, like `PAID-FEATURES.md`]:

| | **A. End-to-end, a small server of our own** (recommended) | B. End-to-end, Cloudflare without a server | C. A readable master copy on the server (the 9 Oct choice) |
|---|---|---|---|
| What it is | computers chunk, encrypt and merge; one Hetzner server orders and stores ciphertext | the same protocol on Workers, one Durable Object per workspace, R2 and D1, all `jurisdiction: eu` | the server keeps each customer's database, applies every change with `gizai-core`, makes the daily snapshots and settles conflicts |
| Server work | accounts, devices, plans, objects, snapshots, the transaction log, claims: about 8 to 10 days | the same in TypeScript: about 7 to 9 days | the same plus applying and migrating every customer's database and server-side snapshots: about 15 to 20 days |
| App work | encryption, chunks, upload and restore (4 to 6); the sync engine (8 to 12); sync through the cloud (4 to 6); screens (4 to 6) | the same | no encryption, but the same sync engine on both ends |
| Libraries | `chacha20poly1305`, `hkdf`, `hmac`, `ed25519-dalek`, `fastcdc` (all MIT or Apache-2.0); server: `axum`, `rusqlite`, `rusty-s3` (BSD-2), `jsonwebtoken` (MIT); Litestream (Apache-2.0) | Workers runtime, TypeScript, Cloudflare's SDKs | envelope encryption with per-customer keys; the same server crates plus `gizai-core` |
| Risks | a lost recovery code with no computer left; Hetzner's capacity and 2026 price rises; you run a server | a US company in the data path (ciphertext only); lock-in; Rust on Workers is rough | plaintext of your customers' clients on one server; breaches reach their clients; every app release with a migration needs a server release first |
| Monthly cost at 10 / 100 / 1,000 customers | about €14 / €14 / €71 | about €5 / €8 / €78 | as A, plus encryption keys and more compute |
| Total | **about 30 to 40 agent-days** | about 30 to 40 | about 40 to 50 |

**Recommended: A**, because it keeps the server small, schema-free and in one EU company, keeps your customers' clients unreadable to YoTech, and stays in Rust; B is the fallback if running a server becomes the bottleneck. C only makes sense when a web view or phone access to all data becomes the product.

## 5. The design (option A)

### 5.1 The screens

A new Settings tab, **Hersir Cloud** (`#/settings/cloud`, after GitHub and Bitbucket in `SettingsPage.tsx:33-36`), and a *Restore from Hersir Cloud* choice on the first screen of a new install (`src/pages/OnboardingPage.tsx`). The local Backups section stays in General as it is.

Signed out:

```
Hersir Cloud
  Encrypted backups of your data, kept off your computer. Restore them on a new one.
  Personal: €5 a month or €48 a year, excl. VAT. Hersir can't read what it stores.
  [Sign in]  [Plans ↗]                        Signing in opens your browser.
```

After the first sign-in, before anything is uploaded (once per account):

```
Save your recovery code
  Hersir encrypts your data on this computer before it leaves. Only your computers and this code
  can read it: lose every computer and this code, and nobody can open your cloud backups, Hersir included.

      HC-7K2Q-M4XD-9TRP-B6WN-3HFJ-Q8YC          (mono)
  [Copy]  [Save as a file…]
  Type its last group to show you have it:  [____]                 [Turn on Cloud Backup]
```

Signed in:

```
Hersir Cloud
  Account        jeffrey@… · Personal, renews 9 Nov 2027              [Manage plan ↗]  [Sign out]
  Storage        ▓▓▓▓░░░░░░░░░░░░  1.4 of 10 GB
  Recovery code  Saved on 9 Nov 2026                                  [Show recovery code]

Cloud Backup
  Back up        (•) Daily   ( ) Only when I ask       [x] Also when I quit Hersir completely
  Last backup    Today 09:02 · 38 MB sent · next tomorrow 09:02       [Back up now]
  Backups        Today 09:02       Daily · this computer      1.2 GB  [Restore]
                 Yesterday 18:40   On quit · Laptop           1.2 GB  [Restore]

Your computers                                                          (Multiple devices)
  Desktop · this computer      synced 1 minute ago
  Laptop                       synced 2 hours ago
  [x] Keep my clients, projects and cards in sync on my computers
```

A new computer (the first screen, or Settings → Hersir Cloud while signed out):

```
Restore from Hersir Cloud
  1  Sign in              [Sign in]   opens your browser
  2  Your recovery code   [HC-____-____-____-____-____-____]
  3  Pick a backup        Today 09:02 · Desktop · 1.2 GB      [Restore]
     Hersir restarts to finish, as a restore from a folder does.
```

Elsewhere, for Multiple devices: a card claimed by another computer shows "Running on Laptop" where a live run would show; a card made offline shows its number as "not synced yet"; an agent's form gets *Runs on*; a Team Lead chat on another computer says "The Team Lead works on Desktop. Chat there."; a computer on an older Hersir gets a band: "Update Hersir to go on syncing: Laptop uses a newer version."

**How it fits the design system** (`design/README.md`) [V components; I use]: `FormSection` and `Field` as every Settings section; buttons `btn`, `btn primary`, `btn ghost`, `btn sm`; the radios and check of `BackupSettings.tsx`; its `backup-list` rows for the cloud list; its `ask` dialog for Restore's question; `gh-line` status lines with the step and `LoaderCircle`; `ok-banner` and `error-banner`. The recovery code and sizes in mono. Blue `accent` for actions; gold `needs` only for "Save your recovery code" and "Update Hersir", which need you; teal `live` never (no agent works here); `danger` for failures. Sentence case, buttons that say what happens. Each state in dark and light.

### 5.2 The data

**On the computer** (Cloud Backup): this computer's settings (added to `scope::THIS_COMPUTER`, so a restore keeps them): `cloud_account` (`{ id, email }`), `cloud_backup` (`{ on, every: "daily"|"manual", onQuit }`), `cloud_last_backup`, `cloud_last_error`. Keychain: `cloud/device` (the device token) and `cloud/key` (the account key). Multiple devices adds `sync_log`, `sync_clock` and `cloud_link` (3.7) and the synced column `agent_configs.home_device_id`, in migration `0022_sync.sql`.

**On the server** (`hersir-cloud`, SQLite):

```
accounts(id, zitadel_sub UNIQUE, email, created_at)
devices(id, account_id, name, platform, app_version, token_hash, created_at, last_seen_at, revoked_at)
keys(account_id PRIMARY KEY, key_id, sealed_key, created_at)          -- AK sealed by the recovery code
subscriptions(account_id, paddle_subscription_id, plan, status, current_period_end, updated_at)
comps(account_id, plan, valid_until, note)                            -- plans given by hand
objects(account_id, id, bytes, created_at, PRIMARY KEY(account_id, id))
snapshots(account_id, name, made_at, device_id, reason, schema_version, app_version, bytes, manifest_id, created_at)
snapshot_objects(account_id, snapshot, object_id)
webhook_events(id PRIMARY KEY, received_at)
-- Multiple devices
workspaces(id, account_id, created_at)                                -- id = the app's orgs.id
txs(workspace_id, seq, id UNIQUE, device_id, bytes, body BLOB, created_at, PRIMARY KEY(workspace_id, seq))
claims(workspace_id, task_id, device_id, run_id, expires_at, PRIMARY KEY(workspace_id, task_id))
```

Object storage: `objects/<account id>/<object id>`; the bytes are `HC1` (3 bytes) ‖ a 24-byte nonce ‖ ciphertext and tag.

### 5.3 What changes in which files

| Where | What |
|---|---|
| new `crates/gizai-cloud-client` (no Tauri) | `api.rs` (HTTP with `ureq`, as `oauth.rs`); `keys.rs` (AK, HKDF, the recovery code with a check digit, sealing); `chunks.rs` (page chunks, FastCDC for big blobs, zstd, seal and open); `backup.rs` (upload a snapshot); `restore.rs` (download into a format-1 folder); `entitlement.rs` (Ed25519 check) |
| `crates/gizai-agents/src/oauth.rs` | an entry point for a fixed issuer (OpenID discovery, a fixed client id) and an option to leave out `resource` |
| `crates/gizai-core/src/backups.rs` | a page-for-page copy (rusqlite's `backup` feature) beside `VACUUM INTO`; the manifest stays format 1 |
| `crates/gizai-core/src/scope.rs` | the cloud settings as this computer's |
| `src-tauri/src/cloud.rs` (new), `lib.rs` | the commands, the schedule (on the ticks of `backups.rs`), On quit, the event `cloud-changed` |
| `src/api.ts`, `src/types.ts`, `src/router.ts` | the commands and types; `cloud` in `SETTINGS_TABS` |
| `src/components/CloudSettings.tsx` (new), `src/pages/SettingsPage.tsx`, `src/pages/OnboardingPage.tsx` | the tab and *Restore from Hersir Cloud* |
| later: `crates/gizai-core/src/sync/` (new), `migrations/0022_sync.sql`, `src-tauri/src/sync.rs`, `workflow.rs`, `runs.rs` | the sync engine, its loop, the home device and claims |

### 5.4 Contracts

**The server's API** (cards 2, 3, 5 and 7). JSON over HTTPS, `https://api.hersir.dev/v1` (the plan's name); `Authorization: Bearer <access token>`; errors `{ "error": "<code>", "message": "<a sentence for people>" }` with 401 (signed out or removed), 402 (`plan_required`, `read_only`), 409 (`claimed`, `exists`), 413 (`quota`), 426 (`update_hersir`).

| Call | Body → answer |
|---|---|
| `POST /v1/devices` (with the Zitadel token) | `{ device_id, name, platform, app_version }` → `{ account_id, device_token }` |
| `POST /v1/token` (with the device token) | `{}` → `{ access_token, expires_at }` (15 minutes, Ed25519-signed by the server) |
| `GET /v1/devices`, `DELETE /v1/devices/:id` | → `[{ id, name, platform, app_version, last_seen_at, this }]`; 204 |
| `GET /v1/me/entitlements` | → `{ payload, sig }`; payload `{ v: 1, account, plan: "personal"\|"none", status: "active"\|"past_due"\|"lapsed"\|"none", features: ["backup","sync"], quota_bytes, used_bytes, valid_until, read_only_until, issued_at }` |
| `GET /v1/keys`, `PUT /v1/keys` | → `{ key_id, sealed_key }` or 404; `{ key_id, sealed_key }` → 201, or 409 when one is set |
| `POST /v1/objects/missing` | `{ ids: [hex64] }` → `{ missing: [hex64] }` |
| `POST /v1/objects/upload` | `{ objects: [{ id, bytes }] }` → `{ urls: [{ id, url, expires_at }] }` (a presigned PUT with its length signed; 402, 413) |
| `POST /v1/objects/download` | `{ ids }` → `{ urls: [{ id, url, expires_at }] }` (allowed while read-only) |
| `POST /v1/snapshots` | `{ name, made_at, device_id, reason, schema_version, app_version, manifest_id, object_ids, bytes }` → 201 (every object must be there) |
| `GET /v1/snapshots`, `DELETE /v1/snapshots/:name` | → `[{ name, made_at, device_id, device_name, reason, schema_version, app_version, bytes, manifest_id }]`, newest first; 204 |
| later: `POST /v1/workspaces/:id/txs` | `{ txs: [{ id, device_id, body }] }` (body: base64 ciphertext of one transaction) → `{ acked: [{ id, seq }] }`; 426 when a newer client set the workspace's minimum |
| later: `GET /v1/workspaces/:id/txs?after=<seq>&wait=30` | → `{ txs: [{ seq, id, device_id, body }], next }` (waits up to 30 seconds for new ones) |
| later: `POST /v1/workspaces/:id/claims`, `PUT`/`DELETE /v1/workspaces/:id/claims/:task_id`, `GET /v1/workspaces/:id/claims` | `{ task_id, run_id, lease_ms }` → 201 `{ expires_at }` or 409 `{ device_id, device_name, expires_at }`; renew; release; `[{ task_id, device_id, device_name, expires_at }]` |

**Hersir's commands for the Hersir Cloud tab** (cards 3 and 4), in `src/api.ts` with the Tauri command in brackets:

```ts
type CloudPlan = { name: "personal" | "none"; status: "active" | "past_due" | "lapsed" | "none";
  validUntil: number | null; readOnlyUntil: number | null; quotaBytes: number; usedBytes: number };
type CloudStep = "copying" | "chunking" | "uploading" | "registering" | "downloading" | "checking";
type CloudStatus = {
  signedIn: boolean; email: string | null; signingIn: boolean;
  plan: CloudPlan | null;                       // from the signed entitlement; null while signed out
  key: "missing" | "here" | "needed";           // missing: none made yet; needed: the account has one, this computer doesn't
  recoverySavedAt: number | null;
  backup: { on: boolean; every: "daily" | "manual"; onQuit: boolean;
            running: { step: CloudStep; done: number; total: number } | null;
            last: { at: number; name: string; sentBytes: number } | null; next: number | null;
            lastError: { at: number; message: string } | null };
};
type CloudBackup = { name: string; madeAt: number; device: { id: string; name: string; this: boolean };
  reason: "daily" | "manual" | "quit" | "before-restore"; bytes: number; restorable: boolean; problem: string | null };

cloudStatus(): Promise<CloudStatus>                                   // cloud_status
cloudSignIn(): Promise<CloudStatus>                                   // cloud_sign_in: opens the browser, waits up to 10 min
cloudCancelSignIn(): Promise<void>                                    // cloud_cancel_sign_in
cloudSignOut(): Promise<CloudStatus>                                  // cloud_sign_out
cloudMakeKey(): Promise<{ recoveryCode: string }>                     // cloud_make_key: first computer only
cloudConfirmRecovery(lastGroup: string): Promise<CloudStatus>         // cloud_confirm_recovery: turns Cloud Backup on
cloudEnterRecovery(code: string): Promise<CloudStatus>                // cloud_enter_recovery: a computer whose key is "needed"
cloudShowRecovery(): Promise<{ recoveryCode: string }>                // cloud_show_recovery
cloudSaveSettings(s: { on?: boolean; every?: "daily" | "manual"; onQuit?: boolean }): Promise<CloudStatus>  // cloud_save_settings
cloudBackUpNow(): Promise<CloudStatus>                                // cloud_back_up_now
cloudListBackups(): Promise<CloudBackup[]>                            // cloud_list_backups
cloudStartRestore(name: string): Promise<RestoreStatus>               // cloud_start_restore: then GA-218's restore, its events and Restart now
cloudOpenAccount(): Promise<void>                                     // cloud_open_account: the account page in the browser
onCloudChanged(f: (s: CloudStatus) => void): Promise<() => void>      // event "cloud-changed"
```

Refusals are sentences, as in the backups contract: "Sign in to Hersir Cloud first.", "That recovery code doesn't open this account's backups.", "Hersir Cloud is full: 10.0 of 10 GB.", "Your plan ended on 9 Dec 2026: backups can be restored until 7 Feb 2027.".

**Hersir's commands for Multiple devices** (cards 7 and 8):

```ts
type SyncStatus = { on: boolean; lastSyncAt: number | null; toSend: number;
  problem: null | { kind: "offline" | "update_hersir" | "plan" | "error"; message: string };
  computers: { id: string; name: string; lastSeenAt: number | null; this: boolean }[] };
syncStatus(): Promise<SyncStatus>               // sync_status
syncSet(on: boolean): Promise<SyncStatus>       // sync_set
onSyncChanged(f: (s: SyncStatus) => void)       // event "sync-changed"
// on a card (types.ts Task): runningOn: { deviceId: string; deviceName: string } | null; numberSynced: boolean
// on an agent (AgentConfig): homeDeviceId: string | null   (saved with the agent's form)
```

### 5.5 Security and privacy

- Keys only in the keychain, wiped from memory after use (`zeroize`), never in errors or logs (as `oauth.rs` keeps tokens out of them). The recovery code is shown on request on a signed-in computer.
- The server sees ciphertext, keyed object names, sizes, times and device names; snapshot manifests are encrypted. Logs hold ids and sizes, never content or email bodies.
- Device tokens are random, stored hashed, revocable from the account page; access tokens last 15 minutes. Presigned URLs last minutes, with the length signed; computers never get delete URLs: only the server deletes, by its retention and quota rules.
- The server: SSH keys only, a firewall (443, and 22 from your addresses), unattended security updates, secrets in systemd credentials, not in the repository. Cloudflare in DNS only.
- A stolen laptop with a running Hersir has the key: remove it on the account page, then make a new recovery code. (Changing the account key itself is left for later.)

### 5.6 How it is tested

- **The client crate**: test vectors for HKDF and the recovery code; seal and open; restore(backup(db)) equals db; an unchanged database uploads no new chunks; an upload cut halfway resumes without sending a chunk twice; the entitlement signature (good, bad, expired). A fake server in the test (a small `axum` or `tiny_http` listener on 127.0.0.1) and the memory keychain.
- **The app**: the commands under `scripts/ui-test.sh` with `GIZAI_FAKE_KEYCHAIN` and a `HERSIR_CLOUD_URL` pointing at the fake server; never a real account in tests.
- **The server**: integration tests with an S3 fake, Paddle webhook fixtures from the sandbox (signature, retries, out-of-order events), retention and garbage collection with a fake clock, the quota, the lapse dates.
- **Sync**: 3.7's property tests and fault injection, then two computers in real use.

### 5.7 What is left out

The phone chat; agents across devices; teams and SSO; a passphrase unlock, approval from another computer and passkey unlock (v2 candidates); changing the account key; server-side search or a web view of the data; a self-hosted server; bucket locks; packing chunks; Litestream on the desktop; sharing with clients.

## 6. Proposed cards

Eight cards: five for Cloud Backup (one is the design both features use), three for Multiple devices. Each is one pull request. **At the same time**: cards 1, 2, 3 and 6 can start together (2 and 3 meet at the contract in 5.4; 3 builds against a fake server); then 4 (after 1 and 3) and 5 (after 2); then 7 (after 2, 3 and 6); then 8 (after 1 and 7).

### Cloud Backup

**1. Design: Hersir Cloud in Settings, the recovery code, and a restore on a new computer**
- Column: Design. Role: design. Builds on: none.
- Builds: `design/hersir-cloud.md` and its screens for every state of 5.1: signed out, signing in, the recovery code, signed in, a backup running, a failure, the quota full, a lapsed plan, *Restore from Hersir Cloud* (first screen and Settings), the confirm and its steps; Multiple devices' parts (your computers, the sync switch and status, "Running on Laptop", "not synced yet", *Runs on*, the Team Lead note, the update band); the account page.
- Acceptance:
  - [ ] every state drawn, in dark and light, with the final texts
  - [ ] only existing components and tokens (5.1); gold only where you are needed, no teal
  - [ ] each screen names its component and the commands of 5.4 it shows

**2. Hersir Cloud server: accounts, computers, plans and encrypted backup storage**
- Column: To do. Role: backend. Builds on: none (your set-up, 3.9 items 1 to 6). New repository `hersir-cloud`.
- Builds: Rust, `axum`, SQLite with Litestream, `rusty-s3` presigning for Hetzner Object Storage; the calls of 5.4 up to `/v1/snapshots` (Zitadel token check, device tokens, signed entitlements with comp plans from an admin command, keys, objects, snapshots); retention (all for 7 days, then the newest of each day to 90) and garbage collection; the quota; the lapse dates; a deploy script, systemd unit and Caddy file; healthchecks pings; a README with the protocol.
- Acceptance:
  - [ ] every call of 5.4 (Cloud Backup part) with its errors, covered by tests against an S3 fake
  - [ ] a computer can't read another account's objects or snapshots; removed devices get 401
  - [ ] retention and garbage collection keep every object a kept snapshot uses (fake-clock test)
  - [ ] the entitlement verifies with the published public key
  - [ ] deployed on the Hetzner server; a restore drill of its database from Litestream written down and done once

**3. Cloud Backup in the app: sign in, encrypt, upload and restore**
- Column: To do. Role: backend. Builds on: card 2's contract (can be built at the same time, against a fake server).
- Builds: the crate `gizai-cloud-client` and `src-tauri/src/cloud.rs` of 5.3; sign-in through `oauth.rs`; the account key, recovery code and key id; the page-for-page copy, chunks, sealing and upload; daily, On quit and Back up now; *Restore from Hersir Cloud* into a format-1 folder and on through GA-218's restore; the commands and event of 5.4.
- Acceptance:
  - [ ] a backup made on one computer restores on a clean one (scratch data folders), with the device rule and this computer's settings kept
  - [ ] the second daily backup of an unchanged database uploads nothing but the manifest
  - [ ] a wrong recovery code is refused with a sentence; nothing unencrypted leaves the computer (a test reads what the fake server got)
  - [ ] a lapsed plan stops uploads and still restores
  - [ ] `cargo test --workspace`, `npm test`, `npm run build` pass on Linux, macOS and Windows CI

**4. Settings → Hersir Cloud**
- Column: To do. Role: frontend. Builds on: cards 1 and 3.
- Builds: `CloudSettings.tsx`, the tab, *Restore from Hersir Cloud* on the first screen, every state of card 1's design, on the commands of 5.4.
- Acceptance:
  - [ ] each state of card 1 shows from the matching `CloudStatus` (component tests)
  - [ ] the recovery code can't be skipped before the first backup
  - [ ] restore shows GA-218's steps and Restart now; `scripts/ui-test.sh` covers sign-in to restore against the fake server

**5. Paddle and the account page**
- Column: To do. Role: backend. Builds on: card 2 (and card 1's account page). Needed before the first customer, not for your own use.
- Builds: in `hersir-cloud`: the account pages (sign in with Zitadel; plan, storage, computers with Remove; Subscribe with Paddle's checkout; Manage plan with a portal link); Paddle webhooks to `subscriptions` and the entitlement; emails when a plan lapses, after 30 days and a week before deletion.
- Acceptance:
  - [ ] a sandbox subscription turns the plan on in the app within a minute; a cancelled one lapses at the period's end
  - [ ] webhooks are checked by signature and handled once each, in any order
  - [ ] removing a computer on the page signs it out

### Multiple devices

**6. Sync engine in the core**
- Column: To do. Role: backend. Builds on: none (no network; can start with cards 2 and 3).
- Builds: `crates/gizai-core/src/sync/` and `0022_sync.sql`: the table lists (synced, local, derived, with their device-only columns) and a test that every table is on one; the triggers and `sync_log`; the HLC; apply with last writer wins per field, row deletes, counters, the numbering rule and document conflicts; `agent_configs.home_device_id`; the property tests with faults of 3.7.
- Acceptance:
  - [ ] three replicas converge after random edits with drops, duplicates, reorders, a partition, a crash before an answer and a clock an hour behind
  - [ ] no two live cards share a number; a provisional number settles the same way everywhere
  - [ ] a transaction from a newer schema stops the apply; nothing in "never syncs" is ever captured
  - [ ] the activity feed shows another computer's changes as its own lines

**7. Sync through Hersir Cloud, with card claims**
- Column: To do. Role: backend. Builds on: cards 2, 3 and 6.
- Builds: on the server the workspace, transaction and claim calls of 5.4; in the app the sync loop (push, pull with wait, files by keyed hash, a new computer from the latest cloud backup and the log after it), the schema gate, the home device in the dispatcher and a claim before each start (10-minute lease, renewed); the commands and fields of 5.4.
- Acceptance:
  - [ ] two Hersirs (scratch data folders) against a local server converge after offline edits on both
  - [ ] a card runs on one computer only: the second claim gets 409 and its board says where it runs
  - [ ] a computer that dies mid-run frees the card when its lease runs out
  - [ ] offline, a synced workspace starts no card by itself

**8. Multiple devices on screen**
- Column: To do. Role: frontend. Builds on: cards 1 and 7.
- Builds: Your computers and the sync switch in Settings → Hersir Cloud; "Running on Laptop" on cards; "not synced yet" numbers; *Runs on* in the agent form; the Team Lead note; the update band; per card 1's design.
- Acceptance:
  - [ ] each state shows from `SyncStatus` and the card and agent fields (component tests)
  - [ ] `scripts/ui-test.sh` covers turning sync on and a claimed card

## 7. Open questions for Jefsev

1. **Encryption**: end-to-end for both Cloud Backup and sync, as the plan said, instead of the 9 Oct readable master copy? *Recommended: yes* (3.2).
2. **The key**: a recovery code only in v1 (no passphrase), approval from another computer later? *Recommended: yes.*
3. **The server**: Hetzner in Germany, Rust, with SQLite and Litestream instead of the plan's Postgres? *Recommended: yes*; Cloudflare (option B) if you'd rather run no server.
4. **The sign-in service**: Zitadel Cloud, EU region, Free for your own testing and Pro ($100 a month) with a custom domain before the first customer? *Recommended: yes*; Logto Cloud EU ($24 a month) only if a test shows it takes a changing loopback port and its DPA reads well.
5. **No magic link at launch** (Zitadel has none): passkeys, Google, GitHub and a password? *Recommended: yes*, if its email-code sign-in can't stand alone.
6. **Agents offline**: on a synced workspace, no automatic starts while Hersir Cloud can't be reached (Run by hand still works after a warning)? *Recommended: yes*; the alternative risks one card run twice.
7. **The order**: sell Personal with Cloud Backup once cards 1 to 5 and the legal pack are done, with sync "coming to the same plan"? *Recommended: yes*, as the plan advises.
8. **History**: every snapshot for 7 days, then one a day to 90 days, all inside the 10 GB? *Recommended: yes.*
9. **Payments**: Paddle, with the yearly price pushed (16% fees on the monthly price, 7% on the yearly)? *Recommended: yes*, and ask Paddle for its pricing under $10.
10. **The domain** for `api.`, `account.` and `login.`: the plan says `hersir.dev`. Do you own it? *Recommended: settle it before Zitadel's custom domain*, because passkeys stay bound to it.
11. **The server's code**: a private repository at first, with the protocol published (P4)? *Recommended: yes.*
12. **The Team Lead's chat on another computer**: read-only in v1? *Recommended: yes*; chatting from there is agents across devices.

## 8. Changes proposed to `docs/PAID-FEATURES.md` (not made)

1. **Line 22 and P1**: Personal now holds Cloud Backup and Multiple devices; the phone chat comes later.
2. **Lines 78-87 (the rules)**: end-to-end for backups *and* Personal sync, confirmed; a random account key with a recovery code instead of "Argon2id from a passphrase"; objects named by a keyed hash; why (3.2), with C-413/23 P.
3. **Line 108**: the server on Hetzner (Germany) with SQLite and Litestream instead of Postgres; Hetzner Object Storage, R2 as the alternative; Cloudflare for DNS only.
4. **Line 110**: Zitadel Cloud (EU) as the sign-in service; passkeys, Google, GitHub, password (no magic link); Hersir Cloud's own device tokens after sign-in.
5. **Line 113**: backups as a page-for-page copy cut into 256 KiB chunks, each compressed and encrypted; retention: every snapshot 7 days, daily to 90 days.
6. **Line 121**: Cloud Backup's state as this computer's settings in `scope.rs`; `cloud_link` comes with sync.
7. **Lines 128-139 (Phase 2)**: `changes` can't be the log sync reads (2.4): a trigger-written `sync_log`, a real HLC, numbers settled in the server's order, a home computer per agent and cloud claims, and the list of what never syncs.
8. **Line 167 and P2**: Paddle chosen; today's fees (5% + $0.50; Stripe Managed Payments is sold through Link, LLC at 3.5% on top of Stripe's fees).
9. **Line 171 and P3**: done: NEXO IV GRUPO, S.L. (Spain), trading as YoTech; VAT on the sale to Paddle under art. 69 LIVA, no OSS; the asesor confirms.
10. **Lines 172-178**: the Spanish legal list of 3.8 (LSSI-CE, LOPDGDD, TRLGDCU, the DPA from the Commission's clauses, the Data Act, the DSA).
11. **Line 187**: Hetzner's 2026 prices (CX23 €5.99, Object Storage €6.49 for 1 TB) and the cost table of 3.3.
12. **Decisions**: add P10 (sign-in service), P11 (server stack) and P12 (agents offline on a synced workspace).
13. **Phases 1 and 2's sizes**: replace with the eight cards of section 6 (about 30 to 40 agent-days together).

## 9. Sources

All read on 2026-11-09 by this card's research (the R&D Agent and its helpers), unless marked *search result only*.

**Encryption and GDPR**
- S-E1 Obsidian, Sync security: https://obsidian.md/help/sync/security
- S-E2 Apple, Advanced Data Protection: https://support.apple.com/en-us/102651
- S-E3 Bitwarden, log in with passkeys: https://bitwarden.com/help/login-with-passkeys/
- S-E4 1Password, Secret Key security: https://support.1password.com/secret-key-security/
- S-E5 Joplin, end-to-end encryption: https://joplinapp.org/help/apps/sync/e2ee
- S-E6 Actual Budget, sync: https://actualbudget.org/docs/getting-started/sync/
- S-E7 Standard Notes, encryption: https://standardnotes.com/help/security/encryption
- S-E8 Ente, architecture: https://ente.com/architecture/
- S-E9 Bitwarden, trusted devices: https://bitwarden.com/help/about-trusted-devices/
- S-E15 chacha20poly1305 on crates.io: https://crates.io/api/v1/crates/chacha20poly1305
- S-E16 RustCrypto chacha20poly1305 README (the NCC Group audit): https://github.com/RustCrypto/AEADs/tree/master/chacha20poly1305
- S-E17 age on crates.io: https://crates.io/api/v1/crates/age
- S-E18 RFC 9106 (Argon2): https://www.rfc-editor.org/rfc/rfc9106.html ; OWASP Password Storage Cheat Sheet: https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html
- S-E19 Court of Justice press release 107/25 (C-413/23 P, 4 Sep 2025): https://curia.europa.eu/site/upload/docs/application/pdf/2025-09/cp250107en.pdf
- S-E20 EDPB, 8 July 2026: https://www.edpb.europa.eu/news/edpb-sheds-light-on-anonymisation-and-web-scraping-for-generative-ai-and-adopts-final-version_en
- S-E21 AEPD breach guide (search result only): https://www.aepd.es/guias/guia-brechas-seguridad.pdf

**The server**
- S-S1 to S-S4 Hetzner Cloud: https://www.hetzner.com/cloud/cost-optimized/ , https://www.hetzner.com/cloud/regular-performance/ , https://www.hetzner.com/cloud/general-purpose/ , https://www.hetzner.com/cloud/
- S-S5 Hetzner backups and snapshots: https://docs.hetzner.com/cloud/servers/backups-snapshots/overview/
- S-S6 Hetzner Object Storage: https://www.hetzner.com/storage/object-storage/
- S-S7 Hetzner data protection (AVV): https://docs.hetzner.com/general/company-and-policy/data-protection-at-hetzner/
- S-S8 Hetzner price adjustments (404 today); secondary: https://northflank.com/blog/hetzner-cloud-server-price-increases , https://www.vincentschmalbach.com/hetzner-cheap-cloud-unavailable-price-increases/
- S-S10 Cloudflare customer DPA: https://www.cloudflare.com/cloudflare-customer-dpa/
- S-S11 the DPF appeal C-703/25 P (secondary): https://www.wilmerhale.com/en/insights/blogs/wilmerhale-privacy-and-cybersecurity-law/20251201-european-court-of-justice-to-review-challenge-to-eu-us-data-privacy-framework
- S-S12 D1 data location: https://developers.cloudflare.com/d1/configuration/data-location/
- S-S13 Durable Objects data location: https://developers.cloudflare.com/durable-objects/reference/data-location/
- S-S14 R2 data location: https://developers.cloudflare.com/r2/reference/data-location/
- S-S15 Workers pricing: https://developers.cloudflare.com/workers/platform/pricing/ ; R2 pricing: https://developers.cloudflare.com/r2/pricing/ ; D1 limits: https://developers.cloudflare.com/d1/platform/limits/
- S-S16 workers-rs: https://github.com/cloudflare/workers-rs
- S-S17 Scaleway pricing (403; search results only): https://www.scaleway.com/en/pricing/storage/
- S-S18 Sentry pricing: https://sentry.io/pricing/
- S-S19 healthchecks.io pricing: https://healthchecks.io/pricing/
- S-S20 Backblaze B2 pricing: https://www.backblaze.com/cloud-storage/pricing

**Accounts and sign-in**
- S-A1 Zitadel pricing: https://zitadel.com/pricing
- S-A2 Zitadel repository (licence): https://github.com/zitadel/zitadel
- S-A3 Zitadel, how daily active users are counted: https://help.zitadel.com/how-is-daily-active-users-calculated
- S-A4 Zitadel's loopback handling in source: https://raw.githubusercontent.com/zitadel/oidc/main/pkg/op/auth_request.go
- S-A5 Ory pricing: https://www.ory.com/pricing
- S-A6 Logto pricing: https://logto.io/pricing ; S-A7 Logto native clients in source: https://raw.githubusercontent.com/logto-io/logto/master/packages/core/src/oidc/utils.ts
- S-A8 Hanko pricing: https://www.hanko.io/pricing
- S-A9 Auth0 pricing: https://auth0.com/pricing
- S-A10 Clerk pricing: https://clerk.com/pricing ; S-A11 WorkOS pricing: https://workos.com/pricing
- S-A12 Kinde pricing: https://kinde.com/pricing/ ; FusionAuth pricing: https://fusionauth.io/pricing
- S-A13 Rauthy: https://github.com/sebadob/rauthy ; S-A14 Pocket ID: https://github.com/pocket-id/pocket-id ; S-A15 Keycloak loopback ports: https://github.com/keycloak/keycloak/issues/39880
- S-A16 search results only: Keycloak passkeys (keycloak.org/2025/09/passkeys-support-26-4), Logto data regions (blog.logto.io/data-region-resources), Supabase OAuth flows, Hanko discussion 860, Stytch data compliance, Descope multi-region
- S-A18 RFC 8252: https://www.rfc-editor.org/rfc/rfc8252.html
- S-A19 jsonwebtoken: https://crates.io/api/v1/crates/jsonwebtoken , https://docs.rs/crate/jsonwebtoken/latest

**Payments and tax**
- S-P1 Paddle pricing: https://www.paddle.com/pricing
- S-P2 Paddle seller terms: https://www.paddle.com/legal/terms
- S-P3 Paddle supported countries: https://www.paddle.com/help/start/intro-to-paddle/which-countries-are-supported-by-paddle ; acceptable use: https://www.paddle.com/help/start/intro-to-paddle/what-am-i-not-allowed-to-sell-on-paddle
- S-P4 Paddle webhooks: https://developer.paddle.com/webhooks/overview
- S-P5 Paddle domain review: https://www.paddle.com/help/start/account-verification/what-is-domain-verification
- S-P6 Paddle payouts: https://www.paddle.com/help/manage/get-paid/when-and-how-do-i-get-paid , https://www.paddle.com/help/manage/get-paid/is-there-a-fee-taken-for-payouts
- S-P7 Paddle payment recovery (search result only): https://developer.paddle.com/changelog/2025/default-dunning-payment-recovery/
- S-P8 Stripe Managed Payments eligibility: https://docs.stripe.com/payments/managed-payments/eligibility
- S-P9 Stripe Spain pricing: https://stripe.com/es/pricing
- S-P10 Stripe Managed Payments: https://stripe.com/managed-payments
- S-P11 how Managed Payments works: https://docs.stripe.com/payments/managed-payments/how-it-works , https://docs.stripe.com/payments/managed-payments
- S-P12 "Sold through Link, LLC": https://support.stripe.com/questions/why-is-sold-through-link-llc-the-legal-entity-on-my-invoice-for-stripe-managed-payments-fees
- S-P13 AEAT place-of-supply assistant (search result only): https://www2.agenciatributaria.gob.es/wlpl/AVAC-CALC/LocalizadorServicios2023?id=10.1.1.2
- S-P14 Implementing Regulation 282/2011, Art. 9a (UK copy, as EUR-Lex didn't render): https://www.legislation.gov.uk/eur/2011/282/article/9a
- Others: Polar https://polar.sh/resources/pricing , Creem https://www.creem.io/pricing , Dodo Payments https://dodopayments.com/pricing

**Legal**
- S-L1 LSSI-CE (BOE): https://www.boe.es/buscar/act.php?id=BOE-A-2002-13758
- S-L2 LOPDGDD (BOE): https://www.boe.es/buscar/act.php?id=BOE-A-2018-16673
- S-L3 AEPD FAQ on breach notification: https://www.aepd.es/preguntas-frecuentes/2-rgpd/11-brechas-de-datos-personales/FAQ-0233-a-quien-hay-que-notificar-las-brechas-de-datos-personales
- S-L4 AEPD tools: https://www.aepd.es/guias-y-herramientas/herramientas
- S-L5 AEPD contract guidelines (search result only): https://www.aepd.es/es/node/813
- S-L6 European Commission, standard contractual clauses for controllers and processors: https://commission.europa.eu/publications/standard-contractual-clauses-controllers-and-processors-eueea_en
- S-L7 TRLGDCU (BOE): https://www.boe.es/buscar/act.php?id=BOE-A-2007-20555
- S-L8 Directive 2023/2673 (did not render): https://eur-lex.europa.eu/eli/dir/2023/2673/oj/eng
- S-L9 OCU, 19 June 2026 (secondary): https://www.ocu.org/organizacion/prensa/notas-de-prensa/2026/normabotondesistimiento190626
- S-L10 Paddle checkout buyer terms: https://www.paddle.com/legal/checkout-buyer-terms
- S-L11 the ODR platform's closure (search result only): https://consumer-redress.ec.europa.eu/site-relocation_en
- S-L12 the Data Act explained: https://digital-strategy.ec.europa.eu/en/policies/data-act-explained ; S-L13 Digital package FAQ: https://digital-strategy.ec.europa.eu/en/faqs/digital-package
- S-L14 the DSA package: https://digital-strategy.ec.europa.eu/en/policies/digital-services-act-package ; S-L15 DSA Q&A: https://digital-strategy.ec.europa.eu/en/faqs/digital-services-act-questions-and-answers
- S-L16 NIS2: https://digital-strategy.ec.europa.eu/en/policies/nis2-directive
- S-L17 Ley 11/2023 (not read): https://www.boe.es/buscar/pdf/2023/BOE-A-2023-11022-consolidado.pdf
- S-L18 the DPF appeal (search result only): https://eur-lex.europa.eu/eli/C/2025/6610/oj/eng ; S-L19 NIS2 in Spain (secondary): https://nisd2.eu/en/wiki/timelines-and-status/nis2-status-spain

**Backup and sync engineering**
- S-B1 sqlite3_rsync (page-level differences): https://sqlite.org/rsync.html
- S-B2 restic design: https://raw.githubusercontent.com/restic/restic/master/doc/design.rst ; kopia architecture: https://kopia.io/docs/advanced/architecture/ ; rustic: https://github.com/rustic-rs/rustic
- S-B3 Kulkarni and others, Logical Physical Clocks (HLC), 2014: https://cse.buffalo.edu/tech-reports/2014-04.pdf ; uhlc: https://crates.io/api/v1/crates/uhlc
- S-B4 James Long, Using CRDTs in the wild (Actual Budget): https://archive.jlongster.com/using-crdts-in-the-wild
- S-B5 cr-sqlite: https://github.com/vlcn-io/cr-sqlite ; Linear's sync engine talk: https://linear.app/now/scaling-the-linear-sync-engine ; SQLite Sync: https://github.com/sqliteai/sqlite-sync ; PowerSync service licence: https://raw.githubusercontent.com/powersync-ja/powersync-service/main/LICENSE
- S-B6 proptest: https://crates.io/api/v1/crates/proptest
- S-B7 toxiproxy: https://api.github.com/repos/Shopify/toxiproxy/releases/latest
- S-B8 turmoil: https://crates.io/api/v1/crates/turmoil
- S-B9 Linear's sync engine, reverse-engineered (secondary): https://github.com/wzhudev/reverse-linear-sync-engine
- S-B10 fastcdc: https://crates.io/api/v1/crates/fastcdc ; rusty-s3: https://crates.io/api/v1/crates/rusty-s3 ; R2 presigned URLs: https://developers.cloudflare.com/r2/api/s3/presigned-urls/ ; R2 lifecycles: https://developers.cloudflare.com/r2/buckets/object-lifecycles/ ; Hetzner Object Storage actions: https://docs.hetzner.com/storage/object-storage/supported-actions/ ; S3 multipart limits: https://docs.aws.amazon.com/AmazonS3/latest/userguide/qfacts.html
- S-B11 Litestream v0.5: https://fly.io/blog/litestream-v050-is-here/

**In this repository** (read 2026-11-09): `docs/PAID-FEATURES.md`, `docs/research/paid-offering-2026-10.md`, `docs/backup-format.md`, `crates/gizai-core/src/{db,tasks,runs,scope,settings,columns,backups,restore,workflow}.rs`, `crates/gizai-core/migrations/`, `crates/gizai-agents/src/{oauth,secrets}.rs`, `src-tauri/src/{lib,backups}.rs`, `src/pages/SettingsPage.tsx`, `src/components/BackupSettings.tsx`, `design/README.md`; and `~/Herd/cordon/notes/E-sqlite-sync.md` (the 5 Oct sync research), `~/Herd/gizai-website/src/data/roadmap.ts:68-74` (the website's lines).
