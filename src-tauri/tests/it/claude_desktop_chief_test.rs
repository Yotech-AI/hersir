// Linux and macOS only: over the Unix socket, and start_agent_run runs a shell script as a fake Claude Code.
#![cfg(unix)]
//! GA-341 QA: Claude Desktop works like the Chief. Over the socket, with the token Connect makes
//! (`claude_desktop::connect`, the scope `{"client":"claude-desktop"}`, actor: you), as the hersir-mcp shim talks:
//! - tools/list is get_chief_brief (GA-342: then ask_chief and chief_reply), then every tool of the Chief's chat turns
//!   (`tools::catalog`) but start_chat and update_checkout (GA-345: board_changes is the view's now); a chat's, a board check's and a
//!   question's tokens don't get get_chief_brief.
//! - start_chat and update_checkout answer isError with the card's texts and change nothing.
//! - The tools that change the board run at once, as you: no pending action, and the activity and comments are yours.
//! - The Chief's rules hold: GA-101's hold (ONLY_THE_USER, and move_task keeps it), GA-323's new commands (not run, no
//!   chat to keep them in), attach_file (only the Chief's copies of the code) and merge_pull_request's rules; nothing
//!   kept for your Approve is approved from here.
//! - get_chief_brief: the card's line with your name, a blank line, then the Chief's instructions as they are.
//! Never the real Claude Code or Claude Desktop: a fake script, and Claude's folder is the test's own.
use std::path::{Path, PathBuf};
use std::time::Duration;

use gizai_core::model::*;
use gizai_core::{chat, projects, settings, tasks, team, tokens};
use gizai_lib::claude_desktop as desktop;
use gizai_lib::{AppState, tools};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

const FAKE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude.sh");
const WAIT: Duration = Duration::from_secs(10);

// The card's texts, word for word.
const START_CHAT: &str = "start_chat isn't available in Claude Desktop: ask the user here, in this chat.";
const UPDATE_CHECKOUT: &str = "update_checkout isn't available in Claude Desktop: it works only in Hersir's chat, where its result comes as a message.";
const BRIEF_DESCRIPTION: &str = "What Hersir's Chief works by: its instructions. Read it before you change the board, and follow it as the Chief does.";
const ONLY_THE_USER: &str = "Only the user takes this hold off, on the card: it waits for their OK after a tool from outside Hersir.";

fn brief_line(you: &str) -> String {
    format!("You are working as Hersir's Chief for {you}, from Claude Desktop. Hersir's tools act at once, like the Chief's do in Hersir's \
             chat. Leave out what only fits a run on a card, like the GIZAI_RESULT line. Look things up in Hersir's memory (memory_search) \
             before you ask the user.")
}

/// attach_file's refusal from Claude Desktop for a file outside the Chief's copies (the hand-over's text).
fn not_in_copies(raw: &str) -> String {
    format!("From Claude Desktop, attach_file takes only a file inside the Chief's copies of the projects' code; {raw} isn't one. Nothing \
             was attached: the user can attach it themselves in Hersir.")
}

/// The Chief's instructions in its agent form: Markdown with a heading, a list, a trailing double space and a last newline,
/// to see they come back as they are.
const INSTRUCTIONS: &str = "# How I work\n\nAsk Jefsev before you merge.  \n\n- Keep cards small.\n- Write in plain English.\n";

/// One open connection, as the shim makes it: the hello line with the token, then one JSON-RPC message per line.
struct Conn {
    lines: Lines<BufReader<OwnedReadHalf>>,
    w: OwnedWriteHalf,
    id: i64,
}

impl Conn {
    async fn open(st: &AppState, token: &str) -> Conn {
        let (r, mut w) = UnixStream::connect(&st.mcp_socket).await.unwrap().into_split();
        w.write_all(gizai_mcp::hello_line(token).as_bytes()).await.unwrap();
        let mut c = Conn { lines: BufReader::new(r).lines(), w, id: 1 };
        c.ask(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}})).await;
        c.w.write_all(format!("{}\n", json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).as_bytes()).await.unwrap();
        c
    }
    async fn ask(&mut self, msg: Value) -> Value {
        self.w.write_all(format!("{msg}\n").as_bytes()).await.unwrap();
        match tokio::time::timeout(WAIT, self.lines.next_line()).await {
            Ok(Ok(Some(l))) => serde_json::from_str(&l).unwrap(),
            Ok(_) => panic!("Hersir closed the connection"),
            Err(_) => panic!("Hersir didn't answer in time"),
        }
    }
    async fn list(&mut self) -> Vec<Value> {
        self.id += 1;
        let r = self.ask(json!({"jsonrpc": "2.0", "id": self.id, "method": "tools/list"})).await;
        r["result"]["tools"].as_array().unwrap_or_else(|| panic!("{r}")).clone()
    }
    /// tools/call: the result (content and isError).
    async fn call(&mut self, name: &str, args: Value) -> Value {
        self.id += 1;
        let id = self.id;
        let r = self.ask(json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": args}})).await;
        assert_eq!(r["id"], id, "{r}");
        r["result"].clone()
    }
    /// A call that must work: its answer, parsed when it is JSON.
    async fn ok(&mut self, name: &str, args: Value) -> Value {
        let r = self.call(name, args.clone()).await;
        assert_eq!(r["isError"], false, "{name} {args}: {r}");
        serde_json::from_str(&text(&r)).unwrap_or(Value::String(text(&r)))
    }
    /// A call that must be refused: its text.
    async fn refused(&mut self, name: &str, args: Value) -> String {
        let r = self.call(name, args.clone()).await;
        assert_eq!(r["isError"], true, "{name} {args}: {r}");
        text(&r)
    }
}

