//! The server side of MCP (Model Context Protocol) as Claude Code speaks it over stdio: newline-delimited
//! JSON-RPC 2.0 with `initialize`, `ping`, `tools/list` and `tools/call`. Gizai serves it on a local socket (a
//! named pipe on Windows, see `pipe`); the `hersir-mcp` shim (src/bin) connects Claude Code's stdio to that socket.
//! Claude Desktop runs the shim as `hersir-mcp --desktop`, which keeps answering while Hersir is closed (`desktop`).
//!
//! GA-343: MCP Apps, the `io.modelcontextprotocol/ui` extension ([`APPS_EXTENSION`]): a tool's result can show a view
//! (one HTML file, a `ui://` resource) in the client's chat. Only a client that declares the extension at `initialize`
//! gets it: the extension in the answer, `resources/list` and `resources/read` for the views ([`Tools::apps`]), and the
//! tools the server offers it ([`Tools::list_in`]). Every other client gets exactly what it got before. A connection
//! remembers what its client declared ([`Session`]); Claude Desktop's shim, which answers `initialize` itself, tells
//! Hersir with [`CLIENT_NOTE`].
//!
//! No Tauri, no database.
use std::borrow::Cow;
use std::future::Future;

use serde_json::{Value, json};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

pub mod desktop;

/// Answered when the client doesn't name a protocol version. Otherwise the server answers with the
/// client's version: the four methods served here are the same in every version so far.
pub const DEFAULT_PROTOCOL: &str = "2025-06-18";

const INSTRUCTIONS: &str = "Hersir's tools: clients, projects, tasks, agents, docs, files and the inbox. \
Look items up before changing them; refer to tasks by identifier (KADE-12), projects by key or name.";

/// GA-343: MCP Apps' extension, as a client declares it at `initialize` (`capabilities.extensions`) and the server
/// answers it.
pub const APPS_EXTENSION: &str = "io.modelcontextprotocol/ui";

/// GA-343: the MIME type of an MCP App's view: HTML the client shows in a sandboxed iframe.
pub const APP_MIME: &str = "text/html;profile=mcp-app";

/// GA-343: from Claude Desktop's shim to Hersir, as a notification: the params of the client's `initialize`, which the
/// shim answers itself (`desktop`), so that Hersir knows what the client declared. Never answered; a server that doesn't
/// know it ignores it, as it ignores every notification.
pub const CLIENT_NOTE: &str = "notifications/hersir/client";

/// A tool, as `tools/list` gives it.
#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    /// A JSON Schema object for the arguments.
    pub input_schema: Value,
    /// Reads only (shown to the client as `readOnlyHint`).
    pub read_only: bool,
    /// GA-343: the tool's `_meta`, None for none. For MCP Apps: the view it shows and who may call it ([`app_meta`]).
    pub meta: Option<Value>,
}

/// GA-343: a tool's `_meta` for MCP Apps. `view`: the `ui://` resource its result shows, under `ui.resourceUri` and the
/// older flat key `ui/resourceUri` (hosts read either); None for a tool that shows nothing. `visibility`: who may call
/// it, "model" (the chat's model) and "app" (a view, through the host); `["app"]` keeps it from the model.
pub fn app_meta(view: Option<&str>, visibility: &[&str]) -> Value {
    let mut meta = json!({"ui": {"visibility": visibility}});
    if let Some(uri) = view {
        meta["ui"]["resourceUri"] = json!(uri);
        meta["ui/resourceUri"] = json!(uri);
    }
    meta
}

/// GA-343: a tool's whole answer: its text for the model (`content`); for a tool of an MCP App also the data its view
/// draws (`structuredContent`, which some hosts show the model too) and the result's `_meta`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reply {
    pub text: String,
    pub structured: Option<Value>,
    pub meta: Option<Value>,
}

impl Reply {
    /// An answer of text alone, as `call` gives it: a string as it is, any other value as compact JSON.
    pub fn of(v: &Value) -> Reply {
        Reply { text: text_of(v), ..Reply::default() }
    }
}

