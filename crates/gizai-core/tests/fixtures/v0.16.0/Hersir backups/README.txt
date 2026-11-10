Hersir backups

This folder holds snapshots of Hersir's data, made by Hersir on each computer that backs up here: its database and the
files it stores (attachments and screenshots). Hersir makes them on a schedule, when you press Back up now, and when
you quit Hersir if you asked it to.

snapshots/   one snapshot is two files: <name>.json says what it holds, <name>.db.zst is its database (zstd)
files/       every stored file, once, named by its content (SHA-256), shared by all snapshots

Several computers can back up to this folder: each adds and removes only its own snapshots. Don't rename or change
the files here. They are not encrypted: whoever can open this folder can read them.

Restore in Hersir: Settings → General → Backups.
