// `hersir-mcp --desktop` (GA-339), as Claude Desktop runs it: against a stand-in for Hersir on a temporary Unix socket,
// as shim_test.rs does. Linux and macOS only: Windows has a named pipe instead.
#![cfg(unix)]
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::thread::{JoinHandle, sleep};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const SHIM: &str = env!("CARGO_BIN_EXE_hersir-mcp");
const TOKEN: &str = "tok-desktop-339-not-for-your-eyes";

// The card's texts, word for word: not the crate's constants, so a change to those shows here.
const NOT_RUNNING: &str = "Open Hersir first: Claude Desktop reaches your board through the Hersir app on this computer, and it isn't running.";
const REFUSED: &str = "Hersir disconnected Claude Desktop. To connect it again: in Hersir, Settings → MCP servers → Claude Desktop → Connect, then quit and reopen Claude Desktop.";
const CONNECTED: &str = "Hersir is open and connected. If its tools aren't listed, quit and reopen Claude Desktop.";
const STATUS_DESCRIPTION: &str = "Says whether Hersir is open and connected. Call it when the user asks about their Hersir board, cards or Chief and no other Hersir tool is listed.";
const LIST_CHANGED: &str = r#"{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}"#;
/// Hersir's own refusal of a token it doesn't know (src-tauri/src/mcp.rs).
const REASON: &str = "this token is not valid: the chat turn has ended or it was never issued";

const WAITING_LINE: &str = "hersir-mcp: Hersir isn't running; waiting for it\n";
const CONNECTED_LINE: &str = "hersir-mcp: connected to Hersir\n";

/// How long a test waits for an answer the shim should give at once (a broken shim fails the test, not hangs it).
const PATIENCE: Duration = Duration::from_secs(10);

// ---- the shim ------------------------------------------------------------------------------------------------------

/// `hersir-mcp --desktop` with HERSIR_SOCKET and HERSIR_TOKEN set, its stdout read line by line and its stderr kept.
struct Desktop {
    child: Child,
    stdin: Option<ChildStdin>,
    out: Receiver<String>,
    err: Arc<Mutex<String>>,
    started: Instant,
    /// Every line the shim wrote to stdout so far.
    seen: Vec<String>,
}

impl Desktop {
    fn start(sock: &Path) -> Desktop {
        let mut cmd = Command::new(SHIM);
        for n in ["HERSIR_SOCKET", "HERSIR_TOKEN", "GIZAI_SOCKET", "GIZAI_TOKEN"] {
            cmd.env_remove(n);
        }
        let started = Instant::now();
        let mut child = cmd.arg("--desktop").env("HERSIR_SOCKET", sock).env("HERSIR_TOKEN", TOKEN)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().unwrap();
        let (tx, out) = channel();
        let stdout = child.stdout.take().unwrap();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let err = Arc::new(Mutex::new(String::new()));
        let stderr = child.stderr.take().unwrap();
        let kept = err.clone();
        std::thread::spawn(move || {
            let mut r = BufReader::new(stderr);
            loop {
                let mut line = String::new();
                match r.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => kept.lock().unwrap().push_str(&line),
                }
            }
        });
        let stdin = child.stdin.take();
        Desktop { child, stdin, out, err, started, seen: Vec::new() }
    }

    fn send(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().expect("stdin is open");
        stdin.write_all(format!("{line}\n").as_bytes()).unwrap();
        stdin.flush().unwrap();
    }

    fn notify(&mut self, method: &str) {
        self.send(&json!({"jsonrpc": "2.0", "method": method}).to_string());
    }

    /// The next line on stdout, None when none comes `within`.
    fn next_line(&mut self, within: Duration) -> Option<String> {
        let line = self.out.recv_timeout(within).ok()?;
        self.seen.push(line.clone());
        Some(line)
    }

    /// The next line on stdout as JSON; fails the test when none comes in time.
    fn next_value(&mut self, within: Duration) -> Value {
        let line = self.next_line(within).unwrap_or_else(|| panic!("no answer within {within:?}; stderr: {}", self.stderr()));
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("not JSON on stdout ({e}): {line}"))
    }

    /// Sends a request and returns the next line on stdout, which must be its answer.
    fn call(&mut self, id: impl Into<Value>, method: &str, params: Value) -> Value {
        let id = id.into();
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string());
        let answer = self.next_value(PATIENCE);
        assert_eq!(answer["id"], id, "the next line answers {method}: {answer}");
        answer
    }

    fn call_tool(&mut self, id: impl Into<Value>, name: &str) -> Value {
        self.call(id, "tools/call", json!({"name": name, "arguments": {}}))
    }

    fn running(&mut self) -> bool {
        self.child.try_wait().unwrap().is_none()
    }

    fn stderr(&self) -> String {
        self.err.lock().unwrap().clone()
    }

    /// Closes stdin and waits for the shim to end; its exit code. The lines it still wrote go into `seen`.
    fn close(&mut self) -> Option<i32> {
        drop(self.stdin.take());
        let t0 = Instant::now();
        let status = loop {
            if let Some(s) = self.child.try_wait().unwrap() {
                break s;
            }
            assert!(t0.elapsed() < Duration::from_secs(5), "the shim kept running 5 s after stdin closed");
            sleep(Duration::from_millis(20));
        };
        while let Some(line) = self.next_line(Duration::from_millis(500)) {
            let _ = line;
        }
        status.code()
    }

    /// The token went only to Hersir: never on stderr, and on stdout not at all here (no stand-in sends it back).
    fn assert_no_token(&self) {
        let err = self.stderr();
        assert!(!err.contains(TOKEN), "the token is on stderr: {err}");
        for line in &self.seen {
            assert!(!line.contains(TOKEN), "the token is on stdout: {line}");
        }
    }
}

