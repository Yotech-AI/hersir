//! GA-343 QA: MCP Apps in the hand-written server, at the protocol level (`handle`, `handle_in`, `serve`).
//! - A client that declares nothing (Claude Code) gets exactly what it got before: the same `initialize` capabilities, the
//!   same tools, no `_meta`, no `structuredContent`, and "method not found" for every `resources/…`.
//! - A client that declares `io.modelcontextprotocol/ui` (an object; `mimeTypes` absent or with ours) gets the extension
//!   and the resources capability, its views in `resources/list` and `resources/read` with MIME type
//!   `text/html;profile=mcp-app`, and the tools `list_in` gives it.
//! - `app_meta` gives both resourceUri keys and the visibility; a `Reply` brings `structuredContent` and `_meta`.
//! - `notifications/hersir/client` sets what the connection declared, as Claude Desktop's shim sends it; each
//!   connection of `serve` keeps its own.
use gizai_mcp::{
    APP_MIME, APPS_EXTENSION, AppView, CLIENT_NOTE, Reply, Session, ToolDef, Tools, app_meta, declares_apps, handle, handle_in, serve,
};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const VIEW_URI: &str = "ui://test/app.html";
const VIEW_HTML: &str = "<!doctype html><html><body><div id=\"root\"></div></body></html>";

/// A server with one plain tool, and for a client that declared MCP Apps also a view and two tools of an app.
struct WithApp;

fn plain() -> ToolDef {
    ToolDef { name: "echo".into(), description: "Returns its input.".into(), input_schema: json!({"type": "object"}), read_only: true, meta: None }
}

fn app_tools() -> Vec<ToolDef> {
    vec![
        ToolDef { name: "show".into(), description: "Shows the view.".into(), input_schema: json!({"type": "object"}), read_only: true,
            meta: Some(app_meta(Some(VIEW_URI), &["model", "app"])) },
        ToolDef { name: "read".into(), description: "For the view.".into(), input_schema: json!({"type": "object"}), read_only: true,
            meta: Some(app_meta(None, &["app"])) },
    ]
}

impl Tools for WithApp {
    fn list(&self) -> Vec<ToolDef> {
        vec![plain()]
    }
    async fn call(&self, name: &str, args: Value) -> Result<Value, String> {
        match name {
            "echo" => Ok(args),
            "boom" => Err("it broke".into()),
            other => Err(format!("unknown tool {other}")),
        }
    }
    fn list_in(&self, session: &Session) -> Vec<ToolDef> {
        let mut list = self.list();
        if session.apps {
            list.extend(app_tools());
        }
        list
    }
    async fn reply(&self, name: &str, args: Value, meta: &Value, session: &Session) -> Result<Reply, String> {
        if session.apps && name == "show" {
            return Ok(Reply { text: "The user sees it.".into(), structured: Some(json!({"view": "test", "n": 2})),
                meta: Some(json!({"ui": {"resourceUri": VIEW_URI}, "ui/resourceUri": VIEW_URI})) });
        }
        if session.apps && name == "read" {
            return Ok(Reply { text: "Read.".into(), structured: Some(json!({"view": "test", "n": 3})), meta: None });
        }
        self.call_with_meta(name, args, meta).await.map(|v| Reply::of(&v))
    }
    fn apps(&self) -> Vec<AppView> {
        vec![AppView { uri: VIEW_URI.into(), name: "Test view".into(), description: "A view.".into(), html: VIEW_HTML.into(),
            ui: json!({"prefersBorder": true}) }]
    }
}

/// A server that only says `list` and `call`, as before GA-343: its defaults must not change anything.
struct Plain;

impl Tools for Plain {
    fn list(&self) -> Vec<ToolDef> {
        vec![plain()]
    }
    async fn call(&self, name: &str, args: Value) -> Result<Value, String> {
        if name == "echo" { Ok(args) } else { Err(format!("unknown tool {name}")) }
    }
}

