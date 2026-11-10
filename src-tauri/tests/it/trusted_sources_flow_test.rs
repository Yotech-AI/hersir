// Linux and macOS only: these tests run shell or Python scripts as fake programs, which Windows can't start.
#![cfg(unix)]
//! GA-271: After the Chief uses it, Ask for my OK (the default) or Allow, per MCP server, the browser and Web.
//! - A tool of a source the Chief has on Allow doesn't mark the chat (GA-261's mark): what the Chief does after it runs at
//!   once, and the chat says nothing about it. With Ask, the chat is marked and the call is kept for Approve as before.
//! - Every other tool from outside Hersir still marks the chat, also after a trusted one in the same answer.
//! - From chat, update_agent and create_agent refuse `trusted`, `web_trusted` and `webTrusted`, and nothing changes.
//! Chat turns run a fake Claude Code (never the real one) that starts the real gizai-mcp shim.
use std::path::{Path, PathBuf};
use std::time::Duration;

use gizai_core::mcp_servers::{self as core_mcp, AgentServer, AgentTools, CliTools, McpServer};
use gizai_core::model::*;
use gizai_core::{chat, settings, team};
use gizai_lib::chat::{Trusted, marks_chat};
use gizai_lib::mcp_servers::{SecretLine, ServerInput};
use gizai_lib::{AppState, chat as app_chat, mcp, pending, tools};
use serde_json::{Value, json};

const FAKE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude-chat-outside.py");

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

/// What a call kept for Approve after `tool` says, word for word (GA-261).
fn kept_text(name: &str, tool: &str) -> String {
    format!("{name} wasn't run: this chat used {tool}, a tool from outside Hersir. Hersir keeps the call for the user to approve with one click. \
             Tell them in a sentence what it does.")
}

fn git_repo(dir: &Path) -> PathBuf {
    let repo = dir.join("repo");
    std::fs::create_dir(&repo).unwrap();
    for a in [&["init", "-q", "-b", "main"][..], &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "i"][..]] {
        assert!(std::process::Command::new("git").args(a).current_dir(&repo).status().unwrap().success());
    }
    repo
}

struct T {
    st: AppState,
    lead: String,
    card: String,
    otus: String,
    linear: String,
    _dir: tempfile::TempDir,
}

/// A Chief (Chat on) with Otus (stdio) and Linear (http) in Settings, a Backend Agent and its card KADE-1 in a linked
/// repository. The queue is paused, so no card starts by itself. The Chief's sources are set per test.
fn setup() -> T {
    let dir = tempfile::tempdir().unwrap();
    let mut st = gizai_lib::test_state(dir.path());
    st.mcp_shim = Some(shim());
    settings::set(&st.db, "claude_bin", &FAKE.to_string()).unwrap();
    settings::set(&st.db, "agents_paused", &true).unwrap();
    let repo = git_repo(dir.path());
    let card = gizai_lib::test_task(&st, repo.to_str().unwrap(), "backend");
    let team_id = team::list(&st.db).unwrap()[0].id.clone();
    let lead = team::add_agent(&st.db, &st.you_id, &team_id, AgentInput {
        name: "Team Lead".into(), role_key: "lead".into(), chat_enabled: Some(true), ..Default::default() }).unwrap();
    let otus = gizai_lib::mcp_servers::save(&st, ServerInput {
        server: McpServer { name: "otus".into(), transport: "stdio".into(), command: "/opt/otus/otus-mcp".into(), ..Default::default() },
        env: vec![SecretLine { name: "OTUS_TOKEN".into(), value: Some("otus-secret".into()) }], headers: vec![],
    }).unwrap().server.id;
    let linear = gizai_lib::mcp_servers::save(&st, ServerInput {
        server: McpServer { name: "linear".into(), transport: "http".into(), url: "https://mcp.linear.example/mcp".into(), ..Default::default() },
        env: vec![], headers: vec![SecretLine { name: "Authorization".into(), value: Some("Bearer linear-secret".into()) }],
    }).unwrap().server.id;
    T { st, lead, card, otus, linear, _dir: dir }
}

