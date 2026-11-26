# Backups: format 1

Hersir backs up your data as snapshots in a backups folder (GA-216). This page describes that folder as format 1: its layout, a snapshot's name, the manifest field by field, and how a snapshot is made and removed. Hersir Cloud's backups keep format 1's manifest, and a cloud restore writes a format-1 folder (see [For Hersir Cloud](#for-hersir-cloud)), so one restore reads both. The restore is GA-218's (see [Restore](#restore)). The code is `gizai_core::backups` and `gizai_core::restore`, and the app's side is `src-tauri/src/backups.rs` and `src-tauri/src/restore.rs`.

A snapshot holds only the database and the stored files it names (attachments and screenshots). Nothing from the OS keychain, a worktree, the logs (`runs/`, `chat/`), the Chief's code copies or `update/` goes into it. Snapshots are not encrypted: whoever can open the folder can read them, and a synced folder gets the data as it is.

## The backups folder

Settings → General → Backups names a folder: a USB disk, or a folder Dropbox or Syncthing keeps.

- A folder that holds `gizai-backups.json` is a backups folder, and Hersir uses it as it is, whatever its name.
- Any other folder gets a backups folder inside it, named `Hersir backups`. The first snapshot makes it.
- An older Hersir named that folder `Gizai backups`. It is still found and used as it is, when it holds `gizai-backups.json` and the folder it is in has no `Hersir backups`: snapshots go on there, the list and a restore read it, and nothing is renamed or moved. When both are there, `Hersir backups` is used.
- A folder given to list the snapshots in, or to restore from, is read the same way: the folder itself when it holds `gizai-backups.json`, else the backups folder inside it, as above.

```
Hersir backups/
  gizai-backups.json             {"format":"gizai-backups","format_version":1}
  README.txt                     what this folder is, and: Restore in Hersir: Settings → General → Backups.
  snapshots/
    <name>.json                  the manifest: a snapshot exists once this file does
    <name>.db.zst                its database, compressed with zstd
  files/<sha256[..2]>/<sha256>   every stored file, once, shared by all snapshots (as in the data folder)
```

- `gizai-backups.json` and `README.txt` are written once, by the first Hersir that backs up there. A Hersir whose format is older than the folder's `format_version` doesn't back up there; it says a newer Hersir made the folder.
- Several computers may share one folder. Each writes and removes only its own snapshots; the files in `files/` are shared by all. A computer is known by its `devices.id` (the row with `is_self = 1`), so a restore must keep the restoring computer's own row: two computers with one id would take each other's snapshots for their own.
- A file in `files/` is named by the SHA-256 of its content, in lowercase hex, inside a folder named by its first two characters, as in the data folder. It only ever arrives whole (renamed from a `.part` file) and never changes.

## A snapshot's name

`gizai-<reason>-<stamp>-<device>`, for example `gizai-daily-20261021-090200-123-9f3c2a1b`.

| Part | What it is |
|---|---|
| reason | `daily`, `weekly`, `manual` (Back up now), `quit` (Quit Hersir completely with On quit on) or `before-restore` (made just before a restore) |
| stamp | when the database was copied, in the computer's local time (`db::stamp`): year, month and day, then hours, minutes and seconds, then milliseconds |
| device | the last 8 characters of the computer's `devices.id`, letters and digits only. Ids are UUIDv7, so their first characters are their creation time and their last ones are random. |

A snapshot is two files in `snapshots/`: `<name>.json`, its manifest, and `<name>.db.zst`, its database. It exists once its manifest does.

## The manifest

```json
{
  "format": "gizai-backup",
  "format_version": 1,
  "name": "gizai-daily-20261021-090200-123-9f3c2a1b",
  "reason": "daily",
  "made_at": 1792573322123,
  "app_version": "0.13.0",
  "schema_version": 20,
  "device": { "id": "<devices.id>", "name": "<devices.name>" },
  "db": { "path": "snapshots/<name>.db.zst", "compression": "zstd", "bytes": 0, "sha256": "<sha256 of the uncompressed database>", "db_bytes": 0 },
  "last_change": { "seq": 0, "id": "<changes.id>", "hlc": "<changes.hlc>" },
  "files": [ { "sha256": "<64 lowercase hex>", "bytes": 0 } ],
  "files_missing": [ "<sha256>" ]
}
```

Hersir writes it as indented JSON in UTF-8, its fields in this order.

| Field | What it holds |
|---|---|
| `format` | always `"gizai-backup"` |
| `format_version` | `1`. A Hersir that reads a higher one lists the snapshot but doesn't restore it. |
| `name` | the snapshot's name, as above: the same as its files' names |
| `reason` | `daily`, `weekly`, `manual`, `quit` or `before-restore` |
| `made_at` | when the database was copied: milliseconds since 1970 (UTC) |
| `app_version` | the Hersir version that made it |
| `schema_version` | the copied database's schema version (`PRAGMA user_version`). A Hersir whose own is lower doesn't restore it. |
| `device.id`, `device.name` | the computer that made it: its `devices` row with `is_self = 1` |
| `db.path` | `snapshots/<name>.db.zst`, from the backups folder |
| `db.compression` | `"zstd"` (level 3, one zstd frame that records the database's size) |
| `db.bytes` | the compressed file's size |
| `db.sha256` | the SHA-256 of the uncompressed database, in lowercase hex |
| `db.db_bytes` | the uncompressed database's size |
| `last_change` | the copied database's newest `changes` row (the highest `seq`): its `seq`, `id` and `hlc`. `null` when it has none. It is where a sync continues. |
| `files` | every blob the copied database names, each once, sorted by hash: the `sha256` of each `files` row (removed ones too, as their blob is kept), and each `artifact_versions` row's `sha256` and `thumbnail_sha256`. `bytes` is its size; for a missing one, the size its `files` row gives, else 0. |
| `files_missing` | the hashes among `files` the snapshot couldn't store: neither the backups folder nor the data folder had them, or the data folder's copy no longer matched its hash. The snapshot is still made, and says so here. |

The uncompressed database is a complete SQLite database in one file (journal mode DELETE, no `-wal` beside it).

## How a snapshot is made

One snapshot runs at a time on a computer, off the main thread, and Hersir stays usable meanwhile. In this order, so an interrupted snapshot leaves no manifest, only `.part` files:

1. **The folder:** the backups folder, `gizai-backups.json`, `README.txt`, `snapshots/` and `files/` are made when they're missing. Then this computer's `.part` files that are more than an hour old go (see below).
2. **The database:** `VACUUM INTO` a temporary copy, `<data folder>/backups/.snapshot-<name>.db`, as the copies before updates are made. It is consistent, while Hersir goes on writing.
3. **The files:** each blob the copy names goes into `files/`. One the folder has already isn't copied: its modified time is set to now (see Keep). Any other is copied from the data folder as `files/<xx>/<sha256>.<device>.part`, hashed while it is copied, then renamed. One the data folder doesn't have goes into `files_missing`.
4. **The compressed database:** the copy is compressed with zstd (level 3) into `snapshots/<name>.db.zst.part`, hashed while it is read, then renamed.
5. **The manifest:** written as `snapshots/<name>.json.part`, then renamed. The temporary copy is removed. The snapshot exists from now on.
6. **Keep:** see below.

A failure removes this snapshot's `.part` files, its compressed database and the temporary copy, and is kept as the last error (Settings shows it until a snapshot works). A snapshot that was interrupted (Hersir was killed, the computer went off) leaves its `.part` files: the next snapshot on that computer removes them once they are more than an hour old, with a compressed database whose manifest never came and its temporary copy. A `.part` file is always one computer's own: its name ends in that computer's device.

## When snapshots are made

- **Back up now**, with reason `manual`.
- **Daily or weekly** (Every in Settings; daily by default): due when this computer's newest good snapshot in the folder is 24 hours or 7 days old. A good snapshot is one whose manifest reads and whose database file is there, of any reason but `before-restore`, and not dated after now (made while the clock was ahead); with none, the first one is due at once. Hersir checks 2 minutes after it starts and then every hour by the wall clock, which it looks at every 5 minutes, so the hours a computer slept count too. A check up to 5 minutes early counts, so the snapshot doesn't move an hour later each day. A folder that can't be reached now (a USB disk that isn't plugged in) is passed over quietly, and Settings shows why.
- **On quit** (off by default): Quit Hersir completely (the tray's or Settings' Quit, Cmd+Q on macOS) makes a snapshot with reason `quit` once agents at work have stopped, before Hersir exits, waiting at most 5 minutes. A signal, logging out, shutting down, and the restart after an update or a restore don't back up.
- **Before a restore** (GA-218), with reason `before-restore`: in the backups folder set in Settings, or in the folder restored from (see [Restore](#restore)). `gizai_lib::backups::back_up_within` makes it.

## Keep: how snapshots and files are removed

After each snapshot:

1. This computer's snapshots of reasons `daily`, `weekly`, `manual` and `quit`, beyond the newest Keep (1 to 100, 10 by default), are removed: the manifest first, then its `.db.zst`. Never another computer's snapshot, and never a `before-restore` one. A snapshot dated after now (made while the clock was ahead) counts as the oldest, so it never takes the place of the one just made.
2. Then a blob in `files/` is removed when no manifest in the folder names it (`.part` manifests included) and it hasn't been modified for 7 days. So the blobs another computer's snapshot has found while it runs stay. When a manifest can't be read, no blob goes that time (it may name any), unless it is a `.part` manifest more than an hour old.

## Restorable or not

Listing a folder shows each snapshot, newest first, and whether this Hersir can restore it. It can't, and says why, when a newer Hersir made it (its `format_version` or `schema_version` is higher than this Hersir's), when its manifest can't be read as format 1's, when its database file is missing, or when files it names are missing from `files/` ("3 files are missing"). A manifest that can't be read whole is read field by field, so a newer Hersir's snapshot still shows; Keep and the schedule only count manifests that read whole.

## Settings

This computer's, kept in its database; a restore keeps them. Saving checks Keep and a newly chosen folder (a full path Hersir can write in); the folder saved already isn't checked again, so an unplugged USB disk doesn't keep the other settings from changing.

| Key | Value |
|---|---|
| `backup_folder` | the chosen folder, a full path, or `null`: backups are off (the default) |
| `backup_every` | `"daily"` (the default), `"weekly"` or `"manual"` |
| `backup_on_quit` | `true` or `false` (the default) |
| `backup_keep` | 1 to 100 (10 by default) |
| `backup_last_error` | the last snapshot that failed, `{ "at", "message" }`; `null` once one has been made since |

## Restore

A restore (GA-218) brings a snapshot back as this computer's data. It never makes two computers share one device id, this computer's own settings stay this computer's, and Hersir notes which snapshot it came back from, so a sync knows where this computer's data starts. The same rules hold for sync across devices. Nothing in the OS keychain is restored or removed: the restored data asks for the keys this computer doesn't have, as a new install does.

`start_restore(folder, name)` runs in the background, one restore at a time, and is refused while a backup or another restore runs. `folder` is read as for listing (the folder itself when it holds `gizai-backups.json`, else the backups folder inside it, as [The backups folder](#the-backups-folder) says). Each step goes out in `restore-changed` with the status (contract BR's `RestoreStatus`). A restore that is ready already gives way to a new one.

### 1. The checks

Each failure stops the restore with its reason, and nothing is staged:

- the manifest is a snapshot's (`format` is `gizai-backup`) and reads as format 1's;
- its `format_version` is 1 or lower, and its `schema_version` is at most this Hersir's (`db::SCHEMA_VERSION`). A newer one is refused with `Made by a newer Hersir (database version N). Update Hersir first.` (for the format: `backup format N`);
- the database file is there and decompresses, its SHA-256 and size match `db.sha256` and `db.db_bytes`, it passes `PRAGMA integrity_check`, and its `user_version` is the manifest's `schema_version`;
- every blob in `files` is in the folder's `files/`, with its size and its SHA-256. A snapshot with `files_missing` is refused too: those blobs aren't there.

The database is decompressed into `<data folder>/restore/gizai.db.part` for the checks.

### 2. Today's data is saved

A snapshot of today's data with reason `before-restore`, made as any snapshot is: in the backups folder set in Settings, or, when none is set (or it can't be written now), in the folder restored from. A restore from Hersir Cloud uses the data folder's own `backups/` in place of the folder restored from, because its download (`restore/cloud/`) goes with the restore. It waits at most 5 minutes for a snapshot that runs already. When neither can be written, the restore stops: `Hersir can't save today's data first: <why>. Choose a backups folder in Settings → General → Backups.` With Cloud Backup on, today's data also goes to Hersir Cloud as a `before-restore` backup; when that fails, the restore goes on.

### 3. Staging

```
<data folder>/restore/
  gizai.db                    the snapshot's database, decompressed and checked
  files/<sha256[..2]>/<sha256>  the blobs the data folder's files/ lacks, copied and hashed again
  restore.json                the manifest, the folder, the before-restore snapshot's name and when: written last
  failed/                     a restored database that couldn't be opened (see below)
```

Once `restore.json` is there, the restore is ready: it waits for a restart, and today's data keeps working until then. The blobs the data folder has already get their modified time set to now, so the housekeeping (which leaves blobs changed in the last day alone) keeps them until the restart. `restart_to_restore()` quits Hersir the usual way (agents at work are stopped first, and it doesn't back up on quit) and starts the same Hersir again: the installed launcher, as after an update, or else the program that runs now. `cancel_restore()` removes what is staged (`restore/`, apart from `restore/failed/`); the before-restore snapshot stays in its folder, and the next start is a normal one.

### 4. The swap at the next start

Before the database opens, when `restore/restore.json` says ready, in this order, so that whatever fails today's data stays whole:

1. This computer's device (its `devices` row with `is_self = 1`: `id`, `name`, `created_at`) and today's values of this computer's settings are read from today's `gizai.db`, and its WAL is folded back into it (`PRAGMA wal_checkpoint(TRUNCATE)`).
2. The staged database gets them, on a plain connection in one transaction (see the device rule and this computer's settings below), with `agents_paused` set to true and `last_restore`.
3. `restore.json` notes the name today's database gets, then today's database moves to `backups/gizai-before-restore-<stamp>.db`, and the staged one takes its place as `gizai.db`. A start after a swap that was interrupted there carries on from where it was.
4. The staged blobs move into `files/`; a blob that is there already stays. `restore.json` goes: a restore is applied once.
5. The database opens as usual: an older schema is upgraded, with its copy in `backups/gizai-before-v<N>-<stamp>.db`.
6. When the restored database can't be opened (or step 2 fails), it moves to `restore/failed/<snapshot>.db`, today's database comes back, and Hersir starts with it.
7. `restore/` goes, apart from `restore/failed/`. The result (`{ ok, at, snapshot, fromComputer, message }`) is kept as this computer's setting `restore_result`, for the window (`restore_status().result`), until `dismiss_restore_result`.

### The device rule

When the restored database's `is_self` device isn't this computer's, every row gets `is_self = 0`, and this computer's row is added (or updated, when the snapshot knew it) with `is_self = 1` and today's `name` and `created_at`. The snapshot's computer stays in `devices` as another computer, so its `changes` rows keep their device, and new `changes` rows carry this computer's id. This computer's own snapshot leaves `devices` as it was.

### This computer's settings

`gizai_core::scope` lists the app-wide settings (`org_id` '') that belong to this computer; `scope::this_computer(key)` says whether one does. Today's values of these go over the restored ones; a restored value stays only where today had none. Every other app-wide setting, and every org's setting, is shared: it comes from the snapshot.

| Group | Keys |
|---|---|
| programs and paths | `claude_bin`, `gh_bin`, `clis`, `mcp_servers`, `mcp_tools`, `mcp_last_run`, `mcp_browser`, `cli_tools_seen`, `cli_tools_asked`, `editor_roots`, `github_push_over`, `claude_memory_imports` |
| capacity and state | `runs_memory_gb`, `runs_memory_per_run_gb`, `max_concurrent_runs`, `agents_paused`, `subscription_limits`, `onboarding`, `update_auto_check`, `update_last_check`, `preview_port_from`, `preview_port_to` (the ports this computer's previews take, checked against `max_concurrent_runs`) |
| desktop notifications | every `notify_*` key: `notify_hold`, `notify_waiting`, `notify_lead_asks`, `notify_lead_answered` |
| backups | `backup_folder`, `backup_every`, `backup_on_quit`, `backup_keep`, `backup_last_error` (the last error belongs to this computer's folder) |
| restores | `last_restore`, `restore_result` |
| Hersir Cloud | `cloud_account` (the account and its server), `cloud_backup` (Cloud Backup's On, Every and On quit), `cloud_last_backup`, `cloud_last_error` |

Sync will send the shared settings only. A new setting must go on one side: one that is only true on one computer (a path, a program, a port) goes in `scope`'s list.

### `last_restore`

```json
{ "snapshot": "<name>", "made_by": "<device id>", "made_at": 0, "last_change": { "seq": 0, "id": "<changes.id>", "hlc": "<changes.hlc>" }, "restored_at": 0, "saved_today": "<the before-restore snapshot's name>" }
```

`made_by` and `made_at` are the snapshot's computer and when it was made, `last_change` is its manifest's (`null` when it had none), and `saved_today` is the before-restore snapshot of the data it replaced. It is one of this computer's settings, so a later restore keeps it until it writes its own.

## For Hersir Cloud

Cloud Backup (GA-358, `crates/gizai-cloud-client`) keeps format 1's manifest and restore, and changes only how the bytes travel and sit in the cloud. The computer cuts the database and its blobs into chunks, compresses and encrypts each one, names each by a keyed hash, and uploads only the chunks Hersir Cloud doesn't have, straight to Cloudflare R2 with presigned URLs. The server never sees a key or a byte of content: only object ids, sizes, times and the snapshot list. A cloud restore downloads a snapshot into a format-1 folder, and from there it is [the restore](#restore) above. The server's side is `PROTOCOL.md` in `Yotech-AI/hersir-cloud`; this section is what the computers (and later the PWA) write and read.

All sizes are in bytes, `‖` joins byte strings, and base64 is the standard alphabet with padding.

### The keys

| Key | What it is |
|---|---|
| Account key | 32 random bytes, made by the account's first computer. Each signed-in computer keeps it in its keychain; the server only ever has it sealed by the recovery code. It has an epoch, 0 to 255: the first account key is epoch 1, and the epoch is the byte after `HC1` in every object. |
| Data key | HKDF-SHA256 of the account key, no salt, info `hersir-cloud/data-key/1`, 32 bytes: objects are encrypted with it. |
| Id key | HKDF-SHA256 of the account key, no salt, info `hersir-cloud/id-key/1`, 32 bytes: objects are named with it. |
| Key id | HKDF-SHA256 of the account key, no salt, info `hersir-cloud/key-id/1`, 16 bytes, as 32 lowercase hex characters: the `key_id` the server keeps, to tell a recovery code that opens the account key from one that doesn't. |
| Code key | HKDF-SHA256 of the recovery code's 16 bytes, no salt, info `hersir-cloud/recovery-code/1`, 32 bytes: it seals the account key. |
| This computer's keys | An Ed25519 signing key (a 32-byte seed) and an X25519 key (a 32-byte secret), made by each computer and kept in its keychain. Their public halves go to `POST /devices` as `sign_pub` and `box_pub`. |

"No salt" is HKDF's empty salt (RFC 5869: 32 zero bytes). The info strings are ASCII, without a trailing zero.

### The recovery code

- **What it is**: 128 random bits. It is never stored on a computer: it is shown once, for paper or a password manager.
- **Its characters**: Crockford's base32, `0123456789ABCDEFGHJKMNPQRSTVWXYZ` (no I, L, O or U). The 16 bytes are read as one string of bits, the first byte's highest bit first, 5 bits per character: 25 characters, then a 26th holding the last 3 bits, shifted up (its 2 lowest bits are 0). A 27th character, the check character, follows.
- **The check character**: Luhn mod 32 over the 26 characters' values (their places in the alphabet). From the right, every second value, starting with the last, is doubled, and a doubled value of 32 or more counts as its two base-32 digits added (`v / 32 + v % 32`). The values are added up, and the check character's value is `(32 − sum mod 32) mod 32`. Any one wrong character changes it, and most swaps of two neighbours do.
- **How it is shown**: `HC-`, then the 27 characters in groups of four, the last group three, joined by dashes: `HC-0VKQ-MC92-T820-TGCY-YGZ3-FGPE-4WV`. The last group holds the check character; setup asks to type it back.
- **How it is read back**: any case; spaces and dashes (also Unicode's hyphens and dashes) are ignored; `HC` in front may be left out. O is read as 0, and I and L as 1. Then it must be 27 characters of the alphabet, its check character must match, and the 26th character's 2 lowest bits must be 0; otherwise the code was mistyped, and nothing is tried with it.
- **A new recovery code** seals the same account key again (below) and replaces `sealed_by_code` on the server (`PUT /keys` with `if_epoch` the key's epoch), once its last group is typed back. From then on the old code opens nothing. Nothing is encrypted again.

### `sealed_by_code`

The account key sealed by the recovery code, which `PUT /keys` stores (as base64) and `GET /keys` gives back. 77 bytes:

```
"HCR1" (4) ‖ epoch (1) ‖ nonce (24) ‖ XChaCha20-Poly1305(code key, nonce, account key (32)) ‖ tag (16)
associated data: "HCR1" ‖ epoch ‖ the key id's 16 bytes
```

The nonce is random. To open it, a computer takes `key_id` and `sealed_by_code` from `GET /keys`, the code key from the code typed, and the key id's 16 bytes from `key_id`: a wrong code fails the tag, and the opened account key's own key id must be `key_id`. Either failure is "That recovery code doesn't open this account's backups."

### Chunks

- **The database**: a page-for-page copy, made with SQLite's backup API (not `VACUUM INTO`, which renumbers pages), so a page that didn't change stays in the same chunk. It is cut into chunks of 256 KiB (262,144 bytes), the last one shorter.
- **A blob** (an attachment or screenshot that format 1's `files` names): up to 4 MiB (4,194,304 bytes) it is one chunk. A bigger one is cut by content with FastCDC 2020 (the `fastcdc` crate's `v2020`, normalization level 1, its own gear table without a seed): at least 512 KiB, about 1 MiB, at most 2 MiB. A blob up to 4 MiB whose object would be over the limit below (zstd can't shrink it, and it is within about a KiB of 4 MiB) is cut by content too. An empty blob has no chunks.
- **A chunk's id**: HMAC-SHA256(id key, the chunk's bytes), as 64 lowercase hex characters. Never the plain SHA-256: that would tell the server which known files someone has. The same chunk has the same id in every snapshot and on every computer of the account, which is all deduplication needs.

### An object's bytes

Each chunk is one object in R2, at most 4,194,368 bytes (4 MiB and 64):

```
"HC1" (3) ‖ epoch (1) ‖ nonce (24) ‖ XChaCha20-Poly1305(data key, nonce, zstd(chunk)) ‖ tag (16)
associated data: the chunk's id, its 64 lowercase hex characters as ASCII
```

- zstd at level 3, one frame per chunk, which records the chunk's size.
- The nonce is random, so the same chunk sealed twice gives two different objects. Only its id is the same, and the server keeps one.
- Opening one: `HC1`, the epoch of a key this computer has, the tag with the id it was asked for as associated data, zstd (at most 4 MiB), and then the chunk's HMAC must be that id. Anything else is refused: the object was changed, made with another key, or isn't the one asked for.

### The manifest

A cloud snapshot's manifest is format 1's [manifest](#the-manifest), with the ordered chunk ids added. It is compact JSON in UTF-8, sealed and uploaded as an object like the others: its id is the HMAC of its JSON bytes, and that is the snapshot's `manifest_id`.

```json
{
  "format": "gizai-backup",
  "format_version": 1,
  "name": "gizai-daily-20261125-110000-000-9c1a07",
  "reason": "daily",
  "made_at": 1795633200000,
  "app_version": "0.21.0",
  "schema_version": 23,
  "device": { "id": "<devices.id>", "name": "<devices.name>" },
  "db": { "path": "snapshots/<name>.db.zst", "compression": "zstd", "bytes": 0, "sha256": "<sha256 of the database>", "db_bytes": 2473984 },
  "last_change": { "seq": 0, "id": "<changes.id>", "hlc": "<changes.hlc>" },
  "files": [ { "sha256": "<64 lowercase hex>", "bytes": 10000 } ],
  "files_missing": [],
  "chunks": {
    "db": [ "<id>", "…" ],
    "files": { "<sha256>": [ "<id>", "…" ] }
  }
}
```

- Format 1's fields mean what they mean in a backups folder, with one difference: `db.bytes` is 0, as the database isn't one compressed file in the cloud.
- `chunks.db`: the database's chunks in order; put together, they are the database, `db.db_bytes` long, with `db.sha256`.
- `chunks.files`: each stored blob's chunks in order, by its `sha256`; put together, they are the blob. A blob in `files_missing` has none, and isn't restored, so format 1's restore refuses that snapshot, as it does in a backups folder.

### A backup

1. **Chunking**: the database copy and each blob are cut into chunks and named. A blob that isn't in the data folder, or no longer matches its hash, goes into `files_missing`, as in a backups folder.
2. **Uploading**: the manifest is sealed. `POST /objects/missing` with every chunk id once and the manifest's says which objects the account doesn't have; only those are sealed and uploaded, in batches of at most 256 objects and 32 MiB (`POST /objects/upload`, then a `PUT` of each to its presigned URL with its `headers`). An object stored already gets no URL and is passed over. A second backup of a database that didn't change uploads only its manifest.
3. **Registering**: `POST /snapshots` with format 1's `name` (letters, digits and dashes), `made_at`, `device_id`, `reason` (`daily`, `manual`, `quit` or `before-restore`), `schema_version` and `app_version`, the `manifest_id`, every chunk id once and the manifest's as `object_ids`, and `bytes`: the database's size and the stored blobs' together. On 422 `missing_objects` those objects are uploaded again and the snapshot registered again, up to three times.

A 429, a server error (5xx) or a network failure is waited out (the server's `Retry-After`, at most 2 minutes) and tried again, up to five times; any other error stops the backup. Each step reports its progress as (step, done, total): `chunking` counts the bytes read, `uploading` the chunks' bytes, `registering` 0 then 1 of 1.

### A restore

`restore` downloads a snapshot (its `name` and `manifest_id`, from `GET /snapshots`) into a folder, for example `<data folder>/restore/cloud/`:

1. **Downloading**: the manifest, opened and checked (its `name` must be the snapshot's, and its `format_version` at most this Hersir's), then every object it names, once each, into `.cloud-objects/` inside the folder. A restore into the same folder after an interruption doesn't download those again. The progress counts objects.
2. **Checking**: each object is opened and checked (above), and the chunks are put back together: the database into `snapshots/<name>.db.zst` (zstd level 3, one frame that records its size, as format 1 writes it) and each stored blob into `files/<sha256[..2]>/<sha256>`, each through a `.part` file. The database's and each blob's SHA-256 and size must be the manifest's. The progress counts their bytes.

The folder is then a format-1 backups folder: `gizai-backups.json`, `snapshots/<name>.json` (format 1's manifest, without `chunks`, and with `db.bytes` the size of the `.db.zst` written) beside its database, and `files/`. `.cloud-objects/` goes. GA-218's restore takes it from there as it is: `gizai_core::restore::check` with the folder and the name, then today's data saved, staging, and the swap at the next start.

### In the app

`src-tauri/src/cloud.rs` (GA-359) uses the crate:

- **Signing in**: `oauth::discover("<server>/api")`, `begin` with Hersir's desktop client id (no registration), `finish`, then `POST /devices` with this computer's device id, its name, platform, version, `kind: "desktop"` and new public keys. The token is renewed in its last 30 days. A 401 `removed` or `signed_out` signs the computer out and keeps the server's message as Cloud Backup's last error; `removed` and Sign out also forget the account key and the computer's keys.
- **The keychain** (service `hersir`): `cloud/<account>/token`, `cloud/<account>/key` and `cloud/<account>/device`. The recovery code is never stored: the first computer keeps it in memory until its last group is typed back, then only the account key sealed by it goes to Hersir Cloud (`PUT /keys`).
- **When it backs up**: daily, when the last cloud backup is a day old and the newest `changes` row is another one; on Quit Hersir completely with On quit on (after the local snapshot, within the same 5 minutes); Back up now; and before a restore. A lapsed plan, no plan or a full Hersir Cloud makes none. The database is copied page for page into the data folder's `backups/` (`.cloud-<name>.db`), which goes once the backup ends.
- **Restore from Hersir Cloud** downloads into `<data folder>/restore/cloud/`, then it is GA-218's restore of that folder. Cancelling the restore stops the download. A restore applied at the start also ends the first start's onboarding: the restored data has its own team.
- **Debug builds** read `HERSIR_CLOUD_URL`, `HERSIR_CLOUD_CLIENT_ID` and `HERSIR_CLOUD_ENTITLEMENT_KEY` (point them at the fake below); release builds use `https://cloud.hersir.dev`, the desktop client id and the entitlement key built in.

### The fake Hersir Cloud

`gizai_cloud_client::fake::FakeCloud` answers on 127.0.0.1 as the server does, in memory: the device, entitlement, key, object and snapshot calls with their errors, presigned URLs that point to itself, its own entitlement key pair, and the OAuth calls Hersir's sign-in makes (`/oauth/authorize` sends the browser straight back with a code). Tests use it and can ask any call to fail once; `cargo run -p gizai-cloud-client --example fake-cloud [port]` runs one on its own and prints its address and entitlement key (`HERSIR_FAKE_CLOUD_SEED`, the base64 of a 32-byte seed, keeps the key the same between runs).

### For a sync, later

- `last_change` is where a sync continues: the device that restores a snapshot has every change up to it, and asks for the ones after it.
- `files` is what a sync uploads, by hash: a file the cloud has already isn't sent again, as a backups folder stores each file once.
- A device that restores keeps its own id (see [the device rule](#the-device-rule)), and `last_restore` says which snapshot its data starts from: a sync asks for the changes after that snapshot's `last_change`.
- Sync sends the shared settings only, never this computer's (see [this computer's settings](#this-computers-settings)).
- Beyond settings, parts of the database are this computer's too, and a restore brings them back as the snapshot has them: projects' linked folders (`repos.local_path`), agents' folders (`agent_configs.folders_json`), and the worktree paths and process ids in `runs` (`cwd`, `worktree_path`, `pid`). Sync must not send them as they are, nor apply another device's.
- A change's `hlc` is `<unix_ms>-<counter>-<device>`, and its device part is the first 8 characters of the device id. Those are the id's creation time (ids are UUIDv7), so two devices made in the same moment can share them: sync should order changes by the time, the counter and the whole device id (`changes.device_id`), never by the hlc's device part alone.
