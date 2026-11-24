//! Docs records in core: migration 0025 on a schema 24 database (the switches, the changelog lines, the decision records
//! and the baselines, all empty, after a backup); a card's changelog line from its builder (a kind Keep a Changelog
//! names, one plain line, the newer one kept), shown with the card once it is merged; the Chief's decision records,
//! numbered after the project's own; each project's baseline; and the sections a project switches off.
use gizai_core::db::{self, Db};
use gizai_core::devdocs::{self, Baseline, DecisionInput};
use gizai_core::{model::*, projects, seed, tasks};
use rusqlite::Connection;

fn version(c: &Connection) -> i64 {
    c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap()
}

#[test]
fn a_schema_24_database_gets_the_docs_records_empty_after_a_backup() {
    assert_eq!(db::SCHEMA_VERSION, 25, "0025 adds the Docs records");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gizai.db");
    let kade = {
        let db = Db::open(&path).unwrap();
        let s = seed::ensure_seed(&db, "Jeffrey").unwrap();
        projects::create(&db, &s.you_id, ProjectInput { name: "Kade".into(), key: "KADE".into(), dev_docs: Some(true), ..Default::default() }).unwrap()
    };
    // one step back, to the database the version before leaves (schema 24): 0025 adds one column and three tables
    let c = Connection::open(&path).unwrap();
    c.execute_batch("DROP TABLE docs_baselines; DROP TABLE docs_decisions; DROP TABLE docs_changes; ALTER TABLE projects DROP COLUMN docs_off;
                     PRAGMA user_version = 24;").unwrap();
    drop(c);

    let db = Db::open(&path).unwrap();
    db.read(|c| {
        assert_eq!(version(c), 25, "0025 ran");
        for table in ["docs_changes", "docs_decisions", "docs_baselines"] {
            assert_eq!(c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get::<_, i64>(0))?, 0, "{table} is there, empty");
        }
        Ok(())
    }).unwrap();
    let p = projects::get(&db, &kade).unwrap();
    assert!(p.dev_docs && p.docs_off.is_empty(), "every section still shows");
    let snaps: Vec<_> = std::fs::read_dir(dir.path().join("backups")).unwrap().map(|e| e.unwrap().path()).collect();
    assert!(snaps.len() == 1 && snaps[0].file_name().unwrap().to_string_lossy().starts_with("gizai-before-v25-"), "{snaps:?}");
    assert_eq!(version(&Connection::open(&snaps[0]).unwrap()), 24, "the backup is the schema 24 database");
}

struct Set { db: Db, you: String, kade: String }

fn set_up() -> Set {
    let db = Db::open_in_memory().unwrap();
    let s = seed::ensure_seed(&db, "Jeffrey").unwrap();
    let kade = projects::create(&db, &s.you_id, ProjectInput { name: "Kade".into(), key: "KADE".into(), dev_docs: Some(true),
        ..Default::default() }).unwrap();
    Set { db, you: s.you_id, kade }
}

impl Set {
    fn card(&self, title: &str, done: bool) -> String {
        let id = tasks::create(&self.db, &self.you, TaskInput { project_id: self.kade.clone(), title: title.into(), ..Default::default() }).unwrap();
        if done {
            let column: String = self.db.read(|c| Ok(c.query_row("SELECT id FROM workflow_states WHERE category = 'done' AND deleted_at IS NULL \
                ORDER BY sort_key LIMIT 1", [], |r| r.get(0))?)).unwrap();
            tasks::move_to(&self.db, &self.you, &id, &column, "a0").unwrap();
        }
        id
    }
}

#[test]
fn a_cards_changelog_line_shows_with_it_once_it_is_merged() {
    let s = set_up();
    let merged = s.card("Export invoices", true);
    let building = s.card("Search", false);
    let bare = s.card("No line written", true);
    devdocs::set_change(&s.db, None, &merged, "added", "Invoices export   as CSV,\nfrom the list.").unwrap();
    devdocs::set_change(&s.db, None, &building, "Fixed", "Search finds names").unwrap();
    let log = devdocs::changelog(&s.db, &s.kade).unwrap();
    let lines: Vec<(&str, Option<&str>, Option<&str>)> = log.iter().map(|e| (e.title.as_str(), e.kind.as_deref(), e.text.as_deref())).collect();
    assert_eq!(lines, [("No line written", None, None), ("Export invoices", Some("Added"), Some("Invoices export as CSV, from the list."))],
               "one plain line, its kind as Keep a Changelog names it; a card not merged yet isn't there");
    devdocs::set_change(&s.db, None, &merged, "Changed", "Invoices export as CSV and PDF").unwrap();
    assert_eq!(devdocs::changelog(&s.db, &s.kade).unwrap()[1].text.as_deref(), Some("Invoices export as CSV and PDF"), "the newer line kept");
    let _ = bare;

    for (kind, text) in [("Improved", "x"), ("Added", "   ")] {
        assert!(devdocs::set_change(&s.db, None, &merged, kind, text).is_err(), "{kind} {text:?}");
    }
    assert!(devdocs::set_change(&s.db, None, "no-such-card", "Added", "x").is_err());
    let long = "word ".repeat(100);
    devdocs::set_change(&s.db, None, &merged, "Added", &long).unwrap();
    assert_eq!(devdocs::changelog(&s.db, &s.kade).unwrap()[1].text.as_deref().map(|t| t.chars().count()), Some(devdocs::CHANGE_MAX));
}

