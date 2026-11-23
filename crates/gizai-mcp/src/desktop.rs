//! `hersir-mcp --desktop`: the shim as Claude Desktop runs it (GA-339). Claude Desktop starts it once, when Claude
//! Desktop itself starts, and keeps it running. So unlike a chat turn's shim it doesn't end when Hersir isn't there: it
//! answers by itself and says why, and connects when Hersir opens. It ends, with 0, when stdin ends (after up to a
//! second for Hersir's answers to requests it passed on).
//!
//! Stdin and Hersir's answers are read line by line (newline-delimited JSON-RPC), in one of two states:
//! - **connected:** each line passes on both ways unchanged;
//! - **not connected** (the socket isn't there, Hersir closed the connection, or Hersir refused the token): the shim
//!   answers with a tool of its own, [`STATUS_TOOL`], and every call gets the reason ([`NOT_RUNNING`] or [`REFUSED`])
//!   as a tool's error.
//!
//! In both states the shim answers `initialize` and calls of [`STATUS_TOOL`] itself and never passes them on: Hersir's
//! server keeps no state per connection, so it doesn't need an `initialize`.
//!
//! While Hersir isn't running, the shim tries its socket every 2 seconds and before each request. After the hello line
//! it sends a `ping` of its own and keeps the answer to itself, to learn whether Hersir took the token: Hersir writes
//! nothing after the hello until a request comes, and refuses a token with one `{"error": …}` line and a close. Once
//! Hersir answers, the shim writes [`LIST_CHANGED`], so Claude Desktop fetches Hersir's tools; not when it connects as
//! it starts, before Claude Desktop has been given any tools. A request still waiting for Hersir when the connection
//! drops is answered at once. After a refusal the shim stops trying: a refused token never becomes valid (Connect in
//! Hersir makes a new one, which Claude Desktop reads only when it starts again).
//!
//! Stderr gets one line per change of state, which Claude Desktop keeps in its MCP log. The token goes only to Hersir's
//! socket, never to stdout or stderr. On Windows the socket is Hersir's named pipe, with the checks a chat turn's shim
//! makes: the token goes only to a pipe whose server runs as the user's own, and a busy pipe is tried again.
use std::ffi::{OsStr, OsString};
use std::io::{BufRead, Write};
use std::process::ExitCode;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::{DEFAULT_PROTOCOL, INSTRUCTIONS, ToolDef, Tools, error, handle, hello_line, ok};

/// Every call's answer while Hersir isn't running: the shim can't reach it, or it closed the connection.
pub const NOT_RUNNING: &str =
    "Open Hersir first: Claude Desktop reaches your board through the Hersir app on this computer, and it isn't running.";

/// Every call's answer after Hersir refused the token.
pub const REFUSED: &str = "Hersir disconnected Claude Desktop. To connect it again: in Hersir, Settings → MCP servers → \
Claude Desktop → Connect, then quit and reopen Claude Desktop.";

/// [`STATUS_TOOL`]'s answer while connected.
pub const CONNECTED: &str = "Hersir is open and connected. If its tools aren't listed, quit and reopen Claude Desktop.";

/// The shim's own tool: listed while Hersir can't be reached, and always answered by the shim.
pub const STATUS_TOOL: &str = "hersir_status";

/// [`STATUS_TOOL`]'s description.
pub const STATUS_DESCRIPTION: &str = "Says whether Hersir is open and connected. Call it when the user asks about \
their Hersir board, cards or Chief and no other Hersir tool is listed.";

/// Written to stdout once Hersir took the token, so Claude Desktop fetches Hersir's tools.
pub const LIST_CHANGED: &str = "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/tools/list_changed\"}";

/// How often the shim tries Hersir's socket while Hersir isn't running.
const RETRY: Duration = Duration::from_secs(2);

/// How long Hersir may take to answer the shim's ping after the hello: it only looks the token up first.
const CHECK: Duration = Duration::from_secs(3);

