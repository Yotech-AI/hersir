// Linux and macOS only: over the Unix socket, and the Chief's turns run Python scripts as a fake Claude Code.
#![cfg(unix)]
//! GA-351 QA: a message from Claude Desktop (ask_chief) brings Claude Desktop's limits into the Chief's turn. Over the
//! socket with the token Connect makes, as the hersir-mcp shim talks; the Chief's turns run a fake Claude Code
//! (fake-claude-chat.py, fake-claude-chat-outside.py) with the real gizai-mcp shim, never the real one.
//! - ask_chief "attach <a file outside the Chief's copies> to KADE-1": the Chief's attach_file is refused with Claude
//!   Desktop's sentence and nothing is attached (a file elsewhere, the linked folder's .env and its tracked file, not the
//!   copy); a file inside the copies is still attached at once.
//! - In such a turn update_checkout and start_chat answer with Claude Desktop's texts and change nothing; the turn's token
//!   says `{"answers": "claude-desktop"}`, and a chat token with it refuses them (and attach_file outside the copies, also
//!   a file you named, also in a chat marked by a tool from outside Hersir) with nothing changed. Without it, they run.
//! - A message you type in the same thread gets the normal rules again: attach_file of a file you named works, and
//!   start_chat runs; a path that only Claude Desktop's message named is never one you named.
//! - A message from Claude Desktop that waited in the queue keeps its mark (also after Send now), and its turn has Claude
//!   Desktop's limits, also together with one you typed; your message that waited behind Claude Desktop's turn doesn't.
//! - The message from Claude Desktop is marked `{"from": "claude-desktop"}` in its meta (what Hersir's chat shows); yours isn't.
use std::path::{Path, PathBuf};
use std::time::Duration;

use gizai_core::model::*;
use gizai_core::{chat, projects, settings, tasks, team, tokens};
use gizai_lib::claude_desktop as desktop;
use gizai_lib::{AppState, chat as app_chat, mcp, tools};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

const FAKE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude-chat.py");
const FAKE_OUTSIDE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude-chat-outside.py");
const WAIT: Duration = Duration::from_secs(10);

// Claude Desktop's texts (GA-341), word for word.
const START_CHAT: &str = "start_chat isn't available in Claude Desktop: ask the user here, in this chat.";
const UPDATE_CHECKOUT: &str = "update_checkout isn't available in Claude Desktop: it works only in Hersir's chat, where its result comes as a message.";

/// attach_file's refusal from Claude Desktop for a file outside the Chief's copies (GA-341's sentence).
fn not_in_copies(raw: &str) -> String {
    format!("From Claude Desktop, attach_file takes only a file inside the Chief's copies of the projects' code; {raw} isn't one. Nothing \
             was attached: the user can attach it themselves in Hersir.")
}

/// attach_file's refusal in a chat for a file nobody named there (Spec §8).
fn not_named(raw: &str) -> String {
    format!("I can only attach a file you named in this chat, one you added to a message here, or one inside my copies of the projects' \
             code; {raw} is none of these. Ask the user to give the path.")
}

/// start_chat's arguments that work in a turn with the normal rules.
fn start_chat_args() -> Value {
    json!({"title": "KADE-1: start it?", "kind": "question", "body_md": "Shall I start KADE-1?", "tasks": ["KADE-1"]})
}

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

