// Linux and macOS only: over the Unix socket, and a run's and the Chief's Claude Code are scripts.
#![cfg(unix)]
//! GA-345 QA: the board, a card and the Chief's thread in Claude Desktop's chat, as the card's contract has them, over
//! Hersir's socket with the token Connect makes and a client that declared MCP Apps (as Claude Desktop and the view in its
//! chat both call). claude_desktop_board_view_test.rs checks tools/list, the resources and the board view's sentence.
//! - show {view: card, id}: one sentence in `content`, {view, id, data} in `structuredContent`, both resourceUri keys in
//!   the result's `_meta`; view_read the same data with seq, board_changes' counter now.
//! - view_read's data: the board of one project and of every project (the contract's fields on each card: running,
//!   needs_you, hold), a card (its fields, comments, runs and live run) and the Chief's thread (as chief_reply gives it).
//! - An unknown card, project or thread (or view) answers isError and changes nothing.
//! - The view's writes, the model's own tools called as the view calls them (move_task, comment_on_task, start_agent_run,
//!   stop_agent_run, ask_chief, chief_reply), show in the next view_read, and the card's in board_changes.
//! - No result passes 150,000 characters: a card with a long description, 100 comments, long run summaries and a long
//!   live run log; a full board; a long chat. Cut texts end with "…(cut: read the rest in Hersir)".
//! Never the real Claude Code or Claude Desktop: fake scripts, and Claude's folder is the test's own.
use std::path::{Path, PathBuf};
use std::time::Duration;

use gizai_core::model::*;
use gizai_core::{chat, comments, labels, projects, pulls, settings, tasks, team};
use gizai_lib::claude_desktop as desktop;
use gizai_lib::tools::apps;
use gizai_lib::{AppState, tools};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

const WAIT: Duration = Duration::from_secs(30);
const APP_URI: &str = "ui://hersir/app.html";
const APP_MIME: &str = "text/html;profile=mcp-app";
const CHAT_FAKE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude-chat.py");
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fixtures");

/// The contract's limit on a result, and how a cut text ends, word for word.
const MAX_RESULT: usize = 150_000;
const CUT: &str = "…(cut: read the rest in Hersir)";

const DESCRIPTION: &str = "Settings follow the system theme.";
const ACCEPTANCE: &str = "- [ ] Dark settings page";
const HOLD_REASON: &str = "Which provider retries: ours or theirs?";
const PR_URL: &str = "https://github.com/example/kade/pull/7";

/// The contract's fields, sorted.
const BOARD_CARD_KEYS: [&str; 9] = ["assignee", "hold", "identifier", "labels", "needs_you", "priority", "project", "running", "title"];
const CARD_KEYS: [&str; 15] = ["acceptance_md", "assignee", "column", "comments", "description_md", "hold", "hold_reason", "identifier",
    "labels", "live", "pr_url", "priority", "project", "runs", "title"];
const COMMENT_KEYS: [&str; 3] = ["at", "author", "body_md"];
const RUN_KEYS: [&str; 6] = ["agent", "ended_at", "outcome", "started_at", "status", "summary"];
const CHIEF_KEYS: [&str; 4] = ["draft", "messages", "status", "thread"];

/// Claude Desktop's `initialize` params when it shows MCP Apps' views.
fn claude_desktop_apps() -> Value {
    json!({"protocolVersion": "2025-11-25", "capabilities": {"extensions": {"io.modelcontextprotocol/ui": {"mimeTypes": [APP_MIME]}}},
        "clientInfo": {"name": "claude-ai", "version": "0.14.0"}})
}

struct Conn {
    lines: Lines<BufReader<OwnedReadHalf>>,
    w: OwnedWriteHalf,
    id: i64,
}