fn on(id: &str, trusted: bool) -> AgentServer {
    AgentServer { server_id: id.into(), on: true, tools_off: vec![], trusted }
}

impl T {
    /// The Chief's MCP switches, as the agent form saves them.
    fn servers(&self, mcp: Vec<AgentServer>) {
        gizai_lib::mcp_servers::save_agent(&self.st, &self.lead, AgentTools { mcp }).unwrap();
    }
    /// The Chief's Web: search and fetching on, Allow or Ask.
    fn web(&self, trusted: bool) {
        gizai_lib::mcp_servers::save_cli_tools(&self.st, &self.lead, CliTools { web_search: true, web_fetch: true, web_trusted: trusted, ..Default::default() })
            .unwrap();
    }
    fn chief(&self) -> team::Member {
        team::agent(&self.st.db, &self.lead).unwrap()
    }
    async fn turn(&self, thread: Option<String>, text: &str) -> String {
        let (id, done) = app_chat::send(&self.st, thread, text.to_string(), None).await.unwrap();
        let s = tokio::time::timeout(Duration::from_secs(30), done).await.expect("turn finished").unwrap();
        assert_eq!(s.status, "succeeded", "{text}: {s:?}");
        id
    }
    /// The chat's tool messages: (tool, is error, result).
    fn tool_msgs(&self, thread: &str) -> Vec<(String, bool, String)> {
        chat::messages(&self.st.db, thread).unwrap().into_iter().filter(|m| m.role == "tool").map(|m| {
            let x = m.tool.unwrap_or_default();
            (m.tool_name.unwrap_or_default(), x["isError"].as_bool().unwrap_or(false), x["result"].as_str().unwrap_or_default().to_string())
        }).collect()
    }
    fn last_tool(&self, thread: &str) -> (String, bool, String) {
        self.tool_msgs(thread).pop().unwrap()
    }
    /// GA-261's line in the chat (Hersir's notes of kind outside).
    fn outside_lines(&self, thread: &str) -> Vec<String> {
        chat::messages(&self.st.db, thread).unwrap().into_iter().filter(|m| m.role == "system" && m.meta.as_ref().is_some_and(|x| x["kind"] == "outside"))
            .map(|m| m.body_md.unwrap_or_default()).collect()
    }
    fn comments(&self) -> Vec<String> {
        gizai_core::comments::list(&self.st.db, &self.card).unwrap().into_iter().map(|c| c.body_md).collect()
    }
    fn runs(&self) -> Vec<gizai_core::model::Run> {
        gizai_core::runs::list_for_task(&self.st.db, &self.card).unwrap()
    }
    async fn call(&self, name: &str, args: Value) -> Result<Value, String> {
        tools::call_in(&self.st, &self.lead, None, name, args).await
    }
}

