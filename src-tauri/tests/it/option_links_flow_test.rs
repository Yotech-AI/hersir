// Linux and macOS only: these tests run shell scripts as fake programs (and a link's probe through sh), which Windows can't.
#![cfg(unix)]
//! GA-318 (GA-270 #1 and #3, CWE-88), end to end: the Chief can't save a repository link git would read as an option; a
//! link like that saved before the fix stops a card run, the start point and the chat's fetch with a sentence, and git
//! never runs it (a probe file that must not appear); and a tool, an allowed command or an MCP server named like one of
//! Claude Code's options, saved before the fix, never reaches Claude Code's argv. Runs use fake Claude Codes, never the
//! real one; "GitHub" is never asked.
use std::path::{Path, PathBuf};
use std::time::Duration;

use gizai_agents::stream::RunEvent;
use gizai_core::clis::Cli;
use gizai_core::mcp_servers::{self as core_mcp, AgentServer, AgentTools, CliTools, McpServer};
use gizai_core::model::{AgentInput, Project, ProjectInput};
use gizai_lib::{AppState, tools};
use serde_json::{Value, json};

const FAKE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude.sh");
const FAKE_RUN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude-mcp-run.sh");
/// The audit's link: git took it as `--upload-pack=…` and ran it through sh, in the repository's folder.
const AUDIT: &str = "--upload-pack=touch${IFS}PWNED;#@x:y";
const SKIP: &str = "--dangerously-skip-permissions";

fn git_repo(dir: &Path) -> PathBuf {
    let repo = dir.join("repo");
    std::fs::create_dir(&repo).unwrap();
    for a in [&["init", "-q", "-b", "main"][..], &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "i"][..]] {
        assert!(std::process::Command::new("git").args(a).current_dir(&repo).status().unwrap().success());
    }
    repo
}

fn sentence(link: &str) -> String {
    format!("{link} isn't a repository link: it, or its user name, host or port, starts with \"-\", which git reads as an option")
}

fn project(st: &AppState, key: &str) -> Project {
    gizai_core::projects::list(&st.db).unwrap().into_iter().find(|p| p.key == key).unwrap()
}

/// Puts `link` in the database as the project's link the way a link saved before GA-318 is there: never checked.
fn store_link(st: &AppState, key: &str, link: &str) {
    let id = project(st, key).id;
    st.db.write(None, |w| {
        w.conn().execute("UPDATE repos SET remote_url=?2 WHERE project_id=?1 AND deleted_at IS NULL", [id.as_str(), link])?;
        Ok(())
    }).unwrap();
    assert_eq!(project(st, key).repo_url.as_deref(), Some(link), "saved as before GA-318");
}

fn sql(st: &AppState, q: &str, params: [&str; 2]) {
    st.db.write(None, |w| {
        w.conn().execute(q, params)?;
        Ok(())
    }).unwrap();
}

const BAD_LINKS: [&str; 4] = [AUDIT, "-oProxyCommand=x", "git@-oProxyCommand=x:acme/app.git", "ssh://-oProxyCommand=x/acme/app.git"];