fn req(id: i64, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

/// Claude Code's `initialize` params (2.x): roots and elicitation, no extensions.
fn claude_code() -> Value {
    json!({"protocolVersion": "2025-06-18", "capabilities": {"roots": {}, "elicitation": {}}, "clientInfo": {"name": "claude-code", "version": "2.1.0"}})
}

/// A client that shows MCP Apps' views, as ext-apps' hosts declare it.
fn declaring(ext: Value) -> Value {
    json!({"protocolVersion": "2025-11-25", "capabilities": {"extensions": {APPS_EXTENSION: ext}}, "clientInfo": {"name": "claude-ai", "version": "1"}})
}

fn apps_client() -> Value {
    declaring(json!({"mimeTypes": [APP_MIME]}))
}

async fn ask<T: Tools>(tools: &T, session: &mut Session, msg: Value) -> Value {
    handle_in(tools, session, &msg).await.unwrap_or_else(|| panic!("no answer to {msg}"))
}

#[test]
fn the_constants_are_the_cards() {
    assert_eq!(APPS_EXTENSION, "io.modelcontextprotocol/ui");
    assert_eq!(APP_MIME, "text/html;profile=mcp-app");
    assert_eq!(CLIENT_NOTE, "notifications/hersir/client");
}

#[test]
fn declares_apps_takes_an_object_with_our_mime_type_or_none_and_nothing_else() {
    for yes in [json!({}), json!({"mimeTypes": [APP_MIME]}), json!({"mimeTypes": ["text/plain", APP_MIME]}), json!({"mimeTypes": null}),
        json!({"mimeTypes": ["text/html; profile=mcp-app"]}), json!({"mimeTypes": ["TEXT/HTML;PROFILE=MCP-APP"]})] {
        assert!(declares_apps(&declaring(yes.clone())), "declares: {yes}");
    }
    for no in [json!(true), json!("yes"), json!(1), json!(null), json!([]), json!({"mimeTypes": []}), json!({"mimeTypes": ["text/html"]}),
        json!({"mimeTypes": APP_MIME}), json!({"mimeTypes": [1, true]})] {
        assert!(!declares_apps(&declaring(no.clone())), "doesn't declare: {no}");
    }
    // not declared at all, or somewhere else than capabilities.extensions
    for no in [claude_code(), json!({}), json!(null), json!({"capabilities": {}}), json!({"capabilities": {"extensions": {}}}),
        json!({"capabilities": {"experimental": {APPS_EXTENSION: {}}}}), json!({"capabilities": {"extensions": {"io.modelcontextprotocol/other": {}}}}),
        json!({"extensions": {APPS_EXTENSION: {}}})] {
        assert!(!declares_apps(&no), "doesn't declare: {no}");
    }
}

#[tokio::test]
async fn claude_code_gets_exactly_what_it_got_before_no_extension_no_resources_the_same_tools() {
    let mut s = Session::default();
    let init = ask(&WithApp, &mut s, req(1, "initialize", claude_code())).await;
    assert_eq!(init["result"]["capabilities"], json!({"tools": {"listChanged": false}}), "{init}");
    assert!(!s.apps);
    // the same tools as `list`, each without _meta
    let tools = ask(&WithApp, &mut s, req(2, "tools/list", json!({}))).await;
    assert_eq!(tools["result"]["tools"], json!([{"name": "echo", "description": "Returns its input.", "inputSchema": {"type": "object"},
        "annotations": {"readOnlyHint": true}}]), "{tools}");
    // every resources/… is "method not found", as before
    for (id, method, params) in [(3, "resources/list", json!({})), (4, "resources/read", json!({"uri": VIEW_URI})), (5, "resources/templates/list", json!({}))] {
        let r = ask(&WithApp, &mut s, req(id, method, params)).await;
        assert_eq!(r["error"]["code"], -32601, "{method}: {r}");
        assert!(r.get("result").is_none(), "{method}: {r}");
    }
    // a call answers text alone: no structuredContent, no _meta
    let r = ask(&WithApp, &mut s, req(6, "tools/call", json!({"name": "echo", "arguments": {"a": 1}}))).await;
    assert_eq!(r["result"], json!({"content": [{"type": "text", "text": "{\"a\":1}"}], "isError": false}), "{r}");
    // an app's tool called anyway is an unknown tool here
    let r = ask(&WithApp, &mut s, req(7, "tools/call", json!({"name": "show", "arguments": {}}))).await;
    assert_eq!(r["result"], json!({"content": [{"type": "text", "text": "unknown tool show"}], "isError": true}), "{r}");
}

#[tokio::test]
async fn a_server_with_only_list_and_call_answers_a_declaring_client_with_the_extension_and_no_views() {
    let mut s = Session::default();
    let init = ask(&Plain, &mut s, req(1, "initialize", apps_client())).await;
    assert_eq!(init["result"]["capabilities"]["extensions"], json!({APPS_EXTENSION: {"mimeTypes": [APP_MIME]}}), "{init}");
    let tools = ask(&Plain, &mut s, req(2, "tools/list", json!({}))).await;
    assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 1);
    assert!(tools["result"]["tools"][0].get("_meta").is_none(), "{tools}");
    assert_eq!(ask(&Plain, &mut s, req(3, "resources/list", json!({}))).await["result"], json!({"resources": []}));
    let r = ask(&Plain, &mut s, req(4, "resources/read", json!({"uri": VIEW_URI}))).await;
    assert_eq!(r["error"]["code"], -32002, "{r}");
    let r = ask(&Plain, &mut s, req(5, "tools/call", json!({"name": "echo", "arguments": {"a": 1}}))).await;
    assert_eq!(r["result"], json!({"content": [{"type": "text", "text": "{\"a\":1}"}], "isError": false}), "{r}");
}

