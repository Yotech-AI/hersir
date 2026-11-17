// Linux and macOS only: these tests run shell or Python scripts as fake programs, which Windows can't start.
#![cfg(unix)]
//! GA-289: a tool the Chief's turn doesn't have marks nothing. Claude Code refuses such a call without running it ("No such
//! tool available: Bash"), so nothing came in from outside.
//! - A turn has its built-in tools (`--tools`: Read, Glob and Grep, plus WebSearch and WebFetch while Web is on) and the
//!   tools of the MCP servers in its config, by the exact server name (gizai, the Chief's servers, the browser's
//!   chrome-devtools), and what Claude Code's init line lists.
//! - Bash, Edit, Write, Task, WebFetch with Web off and a tool of a server that isn't in the turn leave the chat unmarked,
//!   with no line about it: card text, a comment and a move to an Auto column after them run at once, without a hold.
//! - A tool the turn has still marks the chat (Ask), also after a refused one in the same answer; a chat marked before keeps
//!   its mark, and a new chat starts unmarked.
//! Chat turns run a fake Claude Code (never the real one, `fake-claude-chat-outside.py` with REFUSED and UNLISTED) that
//! starts the real gizai-mcp shim.
use std::path::{Path, PathBuf};
use std::time::Duration;

use gizai_agents::mcp_run::RunServer;
use gizai_agents::tool_catalog::claude_web_names;
use gizai_core::mcp_servers::{self as core_mcp, AgentServer, AgentTools, CliTools, McpServer};
use gizai_core::model::AgentInput;
use gizai_core::{chat, settings, team};
use gizai_lib::chat::{Trusted, TurnTools, marks_chat};
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