impl Conn {
    /// The hello line with the token, then `initialize` as Claude Desktop with MCP Apps.
    async fn open(st: &AppState, token: &str) -> Conn {
        let (r, mut w) = UnixStream::connect(&st.mcp_socket).await.unwrap().into_split();
        w.write_all(gizai_mcp::hello_line(token).as_bytes()).await.unwrap();
        let mut c = Conn { lines: BufReader::new(r).lines(), w, id: 0 };
        let init = c.ask("initialize", claude_desktop_apps()).await;
        assert!(init["result"]["capabilities"]["extensions"].is_object(), "{init}");
        c.w.write_all(format!("{}\n", json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).as_bytes()).await.unwrap();
        c
    }
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
    /// tools/call: the whole result, checked to be under the contract's limit.
    async fn call(&mut self, name: &str, args: Value) -> Value {
        let r = self.ask("tools/call", json!({"name": name, "arguments": args})).await["result"].clone();
        assert!(chars(&r) < MAX_RESULT, "{name} {args}: {} characters", chars(&r));
        r
    }
    /// A model tool's call that must work: its answer, parsed when it is JSON.
    async fn ok(&mut self, name: &str, args: Value) -> Value {
        let r = self.call(name, args.clone()).await;
        assert_eq!(r["isError"], false, "{name} {args}: {r}");
        serde_json::from_str(&text(&r)).unwrap_or(Value::String(text(&r)))
    }
    /// show or view_read that must work: its whole result, with the checks every one passes (one sentence in `content`,
    /// `structuredContent` {view, id, data}, view_read's with seq; show's result `_meta` both resourceUri keys, view_read's none).
    async fn view(&mut self, name: &str, args: Value) -> Value {
        let r = self.call(name, args.clone()).await;
        assert_eq!(r["isError"], false, "{name} {args}: {r}");
        let content = r["content"].as_array().unwrap();
        assert_eq!(content.len(), 1, "one text: {r}");
        let sentence = text(&r);
        assert!(sentence.ends_with('.') && !sentence.contains('\n'), "one sentence: {sentence}");
        let s = &r["structuredContent"];
        assert_eq!(s["view"], args["view"].as_str().unwrap().to_ascii_lowercase().as_str(), "{r}");
        let want: &[&str] = if name == apps::VIEW_READ { &["data", "id", "seq", "view"] } else { &["data", "id", "view"] };
        assert_eq!(keys(s), want, "{r}");
        if name == apps::SHOW {
            assert_eq!(r["_meta"], json!({"ui": {"resourceUri": APP_URI}, "ui/resourceUri": APP_URI}), "{r}");
        } else {
            assert!(r.get("_meta").is_none(), "{r}");
        }
        r
    }
    /// view_read's data and seq.
    async fn read(&mut self, view: &str, id: Option<&str>) -> (Value, i64) {
        let args = match id { Some(id) => json!({"view": view, "id": id}), None => json!({"view": view}) };
        let r = self.view(apps::VIEW_READ, args).await;
        (r["structuredContent"]["data"].clone(), r["structuredContent"]["seq"].as_i64().unwrap())
    }
    /// board_changes since `since`: (seq, cards).
    async fn changes(&mut self, since: i64) -> (i64, Vec<String>) {
        let r = self.call(tools::BOARD_CHANGES, json!({"since": since})).await;
        assert_eq!(r["isError"], false, "{r}");
        let v = &r["structuredContent"];
        let cards = v["cards"].as_array().unwrap_or_else(|| panic!("{r}")).iter().map(|c| c.as_str().unwrap().to_string()).collect();
        (v["seq"].as_i64().unwrap(), cards)
    }
}

fn text(result: &Value) -> String {
    result["content"][0]["text"].as_str().unwrap_or_default().to_string()
}

fn chars(v: &Value) -> usize {
    v.to_string().chars().count()
}

fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().unwrap_or_else(|| panic!("not an object: {v}")).keys().cloned().collect();
    k.sort();
    k
}

/// Every card on a board's data: (column name, card).
fn cards_of(board: &Value) -> Vec<(String, Value)> {
    board["columns"].as_array().unwrap().iter()
        .flat_map(|c| c["cards"].as_array().unwrap().iter().map(|card| (c["name"].as_str().unwrap().to_string(), card.clone())).collect::<Vec<_>>())
        .collect()
}

fn on_board(board: &Value, ident: &str) -> (String, Value) {
    cards_of(board).into_iter().find(|(_, c)| c["identifier"] == ident).unwrap_or_else(|| panic!("{ident} isn't on the board: {board}"))
}

/// Like 2026-11-23T14:05:09Z.
fn is_utc(t: &Value) -> bool {
    let s = t.as_str().unwrap_or_default();
    s.len() == 20 && s.as_bytes()[10] == b'T' && s.ends_with('Z') && s[..4].chars().all(|c| c.is_ascii_digit())
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

fn git(dir: &Path, args: &[&str]) {
    let o = std::process::Command::new("git").args(["-c", "user.email=t@t", "-c", "user.name=t"]).args(args).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
}

/// The shim binary: target/debug/hersir-mcp, which the crate gizai-mcp builds (built here when missing).
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

/// A Claude Code for an agent's run: its init line, then `lines` lines of output (`line 0001 xxx…`, `width` x's each), 100
/// to an event, and then it waits until it is stopped (Stop's SIGINT ends perl).
fn run_fake(dir: &Path, lines: usize, width: usize) -> String {
    let path = dir.join(format!("claude-log-{lines}x{width}.sh"));
    let script = r#"#!/usr/bin/env bash
case " $* " in *" --version "*) echo "2.1.0 (Claude Code)"; exit 0 ;; esac
case " $* " in *" --input-format stream-json "*)
  IFS= read -r _request
  cat "FIXTURES/models-init.jsonl"
  cat > /dev/null
  exit 0 ;;
