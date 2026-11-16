//! GA-264 QA: a data folder and a backup made by Hersir 0.16.0 itself, opened and restored by this Hersir. 0.16.0 has
//! rusqlite 0.40.2 with SQLite 3.53.2 and schema 21; since GA-264 Hersir has rusqlite 0.41 with SQLite 3.54 or later
//! (libsqlite3-sys 0.39, RUSTSEC-2026-0459).
//!
//! `tests/fixtures/v0.16.0/` was made by a throwaway example run on a checkout of tag v0.16.0 (1d66ca3), never by this
//! code: it opened a new data folder with `Db::open`, `seed::ensure_seed(&db, "Jeffrey")`, added a person (Sanne
//! Bakker), a client (Kade Logistics B.V.), a project (Kade portal, KADE), two cards (one In progress with a label, a
//! comment and ü and an emoji in its description, one Done), a doc, an attachment (`files::add_from_path`) and a setting
//! (`ga264.qa`), then made one manual snapshot with `backups::make` into a chosen folder and closed the database.
//! - `data/gizai.db.zst`: that data folder's `gizai.db` as the app left it (WAL mode), compressed with zstd;
//!   `data/files/`: its attachment.
//! - `Hersir backups/`: the backups folder the snapshot made, as it is.
use std::fs;
use std::path::{Path, PathBuf};

use gizai_core::backups::{self as b, Manifest, Reason};
use gizai_core::db::{self, Db};
use gizai_core::restore::{self, Saved};
use gizai_core::{comments, settings};
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};

const SNAPSHOT: &str = "gizai-manual-20261110-183655-639-d0b90cd9";
const DEVICE: &str = "01a1c688-c94b-7366-9e5a-0c7dd0b90cd9";
const BLOB: &str = "6f0740fe719b5394b7e49ac1e12aacc114ead03da137e6a48539bb2f98739fb2";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("v0.16.0")
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            copy_dir(&p, &to.join(e.file_name()));
        } else {
            fs::write(to.join(e.file_name()), fs::read(&p).unwrap()).unwrap();
        }
    }
}

/// 0.16.0's data folder, unpacked in `root/data`.
fn data_016(root: &Path) -> PathBuf {
    let dir = root.join("data");
    copy_dir(&fixture().join("data").join("files"), &dir.join("files"));
    let zst = fs::read(fixture().join("data").join("gizai.db.zst")).unwrap();
    fs::write(dir.join("gizai.db"), zstd::decode_all(&zst[..]).unwrap()).unwrap();
    dir
}

/// 0.16.0's backups folder, in chosen folder `root/usb`. Returns the chosen folder.
fn usb_016(root: &Path) -> PathBuf {
    let usb = root.join("usb");
    copy_dir(&fixture().join("Hersir backups"), &usb.join("Hersir backups"));
    usb
}

/// What 0.16.0 put in its data, read with SQL, in one value to compare.
fn facts(c: &Connection) -> Value {
    let rows = |sql: &str| -> Vec<Value> {
        let mut st = c.prepare(sql).unwrap();
        let n = st.column_count();
        st.query_map([], |r| (0..n).map(|i| r.get::<_, rusqlite::types::Value>(i).map(|v| match v {
            rusqlite::types::Value::Null => Value::Null,
            rusqlite::types::Value::Integer(i) => json!(i),
            rusqlite::types::Value::Text(s) => json!(s),
            other => json!(format!("{other:?}")),
        })).collect::<rusqlite::Result<Vec<Value>>>().map(Value::Array))
            .unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap()
    };
    json!({
        "people": rows("SELECT name FROM actors WHERE kind = 'person' ORDER BY name"),
        "clients": rows("SELECT name, city, coc_number FROM clients ORDER BY name"),
        "projects": rows("SELECT p.name, p.key, c.name, p.goal_md FROM projects p LEFT JOIN clients c ON c.id = p.client_id ORDER BY p.name"),
        "tasks": rows("SELECT t.title, s.name, t.priority, t.description_md FROM tasks t JOIN workflow_states s ON s.id = t.state_id ORDER BY t.title"),
        "labels": rows("SELECT t.title, l.name FROM task_labels tl JOIN tasks t ON t.id = tl.task_id JOIN labels l ON l.id = tl.label_id ORDER BY t.title, l.name"),
        "comments": rows("SELECT t.title, a.name, cm.body_md FROM comments cm JOIN tasks t ON t.id = cm.task_id JOIN actors a ON a.id = cm.author_actor_id ORDER BY cm.created_at"),
        "docs": rows("SELECT title, body_md FROM docs ORDER BY title"),
        "files": rows("SELECT owner_type, sha256 FROM files ORDER BY sha256"),
        "setting": rows("SELECT value_json FROM settings WHERE key = 'ga264.qa'"),
    })
}