/// What Claude Code answers a tool use of a tool the session doesn't have (the fake's REFUSED).
fn refusal(tool: &str) -> String {
    format!("<tool_use_error>Error: No such tool available: {tool}</tool_use_error>")
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// The built-in tools of a chat turn with Web off.
fn read_glob_grep() -> Vec<String> {
    names(&["Read", "Glob", "Grep"])
}

fn server(name: &str) -> RunServer {
    RunServer { name: name.into(), entry: json!({}), tools_off: vec![], known_tools: vec![] }
}

const ASK: Trusted = Trusted { servers: vec![], web: false };

// ---- which tools a turn has ----

#[test]
fn a_turn_with_only_read_glob_and_grep_has_no_other_tool_and_none_of_them_marks_the_chat() {
    let turn = TurnTools::of(&read_glob_grep(), &[]);
    assert_eq!(turn, TurnTools { tools: read_glob_grep(), servers: names(&["gizai"]), listed: vec![] });
    for tool in ["Bash", "Edit", "Write", "Task", "Agent", "NotebookEdit", "WebFetch", "WebSearch", "mcp__otus__search",
                 "mcp__chrome-devtools__navigate_page", "mcp__otus__", "mcp__otus", "read", "bash", "Read2", ""] {
        assert!(marks_chat(tool, &ASK), "{tool}: outside by its name alone, as before");
        assert!(!turn.has(tool), "{tool}: the turn doesn't have it");
        assert!(!turn.marks(tool, &ASK), "{tool} marks nothing");
    }
    for own in ["Read", "Glob", "Grep", "mcp__gizai__get_task", "mcp__gizai__update_task"] {
        assert!(turn.has(own), "{own}: the turn has it");
        assert!(!turn.marks(own, &ASK), "{own} reads nothing from outside");
    }
    // a name that only looks like Gizai's own still counts as outside (GA-261), and the gizai server is in the turn
    for lookalike in ["mcp__gizai___x", "mcp__gizai__notes__x"] {
        assert!(turn.marks(lookalike, &ASK), "{lookalike}");
    }
}

#[test]
fn websearch_and_webfetch_mark_only_while_web_gives_them_to_the_turn_and_not_on_allow() {
    let allow = Trusted { servers: vec![], web: true };
    let with = |search: bool, fetch: bool| TurnTools::of(&[read_glob_grep(), claude_web_names(search, fetch)].concat(), &[]);
    let both = with(true, true);
    for tool in ["WebSearch", "WebFetch"] {
        assert!(both.marks(tool, &ASK), "{tool} with Web on, Ask");
        assert!(!both.marks(tool, &allow), "{tool} with Web on, Allow (GA-271)");
    }
    let fetch_only = with(false, true);
    assert!(!fetch_only.marks("WebSearch", &ASK) && fetch_only.marks("WebFetch", &ASK));
    let search_only = with(true, false);
    assert!(search_only.marks("WebSearch", &ASK) && !search_only.marks("WebFetch", &ASK));
    let off = with(false, false);
    assert!(!off.marks("WebSearch", &ASK) && !off.marks("WebFetch", &ASK), "Web off: neither");
    // by the exact name
    for tool in ["websearch", "webfetch", "WebFetchX", "Web", "WebSearch "] {
        assert!(!both.marks(tool, &ASK), "{tool:?}");
    }
}

#[test]
fn a_tool_of_a_server_in_the_turn_marks_by_the_exact_server_name_and_a_tool_of_another_server_marks_nothing() {
    let turn = TurnTools::of(&read_glob_grep(), &[server("otus"), server(core_mcp::BROWSER)]);
    assert_eq!(turn.servers, ["gizai", "otus", "chrome-devtools"]);
    for marks in ["mcp__otus__search", "mcp__otus__get_card", "mcp__otus___x", "mcp__otus__a__x", "mcp__chrome-devtools__navigate_page",
                  "mcp__chrome-devtools__take_screenshot"] {
        assert!(turn.has(marks) && turn.marks(marks, &ASK), "{marks}: a tool of a server in the turn, on Ask");
    }
    for none in ["mcp__linear__list_issues", "mcp__otusx__search", "mcp__OTUS__search", "mcp__Otus__search", "mcp__ot__search", "mcp__chrome__click",
                 "mcp__chrome-devtoolsx__click", "mcp__otus__", "mcp__otus", "mcp_otus__search", "otus__search", "mcp____search"] {
        assert!(!turn.has(none) && !turn.marks(none, &ASK), "{none}: no such server in the turn");
    }
    // Allow (GA-271) works as before for the servers the turn has: their tools mark nothing, a name only a lookalike
    // server could have still marks
    let allow = Trusted { servers: vec!["otus".into(), core_mcp::BROWSER.into()], web: false };
    for tool in ["mcp__otus__search", "mcp__chrome-devtools__navigate_page"] {
        assert!(!turn.marks(tool, &allow), "{tool} on Allow");
    }
    for tool in ["mcp__otus___x", "mcp__otus__a__x"] {
        assert!(turn.marks(tool, &allow), "{tool} isn't one of otus's on Allow");
    }
    // Allow for a server that isn't in the turn makes none of its tools one the turn has
    let linear = Trusted { servers: vec!["linear".into()], web: false };
    assert!(!turn.marks("mcp__linear__list_issues", &linear) && turn.marks("mcp__otus__search", &linear));
}

#[test]
fn a_tool_claude_codes_init_line_lists_counts_as_one_the_turn_has_by_its_exact_name() {
    let mut turn = TurnTools::of(&read_glob_grep(), &[]);
    assert!(!turn.marks("Bash", &ASK) && !turn.marks("mcp__notes__find", &ASK));
    turn.listed = names(&["Read", "Bash", "mcp__notes__find"]);
    assert!(turn.marks("Bash", &ASK) && turn.marks("mcp__notes__find", &ASK), "Claude Code gives the session these, so they mark");
    for tool in ["Edit", "bash", "mcp__notes__other", "mcp__notes__find_x"] {
        assert!(!turn.marks(tool, &ASK), "{tool}: not listed");
    }
}

// ---- in a chat turn ----

struct T {
    st: AppState,
    lead: String,
    otus: String,
    _dir: tempfile::TempDir,
}

fn git_repo(dir: &Path) -> PathBuf {
    let repo = dir.join("repo");
    std::fs::create_dir(&repo).unwrap();
    for a in [&["init", "-q", "-b", "main"][..], &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "i"][..]] {
        assert!(std::process::Command::new("git").args(a).current_dir(&repo).status().unwrap().success());
    }
    repo
}

