// Linux and macOS only: the Unix socket, with Python scripts as fake CLIs. Windows: mcp_pipe_test.rs.
#![cfg(unix)]
// The MCP socket: a turn's token opens Gizai's tools; anything else is turned away.
use std::path::Path;

use gizai_core::model::AgentInput;
use gizai_core::{team, tokens};
use gizai_lib::{AppState, mcp, tools};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

fn lead(st: &AppState) -> String {
    let team_id = team::list(&st.db).unwrap()[0].id.clone();
    team::add_agent(&st.db, &st.you_id, &team_id, AgentInput { name: "Team Lead".into(), role_key: "lead".into(), chat_enabled: Some(true), ..Default::default() }).unwrap()
}

async fn session(path: &Path, hello: &str, requests: &[Value]) -> Vec<Value> {
    let stream = UnixStream::connect(path).await.unwrap();
    let (r, mut w) = stream.into_split();
    w.write_all(hello.as_bytes()).await.unwrap();
    for q in requests {
        w.write_all(format!("{q}\n").as_bytes()).await.unwrap();
    }
    w.shutdown().await.unwrap();
    let mut lines = BufReader::new(r).lines();
    let mut out = vec![];
    while let Ok(Ok(Some(l))) = tokio::time::timeout(std::time::Duration::from_secs(5), lines.next_line()).await {
        out.push(serde_json::from_str(&l).unwrap());
    }
    out
}

#[test]
fn socket_path_depends_on_the_data_dir() {
    let run = Path::new("/run/user/1000");
    let a = mcp::socket_path_in(Some(run), Path::new("/home/x/.local/share/gizai"));
    let b = mcp::socket_path_in(Some(run), Path::new("/home/x/Herd/gizai/.devdata/uitest"));
    assert_ne!(a, b);
    assert!(a.starts_with("/run/user/1000/gizai") && b.starts_with("/run/user/1000/gizai"), "{a:?} {b:?}");
    assert_eq!(a, mcp::socket_path_in(Some(run), Path::new("/home/x/.local/share/gizai")));
    assert!(a.to_string_lossy().len() < 100);
    assert_eq!(mcp::socket_path_in(None, Path::new("/d")), Path::new("/d/mcp.sock"));
}

#[tokio::test]
async fn a_valid_token_gets_tools() {
    let dir = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(dir.path());
    let lead = lead(&st);
    let _server = mcp::start(&st).unwrap();
    let tok = tokens::mint(&st.db, &lead, json!({"chat": "T"}), 60_000).unwrap();
    let out = session(&st.mcp_socket, &gizai_mcp::hello_line(&tok), &[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "get_overview", "arguments": {}}}),
    ]).await;
    assert_eq!(out.len(), 3, "{out:?}");
    assert_eq!(out[0]["result"]["serverInfo"]["name"], "hersir", "GA-338");
    assert_eq!(out[1]["result"]["tools"].as_array().unwrap().len(), tools::catalog().len());
    assert_eq!(out[2]["result"]["isError"], false, "{}", out[2]);
    let overview: Value = serde_json::from_str(out[2]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(overview["agents"][0]["name"], "Team Lead");
}

#[tokio::test]
async fn a_bad_or_revoked_token_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(dir.path());
    let lead = lead(&st);
    let _server = mcp::start(&st).unwrap();
    let ping = json!({"jsonrpc": "2.0", "id": 1, "method": "ping"});
    let out = session(&st.mcp_socket, &gizai_mcp::hello_line("nope"), &[ping.clone()]).await;
    assert_eq!(out.len(), 1, "{out:?}");
    assert!(out[0]["error"].as_str().unwrap().contains("token"));
    let tok = tokens::mint(&st.db, &lead, json!({}), 60_000).unwrap();
    tokens::revoke(&st.db, &tok).unwrap();
    let out = session(&st.mcp_socket, &gizai_mcp::hello_line(&tok), &[ping.clone()]).await;
    assert!(out[0]["error"].is_string(), "{out:?}");
    let out = session(&st.mcp_socket, "{\"hello\": true}\n", &[ping]).await;
    assert!(out[0]["error"].is_string(), "{out:?}");
}

#[tokio::test]
async fn a_stale_socket_file_is_replaced_and_kept_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(dir.path());
    let lead = lead(&st);
    drop(std::os::unix::net::UnixListener::bind(&st.mcp_socket).unwrap()); // leaves the file behind
    assert!(st.mcp_socket.exists());
    let _server = mcp::start(&st).unwrap();
    let mode = std::fs::metadata(&st.mcp_socket).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let tok = tokens::mint(&st.db, &lead, json!({}), 60_000).unwrap();
    let out = session(&st.mcp_socket, &gizai_mcp::hello_line(&tok), &[json!({"jsonrpc": "2.0", "id": 9, "method": "ping"})]).await;
    assert_eq!(out[0]["id"], 9);
}

