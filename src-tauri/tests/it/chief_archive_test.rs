//! GA-307 QA: the Chief's archive_tasks and restore_tasks, called the way the MCP server calls them.
//! - archive_tasks archives the cards `tasks` names, or (`all_done`) every card in the team's Done column, only a
//!   project's with `project`; a card outside Done, an archived card and one an agent is working on are skipped with why.
//!   Neither or both of `tasks` and `all_done` is refused with why, and nothing changes.
//! - Its answer: the count, at most 50 identifiers with `more` for the rest, and every skipped card with its reason.
//! - restore_tasks puts archived cards back at the bottom of Done; one that isn't archived is skipped with why.
//! - A board check can't use either. After a tool from outside Hersir archive_tasks waits for the user's Approve under
//!   the answer ("Archive N cards in Done"); restore_tasks runs at once.
//! - One call is one write (GA-306's archive_many and restore_many) and one `rows-changed`.
// Linux and macOS only: these tests run shell scripts as fake programs, which Windows can't start.
#![cfg(unix)]
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gizai_core::model::*;
use gizai_core::{board as core_board, chat, projects, settings, tasks, team};
use gizai_lib::{AppState, chat as app_chat, pending, runs, tools};
use serde_json::{Value, json};

const FAKE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude.sh");

/// The tool's description, word for word (the card's text).
const DESCRIPTION: &str = "Archives cards in Done: they leave the board and every list, and keep their ID, comments, runs and branch. \
    list_tasks with archived: true lists them, and restore_tasks puts them back. Give tasks, or all_done for every card in Done. A card \
    outside Done, or one an agent is working on, is skipped with why; the others are archived.";

struct T {
    st: AppState,
    lead: String,
    /// The tables of each `rows-changed` the app sent, in order.
    heard: Arc<Mutex<Vec<&'static str>>>,
    dir: tempfile::TempDir,
}

/// A Chief (Team Lead, Chat on) on the one team; projects Kade portal (KADE) and Blog (BLOG), no cards yet. The queue is
/// paused, so no card starts by itself.
fn setup() -> T {
    let dir = tempfile::tempdir().unwrap();
    let mut st = gizai_lib::test_state(dir.path());
    let heard: Arc<Mutex<Vec<&'static str>>> = Arc::default();
    let h = heard.clone();
    st.notify = Arc::new(move |n| {
        if let runs::Note::RowsChanged(table) = n {
            h.lock().unwrap().push(table);
        }
    });
    settings::set(&st.db, "claude_bin", &FAKE.to_string()).unwrap();
    settings::set(&st.db, "agents_paused", &true).unwrap();
    let team_id = team::list(&st.db).unwrap()[0].id.clone();
    let lead = team::add_agent(&st.db, &st.you_id, &team_id, AgentInput {
        name: "Team Lead".into(), role_key: "lead".into(), chat_enabled: Some(true), ..Default::default() }).unwrap();
    T { st, lead, heard, dir }
}

