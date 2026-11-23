// Linux and macOS only: over the Unix socket, as the hersir-mcp shim talks.
#![cfg(unix)]
//! GA-343 QA, GA-345 QA: the board as an MCP App in Claude Desktop's chat, over Hersir's socket with the tokens Hersir makes.
//! GA-345 replaced the spike's show_board and board_read with show {view, id?} and view_read, and made board_changes one
//! of the view's tools; these tests follow (claude_desktop_views_test.rs checks the card and Chief views, the writes and
//! the size of a result).
//! - Claude Code (a Chief's chat turn) sees exactly what it saw before: `initialize` without the extension or resources,
//!   the Chief's tools without `_meta`, "method not found" for `resources/…`; show, view_read and board_changes aren't its tools.
//! - Claude Desktop without the extension: GA-341's and GA-342's tools as before (GA-345: no board_changes in the list),
//!   no resources.
//! - Claude Desktop that declared `io.modelcontextprotocol/ui` (at `initialize`, or in the shim's
//!   `notifications/hersir/client`): the extension, `ui://hersir/app.html` with `text/html;profile=mcp-app`, and
//!   show, view_read and board_changes after its tools; show with both resourceUri keys, the other two with
//!   `_meta.ui.visibility` `["app"]`; no show_board or board_read.
//! - show {view: board}: one sentence in `content`, {view, id, data} in `structuredContent`, the columns and cards as the
//!   contract has them, both resourceUri keys in the result's `_meta`; view_read: the same data with seq. Nothing secret
//!   on the board.
//! - The real `hersir-mcp --desktop` from Connect's entry, end to end.
use std::path::PathBuf;
use std::time::Duration;

use gizai_core::model::*;
use gizai_core::{chat, projects, tasks, team, tokens};
use gizai_lib::claude_desktop as desktop;
use gizai_lib::tools::apps;
use gizai_lib::{AppState, tools};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

const WAIT: Duration = Duration::from_secs(10);
const APP_URI: &str = "ui://hersir/app.html";
const APP_MIME: &str = "text/html;profile=mcp-app";
/// show's description, word for word as the card (GA-345) has it.
const SHOW_DESCRIPTION: &str = "Shows the user Hersir's board, a card or the Chief's thread inside this chat. Use it when the user wants to see \
                                the board or a card, and before you answer about a card, so they see exactly what it says.";

// What must never reach structuredContent or the sentence: a card's description, acceptance criteria, hold reason, and
// the project's repository.
const SECRET_DESCRIPTION: &str = "the staging password is hunter2-description";
const SECRET_ACCEPTANCE: &str = "accept only with key sk-acceptance-123";
const SECRET_HOLD: &str = "held because of invoice 4711-hold-reason";
const SECRET_REPO: &str = "/home/someone/secret-kade-repo";

/// Claude Code's `initialize` params, as a Chief's chat turn sends them (no extensions).
fn claude_code() -> Value {
    json!({"protocolVersion": "2025-06-18", "capabilities": {"roots": {}}, "clientInfo": {"name": "claude-code", "version": "2.1.0"}})
}

/// Claude Desktop's when it shows MCP Apps' views.
fn claude_desktop_apps() -> Value {
    json!({"protocolVersion": "2025-11-25", "capabilities": {"extensions": {"io.modelcontextprotocol/ui": {"mimeTypes": [APP_MIME]}}},
        "clientInfo": {"name": "claude-ai", "version": "0.14.0"}})
}

/// Claude Desktop's without the extension.
fn claude_desktop_plain() -> Value {
    json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "claude-ai", "version": "0.13.0"}})
}

struct Conn {
    lines: Lines<BufReader<OwnedReadHalf>>,
    w: OwnedWriteHalf,
    id: i64,
}