#[tokio::test]
async fn a_regular_file_in_the_way_is_not_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(dir.path());
    std::fs::write(&st.mcp_socket, "not a socket").unwrap();
    assert!(mcp::start(&st).is_err());
    assert_eq!(std::fs::read_to_string(&st.mcp_socket).unwrap(), "not a socket");
}

#[test]
fn a_long_data_dir_still_gets_a_usable_socket_path() {
    let long = Path::new("/tmp").join("x".repeat(120));
    let p = mcp::socket_path_in(None, &long);
    assert!(p.to_string_lossy().len() < 100, "{p:?}");
    assert_ne!(p, mcp::socket_path_in(None, &Path::new("/tmp").join("y".repeat(120))), "still one per data dir");
    let deep = Path::new("/run/user/1000").join("z".repeat(120));
    assert!(mcp::socket_path_in(Some(&deep), Path::new("/d")).to_string_lossy().len() < 100);
}

#[tokio::test]
async fn start_binds_a_shortened_path_for_a_long_data_dir() {
    let dir = tempfile::tempdir().unwrap();
    let st0 = gizai_lib::test_state(dir.path());
    let mut st = st0.clone();
    st.mcp_socket = mcp::socket_path_in(None, &dir.path().join("w".repeat(110)));
    let _server = mcp::start(&st).unwrap();
    assert!(UnixStream::connect(&st.mcp_socket).await.is_ok());
    mcp::remove_socket(&st);
    let _ = std::fs::remove_dir(st.mcp_socket.parent().unwrap()); // only when empty
}

// ---- GA-266: a turn's token works only for that turn's own process tree ----

/// A token whose scope names a run, as a chat answer's does (a board check's and a question's name one too).
fn turn_token(st: &AppState, actor: &str, run: &str) -> String {
    tokens::mint(&st.db, actor, json!({"chat": "T", "run": run}), 60_000).unwrap()
}

const PING: &str = r#"{"jsonrpc": "2.0", "id": 9, "method": "ping"}"#;

/// Stands in for the turn's Claude Code with Hersir's MCP shim: connects to the socket, sends the hello line and a
/// ping, and prints the first line it gets back; then again for each line on its stdin, until stdin closes.
const SHIM: &str = r#"
import socket, sys
path, hello, ping = sys.argv[1:4]
while True:
    s = socket.socket(socket.AF_UNIX)
    s.connect(path)
    s.sendall((hello + ping + "\n").encode())
    s.shutdown(socket.SHUT_WR)
    print(s.makefile().readline().strip(), flush=True)
    s.close()
    if not sys.stdin.readline():
        break
"#;

/// The first line the socket answers a connection from this test process (not in any turn's tree) with, and how long
/// that took.
async fn answer(path: &Path, hello: &str) -> (Value, std::time::Duration) {
    let t0 = std::time::Instant::now();
    let stream = UnixStream::connect(path).await.unwrap();
    let (r, mut w) = stream.into_split();
    w.write_all(format!("{hello}{PING}\n").as_bytes()).await.unwrap();
    w.shutdown().await.unwrap();
    let mut lines = BufReader::new(r).lines();
    let line = tokio::time::timeout(std::time::Duration::from_secs(30), lines.next_line()).await.unwrap().unwrap().unwrap();
    (serde_json::from_str(&line).unwrap(), t0.elapsed())
}

/// The next line the stand-in shim printed.
async fn next_json(out: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>) -> Value {
    let l = tokio::time::timeout(std::time::Duration::from_secs(30), out.next_line()).await.unwrap().unwrap().unwrap();
    serde_json::from_str(&l).unwrap()
}

fn not_its_turn(v: &Value) -> bool {
    v["error"].as_str().is_some_and(|e| e.contains("works only for the turn it was made for"))
}

/// A process tree as a run's Claude Code gets one (`os::spawn_tree_tokio`), that only waits.
fn other_tree() -> (tokio::process::Child, gizai_agents::os::Tree) {
    let mut cmd = tokio::process::Command::new("sleep");
    cmd.arg("60").stdin(std::process::Stdio::null());
    gizai_agents::os::spawn_tree_tokio(&mut cmd).unwrap()
}