impl Drop for Desktop {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ---- a stand-in for Hersir -----------------------------------------------------------------------------------------

#[derive(Clone)]
enum Mode {
    /// Takes the token and answers each request; see `connection`.
    Answer,
    /// Refuses every token with this reason, as Hersir does: one `{"error": …}` line, then a close.
    Refuse(String),
}

#[derive(Default)]
struct Seen {
    connections: AtomicUsize,
    hellos: Mutex<Vec<String>>,
    /// Every line after a hello, as it came.
    lines: Mutex<Vec<String>>,
    /// The connections, to close them or write to them.
    open: Mutex<Vec<UnixStream>>,
}

struct Server {
    path: PathBuf,
    seen: Arc<Seen>,
    stop: Arc<AtomicBool>,
    accept: Option<JoinHandle<()>>,
}

impl Server {
    fn start(path: &Path, mode: Mode) -> Server {
        let listener = UnixListener::bind(path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let seen = Arc::new(Seen::default());
        let stop = Arc::new(AtomicBool::new(false));
        let accept = {
            let (seen, stop) = (seen.clone(), stop.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            stream.set_nonblocking(false).unwrap();
                            seen.connections.fetch_add(1, Ordering::SeqCst);
                            seen.open.lock().unwrap().push(stream.try_clone().unwrap());
                            let (seen, mode) = (seen.clone(), mode.clone());
                            std::thread::spawn(move || connection(stream, mode, seen));
                        }
                        Err(_) => sleep(Duration::from_millis(10)),
                    }
                }
            })
        };
        Server { path: path.to_owned(), seen, stop, accept: Some(accept) }
    }

    fn connections(&self) -> usize {
        self.seen.connections.load(Ordering::SeqCst)
    }

    fn hellos(&self) -> Vec<String> {
        self.seen.hellos.lock().unwrap().clone()
    }

    fn lines(&self) -> Vec<String> {
        self.seen.lines.lock().unwrap().clone()
    }

    /// The lines whose `method` is `method`.
    fn got(&self, method: &str) -> Vec<Value> {
        self.lines().iter().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter(|m| m["method"] == method).collect()
    }

    /// A line of Hersir's own, on every connection.
    fn say(&self, line: &str) {
        for s in self.seen.open.lock().unwrap().iter_mut() {
            s.write_all(format!("{line}\n").as_bytes()).unwrap();
        }
    }

    /// Hersir quits: its connections close and its socket goes away.
    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.accept.take() {
            t.join().unwrap();
        }
        for s in self.seen.open.lock().unwrap().iter() {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}

