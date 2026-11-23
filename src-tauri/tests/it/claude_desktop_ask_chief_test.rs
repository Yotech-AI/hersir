// Linux and macOS only: over the Unix socket, and the Chief's turns run Python scripts as a fake Claude Code.
#![cfg(unix)]
//! GA-342 QA: Claude Desktop asks Hersir's Chief (ask_chief) and reads its answer (chief_reply). Over the socket, with the
//! token Connect makes (`claude_desktop::connect`), as the hersir-mcp shim talks; the Chief's turns run a fake Claude Code
//! (fake-claude-chat.py, fake-claude-chat-outside.py) with the real gizai-mcp shim, never the real one.
//! - tools/list: get_chief_brief, ask_chief, chief_reply, then the Chief's tools, then board_changes (GA-344); the card's descriptions word for word.
//!   A chat's, a board check's and a question's tokens aren't offered them, and a Chief's turn that calls ask_chief
//!   anyway starts no other turn.
//! - ask_chief: without a thread, the chat From Claude Desktop (made once, the same after a rename, a new one after it
//!   was deleted); answers within 2 s with thread, message and status working; the message is yours and starts a turn,
//!   or waits in the queue while one runs and starts the next; the Chief's turn keeps its normal rules (no pending
//!   action, the chat isn't marked), and GA-102's only after the Chief itself uses a tool from outside Hersir.
//! - chief_reply: working with a draft, answered with the new messages, waiting for you; returns as soon as the turn ends
//!   or a message comes, no later than wait_seconds; after, at most the last 20, no tool calls; gizai: links as their
//!   text, texts and the draft cut at 8,000 characters.
//! - An unknown thread (or message) is refused and changes nothing.
use std::path::PathBuf;
use std::time::{Duration, Instant};

use gizai_core::model::*;
use gizai_core::{chat, projects, settings, tasks, team, tokens};
use gizai_lib::claude_desktop as desktop;
use gizai_lib::tools::desktop::{CUT, MAX_TEXT, cut, plain};
use gizai_lib::{AppState, chat as app_chat, mcp, pending, tools};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

const FAKE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude-chat.py");
const FAKE_OUTSIDE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude-chat-outside.py");
const WAIT: Duration = Duration::from_secs(10);

// The card's texts, word for word.
const ASK_DESCRIPTION: &str = "Hands a message to Hersir's Chief, who answers with its memory, its copies of the code and the board. Answers at \
                               once; then call chief_reply for its answer.";
const REPLY_DESCRIPTION: &str = "Hersir's Chief's answer in a thread from ask_chief: its new messages, and whether it is still working. Call it \
                                 again while status is working.";
const FROM_CLAUDE_DESKTOP: &str = "From Claude Desktop";
const CUT_TEXT: &str = "…(cut: read the rest in Hersir)";

/// The shim binary: target/debug/hersir-mcp, which the crate gizai-mcp builds (built here when missing).
fn shim() -> PathBuf {
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
        c.ask(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}}), WAIT).await;
        c.w.write_all(format!("{}\n", json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).as_bytes()).await.unwrap();
        c
    }
    async fn ask(&mut self, msg: Value, within: Duration) -> Value {
        self.w.write_all(format!("{msg}\n").as_bytes()).await.unwrap();
        match tokio::time::timeout(within, self.lines.next_line()).await {
            Ok(Ok(Some(l))) => serde_json::from_str(&l).unwrap(),
            Ok(_) => panic!("Hersir closed the connection"),
            Err(_) => panic!("Hersir didn't answer within {within:?}"),
        }
    }
    async fn list(&mut self) -> Vec<Value> {
        self.id += 1;
        let r = self.ask(json!({"jsonrpc": "2.0", "id": self.id, "method": "tools/list"}), WAIT).await;
        r["result"]["tools"].as_array().unwrap_or_else(|| panic!("{r}")).clone()
    }
    /// tools/call, answered within `within`: the result (content and isError), and how long it took.
    async fn call_within(&mut self, name: &str, args: Value, within: Duration) -> (Value, Duration) {
        self.id += 1;
        let id = self.id;
        let t0 = Instant::now();
        let r = self.ask(json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": args}}), within).await;
        let took = t0.elapsed();
        assert_eq!(r["id"], id, "{r}");
        (r["result"].clone(), took)
    }
    async fn call(&mut self, name: &str, args: Value) -> Value {
        self.call_within(name, args, WAIT).await.0
    }
    /// A call that must work: its answer, parsed, and how long it took.
    async fn ok_within(&mut self, name: &str, args: Value, within: Duration) -> (Value, Duration) {
        let (r, took) = self.call_within(name, args.clone(), within).await;
        assert_eq!(r["isError"], false, "{name} {args}: {r}");
        (serde_json::from_str(&text(&r)).unwrap_or_else(|_| panic!("{name}: not JSON: {r}")), took)
    }
    /// A call that must be refused: its text.
    async fn refused(&mut self, name: &str, args: Value) -> String {
        let r = self.call(name, args.clone()).await;
        assert_eq!(r["isError"], true, "{name} {args}: {r}");
        text(&r)
    }
    /// ask_chief, which must answer within 2 seconds (the card): (thread, message).
    async fn ask_chief(&mut self, args: Value) -> (String, String) {
        let (r, took) = self.ok_within("ask_chief", args.clone(), WAIT).await;
        assert!(took < Duration::from_secs(2), "ask_chief {args} took {took:?}");
        let keys: Vec<&String> = r.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["message", "status", "thread"], "{r}");
        assert_eq!(r["status"], "working", "{r}");
        (r["thread"].as_str().unwrap().to_string(), r["message"].as_str().unwrap().to_string())
    }
    /// chief_reply, with wait_seconds 0: what it says now.
    async fn reply_now(&mut self, thread: &str, after: Option<&str>) -> Value {
        let mut args = json!({"thread": thread, "wait_seconds": 0});
        if let Some(a) = after {
            args["after"] = json!(a);
        }
        let (r, took) = self.ok_within("chief_reply", args, WAIT).await;
        assert!(took < Duration::from_secs(2), "wait_seconds 0 answers at once, took {took:?}");
        r
    }
    /// What Claude Desktop does after ask_chief: chief_reply with the last id it got, while status is working. Every
    /// message it got (in order), and the last answer.
    async fn reply_until_done(&mut self, thread: &str, after: &str) -> (Vec<Value>, Value) {
        let mut got = vec![];
        let mut after = after.to_string();
        for _ in 0..20 {
            let (r, _) = self.ok_within("chief_reply", json!({"thread": thread, "after": after, "wait_seconds": 20}), Duration::from_secs(30)).await;
            for m in r["messages"].as_array().unwrap() {
                got.push(m.clone());
                after = m["id"].as_str().unwrap().to_string();
            }
            if r["status"] != "working" {
                return (got, r);
            }
        }
        panic!("the Chief kept working: {got:?}");
    }
}