impl Conn {
    /// The hello line with the token, then `initialize` with `params`.
    async fn open(st: &AppState, token: &str, params: Value) -> (Conn, Value) {
        let mut c = Conn::hello(st, token).await;
        let init = c.ask("initialize", params).await;
        c.notify("notifications/initialized", Value::Null).await;
        (c, init)
    }
    /// As Claude Desktop's shim connects: the hello, then the client's initialize params in its note (no initialize).
    async fn as_shim(st: &AppState, token: &str, params: Value) -> Conn {
        let mut c = Conn::hello(st, token).await;
        c.notify("notifications/hersir/client", params).await;
        c
    }
    async fn hello(st: &AppState, token: &str) -> Conn {
        let (r, mut w) = UnixStream::connect(&st.mcp_socket).await.unwrap().into_split();
        w.write_all(gizai_mcp::hello_line(token).as_bytes()).await.unwrap();
        Conn { lines: BufReader::new(r).lines(), w, id: 0 }
    }
    async fn notify(&mut self, method: &str, params: Value) {
        let msg = if params.is_null() { json!({"jsonrpc": "2.0", "method": method}) } else { json!({"jsonrpc": "2.0", "method": method, "params": params}) };
        self.w.write_all(format!("{msg}\n").as_bytes()).await.unwrap();
    }
    /// A request: its whole answer.
    async fn ask(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        let msg = json!({"jsonrpc": "2.0", "id": self.id, "method": method, "params": params});
        self.w.write_all(format!("{msg}\n").as_bytes()).await.unwrap();
        let r: Value = match tokio::time::timeout(WAIT, self.lines.next_line()).await {
            Ok(Ok(Some(l))) => serde_json::from_str(&l).unwrap(),
            Ok(_) => panic!("Hersir closed the connection"),
            Err(_) => panic!("Hersir didn't answer {method} in time"),
        };
        assert_eq!(r["id"], self.id, "{r}");
        r
    }
    async fn list(&mut self) -> Vec<Value> {
        let r = self.ask("tools/list", json!({})).await;
        r["result"]["tools"].as_array().unwrap_or_else(|| panic!("{r}")).clone()
    }
    async fn call(&mut self, name: &str, args: Value) -> Value {
        self.ask("tools/call", json!({"name": name, "arguments": args})).await["result"].clone()
    }
}

fn names(tools: &[Value]) -> Vec<String> {
    tools.iter().map(|d| d["name"].as_str().unwrap().to_string()).collect()
}

fn text(result: &Value) -> String {
    result["content"][0]["text"].as_str().unwrap_or_default().to_string()
}

struct S {
    st: AppState,
    _tmp: tempfile::TempDir,
    lead: String,
    token: String,
    _project: String,
    team: String,
    _server: tokio::task::JoinHandle<()>,
}

/// Hersir with its socket served and Claude Desktop connected (Connect's token): the Chief, project KADE ("Kade portal",
/// a repository path) with three cards: KADE-1 in Backlog, KADE-2 in To do (on hold, with a reason) and KADE-3 in To do,
/// with a description and acceptance criteria that are secret.
fn served(shim: Option<PathBuf>) -> S {
    let tmp = tempfile::tempdir().unwrap();
    let mut st = gizai_lib::test_state(tmp.path());
    st.mcp_shim = Some(shim.unwrap_or_else(|| {
        let p = tmp.path().join("hersir-mcp");
        std::fs::write(&p, "stand-in").unwrap();
        p
    }));
    let team_id = team::list(&st.db).unwrap()[0].id.clone();
    let lead = team::add_agent(&st.db, &st.you_id, &team_id, AgentInput { name: "Team Lead".into(), role_key: "lead".into(),
        chat_enabled: Some(true), ..Default::default() }).unwrap();
    let project = projects::create(&st.db, &st.you_id, ProjectInput { name: "Kade portal".into(), key: "KADE".into(),
        repo_path: Some(SECRET_REPO.into()), ..Default::default() }).unwrap();
    let states = team::get(&st.db, &team_id).unwrap().states;
    let todo = states.iter().find(|s| s.category == "ready").unwrap().id.clone();
    tasks::create(&st.db, &st.you_id, TaskInput { project_id: project.clone(), title: "Export invoices as CSV".into(), ..Default::default() }).unwrap();
    let held = tasks::create(&st.db, &st.you_id, TaskInput { project_id: project.clone(), title: "Retry failed webhooks".into(),
        state_id: Some(todo.clone()), priority: 2, ..Default::default() }).unwrap();
    tasks::update(&st.db, &st.you_id, &held, TaskPatch { hold: Some("needs_decision".into()), hold_reason: Some(SECRET_HOLD.into()),
        ..Default::default() }).unwrap();
    tasks::create(&st.db, &st.you_id, TaskInput { project_id: project.clone(), title: "Dark theme for settings".into(),
        description_md: SECRET_DESCRIPTION.into(), acceptance_md: Some(SECRET_ACCEPTANCE.into()), state_id: Some(todo), ..Default::default() }).unwrap();
    let server = gizai_lib::mcp::start(&st).unwrap();
    let c = desktop::connect(&st, false).unwrap();
    let entry: Value = serde_json::from_str(&c.entry).unwrap();
    let token = entry["mcpServers"]["hersir"]["env"]["HERSIR_TOKEN"].as_str().unwrap().to_string();
    S { st, _tmp: tmp, lead, token, _project: project, team: team_id, _server: server }
}