/// After stdin ended: how long Hersir still has to answer the requests passed on to it.
const GRACE: Duration = Duration::from_secs(1);

/// The shim's own ping after the hello. Its id is null, which no MCP client's request has (MCP's request ids are
/// strings or numbers), so its answer is never one that Claude Desktop waits for.
const CHECK_PING: &str = "{\"jsonrpc\":\"2.0\",\"id\":null,\"method\":\"ping\"}\n";

/// The JSON-RPC error code for a request whose connection to Hersir dropped: "connection closed", as the MCP SDKs
/// use it.
const CONNECTION_CLOSED: i64 = -32000;

/// Runs the shim for Claude Desktop until stdin ends, then returns 0.
pub fn run(socket: &OsStr, token: &str) -> ExitCode {
    match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt.block_on(Shim::new(socket, token).run()),
        Err(e) => {
            say(&format!("can't start: {e}"));
            ExitCode::from(1)
        }
    }
}

enum State {
    /// Hersir isn't running, or it closed the connection: tried again every 2 seconds and before each request.
    Waiting,
    Connected(Link),
    /// Hersir refused the token: never tried again.
    Refused,
}

struct Shim {
    socket: OsString,
    token: String,
    state: State,
    /// While waiting: when the next try is due.
    next_try: Instant,
    /// Stderr said already that Hersir isn't running: a line per change of state, not per try.
    said_waiting: bool,
    /// Stdout is gone: Claude Desktop doesn't read any more.
    gone: bool,
}

impl Shim {
    fn new(socket: &OsStr, token: &str) -> Shim {
        Shim {
            socket: socket.to_owned(),
            token: token.to_owned(),
            state: State::Waiting,
            next_try: Instant::now(),
            said_waiting: false,
            gone: false,
        }
    }

    async fn run(mut self) -> ExitCode {
        let mut input = stdin_lines();
        // At the start Claude Desktop hasn't been given any tools yet, so there is nothing to tell it.
        self.dial(false).await;
        self.next_try = Instant::now() + RETRY;
        while !self.gone {
            let waiting = matches!(self.state, State::Waiting);
            let next_try = self.next_try;
            tokio::select! {
                line = input.recv() => match line {
                    Some(line) => self.on_client_line(line).await,
                    None => break,
                },
                line = self.next_from_hersir() => self.on_hersir_line(line),
                () = tokio::time::sleep_until(next_try), if waiting => {
                    self.dial(true).await;
                    self.next_try = Instant::now() + RETRY;
                }
            }
        }
        self.finish().await;
        ExitCode::SUCCESS
    }

    /// Tries Hersir's socket (only while waiting). `tell`: once connected, tell Claude Desktop to fetch the tools.
    async fn dial(&mut self, tell: bool) {
        match open(&self.socket, &self.token).await {
            Dialed::Connected(link) => {
                self.state = State::Connected(link);
                self.said_waiting = false;
                say("connected to Hersir");
                if tell {
                    self.write(LIST_CHANGED.as_bytes());
                }
            }
            Dialed::Refused(why) => self.refused(&why),
            Dialed::NotRunning => self.say_waiting(),
        }
    }

