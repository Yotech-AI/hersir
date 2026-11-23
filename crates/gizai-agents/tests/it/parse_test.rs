use gizai_agents::{claude::ClaudeArgs, outcome, stream::{parse_line, RunEvent}};

fn args() -> ClaudeArgs {
    ClaudeArgs { bin: "/x/claude".into(), prompt: "hi".into(), session_id: "S".into(), permission_mode: "acceptEdits".into(),
                 allowed_tools: vec!["Bash(npm test:*)".into()], ..Default::default() }
}

#[test]
fn parses_a_normal_run() {
    let evs: Vec<RunEvent> = include_str!("../fixtures/run-ok.jsonl").lines().flat_map(parse_line).collect();
    assert!(matches!(&evs[0], RunEvent::Init { session_id, .. } if session_id == "S1"));
    assert!(evs.iter().any(|e| matches!(e, RunEvent::ToolUse { name, summary } if name == "Read" && summary == "app/Exports/InvoiceExporter.php")));
    assert!(evs.iter().any(|e| matches!(e, RunEvent::ToolUse { name, summary } if name == "Bash" && summary.contains("artisan test"))));
    assert!(evs.iter().any(|e| matches!(e, RunEvent::ToolResult { is_error: false, preview } if preview.starts_with("<?php"))));
    let last = evs.last().unwrap();
    match last { RunEvent::Result { cost_usd, input_tokens, num_turns, is_error, text, .. } => {
        assert_eq!((*cost_usd, *input_tokens, *num_turns, *is_error), (Some(0.42), 38000, 7, false));
        let o = outcome::parse(text).unwrap();
        assert_eq!(o.outcome, "ready_for_testing");
    } other => panic!("{other:?}") }
}

#[test]
fn survives_unknown_and_broken_lines() {
    let evs: Vec<RunEvent> = include_str!("../fixtures/run-weird.jsonl").lines().flat_map(parse_line).collect();
    assert_eq!(evs.len(), 3);
    assert!(evs.iter().all(|e| matches!(e, RunEvent::Other { .. })));
    assert!(matches!(&evs[2], RunEvent::Other { raw_type } if raw_type == "thinking"));
}

#[test]
fn outcome_rejects_unknown_values_and_takes_the_last_line() {
    assert!(outcome::parse("GIZAI_RESULT: {\"outcome\":\"merged\",\"summary\":\"x\"}").is_none());
    let t = "GIZAI_RESULT: {\"outcome\":\"qa_fail\",\"summary\":\"a\",\"issues\":[\"1. pin overlaps\"]}\nmore\nGIZAI_RESULT: {\"outcome\":\"qa_pass\",\"summary\":\"b\"}";
    assert_eq!(outcome::parse(t).unwrap().outcome, "qa_pass");
    assert!(outcome::parse("GIZAI_RESULT: not json").is_none());
    assert!(outcome::parse("no result line").is_none());
}

#[test]
fn argv_contains_the_safety_flags() {
    let a = args().argv();
    for f in ["-p", "--output-format", "stream-json", "--verbose", "--session-id", "--strict-mcp-config", "--setting-sources"] {
        assert!(a.contains(&f.to_string()), "missing {f}");
    }
    // GA-318: the rules as one argument, so none can pass for an option
    assert!(a.contains(&"--allowedTools=Bash(npm test:*)".to_string()), "{a:?}");
    assert!(!a.contains(&"--allowedTools".to_string()), "{a:?}");
    assert!(!a.contains(&"--max-budget-usd".to_string()));
    // the prompt goes in on stdin (argv is capped at 128 KiB per argument on Linux)
    assert!(!a.contains(&"hi".to_string()) && !a.contains(&"--".to_string()));
}

#[test]
fn argv_adds_budget_model_and_system_prompt_when_set() {
    let a = ClaudeArgs { max_budget_usd: Some(2.5), model: Some("sonnet".into()), append_system_prompt: Some("Be brief.".into()), allowed_tools: vec![], ..args() }.argv();
    let after = |f: &str| a[a.iter().position(|x| x == f).unwrap() + 1].clone();
    assert_eq!(after("--max-budget-usd"), "2.50");
    assert_eq!(after("--model"), "sonnet");
    assert_eq!(after("--append-system-prompt"), "Be brief.");
    assert!(!a.iter().any(|x| x.starts_with("--allowedTools")), "{a:?}");
}