impl S {
    /// The team's columns in board order: (id, name, category).
    fn columns(&self) -> Vec<(String, String, String)> {
        let mut states = team::get(&self.st.db, &self.team).unwrap().states;
        states.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
        states.into_iter().map(|s| (s.id, s.name, s.category)).collect()
    }
    fn team_name(&self) -> String {
        team::get(&self.st.db, &self.team).unwrap().name
    }
    /// A Chief's chat turn's token, as Claude Code gets it.
    fn chat_token(&self) -> String {
        let thread = chat::create_thread(&self.st.db, &self.st.you_id, &self.lead, "What's on the board?").unwrap();
        tokens::mint(&self.st.db, &self.lead, json!({"chat": thread}), 60_000).unwrap()
    }
}

/// The board in a result, with the checks every show and view_read answer for the board passes: one text in `content`,
/// which is one sentence; `structuredContent` {view: board, id, data} (view_read's with seq), with nothing secret in it.
/// The board's data.
fn board_of(result: &Value) -> Value {
    assert_eq!(result["isError"], false, "{result}");
    let content = result["content"].as_array().unwrap();
    assert_eq!(content.len(), 1, "one text: {result}");
    assert_eq!(content[0]["type"], "text");
    let sentence = text(result);
    assert!(sentence.ends_with('.') && !sentence.trim_end_matches('.').contains(". ") && !sentence.contains('\n'), "one sentence: {sentence}");
    let all = result.to_string();
    for secret in [SECRET_DESCRIPTION, SECRET_ACCEPTANCE, SECRET_HOLD, SECRET_REPO] {
        assert!(!all.contains(secret), "{secret} in the result: {all}");
    }
    let s = result["structuredContent"].clone();
    assert_eq!(s["view"], "board", "{result}");
    let mut keys: Vec<&str> = s.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort();
    let want: &[&str] = if s.get("seq").is_some() { &["data", "id", "seq", "view"] } else { &["data", "id", "view"] };
    assert_eq!(keys, want, "{result}");
    s["data"].clone()
}

/// The contract's fields of a card on the board, sorted.
const BOARD_CARD_KEYS: [&str; 9] = ["assignee", "hold", "identifier", "labels", "needs_you", "priority", "project", "running", "title"];

/// Every card on a board's data: (column name, card).
fn cards_of(board: &Value) -> Vec<(String, Value)> {
    board["columns"].as_array().unwrap().iter()
        .flat_map(|c| c["cards"].as_array().unwrap().iter().map(|card| (c["name"].as_str().unwrap().to_string(), card.clone())).collect::<Vec<_>>())
        .collect()
}

// ---- Claude Code and every other token: as before ----

