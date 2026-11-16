//! GA-281 QA, round 4: what Hersir's log says when a card's worktree can't be prepared. Users send us that log, and a
//! project's setup command can hold a secret as a plain argument that no redaction rule can tell from other words
//! (`composer config http-basic.<host> <user> <key>`, `docker login -p …`). Decided on 16 Nov (Decisions/GA): Hersir's
//! log lines name "the setup command" instead of its text, as the success line does (`Prepared::summary`), while its
//! own install commands (composer install, npm ci) and why it failed ("failed (exit code 3)") stay, and the card's hold
//! reason keeps the whole text. The lines: the agent's pause (`runs::pause_pull`), the start-failure lines in
//! `start_background`, and the leftovers line in `prepare.rs`.
//!
//! Each case runs in a child process of this test program (this same program, run again with QA_SETUP_LOG_CHILD set), as
//! in `logs_start_test.rs`: `logs::start` points the child's fds 1 and 2 at Hersir's log, as in a release build, then the
//! queue starts a card whose worktree can't be prepared. The parent reads the log file. A program of its own: `start`
//! points the whole process's output at the file. Runs use the fake Claude Code, and the setup command exits before any
//! of its commands with secrets runs, so nothing here touches a real tool's settings.
#![cfg(unix)]
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use gizai_lib::logs;

const CHILD: &str = "QA_SETUP_LOG_CHILD";
const DIR: &str = "QA_SETUP_LOG_DIR";
const FAKE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude.sh");

/// The secrets in `SETUP`, each as a plain argument.
const SECRETS: [&str; 6] = ["nv-Lic3nseK3y", "0123456789abcdef0123456789abcdef01234567",
                            "a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8", "S3cretPw", "Sshp4ssW0rd",
                            "1234567890abcdefghijABCDEFGHIJ123456"];

/// A one-line setup command that exits with 3 before its commands with secrets run: Laravel Nova's documented
/// `composer config http-basic`, composer's GitHub token, npm's token after a space, `docker login -p`, `sshpass -p` and
/// a token stuck to curl's `-u`.
const SETUP: &str = "exit 3; composer config http-basic.nova.laravel.com me@example.com nv-Lic3nseK3y; \
                     composer config --global github-oauth.github.com 0123456789abcdef0123456789abcdef01234567; \
                     npm config set //registry.npmjs.org/:_authToken npm_a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8; \
                     docker login -u me -p S3cretPw ghcr.io; sshpass -p Sshp4ssW0rd ssh deploy@host; \
                     curl -ughp_1234567890abcdefghijABCDEFGHIJ123456: https://api.github.com";

/// What leaves a process running in a group of its own, which the slice's scope ends after the command (the leftovers
/// line): put before `SETUP` when this Linux has a slice for agent runs.
const LEFTOVER: &str = "setsid sleep 20 >/dev/null 2>&1 & ";

fn git(dir: &Path, args: &[&str]) {
    assert!(Command::new("git").args(args).current_dir(dir).status().unwrap().success(), "git {args:?}");
}

/// A main checkout with one commit, holding `files`.
fn git_repo(dir: &Path, files: &[(&str, &str)]) -> PathBuf {
    let repo = dir.join("repo");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    fs::write(repo.join(".gitignore"), "vendor/\nnode_modules/\n").unwrap();
    for (name, text) in files {
        fs::write(repo.join(name), text).unwrap();
    }
    git(&repo, &["add", "."]);
    git(&repo, &["-c", "user.email=t@t", "-c", "user.name=t", "-c", "commit.gpgsign=false", "commit", "-q", "-m", "init"]);
    repo
}

/// Sets project KADE's New worktrees settings: the installs on or off, and the setup command.
fn set_prepare(st: &gizai_lib::AppState, install: bool, setup: &str) {
    let p = gizai_core::projects::list(&st.db).unwrap().into_iter().find(|p| p.key == "KADE").unwrap();
    gizai_core::projects::update(&st.db, &st.you_id, &p.id, gizai_core::model::ProjectInput { name: p.name.clone(), key: p.key.clone(),
        repo_path: p.repo_path.clone(), default_branch: Some(p.default_branch.clone()), worktree_copy: Some(vec![]),
        worktree_install: Some(install), worktree_setup: Some(setup.into()), ..Default::default() }).unwrap();
}