esac
cat > /dev/null
exec perl -e '$| = 1; my ($n, $w) = @ARGV;
  print qq({"type":"system","subtype":"init","session_id":"S1","model":"claude-opus-5-5","tools":["Read"]}\n);
  for (my $i = 1; $i <= $n; $i += 100) {
    my $last = $i + 99 > $n ? $n : $i + 99;
    my $t = join("\\n", map { sprintf("line %04d %s", $_, "x" x $w) } $i .. $last);
    print qq({"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"$t"}]},"session_id":"S1"}\n);
  }
  sleep 600' -- NLINES WIDTH
"#.replace("FIXTURES", FIXTURES).replace("NLINES", &lines.to_string()).replace("WIDTH", &width.to_string());
    // written by a child, so this process never holds the script open for writing (ETXTBSY in other tests' children)
    assert!(std::process::Command::new("sh").args(["-c", "printf '%s' \"$1\" > \"$2\" && chmod +x \"$2\"", "sh", &script])
        .arg(&path).status().unwrap().success());
    path.to_string_lossy().into_owned()
}

struct S {
    st: AppState,
    tmp: tempfile::TempDir,
    lead: String,
    token: String,
    _server: tokio::task::JoinHandle<()>,
}

/// A test that fails part-way still stops the runs it started: the fake run waits 10 minutes otherwise.
impl Drop for S {
    fn drop(&mut self) {
        for r in gizai_lib::runs::live(&self.st) {
            gizai_lib::runs::stop(&self.st, &r.run_id);
        }
    }
}

/// Hersir with its socket served and Claude Desktop connected, the queue paused (only a run started by hand runs):
/// the Chief (Team Lead, Chat on) and a Backend Agent; project KADE ("Kade portal", a git repository) with
/// - KADE-1 "Export invoices as CSV" in Backlog;
/// - KADE-2 "Retry failed webhooks" in To do, on hold (needs_decision, with a reason), priority 2, label payments;
/// - KADE-3 "Dark theme for settings" in To do, for the Backend Agent, with a description, acceptance criteria and a PR;
/// - KADE-4 "Ship the release" in Review, for you;
/// and project OPS ("Ops") with OPS-1 "Rotate keys" in To do. Claude Code is `bin` (None: a stand-in that is never run).
fn served(bin: Option<&str>, shim: Option<PathBuf>) -> S {
    let tmp = tempfile::tempdir().unwrap();
    let mut st = gizai_lib::test_state(tmp.path());
    st.mcp_shim = Some(shim.unwrap_or_else(|| {
        let p = tmp.path().join("hersir-mcp");
        std::fs::write(&p, "stand-in").unwrap();
        p
    }));
    settings::set(&st.db, "claude_bin", &bin.map_or_else(|| tmp.path().join("no-claude").display().to_string(), str::to_string)).unwrap();
    settings::set(&st.db, "agents_paused", &true).unwrap();
    let team_id = team::list(&st.db).unwrap()[0].id.clone();
    let lead = team::add_agent(&st.db, &st.you_id, &team_id, AgentInput { name: "Team Lead".into(), role_key: "lead".into(),
        chat_enabled: Some(true), ..Default::default() }).unwrap();
    let backend = team::add_agent(&st.db, &st.you_id, &team_id, AgentInput { name: "Backend Agent".into(), role_key: "backend".into(),
        ..Default::default() }).unwrap();
    let repo = tmp.path().join("kade");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("README.md"), "# Kade").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "init"]);
    let kade = projects::create(&st.db, &st.you_id, ProjectInput { name: "Kade portal".into(), key: "KADE".into(),
        repo_path: Some(repo.display().to_string()), ..Default::default() }).unwrap();
    let ops = projects::create(&st.db, &st.you_id, ProjectInput { name: "Ops".into(), key: "OPS".into(), ..Default::default() }).unwrap();
    let states = team::get(&st.db, &team_id).unwrap().states;
    let col = |cat: &str| states.iter().find(|s| s.category == cat).unwrap().id.clone();
    tasks::create(&st.db, &st.you_id, TaskInput { project_id: kade.clone(), title: "Export invoices as CSV".into(), ..Default::default() }).unwrap();
    let held = tasks::create(&st.db, &st.you_id, TaskInput { project_id: kade.clone(), title: "Retry failed webhooks".into(),
        state_id: Some(col("ready")), priority: 2, ..Default::default() }).unwrap();
    tasks::update(&st.db, &st.you_id, &held, TaskPatch { hold: Some("needs_decision".into()), hold_reason: Some(HOLD_REASON.into()),
        ..Default::default() }).unwrap();
    let bug = labels::save(&st.db, &st.you_id, None, "payments", Some("#f2706b")).unwrap();
    tasks::set_labels(&st.db, &st.you_id, &held, vec![bug]).unwrap();
    let dark = tasks::create(&st.db, &st.you_id, TaskInput { project_id: kade.clone(), title: "Dark theme for settings".into(),
        description_md: DESCRIPTION.into(), acceptance_md: Some(ACCEPTANCE.into()), state_id: Some(col("ready")),
        assignee_id: Some(backend.clone()), ..Default::default() }).unwrap();
    pulls::record(&st.db, None, &dark, PR_URL, "open", true).unwrap();
    tasks::create(&st.db, &st.you_id, TaskInput { project_id: kade, title: "Ship the release".into(), state_id: Some(col("review")),
        assignee_id: Some(st.you_id.clone()), ..Default::default() }).unwrap();
    tasks::create(&st.db, &st.you_id, TaskInput { project_id: ops, title: "Rotate keys".into(), state_id: Some(col("ready")),
        ..Default::default() }).unwrap();
    let server = gizai_lib::mcp::start(&st).unwrap();
    let c = desktop::connect(&st, false).unwrap();
    let entry: Value = serde_json::from_str(&c.entry).unwrap();
    let token = entry["mcpServers"]["hersir"]["env"]["HERSIR_TOKEN"].as_str().unwrap().to_string();
    S { st, tmp, lead, token, _server: server }
}