fn text(result: &Value) -> String {
    result["content"][0]["text"].as_str().unwrap_or_default().to_string()
}

fn names(tools: &[Value]) -> Vec<String> {
    tools.iter().map(|d| d["name"].as_str().unwrap().to_string()).collect()
}

/// (from, text) of chief_reply's messages.
fn said(messages: &[Value]) -> Vec<(String, String)> {
    messages.iter().map(|m| (m["from"].as_str().unwrap().to_string(), m["text"].as_str().unwrap().to_string())).collect()
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
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

struct S {
    st: AppState,
    lead: String,
    token: String,
    _tmp: tempfile::TempDir,
    _server: tokio::task::JoinHandle<()>,
}

/// Hersir with its socket served and Claude Desktop connected: the Chief (Team Lead, Chat on), project KADE with KADE-1
/// "Export" in Backlog, the queue paused. The Chief's Claude Code is `bin`, a fake, with the real gizai-mcp shim.
fn served(bin: &str) -> S {
    let tmp = tempfile::tempdir().unwrap();
    let mut st = gizai_lib::test_state(tmp.path());
    st.mcp_shim = Some(shim());
    settings::set(&st.db, "claude_bin", &bin.to_string()).unwrap();
    settings::set(&st.db, "agents_paused", &true).unwrap();
    let team_id = team::list(&st.db).unwrap()[0].id.clone();
    let lead = team::add_agent(&st.db, &st.you_id, &team_id, AgentInput { name: "Team Lead".into(), role_key: "lead".into(),
        chat_enabled: Some(true), ..Default::default() }).unwrap();
    let project = projects::create(&st.db, &st.you_id, ProjectInput { name: "Kade portal".into(), key: "KADE".into(), ..Default::default() }).unwrap();
    tasks::create(&st.db, &st.you_id, TaskInput { project_id: project, title: "Export".into(), ..Default::default() }).unwrap();
    let server = mcp::start(&st).unwrap();
    let c = desktop::connect(&st, false).unwrap();
    let entry: Value = serde_json::from_str(&c.entry).unwrap();
    let token = entry["mcpServers"]["hersir"]["env"]["HERSIR_TOKEN"].as_str().unwrap().to_string();
    S { st, lead, token, _tmp: tmp, _server: server }
}

impl S {
    async fn conn(&self) -> Conn {
        Conn::open(&self.st, &self.token).await
    }
    fn answering(&self, thread: &str) -> bool {
        app_chat::live(&self.st).iter().any(|l| l.thread_id == thread)
    }
    /// Waits until the Chief no longer answers in `thread` (its queue went too).
    async fn idle(&self, thread: &str) {
        for _ in 0..600 {
            if !self.answering(thread) && !chat::queue_ready(&self.st.db, thread).unwrap() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("the Chief kept answering in {thread}");
    }
    /// Waits until the fake waits for fake-go (FAKE_CHAT_WAIT).
    async fn fake_waits(&self) {
        let f = self.st.data_dir.join("chat/fake-waiting");
        for _ in 0..400 {
            if f.exists() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("the fake never waited");
    }
    fn go(&self) {
        std::fs::write(self.st.data_dir.join("chat/fake-go"), "").unwrap();
    }
    /// Waits until the Chief's answer in `thread` streams the draft `want`.
    async fn draft_is(&self, thread: &str, want: &str) {
        for _ in 0..200 {
            if app_chat::live(&self.st).iter().any(|l| l.thread_id == thread && l.draft == want) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("no draft {want:?}: {:?}", app_chat::live(&self.st).iter().map(|l| l.draft.clone()).collect::<Vec<_>>());
    }
    /// The chat's runs: (trigger, status), oldest first.
    fn runs(&self, thread: &str) -> Vec<(String, String)> {
        self.st.db.read(|c| {
            let mut q = c.prepare("SELECT trigger, status FROM runs WHERE chat_thread_id=?1 ORDER BY created_at, rowid")?;
            let rows = q.query_map([thread], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }).unwrap()
    }
    /// The chat's messages: (role, author, text), tool calls left out.
    fn messages(&self, thread: &str) -> Vec<(String, Option<String>, String)> {
        chat::messages(&self.st.db, thread).unwrap().into_iter().filter(|m| m.role != "tool")
            .map(|m| (m.role, m.author_id, m.body_md.unwrap_or_default())).collect()
    }
    fn desktop_chats(&self) -> i64 {
        count(&self.st, "SELECT count(*) FROM chat_threads WHERE title='From Claude Desktop' AND deleted_at IS NULL")
    }
    fn calls(&self) -> Vec<Value> {
        std::fs::read_to_string(self.st.data_dir.join("chat/fake-calls.jsonl")).unwrap_or_default()
            .lines().map(|l| serde_json::from_str(l).unwrap()).collect()
    }
}

// ---- tools/list ----

#[tokio::test]
async fn the_desktop_token_lists_ask_chief_and_chief_reply_after_get_chief_brief_with_the_cards_descriptions() {
    let s = served(FAKE);
    let mut c = s.conn().await;
    let listed = c.list().await;
    let mut want = vec!["get_chief_brief".to_string(), "ask_chief".into(), "chief_reply".into()];
    want.extend(tools::catalog().into_iter().map(|d| d.name).filter(|n| n != "start_chat" && n != "update_checkout"));
    // GA-344: and last board_changes
    want.push(tools::BOARD_CHANGES.to_string());
    assert_eq!(names(&listed), want);
    let ask = &listed[1];
    assert_eq!(ask["description"], ASK_DESCRIPTION);
    assert_eq!(ask["annotations"]["readOnlyHint"], false, "{ask}");
    assert_eq!(ask["inputSchema"]["required"], json!(["message"]), "{ask}");
    let props = ask["inputSchema"]["properties"].as_object().unwrap();
    assert_eq!(props.keys().collect::<Vec<_>>(), ["message", "thread"], "{ask}");
    assert!(props.values().all(|p| p["type"] == "string"), "{ask}");
    let reply = &listed[2];
    assert_eq!(reply["description"], REPLY_DESCRIPTION);
    assert_eq!(reply["annotations"]["readOnlyHint"], true, "{reply}");
    assert_eq!(reply["inputSchema"]["required"], json!(["thread"]), "{reply}");
    let props = reply["inputSchema"]["properties"].as_object().unwrap();
    assert_eq!(props.keys().collect::<Vec<_>>(), ["after", "thread", "wait_seconds"], "{reply}");
    assert_eq!((&props["thread"]["type"], &props["after"]["type"], &props["wait_seconds"]["type"]), (&json!("string"), &json!("string"), &json!("integer")));
    // the Chief's own turns aren't offered them
    let catalog: Vec<String> = tools::catalog().into_iter().map(|d| d.name).collect();
    assert!(!catalog.contains(&"ask_chief".to_string()) && !catalog.contains(&"chief_reply".to_string()), "{catalog:?}");
}

#[tokio::test]
async fn a_chats_a_board_checks_and_a_questions_tokens_arent_offered_ask_chief_or_chief_reply_and_calling_them_changes_nothing() {
    let s = served(FAKE);
    let thread = chat::create_thread(&s.st.db, &s.st.you_id, &s.lead, "What's on the board?").unwrap();
    for scope in [json!({"chat": thread}), json!({"check": "R1"}), json!({"question": "R1"})] {
        let token = tokens::mint(&s.st.db, &s.lead, scope.clone(), 60_000).unwrap();
        let mut c = Conn::open(&s.st, &token).await;
        let listed = names(&c.list().await);
        assert!(!listed.iter().any(|n| n == "ask_chief" || n == "chief_reply"), "{scope}: {listed:?}");
        let before = dump(&s.st);
        for (name, args) in [("ask_chief", json!({"message": "Start another turn"})), ("ask_chief", json!({"message": "Again", "thread": thread})),
                             ("chief_reply", json!({"thread": thread}))] {
            let e = c.refused(name, args.clone()).await;
            if scope.get("question").is_some() {
                assert!(e.starts_with(&format!("{name} can't be used while you look at an agent's question")), "{scope} {name}: {e}");
            } else {
                assert_eq!(e, format!("unknown tool {name}"), "{scope} {args}");
            }
        }
        assert_eq!(dump(&s.st), before, "{scope}: no chat, no message, no turn");
        assert_eq!(s.desktop_chats(), 0);
    }
    assert!(app_chat::live(&s.st).is_empty(), "no Chief turn started");
}

#[tokio::test]
async fn a_chiefs_turn_that_calls_ask_chief_or_chief_reply_is_refused_and_starts_no_other_turn() {
    let s = served(FAKE_OUTSIDE);
    let mut c = s.conn().await;
    let (thread, _) = c.ask_chief(json!({"message": "CALL ask_chief {\"message\": \"Start another turn\"}"})).await;
    s.idle(&thread).await;
    let (thread2, _) = c.ask_chief(json!({"message": format!("CALL chief_reply {{\"thread\": \"{thread}\"}}")})).await;
    assert_eq!(thread2, thread);
    s.idle(&thread).await;
    let msgs = chat::messages(&s.st.db, &thread).unwrap();
    let results: Vec<(Option<String>, Value)> = msgs.iter().filter(|m| m.role == "tool")
        .map(|m| (m.tool_name.clone(), m.tool.clone().unwrap())).collect();
    assert_eq!(results.len(), 2, "{results:?}");
    for ((name, tool), want) in results.iter().zip(["ask_chief", "chief_reply"]) {
        assert_eq!(name.as_deref(), Some(format!("mcp__gizai__{want}").as_str()));
        assert_eq!(tool["isError"], true, "{tool}");
        assert_eq!(tool["result"], format!("unknown tool {want}"), "{tool}");
    }
    assert_eq!(s.runs(&thread), [("chat".to_string(), "succeeded".to_string()), ("chat".into(), "succeeded".into())], "only the two turns asked");
    assert_eq!(count(&s.st, "SELECT count(*) FROM chat_threads"), 1, "no other chat");
    assert_eq!(count(&s.st, "SELECT count(*) FROM runs"), 2, "no other run");
    let mine = s.messages(&thread).into_iter().filter(|m| m.0 == "user").count();
    assert_eq!(mine, 2, "only Claude Desktop's two messages");
    assert!(app_chat::live(&s.st).is_empty(), "nothing else answers");
}

// ---- ask_chief: the thread ----

#[tokio::test]
async fn ask_chief_without_a_thread_makes_the_chat_from_claude_desktop_the_first_time_and_uses_it_after_that() {
    let s = served(FAKE);
    let mut c = s.conn().await;
    let (thread, m1) = c.ask_chief(json!({"message": "What is going on?"})).await;
    let t = chat::get_thread(&s.st.db, &thread).unwrap();
    assert_eq!((t.title.as_str(), t.agent_id.as_str(), t.created_by.as_deref(), t.kind.as_deref()),
               (FROM_CLAUDE_DESKTOP, s.lead.as_str(), Some(s.st.you_id.as_str()), None), "a chat of yours with the Chief");
    assert_eq!(settings::get::<String>(&s.st.db, chat::DESKTOP_CHAT).unwrap().as_deref(), Some(thread.as_str()));
    s.idle(&thread).await;
    let (again, m2) = c.ask_chief(json!({"message": "And now?"})).await;
    assert_eq!(again, thread, "the same chat");
    assert_ne!(m1, m2);
    s.idle(&thread).await;
    assert_eq!(s.desktop_chats(), 1);
    // Hersir's chat list shows it, and you carry on with it there
    let listed = chat::list_threads(&s.st.db).unwrap();
    assert_eq!(listed.iter().map(|t| (t.id.as_str(), t.title.as_str())).collect::<Vec<_>>(), [(thread.as_str(), FROM_CLAUDE_DESKTOP)]);
    let (same, done) = app_chat::send(&s.st, Some(thread.clone()), "From Hersir now".into(), None).await.unwrap();
    assert_eq!(same, thread);
    assert_eq!(tokio::time::timeout(Duration::from_secs(30), done).await.unwrap().unwrap().status, "succeeded");
    assert_eq!(s.messages(&thread).iter().filter(|m| m.0 == "user").map(|m| m.2.as_str()).collect::<Vec<_>>(),
               ["What is going on?", "And now?", "From Hersir now"]);
    // renamed in Hersir: still the same chat
    chat::rename_thread(&s.st.db, &s.st.you_id, &thread, "Desktop questions").unwrap();
    let (renamed, _) = c.ask_chief(json!({"message": "Still there?"})).await;
    assert_eq!(renamed, thread);
    assert_eq!(chat::get_thread(&s.st.db, &thread).unwrap().title, "Desktop questions");
    s.idle(&thread).await;
    // deleted: a new From Claude Desktop
    s.st.db.write(None, |w| { w.conn().execute("UPDATE chat_threads SET deleted_at=1 WHERE id=?1", [&thread])?; Ok(()) }).unwrap();
    let (fresh, _) = c.ask_chief(json!({"message": "Hello again"})).await;
    assert_ne!(fresh, thread);
    assert_eq!(chat::get_thread(&s.st.db, &fresh).unwrap().title, FROM_CLAUDE_DESKTOP);
    assert_eq!(s.desktop_chats(), 1);
    s.idle(&fresh).await;
    let (still, _) = c.ask_chief(json!({"message": "Same one?"})).await;
    assert_eq!(still, fresh);
    s.idle(&fresh).await;
}

#[tokio::test]
async fn ask_chief_with_a_thread_writes_in_that_chat_with_the_chief() {
    let s = served(FAKE);
    let mut c = s.conn().await;
    let (mine, done) = app_chat::send(&s.st, None, "Started in Hersir".into(), None).await.unwrap();
    tokio::time::timeout(Duration::from_secs(30), done).await.unwrap().unwrap();
    let (thread, m) = c.ask_chief(json!({"message": "From Claude Desktop into your chat", "thread": mine})).await;
    assert_eq!(thread, mine);
    s.idle(&mine).await;
    assert_eq!(s.desktop_chats(), 0, "no chat From Claude Desktop made");
    let msgs = s.messages(&mine);
    assert_eq!(msgs.iter().filter(|m| m.0 == "user").map(|m| m.2.as_str()).collect::<Vec<_>>(), ["Started in Hersir", "From Claude Desktop into your chat"]);
    let r = c.reply_now(&mine, Some(&m)).await;
    assert_eq!(r["status"], "answered", "{r}");
    assert_eq!(said(r["messages"].as_array().unwrap()), pairs(&[("Chief", "Sure, on it."), ("Chief", "Here is the overview.")]));
    assert_eq!(s.runs(&mine).len(), 2);
}

// ---- ask_chief: the message is yours and starts a turn, or waits in the queue ----

#[tokio::test]
async fn the_message_is_yours_and_starts_the_chiefs_turn_like_one_typed_in_hersirs_chat() {
    let s = served(FAKE);
    let mut c = s.conn().await;
    // the same message typed in Hersir's chat, for comparison
    let (typed, done) = app_chat::send(&s.st, None, "Hello there".into(), None).await.unwrap();
    tokio::time::timeout(Duration::from_secs(30), done).await.unwrap().unwrap();
    let (thread, m) = c.ask_chief(json!({"message": "Hello there"})).await;
    let msg = chat::messages(&s.st.db, &thread).unwrap().into_iter().find(|x| x.id == m).expect("saved before ask_chief answers");
    assert_eq!((msg.role.as_str(), msg.author_id.as_deref(), msg.body_md.as_deref()), ("user", Some(s.st.you_id.as_str()), Some("Hello there")));
    let typed_msg = &chat::messages(&s.st.db, &typed).unwrap()[0];
    assert_eq!((typed_msg.role.as_str(), typed_msg.author_id.as_deref()), ("user", msg.author_id.as_deref()), "as yours as a typed one");
    s.idle(&thread).await;
    assert_eq!(s.runs(&thread), [("chat".to_string(), "succeeded".to_string())], "the Chief's turn ran");
    // Claude Code got it as it gets a message typed in Hersir: the same flags, the text in the prompt
    let calls = s.calls();
    assert_eq!(calls.len(), 2);
    let flags = |call: &Value| -> Vec<String> {
        call["argv"].as_array().unwrap().iter().filter_map(|a| a.as_str()).filter(|a| a.starts_with("--")).map(str::to_string).collect()
    };
    assert_eq!(flags(&calls[1]), flags(&calls[0]));
    let (p0, p1) = (calls[0]["prompt"].as_str().unwrap(), calls[1]["prompt"].as_str().unwrap());
    assert!(p0.contains("Hello there") && p1.contains("Hello there"), "{p1}");
    assert_eq!(p1, p0, "the same prompt as the typed message");
}

#[tokio::test]
async fn while_a_turn_runs_the_message_waits_in_the_queue_and_starts_the_next_turn() {
    let s = served(FAKE);
    let mut c = s.conn().await;
    let (thread, m1) = c.ask_chief(json!({"message": "FAKE_CHAT_WAIT first"})).await;
    s.fake_waits().await;
    let (same, q) = c.ask_chief(json!({"message": "second", "thread": thread})).await;
    assert_eq!(same, thread);
    let queue = chat::queue(&s.st.db, &thread).unwrap();
    assert_eq!(queue.iter().map(|x| (x.id.as_str(), x.body_md.as_str(), x.held)).collect::<Vec<_>>(), [(q.as_str(), "second", false)]);
    // and without a thread too: the same chat, the same queue
    let (third, q3) = c.ask_chief(json!({"message": "third"})).await;
    assert_eq!(third, thread);
    assert_eq!(chat::queue(&s.st.db, &thread).unwrap().len(), 2);
    let r = c.reply_now(&thread, Some(&m1)).await;
    assert_eq!(r["status"], "working", "{r}");
    assert_eq!(r["messages"], json!([]), "{r}");
    // the queued one isn't a message yet: nothing comes after it
    assert_eq!(c.reply_now(&thread, Some(&q)).await["messages"], json!([]));
    s.go();
    let (got, last) = c.reply_until_done(&thread, &m1).await;
    assert_eq!(last["status"], "answered", "{last}");
    assert!(last.get("draft").is_none(), "{last}");
    s.idle(&thread).await;
    // the queued messages went as one next turn, keeping their ids, and they are yours
    let all = chat::messages(&s.st.db, &thread).unwrap();
    for (id, body) in [(&q, "second"), (&q3, "third")] {
        let m = all.iter().find(|m| &m.id == id).unwrap_or_else(|| panic!("{body} kept its id"));
        assert_eq!((m.role.as_str(), m.author_id.as_deref(), m.body_md.as_deref()), ("user", Some(s.st.you_id.as_str()), Some(body)));
    }
    assert!(chat::queue(&s.st.db, &thread).unwrap().is_empty());
    assert_eq!(s.runs(&thread), [("chat".to_string(), "succeeded".to_string()), ("chat".into(), "succeeded".into())]);
    let calls = s.calls();
    assert!(calls[1]["prompt"].as_str().unwrap().contains("second") && calls[1]["prompt"].as_str().unwrap().contains("third"), "{}", calls[1]["prompt"]);
    let ids: Vec<&str> = got.iter().map(|m| m["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&q.as_str()) && ids.contains(&q3.as_str()), "chief_reply shows your queued messages once they go: {got:?}");
    assert_eq!(said(&got), pairs(&[("Chief", "Sure, on it."), ("Chief", "Here is the overview."), ("you", "second"), ("you", "third"),
                                  ("Chief", "Sure, on it."), ("Chief", "Here is the overview.")]));
}

// ---- the Chief's turn keeps its normal rules ----

#[tokio::test]
async fn the_chiefs_turn_from_claude_desktop_acts_at_once_and_only_its_own_outside_tool_makes_calls_wait_for_your_approve() {
    let s = served(FAKE);
    let mut c = s.conn().await;
    let (thread, m) = c.ask_chief(json!({"message": "create task Made by the Chief in KADE"})).await;
    let (got, last) = c.reply_until_done(&thread, &m).await;
    assert_eq!(last["status"], "answered", "{last}");
    assert_eq!(said(&got).last().unwrap(), &("Chief".to_string(), "Done: KADE-2.".to_string()));
    let made = tasks::get(&s.st.db, &tasks::id_of(&s.st.db, "KADE-2").unwrap()).unwrap();
    assert_eq!((made.title.as_str(), made.hold.as_deref()), ("Made by the Chief", None), "made at once");
    assert_eq!(tasks::activity(&s.st.db, &made.id).unwrap()[0].actor_name.as_deref(), Some("Team Lead"), "by the Chief, as in any chat");
    assert_eq!(count(&s.st, "SELECT count(*) FROM pending_actions"), 0, "no pending action");
    assert_eq!(chat::outside_tool(&s.st.db, &thread).unwrap(), None, "the chat isn't marked: Claude Desktop isn't a tool from outside Hersir");
    // a call that runs at once in any chat runs at once here too
    settings::set(&s.st.db, "claude_bin", &FAKE_OUTSIDE.to_string()).unwrap();
    let (_, m) = c.ask_chief(json!({"message": "CALL comment_on_task {\"task\": \"KADE-1\", \"body_md\": \"Before otus\"}"})).await;
    let (_, last) = c.reply_until_done(&thread, &m).await;
    assert_eq!(last["status"], "answered", "{last}");
    let kade1 = tasks::id_of(&s.st.db, "KADE-1").unwrap();
    let comments = |st: &AppState| gizai_core::comments::list(&st.db, &kade1).unwrap().into_iter().map(|c| c.body_md).collect::<Vec<_>>();
    assert_eq!(comments(&s.st), ["Before otus"]);
    assert_eq!(chat::outside_tool(&s.st.db, &thread).unwrap(), None);
    // the Chief itself uses a tool from outside Hersir: GA-102's rules, as in any chat
    let (_, m) = c.ask_chief(json!({"message": "OUTSIDE CALL comment_on_task {\"task\": \"KADE-1\", \"body_md\": \"After otus\"}"})).await;
    let (got, last) = c.reply_until_done(&thread, &m).await;
    assert_eq!(last["status"], "waiting for you", "{last}");
    assert!(said(&got).iter().any(|(from, t)| from == "Chief" && t.starts_with("It was refused.")), "{got:?}");
    assert_eq!(chat::outside_tool(&s.st.db, &thread).unwrap().as_deref(), Some("mcp__otus__search"));
    assert_eq!(comments(&s.st), ["Before otus"], "the comment waits for your Approve");
    let kept = pending::list(&s.st, &thread).unwrap();
    assert_eq!(kept.iter().map(|p| (p.action.tool.as_str(), p.action.status.as_str())).collect::<Vec<_>>(), [("comment_on_task", "pending")]);
    // asked again, still waiting for you, with nothing new; once you discard it in Hersir: answered
    let r = c.reply_now(&thread, None).await;
    assert_eq!(r["status"], "waiting for you", "{r}");
    pending::discard(&s.st, &kept[0].action.id).unwrap();
    assert_eq!(c.reply_now(&thread, None).await["status"], "answered");
}

// ---- chief_reply ----

#[tokio::test]
async fn chief_reply_says_working_with_the_draft_while_the_turn_runs_and_waits_no_longer_than_wait_seconds() {
    let s = served(FAKE);
    let mut c = s.conn().await;
    let (thread, m) = c.ask_chief(json!({"message": "FAKE_CHAT_HANG"})).await;
    s.draft_is(&thread, "Thinking about ").await;
    let r = c.reply_now(&thread, None).await;
    assert_eq!((&r["status"], &r["draft"]), (&json!("working"), &json!("Thinking about ")), "{r}");
    assert_eq!(said(r["messages"].as_array().unwrap()), pairs(&[("you", "FAKE_CHAT_HANG")]));
    assert_eq!(r["messages"][0]["id"], m.as_str());
    assert_eq!(r["thread"], thread.as_str());
    // nothing new: it waits wait_seconds, and no longer
    let (r, took) = c.ok_within("chief_reply", json!({"thread": thread, "after": m, "wait_seconds": 2}), WAIT).await;
    assert!(took >= Duration::from_millis(1900) && took < Duration::from_secs(4), "took {took:?}");
    assert_eq!((&r["status"], &r["messages"], &r["draft"]), (&json!("working"), &json!([]), &json!("Thinking about ")), "{r}");
    // wait_seconds left out (60): it returns as soon as the turn ends, here by your Stop in Hersir
    let mut c2 = s.conn().await;
    let (thread2, after) = (thread.clone(), m.clone());
    let waiting = tokio::spawn(async move {
        c2.ok_within("chief_reply", json!({"thread": thread2, "after": after}), Duration::from_secs(70)).await
    });
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(!waiting.is_finished(), "it waits while the Chief works");
    let stopped = Instant::now();
    app_chat::stop(&s.st, &thread);
    let (r, _) = waiting.await.unwrap();
    assert!(stopped.elapsed() < Duration::from_secs(5), "returned {:?} after the Stop", stopped.elapsed());
    assert!(!r["messages"].as_array().unwrap().is_empty(), "{r}");
    s.idle(&thread).await;
    let r = c.reply_now(&thread, Some(&m)).await;
    assert_eq!(r["status"], "answered", "{r}");
    assert!(r.get("draft").is_none(), "no draft once it answered: {r}");
    assert_eq!(said(r["messages"].as_array().unwrap()), pairs(&[("Chief", "Thinking about"), ("Hersir", "Stopped.")]));
    // answered: even with wait_seconds it answers at once
    let (r, took) = c.ok_within("chief_reply", json!({"thread": thread, "wait_seconds": 120}), WAIT).await;
    assert!(took < Duration::from_secs(2), "took {took:?}");
    assert_eq!(r["status"], "answered");
}

#[tokio::test]
async fn chief_reply_returns_as_soon_as_the_turn_ends_with_the_chiefs_new_messages() {
    let s = served(FAKE);
    let mut c = s.conn().await;
    let (thread, m) = c.ask_chief(json!({"message": "FAKE_CHAT_WAIT How are things?"})).await;
    s.fake_waits().await;
    let mut c2 = s.conn().await;
    let (thread2, after) = (thread.clone(), m.clone());
    let waiting = tokio::spawn(async move {
        c2.ok_within("chief_reply", json!({"thread": thread2, "after": after, "wait_seconds": 30}), Duration::from_secs(40)).await
    });
    tokio::time::sleep(Duration::from_millis(1000)).await;
    assert!(!waiting.is_finished(), "it waits while the Chief works");
    let went = Instant::now();
    s.go();
    let (first, _) = waiting.await.unwrap();
    assert!(went.elapsed() < Duration::from_secs(5), "returned {:?} after the Chief went on, not after 30 s", went.elapsed());
    assert!(!first["messages"].as_array().unwrap().is_empty(), "{first}");
    let after = first["messages"].as_array().unwrap().last().unwrap()["id"].as_str().unwrap().to_string();
    let (mut got, last) = if first["status"] == "working" { c.reply_until_done(&thread, &after).await } else { (vec![], first.clone()) };
    assert!(went.elapsed() < Duration::from_secs(10), "{:?}", went.elapsed());
    got.splice(0..0, first["messages"].as_array().unwrap().iter().cloned());
    assert_eq!(last["status"], "answered", "{last}");
    assert_eq!(said(&got), pairs(&[("Chief", "Sure, on it."), ("Chief", "Here is the overview.")]), "the new messages, no tool call");
    for msg in &got {
        let keys: Vec<&String> = msg.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["from", "id", "text", "time"], "{msg}");
    }
}

#[tokio::test]
async fn chief_reply_shows_the_messages_after_after_at_most_the_last_20_from_you_the_chief_and_hersir_with_their_time() {
    let s = served(FAKE);
    let thread = chat::create_thread(&s.st.db, &s.st.you_id, &s.lead, "Long chat").unwrap();
    let mut ids = vec![];
    for i in 0..30 {
        let (role, author) = match i % 4 {
            0 => ("user", Some(s.st.you_id.clone())),
            1 => ("agent", Some(s.lead.clone())),
            2 => ("tool", Some(s.lead.clone())),
            _ => ("system", None),
        };
        let m = chat::add_message(&s.st.db, chat::NewMessage { thread_id: thread.clone(), role: role.into(), author_id: author,
            body_md: Some(format!("message {i}")), tool_name: (role == "tool").then(|| "mcp__gizai__get_overview".to_string()),
            ..Default::default() }).unwrap();
        ids.push(m.id);
    }
    // a known time: 2026-11-23 14:05:09 UTC
    s.st.db.write(None, |w| { w.conn().execute("UPDATE chat_messages SET created_at=1795442709123 WHERE id=?1", [&ids[29]])?; Ok(()) }).unwrap();
    let mut c = s.conn().await;
    let r = c.reply_now(&thread, None).await;
    assert_eq!(r["status"], "answered", "{r}");
    let shown = r["messages"].as_array().unwrap();
    let want: Vec<usize> = (0..30).filter(|i| i % 4 != 2).collect();
    let want = &want[want.len() - 20..];
    assert_eq!(shown.iter().map(|m| m["id"].as_str().unwrap()).collect::<Vec<_>>(), want.iter().map(|i| ids[*i].as_str()).collect::<Vec<_>>(),
               "the last 20, no tool calls");
    let from: Vec<&str> = want.iter().map(|i| ["you", "Chief", "", "Hersir"][i % 4]).collect();
    assert_eq!(shown.iter().map(|m| m["from"].as_str().unwrap()).collect::<Vec<_>>(), from);
    assert_eq!(shown.last().unwrap()["time"], "2026-11-23T14:05:09Z");
    assert_eq!(shown.last().unwrap()["text"], "message 29");
    // after a message: only the ones after it
    let r = c.reply_now(&thread, Some(&ids[24])).await;
    assert_eq!(said(r["messages"].as_array().unwrap()), pairs(&[("Chief", "message 25"), ("Hersir", "message 27"), ("you", "message 28"),
                                                                  ("Chief", "message 29")]));
    // after a tool call: the ones after it too
    assert_eq!(c.reply_now(&thread, Some(&ids[26])).await["messages"].as_array().unwrap().len(), 3);
    // after the last: none
    assert_eq!(c.reply_now(&thread, Some(&ids[29])).await["messages"], json!([]));
    // after an early one: still at most the last 20
    assert_eq!(c.reply_now(&thread, Some(&ids[0])).await["messages"].as_array().unwrap().len(), 20);
}

// ---- links and long texts ----

#[tokio::test]
async fn gizai_links_come_back_as_their_text_and_texts_over_8000_characters_are_cut() {
    let s = served(FAKE);
    let mut c = s.conn().await;
    let linked = "See [KADE-1](gizai:task/KADE-1) and [the \\[spec\\]](gizai:doc/abc), not [a site](https://example.com).";
    let (thread, m1) = c.ask_chief(json!({"message": linked})).await;
    s.idle(&thread).await;
    let long: String = (0..9000).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
    let (_, m2) = c.ask_chief(json!({"message": long})).await;
    s.idle(&thread).await;
    let exact: String = "é".repeat(8000);
    let (_, m3) = c.ask_chief(json!({"message": exact})).await;
    s.idle(&thread).await;
    let over: String = "é".repeat(8001);
    let (_, m4) = c.ask_chief(json!({"message": over})).await;
    s.idle(&thread).await;
    // a Chief's answer with links, and a long one
    for body in ["Made [KADE-2](gizai:task/KADE-2) for [Kade portal](gizai:project/KADE).".to_string(), format!("{long}{long}")] {
        chat::add_message(&s.st.db, chat::NewMessage { thread_id: thread.clone(), role: "agent".into(), author_id: Some(s.lead.clone()),
            body_md: Some(body), ..Default::default() }).unwrap();
    }
    let r = c.reply_now(&thread, None).await;
    let shown = r["messages"].as_array().unwrap();
    let by_id = |id: &str| shown.iter().find(|x| x["id"] == id).unwrap_or_else(|| panic!("{id} in {r}"))["text"].as_str().unwrap().to_string();
    assert_eq!(by_id(&m1), "See KADE-1 and the [spec], not [a site](https://example.com).");
    let cut_long = by_id(&m2);
    assert_eq!(cut_long.chars().count(), 8000);
    assert!(cut_long.ends_with(CUT_TEXT), "{}", &cut_long[cut_long.len() - 60..]);
    assert_eq!(cut_long, format!("{}{CUT_TEXT}", &long[..8000 - CUT_TEXT.chars().count()]));
    assert_eq!(by_id(&m3), exact, "8,000 characters aren't cut");
    let cut_over = by_id(&m4);
    assert_eq!(cut_over.chars().count(), 8000);
    assert_eq!(cut_over, format!("{}{CUT_TEXT}", "é".repeat(8000 - CUT_TEXT.chars().count())));
    let chiefs: Vec<String> = shown.iter().rev().take(2).map(|x| x["text"].as_str().unwrap().to_string()).collect();
    assert_eq!(chiefs[1], "Made KADE-2 for Kade portal.");
    assert_eq!(chiefs[0].chars().count(), 8000);
    assert!(chiefs[0].ends_with(CUT_TEXT));
    // Hersir keeps them as they are
    let saved = chat::messages(&s.st.db, &thread).unwrap();
    assert_eq!(saved.iter().find(|x| x.id == m1).unwrap().body_md.as_deref(), Some(linked));
    assert_eq!(saved.iter().find(|x| x.id == m2).unwrap().body_md.as_deref().map(str::len), Some(9000));
}

#[tokio::test]
async fn the_draft_is_cut_at_8000_characters_while_the_chief_writes_a_long_answer() {
    let s = served(FAKE);
    let mut c = s.conn().await;
    let (thread, _) = c.ask_chief(json!({"message": "FAKE_CHAT_STREAM"})).await;
    let mut longest = 0;
    let mut seen_cut = false;
    for _ in 0..400 {
        let r = c.reply_now(&thread, None).await;
        if r["status"] != "working" {
            break;
        }
        let d = r["draft"].as_str().unwrap_or_else(|| panic!("a draft while working: {r}"));
        let n = d.chars().count();
        assert!(n <= 8000, "{n}");
        longest = longest.max(n);
        if d.ends_with(CUT_TEXT) {
            assert_eq!(n, 8000);
            seen_cut = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(seen_cut, "the 20 KB draft came back cut (longest {longest})");
    app_chat::stop(&s.st, &thread);
    s.idle(&thread).await;
}

#[test]
fn plain_and_cut_follow_the_card() {
    assert_eq!(CUT, CUT_TEXT);
    assert_eq!(MAX_TEXT, 8000);
    for (text, want) in [
        ("[KADE-1](gizai:task/KADE-1)", "KADE-1"),
        ("a [x](gizai:agent/abc) b [y](gizai:person/def)", "a x b y"),
        ("[a \\] b](gizai:task/X)", "a ] b"),
        ("[C:\\path](gizai:doc/X)", "C:\\path"),
        ("[[KADE-1](gizai:task/KADE-1)]", "[KADE-1]"),
        // not a gizai: link, or not a link: as it is
        ("[x](https://example.com)", "[x](https://example.com)"),
        ("[](gizai:task/X)", "[](gizai:task/X)"),
        ("[two\nlines](gizai:task/X)", "[two\nlines](gizai:task/X)"),
        ("[x](gizai:task/a b)", "[x](gizai:task/a b)"),
        ("[x] (gizai:task/X)", "[x] (gizai:task/X)"),
        ("[x](gizai:task/X", "[x](gizai:task/X"),
        ("a [ b", "a [ b"),
        ("é [ü](gizai:task/É) ñ", "é ü ñ"),
        ("", ""),
    ] {
        assert_eq!(plain(text), want, "{text:?}");
    }
    assert_eq!(cut("short"), "short");
    let max = "x".repeat(8000);
    assert_eq!(cut(&max), max);
    let over = format!("{max}y");
    let c = cut(&over);
    assert_eq!(c.chars().count(), 8000);
    assert!(c.ends_with(CUT_TEXT) && c.starts_with("xxx"));
    let emoji = "🦀".repeat(9000);
    assert_eq!(cut(&emoji).chars().count(), 8000);
}

// ---- unknown threads and messages ----

#[tokio::test]
async fn with_an_unknown_thread_ask_chief_and_chief_reply_say_so_and_change_nothing() {
    let s = served(FAKE);
    let mut c = s.conn().await;
    let gone = chat::create_thread(&s.st.db, &s.st.you_id, &s.lead, "Deleted chat").unwrap();
    s.st.db.write(None, |w| { w.conn().execute("UPDATE chat_threads SET deleted_at=1 WHERE id=?1", [&gone])?; Ok(()) }).unwrap();
    let before = dump(&s.st);
    for thread in ["nope", gone.as_str()] {
        let e = c.refused("ask_chief", json!({"message": "Hello", "thread": thread})).await;
        assert!(e.contains(&format!("There is no chat {thread} in Hersir")), "{e}");
        let e = c.refused("chief_reply", json!({"thread": thread})).await;
        assert!(e.contains(&format!("There is no chat {thread} in Hersir")), "{e}");
        let e = c.refused("chief_reply", json!({"thread": thread, "after": "x", "wait_seconds": 5})).await;
        assert!(e.contains(&format!("There is no chat {thread} in Hersir")), "{e}");
    }
    // no message, or no thread for chief_reply
    for (name, args) in [("ask_chief", json!({})), ("ask_chief", json!({"message": "   "})), ("chief_reply", json!({}))] {
        c.refused(name, args).await;
    }
    assert_eq!(dump(&s.st), before, "no chat, no message, no turn");
    assert!(app_chat::live(&s.st).is_empty());
    // a message that isn't in the chat
    let (thread, _) = c.ask_chief(json!({"message": "Hello"})).await;
    s.idle(&thread).await;
    let before = dump(&s.st);
    let e = c.refused("chief_reply", json!({"thread": thread, "after": "nope"})).await;
    assert!(e.contains("There is no message nope in chat"), "{e}");
    assert_eq!(dump(&s.st), before);
}

#[tokio::test]
async fn ask_chief_without_a_chief_says_why_and_makes_no_empty_chat() {
    let s = served(FAKE);
    s.st.db.write(None, |w| { w.conn().execute("UPDATE agent_configs SET chat_enabled=0", [])?; Ok(()) }).unwrap();
    let mut c = s.conn().await;
    let before = dump(&s.st);
    let e = c.refused("ask_chief", json!({"message": "Anyone there?"})).await;
    assert!(!e.is_empty());
    assert_eq!(dump(&s.st), before, "no chat From Claude Desktop left behind");
    assert_eq!(s.desktop_chats(), 0);
}

#[tokio::test]
async fn a_message_left_waiting_for_send_now_after_a_stop_is_waiting_for_you() {
    let s = served(FAKE);
    let mut c = s.conn().await;
    let (thread, _) = c.ask_chief(json!({"message": "FAKE_CHAT_HANG"})).await;
    s.draft_is(&thread, "Thinking about ").await;
    let (_, q) = c.ask_chief(json!({"message": "queued meanwhile"})).await;
    app_chat::stop(&s.st, &thread);
    s.idle(&thread).await;
    let queue = chat::queue(&s.st.db, &thread).unwrap();
    assert_eq!(queue.iter().map(|x| (x.id.as_str(), x.held)).collect::<Vec<_>>(), [(q.as_str(), true)], "it waits for Send now");
    let r = c.reply_now(&thread, Some(&q)).await;
    assert_eq!((&r["status"], &r["messages"]), (&json!("waiting for you"), &json!([])), "{r}");
}