#[tokio::test]
async fn the_chiefs_create_project_update_project_and_the_project_form_refuse_a_link_git_would_read_as_an_option() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    gizai_core::settings::set(&st.db, "claude_bin", &FAKE.to_string()).unwrap();
    let team_id = gizai_core::team::list(&st.db).unwrap()[0].id.clone();
    let lead = gizai_core::team::add_agent(&st.db, &st.you_id, &team_id, AgentInput {
        name: "Team Lead".into(), role_key: "lead".into(), chat_enabled: Some(true), ..Default::default() }).unwrap();
    let repo = git_repo(tmp.path()).display().to_string();
    let before = gizai_core::projects::list(&st.db).unwrap().len();
    for bad in BAD_LINKS {
        for arg in ["repository", "github"] {
            let e = tools::call(&st, &lead, "create_project", json!({"name": "Shop", "key": "SHOP", "repo_path": repo, arg: bad})).await.unwrap_err();
            assert!(e.contains(&sentence(bad)), "{arg} {bad}: {e}");
        }
    }
    assert_eq!(gizai_core::projects::list(&st.db).unwrap().len(), before, "no project made");

    tools::call(&st, &lead, "create_project", json!({"name": "Shop", "key": "SHOP", "repo_path": repo, "repository": "git@github.com:acme-labs/shop-app.git"}))
        .await.unwrap();
    for bad in BAD_LINKS {
        for arg in ["repository", "github"] {
            let e = tools::call(&st, &lead, "update_project", json!({"project": "SHOP", arg: bad})).await.unwrap_err();
            assert!(e.contains(&sentence(bad)), "{arg} {bad}: {e}");
        }
    }
    let p = project(&st, "SHOP");
    assert_eq!(p.repo_url.as_deref(), Some("https://github.com/acme-labs/shop-app"), "nothing changed");
    // the project form's Save (save_project) goes through runs::update_project, and projects::create for a new project
    for bad in BAD_LINKS {
        let input = ProjectInput { name: p.name.clone(), key: p.key.clone(), repo_path: p.repo_path.clone(), repo_url: Some(bad.into()),
                                   default_branch: Some("main".into()), ..Default::default() };
        assert_eq!(gizai_lib::runs::update_project(&st, &st.you_id, &p.id, input.clone()).unwrap_err().to_string(), sentence(bad));
        let new = ProjectInput { key: "NEW".into(), name: "New".into(), ..input };
        assert_eq!(gizai_core::projects::create(&st.db, &st.you_id, new).unwrap_err().to_string(), sentence(bad));
    }
    assert_eq!(project(&st, "SHOP").repo_url.as_deref(), Some("https://github.com/acme-labs/shop-app"), "nothing changed");
    // a usual link still saves from the form
    let input = ProjectInput { name: p.name.clone(), key: p.key.clone(), repo_path: p.repo_path.clone(),
                               repo_url: Some("git@ssh.bitbucket.org:acme/shop-app.git".into()), default_branch: Some("main".into()), ..Default::default() };
    gizai_lib::runs::update_project(&st, &st.you_id, &p.id, input).unwrap();
    assert_eq!(project(&st, "SHOP").repo_url.as_deref(), Some("https://bitbucket.org/acme/shop-app"));
}

#[tokio::test]
async fn a_link_saved_before_that_git_would_read_as_an_option_stops_a_card_run_with_a_sentence_and_git_never_runs_it() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    let repo = git_repo(tmp.path());
    let task = gizai_lib::test_task(&st, repo.to_str().unwrap(), "backend");
    let probe = tmp.path().join("probe");
    let links = [AUDIT.to_string(), format!("--upload-pack=touch${{IFS}}{};#@x:y", probe.display()), format!("-oProxyCommand=touch {}", probe.display())];
    for link in &links {
        store_link(&st, "KADE", link);
        let e = gizai_lib::runs::run_once(&st, &task, None, Some(FAKE.into())).await.unwrap_err();
        assert!(e.contains(&format!("{}: change Kade's link on its page (Edit)", sentence(link))), "{link}: {e}");
        // every start (a card run, the chat's copies, Changes, the Editor, the push after a run) asks here first
        let p = project(&st, "KADE");
        for fetch in [Some(gizai_lib::git::START_FETCH_LIMIT), None] {
            let e = gizai_lib::git::start_point(&p, &repo, fetch).unwrap_err().to_string();
            assert!(e.contains(&sentence(link)), "{link}, fetch {fetch:?}: {e}");
        }
        assert!(!probe.exists() && !repo.join("PWNED").exists(), "{link} ran a command");
    }
    assert!(gizai_core::runs::list_for_task(&st.db, &task).unwrap().iter().all(|r| r.worktree_path.is_none()), "no worktree made");
    // a usual link (here a local path) still starts the card
    let github = tmp.path().join("github.git");
    assert!(std::process::Command::new("git").args(["clone", "-q", "--bare", repo.to_str().unwrap(), github.to_str().unwrap()]).status().unwrap().success());
    store_link(&st, "KADE", github.to_str().unwrap());
    let s = gizai_lib::runs::run_once(&st, &task, None, Some(FAKE.into())).await.unwrap();
    assert_eq!(s.status, "succeeded", "{:?}", s.error);
}