    /// A line from Claude Desktop.
    async fn on_client_line(&mut self, mut line: Vec<u8>) {
        if line.iter().all(u8::is_ascii_whitespace) {
            return;
        }
        if !line.ends_with(b"\n") {
            line.push(b'\n');
        }
        let Ok(msg) = serde_json::from_slice::<Value>(&line) else {
            // Not JSON: Hersir says so itself while connected, as `serve` does.
            if matches!(self.state, State::Connected(_)) {
                self.pass_on(line, None);
            } else {
                self.write_value(&error(Value::Null, -32700, "parse error"));
            }
            return;
        };
        let method = msg.get("method").and_then(Value::as_str);
        // A request has an id; a notification has none and gets no answer.
        let id = method.and(msg.get("id")).cloned();
        if method == Some("initialize") {
            if let Some(id) = id {
                self.write_value(&initialize(id, msg.get("params")));
            }
            return;
        }
        let call = method == Some("tools/call");
        let own = call && msg.pointer("/params/name").and_then(Value::as_str) == Some(STATUS_TOOL);
        if id.is_some() && matches!(self.state, State::Waiting) {
            // Hersir may have opened since the last try.
            self.dial(true).await;
        }
        if !own && matches!(self.state, State::Connected(_)) {
            return self.pass_on(line, id.map(|id| Pending { id, call }));
        }
        let tools = Own { down: self.down() };
        if let Some(answer) = handle(&tools, &msg).await {
            self.write_value(&answer);
        }
    }

    /// Passes a line from Claude Desktop on to Hersir (only while connected); a request waits in `pending` for its
    /// answer.
    fn pass_on(&mut self, line: Vec<u8>, request: Option<Pending>) {
        let State::Connected(link) = &mut self.state else { return };
        link.pending.extend(request);
        if link.to.send(line).is_err() {
            // Hersir stopped taking lines: as good as closed.
            self.lost();
        }
    }

    /// The next line from Hersir, None once the connection ended. Never comes while not connected.
    async fn next_from_hersir(&mut self) -> Option<Vec<u8>> {
        match &mut self.state {
            State::Connected(link) => link.from.recv().await,
            _ => std::future::pending().await,
        }
    }

    /// A line from Hersir goes to Claude Desktop unchanged; None: Hersir closed the connection.
    fn on_hersir_line(&mut self, line: Option<Vec<u8>>) {
        let Some(line) = line else { return self.lost() };
        let msg = serde_json::from_slice::<Value>(&line).ok();
        if let Some(why) = msg.as_ref().and_then(refusal) {
            return self.refused(&why);
        }
        // An answer: its request doesn't wait any more.
        if let (Some(msg), State::Connected(link)) = (&msg, &mut self.state)
            && msg.get("method").is_none()
            && let Some(at) = msg.get("id").and_then(|id| link.pending.iter().position(|p| p.id == *id))
        {
            link.pending.remove(at);
        }
        self.write(&line);
    }

    /// Hersir closed the connection: the requests still waiting for it get their answer now, and the shim tries again
    /// every 2 seconds.
    fn lost(&mut self) {
        self.leave(State::Waiting, NOT_RUNNING);
        self.say_waiting();
        self.next_try = Instant::now() + RETRY;
    }

    /// Hersir refused the token (after the hello, or later if it ever does): no more tries.
    fn refused(&mut self, why: &str) {
        self.leave(State::Refused, REFUSED);
        let why = if self.token.is_empty() { why.to_string() } else { why.replace(self.token.as_str(), "<token>") };
        say(&format!("Hersir refused the token: {}", why.split_whitespace().collect::<Vec<_>>().join(" ")));
    }

    /// Goes to state `next`. Leaving a connection, each request still waiting for Hersir gets `why` as its answer: a
    /// `tools/call` as a tool's error, any other request as a JSON-RPC error.
    fn leave(&mut self, next: State, why: &str) {
        if let State::Connected(link) = std::mem::replace(&mut self.state, next) {
            for p in &link.pending {
                let id = p.id.clone();
                let answer = if p.call { tool_error(id, why) } else { error(id, CONNECTION_CLOSED, why) };
                self.write_value(&answer);
            }
        }
    }

    /// Not connected: says so on stderr, once until the next connection.
    fn say_waiting(&mut self) {
        if !self.said_waiting {
            self.said_waiting = true;
            say("Hersir isn't running; waiting for it");
        }
    }