/// A Chief (Chat on, Web off, no MCP servers on) with Otus (stdio) and Linear (http) in Settings, a Backend Agent, its card
/// KADE-1 in To do (an Auto column) in a linked repository, and KADE-2 in Backlog. The queue is paused, so no card starts
/// by itself.
async fn setup() -> T {
    let dir = tempfile::tempdir().unwrap();
    let mut st = gizai_lib::test_state(dir.path());
    st.mcp_shim = Some(shim());
    settings::set(&st.db, "claude_bin", &FAKE.to_string()).unwrap();
    settings::set(&st.db, "agents_paused", &true).unwrap();
    let repo = git_repo(dir.path());
    gizai_lib::test_task(&st, repo.to_str().unwrap(), "backend");
    let team_id = team::list(&st.db).unwrap()[0].id.clone();
    let lead = team::add_agent(&st.db, &st.you_id, &team_id, AgentInput {
        name: "Team Lead".into(), role_key: "lead".into(), chat_enabled: Some(true), ..Default::default() }).unwrap();
    let otus = gizai_lib::mcp_servers::save(&st, ServerInput {
        server: McpServer { name: "otus".into(), transport: "stdio".into(), command: "/opt/otus/otus-mcp".into(), ..Default::default() },
        env: vec![SecretLine { name: "OTUS_TOKEN".into(), value: Some("otus-secret".into()) }], headers: vec![],
    }).unwrap().server.id;
    gizai_lib::mcp_servers::save(&st, ServerInput {
        server: McpServer { name: "linear".into(), transport: "http".into(), url: "https://mcp.linear.example/mcp".into(), ..Default::default() },
        env: vec![], headers: vec![SecretLine { name: "Authorization".into(), value: Some("Bearer linear-secret".into()) }],
    }).unwrap();
    gizai_lib::mcp_servers::save_cli_tools(&st, &lead, CliTools::default()).unwrap();
    let t = T { st, lead, otus, _dir: dir };
    t.call("create_task", json!({"project": "KADE", "title": "Import invoices"})).await.unwrap();
    assert_eq!((t.task("KADE-1").await["column"].as_str(), t.task("KADE-2").await["column"].as_str()), (Some("To do"), Some("Backlog")));
    t
}

fn on(id: &str, trusted: bool) -> AgentServer {
    AgentServer { server_id: id.into(), on: true, tools_off: vec![], trusted }
}