/// GA-343: one view of an MCP App: a `ui://` resource, one HTML file with everything in it, which the client shows in a
/// sandboxed iframe ([`APP_MIME`]).
#[derive(Debug, Clone)]
pub struct AppView {
    /// `ui://…`
    pub uri: String,
    pub name: String,
    pub description: String,
    pub html: Cow<'static, str>,
    /// The resource's `_meta.ui` (MCP Apps' `McpUiResourceMeta`): `csp`, `prefersBorder`, …; Null for none.
    pub ui: Value,
}

/// GA-343: what a connection's client declared at `initialize` (or, through Claude Desktop's shim, in [`CLIENT_NOTE`]).
/// A new connection starts with nothing declared.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Session {
    /// The client shows MCP Apps' views ([`declares_apps`]).
    pub apps: bool,
}

impl Session {
    /// The session for a client's `initialize` params.
    pub fn from_initialize(params: &Value) -> Session {
        Session { apps: declares_apps(params) }
    }
}

/// GA-343: whether a client's `initialize` params declare MCP Apps: `capabilities.extensions["io.modelcontextprotocol/ui"]`
/// is an object, and its `mimeTypes`, when given, include [`APP_MIME`].
pub fn declares_apps(params: &Value) -> bool {
    let Some(ext) = params.pointer("/capabilities/extensions").and_then(|e| e.get(APPS_EXTENSION)).filter(|e| e.is_object()) else {
        return false;
    };
    let ours = |t: &Value| t.as_str().is_some_and(|t| t.replace(' ', "").eq_ignore_ascii_case(APP_MIME));
    match ext.get("mimeTypes") {
        None | Some(Value::Null) => true,
        Some(Value::Array(types)) => types.iter().any(ours),
        Some(_) => false,
    }
}

/// The `capabilities` of an `initialize` answer: the tools (`list_changed`: their list can change while connected), and
/// for a client that declared MCP Apps also the resources (the views) and the extension (GA-343).
pub fn capabilities(session: &Session, list_changed: bool) -> Value {
    let mut c = json!({"tools": {"listChanged": list_changed}});
    if session.apps {
        c["resources"] = json!({"listChanged": list_changed});
        c["extensions"] = json!({APPS_EXTENSION: {"mimeTypes": [APP_MIME]}});
    }
    c
}