#[tokio::test]
async fn claude_code_in_a_chiefs_chat_turn_sees_exactly_the_tools_it_saw_before_no_resources_no_extension() {
    let s = served(None);
    let (mut c, init) = Conn::open(&s.st, &s.chat_token(), claude_code()).await;
    assert_eq!(init["result"]["capabilities"], json!({"tools": {"listChanged": false}}), "{init}");
    let listed = c.list().await;
    let catalog = tools::catalog();
    assert_eq!(names(&listed), catalog.iter().map(|t| t.name.clone()).collect::<Vec<_>>());
    for (got, want) in listed.iter().zip(&catalog) {
        assert_eq!(got, &json!({"name": want.name, "description": want.description, "inputSchema": want.input_schema,
            "annotations": {"readOnlyHint": want.read_only}}), "{}: exactly as before, no _meta", want.name);
    }
    for method in ["resources/list", "resources/read", "resources/templates/list"] {
        let r = c.ask(method, json!({"uri": APP_URI})).await;
        assert_eq!(r["error"]["code"], -32601, "{method}: {r}");
    }
    // show, view_read and board_changes aren't its tools (nor the spike's two), and a call of another tool answers text alone
    for name in [apps::SHOW, apps::VIEW_READ, tools::BOARD_CHANGES, "show_board", "board_read"] {
        let r = c.call(name, json!({"view": "board"})).await;
        assert_eq!(r["isError"], true, "{name}: {r}");
        assert!(r.get("structuredContent").is_none() && r.get("_meta").is_none(), "{name}: {r}");
    }
    let r = c.call("list_tasks", json!({"project": "KADE"})).await;
    assert_eq!(r["isError"], false, "{r}");
    assert!(r.get("structuredContent").is_none() && r.get("_meta").is_none(), "{r}");
}

#[tokio::test]
async fn a_chiefs_chat_token_gets_no_board_tools_even_if_its_client_declared_the_extension() {
    let s = served(None);
    let (mut c, _) = Conn::open(&s.st, &s.chat_token(), claude_desktop_apps()).await;
    let listed = names(&c.list().await);
    assert_eq!(listed, tools::catalog().into_iter().map(|t| t.name).collect::<Vec<_>>());
    for name in [apps::SHOW, apps::VIEW_READ, tools::BOARD_CHANGES] {
        assert_eq!(c.call(name, json!({"view": "board"})).await["isError"], true, "{name}");
    }
}

#[tokio::test]
async fn claude_desktop_without_the_extension_gets_its_tools_as_before_and_no_resources() {
    let s = served(None);
    let (mut c, init) = Conn::open(&s.st, &s.token, claude_desktop_plain()).await;
    assert_eq!(init["result"]["capabilities"], json!({"tools": {"listChanged": false}}), "{init}");
    let listed = c.list().await;
    assert_eq!(names(&listed), tools::desktop_catalog().into_iter().map(|t| t.name).collect::<Vec<_>>());
    assert!(listed.iter().all(|t| t.get("_meta").is_none()), "no _meta on any tool");
    // GA-345: board_changes is the view's, so it left this list, and show and view_read aren't in it either
    for name in [apps::SHOW, apps::VIEW_READ, tools::BOARD_CHANGES, "show_board", "board_read"] {
        assert!(!names(&listed).iter().any(|n| n == name), "{name}");
    }
    assert_eq!(c.ask("resources/list", json!({})).await["error"]["code"], -32601);
    // through the shim's note, a client that declared nothing is the same
    let mut c = Conn::as_shim(&s.st, &s.token, claude_desktop_plain()).await;
    assert_eq!(names(&c.list().await), tools::desktop_catalog().into_iter().map(|t| t.name).collect::<Vec<_>>());
    assert_eq!(c.ask("resources/list", json!({})).await["error"]["code"], -32601);
}

// ---- Claude Desktop with MCP Apps ----