/// One connection to the stand-in: the hello, then a line per request. Pings get `{}`; a request whose id starts with
/// "hold" never gets an answer; one whose id starts with "late" gets it after 300 ms; every other request gets
/// `answer_line`.
fn connection(stream: UnixStream, mode: Mode, seen: Arc<Seen>) {
    let mut w = stream.try_clone().unwrap();
    let mut r = BufReader::new(stream);
    let mut hello = String::new();
    if r.read_line(&mut hello).unwrap_or(0) == 0 {
        return;
    }
    seen.hellos.lock().unwrap().push(hello);
    if let Mode::Refuse(why) = mode {
        let _ = w.write_all(format!("{}\n", json!({"error": why})).as_bytes());
        let _ = w.shutdown(std::net::Shutdown::Both);
        return;
    }
    loop {
        let mut line = String::new();
        match r.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        seen.lines.lock().unwrap().push(line.clone());
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        let (Some(id), Some(method)) = (msg.get("id"), msg.get("method").and_then(Value::as_str)) else { continue };
        let tag = id.as_str().unwrap_or_default();
        if tag.starts_with("hold") {
            continue;
        }
        let answer = if method == "ping" { format!("{}\n", json!({"jsonrpc": "2.0", "id": id, "result": {}})) } else { answer_line(id, method) };
        if tag.starts_with("late") {
            let mut w = w.try_clone().unwrap();
            std::thread::spawn(move || {
                sleep(Duration::from_millis(300));
                let _ = w.write_all(answer.as_bytes());
            });
            continue;
        }
        if w.write_all(answer.as_bytes()).is_err() {
            break;
        }
    }
}

/// The stand-in's answer, spaced and ordered as serde_json never writes it: a shim that parsed it and wrote it again
/// would show.
fn answer_line(id: &Value, method: &str) -> String {
    format!("{{ \"result\" : {{\"from\": \"stand-in\", \"method\": \"{method}\"}}, \"id\" : {id}, \"jsonrpc\" : \"2.0\" }}\n")
}

// ---- helpers -------------------------------------------------------------------------------------------------------

fn wait_until(within: Duration, mut ok: impl FnMut() -> bool) -> bool {
    let t0 = Instant::now();
    while t0.elapsed() < within {
        if ok() {
            return true;
        }
        sleep(Duration::from_millis(20));
    }
    ok()
}

/// The instructions Hersir sends in its own `initialize` answer.
fn hersir_instructions() -> Value {
    struct NoTools;
    impl gizai_mcp::Tools for NoTools {
        fn list(&self) -> Vec<gizai_mcp::ToolDef> {
            Vec::new()
        }
        async fn call(&self, _: &str, _: Value) -> Result<Value, String> {
            Err("none".into())
        }
    }
    let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
    let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}});
    let answer = rt.block_on(gizai_mcp::handle(&NoTools, &init)).unwrap();
    answer["result"]["instructions"].clone()
}

fn tool_error(text: &str) -> Value {
    json!({"content": [{"type": "text", "text": text}], "isError": true})
}

/// A `tools/list` answer with the shim's own tool alone.
fn assert_only_status(list: &Value) {
    let tools = list["result"]["tools"].as_array().unwrap_or_else(|| panic!("no tools: {list}"));
    assert_eq!(tools.len(), 1, "exactly one tool: {list}");
    let t = &tools[0];
    assert_eq!(t["name"], "hersir_status");
    assert_eq!(t["description"], STATUS_DESCRIPTION);
    assert_eq!(t["annotations"]["readOnlyHint"], true);
    assert_eq!(t["inputSchema"]["type"], "object");
    assert!(t["inputSchema"]["properties"].as_object().is_none_or(|p| p.is_empty()), "no arguments: {t}");
    assert!(t["inputSchema"]["required"].as_array().is_none_or(|r| r.is_empty()), "no arguments: {t}");
}

fn initialize_params() -> Value {
    json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "claude-ai", "version": "0.1.0"}})
}

fn assert_own_initialize(answer: &Value, version: &str) {
    let r = &answer["result"];
    assert_eq!(r["protocolVersion"], version, "{answer}");
    assert_eq!(r["capabilities"], json!({"tools": {"listChanged": true}}), "{answer}");
    assert_eq!(r["serverInfo"], json!({"name": "hersir", "version": env!("CARGO_PKG_VERSION")}), "{answer}");
    assert_eq!(r["instructions"], hersir_instructions(), "Hersir's own instructions: {answer}");
}

// ---- the tests -----------------------------------------------------------------------------------------------------

