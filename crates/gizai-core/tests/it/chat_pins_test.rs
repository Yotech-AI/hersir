// GA-257 QA: Chat → Pinned. The pins are the setting `chat_pins` (the chat ids, the newest pin first): pinned_threads lists
// the pinned chats that exist, of any age, the newest pin first; set_pinned pins a chat to the top or unpins it, drops the
// pins of chats that no longer exist and refuses a chat that doesn't exist. No schema change for the pins (SCHEMA_VERSION is
// 22 for GA-261's outside marks), and
// the pins survive a restart (the database opened again) and are in a snapshot of the database, as a shared setting.
use gizai_core::chat::{self, NewMessage};
use gizai_core::model::*;
use gizai_core::{db, db::Db, scope, seed, settings, team};

struct W {
    db: Db,
    you: String,
    lead: String,
}

fn on(db: Db) -> W {
    let s = seed::ensure_seed(&db, "Jeffrey").unwrap();
    let lead = team::add_agent(&db, &s.you_id, &s.team_id,
        AgentInput { name: "Team Lead".into(), role_key: "lead".into(), chat_enabled: Some(true), ..Default::default() }).unwrap();
    W { db, you: s.you_id, lead }
}

fn setup() -> W {
    on(Db::open_in_memory().unwrap())
}

impl W {
    fn chat(&self, title: &str) -> String {
        chat::create_thread(&self.db, &self.you, &self.lead, title).unwrap()
    }
    /// Sets a chat's last activity, so the order doesn't hang on the clock.
    fn at(&self, thread: &str, updated_at: i64) {
        self.db.write(None, |w| { w.conn().execute("UPDATE chat_threads SET updated_at = ?2 WHERE id = ?1", rusqlite::params![thread, updated_at])?; Ok(()) }).unwrap();
    }
    fn delete(&self, thread: &str) {
        self.db.write(None, |w| { w.conn().execute("UPDATE chat_threads SET deleted_at = 1 WHERE id = ?1", [thread])?; Ok(()) }).unwrap();
    }
    fn pin(&self, thread: &str) -> Vec<String> {
        chat::set_pinned(&self.db, thread, true).unwrap()
    }
    fn unpin(&self, thread: &str) -> Vec<String> {
        chat::set_pinned(&self.db, thread, false).unwrap()
    }
    fn pinned(&self) -> Vec<String> {
        chat::pinned_threads(&self.db).unwrap().into_iter().map(|t| t.id).collect()
    }
    fn setting(&self) -> Option<Vec<String>> {
        settings::get(&self.db, chat::PINS).unwrap()
    }
    fn recent(&self) -> Vec<String> {
        chat::recent_threads(&self.db, 30).unwrap().into_iter().map(|t| t.id).collect()
    }
}

/// `n` chats called "Chat 0" … ; chat i's last activity is 1000 + i, so the last one made is the newest.
fn many(w: &W, n: usize) -> Vec<String> {
    (0..n).map(|i| {
        let id = w.chat(&format!("Chat {i}"));
        w.at(&id, 1000 + i as i64);
        id
    }).collect()
}

#[test]
fn the_setting_is_chat_pins_and_no_pins_at_first() {
    let w = setup();
    assert_eq!(chat::PINS, "chat_pins");
    many(&w, 3);
    assert!(w.pinned().is_empty());
    assert_eq!(w.setting(), None, "nothing is written before the first pin");
}

#[test]
fn the_most_recently_pinned_chat_comes_first_whatever_its_activity() {
    let w = setup();
    let ids = many(&w, 5);
    assert_eq!(w.pin(&ids[1]), [ids[1].clone()]);
    assert_eq!(w.pin(&ids[4]), [ids[4].clone(), ids[1].clone()]);
    assert_eq!(w.pin(&ids[0]), [ids[0].clone(), ids[4].clone(), ids[1].clone()]);
    assert_eq!(w.pinned(), [ids[0].clone(), ids[4].clone(), ids[1].clone()], "newest pin first, not newest activity first");
    assert_eq!(w.setting(), Some(vec![ids[0].clone(), ids[4].clone(), ids[1].clone()]), "the setting holds the ids, the newest pin first");
    // a new message in a pinned chat moves nothing under Pinned
    chat::add_message(&w.db, NewMessage { thread_id: ids[1].clone(), role: "user".into(), author_id: Some(w.you.clone()), body_md: Some("Still there?".into()),
        ..Default::default() }).unwrap();
    assert_eq!(w.pinned(), [ids[0].clone(), ids[4].clone(), ids[1].clone()]);
    // pinning a pinned chat again takes it to the top, once
    assert_eq!(w.pin(&ids[1]), [ids[1].clone(), ids[0].clone(), ids[4].clone()]);
    assert_eq!(w.pinned(), [ids[1].clone(), ids[0].clone(), ids[4].clone()]);
}

#[test]
fn pinned_chats_carry_what_recent_shows() {
    let w = setup();
    let id = w.chat("Which database?");
    w.at(&id, 4242);
    w.pin(&id);
    let pinned = chat::pinned_threads(&w.db).unwrap();
    let recent = chat::recent_threads(&w.db, 30).unwrap();
    assert_eq!(pinned.len(), 1);
    // the same chat as Recent has it: title, time, label fields
    assert_eq!(serde_json::to_value(&pinned[0]).unwrap(), serde_json::to_value(&recent[0]).unwrap());
    assert_eq!((pinned[0].title.as_str(), pinned[0].updated_at), ("Which database?", 4242));
}

