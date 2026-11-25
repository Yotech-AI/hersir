//! Docs: a builder's run on a project whose Docs page keeps a changelog is asked for the card's line (the Docs section
//! of its prompt), and the line on its result keeps the card's place in the changelog; QA, the Chief and DevOps aren't
//! asked, nor anyone while Docs is off or its Changelog switched off.
#![cfg(unix)]
use std::path::{Path, PathBuf};

use gizai_core::clis::Cli;
use gizai_core::model::*;
use gizai_core::{projects, runs as core_runs, tasks, team};
use gizai_lib::AppState;

const FAKE_CLAUDE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude.sh");

fn git_repo(dir: &Path) -> PathBuf {
    let repo = dir.join("repo");
    std::fs::create_dir(&repo).unwrap();
    for a in [&["init", "-q", "-b", "main"][..], &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "i"][..]] {
        assert!(std::process::Command::new("git").args(a).current_dir(&repo).status().unwrap().success());
    }
    repo
}

/// Kade with Docs on, every column Manual, and a CLI that prints its prompt; (state, repo, project, cli).
fn setup(tmp: &Path) -> (AppState, PathBuf, String, String) {
    let st = gizai_lib::test_state(tmp);
    let repo = git_repo(tmp);
    let kade = projects::create(&st.db, &st.you_id, ProjectInput { name: "Kade".into(), key: "KADE".into(), dev_docs: Some(true),
        repo_path: Some(repo.to_string_lossy().into()), ..Default::default() }).unwrap();
    let team_id = team::list(&st.db).unwrap()[0].id.clone();
    for s in team::get(&st.db, &team_id).unwrap().states {
        gizai_core::columns::set_column(&st.db, &st.you_id, &s.id, gizai_core::columns::ColumnInput { auto: Some(false), ..Default::default() }).unwrap();
    }
    let mut list: Vec<Cli> = gizai_core::clis::list(&st.db).unwrap();
    list.push(Cli { name: "Claude Code (prints its prompt)".into(), kind: "claude_code".into(), command: FAKE_CLAUDE.into(),
        env: vec!["FAKE_TEMP=1".into()], ..Default::default() });
    let cli = gizai_lib::clis::save(&st, list).unwrap().into_iter().find(|c| c.cli.name == "Claude Code (prints its prompt)").unwrap().cli.id;
    (st, repo, kade, cli)
}

/// A card for a "<Role> Agent" on the fake CLI, which ends with a changelog line; its run's (prompt, run).
async fn run(st: &AppState, repo: &Path, cli: &str, role: &str) -> (String, String, Run) {
    let task = gizai_lib::test_task(st, repo.to_str().unwrap(), role);
    tasks::update(&st.db, &st.you_id, &task, TaskPatch { description_md: Some("Export invoices as CSV. FAKE_CHANGELOG".into()), ..Default::default() }).unwrap();
    let agent = tasks::get(&st.db, &task).unwrap().assignee_id.expect("assigned");
    let m = team::agent(&st.db, &agent).unwrap();
    team::update_agent(&st.db, &st.you_id, &agent, AgentInput { name: m.name, role_key: m.role_key, adapter: cli.into(), ..Default::default() }).unwrap();
    let s = gizai_lib::runs::run_once(st, &task, None, None).await.unwrap();
    assert_eq!((s.status.as_str(), s.outcome.as_deref()), ("succeeded", Some("ready_for_testing")));
    let r = core_runs::get(&st.db, &s.run_id).unwrap();
    let err = std::fs::read_to_string(Path::new(&r.log_path).with_extension("stderr.log")).unwrap();
    let start = err.find("prompt>>").expect("its prompt") + "prompt>>".len();
    let prompt = err[start..start + err[start..].find("<<prompt").unwrap()].to_string();
    (task, prompt, r)
}

fn merge(st: &AppState, task: &str) {
    let done: String = st.db.read(|c| Ok(c.query_row("SELECT id FROM workflow_states WHERE category = 'done' AND deleted_at IS NULL \
        ORDER BY sort_key LIMIT 1", [], |r| r.get(0))?)).unwrap();
    tasks::move_to(&st.db, &st.you_id, task, &done, "a0").unwrap();
}

#[tokio::test]
async fn a_builder_is_asked_for_the_cards_line_and_its_line_is_kept() {
    let tmp = tempfile::tempdir().unwrap();
    let (st, repo, kade, cli) = setup(tmp.path());
    let (task, prompt, _) = run(&st, &repo, &cli, "backend").await;
    let docs = prompt.find("## Docs").unwrap_or_else(|| panic!("the Docs section: {prompt}"));
    assert!(docs < prompt.find("How this run works").unwrap_or(prompt.len()), "after the task, before the run's rules: {prompt}");
    assert!(prompt.contains("\"changelog\": {\"type\": \"Added\""), "{prompt}");

    merge(&st, &task);
    let log = gizai_core::devdocs::changelog(&st.db, &kade).unwrap();
    assert_eq!((log[0].kind.as_deref(), log[0].text.as_deref()), (Some("Added"), Some("Invoices export as CSV from the invoice list.")),
               "the builder's line, its kind as Keep a Changelog names it");
}

#[tokio::test]
async fn qa_isnt_asked_nor_anyone_with_docs_or_its_changelog_off() {
    let tmp = tempfile::tempdir().unwrap();
    let (st, repo, kade, cli) = setup(tmp.path());
    let (_, prompt, _) = run(&st, &repo, &cli, "qa").await;
    assert!(!prompt.contains("## Docs"), "QA writes no changelog line: {prompt}");

    let input = |dev_docs: bool, off: Vec<&str>| ProjectInput { name: "Kade".into(), key: "KADE".into(), dev_docs: Some(dev_docs),
        docs_off: Some(off.into_iter().map(String::from).collect()), repo_path: Some(repo.to_string_lossy().into()), ..Default::default() };
    projects::update(&st.db, &st.you_id, &kade, input(true, vec!["changelog"])).unwrap();
    let (_, prompt, _) = run(&st, &repo, &cli, "backend").await;
    assert!(!prompt.contains("## Docs"), "its Changelog switched off: {prompt}");
    projects::update(&st.db, &st.you_id, &kade, input(false, vec![])).unwrap();
    let (_, prompt, _) = run(&st, &repo, &cli, "backend").await;
    assert!(!prompt.contains("## Docs"), "Docs off: {prompt}");
}