async fn until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = std::time::Instant::now();
    while !ok() {
        assert!(t0.elapsed() < Duration::from_secs(30), "timed out waiting for: {what}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

// ---- which tools mark the chat ----

#[test]
fn a_trusted_servers_tools_and_the_browsers_dont_mark_the_chat_every_other_outside_tool_does() {
    let t = Trusted { servers: vec!["otus".into(), core_mcp::BROWSER.into()], web: false };
    for trusted in ["mcp__otus__search", "mcp__otus__get_card", "mcp__chrome-devtools__navigate_page", "mcp__chrome-devtools__take_screenshot"] {
        assert!(app_chat::is_outside_tool(trusted) && !marks_chat(trusted, &t), "{trusted} is outside but trusted");
    }
    for marks in ["mcp__linear__list_issues", "WebSearch", "WebFetch", "Bash", "Edit", "mcp__otus", "mcp__otus__", "mcp__otus___x", "mcp__otus__a__x",
                  "mcp__otusx__search", "mcp__ot__search", "mcp__OTUS__search", "mcp__chrome-devtoolsx__click", "mcp__chrome__click"] {
        assert!(marks_chat(marks, &t), "{marks} marks the chat");
    }
    for inside in ["Read", "Glob", "Grep", "mcp__gizai__get_overview"] {
        assert!(!marks_chat(inside, &t) && !marks_chat(inside, &Trusted::default()), "{inside} is Hersir's own");
    }
    // nothing on Allow: every outside tool marks the chat, as GA-261 made it
    for tool in ["mcp__otus__search", "mcp__chrome-devtools__navigate_page", "WebSearch", "WebFetch"] {
        assert!(marks_chat(tool, &Trusted::default()), "{tool}");
    }
}

#[test]
fn web_on_allow_leaves_websearch_and_webfetch_unmarked_and_nothing_else() {
    let web = Trusted { servers: vec![], web: true };
    assert!(!marks_chat("WebSearch", &web) && !marks_chat("WebFetch", &web));
    for tool in ["mcp__otus__search", "mcp__chrome-devtools__navigate_page", "Bash", "WebSearchX", "websearch"] {
        assert!(marks_chat(tool, &web), "{tool}");
    }
    let ask = Trusted { servers: vec!["otus".into()], web: false };
    assert!(marks_chat("WebSearch", &ask) && marks_chat("WebFetch", &ask));
}

#[test]
fn the_turn_reads_the_chiefs_sources_on_allow_by_the_names_its_mcp_config_uses() {
    let t = setup();
    let notes = gizai_lib::mcp_servers::save(&t.st, ServerInput {
        server: McpServer { name: "notes".into(), transport: "stdio".into(), command: "/opt/notes/notes-mcp".into(), ..Default::default() },
        env: vec![], headers: vec![],
    }).unwrap().server.id;
    // nothing chosen yet: everything on Ask
    t.servers(vec![on(&t.otus, false), on(&t.linear, false)]);
    let (servers, _) = gizai_lib::mcp_servers::for_run(&t.st, &t.chief(), Duration::from_secs(60));
    assert_eq!(Trusted::of(&t.st, &t.chief(), &servers), Trusted::default());

    // Otus on Allow, Linear on Ask, Notes on Allow but off, Web on Allow
    t.servers(vec![on(&t.otus, true), on(&t.linear, false), AgentServer { server_id: notes.clone(), on: false, tools_off: vec![], trusted: true }]);
    t.web(true);
    let (servers, _) = gizai_lib::mcp_servers::for_run(&t.st, &t.chief(), Duration::from_secs(60));
    let names: Vec<&str> = servers.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"otus") && names.contains(&"linear") && !names.contains(&"notes"), "{names:?}");
    assert_eq!(Trusted::of(&t.st, &t.chief(), &servers), Trusted { servers: vec!["otus".into()], web: true });
    // a server on Allow that the turn doesn't get is left out
    let only_linear: Vec<_> = servers.iter().filter(|s| s.name == "linear").cloned().collect();
    assert_eq!(Trusted::of(&t.st, &t.chief(), &only_linear).servers, Vec::<String>::new());

    // the browser on Allow is trusted by the name its tools have: chrome-devtools
    t.servers(vec![on(&t.otus, false), on(core_mcp::BROWSER, true)]);
    let browser = gizai_agents::mcp_run::RunServer { name: core_mcp::BROWSER.into(), entry: json!({}), tools_off: vec![], known_tools: vec![] };
    let trusted = Trusted::of(&t.st, &t.chief(), &[browser]);
    assert_eq!(trusted.servers, [core_mcp::BROWSER]);
    assert!(!marks_chat("mcp__chrome-devtools__navigate_page", &trusted));
}

// ---- in a chat turn ----