impl T {
    async fn call(&self, name: &str, args: Value) -> Result<Value, String> { tools::call(&self.st, &self.lead, name, args).await }
    async fn ok(&self, name: &str, args: Value) -> Value { self.call(name, args).await.unwrap_or_else(|e| panic!("{name} failed: {e}")) }
    async fn call_in(&self, thread: &str, name: &str, args: Value) -> Result<Value, String> {
        tools::call_in(&self.st, &self.lead, Some(thread), name, args).await
    }
    fn team_id(&self) -> String { team::list(&self.st.db).unwrap()[0].id.clone() }
    fn team_name(&self) -> String { team::list(&self.st.db).unwrap()[0].name.clone() }
    fn project(&self, name: &str, key: &str, team_id: Option<&str>) -> String {
        match projects::list(&self.st.db).unwrap().into_iter().find(|p| p.key == key) {
            Some(p) => p.id,
            None => projects::create(&self.st.db, &self.st.you_id, ProjectInput { name: name.into(), key: key.into(),
                team_id: team_id.map(String::from), ..Default::default() }).unwrap(),
        }
    }
    fn col(&self, team_id: &str, name: &str) -> String {
        team::get(&self.st.db, team_id).unwrap().states.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("no column {name}")).id.clone()
    }
    /// A card in `column` of the first team's board, at its bottom: its identifier.
    fn card(&self, key: &str, column: &str, title: &str) -> String {
        let name = if key == "KADE" { "Kade portal" } else { "Blog" };
        let project = self.project(name, key, None);
        let id = tasks::create(&self.st.db, &self.st.you_id, TaskInput { project_id: project, title: title.into(),
            state_id: Some(self.col(&self.team_id(), column)), ..Default::default() }).unwrap();
        tasks::get(&self.st.db, &id).unwrap().identifier
    }
    fn all(&self) -> Vec<Task> {
        tasks::list(&self.st.db, &TaskFilter::default()).unwrap().into_iter().chain(tasks::archived(&self.st.db, None).unwrap()).collect()
    }
    fn id(&self, identifier: &str) -> String {
        self.all().into_iter().find(|t| t.identifier == identifier).unwrap_or_else(|| panic!("no {identifier}")).id
    }
    fn task(&self, identifier: &str) -> Task { tasks::get(&self.st.db, &self.id(identifier)).unwrap() }
    fn archived(&self, identifier: &str) -> bool { self.task(identifier).archived_at.is_some() }
    /// The cards in the bin, by identifier, sorted.
    fn bin(&self) -> Vec<String> {
        let mut v: Vec<String> = tasks::archived(&self.st.db, None).unwrap().into_iter().map(|t| t.identifier).collect();
        v.sort();
        v
    }
    /// The first team's column `name`, top to bottom, by identifier.
    fn column(&self, name: &str) -> Vec<String> {
        tasks::list(&self.st.db, &TaskFilter::default()).unwrap().into_iter().filter(|t| t.state_name == name).map(|t| t.identifier).collect()
    }
    fn lead_name(&self) -> String { team::agent(&self.st.db, &self.lead).unwrap().name }
    /// The `rows-changed` heard since the last call, and forgets them.
    fn heard(&self) -> Vec<&'static str> { std::mem::take(&mut *self.heard.lock().unwrap()) }
    /// A chat where you said `text`, whose answer used mcp__otus__search, a tool from outside Hersir.
    fn marked(&self, text: &str) -> String {
        let id = chat::create_thread(&self.st.db, &self.st.you_id, &self.lead, text).unwrap();
        chat::add_message(&self.st.db, chat::NewMessage { thread_id: id.clone(), role: "user".into(), author_id: Some(self.st.you_id.clone()),
            body_md: Some(text.into()), ..Default::default() }).unwrap();
        app_chat::mark_outside(&self.st, &id, "mcp__otus__search");
        id
    }
}

fn listed(v: &Value) -> Vec<String> {
    v["tasks"].as_array().unwrap().iter().map(|t| t["task"].as_str().unwrap().to_string()).collect()
}

/// KADE-1 Export invoices (Done), KADE-2 Old import (Done), KADE-3 Import drivers (To do), KADE-4 Release notes (Deploy);
/// BLOG-1 New post (Done).
fn cards(t: &T) {
    for (key, column, title) in [("KADE", "Done", "Export invoices"), ("KADE", "Done", "Old import"), ("KADE", "To do", "Import drivers"),
                                 ("KADE", "Deploy", "Release notes"), ("BLOG", "Done", "New post")] {
        t.card(key, column, title);
    }
}

// ---- the tools in the Chief's list ----