fn text(result: &Value) -> String {
    result["content"][0]["text"].as_str().unwrap_or_default().to_string()
}

fn names(tools: &[Value]) -> Vec<String> {
    tools.iter().map(|d| d["name"].as_str().unwrap().to_string()).collect()
}

/// Every row of every table, as text: to see that a call changed nothing.
fn dump(st: &AppState) -> Vec<String> {
    st.db.read(|c| {
        let tables: Vec<String> = c.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?
            .query_map([], |r| r.get::<_, String>(0))?.collect::<Result<_, _>>()?;
        let mut out = vec![];
        for table in tables {
            let mut q = c.prepare(&format!("SELECT * FROM \"{table}\""))?;
            let n = q.column_count();
            let mut rows = q.query([])?;
            let mut lines = vec![];
            while let Some(r) = rows.next()? {
                lines.push(format!("{table}: {:?}", (0..n).map(|i| format!("{:?}", r.get_ref(i))).collect::<Vec<_>>()));
            }
            lines.sort();
            out.extend(lines);
        }
        Ok(out)
    }).unwrap()
}

fn count(st: &AppState, sql: &str) -> i64 {
    st.db.read(|c| Ok(c.query_row(sql, [], |r| r.get(0))?)).unwrap()
}

fn last_seq(st: &AppState) -> i64 {
    count(st, "SELECT coalesce(max(seq), 0) FROM changes")
}

/// The actors of the activity lines (`changes`) after `seq`, by table, without repeats.
fn actors_since(st: &AppState, seq: i64) -> Vec<(String, Option<String>)> {
    st.db.read(|c| {
        let mut q = c.prepare("SELECT DISTINCT table_name, actor_id FROM changes WHERE seq > ?1 ORDER BY table_name")?;
        let rows = q.query_map([seq], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }).unwrap()
}

fn git(dir: &Path, args: &[&str]) {
    let o = std::process::Command::new("git").args(["-c", "user.email=t@t", "-c", "user.name=t"]).args(args).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
}

struct S {
    st: AppState,
    tmp: tempfile::TempDir,
    lead: String,
    backend: String,
    repo: PathBuf,
    token: String,
    _server: tokio::task::JoinHandle<()>,
}

/// Hersir with its socket served and Claude Desktop connected: the Chief (Team Lead with Chat on and its own instructions),
/// a Backend Agent, project KADE linked to a git repository (docs/spec.md tracked, .env not) with KADE-1 in Backlog, made
/// by you. Claude Code is the fake script.
fn served() -> S {
    let tmp = tempfile::tempdir().unwrap();
    let mut st = gizai_lib::test_state(tmp.path());
    let bin = tmp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let shim = bin.join("hersir-mcp");
    std::fs::write(&shim, "stand-in").unwrap();
    st.mcp_shim = Some(shim);
    settings::set(&st.db, "claude_bin", &FAKE.to_string()).unwrap();
    let team_id = team::list(&st.db).unwrap()[0].id.clone();
    let lead = team::add_agent(&st.db, &st.you_id, &team_id, AgentInput { name: "Team Lead".into(), role_key: "lead".into(),
        chat_enabled: Some(true), instructions_md: Some(INSTRUCTIONS.into()), ..Default::default() }).unwrap();
    let backend = team::add_agent(&st.db, &st.you_id, &team_id, AgentInput { name: "Backend Agent".into(), role_key: "backend".into(),
        ..Default::default() }).unwrap();
    let repo = tmp.path().join("kade");
    std::fs::create_dir_all(repo.join("docs")).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join(".gitignore"), ".env\n").unwrap();
    std::fs::write(repo.join("docs/spec.md"), "# Spec").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "init"]);
    std::fs::write(repo.join(".env"), "APP_KEY=secret\n").unwrap();
    let project = projects::create(&st.db, &st.you_id, ProjectInput { name: "Kade portal".into(), key: "KADE".into(),
        repo_path: Some(repo.display().to_string()), ..Default::default() }).unwrap();
    tasks::create(&st.db, &st.you_id, TaskInput { project_id: project, title: "Export invoices as CSV".into(), ..Default::default() }).unwrap();
    let server = gizai_lib::mcp::start(&st).unwrap();
    let c = desktop::connect(&st, false).unwrap();
    let entry: Value = serde_json::from_str(&c.entry).unwrap();
    let token = entry["mcpServers"]["hersir"]["env"]["HERSIR_TOKEN"].as_str().unwrap().to_string();
    S { st, tmp, lead, backend, repo, token, _server: server }
}