#[test]
fn the_chiefs_decision_records_are_numbered_after_the_projects_own() {
    let s = set_up();
    let card = s.card("Pick a database", false);
    let input = |title: &str| DecisionInput { title: title.into(), decision: "Postgres, not MySQL.".into(), context: "We need JSON queries.".into(),
        ..Default::default() };
    let first = devdocs::record_decision(&s.db, &s.you, "Chief", &s.kade, 119, DecisionInput { task_id: Some(card.clone()),
        related_files: vec!["config/database.php".into()], ..input("Use Postgres") }).unwrap();
    assert_eq!((first.number, first.status.as_str(), first.task.as_deref(), first.decided_by.as_str()), (120, "accepted", Some("KADE-1"), "Chief"),
               "after the project's own 0119; accepted by default");
    assert_eq!(first.related_files, ["config/database.php"]);
    let second = devdocs::record_decision(&s.db, &s.you, "Chief", &s.kade, 3, DecisionInput { status: Some("Proposed".into()), supersedes: Some(120),
        ..input("Use Postgres 17") }).unwrap();
    assert_eq!((second.number, second.status.as_str(), second.supersedes), (121, "proposed", Some(120)), "after Hersir's own when those are higher");
    let all = devdocs::decision_records(&s.db, &s.kade).unwrap();
    assert_eq!(all.iter().map(|d| (d.number, d.status.as_str())).collect::<Vec<_>>(), [(121, "proposed"), (120, "superseded")]);

    for bad in [DecisionInput { decision: "x".into(), ..Default::default() }, DecisionInput { title: "No decision".into(), ..Default::default() },
                DecisionInput { status: Some("rejected".into()), ..input("Rejected already") }] {
        assert!(devdocs::record_decision(&s.db, &s.you, "Chief", &s.kade, 0, bad).is_err());
    }
    assert!(devdocs::record_decision(&s.db, &s.you, "Chief", "no-such-project", 0, input("x")).is_err());
}

#[test]
fn each_project_keeps_one_baseline() {
    let s = set_up();
    assert_eq!(devdocs::baseline(&s.db, &s.kade).unwrap(), None);
    let b = |commit: &str| Baseline { commit: commit.into(), taken_at: 1, entries: format!("{{\"{commit}\":1}}") };
    devdocs::set_baseline(&s.db, &s.kade, &b("abc1234")).unwrap();
    devdocs::set_baseline(&s.db, &s.kade, &b("def5678")).unwrap();
    assert_eq!(devdocs::baseline(&s.db, &s.kade).unwrap(), Some(b("def5678")));
}

#[test]
fn a_project_switches_sections_off_by_name() {
    let s = set_up();
    let input = |off: Option<Vec<&str>>| ProjectInput { name: "Kade".into(), key: "KADE".into(), docs_off: off.map(|o| o.into_iter().map(String::from).collect()),
        ..Default::default() };
    projects::update(&s.db, &s.you, &s.kade, input(Some(vec!["mcp", "reference", "nonsense", "mcp"]))).unwrap();
    assert_eq!(projects::get(&s.db, &s.kade).unwrap().docs_off, ["reference", "mcp"], "the known ones, once each, in the page's order");
    projects::update(&s.db, &s.you, &s.kade, input(None)).unwrap();
    assert_eq!(projects::get(&s.db, &s.kade).unwrap().docs_off, ["reference", "mcp"], "left out keeps them");
    projects::update(&s.db, &s.you, &s.kade, input(Some(vec![]))).unwrap();
    assert!(projects::get(&s.db, &s.kade).unwrap().docs_off.is_empty(), "all on again");
    let json = serde_json::to_value(projects::get(&s.db, &s.kade).unwrap()).unwrap();
    assert!(json.get("docsOff").is_some(), "named docsOff for the app: {json}");
}
