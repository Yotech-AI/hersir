//! Docs in core: migration 0023 on a schema 22 database (every project starts with Docs off, after a backup), and the
//! project setting: off for a new project, turned on and off from the input, kept when left out, and named devDocs for
//! the app.
use gizai_core::db::{self, Db};
use gizai_core::{model::*, projects, seed};
use rusqlite::Connection;

fn version(c: &Connection) -> i64 {
    c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap()
}

#[test]
fn a_schema_22_database_gets_docs_off_for_every_project_after_a_backup() {
    assert_eq!(db::SCHEMA_VERSION, 24, "0023 adds projects.dev_docs, and 0024 the Docs records after it");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gizai.db");
    let kade = {
        let db = Db::open(&path).unwrap();
        let s = seed::ensure_seed(&db, "Jeffrey").unwrap();
        projects::create(&db, &s.you_id, ProjectInput { name: "Kade".into(), key: "KADE".into(), ..Default::default() }).unwrap()
    };
    // two steps back, to the database the version before Docs leaves (schema 22): 0023 only adds the column, 0024 after it
    // one more and three tables
    let c = Connection::open(&path).unwrap();
    c.execute_batch("DROP TABLE docs_baselines; DROP TABLE docs_decisions; DROP TABLE docs_changes; ALTER TABLE projects DROP COLUMN docs_off; ALTER TABLE projects DROP COLUMN dev_docs; PRAGMA user_version = 22;").unwrap();
    drop(c);

    let db = Db::open(&path).unwrap();
    db.read(|c| {
        assert_eq!(version(c), 24, "0023 ran (and 0024 after it)");
        let col: (String, i64, Option<String>) = c.query_row(
            "SELECT type, \"notnull\", dflt_value FROM pragma_table_info('projects') WHERE name = 'dev_docs'", [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        assert_eq!(col, ("INTEGER".to_string(), 1, Some("0".to_string())), "on or off, off by default");
        Ok(())
    }).unwrap();
    assert!(!projects::get(&db, &kade).unwrap().dev_docs, "Docs is off for a project that was there before");
    // the backup made before the upgrade is the schema 22 database
    let snaps: Vec<_> = std::fs::read_dir(dir.path().join("backups")).unwrap().map(|e| e.unwrap().path()).collect();
    assert!(snaps.len() == 1 && snaps[0].file_name().unwrap().to_string_lossy().starts_with("gizai-before-v24-"), "{snaps:?}");
    assert_eq!(version(&Connection::open(&snaps[0]).unwrap()), 22, "the backup is the schema 22 database");
}

#[test]
fn docs_is_off_for_a_new_project_and_turns_on_and_off() {
    let db = Db::open_in_memory().unwrap();
    let s = seed::ensure_seed(&db, "Jeffrey").unwrap();
    let input = |dev_docs: Option<bool>| ProjectInput { name: "Kade".into(), key: "KADE".into(), dev_docs, ..Default::default() };
    let id = projects::create(&db, &s.you_id, input(None)).unwrap();
    assert!(!projects::get(&db, &id).unwrap().dev_docs, "off for a new project");
    projects::update(&db, &s.you_id, &id, input(Some(true))).unwrap();
    assert!(projects::get(&db, &id).unwrap().dev_docs, "on");
    projects::update(&db, &s.you_id, &id, input(None)).unwrap();
    assert!(projects::get(&db, &id).unwrap().dev_docs, "left out keeps it");
    assert!(projects::list(&db).unwrap().iter().any(|p| p.id == id && p.dev_docs), "the list says so too");
    projects::update(&db, &s.you_id, &id, input(Some(false))).unwrap();
    assert!(!projects::get(&db, &id).unwrap().dev_docs, "off again");
    let shop = projects::create(&db, &s.you_id, ProjectInput { name: "Shop".into(), key: "SHOP".into(), dev_docs: Some(true),
        ..Default::default() }).unwrap();
    assert!(projects::get(&db, &shop).unwrap().dev_docs, "a new project can start with it on");
}

#[test]
fn the_app_reads_and_sends_it_as_dev_docs() {
    let db = Db::open_in_memory().unwrap();
    let s = seed::ensure_seed(&db, "Jeffrey").unwrap();
    let id = projects::create(&db, &s.you_id, ProjectInput { name: "Kade".into(), key: "KADE".into(), dev_docs: Some(true),
        ..Default::default() }).unwrap();
    let json = serde_json::to_value(projects::get(&db, &id).unwrap()).unwrap();
    assert_eq!(json["devDocs"], serde_json::json!(true));
    let input: ProjectInput = serde_json::from_value(serde_json::json!({ "name": "Kade", "key": "KADE", "devDocs": false })).unwrap();
    assert_eq!(input.dev_docs, Some(false));
}