#[test]
fn argv_for_a_task_run_is_unchanged() {
    let a = args().argv();
    let after = |f: &str| a[a.iter().position(|x| x == f).unwrap() + 1].clone();
    assert_eq!(after("--session-id"), "S");
    assert_eq!(after("--setting-sources"), "user");
    for f in ["--resume", "--restricted", "--include-partial-messages", "--mcp-config", "--tools", "--permission-prompts", "--add-dir", "--no-session-persistence"] {
        assert!(!a.contains(&f.to_string()), "unexpected {f}");
    }
}

#[test]
fn argv_for_a_chat_turn() {
    let a = ClaudeArgs {
        bin: "/x/claude".into(), prompt: "hi".into(), session_id: "S".into(), permission_mode: "manual".into(),
        allowed_tools: vec!["mcp__gizai".into()], resume: true, mcp_config: Some("/d/chat/r.mcp.json".into()), partial_messages: true,
        restricted: true, tools: Some(vec!["Read".into(), "Glob".into(), "Grep".into()]), permission_prompts_none: true,
        add_dirs: vec!["/a".into(), "/b".into()], no_session_persistence: true, ..Default::default()
    }.argv();
    let after = |f: &str| a[a.iter().position(|x| x == f).unwrap_or_else(|| panic!("missing {f}")) + 1].clone();
    assert_eq!(after("--resume"), "S");
    assert!(!a.contains(&"--session-id".to_string()));
    assert!(a.contains(&"--include-partial-messages".to_string()));
    assert!(a.contains(&"--restricted".to_string()));
    assert!(!a.contains(&"--setting-sources".to_string()));
    assert!(a.contains(&"--strict-mcp-config".to_string()));
    assert_eq!(after("--mcp-config"), "/d/chat/r.mcp.json");
    assert_eq!(after("--tools"), "Read,Glob,Grep");
    assert_eq!(after("--permission-prompts"), "none");
    assert_eq!(after("--permission-mode"), "manual");
    let dirs = a.iter().position(|x| x == "--add-dir").unwrap();
    assert_eq!(&a[dirs + 1..dirs + 3], ["/a", "/b"]);
    assert!(a.contains(&"--no-session-persistence".to_string()));
    // variadic options must not swallow each other: the allow-list comes last, as one argument (GA-318)
    assert_eq!(&a[a.len() - 2..], ["/b", "--allowedTools=mcp__gizai"]);
}

/// GA-318 (GA-270 #3, CWE-88): allowed and disallowed tools reach Claude Code only as the values of `--allowedTools=` and
/// `--disallowedTools=`, one argument each, and a rule that doesn't start with a letter (a tool called
/// `--dangerously-skip-permissions`, say) never reaches its argv: no argument of it can turn off the permission checks.
#[test]
fn allowed_and_disallowed_tools_go_as_one_value_each_and_a_rule_that_starts_with_a_dash_never_reaches_argv() {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<String>>();
    let a = ClaudeArgs {
        allowed_tools: s(&["Bash(npm test:*)", "Read", "--dangerously-skip-permissions", " -x ", "mcp__otus__get_card", "_y", "9z",
                           " Bash(git status:*) ", "--permission-mode=bypassPermissions"]),
        disallowed_tools: s(&["Edit(//home/me/My docs/**)", "--dangerously-skip-permissions", "mcp__docs__delete_page", "-"]),
        add_dirs: s(&["/a"]),
        ..args()
    }.argv();
    let of = |flag: &str| a.iter().filter(|x| x.starts_with(flag)).cloned().collect::<Vec<_>>();
    assert_eq!(of("--allowedTools"), ["--allowedTools=Bash(npm test:*),Read,mcp__otus__get_card,Bash(git status:*)"]);
    assert_eq!(of("--disallowedTools"), ["--disallowedTools=Edit(//home/me/My docs/**),mcp__docs__delete_page"]);
    for x in &a {
        assert!(!x.contains("dangerously") && !x.contains("bypassPermissions") && x != "-x" && x != "-" && x != "_y" && x != "9z", "{x}: {a:?}");
    }
    assert_eq!(a[a.iter().position(|x| x == "--permission-mode").unwrap() + 1], "acceptEdits", "the permission mode stays");
    // every argument that starts with "-" is one of the options Hersir passes itself
    let own = ["-p", "--output-format", "--verbose", "--session-id", "--permission-mode", "--setting-sources", "--strict-mcp-config",
               "--add-dir", "--allowedTools", "--disallowedTools"];
    for x in a.iter().filter(|x| x.starts_with('-')) {
        assert!(own.contains(&x.split('=').next().unwrap()), "{x} isn't one of Hersir's own options: {a:?}");
    }
    // only such rules: no flag at all
    let a = ClaudeArgs { allowed_tools: s(&["--dangerously-skip-permissions"]), disallowed_tools: s(&["-x"]), ..args() }.argv();
    assert!(!a.iter().any(|x| x.starts_with("--allowedTools") || x.starts_with("--disallowedTools") || x.contains("dangerously")), "{a:?}");
}

