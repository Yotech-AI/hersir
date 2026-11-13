//! GA-261 QA: migration 0022 on a schema 21 database, after a backup. It adds `chat_threads.outside_tool` and
//! `tasks.hold_kind`, and nothing else; a chat that used a tool from outside Hersir before the upgrade is marked with its first
//! such tool (not Read, Glob, Grep or one of Hersir's own, as `chat::is_outside_tool` tells them), without a note; a card on
//! GA-101's hold (needs_decision, "Waiting for your OK: the … put this card here after using …") gets the kind only the user
//! lifts, and every other hold none. Opening it again changes nothing.
use std::path::Path;

use gizai_core::chat::{self, NewMessage};
use gizai_core::db::{self, Db};
use gizai_core::model::*;
use gizai_core::{projects, seed, tasks, team};
use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};

/// Migrations 0001..0021 on a new file at `path`: a genuine schema 21 database, as the version before GA-261 leaves it.
fn v21_db(path: &Path) -> Connection {
    let all = [
        include_str!("../../migrations/0001_init.sql"), include_str!("../../migrations/0002_agents.sql"), include_str!("../../migrations/0003_chat.sql"),
        include_str!("../../migrations/0004_effort.sql"), include_str!("../../migrations/0005_pull_requests.sql"),
        include_str!("../../migrations/0006_worktree_prepare.sql"), include_str!("../../migrations/0007_card_flow.sql"),
        include_str!("../../migrations/0008_board_check.sql"), include_str!("../../migrations/0009_agent_folders.sql"),
        include_str!("../../migrations/0010_run_refusals.sql"), include_str!("../../migrations/0011_column_agents.sql"),
        include_str!("../../migrations/0012_chat_runs_on.sql"), include_str!("../../migrations/0013_bitbucket.sql"),
        include_str!("../../migrations/0014_memory.sql"), include_str!("../../migrations/0015_lead_merges.sql"),
        include_str!("../../migrations/0016_memory_groups.sql"), include_str!("../../migrations/0017_pending_actions.sql"),
        include_str!("../../migrations/0018_indexes.sql"), include_str!("../../migrations/0019_screenshots.sql"),
        include_str!("../../migrations/0020_test_runs_at_once.sql"), include_str!("../../migrations/0021_builds_on.sql"),
    ];
    assert_eq!(all.len() as i64, db::SCHEMA_VERSION - 1, "one schema step back");
    let mut c = Connection::open(path).unwrap();
    c.pragma_update(None, "foreign_keys", "OFF").unwrap();
    Migrations::new(all.iter().map(|sql| M::up(sql)).collect()).to_latest(&mut c).unwrap();
    c.pragma_update(None, "foreign_keys", "ON").unwrap();
    c
}

fn schema(c: &Connection) -> Vec<(String, String, Option<String>)> {
    let mut v: Vec<_> = c.prepare("SELECT type, name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'").unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(Result::unwrap).collect();
    v.sort();
    v
}

fn version(c: &Connection) -> i64 {
    c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap()
}

/// The chats, each with the tools its answers used, oldest first, and the mark 0022 gives it.
const CHATS: [(&str, &[&str], Option<&str>); 7] = [
    ("Read Otus", &["mcp__gizai__get_overview", "Read", "mcp__otus__search", "WebFetch"], Some("mcp__otus__search")),
    ("Only Hersir's own", &["mcp__gizai__get_task", "Read", "Glob", "Grep", "mcp__gizai__update_task"], None),
    ("The web", &["WebSearch"], Some("WebSearch")),
    ("The browser", &["mcp__gizai__list_tasks", "mcp__chrome-devtools__navigate_page"], Some("mcp__chrome-devtools__navigate_page")),
    // a server named gizai__notes or gizai_ would pass for Hersir's own: its tools count as outside
    ("Looks like Hersir's", &["mcp__gizai__notes__search"], Some("mcp__gizai__notes__search")),
    ("Trailing underscore", &["mcp__gizai___x"], Some("mcp__gizai___x")),
    ("No tools", &[], None),
];

/// The cards' holds before the upgrade (hold, reason) and whether 0022 gives them GA-101's kind.
const HOLDS: [(Option<&str>, &str, bool); 6] = [
    (Some("needs_decision"), "Waiting for your OK: the Team Lead put this card here after using mcp__otus__search, a tool from outside Gizai. \
                              Check it, then take it off hold.", true),
    (Some("needs_decision"), "Waiting for your OK: the Chief put this card here after using WebFetch, a tool from outside Hersir. Check it, \
                              then take it off hold.", true),
    (Some("needs_decision"), "Which format: CSV or JSON?", false),
    (Some("blocked"), "Waiting for your OK: the Chief put this card here after using WebFetch, a tool from outside Hersir.", false),
    (Some("needs_decision"), "Waiting for your OK: the keys come on Monday.", false),
    (None, "", false),
];