#[test]
fn without_hersir_the_desktop_shim_answers_by_itself_and_keeps_running() {
    let dir = tempfile::tempdir().unwrap();
    let mut shim = Desktop::start(&dir.path().join("missing.sock"));

    // initialize: the shim's own, with the client's version, else the default
    let init = shim.call(1, "initialize", json!({"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "claude-ai"}}));
    assert_own_initialize(&init, "2025-11-25");
    let init = shim.call(2, "initialize", json!({}));
    assert_own_initialize(&init, gizai_mcp::DEFAULT_PROTOCOL);

    // a notification gets no answer: the next line on stdout is the ping's
    shim.notify("notifications/initialized");
    assert_eq!(shim.call(3, "ping", json!({}))["result"], json!({}));

    // tools/list: hersir_status alone
    assert_only_status(&shim.call(4, "tools/list", json!({})));

    // every tools/call: isError with the reason, word for word, hersir_status too
    assert_eq!(shim.call_tool(5, "get_overview")["result"], tool_error(NOT_RUNNING));
    assert_eq!(shim.call_tool("s-6", "hersir_status")["result"], tool_error(NOT_RUNNING));

    // anything else: method not found, as `handle` answers it; a line that isn't JSON: a parse error
    assert_eq!(shim.call(7, "resources/list", json!({}))["error"]["code"], -32601);
    shim.send("this is not json");
    let parse = shim.next_value(PATIENCE);
    assert_eq!((parse["id"].clone(), parse["error"]["code"].clone()), (Value::Null, json!(-32700)), "{parse}");

    // still running after 5 s, and still answering
    sleep(Duration::from_secs(5).saturating_sub(shim.started.elapsed()));
    assert!(shim.running(), "the shim ended without Hersir");
    assert_eq!(shim.call(8, "ping", json!({}))["result"], json!({}));

    // one line on stderr for the one state it was in, though it tried the socket several times
    assert_eq!(shim.stderr(), WAITING_LINE);
    assert_eq!(shim.close(), Some(0));
    assert_no_lines_but_answers(&shim);
    shim.assert_no_token();
}

/// Nothing on stdout but answers to the test's own requests (no list_changed, no answer to the shim's own ping).
fn assert_no_lines_but_answers(shim: &Desktop) {
    for line in &shim.seen {
        let v: Value = serde_json::from_str(line).unwrap();
        assert!(v.get("method").is_none(), "only answers on stdout: {line}");
    }
}

#[test]
fn the_desktop_shim_connects_when_hersir_opens_after_it_and_tells_claude_desktop_to_fetch_the_tools() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("h.sock");
    let mut shim = Desktop::start(&sock);
    assert_own_initialize(&shim.call(1, "initialize", initialize_params()), "2025-06-18");
    shim.notify("notifications/initialized");

    // Hersir opens 1 s after the shim
    sleep(Duration::from_secs(1).saturating_sub(shim.started.elapsed()));
    let server = Server::start(&sock, Mode::Answer);
    let opened = Instant::now();

    // within 3 s: the hello with the token, and list_changed on stdout
    assert!(wait_until(Duration::from_secs(3), || !server.hellos().is_empty()), "no hello within 3 s of Hersir opening");
    assert_eq!(gizai_mcp::parse_hello(&server.hellos()[0]).as_deref(), Some(TOKEN));
    let line = shim.next_line(Duration::from_secs(3).saturating_sub(opened.elapsed()));
    assert_eq!(line.as_deref(), Some(LIST_CHANGED), "list_changed within 3 s of Hersir opening");

    // the shim's own ping reached Hersir with an id no MCP request has, and its answer stayed with the shim
    let pings = server.got("ping");
    assert_eq!(pings.len(), 1, "{:?}", server.lines());
    let id = pings[0].get("id").expect("the check is a request, with an id");
    assert!(!id.is_string() && !id.is_number(), "an id the client can't use: {id}");

    // the next tools/list is Hersir's, unchanged
    shim.send(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
    assert_eq!(shim.next_line(PATIENCE).as_deref(), Some(answer_line(&json!(2), "tools/list").trim_end()));
    assert_eq!(server.got("tools/list").len(), 1);

    assert_eq!(server.connections(), 1);
    assert_eq!(shim.stderr(), format!("{WAITING_LINE}{CONNECTED_LINE}"));
    assert_eq!(shim.close(), Some(0));
    assert!(shim.seen.iter().all(|l| !l.contains("\"id\":null")), "the check's answer never reaches stdout: {:?}", shim.seen);
    shim.assert_no_token();
}

#[test]
fn the_desktop_shim_tries_hersir_again_before_a_request_without_waiting_for_the_next_try() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("h.sock");
    let mut shim = Desktop::start(&sock);
    assert_own_initialize(&shim.call(1, "initialize", initialize_params()), "2025-06-18");
    assert_only_status(&shim.call(2, "tools/list", json!({})));

    // Hersir opens, and a request comes well before the shim's next try (2 s after its first)
    let _server = Server::start(&sock, Mode::Answer);
    assert!(shim.started.elapsed() < Duration::from_millis(1500), "the test machine is too slow to tell the try before a request from the next try");
    shim.send(r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#);
    assert_eq!(shim.next_line(PATIENCE).as_deref(), Some(LIST_CHANGED));
    assert_eq!(shim.next_line(PATIENCE).as_deref(), Some(answer_line(&json!(3), "tools/list").trim_end()), "the request went to Hersir");
    assert_eq!(shim.close(), Some(0));
    shim.assert_no_token();
}

#[test]
fn the_connected_desktop_shim_passes_lines_on_unchanged_but_answers_initialize_and_hersir_status_itself() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("h.sock");
    let server = Server::start(&sock, Mode::Answer);
    let mut shim = Desktop::start(&sock);

    // initialize: the shim's own (the first line on stdout: no list_changed for a connection made as it starts)
    assert_own_initialize(&shim.call(1, "initialize", initialize_params()), "2025-06-18");
    shim.notify("notifications/initialized");
    assert!(wait_until(PATIENCE, || server.connections() == 1 && !server.hellos().is_empty()), "the shim connected as it started");
    assert_eq!(gizai_mcp::parse_hello(&server.hellos()[0]).as_deref(), Some(TOKEN));

    // requests reach Hersir byte for byte, and its answers come back unchanged
    let call = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_overview","arguments":{"project":"GA"}}}"#;
    shim.send(call);
    assert_eq!(shim.next_line(PATIENCE).as_deref(), Some(answer_line(&json!(2), "tools/call").trim_end()));
    shim.send(r#"{"jsonrpc":"2.0","id":"abc","method":"tools/list"}"#);
    assert_eq!(shim.next_line(PATIENCE).as_deref(), Some(answer_line(&json!("abc"), "tools/list").trim_end()));
    assert_eq!(shim.call(3, "ping", json!({}))["result"], json!({}));

    // hersir_status: the shim's own, connected
    assert_eq!(shim.call_tool(4, "hersir_status")["result"], json!({"content": [{"type": "text", "text": CONNECTED}], "isError": false}));

    // a line Hersir sends by itself comes through unchanged too
    let own = r#"{ "jsonrpc":"2.0", "method":"notifications/message", "params":{"level":"info","data":"hi"} }"#;
    server.say(own);
    assert_eq!(shim.next_line(PATIENCE).as_deref(), Some(own));

    // what Hersir got: the client's lines as they were, never initialize and never hersir_status
    let lines = server.lines();
    assert!(lines.contains(&format!("{call}\n")), "{lines:?}");
    assert!(server.got("initialize").is_empty(), "initialize never reaches Hersir: {lines:?}");
    assert!(lines.iter().all(|l| !l.contains("hersir_status")), "hersir_status never reaches Hersir: {lines:?}");
    assert_eq!(server.got("notifications/initialized").len(), 1, "a notification passes on: {lines:?}");
    // the check ping, and the test's own
    assert_eq!(server.got("ping").len(), 2, "{lines:?}");

    // stdin closes while an answer is still on its way: it still comes, and the shim ends with 0
    shim.send(r#"{"jsonrpc":"2.0","id":"late-1","method":"tools/list"}"#);
    assert!(wait_until(PATIENCE, || server.lines().iter().any(|l| l.contains("late-1"))));
    assert_eq!(shim.close(), Some(0));
    assert_eq!(shim.seen.last().map(String::as_str), Some(answer_line(&json!("late-1"), "tools/list").trim_end()));
    assert!(!shim.seen.iter().any(|l| l == LIST_CHANGED), "no list_changed: Claude Desktop had no tools yet when it connected");

    assert_eq!(server.connections(), 1);
    assert_eq!(shim.stderr(), CONNECTED_LINE);
    shim.assert_no_token();
}

#[test]
fn after_hersir_refuses_the_token_the_desktop_shim_says_why_and_never_dials_again() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("h.sock");
    let server = Server::start(&sock, Mode::Refuse(REASON.into()));
    let mut shim = Desktop::start(&sock);

    assert_own_initialize(&shim.call(1, "initialize", initialize_params()), "2025-06-18");
    assert!(wait_until(PATIENCE, || server.hellos().len() == 1));
    assert_eq!(gizai_mcp::parse_hello(&server.hellos()[0]).as_deref(), Some(TOKEN));

    // every tools/call: isError with the reason, word for word
    assert_eq!(shim.call_tool(2, "get_overview")["result"], tool_error(REFUSED));
    assert_eq!(shim.call_tool(3, "hersir_status")["result"], tool_error(REFUSED));
    assert_only_status(&shim.call(4, "tools/list", json!({})));
    assert_eq!(shim.call(5, "ping", json!({}))["result"], json!({}));

    // more than two tries' time later, with a request too: still running, still one connection
    sleep(Duration::from_millis(5500).saturating_sub(shim.started.elapsed()));
    assert_eq!(shim.call_tool(6, "list_cards")["result"], tool_error(REFUSED));
    assert!(shim.running(), "the shim ended after the refusal");
    assert_eq!(server.connections(), 1, "the shim dialed again after a refusal");

    assert_eq!(shim.stderr(), format!("hersir-mcp: Hersir refused the token: {REASON}\n"));
    assert_eq!(shim.close(), Some(0));
    assert_no_lines_but_answers(&shim);
    shim.assert_no_token();
}

#[test]
fn a_refusal_that_names_the_token_does_not_put_it_on_stderr() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("h.sock");
    let _server = Server::start(&sock, Mode::Refuse(format!("this token is not valid: {TOKEN} was never issued")));
    let mut shim = Desktop::start(&sock);
    assert_eq!(shim.call_tool(1, "get_overview")["result"], tool_error(REFUSED));
    assert_eq!(shim.close(), Some(0));
    let err = shim.stderr();
    assert!(err.starts_with("hersir-mcp: Hersir refused the token: this token is not valid:"), "{err}");
    assert_eq!(err.lines().count(), 1, "{err}");
    shim.assert_no_token();
}

#[test]
fn the_desktop_shim_answers_waiting_requests_at_once_when_hersir_closes_and_connects_again_when_it_is_back() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("h.sock");
    let mut server = Server::start(&sock, Mode::Answer);
    let mut shim = Desktop::start(&sock);
    assert_own_initialize(&shim.call(1, "initialize", initialize_params()), "2025-06-18");
    shim.notify("notifications/initialized");

    // a tools/call and another request wait for Hersir when it quits
    shim.send(r#"{"jsonrpc":"2.0","id":"hold-call","method":"tools/call","params":{"name":"get_overview","arguments":{}}}"#);
    shim.send(r#"{"jsonrpc":"2.0","id":"hold-read","method":"resources/read","params":{"uri":"hersir://board"}}"#);
    assert!(wait_until(PATIENCE, || server.lines().iter().filter(|l| l.contains("\"hold-")).count() == 2), "{:?}", server.lines());
    server.stop();
    let closed = Instant::now();

    // both get their answer within 1 s
    let call = shim.next_value(Duration::from_secs(1));
    let other = shim.next_value(Duration::from_secs(1).saturating_sub(closed.elapsed()));
    assert!(closed.elapsed() <= Duration::from_secs(1), "answered {:?} after Hersir closed", closed.elapsed());
    assert_eq!(call, json!({"jsonrpc": "2.0", "id": "hold-call", "result": tool_error(NOT_RUNNING)}));
    assert_eq!(other["id"], "hold-read", "{other}");
    assert_eq!(other["error"]["message"], NOT_RUNNING, "a JSON-RPC error with the reason: {other}");
    assert!(other["error"]["code"].is_i64() && other.get("result").is_none(), "{other}");

    // while Hersir is gone, the shim answers itself
    assert_eq!(shim.call_tool(5, "get_overview")["result"], tool_error(NOT_RUNNING));
    assert_only_status(&shim.call(6, "tools/list", json!({})));
    assert!(shim.running());

    // Hersir is back: at the next try (2 s after it closed) the shim connects, tells Claude Desktop, and passes on
    let server = Server::start(&sock, Mode::Answer);
    assert_eq!(shim.next_line(Duration::from_secs(4)).as_deref(), Some(LIST_CHANGED), "no list_changed after Hersir came back");
    assert!(closed.elapsed() < Duration::from_secs(4), "connected {:?} after Hersir closed", closed.elapsed());
    assert_eq!(server.hellos().len(), 1);
    assert_eq!(gizai_mcp::parse_hello(&server.hellos()[0]).as_deref(), Some(TOKEN));
    shim.send(r#"{"jsonrpc":"2.0","id":7,"method":"tools/list"}"#);
    assert_eq!(shim.next_line(PATIENCE).as_deref(), Some(answer_line(&json!(7), "tools/list").trim_end()));
    assert_eq!(shim.call_tool(8, "hersir_status")["result"]["content"][0]["text"], CONNECTED);

    assert_eq!(shim.stderr(), format!("{CONNECTED_LINE}{WAITING_LINE}{CONNECTED_LINE}"));
    assert_eq!(shim.close(), Some(0));
    shim.assert_no_token();
}

#[test]
fn the_desktop_shim_still_needs_its_environment() {
    let out = Command::new(SHIM).arg("--desktop")
        .env_remove("HERSIR_SOCKET").env_remove("HERSIR_TOKEN").env_remove("GIZAI_SOCKET").env_remove("GIZAI_TOKEN")
        .stdin(Stdio::null()).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.starts_with("hersir-mcp:") && err.contains("HERSIR_SOCKET") && err.contains("HERSIR_TOKEN"), "{err}");
    assert!(out.stdout.is_empty());
}

// ---- a token revoked on an open connection (GA-338's serve_while) --------------------------------------------------

/// What the stand-in board's one tool answers.
const BOARD: &str = "KADE-1 Export invoices as CSV";

/// Hersir as it serves Claude Desktop's lasting token since GA-338: the hello, then Hersir's own loop,
/// `gizai_mcp::serve_while`, which asks before each message whether the token still holds. Once revoked (Disconnect in
/// Hersir, or a new Connect), the next message gets one `{"error": …}` line instead of its answer and the connection
/// closes; a new connection with that token is refused at its hello.
struct Revocable {
    revoked: Arc<AtomicBool>,
    connections: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    accept: Option<JoinHandle<()>>,
}

/// The stand-in board: one tool that reads.
struct Board;

impl gizai_mcp::Tools for Board {
    fn list(&self) -> Vec<gizai_mcp::ToolDef> {
        vec![gizai_mcp::ToolDef { name: "get_overview".into(), description: "The board.".into(),
            input_schema: json!({"type": "object", "properties": {}}), read_only: true }]
    }
    async fn call(&self, name: &str, _: Value) -> Result<Value, String> {
        if name == "get_overview" { Ok(Value::String(BOARD.into())) } else { Err(format!("unknown tool {name}")) }
    }
}

impl Revocable {
    fn start(path: &Path) -> Revocable {
        let listener = UnixListener::bind(path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let (revoked, connections, stop) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicUsize::new(0)), Arc::new(AtomicBool::new(false)));
        let accept = {
            let (revoked, connections, stop) = (revoked.clone(), connections.clone(), stop.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            connections.fetch_add(1, Ordering::SeqCst);
                            let revoked = revoked.clone();
                            std::thread::spawn(move || serve_revocable(stream, revoked));
                        }
                        Err(_) => sleep(Duration::from_millis(10)),
                    }
                }
            })
        };
        Revocable { revoked, connections, stop, accept: Some(accept) }
    }

    /// Disconnect in Hersir: the token is revoked, the open connection stays until its next message.
    fn revoke(&self) {
        self.revoked.store(true, Ordering::SeqCst);
    }

    fn connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }
}

