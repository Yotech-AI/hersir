//! GA-344 QA: board_changes, Claude Desktop's own tool, sees every write, whoever makes it: a tool (the Chief's
//! comment_on_task), a Tauri command (what update_task runs: `tasks::update` as you; a test can't start the window, so
//! not the command itself), an agent's run (the fake Claude Code) and gizai-core itself (Hersir's pull request check,
//! `pulls::record`). Each write shows its card after the counter before it; with the latest counter no cards come back.
//! Only a Claude Desktop token is offered and knows the tool. The window still hears of every change through the one
//! notifier: the tools tell `AppState::notify`, and the Tauri commands' `changed` now does too, with
//! `RowsChangedInWindow`, which the window's notifier sends as the same `rows-changed`.
use std::sync::{Arc, Mutex};

use gizai_core::model::*;
use gizai_core::{projects, pulls, tasks, team};
use gizai_lib::runs::Note;
use gizai_lib::tools::{self, GizaiTools};
use gizai_lib::AppState;
use gizai_mcp::Tools as _;
use serde_json::json;

/// The tools as a token's scope gives them: Claude Desktop's (`client`), or another's.
fn tools_for(st: &AppState, client: Option<&str>, question: Option<&str>) -> GizaiTools {
    GizaiTools { st: st.clone(), actor: st.you_id.clone(), thread: None, check: None, run: None, question: question.map(String::from),
                 client: client.map(String::from) }
}

fn desktop(st: &AppState) -> GizaiTools {
    tools_for(st, Some(tools::CLAUDE_DESKTOP), None)
}

/// board_changes as Claude Desktop calls it: (counter, cards, board, reload).
async fn board_changes(st: &AppState, since: Option<i64>) -> (i64, Vec<String>, bool, bool) {
    let args = since.map_or(json!({}), |n| json!({"since": n}));
    let v = desktop(st).call(tools::BOARD_CHANGES, args).await.unwrap();
    let cards = v["tasks"].as_array().unwrap_or_else(|| panic!("{v}")).iter().map(|t| t.as_str().unwrap().to_string()).collect();
    (v["counter"].as_i64().unwrap(), cards, v["board"].as_bool().unwrap(), v["reload"].as_bool().unwrap())
}

async fn counter(st: &AppState) -> i64 {
    board_changes(st, None).await.0
}

/// What the notifier was told, as text.
type Heard = Arc<Mutex<Vec<String>>>;

/// Hersir for a test whose notifier writes down what it hears.
fn state(dir: &std::path::Path) -> (AppState, Heard) {
    let mut st = gizai_lib::test_state(dir);
    let heard: Heard = Arc::default();
    let h = heard.clone();
    st.notify = Arc::new(move |n: Note| {
        let s = match n {
            Note::RowsChanged(t) => format!("rows:{t}"),
            Note::RowsChangedInWindow(t) => format!("window:{t}"),
            Note::RunsChanged => "runs".into(),
            _ => "other".into(),
        };
        h.lock().unwrap().push(s);
    });
    (st, heard)
}

/// A card in Backlog with nobody on it, so no agent picks it up.
fn backlog_card(st: &AppState, project: &str, title: &str) -> String {
    let team_id = team::list(&st.db).unwrap()[0].id.clone();
    let backlog = team::get(&st.db, &team_id).unwrap().states.into_iter().find(|s| s.category == "backlog").unwrap().id;
    tasks::create(&st.db, &st.you_id, TaskInput { project_id: project.to_string(), title: title.into(), state_id: Some(backlog),
        ..Default::default() }).unwrap()
}

fn ident(st: &AppState, id: &str) -> String {
    tasks::get(&st.db, id).unwrap().identifier
}

fn lead(st: &AppState) -> String {
    let team_id = team::list(&st.db).unwrap()[0].id.clone();
    team::add_agent(&st.db, &st.you_id, &team_id, AgentInput { name: "Team Lead".into(), role_key: "lead".into(), chat_enabled: Some(true),
        ..Default::default() }).unwrap()
}