#[test]
fn archive_tasks_and_restore_tasks_are_in_the_chiefs_tools_with_the_cards_words() {
    let all = tools::catalog();
    let archive = all.iter().find(|t| t.name == "archive_tasks").expect("archive_tasks");
    assert_eq!(archive.description, DESCRIPTION);
    assert!(!archive.read_only);
    let s = &archive.input_schema;
    assert_eq!(s["properties"]["tasks"], json!({"type": "array", "items": {"type": "string"}, "description": "The cards' identifiers, like [\"GA-12\"]"}));
    assert_eq!(s["properties"]["all_done"], json!({"type": "boolean", "description": "Every card in the team's Done column instead"}));
    assert_eq!(s["properties"]["project"], json!({"type": "string", "description": "With all_done: only this project's cards, by key, name or id"}));
    let add_column = all.iter().find(|t| t.name == "add_column").unwrap();
    assert_eq!(s["properties"]["team"], add_column.input_schema["properties"]["team"], "TEAM as add_column has it");
    assert_eq!(s["required"], json!([]), "exactly one of tasks and all_done, checked by the tool");

    let restore = all.iter().find(|t| t.name == "restore_tasks").expect("restore_tasks");
    assert_eq!(restore.description, "Puts archived cards back at the bottom of their Done column, as they were.");
    assert!(!restore.read_only);
    assert_eq!(restore.input_schema["properties"]["tasks"]["type"], "array");
    assert_eq!(restore.input_schema["required"], json!(["tasks"]));
}

// ---- archive_tasks with tasks ----

#[tokio::test]
async fn archive_tasks_with_tasks_archives_exactly_those_cards_in_done_and_list_tasks_moves_them_to_the_bin() {
    let t = setup();
    cards(&t);
    t.heard();
    let a = t.ok("archive_tasks", json!({"tasks": ["KADE-1", "kade-2"]})).await;
    assert_eq!(a, json!({"ok": true, "done": "archived", "archived": 2, "tasks": ["KADE-1", "KADE-2"], "skipped": []}), "no more: none are left");
    assert_eq!(t.heard(), ["tasks"], "one rows-changed");
    assert_eq!(t.bin(), ["KADE-1", "KADE-2"], "exactly those cards");
    assert_eq!(t.column("Done"), ["BLOG-1"]);

    // list_tasks leaves them out, archived: true lists them, get_task says archived
    let open = t.ok("list_tasks", json!({"include_done": true})).await;
    let on_board = listed(&open);
    assert!(!on_board.contains(&"KADE-1".to_string()) && !on_board.contains(&"KADE-2".to_string()), "{open}");
    assert!(on_board.contains(&"BLOG-1".to_string()) && on_board.contains(&"KADE-3".to_string()), "{open}");
    let mut bin = listed(&t.ok("list_tasks", json!({"archived": true})).await);
    bin.sort();
    assert_eq!(bin, ["KADE-1", "KADE-2"]);
    let got = t.ok("get_task", json!({"task": "KADE-1"})).await;
    assert_eq!((got["task"]["archived"].as_bool(), got["task"]["archived_by"].as_str()), (Some(true), Some(t.lead_name().as_str())));

    // the activity says the Chief archived them, as a single archive's does; they keep their column
    for id in ["KADE-1", "KADE-2"] {
        let task = t.task(id);
        assert_eq!((task.state_name.as_str(), task.archived_by.as_deref()), ("Done", Some(t.lead_name().as_str())));
        let last = tasks::activity(&t.st.db, &task.id).unwrap().pop().unwrap();
        assert_eq!((last.table.as_str(), last.op.as_str(), last.actor_name.as_deref()), ("tasks", "delete", Some(t.lead_name().as_str())));
    }
    // by its id too
    let id = t.id("BLOG-1");
    let b = t.ok("archive_tasks", json!({"tasks": [id]})).await;
    assert_eq!((b["archived"].as_u64(), &b["tasks"]), (Some(1), &json!(["BLOG-1"])));
}