impl S {
    async fn conn(&self) -> Conn {
        Conn::open(&self.st, &self.token).await
    }
    fn card(&self, identifier: &str) -> Task {
        let id: String = self.st.db.read(|c| Ok(c.query_row("SELECT id FROM tasks WHERE identifier=?1", [identifier], |r| r.get(0))?)).unwrap();
        tasks::get(&self.st.db, &id).unwrap()
    }
    fn you_name(&self) -> String {
        gizai_core::users::list(&self.st.db).unwrap().into_iter().find(|p| p.id == self.st.you_id).unwrap().name
    }
    fn pending(&self) -> i64 {
        count(&self.st, "SELECT count(*) FROM pending_actions")
    }
    fn column(&self, name: &str) -> String {
        let team_id = team::list(&self.st.db).unwrap()[0].id.clone();
        team::get(&self.st.db, &team_id).unwrap().states.into_iter().find(|s| s.name == name).unwrap().id
    }
    /// GA-101's hold on `identifier`, as the Chief's tools put it after a tool from outside Hersir.
    fn hold_for_your_ok(&self, identifier: &str) {
        tasks::update(&self.st.db, &self.lead, &self.card(identifier).id, TaskPatch { hold: Some("needs_decision".into()),
            hold_reason: Some("Waiting for your OK: the Chief put this card here after using mcp__otus__search, a tool from outside Hersir. \
                               Check it, then take it off hold.".into()),
            hold_kind: Some(HOLD_WAITING_FOR_OK.into()), ..Default::default() }).unwrap();
        assert!(self.card(identifier).waits_for_your_ok());
    }
    /// Waits until no run of the card is queued or running (the fake ends in a second or two).
    async fn runs_ended(&self, identifier: &str) {
        let id = self.card(identifier).id;
        for _ in 0..200 {
            let live = gizai_core::runs::list_for_task(&self.st.db, &id).unwrap().iter()
                .any(|r| matches!(r.status.as_str(), "queued" | "running" | "waiting_approval"));
            if !live && gizai_lib::runs::live(&self.st).iter().all(|r| r.task_id != id) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("{identifier}'s run didn't end");
    }
}

// ---- tools/list ----

#[tokio::test]
async fn the_desktop_token_lists_get_chief_brief_then_every_tool_of_the_chiefs_chat_turns_but_start_chat_and_update_checkout() {
    let s = served();
    let mut c = s.conn().await;
    let listed = c.list().await;
    let catalog = tools::catalog();
    // GA-342: ask_chief and chief_reply come after get_chief_brief (claude_desktop_ask_chief_test.rs checks them)
    let mut want = vec!["get_chief_brief".to_string(), "ask_chief".into(), "chief_reply".into()];
    want.extend(catalog.iter().map(|d| d.name.clone()).filter(|n| n != "start_chat" && n != "update_checkout"));
    // GA-345: board_changes (GA-344) is no longer here: it is the view's, offered only with MCP Apps
    // (claude_desktop_board_view_test.rs)
    assert_eq!(names(&listed), want, "get_chief_brief first, then ask_chief and chief_reply, then the Chief's tools in their order");
    assert_eq!(listed.len(), catalog.len() + 1);
    // get_chief_brief: read-only, the card's description, no arguments
    let brief = &listed[0];
    assert_eq!(brief["annotations"]["readOnlyHint"], true, "{brief}");
    assert_eq!(brief["description"], BRIEF_DESCRIPTION);
    assert_eq!(brief["inputSchema"]["type"], "object");
    assert!(brief["inputSchema"]["properties"].as_object().unwrap().is_empty(), "{brief}");
    // the others exactly as the Chief gets them: description, schema and readOnlyHint
    for d in &listed[3..] {
        let chief = catalog.iter().find(|t| t.name == d["name"].as_str().unwrap()).unwrap();
        assert_eq!(d["description"], chief.description.as_str(), "{}", chief.name);
        assert_eq!(d["inputSchema"], chief.input_schema, "{}", chief.name);
        assert_eq!(d["annotations"]["readOnlyHint"], chief.read_only, "{}", chief.name);
    }
    // nothing that approves a call kept for your Approve
    assert!(names(&listed).iter().all(|n| !n.contains("approve") && !n.contains("pending")), "{:?}", names(&listed));
    assert_eq!(tools::desktop_catalog().iter().map(|d| d.name.clone()).collect::<Vec<_>>(), want);
}

#[tokio::test]
async fn a_chats_a_board_checks_and_a_questions_tokens_are_offered_what_they_were_without_get_chief_brief() {
    let s = served();
    let thread = chat::create_thread(&s.st.db, &s.st.you_id, &s.lead, "What's on the board?").unwrap();
    let catalog: Vec<String> = tools::catalog().into_iter().map(|d| d.name).collect();
    let reads: Vec<String> = tools::catalog().into_iter().filter(|d| d.read_only).map(|d| d.name).collect();
    for (scope, want) in [(json!({"chat": thread}), &catalog), (json!({"check": "R1"}), &catalog), (json!({"question": "R1"}), &reads)] {
        let token = tokens::mint(&s.st.db, &s.lead, scope.clone(), 60_000).unwrap();
        let mut c = Conn::open(&s.st, &token).await;
        let listed = names(&c.list().await);
        assert_eq!(&listed, want, "{scope}");
        assert!(!listed.contains(&"get_chief_brief".to_string()), "{scope}");
        // called anyway, it isn't the Chief's tool, and nothing changes
        let before = dump(&s.st);
        let e = c.refused("get_chief_brief", json!({})).await;
        assert!(!e.contains("You are working as Hersir's Chief") && !e.contains("Keep cards small"), "{scope}: {e}");
        assert_eq!(dump(&s.st), before, "{scope}");
    }
    // the Chief's chat turns still have start_chat and update_checkout
    assert!(catalog.contains(&"start_chat".to_string()) && catalog.contains(&"update_checkout".to_string()));
}

// ---- start_chat and update_checkout ----

#[tokio::test]
async fn start_chat_and_update_checkout_answer_is_error_with_the_cards_texts_and_change_nothing() {
    let s = served();
    let mut c = s.conn().await;
    c.ok("get_overview", json!({})).await;
    let before = dump(&s.st);
    for args in [json!({"message": "Shall I start KADE-1?", "title": "KADE-1", "tasks": ["KADE-1"]}), json!({})] {
        assert_eq!(c.refused("start_chat", args.clone()).await, START_CHAT, "{args}");
    }
    for args in [json!({"project": "KADE"}), json!({})] {
        assert_eq!(c.refused("update_checkout", args.clone()).await, UPDATE_CHECKOUT, "{args}");
    }
    assert_eq!(dump(&s.st), before, "no chat, no message, no activity line");
    assert_eq!(count(&s.st, "SELECT count(*) FROM chat_threads"), 0);
}

// ---- the tools act at once, as you ----

#[tokio::test]
async fn create_update_move_comment_memory_write_and_archive_run_at_once_as_you_with_no_pending_action() {
    let s = served();
    let you = s.st.you_id.clone();
    let mut c = s.conn().await;
    let seq = last_seq(&s.st);

    let made = c.ok("create_task", json!({"project": "KADE", "title": "Made from Claude Desktop", "acceptance_md": "- [ ] It works"})).await;
    assert_eq!(made["task"]["identifier"], "KADE-2", "{made}");
    let card = s.card("KADE-2");
    assert_eq!((card.title.as_str(), card.hold.as_deref()), ("Made from Claude Desktop", None), "made at once, not on hold");
    let (owner, created_by, creator): (Option<String>, Option<String>, Option<String>) = s.st.db.read(|db| Ok(db.query_row(
        "SELECT owner_person_id, created_by, creator_actor_id FROM tasks WHERE identifier='KADE-2'", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?))
        .unwrap();
    assert_eq!((owner.as_deref(), created_by.as_deref(), creator.as_deref()), (Some(you.as_str()), Some(you.as_str()), Some(you.as_str())),
               "the card is yours: you review it in Review");

    c.ok("update_task", json!({"task": "KADE-2", "description_md": "From Claude Desktop.", "priority": 2})).await;
    let card = s.card("KADE-2");
    assert_eq!((card.description_md.as_str(), card.priority), ("From Claude Desktop.", 2));

    c.ok("move_task", json!({"task": "KADE-2", "column": "Done"})).await;
    assert_eq!(s.card("KADE-2").state_name, "Done");

    c.ok("comment_on_task", json!({"task": "KADE-2", "body_md": "A comment from Claude Desktop"})).await;
    let author: String = s.st.db.read(|db| Ok(db.query_row(
        "SELECT author_actor_id FROM comments WHERE body_md='A comment from Claude Desktop'", [], |r| r.get(0))?)).unwrap();
    assert_eq!(author, you, "the comment is yours");

    c.ok("memory_write", json!({"path": "Decisions/From Claude Desktop", "body_md": "Written from Claude Desktop."})).await;
    let note = c.ok("memory_read", json!({"note": "Decisions/From Claude Desktop"})).await;
    assert_eq!(note["note"]["body_md"], "Written from Claude Desktop.", "{note}");

    let archived = c.ok("archive_tasks", json!({"tasks": ["KADE-2"]})).await;
    assert!(s.card("KADE-2").archived_at.is_some(), "{archived}");

    assert_eq!(s.pending(), 0, "nothing waits for an Approve");
    let actors = actors_since(&s.st, seq);
    assert!(!actors.is_empty());
    assert!(actors.iter().all(|(_, a)| a.as_deref() == Some(you.as_str())), "every activity line is yours: {actors:?}");
    for table in ["tasks", "comments", "docs"] {
        assert!(actors.iter().any(|(t, _)| t == table), "{table} in {actors:?}");
    }
}

#[tokio::test]
async fn start_agent_run_from_claude_desktop_starts_the_run_at_once_with_no_pending_action() {
    let s = served();
    let mut c = s.conn().await;
    let seq = last_seq(&s.st);
    let r = c.ok("start_agent_run", json!({"task": "KADE-1", "agent": "Backend Agent"})).await;
    assert_eq!(r["done"], "started", "{r}");
    assert_eq!(r["run"]["agent"], "Backend Agent", "{r}");
    assert_eq!(s.pending(), 0);
    s.runs_ended("KADE-1").await;
    let runs = gizai_core::runs::list_for_task(&s.st.db, &s.card("KADE-1").id).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].agent_id, s.backend);
    // the run's own lines (the run, the card it claims and moves on, its comment) are the agent's or Hersir's, exactly as
    // when the Chief starts a run: start_agent_run records no line of who started it, so none is yours or the Chief's
    let desktop = actors_since(&s.st, seq);
    assert!(desktop.iter().all(|(_, a)| a.is_none() || a.as_deref() == Some(s.backend.as_str())), "{desktop:?}");
    let card = tasks::create(&s.st.db, &s.st.you_id, TaskInput { project_id: s.card("KADE-1").project_id.unwrap(), title: "Second".into(),
        ..Default::default() }).unwrap();
    let seq = last_seq(&s.st);
    tools::call(&s.st, &s.lead, "start_agent_run", json!({"task": "KADE-2", "agent": "Backend Agent"})).await.unwrap();
    s.runs_ended("KADE-2").await;
    let chief = actors_since(&s.st, seq);
    assert_eq!(chief, desktop, "the same lines as when the Chief starts it");
    assert_eq!(gizai_core::runs::list_for_task(&s.st.db, &card).unwrap().len(), 1);
}