/// The three writes that need no agent's run, each on its own card: (what, card). After each, board_changes after the
/// counter before it names that card, and the counter moved on.
async fn tool_command_and_core_writes(st: &AppState, project: &str) -> Vec<(&'static str, String)> {
    let chief = lead(st);
    let (by_tool, by_command, by_core) = (backlog_card(st, project, "By a tool"), backlog_card(st, project, "By a Tauri command"),
                                          backlog_card(st, project, "By Hersir itself"));
    // a tool: the Chief comments on the card
    let before = counter(st).await;
    tools::call(st, &chief, "comment_on_task", json!({"task": ident(st, &by_tool), "body_md": "A comment from the Chief"})).await.unwrap();
    let (after, cards, _, reload) = board_changes(st, Some(before)).await;
    assert!(after > before && !reload);
    assert_eq!(cards, vec![ident(st, &by_tool)], "the tool's write");
    // a Tauri command: update_task runs tasks::update as you
    let before = after;
    tasks::update(&st.db, &st.you_id, &by_command, TaskPatch { title: Some("Renamed in the window".into()), ..Default::default() }).unwrap();
    let (after, cards, _, _) = board_changes(st, Some(before)).await;
    assert!(after > before);
    assert_eq!(cards, vec![ident(st, &by_command)], "the Tauri command's write");
    // gizai-core itself: the pull request check records the card's pull request (no actor: Hersir)
    let before = after;
    pulls::record(&st.db, None, &by_core, "https://github.com/acme/shop/pull/7", "open", true).unwrap();
    let (after, cards, _, _) = board_changes(st, Some(before)).await;
    assert!(after > before);
    assert_eq!(cards, vec![ident(st, &by_core)], "gizai-core's write");
    vec![("a tool", by_tool), ("a Tauri command", by_command), ("gizai-core", by_core)]
}

// Linux and macOS only: the run's Claude Code is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn one_write_of_each_kind_a_tool_a_tauri_command_a_run_and_gizai_core_shows_in_board_changes() {
    const FAKE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude.sh");
    let tmp = tempfile::tempdir().unwrap();
    let (st, _) = state(tmp.path());
    // any run that starts uses the fake Claude Code, never the real one
    gizai_core::settings::set(&st.db, "claude_bin", &FAKE.to_string()).unwrap();
    let repo = tmp.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    for a in [&["init", "-q", "-b", "main"][..], &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "i"][..]] {
        assert!(std::process::Command::new("git").args(a).current_dir(&repo).status().unwrap().success());
    }
    let by_run = gizai_lib::test_task(&st, repo.to_str().unwrap(), "backend");
    let project = projects::list(&st.db).unwrap().into_iter().find(|p| p.key == "KADE").unwrap().id;
    let (start, cards, _, _) = board_changes(&st, None).await;
    assert!(cards.is_empty(), "without since: only the counter");

    // a run: the Backend Agent's run on its card
    let s = gizai_lib::runs::run_once(&st, &by_run, None, Some(FAKE.into())).await.unwrap();
    assert_eq!(s.status, "succeeded", "{:?}", s.error);
    let (after_run, cards, _, _) = board_changes(&st, Some(start)).await;
    assert!(after_run > start);
    assert_eq!(cards, vec![ident(&st, &by_run)], "the run's writes");

    let others = tool_command_and_core_writes(&st, &project).await;

    // from the first counter: all four cards, the one changed last first
    let (now, cards, _, reload) = board_changes(&st, Some(start)).await;
    assert!(!reload);
    let mut want: Vec<String> = others.iter().rev().map(|(_, id)| ident(&st, id)).collect();
    want.push(ident(&st, &by_run));
    assert_eq!(cards, want, "each kind of write, newest first");
    // with the latest counter: no cards
    assert_eq!(board_changes(&st, Some(now)).await, (now, vec![], false, false));
}

#[tokio::test]
async fn a_tool_a_tauri_command_and_gizai_core_show_in_board_changes_and_the_latest_counter_gives_no_cards() {
    let tmp = tempfile::tempdir().unwrap();
    let (st, heard) = state(tmp.path());
    let project = projects::create(&st.db, &st.you_id, ProjectInput { name: "Kade portal".into(), key: "KADE".into(), ..Default::default() }).unwrap();
    let start = counter(&st).await;
    let writes = tool_command_and_core_writes(&st, &project).await;
    let (now, cards, _, _) = board_changes(&st, Some(start)).await;
    assert_eq!(cards, writes.iter().rev().map(|(_, id)| ident(&st, id)).collect::<Vec<_>>());
    assert_eq!(board_changes(&st, Some(now)).await, (now, vec![], false, false), "the latest counter: no cards");
    // asking again changes nothing: board_changes only reads
    assert_eq!(board_changes(&st, Some(now)).await.0, now);
    assert_eq!(counter(&st).await, now);
    // the tool told the window through the one notifier, as before
    assert!(heard.lock().unwrap().iter().any(|h| h == "rows:comments" || h == "rows:tasks"), "{:?}", heard.lock().unwrap());
}