#[tokio::test]
async fn claude_desktop_with_the_extension_gets_it_the_view_and_show_view_read_and_board_changes_after_its_tools() {
    let s = served(None);
    for how in ["initialize", "the shim's note"] {
        let mut c = if how == "initialize" {
            let (c, init) = Conn::open(&s.st, &s.token, claude_desktop_apps()).await;
            assert_eq!(init["result"]["capabilities"], json!({
                "tools": {"listChanged": false}, "resources": {"listChanged": false},
                "extensions": {"io.modelcontextprotocol/ui": {"mimeTypes": [APP_MIME]}},
            }), "{init}");
            c
        } else {
            Conn::as_shim(&s.st, &s.token, claude_desktop_apps()).await
        };
        let listed = c.list().await;
        let mut want: Vec<String> = tools::desktop_catalog().into_iter().map(|t| t.name).collect();
        want.extend([apps::SHOW.to_string(), apps::VIEW_READ.to_string(), tools::BOARD_CHANGES.to_string()]);
        assert_eq!(names(&listed), want, "{how}");
        assert!(!names(&listed).iter().any(|n| n == "show_board" || n == "board_read"), "{how}: the spike's tools are gone");
        let show = &listed[listed.len() - 3];
        let read = &listed[listed.len() - 2];
        let changes = &listed[listed.len() - 1];
        assert_eq!(show["name"], "show");
        assert_eq!(show["description"], SHOW_DESCRIPTION, "{how}: the card's description, word for word");
        assert_eq!(show["_meta"]["ui"]["resourceUri"], APP_URI, "{how}");
        assert_eq!(show["_meta"]["ui/resourceUri"], APP_URI, "{how}");
        assert_eq!(show["_meta"], json!({"ui": {"visibility": ["model", "app"], "resourceUri": APP_URI}, "ui/resourceUri": APP_URI}), "{how}");
        assert_eq!(read["_meta"], json!({"ui": {"visibility": ["app"]}}), "{how}: view_read is for the view alone");
        assert_eq!(changes["_meta"], json!({"ui": {"visibility": ["app"]}}), "{how}: board_changes is for the view alone");
        for t in [show, read] {
            assert_eq!(t["annotations"]["readOnlyHint"], true, "{t}");
            assert_eq!(t["inputSchema"]["properties"]["view"]["enum"], json!(["board", "card", "chief"]), "{t}");
            assert_eq!(t["inputSchema"]["properties"]["id"]["type"], "string", "{t}");
            assert_eq!(t["inputSchema"]["required"], json!(["view"]), "view is needed, id is not: {t}");
        }
        assert_eq!(changes["annotations"]["readOnlyHint"], true, "{changes}");
        assert_eq!(changes["inputSchema"]["properties"]["since"]["type"], "integer", "{changes}");
        assert!(listed[..listed.len() - 3].iter().all(|t| t.get("_meta").is_none()), "{how}: the other tools have no _meta");

        // the view
        let list = c.ask("resources/list", json!({})).await;
        let resources = list["result"]["resources"].as_array().unwrap_or_else(|| panic!("{list}"));
        assert_eq!(resources.len(), 1, "{list}");
        assert_eq!(resources[0]["uri"], APP_URI);
        assert_eq!(resources[0]["mimeType"], APP_MIME);
        let r = c.ask("resources/read", json!({"uri": APP_URI})).await;
        let contents = r["result"]["contents"].as_array().unwrap_or_else(|| panic!("{r}"));
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["uri"], APP_URI);
        assert_eq!(contents[0]["mimeType"], APP_MIME);
        let html = contents[0]["text"].as_str().unwrap();
        assert_eq!(html, &*apps::view().html, "the embedded view");
        assert!(html.to_ascii_lowercase().starts_with("<!doctype html"), "{}", &html[..html.len().min(80)]);
        assert!(html.len() < 1_000_000, "the view is under 1 MB: {} bytes", html.len());
        // its HTML holds no board data
        assert!(!html.contains("Export invoices as CSV") && !html.contains("KADE-1"));
        assert_eq!(c.ask("resources/read", json!({"uri": "ui://hersir/other.html"})).await["error"]["code"], -32002);
    }
}