#[tokio::test]
async fn a_card_outside_done_an_archived_card_an_unknown_one_and_one_an_agent_is_working_on_are_skipped_with_why() {
    let t = setup();
    let repo = git_repo(t.dir.path());
    // the KADE project with a repository, and its card KADE-1 assigned to the Backend Agent, which hangs once started
    let busy = gizai_lib::test_task(&t.st, repo.to_str().unwrap(), "backend");
    tasks::update(&t.st.db, &t.st.you_id, &busy, TaskPatch { description_md: Some("FAKE_HANG".into()), ..Default::default() }).unwrap();
    let done = t.col(&t.team_id(), "Done");
    tasks::move_to(&t.st.db, &t.st.you_id, &busy, &done, "").unwrap();
    for (column, title) in [("Done", "Export"), ("Review", "In review"), ("Done", "Archived before"), ("Done", "Also done")] {
        t.card("KADE", column, title);
    }
    tasks::archive(&t.st.db, &t.st.you_id, &t.id("KADE-4")).unwrap();
    let be = team::get(&t.st.db, &t.team_id()).unwrap().members.iter().find(|m| m.name == "Backend Agent").unwrap().actor_id.clone();
    settings::set(&t.st.db, "agents_paused", &false).unwrap();
    let (run, finished) = runs::start(&t.st, &busy, Some(be), Some(FAKE.into()), "manual").await.unwrap();
    assert!(runs::working_on(&t.st, &busy));
    t.heard();

    let a = t.ok("archive_tasks", json!({"tasks": ["KADE-1", "KADE-2", "KADE-3", "KADE-4", "KADE-99", "KADE-5"]})).await;
    assert_eq!(a, json!({"ok": true, "done": "archived", "archived": 2, "tasks": ["KADE-2", "KADE-5"], "skipped": [
        {"task": "KADE-1", "reason": "An agent is working on this card"},
        {"task": "KADE-3", "reason": "Only a card in Done can be archived, and KADE-3 is in Review"},
        {"task": "KADE-4", "reason": "KADE-4 is already archived"},
        {"task": "KADE-99", "reason": "No task KADE-99: check the identifier with list_tasks"},
    ]}));
    assert_eq!(t.heard(), ["tasks"]);
    assert_eq!(t.bin(), ["KADE-2", "KADE-4", "KADE-5"], "the others are archived");
    assert!(!t.archived("KADE-1") && !t.archived("KADE-3"), "a skipped card stays on the board");
    assert_eq!(t.task("KADE-3").state_name, "Review");

    // the same with all_done: the card the agent works on stays, the rest of Done goes
    let k6 = t.card("KADE", "Done", "Done later");
    let b = t.ok("archive_tasks", json!({"all_done": true})).await;
    assert_eq!(b, json!({"ok": true, "done": "archived", "archived": 1, "tasks": [k6],
                         "skipped": [{"task": "KADE-1", "reason": "An agent is working on this card"}]}));
    runs::stop(&t.st, &run);
    finished.await.unwrap();
    runs::stop_all(&t.st, std::time::Duration::from_secs(10)).await;
    assert!(!runs::working_on(&t.st, &busy));
    assert_eq!(t.ok("archive_tasks", json!({"tasks": ["KADE-1"]})).await["archived"], 1, "once the agent is done, it is archived");
}

// ---- archive_tasks with all_done ----

#[tokio::test]
async fn all_done_archives_every_card_in_the_teams_done_column_top_to_bottom_and_project_only_that_projects() {
    let t = setup();
    cards(&t);
    t.card("KADE", "Done", "Third"); // KADE-5
    t.heard();
    // with project: only that project's cards, top to bottom
    let a = t.ok("archive_tasks", json!({"all_done": true, "project": "Kade portal"})).await;
    assert_eq!(a, json!({"ok": true, "done": "archived", "archived": 3, "tasks": ["KADE-1", "KADE-2", "KADE-5"], "skipped": []}));
    assert_eq!(t.heard(), ["tasks"]);
    assert_eq!(t.column("Done"), ["BLOG-1"], "another project's card stays");
    assert!(!t.archived("KADE-3") && !t.archived("KADE-4"), "cards in other columns stay");
    // by its key too, with nothing left
    let none = t.ok("archive_tasks", json!({"all_done": true, "project": "KADE"})).await;
    assert_eq!(none, json!({"ok": true, "done": "archived", "archived": 0, "tasks": [], "skipped": []}));
    assert!(t.heard().is_empty(), "nothing archived, nothing written, nothing sent");
    // without: every card in Done
    let b = t.ok("archive_tasks", json!({"all_done": true})).await;
    assert_eq!(b, json!({"ok": true, "done": "archived", "archived": 1, "tasks": ["BLOG-1"], "skipped": []}));
    assert!(t.column("Done").is_empty());
    assert_eq!(t.column("To do"), ["KADE-3"]);
    assert_eq!(t.column("Deploy"), ["KADE-4"]);
    assert_eq!(t.bin(), ["BLOG-1", "KADE-1", "KADE-2", "KADE-5"]);
}