#[test]
fn a_schema_21_database_marks_the_chats_that_used_an_outside_tool_and_ga101s_holds_after_a_backup_and_nothing_else() {
    assert_eq!(db::SCHEMA_VERSION, 22, "0022 is the latest migration");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gizai.db");
    let (threads, cards, messages, you) = {
        let db = Db::open(&path).unwrap();
        let s = seed::ensure_seed(&db, "Jeffrey").unwrap();
        let lead = team::add_agent(&db, &s.you_id, &s.team_id, AgentInput { name: "Team Lead".into(), role_key: "lead".into(),
            chat_enabled: Some(true), ..Default::default() }).unwrap();
        let ga = projects::create(&db, &s.you_id, ProjectInput { name: "Giz AI".into(), key: "GA".into(), ..Default::default() }).unwrap();
        let mut threads = vec![];
        for (title, tools, _) in CHATS {
            let id = chat::create_thread(&db, &s.you_id, &lead, title).unwrap();
            chat::add_message(&db, NewMessage { thread_id: id.clone(), role: "user".into(), author_id: Some(s.you_id.clone()),
                body_md: Some(title.into()), ..Default::default() }).unwrap();
            for tool in tools {
                chat::add_message(&db, NewMessage { thread_id: id.clone(), role: "tool".into(), tool_name: Some(tool.to_string()),
                    tool: Some(serde_json::json!({"id": format!("toolu_{tool}"), "input": {}})), ..Default::default() }).unwrap();
            }
            threads.push(id);
        }
        let mut cards = vec![];
        for (n, (hold, reason, _)) in HOLDS.iter().enumerate() {
            let id = tasks::create(&db, &s.you_id, TaskInput { project_id: ga.clone(), title: format!("Card {n}"), ..Default::default() }).unwrap();
            if let Some(h) = hold {
                tasks::update(&db, &s.you_id, &id, TaskPatch { hold: Some(h.to_string()), hold_reason: Some(reason.to_string()), ..Default::default() }).unwrap();
            }
            cards.push(id);
        }
        let messages: Vec<usize> = threads.iter().map(|t| chat::messages(&db, t).unwrap().len()).collect();
        (threads, cards, messages, s.you_id)
    };
    // one step back to the database the version before GA-261 leaves (schema 21): 0022 only adds the two columns
    let c = Connection::open(&path).unwrap();
    c.execute_batch("ALTER TABLE tasks DROP COLUMN hold_kind; ALTER TABLE chat_threads DROP COLUMN outside_tool; PRAGMA user_version = 21;").unwrap();
    let other = tempfile::tempdir().unwrap();
    let genuine = v21_db(&other.path().join("gizai.db"));
    assert_eq!(schema(&c), schema(&genuine), "the step back is exactly schema 21 (migrations 0001..0021)");
    drop(c);

    let db = Db::open(&path).unwrap();
    db.read(|c| {
        assert_eq!(version(c), 22, "0022 ran");
        for (table, column) in [("chat_threads", "outside_tool"), ("tasks", "hold_kind")] {
            let col: (String, i64, Option<String>) = c.query_row(
                &format!("SELECT type, \"notnull\", dflt_value FROM pragma_table_info('{table}') WHERE name = '{column}'"), [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            assert_eq!(col, ("TEXT".to_string(), 0, None), "{table}.{column}: optional text, empty by default");
        }
        let mut rest = schema(c);
        rest.retain(|(kind, name, _)| !(kind == "table" && (name == "chat_threads" || name == "tasks")));
        let mut old = schema(&genuine);
        old.retain(|(kind, name, _)| !(kind == "table" && (name == "chat_threads" || name == "tasks")));
        assert_eq!(rest, old, "nothing else changed");
        Ok(())
    }).unwrap();
    // each chat marked with its first tool from outside Hersir, or not at all, and no note added
    for ((id, (title, _, want)), n) in threads.iter().zip(CHATS).zip(&messages) {
        assert_eq!(chat::outside_tool(&db, id).unwrap().as_deref(), want, "{title}");
        assert_eq!(chat::messages(&db, id).unwrap().len(), *n, "{title}: no note");
    }
    // GA-101's holds get their kind; every other hold, and a card without one, none; the holds themselves are as they were
    for (id, (hold, reason, waits)) in cards.iter().zip(HOLDS) {
        let t = tasks::get(&db, id).unwrap();
        assert_eq!((t.hold.as_deref(), t.hold_reason.as_deref().unwrap_or_default()), (hold, if hold.is_some() { reason } else { "" }), "{reason}");
        assert_eq!(t.waits_for_your_ok(), waits, "{hold:?} {reason}");
        assert_eq!(t.hold_kind.as_deref(), waits.then_some(HOLD_WAITING_FOR_OK), "{reason}");
    }
    // the backup is the schema 21 database
    let snaps: Vec<_> = std::fs::read_dir(dir.path().join("backups")).unwrap().map(|e| e.unwrap().path()).collect();
    assert!(snaps.len() == 1 && snaps[0].file_name().unwrap().to_string_lossy().starts_with("gizai-before-v22-"), "{snaps:?}");
    let old = Connection::open(&snaps[0]).unwrap();
    assert_eq!((version(&old), schema(&old)), (21, schema(&genuine)), "the backup is the schema 21 database");
    drop(old);
    // a mark stays the first tool, the user's Clear hold takes the kind off with the hold, and opening it again changes nothing
    assert!(!chat::mark_outside(&db, &threads[0], "WebSearch").unwrap(), "marked already");
    assert!(chat::mark_outside(&db, &threads[1], "WebSearch").unwrap(), "marked now");
    tasks::update(&db, &you, &cards[1], TaskPatch { hold: Some(String::new()), ..Default::default() }).unwrap();
    let t = tasks::get(&db, &cards[1]).unwrap();
    assert_eq!((t.hold, t.hold_kind), (None, None));
    drop(db);
    let db = Db::open(&path).unwrap();
    assert_eq!(db.read(|c| Ok(version(c))).unwrap(), 22);
    assert_eq!(chat::outside_tool(&db, &threads[0]).unwrap().as_deref(), Some("mcp__otus__search"));
    assert_eq!(chat::outside_tool(&db, &threads[1]).unwrap().as_deref(), Some("WebSearch"));
    assert_eq!(std::fs::read_dir(dir.path().join("backups")).unwrap().count(), 1, "no new backup");
}