#[tokio::test]
async fn show_board_answers_one_sentence_the_columns_and_cards_and_both_resource_uri_keys_and_view_read_the_same_board_with_seq() {
    let s = served(None);
    let mut c = Conn::as_shim(&s.st, &s.token, claude_desktop_apps()).await;
    let r = c.call(apps::SHOW, json!({"view": "board", "id": "KADE"})).await;
    let board = board_of(&r);
    assert_eq!(r["structuredContent"]["id"], "KADE", "{r}");
    assert_eq!(r["_meta"], json!({"ui": {"resourceUri": APP_URI}, "ui/resourceUri": APP_URI}), "{r}");
    let columns = s.columns();
    assert_eq!(text(&r), format!("The user sees the board of Kade portal (KADE) from Hersir in this chat: 3 cards in {} columns, 1 on hold.", columns.len()));

    // the team's columns in board order, each with its kind (get_workflow's) and its cards
    let got: Vec<String> = board["columns"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap().to_string()).collect();
    assert_eq!(got, columns.iter().map(|c| c.1.clone()).collect::<Vec<_>>(), "the team's columns in board order");
    let kinds: Vec<&str> = board["columns"].as_array().unwrap().iter().map(|c| c["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds.first(), Some(&"backlog"), "{kinds:?}");
    assert!(kinds.contains(&"waiting") && kinds.contains(&"review") && kinds.contains(&"done"), "{kinds:?}");
    let cards = cards_of(&board);
    let mut idents: Vec<&str> = cards.iter().map(|(_, c)| c["identifier"].as_str().unwrap()).collect();
    idents.sort();
    assert_eq!(idents, ["KADE-1", "KADE-2", "KADE-3"]);
    let card = |ident: &str| cards.iter().find(|(_, c)| c["identifier"] == ident).unwrap().clone();
    let backlog = columns.iter().find(|c| c.2 == "backlog").unwrap().1.clone();
    let todo = columns.iter().find(|c| c.2 == "ready").unwrap().1.clone();
    assert_eq!(card("KADE-1").0, backlog);
    let (column, held) = card("KADE-2");
    assert_eq!(column, todo);
    assert_eq!(held, json!({"identifier": "KADE-2", "title": "Retry failed webhooks", "project": "Kade portal", "assignee": null,
        "labels": [], "priority": 2, "hold": "needs_decision", "running": false, "needs_you": true}), "on hold: it needs you");
    assert_eq!(card("KADE-3").1["needs_you"], false);
    assert_eq!(card("KADE-3").1["hold"], Value::Null);
    // only the contract's fields on a card
    for (_, card) in &cards {
        let mut keys: Vec<&str> = card.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, BOARD_CARD_KEYS, "{card}");
    }

    // view_read: the same board, with seq (board_changes' counter now), without the result's _meta; after a change, the change
    let read = c.call(apps::VIEW_READ, json!({"view": "board", "id": "KADE"})).await;
    assert_eq!(board_of(&read), board);
    assert_eq!(read["structuredContent"]["seq"], s.st.db.change_seq(), "{read}");
    assert_eq!(text(&read), text(&r), "the same sentence");
    assert!(read.get("_meta").is_none(), "{read}");
    let kade3 = s.st.db.read(|db| Ok(db.query_row("SELECT id FROM tasks WHERE identifier='KADE-3'", [], |r| r.get::<_, String>(0))?)).unwrap();
    let review = columns.iter().find(|c| c.2 == "review").unwrap().clone();
    tasks::move_to(&s.st.db, &s.st.you_id, &kade3, &review.0, "zzz").unwrap();
    let read = c.call(apps::VIEW_READ, json!({"view": "board", "id": "kade"})).await;
    assert_eq!(read["structuredContent"]["id"], "KADE", "the key, as Hersir has it");
    let after = board_of(&read);
    let moved = cards_of(&after).into_iter().find(|(_, c)| c["identifier"] == "KADE-3").unwrap();
    assert_eq!(moved.0, review.1, "{moved:?}");
}

#[tokio::test]
async fn show_board_without_a_project_is_the_first_teams_board_and_an_unknown_project_is_an_error() {
    let s = served(None);
    let mut c = Conn::as_shim(&s.st, &s.token, claude_desktop_apps()).await;
    let r = c.call(apps::SHOW, json!({"view": "board"})).await;
    let board = board_of(&r);
    assert_eq!(r["structuredContent"]["id"], Value::Null, "{r}");
    assert_eq!(cards_of(&board).len(), 3);
    assert_eq!(text(&r), format!("The user sees {}'s board from Hersir in this chat: 3 cards in {} columns, 1 on hold.", s.team_name(), s.columns().len()));
    for name in [apps::SHOW, apps::VIEW_READ] {
        let r = c.call(name, json!({"view": "board", "id": "NOPE"})).await;
        assert_eq!(r["isError"], true, "{name}: {r}");
        assert!(r.get("structuredContent").is_none() && r.get("_meta").is_none(), "{name}: {r}");
        assert!(text(&r).contains("NOPE"), "{name}: {r}");
    }
}

/// GA-345: board_changes is one of the view's tools now, listed last, after show and view_read, and answers the
/// contract's names, seq and cards (with board and reload), as JSON text and in structuredContent, naming a card moved
/// since its counter. A Claude Desktop without the extension isn't offered it, but a call still answers.
#[tokio::test]
async fn a_claude_desktop_that_declared_mcp_apps_calls_board_changes_and_gets_seq_and_cards() {
    let s = served(None);
    let mut c = Conn::as_shim(&s.st, &s.token, claude_desktop_apps()).await;
    let listed = names(&c.list().await);
    let at = listed.iter().position(|n| n == tools::BOARD_CHANGES).unwrap_or_else(|| panic!("board_changes isn't listed: {listed:?}"));
    assert_eq!(&listed[at - 2..], [apps::SHOW, apps::VIEW_READ, tools::BOARD_CHANGES], "{listed:?}");

    let changes = |r: &Value| -> Value {
        assert_eq!(r["isError"], false, "{r}");
        assert!(r.get("_meta").is_none(), "{r}");
        assert_eq!(r["content"].as_array().unwrap().len(), 1, "{r}");
        let v: Value = serde_json::from_str(&text(r)).unwrap_or_else(|e| panic!("{e}: {r}"));
        assert_eq!(r["structuredContent"], v, "the same in structuredContent: {r}");
        let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, ["board", "cards", "reload", "seq"], "{v}");
        v
    };
    let start = changes(&c.call(tools::BOARD_CHANGES, json!({})).await);
    let since = start["seq"].as_i64().unwrap_or_else(|| panic!("{start}"));
    assert_eq!(since, s.st.db.change_seq());
    assert_eq!(start["cards"], json!([]), "{start}");

    let kade3 = s.st.db.read(|db| Ok(db.query_row("SELECT id FROM tasks WHERE identifier='KADE-3'", [], |r| r.get::<_, String>(0))?)).unwrap();
    let other = s.columns().into_iter().find(|c| c.2 != "ready" && c.2 != "backlog").unwrap().0;
    tasks::move_to(&s.st.db, &s.st.you_id, &kade3, &other, "zzz").unwrap();

    let after = changes(&c.call(tools::BOARD_CHANGES, json!({"since": since})).await);
    assert!(after["seq"].as_i64().unwrap() > since, "{after}");
    assert_eq!(after["cards"], json!(["KADE-3"]), "{after}");
    assert_eq!(after["reload"], false, "{after}");

    // a Claude Desktop without the extension: not offered, but a call still answers
    let (mut plain, _) = Conn::open(&s.st, &s.token, claude_desktop_plain()).await;
    assert!(!names(&plain.list().await).iter().any(|n| n == tools::BOARD_CHANGES));
    let r = plain.call(tools::BOARD_CHANGES, json!({"since": since})).await;
    assert_eq!(r["isError"], false, "{r}");
    let v: Value = serde_json::from_str(&text(&r)).unwrap();
    assert_eq!(v["cards"], json!(["KADE-3"]), "{v}");
}