#[tokio::test]
async fn with_two_teams_all_done_needs_the_team_or_a_project_and_leaves_the_other_teams_done_alone() {
    let t = setup();
    cards(&t);
    let mine = t.team_name();
    let research = team::add_team(&t.st.db, &t.st.you_id, "Research").unwrap();
    let res = t.project("Lab", "LAB", Some(&research));
    tasks::create(&t.st.db, &t.st.you_id, TaskInput { project_id: res, title: "Study".into(),
        state_id: Some(t.col(&research, "Done")), ..Default::default() }).unwrap();
    t.heard();
    let e = t.call("archive_tasks", json!({"all_done": true})).await.unwrap_err();
    assert!(e.starts_with("Name the team: "), "{e}");
    assert!(e.contains(&mine) && e.contains("Research"), "{e}");
    // a project on one board, with the other team named
    let e = t.call("archive_tasks", json!({"all_done": true, "project": "LAB", "team": mine})).await.unwrap_err();
    assert!(e.contains("Lab (LAB) is on Research's board") && e.contains("Nothing was archived"), "{e}");
    assert!(t.bin().is_empty() && t.heard().is_empty(), "refused: nothing changes");

    // the team named: only its Done column
    let a = t.ok("archive_tasks", json!({"all_done": true, "team": "Research"})).await;
    assert_eq!(a, json!({"ok": true, "done": "archived", "archived": 1, "tasks": ["LAB-1"], "skipped": []}));
    assert_eq!(t.column("Done"), ["KADE-1", "KADE-2", "BLOG-1"], "the other team's Done stays");
    // a project: its team's Done, only that project's cards; the same team named is fine
    let b = t.ok("archive_tasks", json!({"all_done": true, "project": "BLOG", "team": mine})).await;
    assert_eq!(b["tasks"], json!(["BLOG-1"]));
    let c = t.ok("archive_tasks", json!({"all_done": true, "team": mine})).await;
    assert_eq!(c["tasks"], json!(["KADE-1", "KADE-2"]));
    assert_eq!(t.heard(), ["tasks", "tasks", "tasks"], "one rows-changed per call");
}

// ---- neither or both ----

#[tokio::test]
async fn neither_or_both_of_tasks_and_all_done_is_refused_with_why_and_nothing_is_archived() {
    let t = setup();
    cards(&t);
    t.heard();
    for args in [json!({}), json!({"tasks": []}), json!({"all_done": false}), json!({"project": "KADE"}), json!({"tasks": [], "all_done": false})] {
        let e = t.call("archive_tasks", args.clone()).await.unwrap_err();
        assert!(e.starts_with("Say which cards: give tasks") && e.ends_with("Nothing was archived."), "{args}: {e}");
    }
    for args in [json!({"tasks": ["KADE-1"], "all_done": true}), json!({"tasks": ["KADE-1", "BLOG-1"], "all_done": true, "project": "KADE"})] {
        let e = t.call("archive_tasks", args.clone()).await.unwrap_err();
        assert!(e.starts_with("Give tasks or all_done, not both") && e.ends_with("Nothing was archived."), "{args}: {e}");
    }
    assert!(t.bin().is_empty(), "nothing is archived");
    assert_eq!(t.column("Done"), ["KADE-1", "KADE-2", "BLOG-1"]);
    assert!(t.heard().is_empty(), "and nothing is sent");
    // refused at once in a chat that used a tool from outside Hersir too: it never waits for an Approve
    let thread = t.marked("Read Otus and clean up Done");
    let e = t.call_in(&thread, "archive_tasks", json!({"tasks": ["KADE-1"], "all_done": true})).await.unwrap_err();
    assert!(e.starts_with("Give tasks or all_done, not both"), "{e}");
    let e = t.call_in(&thread, "archive_tasks", json!({})).await.unwrap_err();
    assert!(e.starts_with("Say which cards"), "{e}");
    assert!(pending::list(&t.st, &thread).unwrap().is_empty(), "nothing waits");
}