#[tokio::test]
async fn the_chats_fetch_refuses_a_link_saved_before_that_git_would_read_as_an_option_whole_in_the_turns_line() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    let repo = git_repo(tmp.path());
    gizai_lib::test_task(&st, repo.to_str().unwrap(), "backend");
    assert_eq!(gizai_lib::code::projects_with_code(&st).len(), 1, "KADE gets a copy");
    store_link(&st, "KADE", AUDIT);
    st.code.set_timing(Duration::ZERO, gizai_lib::code::TURN_WAIT);
    let turn = gizai_lib::code::before_turn(&st, "thread-1").await;
    let want = format!("{}: change Kade's link on its page (Edit)", sentence(AUDIT));
    assert!(turn.line.contains("KADE has no copy") && turn.line.contains(&want), "{}", turn.line);
    assert!(turn.dirs.is_empty(), "{:?}", turn.dirs);
    assert!(!repo.join("PWNED").exists() && !gizai_lib::code::dir_of(&st, "KADE").join("PWNED").exists(), "the link ran a command");
}

/// Adds a CLI in Settings → Coding CLIs and returns its id.
fn add_cli(st: &AppState, name: &str, command: &str, env: &[String]) -> String {
    let mut list: Vec<Cli> = gizai_core::clis::list(&st.db).unwrap();
    list.push(Cli { name: name.into(), kind: "claude_code".into(), command: command.into(), env: env.to_vec(), ..Default::default() });
    gizai_lib::clis::save(st, list).unwrap().into_iter().find(|c| c.cli.name == name).unwrap().cli.id
}