#[tokio::test]
async fn a_declaring_client_gets_the_extension_the_resources_and_its_views_with_the_mcp_app_mime_type() {
    let mut s = Session::default();
    let init = ask(&WithApp, &mut s, req(1, "initialize", apps_client())).await;
    assert_eq!(init["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(init["result"]["capabilities"], json!({
        "tools": {"listChanged": false},
        "resources": {"listChanged": false},
        "extensions": {"io.modelcontextprotocol/ui": {"mimeTypes": ["text/html;profile=mcp-app"]}},
    }), "{init}");
    assert!(s.apps);

    let list = ask(&WithApp, &mut s, req(2, "resources/list", json!({}))).await;
    assert_eq!(list["result"]["resources"], json!([{"uri": VIEW_URI, "name": "Test view", "description": "A view.",
        "mimeType": "text/html;profile=mcp-app", "_meta": {"ui": {"prefersBorder": true}}}]), "{list}");
    let read = ask(&WithApp, &mut s, req(3, "resources/read", json!({"uri": VIEW_URI}))).await;
    assert_eq!(read["result"]["contents"], json!([{"uri": VIEW_URI, "mimeType": "text/html;profile=mcp-app", "text": VIEW_HTML,
        "_meta": {"ui": {"prefersBorder": true}}}]), "{read}");
    assert_eq!(ask(&WithApp, &mut s, req(4, "resources/templates/list", json!({}))).await["result"], json!({"resourceTemplates": []}));

    // a view the server doesn't have, and a read without a uri
    let r = ask(&WithApp, &mut s, req(5, "resources/read", json!({"uri": "ui://test/other.html"}))).await;
    assert_eq!(r["error"]["code"], -32002, "{r}");
    assert!(r["error"]["message"].as_str().unwrap().contains("ui://test/other.html"), "{r}");
    let r = ask(&WithApp, &mut s, req(6, "resources/read", json!({}))).await;
    assert_eq!(r["error"]["code"], -32602, "{r}");
}

#[tokio::test]
async fn a_declaring_client_gets_the_apps_tools_with_their_meta_and_results_with_structured_content() {
    let mut s = Session::default();
    ask(&WithApp, &mut s, req(1, "initialize", declaring(json!({})))).await;
    let tools = ask(&WithApp, &mut s, req(2, "tools/list", json!({}))).await;
    let tools = tools["result"]["tools"].as_array().unwrap().clone();
    assert_eq!(tools.iter().map(|t| t["name"].as_str().unwrap()).collect::<Vec<_>>(), ["echo", "show", "read"]);
    assert!(tools[0].get("_meta").is_none(), "the plain tool has no _meta: {}", tools[0]);
    assert_eq!(tools[1]["_meta"], json!({"ui": {"visibility": ["model", "app"], "resourceUri": VIEW_URI}, "ui/resourceUri": VIEW_URI}));
    assert_eq!(tools[2]["_meta"], json!({"ui": {"visibility": ["app"]}}));

    let r = ask(&WithApp, &mut s, req(3, "tools/call", json!({"name": "show", "arguments": {}}))).await;
    assert_eq!(r["result"], json!({
        "content": [{"type": "text", "text": "The user sees it."}], "isError": false,
        "structuredContent": {"view": "test", "n": 2},
        "_meta": {"ui": {"resourceUri": VIEW_URI}, "ui/resourceUri": VIEW_URI},
    }), "{r}");
    let r = ask(&WithApp, &mut s, req(4, "tools/call", json!({"name": "read", "arguments": {}}))).await;
    assert_eq!(r["result"], json!({"content": [{"type": "text", "text": "Read."}], "isError": false, "structuredContent": {"view": "test", "n": 3}}), "{r}");
    // an error has the text alone
    let r = ask(&WithApp, &mut s, req(5, "tools/call", json!({"name": "boom", "arguments": {}}))).await;
    assert_eq!(r["result"], json!({"content": [{"type": "text", "text": "it broke"}], "isError": true}), "{r}");
}

#[test]
fn app_meta_gives_both_resource_uri_keys_only_with_a_view() {
    assert_eq!(app_meta(Some("ui://hersir/app.html"), &["model", "app"]),
        json!({"ui": {"visibility": ["model", "app"], "resourceUri": "ui://hersir/app.html"}, "ui/resourceUri": "ui://hersir/app.html"}));
    assert_eq!(app_meta(None, &["app"]), json!({"ui": {"visibility": ["app"]}}));
    assert_eq!(Reply::of(&json!("text")), Reply { text: "text".into(), structured: None, meta: None });
    assert_eq!(Reply::of(&json!({"a": 1})).text, "{\"a\":1}");
}