/// What the server offers. `call` returns the tool's JSON result, or a sentence saying what went wrong.
pub trait Tools: Send + Sync {
    fn list(&self) -> Vec<ToolDef>;
    fn call(&self, name: &str, args: Value) -> impl Future<Output = Result<Value, String>> + Send;
    /// `call` with the request's `_meta` (Null when it has none): Claude Code names the tool use a call is for there,
    /// as `claudecode/toolUseId`. By default the meta is ignored.
    fn call_with_meta(&self, name: &str, args: Value, meta: &Value) -> impl Future<Output = Result<Value, String>> + Send {
        let _ = meta;
        self.call(name, args)
    }
    /// GA-343: the tools for the client of `session`: `list` by default. A server adds its MCP Apps' tools here for a
    /// client that declared the extension (`Session::apps`).
    fn list_in(&self, session: &Session) -> Vec<ToolDef> {
        let _ = session;
        self.list()
    }
    /// GA-343: a call's whole answer: `call_with_meta`'s value as text by default (`Reply::of`). A tool of an MCP App
    /// answers with the data its view draws too (`Reply::structured`).
    fn reply(&self, name: &str, args: Value, meta: &Value, session: &Session) -> impl Future<Output = Result<Reply, String>> + Send {
        let _ = session;
        async move { self.call_with_meta(name, args, meta).await.map(|v| Reply::of(&v)) }
    }
    /// GA-343: the views of the server's MCP Apps, for a client that declared the extension. None by default.
    fn apps(&self) -> Vec<AppView> {
        Vec::new()
    }
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn ok(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// Plain text for the model: strings as they are, everything else as compact JSON.
fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The JSON-RPC error code for a resource the server doesn't have, as MCP gives it.
const RESOURCE_NOT_FOUND: i64 = -32002;

/// Answers one JSON-RPC message as the first of its connection, whose client declared nothing yet. Notifications (no
/// `id`) get no answer.
pub async fn handle<T: Tools>(tools: &T, msg: &Value) -> Option<Value> {
    handle_in(tools, &mut Session::default(), msg).await
}

/// Answers one JSON-RPC message on a connection whose client declared `session`; an `initialize` (or [`CLIENT_NOTE`])
/// sets it. Notifications (no `id`) get no answer.
pub async fn handle_in<T: Tools>(tools: &T, session: &mut Session, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let Some(method) = msg.get("method").and_then(Value::as_str) else {
        return id.map(|id| error(id, -32600, "invalid request"));
    };
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let Some(id) = id else {
        // GA-343: Claude Desktop's shim says what its client declared at the `initialize` it answered itself.
        if method == CLIENT_NOTE {
            *session = Session::from_initialize(&params);
        }
        return None;
    };
    Some(match method {
        "initialize" => {
            *session = Session::from_initialize(&params);
            let version = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(DEFAULT_PROTOCOL);
            ok(id, json!({
                "protocolVersion": version,
                "capabilities": capabilities(session, false),
                // GA-338: the name Claude Desktop shows. Claude Code names the Chief's tools by the key in its MCP config
                // (`gizai`, so mcp__gizai__…), not by this.
                "serverInfo": {"name": "hersir", "version": env!("CARGO_PKG_VERSION")},
                "instructions": INSTRUCTIONS,
            }))
        }
        "ping" => ok(id, json!({})),
        "tools/list" => {
            let list: Vec<Value> = tools.list_in(session).into_iter().map(|t| {
                let mut v = json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": t.input_schema,
                    "annotations": {"readOnlyHint": t.read_only},
                });
                if let Some(meta) = t.meta {
                    v["_meta"] = meta;
                }
                v
            }).collect();
            ok(id, json!({"tools": list}))
        }
        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return Some(error(id, -32602, "tools/call needs a tool name"));
            };
            let args = match params.get("arguments") {
                Some(a) if !a.is_null() => a.clone(),
                _ => json!({}),
            };
            let (reply, is_error) = match tools.reply(name, args, params.get("_meta").unwrap_or(&Value::Null), session).await {
                Ok(r) => (r, false),
                Err(e) => (Reply { text: e, ..Reply::default() }, true),
            };
            let mut result = json!({"content": [{"type": "text", "text": reply.text}], "isError": is_error});
            if let Some(data) = reply.structured {
                result["structuredContent"] = data;
            }
            if let Some(meta) = reply.meta {
                result["_meta"] = meta;
            }
            ok(id, result)
        }
        // GA-343: the views of MCP Apps, only for a client that declared the extension; any other client gets "method not
        // found", as before.
        "resources/list" if session.apps => {
            let list: Vec<Value> = tools.apps().into_iter().map(|v| {
                let mut r = json!({"uri": v.uri, "name": v.name, "description": v.description, "mimeType": APP_MIME});
                if !v.ui.is_null() {
                    r["_meta"] = json!({"ui": v.ui});
                }
                r
            }).collect();
            ok(id, json!({"resources": list}))
        }
        "resources/templates/list" if session.apps => ok(id, json!({"resourceTemplates": []})),
        "resources/read" if session.apps => {
            let Some(uri) = params.get("uri").and_then(Value::as_str) else {
                return Some(error(id, -32602, "resources/read needs a uri"));
            };
            match tools.apps().into_iter().find(|v| v.uri == uri) {
                Some(v) => {
                    let mut content = json!({"uri": v.uri, "mimeType": APP_MIME, "text": v.html});
                    if !v.ui.is_null() {
                        content["_meta"] = json!({"ui": v.ui});
                    }
                    ok(id, json!({"contents": [content]}))
                }
                None => error(id, RESOURCE_NOT_FOUND, &format!("resource not found: {uri}")),
            }
        }
        _ => error(id, -32601, &format!("method not found: {method}")),
    })
}

/// Reads JSON-RPC messages, one per line, and writes one line per answer until the reader ends.
pub async fn serve<R, W, T>(reader: R, writer: W, tools: &T) -> std::io::Result<()>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
    T: Tools,
{
    serve_while(reader, writer, tools, || None).await
}