// ---- the answer ----

#[tokio::test]
async fn the_answer_names_at_most_50_cards_with_more_for_the_rest_and_every_skipped_card() {
    let t = setup();
    let names: Vec<String> = (1..=55).map(|i| t.card("KADE", "Done", &format!("Card {i}"))).collect();
    let review: Vec<String> = (1..=3).map(|i| t.card("BLOG", "Review", &format!("Draft {i}"))).collect();
    t.heard();
    let mut asked = names.clone();
    asked.extend(review.iter().cloned());
    let a = t.ok("archive_tasks", json!({"tasks": asked})).await;
    assert_eq!(a["archived"], 55);
    assert_eq!(a["tasks"], json!(names[..50]), "the first 50, in the order given");
    assert_eq!(a["more"], 5);
    let skipped: Vec<String> = a["skipped"].as_array().unwrap().iter().map(|s| s["task"].as_str().unwrap().to_string()).collect();
    assert_eq!(skipped, review, "every skipped card");
    assert!(a["skipped"].as_array().unwrap().iter().all(|s| s["reason"].as_str().unwrap().contains("is in Review")), "{a}");
    assert_eq!(t.heard(), ["tasks"], "one rows-changed for 55 cards");
    assert_eq!(t.bin().len(), 55);

    // exactly 50: no more
    let t = setup();
    for i in 1..=50 {
        t.card("KADE", "Done", &format!("Card {i}"));
    }
    let b = t.ok("archive_tasks", json!({"all_done": true})).await;
    assert_eq!((b["archived"].as_u64(), b["tasks"].as_array().unwrap().len()), (Some(50), 50));
    assert!(b.get("more").is_none(), "{b}");
    // and restore_tasks names them the same way
    let all: Vec<String> = (1..=50).map(|i| format!("KADE-{i}")).collect();
    let mut asked = all.clone();
    asked.push("KADE-51".into());
    let r = t.ok("restore_tasks", json!({"tasks": asked})).await;
    assert_eq!((r["done"].as_str(), r["restored"].as_u64(), r["tasks"].as_array().unwrap().len()), (Some("restored"), Some(50), 50));
    assert!(r.get("more").is_none(), "{r}");
}

// ---- restore_tasks ----

#[tokio::test]
async fn restore_tasks_puts_archived_cards_back_at_the_bottom_of_done_in_the_order_given_and_skips_one_that_isnt_archived() {
    let t = setup();
    cards(&t);
    t.ok("archive_tasks", json!({"tasks": ["KADE-1", "KADE-2"]})).await;
    let later = t.card("KADE", "Done", "Done while they were archived");
    t.heard();
    let r = t.ok("restore_tasks", json!({"tasks": ["KADE-2", "kade-1", "BLOG-1", "KADE-3", "NOPE-9"]})).await;
    assert_eq!(r, json!({"ok": true, "done": "restored", "restored": 2, "tasks": ["KADE-2", "KADE-1"], "skipped": [
        {"task": "BLOG-1", "reason": "BLOG-1 isn't archived"},
        {"task": "KADE-3", "reason": "KADE-3 isn't archived"},
        {"task": "NOPE-9", "reason": "No task NOPE-9: check the identifier with list_tasks"},
    ]}));
    assert_eq!(t.heard(), ["tasks"], "one rows-changed");
    assert_eq!(t.column("Done"), ["BLOG-1".to_string(), later, "KADE-2".into(), "KADE-1".into()], "at the bottom of Done, in the order given");
    assert!(t.bin().is_empty());
    let got = t.ok("get_task", json!({"task": "KADE-1"})).await;
    assert_eq!(got["task"]["archived"], false);
    let last = tasks::activity(&t.st.db, &t.id("KADE-1")).unwrap().pop().unwrap();
    assert_eq!((last.op.as_str(), last.actor_name.as_deref(), &last.diff), ("update", Some(t.lead_name().as_str()), &json!({"archived": false})),
               "the activity says the Chief restored it");
    // the Chief's other tools work on it again
    t.ok("move_task", json!({"task": "KADE-1", "column": "Backlog"})).await;
    assert_eq!(t.task("KADE-1").state_name, "Backlog");
    t.heard();
    // no cards: refused with why; nothing named that is archived: nothing written
    for args in [json!({}), json!({"tasks": []})] {
        let e = t.call("restore_tasks", args).await.unwrap_err();
        assert!(e.starts_with("Say which cards") && e.ends_with("Nothing was restored."), "{e}");
    }
    let n = t.ok("restore_tasks", json!({"tasks": ["KADE-9"]})).await;
    assert_eq!((n["restored"].as_u64(), n["tasks"].clone()), (Some(0), json!([])));
    assert!(t.heard().is_empty(), "nothing for the refused and empty restores");
}

