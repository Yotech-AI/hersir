// GA-351 QA: a message from Claude Desktop (ask_chief) is saved as yours, marked {"from": "claude-desktop"} in its meta.
// Only that meta says so, and only on a message of yours. A queued one keeps the mark while it waits, after an edit, after a
// restart (held for Send now) and when it goes; a message you typed has none. A database from before (schema 22) keeps
// its queued messages as typed by you.
use std::path::{Path, PathBuf};

use gizai_core::chat::{self, NewMessage};
use gizai_core::db::{self, Db};
use gizai_core::model::*;
use gizai_core::{seed, team};
use rusqlite::Connection;
use serde_json::json;

/// A database at `path` with you, the Chief (Chat on) and a chat with it: (you, thread).
fn setup(path: &Path) -> (Db, String, String) {
    let db = Db::open(path).unwrap();
    let s = seed::ensure_seed(&db, "Jeffrey").unwrap();
    let lead = team::add_agent(&db, &s.you_id, &s.team_id, AgentInput { name: "Team Lead".into(), role_key: "lead".into(), chat_enabled: Some(true),
        ..Default::default() }).unwrap();
    let thread = chat::create_thread(&db, &s.you_id, &lead, "From Claude Desktop").unwrap();
    (db, s.you_id, thread)
}

fn db_path(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("gizai.db")
}

#[test]
fn only_from_claude_desktop_in_a_messages_meta_says_it_came_from_claude_desktop() {
    assert_eq!(chat::CLAUDE_DESKTOP, "claude-desktop");
    assert_eq!(chat::desktop_meta(), json!({"from": "claude-desktop"}));
    assert!(chat::from_desktop(Some(&chat::desktop_meta())));
    assert!(chat::from_desktop(Some(&json!({"from": "claude-desktop", "other": 1}))));
    for other in [json!({"from": "claude"}), json!({"from": "Claude-Desktop"}), json!({"from": " claude-desktop"}), json!({"from": true}),
                  json!({"kind": "switch", "cli": "claude", "cliName": "Claude Code"}), json!({"client": "claude-desktop"}),
                  json!("claude-desktop"), json!(["claude-desktop"]), json!(null), json!({})] {
        assert!(!chat::from_desktop(Some(&other)), "{other}");
    }
    assert!(!chat::from_desktop(None));
}

#[test]
fn a_sent_message_keeps_its_mark_and_only_a_message_of_yours_is_from_claude_desktop() {
    let dir = tempfile::tempdir().unwrap();
    let (db, you, thread) = setup(&db_path(&dir));
    let say = |role: &str, text: &str, meta: Option<serde_json::Value>| {
        chat::add_message(&db, NewMessage { thread_id: thread.clone(), role: role.into(), author_id: (role == "user").then(|| you.clone()),
            body_md: Some(text.into()), meta, ..Default::default() }).unwrap()
    };
    let desk = say("user", "Which cards are stuck?", Some(chat::desktop_meta()));
    let typed = say("user", "And KADE-1?", None);
    let note = say("system", "A note", Some(chat::desktop_meta()));
    assert!(desk.from_desktop());
    let all = chat::messages(&db, &thread).unwrap();
    let got: Vec<(&str, Option<serde_json::Value>, bool)> = all.iter().map(|m| (m.id.as_str(), m.meta.clone(), m.from_desktop())).collect();
    assert_eq!(got, [(desk.id.as_str(), Some(json!({"from": "claude-desktop"})), true), (typed.id.as_str(), None, false),
                     (note.id.as_str(), Some(json!({"from": "claude-desktop"})), false)],
               "read again: the mark stays, and only your message counts");
    // what the Chat page gets: meta, camelCase like the rest
    let v = serde_json::to_value(&all[0]).unwrap();
    assert_eq!(v["meta"], json!({"from": "claude-desktop"}));
}

