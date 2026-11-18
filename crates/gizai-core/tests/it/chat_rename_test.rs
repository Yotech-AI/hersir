// GA-309 QA: Rename on a chat on the left (chat::rename_thread). The name is made one line (runs of whitespace one space)
// and trimmed; an empty name and one over NAME_MAX (80) characters are refused with the card's words; a chat that doesn't
// exist or is deleted is NotFound; the chat's last activity (updated_at) stays, so it keeps its place in Recent and under
// Pinned; the new name is kept across a restart, is what the Archive's search and the Inbox find, and a rename in the
// middle of the Chief's answer doesn't stop or lose it. New chats still get their title from the first message.
use gizai_core::chat::{self, NewMessage, Totals};
use gizai_core::model::*;
use gizai_core::{db::Db, seed, team, Error};

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
    fn chat(&self, first: &str) -> String {
        chat::create_thread(&self.db, &self.you, &self.lead, first).unwrap()
    }
    fn at(&self, thread: &str, updated_at: i64) {
        self.db.write(None, |w| { w.conn().execute("UPDATE chat_threads SET updated_at = ?2 WHERE id = ?1", rusqlite::params![thread, updated_at])?; Ok(()) }).unwrap();
    }
    fn delete(&self, thread: &str) {
        self.db.write(None, |w| { w.conn().execute("UPDATE chat_threads SET deleted_at = 1 WHERE id = ?1", [thread])?; Ok(()) }).unwrap();
    }
    fn rename(&self, thread: &str, name: &str) -> Result<chat::ChatThread, Error> {
        chat::rename_thread(&self.db, &self.you, thread, name)
    }
    fn title(&self, thread: &str) -> String {
        self.db.read(|c| Ok(c.query_row("SELECT title FROM chat_threads WHERE id = ?1", [thread], |r| r.get::<_, String>(0))?)).unwrap()
    }
    fn updated_at(&self, thread: &str) -> i64 {
        self.db.read(|c| Ok(c.query_row("SELECT updated_at FROM chat_threads WHERE id = ?1", [thread], |r| r.get::<_, i64>(0))?)).unwrap()
    }
    fn recent(&self) -> Vec<String> {
        chat::recent_threads(&self.db, 30).unwrap().into_iter().map(|t| t.id).collect()
    }
    fn pinned(&self) -> Vec<String> {
        chat::pinned_threads(&self.db).unwrap().into_iter().map(|t| t.id).collect()
    }
    fn found(&self, query: &str) -> Vec<String> {
        chat::search_threads(&self.db, query).unwrap().into_iter().map(|h| h.thread.id).collect()
    }
    fn say(&self, thread: &str, role: &str, body: &str) {
        let author = if role == "user" { self.you.clone() } else { self.lead.clone() };
        chat::add_message(&self.db, NewMessage { thread_id: thread.into(), role: role.into(), author_id: Some(author), body_md: Some(body.into()), ..Default::default() })
            .unwrap();
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

fn invalid(r: Result<chat::ChatThread, Error>) -> String {
    match r {
        Err(Error::Invalid(m)) => m,
        other => panic!("expected Invalid, got {other:?}"),
    }
}

fn not_found(r: Result<chat::ChatThread, Error>) -> String {
    match r {
        Err(Error::NotFound(m)) => m,
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn a_name_is_saved_on_one_line_trimmed_and_returned() {
    let w = setup();
    let id = w.chat("For [DOQ studio](gizai:project/DOQ) the new landing page please");
    let t = w.rename(&id, "  Hersir   sidebar  ").unwrap();
    assert_eq!(t.id, id);
    assert_eq!(t.title, "Hersir sidebar");
    assert_eq!(w.title(&id), "Hersir sidebar");
    assert_eq!(chat::get_thread(&w.db, &id).unwrap().title, "Hersir sidebar");
    // new lines and tabs are whitespace too
    assert_eq!(w.rename(&id, "\tPlan\n\nfor  the\r\nweek \n").unwrap().title, "Plan for the week");
    assert_eq!(w.title(&id), "Plan for the week");
}

#[test]
fn an_empty_name_or_only_whitespace_is_refused_and_the_old_title_stays() {
    let w = setup();
    let id = w.chat("Which database?");
    for name in ["", " ", "   \n\t  "] {
        assert_eq!(invalid(w.rename(&id, name)), "Give the chat a name", "{name:?}");
        assert_eq!(w.title(&id), "Which database?");
    }
}

#[test]
fn up_to_80_characters_are_taken_and_81_are_refused() {
    let w = setup();
    let id = w.chat("Which database?");
    assert_eq!(chat::NAME_MAX, 80);
    let eighty = "a".repeat(80);
    assert_eq!(w.rename(&id, &eighty).unwrap().title, eighty);
    assert_eq!(invalid(w.rename(&id, &"b".repeat(81))), "A chat's name is at most 80 characters");
    assert_eq!(w.title(&id), eighty, "a refused name changes nothing");
    // characters, not bytes: 80 accented letters (160 bytes) are fine, 81 aren't
    let accents = "é".repeat(80);
    assert_eq!(w.rename(&id, &accents).unwrap().title, accents);
    assert_eq!(invalid(w.rename(&id, &"é".repeat(81))), "A chat's name is at most 80 characters");
    // counted after the spaces are made one: 80 characters with spaces around them and doubled inside are fine
    let spaced = format!("   {}  {}   ", "c".repeat(40), "d".repeat(39));
    assert_eq!(w.rename(&id, &spaced).unwrap().title.chars().count(), 80);
}

#[test]
fn a_title_from_a_long_first_message_can_be_edited_without_hitting_the_limit() {
    let w = setup();
    let id = w.chat(&"word ".repeat(40));
    let title = w.title(&id);
    assert!(title.ends_with('…'));
    assert!(title.chars().count() <= 61);
    // the user adds a few words in front of it
    let edited = format!("Draft: {title}");
    assert!(edited.chars().count() <= 80);
    assert_eq!(w.rename(&id, &edited).unwrap().title, edited);
}

#[test]
fn a_chat_that_doesnt_exist_or_is_deleted_is_not_found() {
    let w = setup();
    assert_eq!(not_found(w.rename("nope", "A name")), "chat nope");
    let id = w.chat("Which database?");
    w.delete(&id);
    assert_eq!(not_found(w.rename(&id, "A name")), format!("chat {id}"));
    assert_eq!(w.title(&id), "Which database?", "a deleted chat isn't renamed");
}

#[test]
fn the_name_is_checked_before_the_chat_is_looked_up() {
    let w = setup();
    // An empty name for a chat that doesn't exist says what is wrong with the name.
    assert_eq!(invalid(w.rename("nope", "  ")), "Give the chat a name");
}

#[test]
fn a_rename_keeps_updated_at_and_the_chat_keeps_its_place_in_recent() {
    let w = setup();
    let ids = many(&w, 5);
    let before = w.recent();
    assert_eq!(before, ids.iter().rev().cloned().collect::<Vec<_>>());
    let t = w.rename(&ids[1], "An older chat, renamed").unwrap();
    assert_eq!(t.updated_at, 1001);
    assert_eq!(w.updated_at(&ids[1]), 1001);
    assert_eq!(w.recent(), before, "a rename doesn't move the chat to the top");
    assert_eq!(chat::recent_threads(&w.db, 30).unwrap()[3].title, "An older chat, renamed");
}

#[test]
fn a_pinned_chat_keeps_its_place_under_pinned() {
    let w = setup();
    let ids = many(&w, 4);
    chat::set_pinned(&w.db, &ids[0], true).unwrap();
    chat::set_pinned(&w.db, &ids[2], true).unwrap();
    chat::set_pinned(&w.db, &ids[3], true).unwrap();
    let before = w.pinned();
    w.rename(&ids[2], "The middle pin").unwrap();
    assert_eq!(w.pinned(), before);
    let titles: Vec<String> = chat::pinned_threads(&w.db).unwrap().into_iter().map(|t| t.title).collect();
    assert_eq!(titles, ["Chat 3", "The middle pin", "Chat 0"]);
}

#[test]
fn a_rename_keeps_the_chats_other_fields() {
    let w = setup();
    let id = w.chat("Which database?");
    w.say(&id, "user", "Which database?");
    chat::record_session(&w.db, &id, "sess-1", Totals { cost_usd_micros: 1200, input_tokens: 30, output_tokens: 4 }).unwrap();
    let before = chat::get_thread(&w.db, &id).unwrap();
    let after = w.rename(&id, "Database choice").unwrap();
    assert_eq!(after.title, "Database choice");
    assert_eq!((after.agent_id, after.session_id, after.created_at, after.updated_at, after.cost_usd_micros, after.input_tokens, after.output_tokens,
                after.created_by, after.kind, after.cli, after.answered_at),
               (before.agent_id, before.session_id, before.created_at, before.updated_at, before.cost_usd_micros, before.input_tokens,
                before.output_tokens, before.created_by, before.kind, before.cli, before.answered_at));
    assert_eq!(chat::messages(&w.db, &id).unwrap().len(), 1, "messages stay");
}

#[test]
fn a_rename_is_written_as_a_change_of_the_title_by_you() {
    let w = setup();
    let id = w.chat("Which database?");
    w.rename(&id, "Database choice").unwrap();
    let (op, actor, diff): (String, Option<String>, String) = w.db.read(|c| Ok(c.query_row(
        "SELECT op, actor_id, diff_json FROM changes WHERE table_name = 'chat_threads' AND row_id = ?1 ORDER BY rowid DESC LIMIT 1", [&id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?)).unwrap();
    assert_eq!(op, "update");
    assert_eq!(actor.as_deref(), Some(w.you.as_str()));
    assert_eq!(serde_json::from_str::<serde_json::Value>(&diff).unwrap(), serde_json::json!({"title": "Database choice"}));
}

#[test]
fn a_refused_rename_writes_no_change() {
    let w = setup();
    let id = w.chat("Which database?");
    let count = |w: &W| -> i64 { w.db.read(|c| Ok(c.query_row("SELECT count(*) FROM changes WHERE table_name = 'chat_threads'", [], |r| r.get(0))?)).unwrap() };
    let before = count(&w);
    let _ = w.rename(&id, "");
    let _ = w.rename(&id, &"x".repeat(81));
    let _ = w.rename("nope", "A name");
    assert_eq!(count(&w), before);
}

#[test]
fn the_archives_search_finds_the_chat_by_its_new_name() {
    let w = setup();
    let id = w.chat("For [DOQ studio](gizai:project/DOQ) the landing page");
    let other = w.chat("Something else");
    assert!(w.found("Hersir sidebar").is_empty());
    w.rename(&id, "Hersir sidebar").unwrap();
    assert_eq!(w.found("Hersir sidebar"), [id.clone()]);
    assert_eq!(w.found("hersir SIDE"), [id.clone()], "case ignored, as for any title");
    // the old title isn't its name any more (no message holds it)
    assert!(!w.found("DOQ studio").contains(&id));
    assert!(!w.found("Hersir").contains(&other));
}

#[test]
fn a_chief_chat_that_waits_shows_its_new_name_in_the_inbox_and_still_waits() {
    let w = setup();
    let (id, _) = chat::start_lead_chat(&w.db, &w.lead, "May I merge KADE-1?", "approval", &[], "May I merge KADE-1?", None).unwrap();
    w.rename(&id, "Merge KADE-1").unwrap();
    let waiting = chat::waiting_lead_chats(&w.db).unwrap();
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].id, id);
    assert_eq!(waiting[0].title, "Merge KADE-1");
    assert!(waiting[0].waiting, "a rename isn't an answer");
    assert_eq!(waiting[0].kind.as_deref(), Some("approval"));
}

#[test]
fn a_rename_while_the_chief_answers_keeps_the_name_and_the_answer_goes_on() {
    let w = setup();
    let id = w.chat("Which database?");
    w.say(&id, "user", "Which database?");
    w.at(&id, 5000);
    // the Chief's answer is being written: a tool call and the start of the text, then the rename
    w.say(&id, "agent", "Let me look at the project first.");
    w.rename(&id, "Database choice").unwrap();
    // the answer goes on and ends: more text, the session and totals saved
    w.say(&id, "agent", "SQLite fits: one user, one file.");
    chat::record_session(&w.db, &id, "sess-2", Totals { cost_usd_micros: 5000, input_tokens: 100, output_tokens: 20 }).unwrap();
    let t = chat::get_thread(&w.db, &id).unwrap();
    assert_eq!(t.title, "Database choice", "the answer's writes don't put the old title back");
    assert_eq!(t.session_id.as_deref(), Some("sess-2"));
    let bodies: Vec<Option<String>> = chat::messages(&w.db, &id).unwrap().into_iter().map(|m| m.body_md).collect();
    assert_eq!(bodies.len(), 3);
    assert_eq!(bodies[2].as_deref(), Some("SQLite fits: one user, one file."));
    assert!(t.updated_at > 5000, "the answer's own messages still move the chat up, as before");
}

#[test]
fn a_new_name_survives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gizai.db");
    let id = {
        let w = on(Db::open(&path).unwrap());
        let id = w.chat("Which database?");
        w.rename(&id, "Database choice").unwrap();
        id
    };
    let db = Db::open(&path).unwrap();
    assert_eq!(chat::get_thread(&db, &id).unwrap().title, "Database choice");
}

#[test]
fn new_chats_still_get_their_title_from_the_first_message() {
    let w = setup();
    let a = w.chat("Which database?");
    w.rename(&a, "Database choice").unwrap();
    let b = w.chat("  How do we\ndeploy   this?  ");
    assert_eq!(w.title(&b), "How do we deploy this?");
    assert_eq!(w.title(&a), "Database choice");
}