/// `serve`, asking `refused` before each message whether the connection may go on. When it gives a reason, the message
/// isn't answered: the reason goes to the client as a refused hello's does (`{"error": why}`), and the connection ends.
/// GA-338: Claude Desktop's lasting token, revoked while its connection is open.
pub async fn serve_while<R, W, T>(reader: R, mut writer: W, tools: &T, refused: impl Fn() -> Option<String>) -> std::io::Result<()>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
    T: Tools,
{
    let mut lines = reader.lines();
    // GA-343: what this connection's client declared, from its `initialize` on.
    let mut session = Session::default();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(why) = refused() {
            writer.write_all(format!("{}\n", json!({"error": why})).as_bytes()).await?;
            writer.flush().await?;
            return writer.shutdown().await;
        }
        let answer = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle_in(tools, &mut session, &msg).await,
            Err(_) => Some(error(Value::Null, -32700, "parse error")),
        };
        if let Some(a) = answer {
            writer.write_all(format!("{a}\n").as_bytes()).await?;
            writer.flush().await?;
        }
    }
    Ok(())
}

/// The shim's first line on the socket: the turn's token.
pub fn hello_line(token: &str) -> String {
    format!("{}\n", json!({"token": token}))
}

/// The token from a hello line, if it is one.
pub fn parse_hello(line: &str) -> Option<String> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    v.get("token")?.as_str().map(str::to_string)
}