#[test]
fn a_queued_message_keeps_its_mark_while_it_waits_after_an_edit_and_a_restart_and_when_it_goes() {
    let dir = tempfile::tempdir().unwrap();
    let path = db_path(&dir);
    let (db, you, thread) = setup(&path);
    let desk = chat::enqueue_with_meta(&db, &you, &thread, "Which cards are stuck?", &[], Some(&chat::desktop_meta())).unwrap();
    assert_eq!(desk.meta, Some(json!({"from": "claude-desktop"})));
    let typed = chat::enqueue(&db, &you, &thread, "And KADE-1?").unwrap();
    assert_eq!(typed.meta, None, "one you typed has none");
    let plain = chat::enqueue_with_meta(&db, &you, &thread, "No details", &[], None).unwrap();
    assert_eq!(plain.meta, None);
    let edited = chat::edit_queued(&db, &you, &desk.id, "Which cards are stuck, and why?").unwrap();
    assert_eq!(edited.meta, Some(json!({"from": "claude-desktop"})), "an edit keeps it");
    // the Chat page gets it with the queued message
    assert_eq!(serde_json::to_value(&edited).unwrap()["meta"], json!({"from": "claude-desktop"}));
    // a restart: the database opened again, the queue held for Send now as at start-up
    drop(db);
    let db = Db::open(&path).unwrap();
    chat::hold_all_queues(&db).unwrap();
    let queue = chat::queue(&db, &thread).unwrap();
    assert_eq!(queue.iter().map(|q| (q.id.as_str(), q.held, q.meta.clone())).collect::<Vec<_>>(),
               [(desk.id.as_str(), true, Some(json!({"from": "claude-desktop"}))), (typed.id.as_str(), true, None), (plain.id.as_str(), true, None)]);
    // Send now: each goes with its id and its details
    let sent = chat::send_queued(&db, &thread, true).unwrap();
    let want = [(desk.id.clone(), Some(json!({"from": "claude-desktop"})), true), (typed.id.clone(), None, false), (plain.id.clone(), None, false)];
    assert_eq!(sent.iter().map(|m| (m.id.clone(), m.meta.clone(), m.from_desktop())).collect::<Vec<_>>(), want);
    let read: Vec<_> = chat::messages(&db, &thread).unwrap().iter().map(|m| (m.id.clone(), m.meta.clone(), m.from_desktop())).collect();
    assert_eq!(read, want, "read again");
    assert_eq!(sent[0].body_md.as_deref(), Some("Which cards are stuck, and why?"));
    assert!(chat::queue(&db, &thread).unwrap().is_empty());
}

#[test]
fn a_message_that_goes_by_itself_keeps_its_mark_and_a_removed_one_takes_it_along() {
    let dir = tempfile::tempdir().unwrap();
    let (db, you, thread) = setup(&db_path(&dir));
    let gone = chat::enqueue_with_meta(&db, &you, &thread, "Never mind", &[], Some(&chat::desktop_meta())).unwrap();
    chat::remove_queued(&db, &you, &gone.id).unwrap();
    let desk = chat::enqueue_with_meta(&db, &you, &thread, "Which cards are stuck?", &[], Some(&chat::desktop_meta())).unwrap();
    let sent = chat::send_queued(&db, &thread, false).unwrap();
    assert_eq!(sent.iter().map(|m| (m.id.clone(), m.from_desktop())).collect::<Vec<_>>(), [(desk.id.clone(), true)]);
    let n: i64 = db.read(|c| Ok(c.query_row("SELECT count(*) FROM chat_queue", [], |r| r.get(0))?)).unwrap();
    assert_eq!(n, 0);
}

#[test]
fn a_schema_22_database_keeps_its_queued_messages_as_typed_by_you() {
    let dir = tempfile::tempdir().unwrap();
    let path = db_path(&dir);
    let (thread, id) = {
        let (db, you, thread) = setup(&path);
        let q = chat::enqueue(&db, &you, &thread, "Waiting since 0.20.0").unwrap();
        (thread, q.id)
    };
    // three steps back: the database 0.20.0 leaves (schema 22), without the column (and without Docs' 0024 and 0025 after it)
    let c = Connection::open(&path).unwrap();
    c.execute_batch("DROP TABLE docs_baselines; DROP TABLE docs_decisions; DROP TABLE docs_changes; ALTER TABLE projects DROP COLUMN docs_off; \
        ALTER TABLE projects DROP COLUMN dev_docs; ALTER TABLE chat_queue DROP COLUMN meta_json; PRAGMA user_version = 22;").unwrap();
    drop(c);
    let db = Db::open(&path).unwrap();
    let version: i64 = db.read(|c| Ok(c.query_row("PRAGMA user_version", [], |r| r.get(0))?)).unwrap();
    assert_eq!(version, db::SCHEMA_VERSION);
    let col: (String, i64) = db.read(|c| Ok(c.query_row("SELECT type, \"notnull\" FROM pragma_table_info('chat_queue') WHERE name='meta_json'", [],
        |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap();
    assert_eq!(col, ("TEXT".to_string(), 0), "an optional text");
    let queue = chat::queue(&db, &thread).unwrap();
    assert_eq!(queue.iter().map(|q| (q.id.as_str(), q.body_md.as_str(), q.meta.clone())).collect::<Vec<_>>(),
               [(id.as_str(), "Waiting since 0.20.0", None)]);
    let sent = chat::send_queued(&db, &thread, true).unwrap();
    assert_eq!((sent[0].meta.clone(), sent[0].from_desktop()), (None, false));
}