    /// Why Hersir can't be reached, None while connected.
    fn down(&self) -> Option<&'static str> {
        match self.state {
            State::Waiting => Some(NOT_RUNNING),
            State::Connected(_) => None,
            State::Refused => Some(REFUSED),
        }
    }

    /// Stdin ended: Hersir's answers to the requests passed on still come back for a moment, then the shim ends.
    async fn finish(&mut self) {
        let until = Instant::now() + GRACE;
        while !self.gone
            && let State::Connected(link) = &mut self.state
            && !link.pending.is_empty()
        {
            match tokio::time::timeout_at(until, link.from.recv()).await {
                Ok(Some(line)) => self.on_hersir_line(Some(line)),
                _ => break,
            }
        }
    }

    /// A line to Claude Desktop, flushed at once.
    fn write(&mut self, line: &[u8]) {
        if self.gone {
            return;
        }
        let end: &[u8] = if line.ends_with(b"\n") { b"" } else { b"\n" };
        let mut out = std::io::stdout().lock();
        if out.write_all(line).and_then(|()| out.write_all(end)).and_then(|()| out.flush()).is_err() {
            self.gone = true;
        }
    }

    fn write_value(&mut self, v: &Value) {
        self.write(format!("{v}\n").as_bytes());
    }
}

/// A connection to Hersir. Its lines go each way through a task of its own, so neither side ever waits for the other.
/// Dropping it closes the connection.
struct Link {
    to: UnboundedSender<Vec<u8>>,
    from: UnboundedReceiver<Vec<u8>>,
    /// The requests passed on that Hersir hasn't answered yet.
    pending: Vec<Pending>,
    tasks: [JoinHandle<()>; 2],
}

/// A request passed on to Hersir, by its id; `call`: a `tools/call`.
struct Pending {
    id: Value,
    call: bool,
}

impl Link {
    fn new<S: AsyncRead + AsyncWrite + Send + 'static>(stream: S) -> Link {
        let (read, mut write) = tokio::io::split(stream);
        let (to, mut outgoing) = unbounded_channel::<Vec<u8>>();
        let (incoming, from) = unbounded_channel::<Vec<u8>>();
        let writer = tokio::spawn(async move {
            while let Some(bytes) = outgoing.recv().await {
                if write.write_all(&bytes).await.is_err() || write.flush().await.is_err() {
                    break;
                }
            }
        });
        let reader = tokio::spawn(async move {
            let mut read = BufReader::new(read);
            loop {
                let mut line = Vec::new();
                match read.read_until(b'\n', &mut line).await {
                    Ok(n) if n > 0 => {
                        if incoming.send(line).is_err() {
                            break;
                        }
                    }
                    _ => break,
                }
            }
        });
        Link { to, from, pending: Vec::new(), tasks: [reader, writer] }
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        // The tasks hold the connection's two halves.
        for task in &self.tasks {
            task.abort();
        }
    }
}

enum Dialed {
    Connected(Link),
    Refused(String),
    NotRunning,
}

/// Connects to Hersir, sends the hello with the token, and learns from a ping of the shim's own whether Hersir took it.
async fn open(socket: &OsStr, token: &str) -> Dialed {
    let Ok(stream) = connect(socket).await else {
        return Dialed::NotRunning;
    };
    let mut link = Link::new(stream);
    let _ = link.to.send(format!("{}{}", hello_line(token), CHECK_PING).into_bytes());
    let until = Instant::now() + CHECK;
    loop {
        let Ok(Some(line)) = tokio::time::timeout_at(until, link.from.recv()).await else {
            // Closed without a word, or no answer in time.
            return Dialed::NotRunning;
        };
        let Ok(msg) = serde_json::from_slice::<Value>(&line) else { continue };
        if let Some(why) = refusal(&msg) {
            return Dialed::Refused(why);
        }
        // The answer to the ping: Hersir sends nothing else before it (a message of its own would have a method).
        if msg.is_object() && msg.get("method").is_none() {
            return Dialed::Connected(link);
        }
    }
}