// ---- the real hersir-mcp --desktop ----

/// The real hersir-mcp: target/debug/hersir-mcp, which the crate gizai-mcp builds (built here when missing).
fn real_shim() -> PathBuf {
    static BUILT: std::sync::Once = std::sync::Once::new();
    let path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../target/debug/hersir-mcp"));
    BUILT.call_once(|| {
        if path.is_file() {
            return;
        }
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let ok = std::process::Command::new(cargo).args(["build", "-q", "-p", "gizai-mcp"])
            .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/..")).status().map(|s| s.success()).unwrap_or(false);
        assert!(ok && path.is_file(), "could not build hersir-mcp (the crate gizai-mcp)");
    });
    path
}

/// Starts Connect's entry as Claude Desktop does, with only its environment, sends `requests` and returns the lines it
/// wrote until each request has its answer.
fn run_entry(st: &AppState, requests: &[Value]) -> Vec<Value> {
    use std::io::{BufRead, Write};
    let entry: Value = serde_json::from_str(&desktop::connect(st, false).unwrap().entry).unwrap();
    let s = &entry["mcpServers"]["hersir"];
    let mut cmd = std::process::Command::new(s["command"].as_str().unwrap());
    cmd.args(s["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap())).env_clear();
    for (k, v) in s["env"].as_object().unwrap() {
        cmd.env(k, v.as_str().unwrap());
    }
    let mut child = cmd.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null())
        .spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for q in requests {
        stdin.write_all(format!("{q}\n").as_bytes()).unwrap();
    }
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for l in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(l).is_err() {
                break;
            }
        }
    });
    let mut out: Vec<Value> = vec![];
    let want = requests.iter().filter(|q| q.get("id").is_some()).count();
    while out.iter().filter(|l| l.get("id").is_some()).count() < want {
        match rx.recv_timeout(WAIT) {
            Ok(l) => out.push(serde_json::from_str(&l).unwrap_or(Value::String(l))),
            Err(_) => break,
        }
    }
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn the_real_hersir_mcp_desktop_gives_a_declaring_claude_desktop_the_extension_the_view_and_show() {
    let s = served(Some(real_shim()));
    let req = |id: i64, method: &str, params: Value| json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    let requests = [
        req(1, "initialize", claude_desktop_apps()),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        req(2, "tools/list", json!({})),
        req(3, "resources/list", json!({})),
        req(4, "resources/read", json!({"uri": APP_URI})),
        req(5, "tools/call", json!({"name": "show", "arguments": {"view": "board", "id": "KADE"}})),
        req(6, "tools/call", json!({"name": "view_read", "arguments": {"view": "card", "id": "KADE-1"}})),
    ];
    let st = s.st.clone();
    let out = tokio::task::spawn_blocking(move || run_entry(&st, &requests)).await.unwrap();
    let answer = |id: i64| out.iter().find(|l| l["id"] == id).unwrap_or_else(|| panic!("no answer to {id}: {out:?}")).clone();
    assert_eq!(answer(1)["result"]["capabilities"]["extensions"], json!({"io.modelcontextprotocol/ui": {"mimeTypes": [APP_MIME]}}));
    assert_eq!(answer(1)["result"]["capabilities"]["resources"], json!({"listChanged": true}));
    let listed = names(answer(2)["result"]["tools"].as_array().unwrap());
    assert_eq!(&listed[listed.len() - 3..], ["show", "view_read", "board_changes"], "{listed:?}");
    assert_eq!(answer(3)["result"]["resources"][0]["uri"], APP_URI);
    assert_eq!(answer(4)["result"]["contents"][0]["mimeType"], APP_MIME);
    assert_eq!(answer(4)["result"]["contents"][0]["text"], apps::view().html.to_string());
    let shown = answer(5)["result"].clone();
    assert_eq!(board_of(&shown)["project"]["key"], "KADE");
    assert_eq!(shown["structuredContent"]["id"], "KADE");
    assert_eq!(shown["_meta"], json!({"ui": {"resourceUri": APP_URI}, "ui/resourceUri": APP_URI}));
    let card = answer(6)["result"].clone();
    assert_eq!(card["isError"], false, "{card}");
    assert_eq!(card["structuredContent"]["data"]["identifier"], "KADE-1", "{card}");
    assert_eq!(card["structuredContent"]["seq"], s.st.db.change_seq(), "{card}");

    // Claude Desktop without the extension through the same entry: as before
    let requests = [req(1, "initialize", claude_desktop_plain()), req(2, "tools/list", json!({})), req(3, "resources/list", json!({}))];
    let st = s.st.clone();
    let out = tokio::task::spawn_blocking(move || run_entry(&st, &requests)).await.unwrap();
    let answer = |id: i64| out.iter().find(|l| l["id"] == id).unwrap_or_else(|| panic!("no answer to {id}: {out:?}")).clone();
    assert_eq!(answer(1)["result"]["capabilities"], json!({"tools": {"listChanged": true}}));
    assert_eq!(names(answer(2)["result"]["tools"].as_array().unwrap()), tools::desktop_catalog().into_iter().map(|t| t.name).collect::<Vec<_>>());
    assert_eq!(answer(3)["error"]["code"], -32601);
}