/// The data 0.16.0 wrote, as `facts` reads it.
fn facts_016() -> Value {
    json!({
        "people": [["Jeffrey"], ["Sanne Bakker"]],
        "clients": [["Kade Logistics B.V.", "Amsterdam", "34098812"]],
        "projects": [["Kade portal", "KADE", "Kade Logistics B.V.", "Give Kade's office one place for shipments, invoices and drivers."]],
        "tasks": [
            ["Export invoices as CSV", "In progress", 2, "## Context\n\nCSV export, with ü and 😀.\n\n- [ ] first step"],
            ["Rate limit the tracking endpoint", "Done", 3, ""],
        ],
        "labels": [["Export invoices as CSV", "backend"]],
        "comments": [["Export invoices as CSV", "Jeffrey", "Use Filament's exporter. Sample file is attached."]],
        "docs": [["Requirements", "# Requirements\n\nThe portal replaces the Excel lists."]],
        "files": [["project", BLOB]],
        "setting": [["{\"made_by\":\"0.16.0\"}"]],
    })
}

fn read_facts(db: &Db) -> Value {
    db.read(|c| Ok(facts(c))).unwrap()
}

/// `PRAGMA integrity_check` and `PRAGMA foreign_key_check`, and `user_version`.
fn health(db: &Db) -> (String, i64, i64) {
    db.read(|c| {
        Ok((c.query_row("PRAGMA integrity_check", [], |r| r.get(0))?,
            c.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r.get(0))?,
            c.query_row("PRAGMA user_version", [], |r| r.get(0))?))
    }).unwrap()
}

/// Every row count of every table, by table: nothing lost or doubled.
fn counts(c: &Connection, tables: &[String]) -> Vec<(String, i64)> {
    tables.iter().map(|t| (t.clone(), c.query_row(&format!("SELECT count(*) FROM \"{t}\""), [], |r| r.get(0)).unwrap())).collect()
}