#[tokio::test]
async fn the_shims_client_note_sets_what_the_connection_declared_and_gets_no_answer() {
    let mut s = Session::default();
    let note = json!({"jsonrpc": "2.0", "method": CLIENT_NOTE, "params": apps_client()});
    assert!(handle_in(&WithApp, &mut s, &note).await.is_none(), "a notification gets no answer");
    assert!(s.apps);
    let list = ask(&WithApp, &mut s, req(1, "resources/list", json!({}))).await;
    assert_eq!(list["result"]["resources"][0]["uri"], VIEW_URI, "{list}");
    assert_eq!(ask(&WithApp, &mut s, req(2, "tools/list", json!({}))).await["result"]["tools"].as_array().unwrap().len(), 3);
    // a note for a client that declares nothing takes it away again
    let note = json!({"jsonrpc": "2.0", "method": CLIENT_NOTE, "params": claude_code()});
    assert!(handle_in(&WithApp, &mut s, &note).await.is_none());
    assert!(!s.apps);
    assert_eq!(ask(&WithApp, &mut s, req(3, "resources/list", json!({}))).await["error"]["code"], -32601);
    // and so does a later initialize without the extension
    let note = json!({"jsonrpc": "2.0", "method": CLIENT_NOTE, "params": apps_client()});
    handle_in(&WithApp, &mut s, &note).await;
    ask(&WithApp, &mut s, req(4, "initialize", claude_code())).await;
    assert!(!s.apps);
    // a request with the note's method is no note: it changes nothing
    let mut s = Session::default();
    let r = ask(&WithApp, &mut s, req(5, CLIENT_NOTE, apps_client())).await;
    assert_eq!(r["error"]["code"], -32601, "{r}");
    assert!(!s.apps);
}

#[tokio::test]
async fn handle_answers_as_the_first_message_of_a_connection_so_resources_stay_method_not_found() {
    // `handle` keeps no session: even after a declaring initialize, the next message is a new connection's
    let init = handle(&WithApp, &req(1, "initialize", apps_client())).await.unwrap();
    assert!(init["result"]["capabilities"]["extensions"].is_object(), "{init}");
    let r = handle(&WithApp, &req(2, "resources/list", json!({}))).await.unwrap();
    assert_eq!(r["error"]["code"], -32601, "{r}");
}

/// One connection to `serve`: its lines in, its answers out.
async fn served_connection(lines: &[Value]) -> Vec<Value> {
    let (client, server) = tokio::io::duplex(1 << 16);
    let (sr, sw) = tokio::io::split(server);
    let task = tokio::spawn(async move { serve(BufReader::new(sr), sw, &WithApp).await });
    let (cr, mut cw) = tokio::io::split(client);
    for l in lines {
        cw.write_all(format!("{l}\n").as_bytes()).await.unwrap();
    }
    cw.shutdown().await.unwrap();
    let mut out = vec![];
    let mut r = BufReader::new(cr).lines();
    while let Some(l) = r.next_line().await.unwrap() {
        out.push(serde_json::from_str(&l).unwrap());
    }
    task.await.unwrap().unwrap();
    out
}

#[tokio::test]
async fn each_connection_of_serve_keeps_what_its_own_client_declared() {
    let declares = served_connection(&[
        req(1, "initialize", apps_client()),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        req(2, "resources/list", json!({})),
        req(3, "tools/list", json!({})),
    ]).await;
    assert_eq!(declares.len(), 3, "{declares:?}");
    assert_eq!(declares[1]["result"]["resources"][0]["uri"], VIEW_URI, "{declares:?}");
    assert_eq!(declares[2]["result"]["tools"].as_array().unwrap().len(), 3);

    // a second connection, after the first: it declared nothing, so nothing of the first carries over
    let plain = served_connection(&[req(1, "initialize", claude_code()), req(2, "resources/list", json!({})), req(3, "tools/list", json!({}))]).await;
    assert_eq!(plain[0]["result"]["capabilities"], json!({"tools": {"listChanged": false}}));
    assert_eq!(plain[1]["error"]["code"], -32601, "{plain:?}");
    assert_eq!(plain[2]["result"]["tools"].as_array().unwrap().len(), 1);

    // Claude Desktop's shim: no initialize, the client note first (it answered initialize itself)
    let shim = served_connection(&[
        json!({"jsonrpc": "2.0", "method": CLIENT_NOTE, "params": apps_client()}),
        req(1, "resources/read", json!({"uri": VIEW_URI})),
        req(2, "tools/call", json!({"name": "show", "arguments": {}})),
    ]).await;
    assert_eq!(shim.len(), 2, "the note gets no answer: {shim:?}");
    assert_eq!(shim[0]["result"]["contents"][0]["mimeType"], APP_MIME, "{shim:?}");
    assert_eq!(shim[1]["result"]["structuredContent"], json!({"view": "test", "n": 2}), "{shim:?}");
}