#[test]
fn argv_can_switch_off_hooks_and_skills() {
    let a = ClaudeArgs { disable_hooks: true, disable_skills: true, ..args() }.argv();
    let after = |f: &str| a[a.iter().position(|x| x == f).unwrap_or_else(|| panic!("missing {f}")) + 1].clone();
    assert_eq!(after("--settings"), r#"{"disableAllHooks":true}"#);
    assert!(a.contains(&"--disable-slash-commands".to_string()));
    assert!(!args().argv().contains(&"--settings".to_string()));
}

#[test]
fn exec_can_switch_off_claude_codes_own_memory_after_the_accounts_lines() {
    // GA-85: Claude Code's own variable, set to 1 after the account's own lines (the last one wins), not in the arguments.
    let env = vec![("CLAUDE_CONFIG_DIR".to_string(), "/home/u/.claude-2".to_string()),
                   ("CLAUDE_CODE_DISABLE_AUTO_MEMORY".to_string(), "0".to_string())];
    let on = ClaudeArgs { disable_auto_memory: true, env: env.clone(), ..args() };
    let e = on.exec();
    assert_eq!(e.env.last(), Some(&(gizai_agents::claude::AUTO_MEMORY_OFF.to_string(), "1".to_string())));
    assert_eq!(&e.env[..2], &env[..], "the account's lines stay, before it");
    assert_eq!(e.args, on.argv(), "no argument for it");
    assert!(!e.args.iter().any(|a| a.contains("AUTO_MEMORY") || a.contains("autoMemory")), "{:?}", e.args);
    // off unless asked for
    assert_eq!(ClaudeArgs { env: env.clone(), ..args() }.exec().env, env);
    assert!(args().exec().env.is_empty());
}

#[test]
fn argv_passes_the_effort_level() {
    let a = ClaudeArgs { effort: Some("xhigh".into()), ..args() }.argv();
    assert_eq!(a[a.iter().position(|x| x == "--effort").unwrap() + 1], "xhigh");
    assert!(!args().argv().contains(&"--effort".to_string()));
}

// GA-31, "Run this for me": a needs_decision result line may name the commands the agent asks the user to run.
#[test]
fn a_result_line_may_ask_the_user_to_run_commands_and_older_lines_still_parse() {
    // the fixture the fake Claude Code ends with for FAKE_RUN_FOR_ME: each command exactly as written, quotes and all
    let evs: Vec<RunEvent> = include_str!("../fixtures/run-for-me.jsonl").lines().flat_map(parse_line).collect();
    let RunEvent::Result { text, .. } = evs.last().unwrap() else { panic!("{evs:?}") };
    let o = outcome::parse(text).unwrap();
    assert_eq!((o.outcome.as_str(), o.issues.len()), ("needs_decision", 0));
    assert_eq!(o.run_for_me, ["sudo pacman -S libayatana-appindicator",
        "echo \"fs.inotify.max_user_watches=524288\" | sudo tee /etc/sysctl.d/40-watches.conf && sudo sysctl --system"]);
    // older result lines have none, and parse as before
    for line in ["GIZAI_RESULT: {\"outcome\":\"needs_decision\",\"summary\":\"CSV or JSON?\",\"issues\":[\"Which format?\"]}",
                 "GIZAI_RESULT: {\"outcome\":\"qa_pass\",\"summary\":\"ok\"}", "GIZAI_RESULT: {\"outcome\":\"ready_for_testing\"}"] {
        let o = outcome::parse(line).unwrap_or_else(|| panic!("{line}"));
        assert!(o.run_for_me.is_empty(), "{line}");
    }
    let RunEvent::Result { text, .. } = include_str!("../fixtures/run-asks.jsonl").lines().flat_map(parse_line).last().unwrap() else { panic!() };
    let o = outcome::parse(&text).unwrap();
    assert_eq!((o.outcome.as_str(), o.summary.as_str(), o.run_for_me.len()), ("needs_decision", "CSV or JSON for the export?", 0));
}

#[test]
fn run_for_me_takes_one_command_as_a_string_drops_blank_entries_and_never_spoils_the_line() {
    let p = |rfm: &str| outcome::parse(&format!("GIZAI_RESULT: {{\"outcome\":\"needs_decision\",\"summary\":\"s\",\"run_for_me\":{rfm}}}"))
        .unwrap_or_else(|| panic!("run_for_me {rfm} spoiled the line"));
    assert_eq!(p("\"sudo pacman -S libayatana-appindicator\"").run_for_me, ["sudo pacman -S libayatana-appindicator"]);
    assert_eq!(p("[\"  sudo make install  \", \"\", \"   \", \"npm i -g pnpm\"]").run_for_me, ["sudo make install", "npm i -g pnpm"], "trimmed, blanks out");
    assert_eq!(p("[\"a\", 3, null, {\"cmd\":\"b\"}, \"c\"]").run_for_me, ["a", "c"], "only the strings");
    for odd in ["42", "true", "null", "{\"cmd\":\"x\"}", "[]", "\"\""] {
        assert!(p(odd).run_for_me.is_empty(), "{odd}");
    }
    // the verdict as Gizai saves it again: no run_for_me key without commands
    let o = p("[]");
    assert!(!serde_json::to_string(&o).unwrap().contains("run_for_me"));
    assert!(serde_json::to_string(&p("[\"x\"]")).unwrap().contains("\"run_for_me\":[\"x\"]"));
}

// GA-62: Claude Code's rate_limit_event, shaped like 2.1.x's own schema (rate_limit_info with unifiedWindows), is a Limits
// event with the info as Claude Code wrote it; Gizai keeps it for the run's coding CLI and the Run panel doesn't show it.

#[test]
fn a_rate_limit_event_is_a_limits_event_with_its_info_as_written() {
    let evs: Vec<RunEvent> = include_str!("../fixtures/run-limits.jsonl").lines().flat_map(parse_line).collect();
    let limits: Vec<&serde_json::Value> = evs.iter().filter_map(|e| match e { RunEvent::Limits { info } => Some(info), _ => None }).collect();
    assert_eq!(limits.len(), 1, "{evs:?}");
    assert_eq!(limits[0]["rateLimitType"], "five_hour");
    assert_eq!(limits[0]["unifiedWindows"]["seven_day_overage_included"]["utilization"], 0.05);
    assert!(matches!(evs.last(), Some(RunEvent::Result { is_error: false, .. })), "the rest of the run reads as before");
    // The same lines in a saved run log (the Run panel filters Limits out of a finished run's events).
    let logged = gizai_agents::cli::parse_log(include_str!("../fixtures/run-limits.jsonl"));
    assert_eq!(logged.iter().filter(|e| matches!(e, RunEvent::Limits { .. })).count(), 1);
}

#[test]
fn a_rate_limit_event_without_an_info_object_stays_other() {
    for line in [r#"{"type":"rate_limit_event"}"#, r#"{"type":"rate_limit_event","rate_limit_info":null}"#, r#"{"type":"rate_limit_event","rate_limit_info":"x"}"#] {
        assert!(matches!(&parse_line(line)[..], [RunEvent::Other { raw_type }] if raw_type == "rate_limit_event"), "{line}");
    }
    let evs: Vec<RunEvent> = include_str!("../fixtures/run-weird.jsonl").lines().flat_map(parse_line).collect();
    assert!(!evs.iter().any(|e| matches!(e, RunEvent::Limits { .. })), "the old placeholder ('info', not 'rate_limit_info') isn't a reading");
}
