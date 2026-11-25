//! GA-266: secrets in runs. What a run keeps of Hersir's environment (`run_env`), its MCP config gone once the CLI's
//! init line is in and in any case when the run ends (`process::spawn_exec`), and the process tree a turn's token works
//! for (`os::Tree::contains`). The environment a run's CLI really gets, with keys set in the test process's own
//! environment, is checked in tests/run_env_spawn_test.rs, a program of its own. The socket's side is in
//! src-tauri/tests/it/mcp_socket_test.rs.
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use gizai_agents::claude::ClaudeArgs;
use gizai_agents::cli::{CliSpec, Kind, TaskRun, task_exec};
use gizai_agents::os::{self, End};
use gizai_agents::run_env::{self, left_out};
use tokio::io::{AsyncBufReadExt, BufReader};

fn vars(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    pairs.iter().map(|(k, v)| (OsString::from(k), OsString::from(v))).collect()
}

fn set(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn sorted(names: Vec<OsString>) -> Vec<String> {
    let mut n: Vec<String> = names.into_iter().map(|s| s.to_string_lossy().into_owned()).collect();
    n.sort();
    n
}

/// Hersir's environment in these tests: the card's three keys, each CLI's own, other secrets and plain variables.
fn hersir() -> Vec<(OsString, OsString)> {
    vars(&[
        ("PATH", "/usr/bin"), ("HOME", "/home/u"), ("CLAUDE_CONFIG_DIR", "/home/u/.claude-2"), ("TOKENIZERS_PARALLELISM", "false"),
        ("OPENAI_API_KEY", "sk-openai"), ("GITHUB_TOKEN", "ghp_x"), ("AWS_SECRET_ACCESS_KEY", "aws-secret"), ("AWS_REGION", "eu-west-1"),
        ("ANTHROPIC_API_KEY", "sk-ant"), ("CLAUDE_CODE_OAUTH_TOKEN", "oat"), ("CODEX_API_KEY", "cx"), ("GEMINI_API_KEY", "gm"),
        ("GOOGLE_API_KEY", "gg"), ("DB_PASSWORD", "pw"), ("my_client_secret", "s"), ("NPM_TOKEN", "npm"),
    ])
}

#[test]
fn a_secrets_name_is_one_of_the_cards_patterns_in_any_case() {
    for name in ["OPENAI_API_KEY", "GITHUB_TOKEN", "AWS_SECRET_ACCESS_KEY", "AWS_REGION", "AWS_PROFILE", "DB_PASSWORD",
                 "MYSQL_ROOT_PASSWORD", "my_client_secret", "CLIENT_SECRET_ID", "npm_token", "openai_api_key", "aws_profile"] {
        assert!(run_env::secret(name.as_ref()), "{name} has a secret's name");
    }
    for name in ["PATH", "HOME", "TOKENIZERS_PARALLELISM", "API_KEY_FILE", "GITHUB_TOKEN_PATH", "SSH_AUTH_SOCK",
                 "XAWS_REGION", "CLAUDE_CONFIG_DIR", "TOKEN"] {
        assert!(!run_env::secret(name.as_ref()), "{name} isn't a secret's name");
    }
}

#[test]
fn a_claude_code_run_gets_none_of_the_three_keys_but_keeps_its_own_and_plain_variables() {
    let out = sorted(left_out(hersir(), Some(Kind::ClaudeCode), &[]));
    assert_eq!(out, ["AWS_REGION", "AWS_SECRET_ACCESS_KEY", "CODEX_API_KEY", "DB_PASSWORD", "GEMINI_API_KEY", "GITHUB_TOKEN",
                     "GOOGLE_API_KEY", "NPM_TOKEN", "OPENAI_API_KEY", "my_client_secret"]);
    // each of Claude Code's own sign-in keys stays, whatever its name looks like
    for own in run_env::own(Kind::ClaudeCode) {
        let env = vars(&[(*own, "v"), ("GITHUB_TOKEN", "g")]);
        assert_eq!(sorted(left_out(env, Some(Kind::ClaudeCode), &[])), ["GITHUB_TOKEN"], "{own}");
    }
    assert!(run_env::own(Kind::ClaudeCode).contains(&"ANTHROPIC_API_KEY"));
    assert!(run_env::own(Kind::ClaudeCode).contains(&"CLAUDE_CODE_OAUTH_TOKEN"));
}

#[test]
fn a_codex_run_still_has_the_key_codex_reads_and_none_of_the_others() {
    let out = sorted(left_out(hersir(), Some(Kind::Codex), &[]));
    assert!(!out.contains(&"OPENAI_API_KEY".to_string()), "{out:?}");
    assert!(!out.contains(&"CODEX_API_KEY".to_string()), "{out:?}");
    for gone in ["GITHUB_TOKEN", "AWS_SECRET_ACCESS_KEY", "ANTHROPIC_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN", "GEMINI_API_KEY"] {
        assert!(out.contains(&gone.to_string()), "{gone}: {out:?}");
    }
}

#[test]
fn a_gemini_run_keeps_geminis_keys_and_another_cli_or_none_keeps_no_secret() {
    let gemini = sorted(left_out(hersir(), Some(Kind::Gemini), &[]));
    assert!(!gemini.contains(&"GEMINI_API_KEY".to_string()) && !gemini.contains(&"GOOGLE_API_KEY".to_string()), "{gemini:?}");
    assert!(gemini.contains(&"OPENAI_API_KEY".to_string()) && gemini.contains(&"ANTHROPIC_API_KEY".to_string()), "{gemini:?}");
    let every_secret = ["ANTHROPIC_API_KEY", "AWS_REGION", "AWS_SECRET_ACCESS_KEY", "CLAUDE_CODE_OAUTH_TOKEN", "CODEX_API_KEY",
                        "DB_PASSWORD", "GEMINI_API_KEY", "GITHUB_TOKEN", "GOOGLE_API_KEY", "NPM_TOKEN", "OPENAI_API_KEY",
                        "my_client_secret"];
    assert_eq!(sorted(left_out(hersir(), Some(Kind::Other), &[])), every_secret);
    assert_eq!(sorted(left_out(hersir(), None, &[])), every_secret);
    assert!(run_env::own(Kind::Other).is_empty());
}

#[test]
fn what_the_clis_settings_set_the_run_keeps() {
    let lines = set(&[("GITHUB_TOKEN", "from-settings"), ("AWS_SECRET_ACCESS_KEY", "from-settings")]);
    let out = sorted(left_out(hersir(), Some(Kind::ClaudeCode), &lines));
    assert!(!out.contains(&"GITHUB_TOKEN".to_string()) && !out.contains(&"AWS_SECRET_ACCESS_KEY".to_string()), "{out:?}");
    assert!(out.contains(&"OPENAI_API_KEY".to_string()) && out.contains(&"AWS_REGION".to_string()), "{out:?}");
    // another CLI gets its key only from its lines in Settings → Coding CLIs
    let out = sorted(left_out(hersir(), Some(Kind::Other), &set(&[("OPENAI_API_KEY", "x")])));
    assert!(!out.contains(&"OPENAI_API_KEY".to_string()), "{out:?}");
}

#[test]
fn claude_code_on_amazon_bedrock_keeps_every_aws_variable_and_only_then() {
    let mut env = hersir();
    env.push(("CLAUDE_CODE_USE_BEDROCK".into(), "1".into()));
    let out = sorted(left_out(env.clone(), Some(Kind::ClaudeCode), &[]));
    assert!(!out.iter().any(|n| n.starts_with("AWS_")), "{out:?}");
    assert!(out.contains(&"GITHUB_TOKEN".to_string()) && out.contains(&"OPENAI_API_KEY".to_string()), "{out:?}");
    // only Claude Code signs in with them
    let codex = sorted(left_out(env.clone(), Some(Kind::Codex), &[]));
    assert!(codex.contains(&"AWS_SECRET_ACCESS_KEY".to_string()), "{codex:?}");
    // the CLI's settings win over Hersir's environment, either way
    let off = sorted(left_out(env, Some(Kind::ClaudeCode), &set(&[("CLAUDE_CODE_USE_BEDROCK", "0")])));
    assert!(off.contains(&"AWS_SECRET_ACCESS_KEY".to_string()), "{off:?}");
    for on in ["1", "true", "TRUE", "yes", " on "] {
        let out = sorted(left_out(hersir(), Some(Kind::ClaudeCode), &set(&[("CLAUDE_CODE_USE_BEDROCK", on)])));
        assert!(!out.contains(&"AWS_SECRET_ACCESS_KEY".to_string()), "{on:?}: {out:?}");
    }
    for off in ["0", "false", "no", ""] {
        let mut env = hersir();
        env.push(("CLAUDE_CODE_USE_BEDROCK".into(), off.into()));
        let out = sorted(left_out(env, Some(Kind::ClaudeCode), &[]));
        assert!(out.contains(&"AWS_SECRET_ACCESS_KEY".to_string()), "{off:?}: {out:?}");
    }
}

#[test]
fn each_clis_start_says_which_cli_it_is_and_claude_code_carries_its_mcp_config() {
    let config = PathBuf::from("/data/runs/R1.mcp.json");
    let exec = ClaudeArgs { bin: "claude".into(), prompt: "p".into(), mcp_config: Some(config.clone()), ..Default::default() }.exec();
    assert_eq!(exec.kind, Some(Kind::ClaudeCode));
    assert_eq!(exec.mcp_config.as_deref(), Some(config.as_path()));
    let run = TaskRun { prompt: "p".into(), session_id: "S".into(), mcp_config: Some(config.clone()), ..Default::default() };
    for kind in [Kind::ClaudeCode, Kind::Codex, Kind::Gemini, Kind::Other] {
        let cli = CliSpec { kind, bin: "cli".into(), env: vec![], args: String::new() };
        assert_eq!(task_exec(&cli, &run).kind, Some(kind), "{kind:?}");
    }
    let claude = CliSpec { kind: Kind::ClaudeCode, bin: "claude".into(), env: vec![], args: String::new() };
    assert_eq!(task_exec(&claude, &run).mcp_config.as_deref(), Some(config.as_path()), "a task run's MCP config");
}

// ---- the MCP config: gone once the init line is in, and in any case at the end (Linux and macOS: fake-claude.sh) ----

#[cfg(unix)]
mod config_gone {
    use super::*;
    use std::path::Path;
    use gizai_agents::chat_stream::ChatEvent;
    use gizai_agents::process::{AFTER_RESULT, Caps, RunHandle, StreamEvent, spawn};
    use gizai_agents::stream::RunEvent;

    fn caps() -> Caps {
        Caps { max_time: Duration::from_secs(60), max_tool_calls: 80, after_result: AFTER_RESULT }
    }

    /// The run's MCP config as Hersir writes one (0600), with a server's secret in it.
    fn config_in(dir: &Path) -> PathBuf {
        let p = dir.join("R1.mcp.json");
        let config = serde_json::json!({"mcpServers": {"docs": {"type": "stdio", "command": "docs-server", "env": {"DOCS_TOKEN": "s3cret"}}}});
        gizai_agents::mcp_run::write_config(&p, &config).unwrap();
        assert!(p.is_file());
        p
    }

    fn claude(prompt: &str, config: &Path, env: Vec<(String, String)>) -> ClaudeArgs {
        ClaudeArgs { bin: concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fake-claude.sh").into(), prompt: prompt.into(),
                     session_id: "S".into(), permission_mode: "acceptEdits".into(), mcp_config: Some(config.into()), env,
                     ..Default::default() }
    }

    async fn drain<E: StreamEvent>(h: &mut RunHandle<E>) -> Vec<E> {
        let mut evs = vec![];
        while let Some(e) = h.events.recv().await {
            evs.push(e);
        }
        evs
    }

    fn exit_of(evs: &[RunEvent]) -> &str {
        match evs.last() {
            Some(RunEvent::Other { raw_type }) if raw_type.starts_with("exit:") => raw_type,
            other => panic!("the run should end with its exit, not {other:?}"),
        }
    }

    #[tokio::test]
    async fn the_mcp_config_is_there_while_claude_code_starts_and_gone_once_its_init_line_is_in() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_in(tmp.path());
        let gate = tmp.path().join("gate");
        // the fake waits for the gate before it prints anything: Claude Code still starting
        let args = claude("p", &config, vec![("FAKE_GATE".into(), gate.display().to_string())]);
        let mut h = spawn::<RunEvent>(&args, tmp.path(), &tmp.path().join("r.jsonl"), caps()).unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(config.is_file(), "the MCP config went before Claude Code could read it");
        std::fs::write(&gate, "").unwrap();
        let first = tokio::time::timeout(Duration::from_secs(20), h.events.recv()).await.unwrap().unwrap();
        assert!(matches!(first, RunEvent::Init { .. }), "{first:?}");
        assert!(!config.exists(), "the MCP config is still there after the init line");
        let rest = drain(&mut h).await;
        assert!(rest.iter().any(|e| matches!(e, RunEvent::Result { .. })), "the run goes on as before: {rest:?}");
        assert_eq!(exit_of(&rest), "exit:0");
        assert!(tmp.path().join("r.jsonl").is_file() && gate.is_file(), "only the MCP config goes");
    }

    #[tokio::test]
    async fn a_task_runs_mcp_config_is_gone_while_its_cli_still_runs() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_in(tmp.path());
        // "hang": the init line, then it waits until it is stopped
        let mut h = spawn::<RunEvent>(&claude("hang", &config, vec![]), tmp.path(), &tmp.path().join("r.jsonl"), caps()).unwrap();
        let first = tokio::time::timeout(Duration::from_secs(20), h.events.recv()).await.unwrap().unwrap();
        assert!(matches!(first, RunEvent::Init { .. }), "{first:?}");
        assert!(!config.exists(), "the MCP config is still there after the init line");
        assert!(h.stop.tree().alive(), "the CLI still runs");
        h.stop.stop();
        assert_eq!(exit_of(&drain(&mut h).await), "exit:130");
    }

    #[tokio::test]
    async fn a_chat_turns_mcp_config_is_gone_once_its_init_line_is_in() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_in(tmp.path());
        let mut h = spawn::<ChatEvent>(&claude("hang", &config, vec![]), tmp.path(), &tmp.path().join("r.jsonl"), caps()).unwrap();
        let first = tokio::time::timeout(Duration::from_secs(20), h.events.recv()).await.unwrap().unwrap();
        assert!(matches!(first, ChatEvent::Init { .. }), "{first:?}");
        assert!(!config.exists(), "the chat turn's MCP config is still there after the init line");
        h.stop.stop();
        drain(&mut h).await;
    }

    #[tokio::test]
    async fn a_run_that_fails_before_its_init_line_leaves_no_mcp_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_in(tmp.path());
        // FAKE_CRASH: an error on stderr and exit 1, no line on stdout
        let mut h = spawn::<RunEvent>(&claude("FAKE_CRASH", &config, vec![]), tmp.path(), &tmp.path().join("r.jsonl"), caps()).unwrap();
        let evs = drain(&mut h).await;
        assert!(!evs.iter().any(|e| matches!(e, RunEvent::Init { .. })), "{evs:?}");
        assert_eq!(exit_of(&evs), "exit:1");
        assert!(!config.exists(), "a run that failed before its init line left its MCP config");
    }

    #[tokio::test]
    async fn a_run_stopped_before_its_init_line_leaves_no_mcp_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_in(tmp.path());
        let args = claude("p", &config, vec![("FAKE_GATE".into(), tmp.path().join("never").display().to_string())]);
        let mut h = spawn::<RunEvent>(&args, tmp.path(), &tmp.path().join("r.jsonl"), caps()).unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(config.is_file());
        h.stop.stop();
        let evs = drain(&mut h).await;
        assert!(!evs.iter().any(|e| matches!(e, RunEvent::Init { .. })), "{evs:?}");
        assert!(!config.exists(), "a run stopped before its init line left its MCP config");
    }

    #[tokio::test]
    async fn a_run_whose_cli_cant_start_leaves_no_mcp_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_in(tmp.path());
        let args = ClaudeArgs { bin: tmp.path().join("no-such-claude"), ..claude("p", &config, vec![]) };
        assert!(spawn::<RunEvent>(&args, tmp.path(), &tmp.path().join("r.jsonl"), caps()).is_err());
        assert!(!config.exists(), "a run that couldn't start left its MCP config");
    }
}