/// Hersir's refusal of a hello: `{"error": "<why>"}`. Not a JSON-RPC error, whose `error` is an object.
fn refusal(msg: &Value) -> Option<String> {
    msg.get("error")?.as_str().map(str::to_string)
}

#[cfg(unix)]
async fn connect(socket: &OsStr) -> std::io::Result<tokio::net::UnixStream> {
    tokio::net::UnixStream::connect(socket).await
}

/// Windows: Hersir's named pipe, with the checks a chat turn's shim makes (see src/bin/gizai-mcp.rs).
#[cfg(windows)]
async fn connect(socket: &OsStr) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    use std::os::windows::io::AsRawHandle;

    use tokio::net::windows::named_pipe::ClientOptions;
    use windows_sys::Win32::Foundation::ERROR_PIPE_BUSY;

    // Every instance busy (Hersir is just letting another connection in): try again for a moment.
    let deadline = Instant::now() + Duration::from_secs(2);
    let pipe = loop {
        match ClientOptions::new().open(socket) {
            Ok(pipe) => break pipe,
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) && Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(50)).await
            }
            Err(e) => return Err(e),
        }
    };
    // Another program could have made a pipe by this name before Hersir started: the token goes only to our own user.
    // SAFETY: the handle is open while `pipe` lives.
    let server = unsafe { crate::pipe::server_user(pipe.as_raw_handle()) };
    match (server, crate::pipe::User::current()) {
        (Ok(server), Ok(me)) if server == me => Ok(pipe),
        _ => Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "the pipe is not served by your Hersir")),
    }
}

/// Stdin's lines, read on a thread of its own (a read of stdin can't be cancelled; the thread ends with the shim). The
/// channel closes when stdin ends.
fn stdin_lines() -> UnboundedReceiver<Vec<u8>> {
    let (tx, rx) = unbounded_channel();
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        loop {
            let mut line = Vec::new();
            match input.read_until(b'\n', &mut line) {
                Ok(n) if n > 0 => {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
                _ => break,
            }
        }
    });
    rx
}

/// The shim's own tools, [`STATUS_TOOL`] alone. `down`: why Hersir can't be reached, None while connected.
struct Own {
    down: Option<&'static str>,
}

impl Tools for Own {
    fn list(&self) -> Vec<ToolDef> {
        vec![ToolDef {
            name: STATUS_TOOL.into(),
            description: STATUS_DESCRIPTION.into(),
            input_schema: json!({"type": "object", "properties": {}, "required": []}),
            read_only: true,
        }]
    }

    async fn call(&self, name: &str, _args: Value) -> Result<Value, String> {
        match self.down {
            Some(why) => Err(why.to_string()),
            None if name == STATUS_TOOL => Ok(Value::String(CONNECTED.to_string())),
            // Not asked: while connected, every other call goes to Hersir.
            None => Err(format!("unknown tool {name}")),
        }
    }
}

/// The shim's own answer to `initialize`, in both states. Its tools change when Hersir opens or closes, so it says
/// `listChanged`.
fn initialize(id: Value, params: Option<&Value>) -> Value {
    let version = params.and_then(|p| p.get("protocolVersion")).and_then(Value::as_str).unwrap_or(DEFAULT_PROTOCOL);
    ok(id, json!({
        "protocolVersion": version,
        "capabilities": {"tools": {"listChanged": true}},
        "serverInfo": {"name": "hersir", "version": env!("CARGO_PKG_VERSION")},
        "instructions": INSTRUCTIONS,
    }))
}

/// A call's answer as a tool's error, as `handle` gives it for a call that failed.
fn tool_error(id: Value, text: &str) -> Value {
    ok(id, json!({"content": [{"type": "text", "text": text}], "isError": true}))
}

/// One line on stderr, which Claude Desktop keeps in its MCP log.
fn say(what: &str) {
    let _ = writeln!(std::io::stderr(), "hersir-mcp: {what}");
}