// ---- GA-101's hold: only you take it off ----

#[tokio::test]
async fn on_a_card_waiting_for_your_ok_clear_hold_continue_and_start_answer_only_the_user_and_it_stays_on_hold() {
    let s = served();
    s.hold_for_your_ok("KADE-1");
    let mut c = s.conn().await;
    c.ok("get_overview", json!({})).await;
    let before = dump(&s.st);
    for (name, args) in [
        ("update_task", json!({"task": "KADE-1", "clear_hold": true})),
        ("update_task", json!({"task": "KADE-1", "hold": "blocked", "hold_reason": "Another hold"})),
        ("update_task", json!({"task": "KADE-1", "hold_reason": "Checked, go ahead"})),
        ("continue_agent_run", json!({"task": "KADE-1", "note": "Go on."})),
        ("start_agent_run", json!({"task": "KADE-1", "agent": "Backend Agent"})),
        ("start_agent_run", json!({"task": "KADE-1"})),
    ] {
        assert_eq!(c.refused(name, args.clone()).await, ONLY_THE_USER, "{name} {args}");
        assert!(s.card("KADE-1").waits_for_your_ok(), "{name} {args}");
    }
    assert_eq!(dump(&s.st), before, "nothing changed");
    assert!(gizai_core::runs::list_for_task(&s.st.db, &s.card("KADE-1").id).unwrap().is_empty(), "no run started");
    assert_eq!(s.pending(), 0);
}