#[test]
fn a_chat_older_than_the_30_newest_stays_pinned() {
    let w = setup();
    let ids = many(&w, 36);
    // ids[0] .. ids[5] are older than the 30 newest
    assert!(!w.recent().contains(&ids[0]));
    w.pin(&ids[0]);
    w.pin(&ids[33]);
    assert_eq!(w.pinned(), [ids[33].clone(), ids[0].clone()]);
    let pinned = chat::pinned_threads(&w.db).unwrap();
    assert_eq!(pinned[1].title, "Chat 0");
    // pinning changes nothing in Recent: still the 30 newest by activity (the page leaves the pinned ones out)
    let newest_first: Vec<String> = ids.iter().rev().cloned().collect();
    assert_eq!(w.recent(), newest_first[..30]);
}

#[test]
fn unpinning_takes_the_chat_out_of_pinned_only() {
    let w = setup();
    let ids = many(&w, 36);
    w.pin(&ids[0]);
    w.pin(&ids[35]);
    w.pin(&ids[20]);
    assert_eq!(w.unpin(&ids[35]), [ids[20].clone(), ids[0].clone()]);
    assert_eq!(w.pinned(), [ids[20].clone(), ids[0].clone()]);
    // one of the 30 newest: back in Recent at its place
    assert_eq!(w.recent().first(), Some(&ids[35]));
    // an old one: only in the Archive (every chat), not in Recent
    assert_eq!(w.unpin(&ids[0]), [ids[20].clone()]);
    assert!(!w.recent().contains(&ids[0]));
    assert!(chat::search_threads(&w.db, "").unwrap().iter().any(|h| h.thread.id == ids[0]), "the Archive still has it");
    // unpinning a chat that isn't pinned changes nothing and doesn't fail
    assert_eq!(w.unpin(&ids[3]), [ids[20].clone()]);
    assert_eq!(w.unpin(&ids[20]), Vec::<String>::new());
    assert!(w.pinned().is_empty());
    assert_eq!(w.setting(), Some(vec![]));
}

#[test]
fn a_pinned_chat_that_no_longer_exists_drops_out() {
    let w = setup();
    let ids = many(&w, 3);
    w.pin(&ids[0]);
    w.pin(&ids[1]);
    w.pin(&ids[2]);
    w.delete(&ids[1]);
    // at once when read
    assert_eq!(w.pinned(), [ids[2].clone(), ids[0].clone()]);
    // and from the setting at the next pin or unpin
    assert_eq!(w.setting().unwrap().len(), 3);
    assert_eq!(w.unpin(&ids[2]), [ids[0].clone()]);
    assert_eq!(w.setting(), Some(vec![ids[0].clone()]));
    // a pin of an id that was never a chat (a hand-edited setting) is left out as well
    settings::set(&w.db, chat::PINS, &vec!["no-such-chat".to_string(), ids[0].clone()]).unwrap();
    assert_eq!(w.pinned(), [ids[0].clone()]);
    assert_eq!(w.pin(&ids[2]), [ids[2].clone(), ids[0].clone()]);
}

#[test]
fn pinning_a_chat_that_doesnt_exist_fails_and_changes_nothing() {
    let w = setup();
    let ids = many(&w, 2);
    w.pin(&ids[0]);
    let err = chat::set_pinned(&w.db, "no-such-chat", true).unwrap_err().to_string();
    assert!(err.contains("not found: chat no-such-chat"), "{err}");
    w.delete(&ids[1]);
    let err = chat::set_pinned(&w.db, &ids[1], true).unwrap_err().to_string();
    assert!(err.contains("not found"), "a deleted chat can't be pinned: {err}");
    assert_eq!(w.setting(), Some(vec![ids[0].clone()]));
    assert_eq!(w.pinned(), [ids[0].clone()]);
}

#[test]
fn a_broken_setting_reads_as_no_pins() {
    let w = setup();
    let ids = many(&w, 2);
    settings::set(&w.db, chat::PINS, &serde_json::json!({"not": "a list"})).unwrap();
    assert!(w.pinned().is_empty());
    assert_eq!(w.pin(&ids[1]), [ids[1].clone()]);
    assert_eq!(w.pinned(), [ids[1].clone()]);
}

#[test]
fn pins_survive_a_restart_and_are_in_a_snapshot_without_a_schema_change() {
    assert_eq!(db::SCHEMA_VERSION, 22, "the pins need no schema change: 22 is GA-261's 0022_outside_marks.sql");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gizai.db");
    let ids = {
        let w = on(Db::open(&path).unwrap());
        let ids = many(&w, 33);
        w.pin(&ids[32]);
        w.pin(&ids[0]);
        w.pin(&ids[10]);
        w.unpin(&ids[32]);
        ids
    };
    // a restart: the database opened again
    let db = Db::open(&path).unwrap();
    let pinned: Vec<String> = chat::pinned_threads(&db).unwrap().into_iter().map(|t| t.id).collect();
    assert_eq!(pinned, [ids[10].clone(), ids[0].clone()]);
    let version: i64 = db.read(|c| Ok(c.query_row("PRAGMA user_version", [], |r| r.get(0))?)).unwrap();
    assert_eq!(version, db::SCHEMA_VERSION);
    // a snapshot of the database (what backups and the upgrade copy are made of) has them too
    let snap = db::snapshot(&path, &dir.path().join("backups"), "pins").unwrap();
    drop(db);
    let copy = Db::open(&snap).unwrap();
    let pinned: Vec<String> = chat::pinned_threads(&copy).unwrap().into_iter().map(|t| t.id).collect();
    assert_eq!(pinned, [ids[10].clone(), ids[0].clone()]);
    // a shared setting: a restore takes it from the snapshot, and it isn't this computer's
    assert!(!scope::this_computer(chat::PINS));
    assert!(scope::shared(chat::PINS, ""));
}