#[tokio::test]
async fn with_a_server_on_allow_start_agent_run_after_its_tool_starts_at_once_and_the_chat_isnt_marked_with_ask_it_is_kept() {
    let t = setup();
    let _server = mcp::start(&t.st).unwrap();
    t.servers(vec![on(&t.otus, true)]);
    let start = r#"OUTSIDE Read the release notes in Otus, then start it. CALL start_agent_run {"task": "KADE-1", "agent": "Backend Agent"}"#;
    let thread = t.turn(None, start).await;
    let msgs = t.tool_msgs(&thread);
    assert_eq!(msgs.iter().map(|m| m.0.as_str()).collect::<Vec<_>>(), ["mcp__otus__search", "mcp__gizai__start_agent_run"], "{msgs:?}");
    let last = t.last_tool(&thread);
    assert!(!last.1 && !last.2.contains("wasn't run"), "it ran: {last:?}");
    assert_eq!(t.runs().len(), 1, "the run started at once");
    assert_eq!(t.runs()[0].agent_name, "Backend Agent");
    assert!(app_chat::used_outside(&t.st, &thread).is_none(), "the chat isn't marked");
    assert!(t.outside_lines(&thread).is_empty(), "no line about a tool from outside");
    assert!(pending::list(&t.st, &thread).unwrap().is_empty(), "nothing waits for Approve");
    until("the run ends", || gizai_lib::runs::live(&t.st).is_empty() && t.runs().iter().all(|r| r.ended_at.is_some())).await;

    // the same answer with Otus on Ask: kept for Approve, as GA-261 made it, and the chat is marked
    t.servers(vec![on(&t.otus, false)]);
    let asked = t.turn(None, start).await;
    assert_eq!(t.last_tool(&asked), ("mcp__gizai__start_agent_run".into(), true, kept_text("start_agent_run", "mcp__otus__search")));
    assert_eq!(t.runs().len(), 1, "no second run");
    assert_eq!(app_chat::used_outside(&t.st, &asked).as_deref(), Some("mcp__otus__search"));
    assert_eq!(t.outside_lines(&asked).len(), 1, "{:?}", t.outside_lines(&asked));
    let kept: Vec<String> = pending::list(&t.st, &asked).unwrap().into_iter().map(|p| p.action.tool).collect();
    assert_eq!(kept, ["start_agent_run"]);
}