// ---- the tree a turn's token works for (Tree::contains): node as the CLI, so it runs on every system ----

fn node() -> PathBuf {
    os::find_in("node", &std::env::var_os("PATH").unwrap_or_default()).expect("node on PATH")
}

/// Starts a stand-in CLI as a run's CLI starts (`spawn_tree_tokio`); with `child`, it starts a child of its own and
/// prints that child's pid.
fn stand_in(child: bool) -> (tokio::process::Child, os::Tree) {
    let script = if child {
        "const c = require('child_process').spawn(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], { stdio: 'ignore' }); \
         console.log(c.pid); setInterval(() => {}, 1000)"
    } else {
        "setInterval(() => {}, 1000)"
    };
    let mut cmd = os::tokio_command(node());
    cmd.args(["-e", script]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    os::spawn_tree_tokio(&mut cmd).unwrap()
}

#[tokio::test]
async fn a_runs_tree_holds_its_cli_and_what_that_starts_and_no_other_process() {
    let (mut cli, tree) = stand_in(true);
    let cli_pid = cli.id().unwrap();
    let mut lines = BufReader::new(cli.stdout.take().unwrap()).lines();
    let line = tokio::time::timeout(Duration::from_secs(20), lines.next_line()).await.unwrap().unwrap().unwrap();
    let its_child: u32 = line.trim().parse().unwrap();
    let (mut other, other_tree) = stand_in(false);
    let other_pid = other.id().unwrap();

    assert!(tree.contains(cli_pid), "the run's CLI");
    assert!(tree.contains(its_child), "what the CLI started (the MCP shim, say)");
    assert!(!tree.contains(std::process::id()), "Hersir itself isn't in a run's tree");
    assert!(!tree.contains(other_pid), "another run's CLI");
    assert!(other_tree.contains(other_pid) && !other_tree.contains(cli_pid) && !other_tree.contains(its_child));
    assert!(!tree.contains(0) && !tree.contains(1));

    tree.end(End::Kill);
    other_tree.end(End::Kill);
    cli.wait().await.unwrap();
    other.wait().await.unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while tree.alive() || other_tree.alive() {
        assert!(Instant::now() < until, "something in the trees still runs");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    #[cfg(unix)]
    assert!(!tree.contains(cli_pid), "a process that is gone");
}

/// Linux and macOS: a run's CLI leads a session of its own, so another process of the same user can't move itself into
/// the run's process group (setpgid) to pass for one of the run's own.
#[cfg(unix)]
#[tokio::test]
async fn a_runs_cli_leads_a_session_of_its_own_so_no_outside_process_can_join_its_group() {
    let (mut cli, tree) = stand_in(false);
    let pid = cli.id().unwrap() as libc::pid_t;
    // SAFETY: getsid and getpgid only read.
    assert_eq!(unsafe { libc::getsid(pid) }, pid, "the CLI leads a session of its own");
    assert_eq!(unsafe { libc::getpgid(pid) }, pid, "and its own process group");
    let join = "import os, sys\ntry:\n    os.setpgid(0, int(sys.argv[1]))\n    print('joined')\nexcept PermissionError:\n    print('refused')\n";
    let out = std::process::Command::new("python3").args(["-c", join, &pid.to_string()]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "refused", "{}", String::from_utf8_lossy(&out.stderr));
    tree.end(End::Kill);
    cli.wait().await.unwrap();
}