fn git(dir: &Path, args: &[&str]) -> String {
    let o = std::process::Command::new("git").args(["-c", "user.email=t@t", "-c", "user.name=t"]).args(args).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
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
        c.ask(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}})).await;
        c.w.write_all(format!("{}\n", json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).as_bytes()).await.unwrap();
        c
    }
    async fn ask(&mut self, msg: Value) -> Value {
        self.w.write_all(format!("{msg}\n").as_bytes()).await.unwrap();
        match tokio::time::timeout(WAIT, self.lines.next_line()).await {
            Ok(Ok(Some(l))) => serde_json::from_str(&l).unwrap(),
            Ok(_) => panic!("Hersir closed the connection"),
            Err(_) => panic!("Hersir didn't answer within {WAIT:?}"),
        }
    }
    async fn call(&mut self, name: &str, args: Value) -> Value {
        self.id += 1;
        let id = self.id;
        let r = self.ask(json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": args}})).await;
        assert_eq!(r["id"], id, "{r}");
        r["result"].clone()
    }
    /// A call that must work: its answer, parsed.
    async fn ok(&mut self, name: &str, args: Value) -> Value {
        let r = self.call(name, args.clone()).await;
        assert_eq!(r["isError"], false, "{name} {args}: {r}");
        serde_json::from_str(&text(&r)).unwrap_or_else(|_| panic!("{name}: not JSON: {r}"))
    }
    /// A call that must be refused: its text.
    async fn refused(&mut self, name: &str, args: Value) -> String {
        let r = self.call(name, args.clone()).await;
        assert_eq!(r["isError"], true, "{name} {args}: {r}");
        text(&r)
    }
    /// ask_chief: (thread, message).
    async fn ask_chief(&mut self, args: Value) -> (String, String) {
        let r = self.ok("ask_chief", args).await;
        assert_eq!(r["status"], "working", "{r}");
        (r["thread"].as_str().unwrap().to_string(), r["message"].as_str().unwrap().to_string())
    }
}

fn text(result: &Value) -> String {
    result["content"][0]["text"].as_str().unwrap_or_default().to_string()
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
    repo: PathBuf,
    /// A file outside the Chief's copies of the code, like a key in ~/.ssh.
    secret: PathBuf,
    /// Another file outside the copies, one you name yourself in Hersir's chat.
    brief: PathBuf,
    _tmp: tempfile::TempDir,
    _server: tokio::task::JoinHandle<()>,
}

/// Hersir with its socket served and Claude Desktop connected: the Chief (Team Lead, Chat on), project KADE linked to a git
/// repository (docs/spec.md tracked, .env not) with KADE-1 "Export" in Backlog, the queue paused, and the Chief's copies of
/// the code made. The Chief's Claude Code is fake-claude-chat.py with the real gizai-mcp shim.
async fn served() -> S {
    let tmp = tempfile::tempdir().unwrap();
    let mut st = gizai_lib::test_state(tmp.path());
    st.mcp_shim = Some(shim());
    settings::set(&st.db, "claude_bin", &FAKE.to_string()).unwrap();
    settings::set(&st.db, "agents_paused", &true).unwrap();
    let team_id = team::list(&st.db).unwrap()[0].id.clone();
    let lead = team::add_agent(&st.db, &st.you_id, &team_id, AgentInput { name: "Team Lead".into(), role_key: "lead".into(),
        chat_enabled: Some(true), ..Default::default() }).unwrap();
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
    tasks::create(&st.db, &st.you_id, TaskInput { project_id: project, title: "Export".into(), ..Default::default() }).unwrap();
    let secret = tmp.path().join("ssh/id_test");
    std::fs::create_dir_all(secret.parent().unwrap()).unwrap();
    std::fs::write(&secret, "-----BEGIN TEST KEY-----").unwrap();
    let brief = tmp.path().join("brief.txt");
    std::fs::write(&brief, "the brief").unwrap();
    let server = mcp::start(&st).unwrap();
    gizai_lib::code::startup(&st).await;
    assert!(st.data_dir.join("code/KADE/docs/spec.md").is_file(), "the Chief's copy of KADE");
    let c = desktop::connect(&st, false).unwrap();
    let entry: Value = serde_json::from_str(&c.entry).unwrap();
    let token = entry["mcpServers"]["hersir"]["env"]["HERSIR_TOKEN"].as_str().unwrap().to_string();
    S { st, lead, token, repo, secret, brief, _tmp: tmp, _server: server }
}

