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

### 3.2 Question 1: Encryption

**Recommendation: end-to-end for both Cloud Backup and Multiple devices** [I]. The server stores and orders ciphertext; your computers encrypt, decrypt and merge. Keep the rest of the 9 Oct choice: one workspace per customer in the EU, the cloud as the shared copy every computer syncs with, and a full local copy on every computer that works offline.

Why, now that the phone chat comes later [I]:

- **What the readable copy bought is out of scope now**: a phone or web view reading data through an API, snapshots made on the server, conflicts settled on the server.
- **It would hold your customers' clients in plaintext**: names, emails, phone numbers, IBANs and notes, on a server one person guards. Encryption at rest protects stolen disks and leaked backups, not a hacked app server, the operator or a court order (S-E14). A readable sync would also undo the backups' encryption: the same data would sit in plaintext beside them.
- **An end-to-end server never needs Hersir's schema.** It doesn't migrate anything, so a Hersir release with a migration needs no server release. A readable master copy must apply every change with `gizai-core` and migrate every customer's database in step with each app release: release coupling a one-person team pays for every time.
- **GDPR**: a breach of ciphertext whose keys only the customer holds is, in the main, a matter for the breach record, not for telling the people in it (Art. 34(3)(a), below).
- **It is what the plan, the website and comparable tools say**: `PAID-FEATURES.md:82-87`; "Encrypted backups of your data"; end-to-end by default at Obsidian, Standard Notes, Joplin, Actual Budget and Ente (S-E1 to S-E9).

#### How the keys work

| Key | What it is | Where it lives |
|---|---|---|
| Account key (AK) | 256 random bits, made by the first computer | each signed-in computer's keychain (`cloud/<account>/key`); never on the server as it is |
| Data key, id key, key id | derived from AK with HKDF-SHA256 | computed when needed; the key id (16 bytes) is on the server, to tell a wrong code from a right one |
| Recovery code | 128 random bits, shown as 26 characters in groups (`HC-7K2Q-…`) | on paper or in your password manager; the server keeps AK sealed with a key derived from it |

- **A new computer gets the key** by signing in and typing the recovery code (v1). Later: *Approve on your other computer*, a short code and a key exchange (X25519) through the server, as Signal links devices (S-E10). Passkeys (WebAuthn PRF) could add one more way, once support is less patchy than in 2026 (S-E11, S-E12).
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

- **With end-to-end**: the phone gets the key (or a chat-only key) by scanning a code on a computer, as WhatsApp and Signal link devices; push notifications carry a wake-up or a small encrypted payload that the phone decrypts (an iOS Notification Service Extension; an Android data message) (S-E10) [I].
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
| **A. Hetzner server + Hetzner Object Storage** (recommended) | one EU company; cheapest at scale (about €6.35 a TB against $15 on R2); keeps Rust | you run a server: about 1 to 3 hours a month (S-S9) |
| B. Hetzner server + Cloudflare R2 (EU jurisdiction) | R2 is mature, free egress, $0.015 a GB-month | a US sub-processor under the EU-US Data Privacy Framework (whose appeal, C-703/25 P, is pending) and SCCs (S-S10, S-S11) |
| C. Cloudflare Workers + one Durable Object per customer + R2, all with `jurisdiction: eu` | no server at all; cheapest at 10 to 100 customers; Durable Objects order a workspace's log by nature | US company; Durable Object ids are logged outside the jurisdiction; `workers-rs` "expect a few rough edges", D1 in Rust is alpha, so in practice TypeScript; lock-in (S-S12 to S-S16) |
| D. Scaleway (France) servers, storage and managed PostgreSQL | EU company with managed Postgres (about €11 a month) | its price pages returned 403 today (search snippets only) (S-S17) |

Cloudflare D1, Durable Objects and R2 all take a jurisdiction (`eu`) at creation, which can never change; location hints are "best effort" (S-S12 to S-S14).

#### Deploys, its own backups, monitoring [I, prices V]

- **Deploys**: GitHub Actions builds a static Linux binary on a tag; a script copies it to the server and restarts the systemd unit; the previous binary stays beside it for a rollback. Migrations of the server's database run at start, forward only, as in the app.
- **Its own backups**: Litestream replicates the server's SQLite to a bucket at a second provider (Scaleway or Backblaze B2), so one locked account can't take everything; Hetzner's 7 daily images on top. A monthly restore drill onto a throwaway server. The customers' objects stay in one location (they are backups already, and every computer keeps its data).
- **Monitoring**: healthchecks.io (free for 20 checks) for Litestream, the retention job and a minute-by-minute ping; an uptime check with a status page (Uptime Kuma on a second small server, or Better Stack's free tier); errors to email, or Sentry (from $26 a month, US company, a data-residency option) once there are paying customers. No customer data in logs: ids and sizes only (S-S18 to S-S20).

#### Cost a month (EUR, excl. VAT) [I, from the prices above, S-S9]

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