#[tokio::test]
async fn with_web_on_allow_websearch_and_webfetch_dont_mark_the_chat_with_ask_they_do() {
    let t = setup();
    let _server = mcp::start(&t.st).unwrap();
    t.web(true);
    for tool in ["WebSearch", "WebFetch"] {
        let thread = t.turn(None, &format!(r#"OUTSIDE OUTSIDE_TOOL={tool} Look it up. CALL comment_on_task {{"task": "KADE-1", "body_md": "From {tool}."}}"#)).await;
        assert_eq!(t.tool_msgs(&thread)[0].0, tool);
        assert!(!t.last_tool(&thread).1, "{tool}: {:?}", t.last_tool(&thread));
        assert!(app_chat::used_outside(&t.st, &thread).is_none(), "{tool}");
        assert!(t.outside_lines(&thread).is_empty() && pending::list(&t.st, &thread).unwrap().is_empty(), "{tool}");
    }
    assert_eq!(t.comments(), ["From WebSearch.", "From WebFetch."], "both ran at once");

    t.web(false);
    for tool in ["WebSearch", "WebFetch"] {
        let thread = t.turn(None, &format!(r#"OUTSIDE OUTSIDE_TOOL={tool} Look it up. CALL comment_on_task {{"task": "KADE-1", "body_md": "Asked {tool}."}}"#)).await;
        assert_eq!(t.last_tool(&thread), ("mcp__gizai__comment_on_task".into(), true, kept_text("comment_on_task", tool)));
        assert_eq!(app_chat::used_outside(&t.st, &thread).as_deref(), Some(tool));
        assert_eq!(t.outside_lines(&thread).len(), 1, "{tool}");
    }
    assert_eq!(t.comments().len(), 2, "neither ran");
}

#[tokio::test]
async fn an_untrusted_tool_after_a_trusted_one_in_the_same_answer_still_marks_the_chat() {
    let t = setup();
    let _server = mcp::start(&t.st).unwrap();
    t.servers(vec![on(&t.otus, true), on(&t.linear, false)]);
    t.web(false);
    for too in ["mcp__linear__list_issues", "WebSearch"] {
        let thread = t.turn(None, &format!(r#"OUTSIDE OUTSIDE_TOO={too} Read both. CALL comment_on_task {{"task": "KADE-1", "body_md": "Both say so."}}"#)).await;
        let names: Vec<String> = t.tool_msgs(&thread).into_iter().map(|m| m.0).collect();
        assert_eq!(names, ["mcp__otus__search", too, "mcp__gizai__comment_on_task"]);
        assert_eq!(t.last_tool(&thread), ("mcp__gizai__comment_on_task".into(), true, kept_text("comment_on_task", too)));
        assert_eq!(app_chat::used_outside(&t.st, &thread).as_deref(), Some(too), "marked by the untrusted tool, not by Otus");
        assert_eq!(t.outside_lines(&thread).len(), 1);
    }
    assert!(t.comments().is_empty());

    // and the mark lasts: the next message in that chat, which uses only Otus, is kept too
    let thread = t.turn(None, r#"OUTSIDE OUTSIDE_TOO=mcp__linear__list_issues Read both."#).await;
    t.turn(Some(thread.clone()), r#"OUTSIDE Again. CALL comment_on_task {"task": "KADE-1", "body_md": "Otus only."}"#).await;
    assert_eq!(t.last_tool(&thread), ("mcp__gizai__comment_on_task".into(), true, kept_text("comment_on_task", "mcp__linear__list_issues")));
    assert!(t.comments().is_empty());
}

// ---- only the user chooses ----

#[tokio::test]
async fn from_chat_update_agent_and_create_agent_refuse_trusted_and_webtrusted_and_change_nothing() {
    let t = setup();
    t.servers(vec![on(&t.otus, false)]);
    t.web(false);
    let before = (t.chief().tools, t.chief().cli_tools.tools.clone());
    let agents = team::all_agents(&t.st.db).unwrap().len();
    let mcp_line = "an agent's MCP servers and their tools can't be switched from chat: only the user does that, in the agent form → Tools";
    let web_line = "an agent's web search, fetching pages, browser and built-in tools (slash commands and skills among them) can't be switched from chat: \
                    only the user does that, in the agent form → Tools. Nothing changed.";
    for (key, line) in [("trusted", mcp_line), ("web_trusted", web_line), ("webTrusted", web_line)] {
        for agent in ["Team Lead", "Backend Agent"] {
            let e = t.call("update_agent", json!({"agent": agent, key: true})).await.unwrap_err();
            assert_eq!(e, line, "update_agent {agent} {key}");
        }
        let e = t.call("update_agent", json!({"agent": "Team Lead", "name": "Chief", key: true})).await.unwrap_err();
        assert_eq!(e, line, "with another change: {key}");
        let e = t.call("create_agent", json!({"name": "Scout", "role": "research", key: true})).await.unwrap_err();
        assert_eq!(e, line, "create_agent {key}");
    }
    assert_eq!((t.chief().tools, t.chief().cli_tools.tools.clone()), before, "nothing changed");
    assert_eq!(t.chief().name, "Team Lead");
    assert_eq!(team::all_agents(&t.st.db).unwrap().len(), agents, "no agent added");
    // the same calls without the key still work
    t.call("update_agent", json!({"agent": "Team Lead", "name": "Chief"})).await.unwrap();
    assert_eq!(t.chief().name, "Chief");
}