#[tokio::test]
async fn a_turns_token_works_from_its_own_process_tree_and_is_refused_from_outside_it() {
    let dir = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(dir.path());
    let _server = mcp::start(&st).unwrap();
    let tok = turn_token(&st, &lead(&st), "R1");
    let hello = gizai_mcp::hello_line(&tok);
    // before the token goes out
    mcp::expect_turn(&st, "R1");
    let mut cmd = tokio::process::Command::new("python3");
    cmd.args(["-c", SHIM]).arg(&st.mcp_socket).arg(&hello).arg(PING)
        .stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped());
    let (mut child, tree) = gizai_agents::os::spawn_tree_tokio(&mut cmd).unwrap();
    // its connection waits while Hersir doesn't know the turn's tree yet
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    mcp::bind_turn(&st, "R1", tree.clone());
    let mut out = BufReader::new(child.stdout.take().unwrap()).lines();
    let own = next_json(&mut out).await;
    assert_eq!(own["id"], 9, "the turn's own process gets the tools: {own}");

    // another process of the same user with the same valid token: refused at once
    let (outside, took) = answer(&st.mcp_socket, &hello).await;
    assert!(not_its_turn(&outside), "{outside}");
    assert!(took < std::time::Duration::from_secs(5), "took {took:?}");

    // once the turn has ended, its own process is refused too
    mcp::end_turn(&st, "R1");
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(b"again\n").await.unwrap();
    let after = next_json(&mut out).await;
    assert!(not_its_turn(&after), "{after}");
    drop(stdin);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), child.wait()).await;
    tree.end(gizai_agents::os::End::Kill);
}

#[tokio::test]
async fn a_turns_token_works_for_no_one_when_hersir_doesnt_know_the_turn() {
    let dir = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(dir.path());
    let _server = mcp::start(&st).unwrap();
    let lead = lead(&st);
    let (v, took) = answer(&st.mcp_socket, &gizai_mcp::hello_line(&turn_token(&st, &lead, "never-started"))).await;
    assert!(not_its_turn(&v), "{v}");
    assert!(took < std::time::Duration::from_secs(5), "took {took:?}");
    // a token whose scope names no run (Claude Desktop's, a plain chat token) works as before
    let tok = tokens::mint(&st.db, &lead, json!({"chat": "T"}), 60_000).unwrap();
    let (v, _) = answer(&st.mcp_socket, &gizai_mcp::hello_line(&tok)).await;
    assert_eq!(v["id"], 9, "{v}");
}

#[tokio::test]
async fn a_connection_that_waits_for_its_turn_is_refused_once_the_tree_is_known_and_it_isnt_in_it() {
    let dir = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(dir.path());
    let _server = mcp::start(&st).unwrap();
    let hello = gizai_mcp::hello_line(&turn_token(&st, &lead(&st), "R2"));
    mcp::expect_turn(&st, "R2");
    let path = st.mcp_socket.clone();
    let waiting = tokio::spawn(async move { answer(&path, &hello).await });
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert!(!waiting.is_finished(), "it waits while the turn's Claude Code starts");
    let (mut sleeper, tree) = other_tree();
    mcp::bind_turn(&st, "R2", tree.clone());
    let (v, took) = waiting.await.unwrap();
    assert!(not_its_turn(&v), "{v}");
    assert!(took < std::time::Duration::from_secs(5), "refused once the tree was known, not after the wait: {took:?}");
    tree.end(gizai_agents::os::End::Kill);
    let _ = sleeper.wait().await;
}

#[tokio::test]
async fn a_connection_that_waits_for_its_turn_is_refused_when_the_turn_ends_or_its_cli_never_starts() {
    let dir = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(dir.path());
    let _server = mcp::start(&st).unwrap();
    let lead = lead(&st);
    // the turn ends (its Claude Code failed to start) while the connection waits
    let hello = gizai_mcp::hello_line(&turn_token(&st, &lead, "R3"));
    mcp::expect_turn(&st, "R3");
    let path = st.mcp_socket.clone();
    let waiting = tokio::spawn(async move { answer(&path, &hello).await });
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    mcp::end_turn(&st, "R3");
    let (v, took) = waiting.await.unwrap();
    assert!(not_its_turn(&v), "{v}");
    assert!(took < std::time::Duration::from_secs(5), "took {took:?}");
    // the turn's tree never comes: refused after the wait (10 s)
    let hello = gizai_mcp::hello_line(&turn_token(&st, &lead, "R4"));
    mcp::expect_turn(&st, "R4");
    let (v, took) = answer(&st.mcp_socket, &hello).await;
    assert!(not_its_turn(&v), "{v}");
    assert!(took >= std::time::Duration::from_secs(9) && took < std::time::Duration::from_secs(20), "took {took:?}");
}