/// Windows: the named pipe kept to one user. A security descriptor that lets only that user open the pipe, and who runs
/// the process at the other end: Gizai checks each client, the shim checks that the pipe's server is its own user's.
#[cfg(windows)]
pub mod pipe {
    use std::io;
    use std::ptr::null_mut;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree};
    use windows_sys::Win32::Security::Authorization::{ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1};
    use windows_sys::Win32::Security::{EqualSid, GetTokenInformation, PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser};
    use windows_sys::Win32::System::Pipes::{GetNamedPipeClientProcessId, GetNamedPipeServerProcessId};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION};

    /// A user, by the SID in a process's token. Holds the TOKEN_USER that GetTokenInformation wrote, with the SID it
    /// points to (u64s keep it aligned).
    pub struct User(Vec<u64>);

    impl User {
        /// The user this process runs as.
        pub fn current() -> io::Result<User> {
            // SAFETY: GetCurrentProcess returns a pseudo handle that is always valid and needs no closing.
            unsafe { user_of(GetCurrentProcess()) }
        }

        /// The user process `pid` runs as.
        pub fn of_process(pid: u32) -> io::Result<User> {
            // SAFETY: the process handle is checked, and closed before returning.
            unsafe {
                let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if process.is_null() {
                    return Err(io::Error::last_os_error());
                }
                let user = user_of(process);
                CloseHandle(process);
                user
            }
        }

        fn sid(&self) -> PSID {
            // SAFETY: the buffer starts with the TOKEN_USER that GetTokenInformation wrote (see `user_of`).
            unsafe { (*self.0.as_ptr().cast::<TOKEN_USER>()).User.Sid }
        }

        /// The SID as text, e.g. `S-1-5-21-…-1001`.
        pub fn sid_string(&self) -> io::Result<String> {
            let mut text: *mut u16 = null_mut();
            // SAFETY: the SID is valid; Windows allocates the NUL-ended text with LocalAlloc, freed here.
            unsafe {
                if ConvertSidToStringSidW(self.sid(), &mut text) == 0 {
                    return Err(io::Error::last_os_error());
                }
                let len = (0..).take_while(|&i| *text.add(i) != 0).count();
                let sid = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
                LocalFree(text.cast());
                Ok(sid)
            }
        }
    }

    impl PartialEq for User {
        fn eq(&self, other: &User) -> bool {
            // SAFETY: both SIDs are valid.
            unsafe { EqualSid(self.sid(), other.sid()) != 0 }
        }
    }

    /// The user in a process's token. Safety: `process` is a valid process handle.
    unsafe fn user_of(process: HANDLE) -> io::Result<User> {
        // SAFETY: the token handle is checked, and closed before returning; the buffer is as long as Windows asked.
        unsafe {
            let mut token: HANDLE = null_mut();
            if OpenProcessToken(process, TOKEN_QUERY, &mut token) == 0 {
                return Err(io::Error::last_os_error());
            }
            // Asked once for the size, then for the TOKEN_USER itself.
            let mut len = 0u32;
            GetTokenInformation(token, TokenUser, null_mut(), 0, &mut len);
            let mut buf = vec![0u64; (len as usize).div_ceil(8)];
            let ok = len > 0 && GetTokenInformation(token, TokenUser, buf.as_mut_ptr().cast(), len, &mut len) != 0;
            let err = io::Error::last_os_error();
            CloseHandle(token);
            if ok { Ok(User(buf)) } else { Err(err) }
        }
    }

    /// A security descriptor whose DACL lets one user open the pipe and no one else.
    pub struct OnlyUser(PSECURITY_DESCRIPTOR);

    // SAFETY: the descriptor never changes after it is made; CreateNamedPipeW only reads it.
    unsafe impl Send for OnlyUser {}
    unsafe impl Sync for OnlyUser {}

    impl OnlyUser {
        pub fn new(user: &User) -> io::Result<OnlyUser> {
            // D:P is a protected DACL (nothing inherited); (A;;GA;;;<SID>) allows that user everything. No other entry,
            // so everyone else is denied.
            let sddl: Vec<u16> = format!("D:P(A;;GA;;;{})", user.sid_string()?).encode_utf16().chain([0]).collect();
            let mut sd: PSECURITY_DESCRIPTOR = null_mut();
            // SAFETY: `sddl` ends in a NUL; Windows allocates the descriptor with LocalAlloc, freed in Drop.
            if unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), SDDL_REVISION_1, &mut sd, null_mut()) } == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(OnlyUser(sd))
        }

        /// The SECURITY_ATTRIBUTES for CreateNamedPipeW (tokio's `create_with_security_attributes_raw`); they point at
        /// this descriptor, so they are good while it lives.
        pub fn attributes(&self) -> SECURITY_ATTRIBUTES {
            SECURITY_ATTRIBUTES { nLength: size_of::<SECURITY_ATTRIBUTES>() as u32, lpSecurityDescriptor: self.0, bInheritHandle: 0 }
        }
    }

    impl Drop for OnlyUser {
        fn drop(&mut self) {
            // SAFETY: made by ConvertStringSecurityDescriptorToSecurityDescriptorW, freed once.
            unsafe { LocalFree(self.0) };
        }
    }

    /// The user running the client at the other end of a pipe instance Gizai serves (`pipe`: the server's handle).
    ///
    /// # Safety
    /// `pipe` is an open handle (one that is not a pipe only makes the call fail).
    pub unsafe fn client_user(pipe: HANDLE) -> io::Result<User> {
        // SAFETY: `pipe` is open (the caller's promise).
        User::of_process(unsafe { client_pid(pipe) }?)
    }

    /// The id of the client's process at the other end of a pipe instance Gizai serves (`pipe`: the server's handle):
    /// the process that opened the pipe.
    ///
    /// # Safety
    /// `pipe` is an open handle (one that is not a pipe only makes the call fail).
    pub unsafe fn client_pid(pipe: HANDLE) -> io::Result<u32> {
        let mut pid = 0u32;
        // SAFETY: `pipe` is open (the caller's promise).
        if unsafe { GetNamedPipeClientProcessId(pipe, &mut pid) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(pid)
    }

    /// The user running the server of a pipe the shim opened (`pipe`: the client's handle).
    ///
    /// # Safety
    /// `pipe` is an open handle (one that is not a pipe only makes the call fail).
    pub unsafe fn server_user(pipe: HANDLE) -> io::Result<User> {
        let mut pid = 0u32;
        // SAFETY: `pipe` is open (the caller's promise).
        if unsafe { GetNamedPipeServerProcessId(pipe, &mut pid) } == 0 {
            return Err(io::Error::last_os_error());
        }
        User::of_process(pid)
    }
}