impl S {
    async fn conn(&self) -> Conn {
        Conn::open(&self.st, &self.token).await
    }
    fn card(&self, identifier: &str) -> Task {
        tasks::get(&self.st.db, &tasks::id_of(&self.st.db, identifier).unwrap()).unwrap()
    }
    /// A column's name by its category.
    fn column(&self, category: &str) -> String {
        let team_id = team::list(&self.st.db).unwrap()[0].id.clone();
        team::get(&self.st.db, &team_id).unwrap().states.into_iter().find(|s| s.category == category).unwrap().name
    }
    fn you_name(&self) -> String {
        gizai_core::users::list(&self.st.db).unwrap().into_iter().find(|p| p.id == self.st.you_id).unwrap().name
    }
    fn backend(&self) -> String {
        team::all_agents(&self.st.db).unwrap().into_iter().find(|(_, m)| m.name == "Backend Agent").unwrap().1.actor_id
    }
    fn live_on(&self, identifier: &str) -> bool {
        let id = self.card(identifier).id;
        gizai_lib::runs::live(&self.st).iter().any(|r| r.task_id == id)
    }
    /// Waits until no run of the card is live.
    async fn stopped(&self, identifier: &str) {
        for _ in 0..300 {
            if !self.live_on(identifier) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("{identifier}'s run didn't stop");
    }
}

/// view_read's card until its live run's output has `line`: the card's data.
async fn live_output_with(c: &mut Conn, ident: &str, line: &str) -> Value {
    for _ in 0..300 {
        let (card, _) = c.read("card", Some(ident)).await;
        if card["live"]["output"].as_str().is_some_and(|o| o.contains(line)) {
            return card;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("{ident}'s live output never had {line}");
}

// ---- show and view_read: a card ----

#[tokio::test]
async fn show_a_card_answers_one_sentence_its_data_and_both_resource_uri_keys_and_view_read_the_same_data_with_seq() {
    let s = served(None, None);
    comments::add(&s.st.db, &s.st.you_id, &s.card("KADE-3").id, "Use the system colours.", None).unwrap();
    std::thread::sleep(Duration::from_millis(5));
    comments::add(&s.st.db, &s.backend(), &s.card("KADE-3").id, "Started on it.", None).unwrap();
    let mut c = s.conn().await;
    let todo = s.column("ready");

    let r = c.view(apps::SHOW, json!({"view": "card", "id": "KADE-3"})).await;
    assert_eq!(text(&r), format!("KADE-3 – Dark theme for settings, in {todo}."), "the card's sentence");
    assert_eq!(r["structuredContent"]["id"], "KADE-3");
    let data = r["structuredContent"]["data"].clone();
    assert_eq!(keys(&data), CARD_KEYS, "the contract's fields: {data}");
    let fields = json!({"identifier": data["identifier"], "title": data["title"], "project": data["project"], "column": data["column"],
        "assignee": data["assignee"], "labels": data["labels"], "priority": data["priority"], "hold": data["hold"],
        "hold_reason": data["hold_reason"], "description_md": data["description_md"], "acceptance_md": data["acceptance_md"],
        "pr_url": data["pr_url"], "live": data["live"]});
    assert_eq!(fields, json!({"identifier": "KADE-3", "title": "Dark theme for settings", "project": "Kade portal", "column": todo,
        "assignee": "Backend Agent", "labels": [], "priority": 0, "hold": null, "hold_reason": null, "description_md": DESCRIPTION,
        "acceptance_md": ACCEPTANCE, "pr_url": PR_URL, "live": null}));
    // its comments, oldest first, with who wrote them and when (UTC)
    let comments = data["comments"].as_array().unwrap();
    assert_eq!(comments.len(), 2, "{data}");
    for cm in comments {
        assert_eq!(keys(cm), COMMENT_KEYS, "{cm}");
        assert!(is_utc(&cm["at"]), "{cm}");
    }
    assert_eq!((comments[0]["author"].as_str().unwrap(), comments[0]["body_md"].as_str().unwrap()), (s.you_name().as_str(), "Use the system colours."));
    assert_eq!((comments[1]["author"].as_str().unwrap(), comments[1]["body_md"].as_str().unwrap()), ("Backend Agent", "Started on it."));
    assert_eq!(data["runs"], json!([]));

    // view_read: the same data and sentence, with seq, board_changes' counter now; the identifier in any case
    let read = c.view(apps::VIEW_READ, json!({"view": "card", "id": "kade-3"})).await;
    assert_eq!(read["structuredContent"]["id"], "KADE-3");
    assert_eq!(read["structuredContent"]["data"], data);
    assert_eq!(read["structuredContent"]["seq"], s.st.db.change_seq());
    assert_eq!(text(&read), text(&r));
    let changes = c.call(tools::BOARD_CHANGES, json!({})).await;
    assert_eq!(changes["structuredContent"]["seq"], read["structuredContent"]["seq"], "seq is board_changes' counter");

    // a card on hold says so, with its hold, its reason and its label
    let r = c.view(apps::SHOW, json!({"view": "card", "id": "KADE-2"})).await;
    assert_eq!(text(&r), format!("KADE-2 – Retry failed webhooks, in {todo}, on hold (needs decision)."));
    let held = &r["structuredContent"]["data"];
    assert_eq!((held["hold"].clone(), held["hold_reason"].clone(), held["priority"].clone(), held["labels"].clone()),
        (json!("needs_decision"), json!(HOLD_REASON), json!(2), json!([{"name": "payments", "color": "#f2706b"}])), "{held}");
    assert_eq!((held["description_md"].clone(), held["acceptance_md"].clone(), held["pr_url"].clone(), held["assignee"].clone()),
        (json!(""), Value::Null, Value::Null, Value::Null), "{held}");
    // nothing secret about the project: its repository path stays in Hersir
    assert!(!r.to_string().contains(&s.tmp.path().display().to_string()), "{r}");
}

// ---- view_read: the board ----

#[tokio::test]
async fn view_read_gives_the_board_of_one_project_and_of_every_project_with_the_contracts_fields_and_seq() {
    let s = served(None, None);
    let mut c = s.conn().await;
    let (board, seq) = c.read("board", Some("KADE")).await;
    assert_eq!(seq, s.st.db.change_seq());
    for col in board["columns"].as_array().unwrap() {
        for k in ["name", "kind", "cards"] {
            assert!(col.get(k).is_some(), "{k} on a column: {col}");
        }
    }
    let cards = cards_of(&board);
    let idents: Vec<&str> = cards.iter().map(|(_, c)| c["identifier"].as_str().unwrap()).collect();
    assert_eq!(idents.len(), 4, "{idents:?}");
    assert!(idents.iter().all(|i| i.starts_with("KADE-")), "one project: {idents:?}");
    for (_, card) in &cards {
        assert_eq!(keys(card), BOARD_CARD_KEYS, "{card}");
        assert_eq!(card["running"], false, "{card}");
    }
    let (column, kade2) = on_board(&board, "KADE-2");
    assert_eq!(column, s.column("ready"));
    assert_eq!((kade2["hold"].clone(), kade2["needs_you"].clone(), kade2["labels"].clone()),
        (json!("needs_decision"), json!(true), json!([{"name": "payments", "color": "#f2706b"}])), "on hold: it needs you");
    let (column, kade4) = on_board(&board, "KADE-4");
    assert_eq!(column, s.column("review"));
    assert_eq!((kade4["hold"].clone(), kade4["needs_you"].clone(), kade4["assignee"].clone()), (Value::Null, json!(true), json!(s.you_name())),
        "in Review for you: it needs you");
    let (_, kade3) = on_board(&board, "KADE-3");
    assert_eq!((kade3["needs_you"].clone(), kade3["assignee"].clone(), kade3["project"].clone()),
        (json!(false), json!("Backend Agent"), json!("Kade portal")));
    assert_eq!(on_board(&board, "KADE-1").0, s.column("backlog"));

    // every project: no id, both projects' cards
    let r = c.view(apps::VIEW_READ, json!({"view": "board"})).await;
    assert_eq!(r["structuredContent"]["id"], Value::Null);
    let all = &r["structuredContent"]["data"];
    let mut idents: Vec<String> = cards_of(all).iter().map(|(_, c)| c["identifier"].as_str().unwrap().to_string()).collect();
    idents.sort();
    assert_eq!(idents, ["KADE-1", "KADE-2", "KADE-3", "KADE-4", "OPS-1"]);
    assert_eq!(on_board(all, "OPS-1").1["project"], "Ops");
    // and show for one project, by its key
    let r = c.view(apps::SHOW, json!({"view": "board", "id": "OPS"})).await;
    assert_eq!(r["structuredContent"]["id"], "OPS");
    let ops: Vec<String> = cards_of(&r["structuredContent"]["data"]).iter().map(|(_, c)| c["identifier"].as_str().unwrap().to_string()).collect();
    assert_eq!(ops, ["OPS-1"]);
}

// ---- view_read: the Chief's thread ----

#[tokio::test]
async fn view_read_gives_the_chiefs_thread_as_chief_reply_gives_it_and_without_an_id_the_chat_from_claude_desktop() {
    let s = served(None, None);
    let mut c = s.conn().await;
    // before ask_chief made the chat From Claude Desktop: an empty one, with no thread, and nothing made
    let before = dump(&s.st);
    let r = c.view(apps::SHOW, json!({"view": "chief"})).await;
    assert_eq!(r["structuredContent"]["id"], Value::Null);
    assert_eq!(r["structuredContent"]["data"], json!({"thread": null, "status": "answered", "messages": [], "draft": null}));
    let (data, _) = c.read("chief", None).await;
    assert_eq!(keys(&data), CHIEF_KEYS);
    assert_eq!(dump(&s.st), before, "view_read never makes the chat");

    // a thread by its id: as chief_reply gives it, the draft null while the Chief isn't writing
    let thread = chat::create_thread(&s.st.db, &s.st.you_id, &s.lead, "What's on the board?").unwrap();
    chat::add_message(&s.st.db, chat::NewMessage { thread_id: thread.clone(), role: "user".into(), author_id: Some(s.st.you_id.clone()),
        body_md: Some("What's on the board?".into()), ..Default::default() }).unwrap();
    chat::add_message(&s.st.db, chat::NewMessage { thread_id: thread.clone(), role: "agent".into(), author_id: Some(s.lead.clone()),
        body_md: Some("Four cards on KADE, one on hold.".into()), ..Default::default() }).unwrap();
    let r = c.view(apps::SHOW, json!({"view": "chief", "id": thread})).await;
    assert_eq!(r["structuredContent"]["id"], thread.as_str());
    let data = r["structuredContent"]["data"].clone();
    assert_eq!(keys(&data), CHIEF_KEYS);
    let mut reply = c.ok("chief_reply", json!({"thread": thread, "wait_seconds": 0})).await;
    if reply.get("draft").is_none() {
        reply["draft"] = Value::Null;
    }
    assert_eq!(data, reply, "as chief_reply gives it");
    assert_eq!((data["thread"].clone(), data["status"].clone(), data["draft"].clone()), (json!(thread), json!("answered"), Value::Null));
    let said: Vec<(String, String)> = data["messages"].as_array().unwrap().iter()
        .map(|m| (m["from"].as_str().unwrap().to_string(), m["text"].as_str().unwrap().to_string())).collect();
    assert_eq!(said.last().unwrap(), &("Chief".to_string(), "Four cards on KADE, one on hold.".to_string()), "{said:?}");
    let (read, seq) = c.read("chief", Some(&thread)).await;
    assert_eq!((read, seq), (data, s.st.db.change_seq()));
}

// ---- errors ----

#[tokio::test]
async fn an_unknown_card_project_or_thread_answers_is_error_and_changes_nothing() {
    let s = served(None, None);
    let mut c = s.conn().await;
    let before = dump(&s.st);
    for name in [apps::SHOW, apps::VIEW_READ] {
        for (args, says) in [
            (json!({"view": "card", "id": "KADE-99"}), Some("KADE-99")),
            (json!({"view": "card", "id": "NOPE-1"}), Some("NOPE-1")),
            (json!({"view": "board", "id": "NOPE"}), Some("NOPE")),
            (json!({"view": "chief", "id": "no-such-thread"}), Some("no-such-thread")),
            (json!({"view": "card"}), None),
            (json!({"view": "column", "id": "KADE-1"}), Some("column")),
            (json!({"id": "KADE-1"}), None),
        ] {
            let r = c.call(name, args.clone()).await;
            assert_eq!(r["isError"], true, "{name} {args}: {r}");
            assert!(r.get("structuredContent").is_none() && r.get("_meta").is_none(), "{name} {args}: {r}");
            assert!(!text(&r).is_empty(), "{name} {args}: says why");
            if let Some(word) = says {
                assert!(text(&r).contains(word), "{name} {args}: {r}");
            }
        }
    }
    assert_eq!(dump(&s.st), before, "nothing changed");
}

// ---- the view's writes ----

/// The view writes with the model's own tools, through Claude (here: tools/call on the same connection, as Claude
/// Desktop sends a view's calls): a move, a comment, a run started and stopped each show in the next view_read, and their
/// card in board_changes since the seq before.
#[tokio::test]
async fn a_move_a_comment_and_a_run_from_the_view_show_in_the_next_view_read_and_in_board_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let fake = run_fake(tmp.path(), 3, 10);
    let s = served(Some(&fake), None);
    let mut c = s.conn().await;
    let (_, seq0) = c.read("board", Some("KADE")).await;

    // move_task
    let review = s.column("review");
    c.ok("move_task", json!({"task": "KADE-3", "column": review})).await;
    let (board, seq1) = c.read("board", Some("KADE")).await;
    assert_eq!(on_board(&board, "KADE-3").0, review, "the move shows in the next view_read");
    let (seq, cards) = c.changes(seq0).await;
    assert!(seq > seq0, "{seq} {seq0}");
    assert_eq!(cards, ["KADE-3"]);
    assert_eq!(seq, seq1, "view_read's seq is board_changes' counter");

    // comment_on_task
    c.ok("comment_on_task", json!({"task": "KADE-3", "body_md": "Looks right from the view."})).await;
    let (card, seq2) = c.read("card", Some("KADE-3")).await;
    let last = card["comments"].as_array().unwrap().last().unwrap().clone();
    assert_eq!((last["author"].as_str().unwrap(), last["body_md"].as_str().unwrap()), (s.you_name().as_str(), "Looks right from the view."));
    assert_eq!(card["column"], review.as_str());
    let (seq, cards) = c.changes(seq1).await;
    assert_eq!((seq, cards), (seq2, vec!["KADE-3".to_string()]));

    // start_agent_run: the card runs, its live output shows, and its run
    let r = c.ok("start_agent_run", json!({"task": "KADE-1", "agent": "Backend Agent"})).await;
    assert_eq!(r["done"], "started", "{r}");
    let card = live_output_with(&mut c, "KADE-1", "line 0003").await;
    assert_eq!(card["live"]["agent"], "Backend Agent", "{card}");
    let output = card["live"]["output"].as_str().unwrap();
    assert!(output.ends_with(&format!("line 0003 {}", "x".repeat(10))), "{output}");
    assert!(output.contains("line 0001") && !output.contains(CUT), "the whole short output: {output}");
    let runs = card["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 1, "{card}");
    assert_eq!(keys(&runs[0]), RUN_KEYS);
    assert_eq!((runs[0]["agent"].as_str().unwrap(), runs[0]["status"].as_str().unwrap()), ("Backend Agent", "running"), "{card}");
    assert!(is_utc(&runs[0]["started_at"]), "{card}");
    let (board, seq3) = c.read("board", Some("KADE")).await;
    assert_eq!(on_board(&board, "KADE-1").1["running"], true, "an agent works on it now");
    assert!(cards_of(&board).iter().filter(|(_, c)| c["identifier"] != "KADE-1").all(|(_, c)| c["running"] == false));
    let (seq, cards) = c.changes(seq2).await;
    assert!(seq >= seq3, "{seq} {seq3}");
    assert!(cards.contains(&"KADE-1".to_string()), "{cards:?}");

    // stop_agent_run: no live run any more, and the run ended
    let r = c.ok("stop_agent_run", json!({"task": "KADE-1"})).await;
    assert_eq!(r["done"], "stopping", "{r}");
    s.stopped("KADE-1").await;
    let mut card = Value::Null;
    for _ in 0..100 {
        card = c.read("card", Some("KADE-1")).await.0;
        if card["runs"][0]["status"] != "running" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(card["live"], Value::Null, "{card}");
    assert!(!matches!(card["runs"][0]["status"].as_str(), Some("running" | "queued")), "{card}");
    assert!(is_utc(&card["runs"][0]["ended_at"]), "{card}");
    tokio::time::sleep(Duration::from_millis(500)).await;
    let (board, seq4) = c.read("board", Some("KADE")).await;
    assert_eq!(on_board(&board, "KADE-1").1["running"], false);
    let (seq, cards) = c.changes(seq3).await;
    assert_eq!(seq, seq4);
    assert!(cards.contains(&"KADE-1".to_string()), "{cards:?}");
    // and with the latest seq: nothing new
    assert_eq!(c.changes(seq4).await, (seq4, vec![]));
}

/// ask_chief and chief_reply from the view: the chat From Claude Desktop shows in view_read's Chief view (no id), with
/// your message, then the Chief's answer.
#[tokio::test(flavor = "multi_thread")]
async fn ask_chief_and_chief_reply_from_the_view_show_in_the_next_view_read_of_the_chiefs_thread() {
    let s = served(Some(CHAT_FAKE), Some(real_shim()));
    let mut c = s.conn().await;
    let r = c.ok("ask_chief", json!({"message": "What is on hold on KADE?"})).await;
    let thread = r["thread"].as_str().unwrap_or_else(|| panic!("{r}")).to_string();
    let r = c.view(apps::VIEW_READ, json!({"view": "chief"})).await;
    assert_eq!(r["structuredContent"]["id"], thread.as_str(), "the chat From Claude Desktop");
    let data = &r["structuredContent"]["data"];
    assert_eq!(data["thread"], thread.as_str());
    assert!(data["messages"].as_array().unwrap().iter().any(|m| m["from"] == "you" && m["text"] == "What is on hold on KADE?"), "{data}");
    // chief_reply until the Chief answered
    let mut answered = Value::Null;
    for _ in 0..240 {
        let r = c.ok("chief_reply", json!({"thread": thread, "wait_seconds": 0})).await;
        if r["status"] != "working" {
            answered = r;
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert_eq!(answered["status"], "answered", "{answered}");
    let (data, _) = c.read("chief", None).await;
    assert_eq!(data["status"], "answered");
    assert_eq!(data["draft"], Value::Null);
    let last = data["messages"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(last["from"], "Chief", "{data}");
    assert_eq!(data["messages"], answered["messages"], "as chief_reply gives them");
}

// ---- small results ----

/// A card with a long description and acceptance criteria, 100 long comments, 7 runs with long summaries and a live run
/// with 1,000 long lines of output: show and view_read stay under 150,000 characters, with the last 20 comments, the last
/// 5 runs and the live run's last 200 lines, and every cut text ends with the contract's words.
#[tokio::test]
async fn a_card_with_a_long_description_100_comments_and_a_long_run_log_stays_under_150000_characters() {
    let tmp = tempfile::tempdir().unwrap();
    let fake = run_fake(tmp.path(), 1000, 300);
    let s = served(Some(&fake), None);
    let id = s.card("KADE-1").id;
    let backend = s.backend();
    let description = format!("Start of the description. {}", "d".repeat(400_000));
    tasks::update(&s.st.db, &s.st.you_id, &id, TaskPatch { description_md: Some(description.clone()),
        acceptance_md: Some(format!("- [ ] {}", "a".repeat(100_000))), ..Default::default() }).unwrap();
    for n in 1..=100 {
        comments::add(&s.st.db, &s.st.you_id, &id, &format!("comment {n:03} {}", "c".repeat(9_000)), None).unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    for n in 1..=7 {
        let run = gizai_core::runs::create(&s.st.db, &backend, &id, "backend", &format!("S{n}"), "/tmp", "/tmp", "b", "/tmp/none.log").unwrap();
        gizai_core::runs::finish(&s.st.db, &run, "succeeded", Some(&Outcome { outcome: "qa_pass".into(),
            summary: format!("run {n} {}", "s".repeat(30_000)), issues: vec![] }), 0, 0, 0, None).unwrap();
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut c = s.conn().await;
    c.ok("start_agent_run", json!({"task": "KADE-1", "agent": "Backend Agent"})).await;
    live_output_with(&mut c, "KADE-1", "line 1000").await;

    for name in [apps::SHOW, apps::VIEW_READ] {
        // Conn::call checks the whole result is under 150,000 characters
        let r = c.view(name, json!({"view": "card", "id": "KADE-1"})).await;
        assert!(chars(&r) < MAX_RESULT, "{name}: {}", chars(&r));
        assert_eq!(text(&r), format!("KADE-1 – Export invoices as CSV, in {}.", s.card("KADE-1").state_name), "the run moved it on");
        let d = &r["structuredContent"]["data"];
        assert_eq!(keys(d), CARD_KEYS);
        let desc = d["description_md"].as_str().unwrap();
        assert!(desc.starts_with("Start of the description. ddd") && desc.ends_with(CUT), "{name}: {}", &desc[desc.len() - 60..]);
        assert!(desc.chars().count() < 150_000);
        assert!(d["acceptance_md"].as_str().unwrap().ends_with(CUT), "{name}");
        // the last 20 comments, oldest first, each cut
        let comments = d["comments"].as_array().unwrap();
        assert_eq!(comments.len(), 20, "{name}");
        for (i, cm) in comments.iter().enumerate() {
            let body = cm["body_md"].as_str().unwrap();
            assert!(body.starts_with(&format!("comment {:03} ", 81 + i)), "{name}: comment {i}: {}", &body[..20]);
            assert!(body.ends_with(CUT), "{name}: comment {i}");
        }
        // the last 5 runs, newest first: the live one, then runs 7 to 4, their summaries cut
        let runs = d["runs"].as_array().unwrap();
        assert_eq!(runs.len(), 5, "{name}");
        assert_eq!(runs[0]["status"], "running", "{name}: {}", runs[0]);
        for (i, n) in [7, 6, 5, 4].into_iter().enumerate() {
            let summary = runs[i + 1]["summary"].as_str().unwrap();
            assert!(summary.starts_with(&format!("run {n} ")) && summary.ends_with(CUT), "{name}: run {n}: {}", &summary[..10]);
            assert_eq!(runs[i + 1]["outcome"], "qa_pass");
        }
        // the live run: its last 200 lines at most, the last line kept, then the cut on a line of its own
        let output = d["live"]["output"].as_str().unwrap();
        assert!(output.ends_with(&format!("\n{CUT}")), "{name}: {}", &output[output.len() - 80..]);
        let lines: Vec<&str> = output.lines().collect();
        assert!(lines.len() <= 201, "{name}: {} lines", lines.len());
        assert!(lines[lines.len() - 2].starts_with("line 1000 "), "{name}: {}", lines[lines.len() - 2]);
        for l in &lines[1..lines.len() - 1] {
            let n: usize = l[5..9].parse().unwrap_or_else(|_| panic!("{name}: {l}"));
            assert!(n > 800, "{name}: line {n} is not among the last 200");
        }
    }

    // a held card's long reason is cut too
    tasks::update(&s.st.db, &s.st.you_id, &s.card("KADE-2").id, TaskPatch { hold_reason: Some("r".repeat(300_000)), ..Default::default() }).unwrap();
    let (d, _) = c.read("card", Some("KADE-2")).await;
    assert!(d["hold_reason"].as_str().unwrap().ends_with(CUT));

    c.ok("stop_agent_run", json!({"task": "KADE-1"})).await;
    s.stopped("KADE-1").await;
}

/// A board with 60 cards with long titles in every column, and every project's: show and view_read stay under 150,000
/// characters; each column says how many cards it has and sends at most 50 (20 in Backlog and Done).
#[tokio::test]
async fn a_full_board_stays_under_150000_characters_and_each_column_says_how_many_cards_it_has() {
    let s = served(None, None);
    let team_id = team::list(&s.st.db).unwrap()[0].id.clone();
    let states = team::get(&s.st.db, &team_id).unwrap().states;
    let ops = projects::list(&s.st.db).unwrap().into_iter().find(|p| p.key == "OPS").unwrap().id;
    let mut want = std::collections::HashMap::new();
    for st in &states {
        for n in 0..60 {
            tasks::create(&s.st.db, &s.st.you_id, TaskInput { project_id: ops.clone(), title: format!("{} {n} {}", st.name, "t".repeat(170)),
                state_id: Some(st.id.clone()), ..Default::default() }).unwrap();
        }
        let on = tasks::list_lean(&s.st.db, &TaskFilter { project_id: None, open_only: false }).unwrap().into_iter().filter(|t| t.state_id == st.id).count();
        want.insert(st.name.clone(), on);
    }
    let mut c = s.conn().await;
    for (name, args) in [(apps::SHOW, json!({"view": "board"})), (apps::VIEW_READ, json!({"view": "board"})),
                         (apps::VIEW_READ, json!({"view": "board", "id": "OPS"}))] {
        let r = c.view(name, args.clone()).await;
        assert!(chars(&r) < MAX_RESULT, "{name} {args}: {}", chars(&r));
        for col in r["structuredContent"]["data"]["columns"].as_array().unwrap() {
            let col_name = col["name"].as_str().unwrap();
            let sent = col["cards"].as_array().unwrap().len();
            let total = col["total"].as_u64().unwrap() as usize;
            if args.get("id").is_none() {
                assert_eq!(total, want[col_name], "{name} {args}: {col_name}");
            }
            assert!(total >= 60 && sent > 0 && sent < total, "{name} {args}: {col_name}: {sent} of {total}");
            let rail = matches!(col["kind"].as_str().unwrap(), "backlog" | "done" | "cancelled");
            assert!(sent <= if rail { 20 } else { 50 }, "{name} {args}: {col_name}: {sent}");
        }
    }
}

/// A chat with 30 long messages: view_read's Chief view and chief_reply stay under 150,000 characters, the newest
/// message kept, each text cut as chief_reply cuts it.
#[tokio::test]
async fn a_long_chat_with_the_chief_stays_under_150000_characters() {
    let s = served(None, None);
    let thread = chat::create_thread(&s.st.db, &s.st.you_id, &s.lead, "Long").unwrap();
    for n in 1..=30 {
        let (role, author) = if n % 2 == 1 { ("user", s.st.you_id.clone()) } else { ("agent", s.lead.clone()) };
        chat::add_message(&s.st.db, chat::NewMessage { thread_id: thread.clone(), role: role.into(), author_id: Some(author),
            body_md: Some(format!("message {n:02} {}", "m".repeat(10_000))), ..Default::default() }).unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    let mut c = s.conn().await;
    let r = c.view(apps::VIEW_READ, json!({"view": "chief", "id": thread})).await;
    assert!(chars(&r) < MAX_RESULT, "{}", chars(&r));
    let messages = r["structuredContent"]["data"]["messages"].as_array().unwrap();
    assert!(!messages.is_empty() && messages.len() <= 20, "{}", messages.len());
    assert!(messages.last().unwrap()["text"].as_str().unwrap().starts_with("message 30 "), "the newest message is kept");
    assert!(messages.iter().all(|m| m["text"].as_str().unwrap().ends_with(CUT)));
    let reply = c.call("chief_reply", json!({"thread": thread, "wait_seconds": 0})).await;
    assert_eq!(reply["isError"], false, "{reply}");
    assert!(chars(&reply) < MAX_RESULT, "chief_reply: {}", chars(&reply));
}
