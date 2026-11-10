// Linux and macOS only: the shim's Unix socket. Windows has a named pipe instead.
#![cfg(unix)]
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixListener;
use std::process::{Command, Stdio};

const SHIM: &str = env!("CARGO_BIN_EXE_hersir-mcp");

/// A server on `sock` for one shim: the hello line and the first request it got, after it answered that request.
fn serve_once(sock: &std::path::Path) -> std::thread::JoinHandle<(String, String)> {
    let listener = UnixListener::bind(sock).unwrap();
    listener.set_nonblocking(true).unwrap();
    std::thread::spawn(move || {
        // A broken shim must fail the test, not hang it.
        let t0 = std::time::Instant::now();
        let stream = loop {
            match listener.accept() {
                Ok((s, _)) => break s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && t0.elapsed().as_secs() < 10 => std::thread::sleep(std::time::Duration::from_millis(20)),
                Err(e) => panic!("the shim never connected: {e}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut hello = String::new();
        reader.read_line(&mut hello).unwrap();
        let mut request = String::new();
        reader.read_line(&mut request).unwrap();
        let mut w = stream;
        w.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n").unwrap();
        (hello, request)
    })
}

/// The shim with only `env` of the socket's and token's four names (GA-249: HERSIR_ and GIZAI_) set.
fn shim(env: &[(&str, &std::ffi::OsStr)]) -> Command {
    let mut c = Command::new(SHIM);
    for n in ["HERSIR_SOCKET", "HERSIR_TOKEN", "GIZAI_SOCKET", "GIZAI_TOKEN"] {
        c.env_remove(n);
    }
    c.envs(env.iter().copied());
    c
}

/// Starts `cmd`, sends one ping, and checks the server got `token` and the shim passed the answer back.
fn ping_through(mut cmd: Command, server: std::thread::JoinHandle<(String, String)>, token: &str) {
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n").unwrap();
    let (hello, request) = server.join().unwrap();
    assert_eq!(gizai_mcp::parse_hello(&hello).as_deref(), Some(token));
    assert!(request.contains("\"ping\""));
    let mut out = String::new();
    child.stdout.take().unwrap().read_to_string(&mut out).unwrap();
    assert_eq!(out, "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n");
    assert!(child.wait().unwrap().success());
}

#[test]
fn pipes_stdio_to_the_socket_after_saying_hello() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("t.sock");
    let server = serve_once(&sock);
    // the names from before 0.17.0 alone still work
    ping_through(shim(&[("GIZAI_SOCKET", sock.as_os_str()), ("GIZAI_TOKEN", "tok-123".as_ref())]), server, "tok-123");
}

#[test]
fn takes_the_socket_and_token_under_their_hersir_names_which_win_over_the_gizai_ones() {
    let dir = tempfile::tempdir().unwrap();
    // HERSIR_ alone
    let sock = dir.path().join("h.sock");
    let server = serve_once(&sock);
    ping_through(shim(&[("HERSIR_SOCKET", sock.as_os_str()), ("HERSIR_TOKEN", "tok-h".as_ref())]), server, "tok-h");
    // both, as Hersir sets them: the HERSIR_ ones win (the GIZAI_ socket isn't there, its token is another)
    let sock = dir.path().join("both.sock");
    let missing = dir.path().join("missing.sock");
    let server = serve_once(&sock);
    ping_through(shim(&[("HERSIR_SOCKET", sock.as_os_str()), ("HERSIR_TOKEN", "tok-new".as_ref()),
        ("GIZAI_SOCKET", missing.as_os_str()), ("GIZAI_TOKEN", "tok-old".as_ref())]), server, "tok-new");
    // an empty HERSIR_ one gives way to its GIZAI_ name
    let sock = dir.path().join("empty.sock");
    let server = serve_once(&sock);
    ping_through(shim(&[("HERSIR_SOCKET", "".as_ref()), ("HERSIR_TOKEN", "".as_ref()),
        ("GIZAI_SOCKET", sock.as_os_str()), ("GIZAI_TOKEN", "tok-old".as_ref())]), server, "tok-old");
}

#[test]
fn says_why_when_gizai_is_not_running() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing.sock");
    let out = shim(&[("GIZAI_SOCKET", missing.as_os_str()), ("GIZAI_TOKEN", "x".as_ref())]).stdin(Stdio::null()).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.starts_with("hersir-mcp:"), "{err}");
    assert!(err.contains("Hersir"), "{err}");
}

#[test]
fn needs_its_environment() {
    let out = shim(&[]).stdin(Stdio::null()).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("HERSIR_SOCKET") && err.contains("GIZAI_SOCKET"), "names both: {err}");
    // a socket without a token is not enough, under either name
    let out = shim(&[("HERSIR_SOCKET", "/nowhere.sock".as_ref())]).stdin(Stdio::null()).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn exits_when_gizai_closes_the_connection_even_with_stdin_open() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("t.sock");
    let listener = UnixListener::bind(&sock).unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut w = stream.try_clone().unwrap();
        let mut hello = String::new();
        BufReader::new(stream).read_line(&mut hello).unwrap();
        w.write_all(b"{\"error\":\"this token is not valid\"}\n").unwrap();
        // dropping both halves closes the connection
    });
    let mut child = shim(&[("HERSIR_SOCKET", sock.as_os_str()), ("HERSIR_TOKEN", "bad".as_ref())])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().unwrap();
    let _keep_stdin_open = child.stdin.take().unwrap();
    server.join().unwrap();
    let t0 = std::time::Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() { break s; }
        if t0.elapsed().as_secs() > 5 { child.kill().unwrap(); panic!("the shim kept running after Hersir closed the connection"); }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(1));
    let mut err = String::new();
    child.stderr.take().unwrap().read_to_string(&mut err).unwrap();
    assert!(err.contains("Hersir closed the connection"), "{err}");
}