// ---- a board check ----

#[tokio::test]
async fn in_a_board_check_both_tools_are_refused_with_their_message_and_nothing_changes() {
    let t = setup();
    cards(&t);
    t.ok("archive_tasks", json!({"tasks": ["KADE-2"]})).await;
    t.heard();
    let run = core_board::create_run(&t.st.db, &t.lead, "S", "/tmp", "/tmp/c.jsonl", &[]).unwrap();
    for args in [json!({"all_done": true}), json!({"tasks": ["KADE-1"]}), json!({})] {
        let e = tools::call_check(&t.st, &t.lead, &run, "archive_tasks", args).await.unwrap_err();
        assert_eq!(e, "A board check never archives cards: ask the user in a chat (start_chat) instead.");
    }
    let e = tools::call_check(&t.st, &t.lead, &run, "restore_tasks", json!({"tasks": ["KADE-2"]})).await.unwrap_err();
    assert_eq!(e, "A board check never restores cards: ask the user in a chat (start_chat) instead.");
    assert_eq!(t.bin(), ["KADE-2"], "nothing changes");
    assert_eq!(t.column("Done"), ["KADE-1", "BLOG-1"]);
    assert!(t.heard().is_empty());
}

// ---- after a tool from outside Hersir ----

/// The answer to a call kept for the user's Approve after mcp__otus__search, word for word.
fn kept_text(name: &str) -> String {
    format!("{name} wasn't run: this chat used mcp__otus__search, a tool from outside Hersir. Hersir keeps the call for the user to approve \
             with one click. Tell them in a sentence what it does.")
}

#[tokio::test]
async fn after_an_outside_tool_archive_tasks_waits_under_the_answer_and_approve_archives_once_while_restore_runs_at_once() {
    let t = setup();
    cards(&t);
    let thread = t.marked("Read Otus and archive Done");
    t.heard();
    let args = json!({"all_done": true});
    let e = t.call_in(&thread, "archive_tasks", args.clone()).await.unwrap_err();
    assert_eq!(e, kept_text("archive_tasks"));
    assert!(t.bin().is_empty(), "nothing archived yet");
    let kept = pending::list(&t.st, &thread).unwrap();
    assert_eq!(kept.len(), 1);
    let v = serde_json::to_value(&kept[0]).unwrap();
    assert_eq!((v["tool"].as_str(), v["status"].as_str(), &v["args"]), (Some("archive_tasks"), Some("pending"), &args));
    assert_eq!(v["preview"]["line"], "Archive 3 cards in Done", "{v}");
    let fields: Vec<(String, String)> = v["preview"]["fields"].as_array().unwrap().iter()
        .map(|f| (f["label"].as_str().unwrap().to_string(), f["value"].as_str().unwrap().to_string())).collect();
    let get = |label: &str| fields.iter().find(|f| f.0 == label).unwrap_or_else(|| panic!("{label} in {fields:?}")).1.clone();
    assert_eq!((get("Column"), get("Cards")), ("Done".to_string(), "KADE-1, KADE-2 and BLOG-1".to_string()));
    assert!(v["preview"]["warning"].is_null(), "{v}");
    let id = kept[0].action.id.clone();

    // Approve: archived, once
    let a = pending::approve(&t.st, &id).await.unwrap();
    assert_eq!((a.status.as_str(), a.result_md.as_deref()), ("approved", Some("Approved: archive 3 cards in Done. Archived 3 cards.")));
    assert_eq!(t.bin(), ["BLOG-1", "KADE-1", "KADE-2"]);
    assert_eq!(t.heard().iter().filter(|x| **x == "tasks").count(), 1, "one rows-changed for the cards");
    assert_eq!(pending::approve(&t.st, &id).await.unwrap_err(), "It was approved already, so it doesn't run again.");
    assert_eq!(t.bin().len(), 3);

    // restore_tasks runs at once: it only undoes an archive
    let r = t.call_in(&thread, "restore_tasks", json!({"tasks": ["KADE-1"]})).await.unwrap();
    assert_eq!((r["restored"].as_u64(), &r["tasks"]), (Some(1), &json!(["KADE-1"])));
    assert!(!t.archived("KADE-1"));
    assert!(pending::list(&t.st, &thread).unwrap().is_empty(), "nothing waits");

    // one card: "Archive KADE-1"
    t.call_in(&thread, "archive_tasks", json!({"tasks": ["KADE-1"]})).await.unwrap_err();
    let kept = pending::list(&t.st, &thread).unwrap();
    let v = serde_json::to_value(&kept[0]).unwrap();
    assert_eq!(v["preview"]["line"], "Archive KADE-1", "{v}");
    assert!(!t.archived("KADE-1"));
    let a = pending::approve(&t.st, &kept[0].action.id).await.unwrap();
    assert_eq!(a.result_md.as_deref(), Some("Approved: archive KADE-1. KADE-1 is archived."));
    assert!(t.archived("KADE-1"));

    // a chat without such a tool: archive_tasks runs at once
    let plain = chat::create_thread(&t.st.db, &t.st.you_id, &t.lead, "Archive Done").unwrap();
    t.card("KADE", "Done", "Fresh");
    assert_eq!(t.call_in(&plain, "archive_tasks", json!({"all_done": true})).await.unwrap()["archived"], 1);
}