impl T {
    /// The Chief's MCP switches, as the agent form saves them.
    fn servers(&self, mcp: Vec<AgentServer>) {
        gizai_lib::mcp_servers::save_agent(&self.st, &self.lead, AgentTools { mcp }).unwrap();
    }
    /// The Chief's Web: search and fetching on, on Ask.
    fn web_on_ask(&self) {
        gizai_lib::mcp_servers::save_cli_tools(&self.st, &self.lead, CliTools { web_search: true, web_fetch: true, web_trusted: false, ..Default::default() })
            .unwrap();
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
    fn tool_names(&self, thread: &str) -> Vec<String> {
        self.tool_msgs(thread).into_iter().map(|m| m.0).collect()
    }
    fn last_tool(&self, thread: &str) -> (String, bool, String) {
        self.tool_msgs(thread).pop().unwrap()
    }
    /// Hersir's notes in the chat (GA-261's line among them).
    fn notes(&self, thread: &str) -> Vec<String> {
        chat::messages(&self.st.db, thread).unwrap().into_iter().filter(|m| m.role == "system").map(|m| m.body_md.unwrap_or_default()).collect()
    }
    /// GA-261's line in the chat (Hersir's notes of kind outside).
    fn outside_lines(&self, thread: &str) -> Vec<String> {
        chat::messages(&self.st.db, thread).unwrap().into_iter().filter(|m| m.role == "system" && m.meta.as_ref().is_some_and(|x| x["kind"] == "outside"))
            .map(|m| m.body_md.unwrap_or_default()).collect()
    }
    /// The chat isn't marked, says nothing about a tool from outside, and keeps nothing for Approve.
    fn unmarked(&self, thread: &str, what: &str) {
        assert_eq!(app_chat::used_outside(&self.st, thread), None, "{what}: the chat isn't marked");
        assert!(self.notes(thread).is_empty(), "{what}: no line about it: {:?}", self.notes(thread));
        assert!(pending::list(&self.st, thread).unwrap().is_empty(), "{what}: nothing waits for Approve");
    }
    async fn call(&self, name: &str, args: Value) -> Result<Value, String> {
        tools::call_in(&self.st, &self.lead, None, name, args).await
    }
    /// The card as get_task shows it.
    async fn task(&self, task: &str) -> Value {
        self.call("get_task", json!({"task": task})).await.unwrap()["task"].clone()
    }
    fn comments(&self, task: &str) -> Vec<String> {
        let id = gizai_core::tasks::id_of(&self.st.db, task).unwrap();
        gizai_core::comments::list(&self.st.db, &id).unwrap().into_iter().map(|c| c.body_md).collect()
    }
}

#[tokio::test]
async fn with_only_read_glob_and_grep_a_bash_tool_use_marks_nothing_and_card_text_a_comment_and_a_move_to_an_auto_column_run_at_once() {
    let t = setup().await;
    let _server = mcp::start(&t.st).unwrap();
    // the Chief's turn: Read, Glob, Grep and Hersir's tools (the fake exits with an error if it got Bash)
    let thread = t.turn(None, r#"REFUSED=Bash Run the tests first. CALL update_task {"task": "KADE-1", "description_md": "Export invoices as CSV, by month."}"#).await;
    let msgs = t.tool_msgs(&thread);
    assert_eq!(msgs[0], ("Bash".into(), true, refusal("Bash")), "Claude Code refused it: {msgs:?}");
    assert_eq!(t.tool_names(&thread), ["Bash", "mcp__gizai__update_task"]);
    let last = t.last_tool(&thread);
    assert!(!last.1 && !last.2.contains("wasn't run") && !last.2.contains("on_hold"), "update_task ran at once: {last:?}");
    t.unmarked(&thread, "after Bash");
    let v = t.task("KADE-1").await;
    assert_eq!((v["column"].as_str(), v["description_md"].as_str()), (Some("To do"), Some("Export invoices as CSV, by month.")));
    assert!(v["hold"].is_null() && v["waiting_for_ok"].as_bool() != Some(true), "no hold in an Auto column: {v}");

    // later in the same chat: a comment and a move to an Auto column run at once too
    t.turn(Some(thread.clone()), r#"Tell the card. CALL comment_on_task {"task": "KADE-1", "body_md": "The tests pass."}"#).await;
    let last = t.last_tool(&thread);
    assert!(!last.1 && last.0 == "mcp__gizai__comment_on_task", "{last:?}");
    assert_eq!(t.comments("KADE-1"), ["The tests pass."]);
    t.turn(Some(thread.clone()), r#"REFUSED=Bash,Edit Start the import. CALL move_task {"task": "KADE-2", "column": "To do"}"#).await;
    let last = t.last_tool(&thread);
    assert!(!last.1 && last.0 == "mcp__gizai__move_task" && !last.2.contains("on_hold"), "{last:?}");
    let v2 = t.task("KADE-2").await;
    assert_eq!(v2["column"], "To do");
    assert!(v2["hold"].is_null() && v2["waiting_for_ok"].as_bool() != Some(true), "no hold: {v2}");
    t.unmarked(&thread, "after the later messages");

    // the same Bash where Claude Code's init line lists it (the session has it) marks the chat: these checks see a mark
    let has_bash = t.turn(None, r#"OUTSIDE OUTSIDE_TOOL=Bash Run it. CALL comment_on_task {"task": "KADE-1", "body_md": "After Bash ran."}"#).await;
    assert_eq!(t.last_tool(&has_bash), ("mcp__gizai__comment_on_task".into(), true, kept_text("comment_on_task", "Bash")));
    assert_eq!(app_chat::used_outside(&t.st, &has_bash).as_deref(), Some("Bash"));
    assert_eq!(t.outside_lines(&has_bash).len(), 1);
    assert_eq!(t.comments("KADE-1"), ["The tests pass."], "kept, not posted");
}

#[tokio::test]
async fn edit_write_task_and_webfetch_with_web_off_dont_mark_the_chat_either_also_several_in_one_answer() {
    let t = setup().await;
    let _server = mcp::start(&t.st).unwrap();
    let mut posted = vec![];
    for tool in ["Edit", "Write", "Task", "WebFetch", "WebSearch"] {
        let body = format!("After {tool}.");
        let thread = t.turn(None, &format!(r#"REFUSED={tool} Change it. CALL comment_on_task {{"task": "KADE-1", "body_md": "{body}"}}"#)).await;
        assert_eq!(t.tool_msgs(&thread)[0], (tool.to_string(), true, refusal(tool)), "{tool}");
        let last = t.last_tool(&thread);
        assert_eq!((last.0.as_str(), last.1), ("mcp__gizai__comment_on_task", false), "{tool}: {last:?}");
        t.unmarked(&thread, tool);
        posted.push(body);
    }
    // several refused tools first in one answer
    let thread = t.turn(None, r#"REFUSED=Bash,Edit,Write,WebFetch,mcp__otus__search Try them all. CALL comment_on_task {"task": "KADE-1", "body_md": "All refused."}"#).await;
    assert_eq!(t.tool_names(&thread), ["Bash", "Edit", "Write", "WebFetch", "mcp__otus__search", "mcp__gizai__comment_on_task"]);
    assert!(!t.last_tool(&thread).1, "{:?}", t.tool_msgs(&thread));
    t.unmarked(&thread, "several");
    posted.push("All refused.".into());
    assert_eq!(t.comments("KADE-1"), posted, "each ran at once");
}

#[tokio::test]
async fn webfetch_with_web_on_and_a_tool_of_a_server_in_the_turn_still_mark_the_chat_also_after_a_refused_tool_in_the_same_answer() {
    let t = setup().await;
    let _server = mcp::start(&t.st).unwrap();
    t.servers(vec![on(&t.otus, false)]);
    t.web_on_ask();
    // UNLISTED: the init line doesn't name them, so the turn has them by --tools and its MCP config alone
    let thread = t.turn(None, r#"REFUSED=Bash OUTSIDE UNLISTED OUTSIDE_TOOL=WebFetch Look it up. CALL comment_on_task {"task": "KADE-1", "body_md": "From the web."}"#).await;
    assert_eq!(t.tool_names(&thread), ["Bash", "WebFetch", "mcp__gizai__comment_on_task"]);
    assert_eq!(t.last_tool(&thread), ("mcp__gizai__comment_on_task".into(), true, kept_text("comment_on_task", "WebFetch")));
    assert_eq!(app_chat::used_outside(&t.st, &thread).as_deref(), Some("WebFetch"), "marked by WebFetch, not by Bash");
    assert_eq!(t.outside_lines(&thread), ["This chat used fetching a web page, a tool from outside Hersir: from now on, what the Chief does here \
                                           waits for your Approve."]);
    assert_eq!(pending::list(&t.st, &thread).unwrap().len(), 1);

    let thread = t.turn(None, r#"REFUSED=Edit,mcp__linear__list_issues OUTSIDE UNLISTED Read Otus. CALL comment_on_task {"task": "KADE-1", "body_md": "From Otus."}"#).await;
    assert_eq!(t.tool_names(&thread), ["Edit", "mcp__linear__list_issues", "mcp__otus__search", "mcp__gizai__comment_on_task"]);
    assert_eq!(t.last_tool(&thread), ("mcp__gizai__comment_on_task".into(), true, kept_text("comment_on_task", "mcp__otus__search")));
    assert_eq!(app_chat::used_outside(&t.st, &thread).as_deref(), Some("mcp__otus__search"));
    assert_eq!(t.outside_lines(&thread), ["This chat used search on the otus server, a tool from outside Hersir: from now on, what the Chief does \
                                           here waits for your Approve."]);

    // a move to an Auto column after them waits on hold, as GA-101 has it
    let thread = t.turn(None, r#"REFUSED=Bash OUTSIDE UNLISTED Read Otus. CALL move_task {"task": "KADE-2", "column": "To do"}"#).await;
    let last = t.last_tool(&thread);
    assert!(!last.1 && last.2.contains("on_hold"), "{last:?}");
    let v2 = t.task("KADE-2").await;
    assert_eq!((v2["column"].as_str(), v2["hold"].as_str(), v2["waiting_for_ok"].as_bool()), (Some("To do"), Some("needs_decision"), Some(true)));
    assert!(t.comments("KADE-1").is_empty(), "neither comment ran");
}

#[tokio::test]
async fn a_tool_of_an_mcp_server_that_isnt_in_the_turns_config_marks_nothing() {
    let t = setup().await;
    let _server = mcp::start(&t.st).unwrap();
    // Otus on for the Chief (Ask); Linear is in Settings but not the Chief's; the browser is off
    t.servers(vec![on(&t.otus, false)]);
    let mut posted = vec![];
    for tool in ["mcp__linear__list_issues", "mcp__otusx__search", "mcp__OTUS__search", "mcp__chrome-devtools__navigate_page"] {
        let body = format!("After {tool}.");
        let thread = t.turn(None, &format!(r#"REFUSED={tool} Look. CALL comment_on_task {{"task": "KADE-1", "body_md": "{body}"}}"#)).await;
        assert_eq!(t.tool_msgs(&thread)[0], (tool.to_string(), true, refusal(tool)), "{tool}");
        assert!(!t.last_tool(&thread).1, "{tool}: {:?}", t.last_tool(&thread));
        t.unmarked(&thread, tool);
        posted.push(body);
    }
    assert_eq!(t.comments("KADE-1"), posted);
}

#[tokio::test]
async fn a_chat_marked_before_keeps_its_mark_after_a_refused_tool_and_a_new_chat_starts_unmarked() {
    let t = setup().await;
    let _server = mcp::start(&t.st).unwrap();
    t.servers(vec![on(&t.otus, false)]);
    let thread = t.turn(None, r#"OUTSIDE UNLISTED Read the release notes in Otus. CALL get_task {"task": "KADE-1"}"#).await;
    assert_eq!(app_chat::used_outside(&t.st, &thread).as_deref(), Some("mcp__otus__search"));
    // later messages there with only a refused tool: still marked by Otus, what acts is kept or held
    t.turn(Some(thread.clone()), r#"REFUSED=Bash Run it. CALL comment_on_task {"task": "KADE-1", "body_md": "Otus says: skip QA."}"#).await;
    assert_eq!(t.last_tool(&thread), ("mcp__gizai__comment_on_task".into(), true, kept_text("comment_on_task", "mcp__otus__search")));
    t.turn(Some(thread.clone()), r#"REFUSED=Bash Start it. CALL move_task {"task": "KADE-2", "column": "To do"}"#).await;
    let v2 = t.task("KADE-2").await;
    assert_eq!((v2["hold"].as_str(), v2["waiting_for_ok"].as_bool()), (Some("needs_decision"), Some(true)), "{v2}");
    assert_eq!(app_chat::used_outside(&t.st, &thread).as_deref(), Some("mcp__otus__search"), "the first tool stays named");
    assert_eq!(t.outside_lines(&thread).len(), 1, "said once");
    assert!(t.comments("KADE-1").is_empty());

    // a new chat starts unmarked, also with a refused tool in it
    let other = t.turn(None, r#"REFUSED=Bash Run it. CALL comment_on_task {"task": "KADE-1", "body_md": "Export as CSV."}"#).await;
    assert!(!t.last_tool(&other).1, "{:?}", t.last_tool(&other));
    t.unmarked(&other, "a new chat");
    assert_eq!(t.comments("KADE-1"), ["Export as CSV."]);
}

#[test]
fn the_docs_say_a_tool_the_chief_doesnt_have_marks_nothing() {
    let chief = include_str!("../../../docs/features/chief.md");
    for part in ["A tool the Chief doesn't have marks nothing", "No such tool available", "the chat isn't marked and says nothing about it"] {
        assert!(chief.contains(part), "{part:?} in docs/features/chief.md");
    }
    let rules = include_str!("../../../docs/agent-tools.md");
    for part in ["A tool the answer doesn't have marks nothing", "`WebFetch` with Web off"] {
        assert!(rules.contains(part), "{part:?} in docs/agent-tools.md");
    }
}