impl Drop for Revocable {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.accept.take() {
            let _ = t.join();
        }
    }
}

fn serve_revocable(stream: UnixStream, revoked: Arc<AtomicBool>) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async move {
        stream.set_nonblocking(true).unwrap();
        let (r, mut w) = tokio::net::UnixStream::from_std(stream).unwrap().into_split();
        let mut r = tokio::io::BufReader::new(r);
        let mut hello = String::new();
        if r.read_line(&mut hello).await.unwrap_or(0) == 0 {
            return;
        }
        if gizai_mcp::parse_hello(&hello).as_deref() != Some(TOKEN) || revoked.load(Ordering::SeqCst) {
            let _ = w.write_all(format!("{}\n", json!({"error": REASON})).as_bytes()).await;
            let _ = w.shutdown().await;
            return;
        }
        let refused = || revoked.load(Ordering::SeqCst).then(|| REASON.to_string());
        let _ = gizai_mcp::serve_while(r, w, &Board, refused).await;
    });
}

/// Starts the shim against `hersir`, initializes it, and checks that Hersir's own tool answers through it.
fn connected_to(sock: &Path, hersir: &Revocable) -> Desktop {
    let mut shim = Desktop::start(sock);
    assert_own_initialize(&shim.call(1, "initialize", initialize_params()), "2025-06-18");
    shim.notify("notifications/initialized");
    assert!(wait_until(PATIENCE, || hersir.connections() == 1), "the shim connected as it started");
    let list = shim.call(2, "tools/list", json!({}));
    assert_eq!(list["result"]["tools"][0]["name"], "get_overview", "Hersir's own list: {list}");
    let read = shim.call_tool(3, "get_overview");
    assert_eq!(read["result"], json!({"content": [{"type": "text", "text": BOARD}], "isError": false}));
    shim
}