#[tokio::test]
async fn move_task_from_claude_desktop_keeps_the_hold_that_waits_for_your_ok_and_the_boards_drag_still_takes_it_off() {
    let s = served();
    s.hold_for_your_ok("KADE-1");
    let mut c = s.conn().await;
    for column in ["To do", "In progress"] {
        c.ok("move_task", json!({"task": "KADE-1", "column": column})).await;
        let card = s.card("KADE-1");
        assert_eq!(card.state_name, column);
        assert!(card.waits_for_your_ok(), "{column}: still waits for your OK ({:?} {:?})", card.hold, card.hold_kind);
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(gizai_core::runs::list_for_task(&s.st.db, &s.card("KADE-1").id).unwrap().is_empty(), "no agent started it");
    // you, dragging it on the board, answer the hold (unchanged)
    let id = s.card("KADE-1").id;
    tasks::move_to(&s.st.db, &s.st.you_id, &id, &s.column("Backlog"), "a").unwrap();
    s.hold_for_your_ok("KADE-1");
    tasks::move_to(&s.st.db, &s.st.you_id, &id, &s.column("To do"), "a").unwrap();
    let card = s.card("KADE-1");
    assert_eq!((card.hold.as_deref(), card.hold_kind.as_deref()), (None, None), "the board's drag by you takes it off");
    tasks::update(&s.st.db, &s.st.you_id, &id, TaskPatch { hold: Some("blocked".into()), ..Default::default() }).unwrap();
    // and a hold that isn't GA-101's comes off from Claude Desktop with clear_hold, as from the Chief
    c.ok("update_task", json!({"task": "KADE-1", "clear_hold": true})).await;
    assert_eq!(s.card("KADE-1").hold, None);
}

// ---- GA-323: a new command for an agent ----

#[tokio::test]
async fn update_agent_or_create_agent_with_a_command_the_agent_doesnt_have_changes_nothing_and_says_to_add_it_in_the_agent_form() {
    let s = served();
    let mut c = s.conn().await;
    let had = team::agent(&s.st.db, &s.backend).unwrap().allowed_tools;
    assert!(!had.iter().any(|t| t == "Bash(terraform plan:*)"), "{had:?}");
    let before = dump(&s.st);
    let mut given = had.clone();
    given.push("Bash(terraform plan:*)".into());
    let e = c.refused("update_agent", json!({"agent": "Backend Agent", "allowed_tools": given, "title": "Builds the API"})).await;
    assert!(e.starts_with("update_agent wasn't run:") && e.contains("terraform plan") && e.contains("new for Backend Agent"), "{e}");
    assert!(e.contains("there is no chat to keep it in") && e.contains("nothing changed"), "{e}");
    assert!(e.contains("in the agent form (Permissions → Allowed commands)"), "{e}");
    let now = team::agent(&s.st.db, &s.backend).unwrap();
    assert_eq!(now.allowed_tools, had, "the agent is unchanged");
    assert_ne!(now.title.as_deref(), Some("Builds the API"), "the whole call isn't run");
    let e = c.refused("create_agent", json!({"name": "Ops Agent", "role": "backend", "allowed_tools": ["Bash(terraform plan:*)"]})).await;
    assert!(e.starts_with("create_agent wasn't run:") && e.contains("in the agent form (Permissions → Allowed commands)"), "{e}");
    assert_eq!(dump(&s.st), before, "no agent changed or made, nothing kept");
    assert_eq!(s.pending(), 0);
    // a command of Hersir's default list that runs no other command is given at once, as from the Chief's chat
    let mut given = had.clone();
    given.push("Bash(sleep:*)".into());
    if !had.iter().any(|t| t == "Bash(sleep:*)") {
        c.ok("update_agent", json!({"agent": "Backend Agent", "allowed_tools": given})).await;
        assert!(team::agent(&s.st.db, &s.backend).unwrap().allowed_tools.iter().any(|t| t == "Bash(sleep:*)"));
    }
}

// ---- GA-352: another CLI or a looser permission mode for an agent ----

#[tokio::test]
async fn moving_an_agent_to_codex_or_a_looser_mode_from_claude_desktop_changes_nothing_and_says_to_do_it_in_the_agent_form() {
    let s = served();
    let mut c = s.conn().await;
    let clis = gizai_core::clis::Cli { name: "Codex".into(), kind: "codex".into(), command: "/usr/bin/codex".into(), ..Default::default() };
    gizai_core::clis::save(&s.st.db, vec![clis]).unwrap();
    let before = dump(&s.st);
    let e = c.refused("update_agent", json!({"agent": "Backend Agent", "runs_on": "Codex", "title": "Builds the API"})).await;
    assert!(e.starts_with("update_agent wasn't run: it moves Backend Agent from Claude Code to Codex, which doesn't keep an agent to its \
                           allowed commands."), "{e}");
    assert!(e.contains("there is no chat to keep it in") && e.ends_with("the change themselves in the agent form (Agent → Runs on)."), "{e}");
    for mode in ["dontAsk", "auto"] {
        let e = c.refused("update_agent", json!({"agent": "Backend Agent", "permission_mode": mode})).await;
        assert!(e.starts_with(&format!("update_agent wasn't run: it changes Backend Agent's permission mode from acceptEdits to {mode}.")), "{e}");
        assert!(e.ends_with("in the agent form (Permissions → Permission mode)."), "{e}");
    }
    let e = c.refused("create_agent", json!({"name": "Ops Agent", "role": "backend", "runs_on": "Codex"})).await;
    assert!(e.starts_with("create_agent wasn't run: it puts Ops Agent on Codex,") && e.ends_with("(Agent → Runs on)."), "{e}");
    assert_eq!(dump(&s.st), before, "no agent changed or made, nothing kept");
    assert_eq!(s.pending(), 0);
    // a stricter mode is saved at once, as from the Chief's chat
    c.ok("update_agent", json!({"agent": "Backend Agent", "permission_mode": "plan"})).await;
    assert_eq!(team::agent(&s.st.db, &s.backend).unwrap().permission_mode.as_deref(), Some("plan"));
}

// ---- attach_file: only the Chief's copies of the code ----

#[tokio::test]
async fn attach_file_from_claude_desktop_takes_a_file_inside_the_chiefs_copies_and_refuses_one_outside() {
    let s = served();
    gizai_lib::code::startup(&s.st).await;
    let copy = s.st.data_dir.join("code/KADE");
    assert!(copy.join("docs/spec.md").is_file(), "the Chief's copy of KADE");
    let brief = s.tmp.path().join("brief.txt");
    std::fs::write(&brief, "the brief").unwrap();
    // a file you named in a chat with the Chief: Claude Desktop has no such chat, so it doesn't count here
    let thread = chat::create_thread(&s.st.db, &s.st.you_id, &s.lead, "Attach my brief").unwrap();
    chat::add_message(&s.st.db, chat::NewMessage { thread_id: thread, role: "user".into(), author_id: Some(s.st.you_id.clone()),
        body_md: Some(format!("Attach {} to KADE-1", brief.display())), ..Default::default() }).unwrap();
    let mut c = s.conn().await;
    let before = dump(&s.st);
    for f in [brief.clone(), s.repo.join(".env"), s.repo.join("docs/spec.md"), s.st.data_dir.join("gizai.db")] {
        let raw = f.display().to_string();
        if !f.exists() {
            continue;
        }
        assert_eq!(c.refused("attach_file", json!({"path": raw, "task": "KADE-1"})).await, not_in_copies(&raw), "{raw}");
    }
    assert_eq!(dump(&s.st), before, "nothing attached");
    let r = c.ok("attach_file", json!({"path": copy.join("docs/spec.md").display().to_string(), "task": "KADE-1"})).await;
    assert_eq!(r["file"]["name"], "spec.md", "{r}");
    let by: String = s.st.db.read(|db| Ok(db.query_row("SELECT uploaded_by_actor_id FROM files WHERE name='spec.md'", [], |r| r.get(0))?)).unwrap();
    assert_eq!(by, s.st.you_id, "attached as you");
    assert_eq!(c.ok("get_task", json!({"task": "KADE-1"})).await["files"][0]["name"], "spec.md");
    assert_eq!(s.pending(), 0);
}

// ---- merge_pull_request keeps its rules ----

#[tokio::test]
async fn merge_pull_request_from_claude_desktop_refuses_when_a_rule_fails_and_chief_may_merge_cant_be_switched_from_there() {
    let s = served();
    let mut c = s.conn().await;
    let project = s.card("KADE-1").project_id.unwrap();
    // the switch is off
    let before = dump(&s.st);
    let e = c.refused("merge_pull_request", json!({"task": "KADE-1"})).await;
    assert!(e.contains("doesn't let you merge") && e.contains("Nothing changed"), "{e}");
    // and Claude Desktop, though it acts as you, can't switch it on
    for (name, args) in [("update_project", json!({"project": "KADE", "lead_may_merge": true})),
                         ("update_project", json!({"project": "KADE", "team_lead_may_merge": true})),
                         ("create_project", json!({"name": "Shop", "key": "SHOP", "lead_may_merge": true}))] {
        let e = c.refused(name, args.clone()).await;
        assert!(e.contains("Chief may merge can't be switched from chat"), "{name} {args}: {e}");
    }
    assert_eq!(dump(&s.st), before);
    assert!(!projects::get(&s.st.db, &project).unwrap().lead_may_merge);
    // you switch it on and link GitHub: the card is in Backlog, not Review
    projects::update(&s.st.db, &s.st.you_id, &project, ProjectInput { name: "Kade portal".into(), key: "KADE".into(),
        repo_path: Some(s.repo.display().to_string()), repo_url: Some("https://github.com/acme/kade".into()), lead_may_merge: Some(true),
        ..Default::default() }).unwrap();
    let before = dump(&s.st);
    let e = c.refused("merge_pull_request", json!({"task": "KADE-1"})).await;
    assert!(e.contains("not Review") && e.contains("Nothing changed"), "{e}");
    // in Review with no QA verdict
    let id = s.card("KADE-1").id;
    tasks::move_to(&s.st.db, &s.st.you_id, &id, &s.column("Review"), "a").unwrap();
    let before_review = dump(&s.st);
    let e = c.refused("merge_pull_request", json!({"task": "KADE-1"})).await;
    assert!(e.contains("QA hasn't passed KADE-1") && e.contains("Nothing changed"), "{e}");
    assert_eq!(dump(&s.st), before_review);
    // on hold in Review
    tasks::update(&s.st.db, &s.st.you_id, &id, TaskPatch { hold: Some("blocked".into()), hold_reason: Some("Waiting for the client".into()),
        ..Default::default() }).unwrap();
    let e = c.refused("merge_pull_request", json!({"task": "KADE-1"})).await;
    assert!(e.contains("is on hold") && e.contains("Waiting for the client"), "{e}");
    // with testing off
    tasks::update(&s.st.db, &s.st.you_id, &id, TaskPatch { hold: Some(String::new()), testing: Some(false), ..Default::default() }).unwrap();
    let e = c.refused("merge_pull_request", json!({"task": "KADE-1"})).await;
    assert!(e.contains("has testing off"), "{e}");
    assert_ne!(before, dump(&s.st));
    assert_eq!(s.card("KADE-1").state_name, "Review", "nothing merged or moved");
}

// ---- nothing kept for your Approve is approved from Claude Desktop ----

#[tokio::test]
async fn a_call_kept_for_your_approve_in_a_chat_stays_yours_when_claude_desktop_makes_the_same_call() {
    let s = served();
    let thread = chat::create_thread(&s.st.db, &s.st.you_id, &s.lead, "Comment on KADE-1").unwrap();
    let args = json!({"task": "KADE-1", "body_md": "From the Chief"});
    let kept = gizai_core::pending::keep(&s.st.db, gizai_core::pending::NewPending { thread_id: thread, run_id: None, actor_id: s.lead.clone(),
        tool: "comment_on_task".into(), args: args.clone(), outside_tool: "mcp__otus__search".into(), note_key: None }).unwrap();
    let mut c = s.conn().await;
    c.ok("comment_on_task", args).await;
    let still = gizai_core::pending::get(&s.st.db, &kept.id).unwrap();
    assert_eq!((still.status.as_str(), still.decided_at), ("pending", None), "the kept call still waits for you in Hersir");
}

// ---- get_chief_brief ----

#[tokio::test]
async fn get_chief_brief_returns_the_cards_line_with_your_name_then_the_chiefs_instructions_as_they_are() {
    let s = served();
    let you = s.you_name();
    let mut c = s.conn().await;
    let before = dump(&s.st);
    let r = c.call("get_chief_brief", json!({})).await;
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(team::agent(&s.st.db, &s.lead).unwrap().instructions_md.as_deref(), Some(INSTRUCTIONS), "saved as given");
    assert_eq!(text(&r), format!("{}\n\n{INSTRUCTIONS}", brief_line(&you)));
    assert_eq!(dump(&s.st), before, "it only reads");
    // your name as Hersir has it now
    s.st.db.write(None, |w| { w.conn().execute("UPDATE actors SET name='Jefsev' WHERE id=?1", [&s.st.you_id])?; Ok(()) }).unwrap();
    assert!(text(&c.call("get_chief_brief", json!({})).await).starts_with(&brief_line("Jefsev")));
    // instructions left empty in the agent form: the Chief's standard ones, its role's template
    s.st.db.write(None, |w| { w.conn().execute("UPDATE agent_configs SET instructions_md='' WHERE actor_id=?1", [&s.lead])?; Ok(()) }).unwrap();
    assert_eq!(text(&c.call("get_chief_brief", json!({})).await), format!("{}\n\n{}", brief_line("Jefsev"), gizai_core::seed::role_template("lead")));
}

#[tokio::test]
async fn get_chief_brief_follows_chat_to_whichever_agent_has_it() {
    let s = served();
    let mut c = s.conn().await;
    // Chat moves to the Backend Agent, with its own instructions
    s.st.db.write(None, |w| {
        w.conn().execute("UPDATE agent_configs SET chat_enabled=0 WHERE actor_id=?1", [&s.lead])?;
        w.conn().execute("UPDATE agent_configs SET chat_enabled=1, instructions_md='Backend as Chief.' WHERE actor_id=?1", [&s.backend])?;
        Ok(())
    }).unwrap();
    let t = text(&c.call("get_chief_brief", json!({})).await);
    assert!(t.ends_with("\n\nBackend as Chief."), "{t}");
    // no agent has Chat on: the Chief's standard instructions
    s.st.db.write(None, |w| { w.conn().execute("UPDATE agent_configs SET chat_enabled=0", [])?; Ok(()) }).unwrap();
    let t = text(&c.call("get_chief_brief", json!({})).await);
    assert!(t.ends_with(&format!("\n\n{}", gizai_core::seed::role_template("lead"))), "{t}");
}