/// The cases the children run. Without QA_SETUP_LOG_CHILD (the test program's own run) it does nothing. A child writes
/// what the parent checks besides the log in files of its own, so its own output (all in the log) holds no secret.
#[test]
fn child_case() {
    let Some(case) = std::env::var_os(CHILD) else { return };
    let dir = PathBuf::from(std::env::var_os(DIR).unwrap());
    logs::start(&dir.join("data"), true).unwrap();
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let st = gizai_lib::test_state(&dir);
        gizai_core::settings::set(&st.db, "claude_bin", &FAKE.to_string()).unwrap();
        let task = match case.to_str().unwrap() {
            // a setup command with secrets that fails, in the slice for agent runs when this Linux has one
            "setup" => {
                let repo = git_repo(&dir, &[]);
                let task = gizai_lib::test_task(&st, repo.to_str().unwrap(), "backend");
                st.run_memory.enable(&gizai_agents::run_memory::slice_name(&st.data_dir), 2 << 30);
                let slice = st.run_memory.slice();
                let setup = if slice.is_some() { format!("{LEFTOVER}{SETUP}") } else { SETUP.to_string() };
                fs::write(dir.join("slice.txt"), if slice.is_some() { "yes" } else { "no" }).unwrap();
                set_prepare(&st, false, &setup);
                gizai_lib::runs::pull(&st).await;
                if let Some(s) = slice {
                    let _ = s.remove();
                }
                task
            }
            // Hersir's own install fails: composer install on a composer.json that isn't JSON
            "install" => {
                let repo = git_repo(&dir, &[("composer.json", "{ not json")]);
                let task = gizai_lib::test_task(&st, repo.to_str().unwrap(), "backend");
                set_prepare(&st, true, "");
                gizai_lib::runs::pull(&st).await;
                task
            }
            other => panic!("no case {other}"),
        };
        let task = gizai_core::tasks::get(&st.db, &task).unwrap();
        fs::write(dir.join("hold.txt"), task.hold_reason.unwrap_or_default()).unwrap();
        let paused = gizai_lib::runs::pull_paused_all(&st);
        fs::write(dir.join("paused.txt"), if paused.is_empty() { "no" } else { "yes" }).unwrap();
    });
    logs::quit();
    std::process::exit(0);
}

/// Runs `case` in a child process on the folder `dir`, and returns its result and Hersir's log.
fn child(case: &str, dir: &Path) -> (Output, String) {
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_case", "--nocapture", "--test-threads=1"])
        .env(CHILD, case)
        .env(DIR, dir)
        .env("RUST_BACKTRACE", "0")
        .output()
        .unwrap();
    let logs: Vec<PathBuf> = fs::read_dir(logs::folder(&dir.join("data"))).map(|d| d.flatten().map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "log")).collect()).unwrap_or_default();
    assert_eq!(logs.len(), 1, "one log for the child's one start: {logs:?}\n{}", String::from_utf8_lossy(&out.stderr));
    (out, fs::read_to_string(&logs[0]).unwrap())
}

fn real_tempdir() -> tempfile::TempDir {
    tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap()
}

/// The setup command's secrets are in no line of Hersir's log. The agent's pause line names "the setup command" and
/// why it failed instead, and so does the line about what it left running (in a slice for agent runs). The card's
/// hold reason keeps the whole command, as decided.
#[test]
fn a_failed_setup_commands_secrets_stay_out_of_hersirs_log_and_its_text_stays_on_the_card() {
    let tmp = real_tempdir();
    let (out, log) = child("setup", tmp.path());
    assert!(out.status.success(), "the child failed ({}); Hersir's log:\n{log}", out.status);
    let read = |name: &str| fs::read_to_string(tmp.path().join(name)).unwrap();
    assert_eq!(read("paused.txt"), "yes", "the agent stops taking cards");
    let hold = read("hold.txt");
    assert!(hold.contains("Preparing its worktree failed") && hold.contains("nv-Lic3nseK3y") && hold.contains("exit code 3"),
            "the card's hold reason keeps the whole setup command and why: {hold}");

    let leaks: Vec<String> = log.lines().filter(|l| SECRETS.iter().any(|s| l.contains(s))).map(String::from).collect();
    assert!(leaks.is_empty(), "Hersir's log holds a secret of the setup command in these lines:\n{}", leaks.join("\n"));
    let paused = log.lines().find(|l| l.contains("stops taking cards until")).unwrap_or_else(|| panic!("no pause line:\n{log}"));
    assert!(paused.contains("the setup command") && paused.contains("failed (exit code 3)"),
            "the pause line names the setup command and why it failed: {paused}");
    if read("slice.txt") == "yes" {
        let ended = log.lines().find(|l| l.starts_with("gizai: ended")).unwrap_or_else(|| panic!("no leftovers line:\n{log}"));
        assert!(ended.contains("the setup command"), "the leftovers line names the setup command: {ended}");
    } else {
        eprintln!("the leftovers line isn't checked: no slice for agent runs on this Linux");
    }
}

/// Hersir's own install command keeps its name in the log, with why it failed.
#[test]
fn a_failed_install_keeps_its_command_in_hersirs_log() {
    let tmp = real_tempdir();
    let (out, log) = child("install", tmp.path());
    assert!(out.status.success(), "the child failed ({}); Hersir's log:\n{log}", out.status);
    assert_eq!(fs::read_to_string(tmp.path().join("paused.txt")).unwrap(), "yes", "the agent stops taking cards");
    let paused = log.lines().find(|l| l.contains("stops taking cards until")).unwrap_or_else(|| panic!("no pause line:\n{log}"));
    assert!(paused.contains("composer install"), "the pause line names Hersir's own install command: {paused}");
    assert!(paused.contains("failed (exit code ") || paused.contains("couldn't start"), "and why it failed: {paused}");
}