impl S {
    async fn conn(&self) -> Conn {
        Conn::open(&self.st, &self.token).await
    }
    /// The Chief's copy of KADE's docs/spec.md.
    fn copy_spec(&self) -> PathBuf {
        self.st.data_dir.join("code/KADE/docs/spec.md")
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
    /// Waits until the fake waits for fake-go (FAKE_CHAT_WAIT); the files of an earlier wait are gone first (`ready_to_wait`).
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
    fn ready_to_wait(&self) {
        let _ = std::fs::remove_file(self.st.data_dir.join("chat/fake-waiting"));
        let _ = std::fs::remove_file(self.st.data_dir.join("chat/fake-go"));
    }
    fn go(&self) {
        std::fs::write(self.st.data_dir.join("chat/fake-go"), "").unwrap();
    }
    /// The results of the Chief's calls of `tool` in `thread`, oldest first: (isError, text).
    fn results(&self, thread: &str, tool: &str) -> Vec<(bool, String)> {
        chat::messages(&self.st.db, thread).unwrap().into_iter()
            .filter(|m| m.role == "tool" && m.tool_name.as_deref().is_some_and(|n| n == tool || n.ends_with(&format!("__{tool}"))))
            .map(|m| {
                let t = m.tool.unwrap_or_default();
                (t["isError"].as_bool().unwrap_or(false), t["result"].as_str().unwrap_or_default().to_string())
            }).collect()
    }
    /// The last result of the Chief's call of `tool` in `thread`.
    fn last_result(&self, thread: &str, tool: &str) -> (bool, String) {
        let all = self.results(thread, tool);
        all.last().cloned().unwrap_or_else(|| panic!("no {tool} call in the chat: {:?}", chat::messages(&self.st.db, thread).unwrap()))
    }
    /// The scopes of the tokens of the Chief's turns in `thread`, oldest first.
    fn turn_scopes(&self, thread: &str) -> Vec<Value> {
        let all: Vec<String> = self.st.db.read(|c| {
            let mut q = c.prepare("SELECT scopes_json FROM api_tokens ORDER BY created_at, rowid")?;
            let rows = q.query_map([], |r| r.get(0))?.collect::<Result<Vec<String>, _>>()?;
            Ok(rows)
        }).unwrap();
        all.iter().map(|s| serde_json::from_str::<Value>(s).unwrap())
            .filter(|v| v["chat"] == json!(thread) && v.get("run").is_some()).collect()
    }
    /// What each turn's token in `thread` says it answers, oldest first.
    fn turns_answer(&self, thread: &str) -> Vec<Option<String>> {
        self.turn_scopes(thread).iter().map(|v| v.get("answers").and_then(Value::as_str).map(str::to_string)).collect()
    }
    fn attached(&self) -> Vec<String> {
        self.st.db.read(|c| {
            let mut q = c.prepare("SELECT name FROM files ORDER BY created_at, rowid")?;
            let rows = q.query_map([], |r| r.get(0))?.collect::<Result<Vec<String>, _>>()?;
            Ok(rows)
        }).unwrap()
    }
    fn pending(&self) -> i64 {
        count(&self.st, "SELECT count(*) FROM pending_actions")
    }
    fn chats(&self) -> i64 {
        count(&self.st, "SELECT count(*) FROM chat_threads WHERE deleted_at IS NULL")
    }
    fn message(&self, thread: &str, id: &str) -> chat::ChatMessage {
        chat::messages(&self.st.db, thread).unwrap().into_iter().find(|m| m.id == id).unwrap_or_else(|| panic!("message {id}"))
    }
    /// You type `text` in Hersir's chat `thread` and send it: (thread, message id) once its answer is done (or it was queued).
    async fn type_in(&self, thread: Option<&str>, text: &str) -> (String, String) {
        let sent = app_chat::send_message(&self.st, thread.map(str::to_string), text.into(), None, vec![], None).await.unwrap();
        tokio::time::timeout(Duration::from_secs(30), sent.done).await.expect("the answer ended").unwrap();
        (sent.thread_id, sent.message_id)
    }
}

/// The message Claude Desktop sends to have a file attached: the fake (FAKE_ATTACH_TO) attaches the path on the "- name: /path" line.
fn attach_message(path: &Path) -> String {
    format!("FAKE_ATTACH_TO KADE-1 Attach this file to KADE-1, please.\n- file: {}", path.display())
}

// ---- attach_file ----

#[tokio::test]
async fn ask_chief_to_attach_a_file_outside_the_copies_is_refused_with_claude_desktops_sentence_and_nothing_is_attached() {
    let s = served().await;
    let mut c = s.conn().await;
    let mut thread = String::new();
    // a key elsewhere on the disk, the linked folder's .env (not tracked), and its tracked file in the linked folder, not the copy
    for f in [s.secret.clone(), s.repo.join(".env"), s.repo.join("docs/spec.md")] {
        let raw = f.display().to_string();
        let (t, m) = c.ask_chief(json!({"message": attach_message(&f)})).await;
        thread = t;
        s.idle(&thread).await;
        assert_eq!(s.last_result(&thread, "attach_file"), (true, not_in_copies(&raw)), "{raw}");
        // the message is yours, marked as from Claude Desktop
        let msg = s.message(&thread, &m);
        assert_eq!((msg.role.as_str(), msg.author_id.as_deref()), ("user", Some(s.st.you_id.as_str())));
        assert_eq!(msg.meta, Some(json!({"from": "claude-desktop"})));
        assert!(msg.from_desktop());
    }
    assert_eq!(s.results(&thread, "attach_file").len(), 3, "one call per message");
    assert!(s.attached().is_empty(), "nothing attached: {:?}", s.attached());
    assert_eq!(s.pending(), 0, "refused at once, not kept for your Approve");
    let kade1 = tasks::get(&s.st.db, &tasks::id_of(&s.st.db, "KADE-1").unwrap()).unwrap();
    assert!(kade1.hold.is_none(), "KADE-1 unchanged");
    assert_eq!(s.turns_answer(&thread), [Some("claude-desktop".to_string()), Some("claude-desktop".into()), Some("claude-desktop".into())],
               "every turn's token says it answers Claude Desktop");
}

#[tokio::test]
async fn from_such_a_turn_a_file_inside_the_chiefs_copies_is_still_attached_at_once() {
    let s = served().await;
    let mut c = s.conn().await;
    let (thread, _) = c.ask_chief(json!({"message": attach_message(&s.copy_spec())})).await;
    s.idle(&thread).await;
    let (is_error, result) = s.last_result(&thread, "attach_file");
    assert!(!is_error, "{result}");
    assert_eq!(s.attached(), ["spec.md"]);
    let on: String = s.st.db.read(|db| Ok(db.query_row("SELECT owner_id FROM files WHERE name='spec.md'", [], |r| r.get(0))?)).unwrap();
    assert_eq!(on, tasks::id_of(&s.st.db, "KADE-1").unwrap(), "on KADE-1");
    assert_eq!(s.pending(), 0, "at once");
    assert_eq!(s.turns_answer(&thread), [Some("claude-desktop".to_string())]);
}

// ---- update_checkout and start_chat ----

#[tokio::test]
async fn in_such_a_turn_update_checkout_and_start_chat_answer_with_claude_desktops_texts_and_change_nothing() {
    let s = served().await;
    settings::set(&s.st.db, "claude_bin", &FAKE_OUTSIDE.to_string()).unwrap();
    let head = git(&s.repo, &["rev-parse", "HEAD"]);
    let mut c = s.conn().await;
    let (thread, _) = c.ask_chief(json!({"message": "CALL update_checkout {\"project\": \"KADE\"}"})).await;
    s.idle(&thread).await;
    assert_eq!(s.last_result(&thread, "update_checkout"), (true, UPDATE_CHECKOUT.to_string()));
    let (_, _) = c.ask_chief(json!({"message": format!("CALL start_chat {}", start_chat_args()), "thread": thread})).await;
    s.idle(&thread).await;
    assert_eq!(s.last_result(&thread, "start_chat"), (true, START_CHAT.to_string()));
    // nothing changed: no other chat, no call kept for your Approve, no update of the linked folder (no note about it)
    assert_eq!(s.chats(), 1, "only the chat From Claude Desktop");
    assert_eq!(s.pending(), 0);
    assert_eq!(git(&s.repo, &["rev-parse", "HEAD"]), head);
    let notes: Vec<String> = chat::messages(&s.st.db, &thread).unwrap().into_iter().filter(|m| m.role == "system")
        .map(|m| m.body_md.unwrap_or_default()).collect();
    assert!(notes.is_empty(), "no note from an update: {notes:?}");
    assert_eq!(s.turns_answer(&thread), [Some("claude-desktop".to_string()), Some("claude-desktop".into())]);
}

#[tokio::test]
async fn a_chat_token_that_answers_claude_desktop_refuses_what_claude_desktop_may_not_do_and_changes_nothing() {
    let s = served().await;
    let thread = chat::create_thread(&s.st.db, &s.st.you_id, &s.lead, "Asked from Claude Desktop").unwrap();
    // a file you named by typing in this chat: in an answer to Claude Desktop only the copies count
    chat::add_message(&s.st.db, chat::NewMessage { thread_id: thread.clone(), role: "user".into(), author_id: Some(s.st.you_id.clone()),
        body_md: Some(format!("Attach {} to KADE-1", s.brief.display())), ..Default::default() }).unwrap();
    let token = tokens::mint(&s.st.db, &s.lead, json!({"chat": thread, "answers": "claude-desktop"}), 60_000).unwrap();
    let mut c = Conn::open(&s.st, &token).await;
    c.ok("get_overview", json!({})).await;
    let before = dump(&s.st);
    for args in [start_chat_args(), json!({})] {
        assert_eq!(c.refused("start_chat", args.clone()).await, START_CHAT, "{args}");
    }
    for args in [json!({"project": "KADE"}), json!({})] {
        assert_eq!(c.refused("update_checkout", args.clone()).await, UPDATE_CHECKOUT, "{args}");
    }
    for f in [s.brief.clone(), s.secret.clone(), s.repo.join(".env")] {
        let raw = f.display().to_string();
        assert_eq!(c.refused("attach_file", json!({"path": raw, "task": "KADE-1"})).await, not_in_copies(&raw), "{raw}");
    }
    assert_eq!(dump(&s.st), before, "nothing changed");
    // a file inside the copies is attached at once
    let r = c.ok("attach_file", json!({"path": s.copy_spec().display().to_string(), "task": "KADE-1"})).await;
    assert_eq!(r["file"]["name"], "spec.md", "{r}");
    // in a chat that used a tool from outside Hersir: refused at once all the same, not kept for your Approve
    app_chat::mark_outside(&s.st, &thread, "mcp__otus__search");
    let before = dump(&s.st);
    assert_eq!(c.refused("start_chat", start_chat_args()).await, START_CHAT);
    assert_eq!(c.refused("update_checkout", json!({"project": "KADE"})).await, UPDATE_CHECKOUT);
    let raw = s.secret.display().to_string();
    assert_eq!(c.refused("attach_file", json!({"path": raw, "task": "KADE-1"})).await, not_in_copies(&raw));
    assert_eq!(dump(&s.st), before, "nothing kept, nothing changed");
    assert_eq!(s.pending(), 0);
    // while a file inside the copies waits for your Approve there, as in any marked chat (GA-102)
    let kept = c.refused("attach_file", json!({"path": s.copy_spec().display().to_string(), "task": "KADE-1"})).await;
    assert!(kept.starts_with("attach_file wasn't run: this chat used mcp__otus__search"), "{kept}");
    assert_eq!(s.pending(), 1);

    // the same chat's token for your own message: the normal rules, the file you named is attached and start_chat runs
    let thread = chat::create_thread(&s.st.db, &s.st.you_id, &s.lead, "Typed in Hersir").unwrap();
    chat::add_message(&s.st.db, chat::NewMessage { thread_id: thread.clone(), role: "user".into(), author_id: Some(s.st.you_id.clone()),
        body_md: Some(format!("Attach {} to KADE-1", s.brief.display())), ..Default::default() }).unwrap();
    let token = tokens::mint(&s.st.db, &s.lead, json!({"chat": thread}), 60_000).unwrap();
    let mut c = Conn::open(&s.st, &token).await;
    let r = c.ok("attach_file", json!({"path": s.brief.display().to_string(), "task": "KADE-1"})).await;
    assert_eq!(r["file"]["name"], "brief.txt", "{r}");
    let chats = s.chats();
    let r = c.ok("start_chat", start_chat_args()).await;
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(s.chats(), chats + 1);
}

// ---- your own message gets the normal rules again ----

#[tokio::test]
async fn a_message_you_type_in_the_same_thread_gets_the_normal_rules_again() {
    let s = served().await;
    let mut c = s.conn().await;
    let secret = s.secret.display().to_string();
    let (thread, m) = c.ask_chief(json!({"message": attach_message(&s.secret)})).await;
    s.idle(&thread).await;
    assert_eq!(s.last_result(&thread, "attach_file"), (true, not_in_copies(&secret)));
    // you type in Hersir's chat in the same thread, naming a file yourself: attached, as before
    let (same, yours) = s.type_in(Some(&thread), &format!("FAKE_ATTACH_TO KADE-1 Yes, and attach my brief.\n- brief: {}", s.brief.display())).await;
    assert_eq!(same, thread);
    let (is_error, result) = s.last_result(&thread, "attach_file");
    assert!(!is_error, "{result}");
    assert_eq!(s.attached(), ["brief.txt"]);
    assert_eq!(s.turns_answer(&thread), [Some("claude-desktop".to_string()), None], "your turn's token answers you");
    // only Claude Desktop's message is marked: Hersir's chat shows it as from Claude Desktop, yours not
    assert_eq!(s.message(&thread, &m).meta, Some(json!({"from": "claude-desktop"})));
    let typed = s.message(&thread, &yours);
    assert_eq!((typed.meta.clone(), typed.from_desktop()), (None, false));
    // a path that only Claude Desktop's message named is never one you named, also in your turn
    let e = tools::call_in(&s.st, &s.lead, Some(&thread), "attach_file", json!({"path": secret, "task": "KADE-1"})).await.unwrap_err();
    assert_eq!(e, not_named(&secret));
    assert_eq!(s.attached(), ["brief.txt"]);
    // start_chat in your own turn runs as it always did
    settings::set(&s.st.db, "claude_bin", &FAKE_OUTSIDE.to_string()).unwrap();
    let chats = s.chats();
    s.type_in(Some(&thread), &format!("CALL start_chat {}", start_chat_args())).await;
    let (is_error, result) = s.last_result(&thread, "start_chat");
    assert!(!is_error, "{result}");
    assert_eq!(s.chats(), chats + 1, "the chat was started");
    // and Claude Desktop's next message gets Claude Desktop's limits again
    c.ask_chief(json!({"message": format!("CALL start_chat {}", start_chat_args()), "thread": thread})).await;
    s.idle(&thread).await;
    assert_eq!(s.last_result(&thread, "start_chat"), (true, START_CHAT.to_string()));
    assert_eq!(s.chats(), chats + 1);
    assert_eq!(s.turns_answer(&thread), [Some("claude-desktop".to_string()), None, None, Some("claude-desktop".into())]);
}

// ---- the queue ----

#[tokio::test]
async fn a_message_from_claude_desktop_that_waited_in_the_queue_keeps_its_mark_and_its_turn_has_claude_desktops_limits() {
    let s = served().await;
    let mut c = s.conn().await;
    let secret = s.secret.display().to_string();
    // you type a message; while the Chief answers it, Claude Desktop's message waits in the queue
    s.ready_to_wait();
    let sent = app_chat::send_message(&s.st, None, "FAKE_CHAT_WAIT How are things?".into(), None, vec![], None).await.unwrap();
    let thread = sent.thread_id.clone();
    s.fake_waits().await;
    let (same, q) = c.ask_chief(json!({"message": attach_message(&s.secret), "thread": thread})).await;
    assert_eq!(same, thread);
    let queue = chat::queue(&s.st.db, &thread).unwrap();
    assert_eq!(queue.iter().map(|x| (x.id.as_str(), x.meta.clone())).collect::<Vec<_>>(), [(q.as_str(), Some(json!({"from": "claude-desktop"})))],
               "queued, marked as from Claude Desktop");
    s.go();
    s.idle(&thread).await;
    assert!(s.message(&thread, &q).from_desktop(), "it keeps its mark when it goes");
    assert_eq!(s.last_result(&thread, "attach_file"), (true, not_in_copies(&secret)));
    assert!(s.attached().is_empty());
    assert_eq!(s.turns_answer(&thread), [None, Some("claude-desktop".to_string())]);
}

#[tokio::test]
async fn your_message_and_one_from_claude_desktop_that_go_together_have_claude_desktops_limits() {
    let s = served().await;
    let mut c = s.conn().await;
    s.ready_to_wait();
    let sent = app_chat::send_message(&s.st, None, "FAKE_CHAT_WAIT How are things?".into(), None, vec![], None).await.unwrap();
    let thread = sent.thread_id.clone();
    s.fake_waits().await;
    // yours first, naming a file; then Claude Desktop's: they go as one turn
    let brief = s.brief.display().to_string();
    let yours = app_chat::send_message(&s.st, Some(thread.clone()), format!("FAKE_ATTACH_TO KADE-1 Attach my brief.\n- brief: {brief}"), None,
                                       vec![], None).await.unwrap();
    let (_, q) = c.ask_chief(json!({"message": "And tell me how KADE-1 is doing.", "thread": thread})).await;
    let queue = chat::queue(&s.st.db, &thread).unwrap();
    assert_eq!(queue.iter().map(|x| (x.id.clone(), x.meta.clone())).collect::<Vec<_>>(),
               [(yours.message_id.clone(), None), (q.clone(), Some(json!({"from": "claude-desktop"})))]);
    s.go();
    s.idle(&thread).await;
    assert_eq!(s.last_result(&thread, "attach_file"), (true, not_in_copies(&brief)), "Claude Desktop's limits, also for the file you named");
    assert!(s.attached().is_empty());
    assert_eq!(s.turns_answer(&thread), [None, Some("claude-desktop".to_string())]);
    assert!(!s.message(&thread, &yours.message_id).from_desktop());
    assert!(s.message(&thread, &q).from_desktop());
}

#[tokio::test]
async fn your_message_that_waited_behind_claude_desktops_turn_gets_the_normal_rules() {
    let s = served().await;
    let mut c = s.conn().await;
    s.ready_to_wait();
    let (thread, _) = c.ask_chief(json!({"message": "FAKE_CHAT_WAIT How are things?"})).await;
    s.fake_waits().await;
    let brief = s.brief.display().to_string();
    let yours = app_chat::send_message(&s.st, Some(thread.clone()), format!("FAKE_ATTACH_TO KADE-1 Attach my brief.\n- brief: {brief}"), None,
                                       vec![], None).await.unwrap();
    assert_eq!(chat::queue(&s.st.db, &thread).unwrap().iter().map(|x| x.meta.clone()).collect::<Vec<_>>(), [None], "yours isn't marked");
    s.go();
    s.idle(&thread).await;
    let (is_error, result) = s.last_result(&thread, "attach_file");
    assert!(!is_error, "{result}");
    assert_eq!(s.attached(), ["brief.txt"]);
    assert_eq!(s.turns_answer(&thread), [Some("claude-desktop".to_string()), None]);
    assert!(!s.message(&thread, &yours.message_id).from_desktop());
}

#[tokio::test]
async fn a_message_from_claude_desktop_sent_with_send_now_after_a_stop_keeps_claude_desktops_limits() {
    let s = served().await;
    let mut c = s.conn().await;
    let secret = s.secret.display().to_string();
    s.ready_to_wait();
    let (thread, _) = c.ask_chief(json!({"message": "FAKE_CHAT_WAIT How are things?"})).await;
    s.fake_waits().await;
    let (_, q) = c.ask_chief(json!({"message": attach_message(&s.secret), "thread": thread})).await;
    app_chat::stop(&s.st, &thread);
    s.idle(&thread).await;
    let queue = chat::queue(&s.st.db, &thread).unwrap();
    assert_eq!(queue.iter().map(|x| (x.id.as_str(), x.held, x.meta.clone())).collect::<Vec<_>>(),
               [(q.as_str(), true, Some(json!({"from": "claude-desktop"})))], "it waits for Send now, still marked");
    // you press Send now in Hersir (fake-go: a new session that is told the stopped message doesn't wait again)
    s.go();
    let done = app_chat::send_queue(&s.st, &thread, None).await.unwrap();
    tokio::time::timeout(Duration::from_secs(30), done).await.expect("the answer ended").unwrap();
    s.idle(&thread).await;
    assert!(s.message(&thread, &q).from_desktop());
    assert_eq!(s.last_result(&thread, "attach_file"), (true, not_in_copies(&secret)));
    assert!(s.attached().is_empty());
    assert_eq!(s.turns_answer(&thread).last(), Some(&Some("claude-desktop".to_string())));
}