#[test]
fn when_hersir_revokes_the_token_on_an_open_connection_a_waiting_tools_call_says_hersir_disconnected_it_and_the_shim_stops_trying() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("h.sock");
    let hersir = Revocable::start(&sock);
    let mut shim = connected_to(&sock, &hersir);

    // Disconnect in Hersir; the next tools/call is the first message Hersir gets after it, and is refused in its place
    hersir.revoke();
    let asked = Instant::now();
    shim.send(r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"get_overview","arguments":{}}}"#);
    let answer = shim.next_value(PATIENCE);
    assert!(asked.elapsed() < Duration::from_secs(1), "answered {:?} after it was asked", asked.elapsed());
    assert_eq!(answer, json!({"jsonrpc": "2.0", "id": 4, "result": tool_error(REFUSED)}), "the card's refusal text, not Hersir's line");

    // from then on the shim answers by itself, with the refusal, and never dials again (more than two tries' time)
    assert_eq!(shim.call_tool(5, "get_overview")["result"], tool_error(REFUSED));
    assert_eq!(shim.call_tool(6, "hersir_status")["result"], tool_error(REFUSED));
    assert_only_status(&shim.call(7, "tools/list", json!({})));
    sleep(Duration::from_millis(4500));
    assert_eq!(shim.call_tool(8, "list_tasks")["result"], tool_error(REFUSED));
    assert!(shim.running(), "the shim ended after the refusal");
    assert_eq!(hersir.connections(), 1, "the shim dialed again after a refusal");

    // Hersir's refusal line never reaches Claude Desktop; stderr has one line per state, with Hersir's reason
    assert!(shim.seen.iter().all(|l| !l.contains("this token is not valid") && !l.contains(r#""error":"#)), "{:?}", shim.seen);
    assert!(shim.seen.iter().all(|l| l != LIST_CHANGED), "{:?}", shim.seen);
    assert_eq!(shim.stderr(), format!("{CONNECTED_LINE}hersir-mcp: Hersir refused the token: {REASON}\n"));
    assert_eq!(shim.close(), Some(0));
    shim.assert_no_token();
}

#[test]
fn when_hersir_revokes_the_token_on_an_open_connection_a_waiting_request_that_is_no_tools_call_gets_a_json_rpc_error() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("h.sock");
    let hersir = Revocable::start(&sock);
    let mut shim = connected_to(&sock, &hersir);

    hersir.revoke();
    let asked = Instant::now();
    shim.send(r#"{"jsonrpc":"2.0","id":"list-4","method":"tools/list"}"#);
    let answer = shim.next_value(PATIENCE);
    assert!(asked.elapsed() < Duration::from_secs(1), "answered {:?} after it was asked", asked.elapsed());
    assert_eq!(answer["id"], "list-4", "{answer}");
    assert_eq!(answer["error"]["message"], REFUSED, "a JSON-RPC error with the refusal: {answer}");
    assert!(answer["error"]["code"].is_i64() && answer.get("result").is_none(), "{answer}");

    // and a later tools/list is the shim's own
    assert_only_status(&shim.call(5, "tools/list", json!({})));
    assert_eq!(hersir.connections(), 1);
    assert_eq!(shim.close(), Some(0));
    shim.assert_no_token();
}