#[tokio::test]
async fn a_tool_an_allowed_command_or_an_mcp_server_named_like_an_option_saved_before_never_reaches_claude_codes_argv() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    let repo = git_repo(tmp.path());
    // never the real claude, whatever the agent runs on
    gizai_core::settings::set(&st.db, "claude_bin", &FAKE_RUN.to_string()).unwrap();
    let task = gizai_lib::test_task(&st, repo.to_str().unwrap(), "backend");
    let out = tmp.path().join("fake-out");
    let cli = add_cli(&st, "Claude Code (fake)", FAKE_RUN, &[format!("FAKE_MCP_OUT={}", out.display())]);
    let (_, a) = gizai_core::team::all_agents(&st.db).unwrap().into_iter().find(|(_, m)| m.role_key == "backend").unwrap();
    gizai_core::team::update_agent(&st.db, &st.you_id, &a.actor_id, AgentInput { name: a.name.clone(), role_key: "backend".into(), adapter: cli,
        allowed_tools: vec!["Bash(git status:*)".into(), "Bash(npm test:*)".into()], permission_mode: "acceptEdits".into(), ..Default::default() }).unwrap();
    let agent = a.actor_id.clone();
    let srv = core_mcp::save(&st.db, McpServer { name: "oddname".into(), transport: "stdio".into(), command: "/bin/true".into(), ..Default::default() }).unwrap();
    gizai_lib::mcp_servers::save_agent(&st, &agent, AgentTools { mcp: vec![AgentServer { server_id: srv.id.clone(), on: true, tools_off: vec![], trusted: false }] })
        .unwrap();
    gizai_lib::mcp_servers::save_cli_tools(&st, &agent, CliTools { builtin: vec!["FancyNewTool".into()], ..Default::default() }).unwrap();

    // As saved before GA-318, or restored from a backup or a shared template: the server renamed "-x", the built-in tool
    // and the allowed command called like Claude Code's option that turns off every permission check.
    let mut servers = core_mcp::list(&st.db).unwrap();
    servers.iter_mut().find(|x| x.id == srv.id).unwrap().name = "-x".into();
    gizai_core::settings::set(&st.db, "mcp_servers", &servers).unwrap();
    let raw: String = st.db.read(|c| Ok(c.query_row("SELECT mcp_extra_json FROM agent_configs WHERE actor_id=?1", [&agent], |r| r.get(0))?)).unwrap();
    let mut extra: Value = serde_json::from_str(&raw).unwrap();
    extra["cli"]["builtin"] = json!(["FancyNewTool", SKIP, "-p"]);
    sql(&st, "UPDATE agent_configs SET mcp_extra_json=?2 WHERE actor_id=?1", [&agent, &extra.to_string()]);
    let allowed = json!(["Bash(git status:*)", SKIP, "Bash(npm test:*)", "--permission-mode=bypassPermissions"]).to_string();
    sql(&st, "UPDATE agent_configs SET allowed_tools_json=?2 WHERE actor_id=?1", [&agent, &allowed]);
    let a = gizai_core::team::agent(&st.db, &agent).unwrap();
    assert!(a.allowed_tools.contains(&SKIP.to_string()) && a.cli_tools.builtin.contains(&SKIP.to_string()), "saved as before GA-318");

    let s = gizai_lib::runs::run_once(&st, &task, None, None).await.unwrap();
    assert_eq!(s.status, "succeeded", "{:?}", s.error);
    let argv: Vec<String> = std::fs::read_to_string(out.join("argv")).unwrap().lines().map(str::to_string).collect();
    for x in &argv {
        assert!(!x.contains("dangerously") && !x.contains("bypassPermissions") && x != "-x" && !x.contains("mcp__-x"), "{x} in {argv:?}");
    }
    assert_eq!(argv.iter().filter(|x| *x == "-p").count(), 1, "only Hersir's own -p: {argv:?}");
    assert_eq!(argv[argv.iter().position(|x| x == "--permission-mode").unwrap() + 1], "acceptEdits", "{argv:?}");
    // allowed and refused tools only as the values of --allowedTools= and --disallowedTools=, each rule starting with a letter
    assert!(!argv.iter().any(|x| x == "--allowedTools" || x == "--disallowedTools"), "{argv:?}");
    let tools: Vec<&String> = argv.iter().filter(|x| x.starts_with("--allowedTools=")).collect();
    assert_eq!(tools.len(), 1, "{argv:?}");
    let rules: Vec<&str> = tools[0]["--allowedTools=".len()..].split(',').collect();
    assert!(rules.iter().all(|r| r.starts_with(|c: char| c.is_ascii_alphabetic())), "{rules:?}");
    for kept in ["Bash(git status:*)", "Bash(npm test:*)", "FancyNewTool"] {
        assert!(rules.contains(&kept), "{kept} stays: {rules:?}");
    }
    // the server isn't in the run's MCP config, and the run says why
    let config: Value = serde_json::from_str(&std::fs::read_to_string(out.join("mcp.json")).unwrap_or_else(|_| "{}".into())).unwrap();
    assert!(config["mcpServers"].get("-x").is_none(), "{config}");
    let notes: Vec<String> = gizai_lib::runs::events_for(&st, &s.run_id).into_iter()
        .filter_map(|e| match e.event { RunEvent::Note { text } => Some(text), _ => None }).collect();
    assert!(notes.iter().any(|n| n.contains("Left out -x: rename it in Settings → MCP servers (an MCP server's name starts with a letter")), "{notes:?}");
    assert!(notes.iter().any(|n| n.contains(&format!("{SKIP} isn't a rule: a rule starts with a tool's name, which starts with a letter"))), "{notes:?}");
}