fn tables(c: &Connection) -> Vec<String> {
    let mut st = c.prepare("SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap();
    st.query_map([], |r| r.get(0)).unwrap().collect::<rusqlite::Result<Vec<String>>>().unwrap()
}

#[test]
fn the_bundled_sqlite_is_3_54_or_later_past_rustsec_2026_0459() {
    assert!(rusqlite::version_number() >= 3_054_000, "SQLite {} bundles the bug RUSTSEC-2026-0459 is about", rusqlite::version());
}

#[test]
fn a_data_folder_made_by_0_16_0_opens_migrates_to_this_schema_and_keeps_its_data() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = data_016(tmp.path());
    let path = dir.join("gizai.db");
    // as 0.16.0 left it: schema 21, in WAL mode
    let (before, names) = {
        let c = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        assert_eq!(c.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)).unwrap(), 21);
        assert_eq!(c.query_row("PRAGMA journal_mode", [], |r| r.get::<_, String>(0)).unwrap(), "wal");
        assert_eq!(facts(&c), facts_016(), "this SQLite reads 0.16.0's file as 0.16.0 wrote it");
        let names = tables(&c);
        (counts(&c, &names), names)
    };

    let db = Db::open(&path).unwrap();
    assert_eq!(health(&db), ("ok".to_string(), 0, db::SCHEMA_VERSION), "migrated, whole, no broken links");
    assert_eq!(read_facts(&db), facts_016());
    assert_eq!(db.read(|c| Ok(counts(c, &names))).unwrap(), before, "no row lost or added in any table 0.16.0 had");
    let added: i64 = db.read(|c| Ok(c.query_row(
        "SELECT (SELECT count(*) FROM pragma_table_info('tasks') WHERE name = 'hold_kind')
              + (SELECT count(*) FROM pragma_table_info('chat_threads') WHERE name = 'outside_tool')", [], |r| r.get(0))?)).unwrap();
    assert_eq!(added, 2, "migration 22's columns");
    assert_eq!(db.device_id(), DEVICE, "the same computer");
    assert_eq!(fs::read(gizai_core::files::blob_path(&dir, BLOB)).unwrap(), b"number;date;amount\n2026-0412;2026-09-30;1250,00\n");
    // Db::open's copy before the upgrade, as 0.16.0 had it
    let copies: Vec<String> = fs::read_dir(dir.join("backups")).unwrap().flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(&format!("gizai-before-v{}-", db::SCHEMA_VERSION))).collect();
    assert_eq!(copies.len(), 1, "{copies:?}");
    let copy = Connection::open(dir.join("backups").join(&copies[0])).unwrap();
    assert_eq!(copy.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)).unwrap(), 21);
    assert_eq!(facts(&copy), facts_016());

    // it takes writes, and the next start is a normal one with them kept
    let (task, you): (String, String) = db.read(|c| Ok(c.query_row(
        "SELECT task_id, author_actor_id FROM comments", [], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap();
    comments::add(&db, &you, &task, "Written after the update.", None).unwrap();
    settings::set(&db, "ga264.qa", &json!({"made_by": "today"})).unwrap();
    drop(db);
    let db = Db::open(&path).unwrap();
    let f = read_facts(&db);
    assert_eq!(f["comments"][1], json!(["Export invoices as CSV", "Jeffrey", "Written after the update."]));
    assert_eq!(f["setting"], json!([["{\"made_by\":\"today\"}"]]));
    assert_eq!(f["tasks"], facts_016()["tasks"]);
    assert_eq!(health(&db).0, "ok");
}

/// Check snapshot `name` in chosen folder `usb`, save today's data there and stage it, as the app does before the restart.
fn stage(db: &Db, dir: &Path, usb: &Path, name: &str) -> Manifest {
    let from = b::backups_dir(usb);
    let m = restore::check(dir, &from, name, &mut |_| Ok(())).unwrap();
    let saved = b::make(dir, db.device_id(), usb, Reason::BeforeRestore, 10, &mut |_| Ok(())).unwrap().manifest;
    let s = Saved { name: saved.name.clone(), made_at: saved.made_at, folder: from.to_string_lossy().into_owned() };
    restore::stage(dir, &from, &m, &s, &mut |_| Ok(())).unwrap();
    m
}

#[test]
fn a_backup_made_by_0_16_0_restores_over_the_updated_data_folder_it_came_from() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = data_016(tmp.path());
    let usb = usb_016(tmp.path());
    let db = Db::open(&dir.join("gizai.db")).unwrap();
    // today's changes, after the update
    let (task, you): (String, String) = db.read(|c| Ok(c.query_row(
        "SELECT task_id, author_actor_id FROM comments", [], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap();
    comments::add(&db, &you, &task, "Only today.", None).unwrap();
    settings::set(&db, "ga264.qa", &json!({"made_by": "today"})).unwrap();
    assert_ne!(read_facts(&db), facts_016());

    let m = stage(&db, &dir, &usb, SNAPSHOT);
    assert_eq!((m.app_version.as_str(), m.schema_version, m.device.id.as_str()), ("0.16.0", 21, DEVICE));
    drop(db);
    let (db, result) = restore::open(&dir).unwrap();
    let result = result.expect("a restore result");
    assert!(result.ok, "{}", result.message);
    assert_eq!(result.snapshot, SNAPSHOT);
    assert_eq!(read_facts(&db), facts_016(), "0.16.0's data, today's changes gone");
    assert_eq!(health(&db), ("ok".to_string(), 0, db::SCHEMA_VERSION), "upgraded from 21");
    assert_eq!(db.device_id(), DEVICE);
    assert_eq!(restore::last_restore(&db).map(|l| l.snapshot), Some(SNAPSHOT.to_string()));
    assert!(gizai_core::files::blob_path(&dir, BLOB).is_file());
}

#[test]
fn a_backup_made_by_0_16_0_restores_on_another_computer_with_a_new_data_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let usb = usb_016(tmp.path());
    let dir = tmp.path().join("new");
    fs::create_dir_all(&dir).unwrap();
    let db = Db::open(&dir.join("gizai.db")).unwrap();
    gizai_core::seed::ensure_seed(&db, "Jeffrey").unwrap();
    let me = db.device_id().to_string();
    assert_ne!(me, DEVICE);

    stage(&db, &dir, &usb, SNAPSHOT);
    drop(db);
    let (db, result) = restore::open(&dir).unwrap();
    let result = result.expect("a restore result");
    assert!(result.ok, "{}", result.message);
    assert_eq!(read_facts(&db), facts_016());
    assert_eq!(health(&db), ("ok".to_string(), 0, db::SCHEMA_VERSION));
    assert_eq!(db.device_id(), me, "this computer keeps its own id");
    assert_eq!(fs::read(gizai_core::files::blob_path(&dir, BLOB)).unwrap(), b"number;date;amount\n2026-0412;2026-09-30;1250,00\n");
}
