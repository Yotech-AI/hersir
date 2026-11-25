//! GA-266: the environment a run's CLI really gets (`process::spawn_exec`). With OPENAI_API_KEY, GITHUB_TOKEN and
//! AWS_SECRET_ACCESS_KEY in Hersir's own environment, a Claude Code run gets none of them unless its CLI's settings set
//! them, and a Codex run still gets the key Codex reads. The CLI is `env`, which prints the environment it got; the run's
//! log keeps each line. A program of its own: it sets the keys in the test process's environment, so its one test runs
//! the runs in turn. Linux and macOS (`env`); the rules themselves are checked on every system in
//! tests/it/run_secrets_test.rs.
#![cfg(unix)]
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use gizai_agents::cli::Kind;
use gizai_agents::process::{AFTER_RESULT, Caps, ClaudeLines, Exec, spawn_exec};
use gizai_agents::stream::RunEvent;

fn set(name: &str, value: &str) {
    // SAFETY: the only test in this binary, and no run is starting meanwhile.
    unsafe { std::env::set_var(name, value) };
}

fn unset(name: &str) {
    // SAFETY: as in `set`.
    unsafe { std::env::remove_var(name) };
}

/// A run of `env` as the CLI `kind` would start, with `lines` as its CLI's lines in Settings → Coding CLIs.
fn run(kind: Option<Kind>, lines: &[(&str, &str)]) -> Exec {
    Exec { bin: "env".into(), env: lines.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), kind, ..Exec::default() }
}

/// The environment the run's CLI got.
async fn env_of(exec: &Exec, dir: &Path, name: &str) -> HashMap<String, String> {
    let log = dir.join(format!("{name}.jsonl"));
    let caps = Caps { max_time: Duration::from_secs(60), max_tool_calls: 80, after_result: AFTER_RESULT };
    let mut h = spawn_exec::<RunEvent, _>(exec, dir, &log, caps, ClaudeLines).unwrap();
    while h.events.recv().await.is_some() {}
    let text = std::fs::read_to_string(&log).unwrap();
    let got: HashMap<String, String> =
        text.lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect();
    assert!(got.contains_key("PATH"), "env printed no environment: {text}");
    got
}

fn get<'a>(env: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    env.get(name).map(String::as_str)
}

#[tokio::test]
async fn a_run_gets_no_other_programs_keys_but_its_own_clis_and_what_its_settings_set() {
    for (k, v) in [("OPENAI_API_KEY", "qa-openai"), ("GITHUB_TOKEN", "qa-github"), ("AWS_SECRET_ACCESS_KEY", "qa-aws"),
                   ("ANTHROPIC_API_KEY", "qa-anthropic"), ("QA_GA266_PLAIN", "plain"), ("qa_ga266_db_password", "pw")] {
        set(k, v);
    }
    unset("CLAUDE_CODE_USE_BEDROCK");
    let tmp = tempfile::tempdir().unwrap();
    let three = ["OPENAI_API_KEY", "GITHUB_TOKEN", "AWS_SECRET_ACCESS_KEY"];

    // Claude Code: none of the three, nor another secret; its own key and the plain variables stay
    let claude = env_of(&run(Some(Kind::ClaudeCode), &[]), tmp.path(), "claude").await;
    for k in three.iter().chain(&["qa_ga266_db_password"]) {
        assert!(!claude.contains_key(*k), "a Claude Code run got {k}");
    }
    assert_eq!(get(&claude, "ANTHROPIC_API_KEY"), Some("qa-anthropic"), "Claude Code's own key");
    assert_eq!(get(&claude, "QA_GA266_PLAIN"), Some("plain"));

    // ...unless its CLI's settings set them, with the value set there
    let lines = [("GITHUB_TOKEN", "from-settings"), ("AWS_SECRET_ACCESS_KEY", "from-settings")];
    let set_there = env_of(&run(Some(Kind::ClaudeCode), &lines), tmp.path(), "claude-settings").await;
    assert_eq!(get(&set_there, "GITHUB_TOKEN"), Some("from-settings"));
    assert_eq!(get(&set_there, "AWS_SECRET_ACCESS_KEY"), Some("from-settings"));
    assert!(!set_there.contains_key("OPENAI_API_KEY"));

    // Codex: the key Codex reads, none of the others
    let codex = env_of(&run(Some(Kind::Codex), &[]), tmp.path(), "codex").await;
    assert_eq!(get(&codex, "OPENAI_API_KEY"), Some("qa-openai"), "the key Codex reads");
    for k in ["GITHUB_TOKEN", "AWS_SECRET_ACCESS_KEY", "ANTHROPIC_API_KEY", "qa_ga266_db_password"] {
        assert!(!codex.contains_key(k), "a Codex run got {k}");
    }

    // Without a CLI kind: no secret at all
    let none = env_of(&run(None, &[]), tmp.path(), "none").await;
    for k in three.iter().chain(&["ANTHROPIC_API_KEY"]) {
        assert!(!none.contains_key(*k), "a run without a CLI kind got {k}");
    }

    // Claude Code on Amazon Bedrock signs in with AWS's own variables
    set("CLAUDE_CODE_USE_BEDROCK", "1");
    let bedrock = env_of(&run(Some(Kind::ClaudeCode), &[]), tmp.path(), "bedrock").await;
    unset("CLAUDE_CODE_USE_BEDROCK");
    assert_eq!(get(&bedrock, "AWS_SECRET_ACCESS_KEY"), Some("qa-aws"));
    assert!(!bedrock.contains_key("GITHUB_TOKEN") && !bedrock.contains_key("OPENAI_API_KEY"));

    // only the run's environment changes, not Hersir's own
    assert_eq!(std::env::var("GITHUB_TOKEN").as_deref(), Ok("qa-github"));
    assert_eq!(std::env::var("OPENAI_API_KEY").as_deref(), Ok("qa-openai"));
}