// ---- one write ----

#[tokio::test]
async fn one_call_is_one_write_a_failure_partway_archives_and_restores_nothing() {
    let t = setup();
    cards(&t);
    let third = t.card("KADE", "Done", "Third");
    // the second card's write fails as a disk would (not a refusal): the whole write is undone
    let fail_on = |id: &str| t.st.db.write(None, |w| {
        w.conn().execute_batch(&format!(
            "CREATE TRIGGER qa_bulk BEFORE UPDATE ON tasks WHEN NEW.id = '{id}' BEGIN SELECT RAISE(ABORT, 'qa: disk full'); END;"))?;
        Ok(())
    }).unwrap();
    let unfail = || t.st.db.write(None, |w| { w.conn().execute_batch("DROP TRIGGER qa_bulk;")?; Ok(()) }).unwrap();
    t.heard();
    fail_on(&t.id("KADE-2"));
    let e = t.call("archive_tasks", json!({"tasks": ["KADE-1", "KADE-2", third.as_str()]})).await.unwrap_err();
    assert!(e.contains("qa: disk full"), "{e}");
    assert!(t.bin().is_empty(), "KADE-1 isn't archived either");
    let e = t.call("archive_tasks", json!({"all_done": true})).await.unwrap_err();
    assert!(e.contains("qa: disk full"), "{e}");
    assert!(t.bin().is_empty());
    assert!(t.heard().is_empty(), "and nothing is sent");
    unfail();

    t.ok("archive_tasks", json!({"all_done": true})).await;
    fail_on(&t.id("KADE-2"));
    t.heard();
    let e = t.call("restore_tasks", json!({"tasks": ["KADE-1", "KADE-2", third.as_str()]})).await.unwrap_err();
    assert!(e.contains("qa: disk full"), "{e}");
    assert_eq!(t.bin().len(), 4, "no card was restored");
    assert!(t.heard().is_empty());
    unfail();
}

fn git_repo(dir: &Path) -> PathBuf {
    let repo = dir.join("repo");
    std::fs::create_dir(&repo).unwrap();
    for a in [&["init", "-q", "-b", "main"][..], &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "i"][..]] {
        assert!(std::process::Command::new("git").args(a).current_dir(&repo).status().unwrap().success());
    }
    repo
}