#[tokio::test]
async fn board_changes_says_board_for_a_new_label_and_reload_for_a_counter_from_before_a_restore() {
    let tmp = tempfile::tempdir().unwrap();
    let (st, _) = state(tmp.path());
    let now = counter(&st).await;
    gizai_core::labels::save(&st.db, &st.you_id, None, "urgent", None).unwrap();
    let (after, cards, board, reload) = board_changes(&st, Some(now)).await;
    assert_eq!((cards.is_empty(), board, reload), (true, true, false));
    assert_eq!(board_changes(&st, Some(after + 50)).await, (after, vec![], false, true), "since past the counter");
    // since as text, as some clients send numbers; not a number is refused
    let v = desktop(&st).call(tools::BOARD_CHANGES, json!({"since": after.to_string()})).await.unwrap();
    assert_eq!(v["counter"], after);
    let err = desktop(&st).call(tools::BOARD_CHANGES, json!({"since": "yesterday"})).await.unwrap_err();
    assert_eq!(err, "\"since\" must be a whole number");
    let err = desktop(&st).call(tools::BOARD_CHANGES, json!(["since"])).await.unwrap_err();
    assert_eq!(err, "arguments must be an object");
}

#[tokio::test]
async fn only_claude_desktop_is_offered_board_changes_and_any_other_token_is_told_it_is_an_unknown_tool() {
    let tmp = tempfile::tempdir().unwrap();
    let (st, _) = state(tmp.path());
    let def = desktop(&st).list().into_iter().find(|d| d.name == tools::BOARD_CHANGES).expect("Claude Desktop is offered board_changes");
    assert!(def.read_only, "it only reads");
    assert!(!tools::catalog().iter().any(|d| d.name == tools::BOARD_CHANGES), "not in the catalog every other token gets");
    for (who, t) in [("no client", tools_for(&st, None, None)), ("another client", tools_for(&st, Some("something-else"), None)),
                     ("a question", tools_for(&st, None, Some("R1")))] {
        assert!(!t.list().iter().any(|d| d.name == tools::BOARD_CHANGES), "{who}: not offered");
        assert_eq!(t.call(tools::BOARD_CHANGES, json!({})).await.unwrap_err(), "unknown tool board_changes", "{who}");
    }
    // the Chief's own tool calls don't know it either
    let chief = lead(&st);
    assert!(tools::call(&st, &chief, tools::BOARD_CHANGES, json!({})).await.is_err());
}

/// What a Tauri command changes goes to the window through `AppState::notify` (`commands::changed`), as the tools'
/// changes do, and the window's notifier sends it as `rows-changed` with its table, as before, without looking at the
/// Inbox again for your own change. A test can't start the window, so this reads the source, as
/// commands_off_main_thread_test does.
#[test]
fn the_tauri_commands_tell_the_window_through_the_one_notifier_and_it_still_sends_rows_changed() {
    let src = |f: &str| std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(f)).unwrap();
    let commands = src("commands.rs");
    let at = commands.find("fn changed(").expect("commands::changed");
    let body = &commands[at..at + commands[at..].find("\n}").unwrap()];
    assert!(body.contains("(st.notify)(Note::RowsChangedInWindow(table))"), "{body}");
    assert!(!commands.contains(".emit("), "no command emits to the window itself any more");
    // every command that told the window before still does (71 on main before GA-344)
    assert!(commands.matches("changed(&app, ").count() >= 71, "{}", commands.matches("changed(&app, ").count());
    let lib = src("lib.rs");
    let at = lib.find("fn ui_notifier(").expect("ui_notifier");
    let notifier = &lib[at..at + lib[at..].find("\n}").unwrap()];
    let arm = notifier.find("runs::Note::RowsChangedInWindow(table)").expect("the window's notifier handles RowsChangedInWindow");
    let rest = &notifier[arm..];
    assert!(rest[..rest.find("runs::Note::RunsChanged").unwrap()].contains(r#"app.emit("rows-changed", serde_json::json!({ "table": table }))"#),
            "{notifier}");
    let poke = &notifier[..notifier.find("notifications::poke").unwrap()];
    assert!(!poke.contains("RowsChangedInWindow"), "your own change in the window doesn't look at the Inbox again, as before");
    // and the window listens for rows-changed with its table, as before
    let api = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/api.ts")).unwrap();
    assert!(api.contains(r#"listen<{ table: string }>("rows-changed""#), "src/api.ts listens for rows-changed with its table");
}
