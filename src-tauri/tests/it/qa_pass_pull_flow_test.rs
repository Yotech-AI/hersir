//! GA-368 QA: after a QA run that ends with qa_pass, Hersir itself opens the card's pull request on GitHub (a fake gh, and
//! "GitHub" as local bare repositories), or gives the branch's open one QA's new summary, so agents need no GitHub login.
//! Covers the title and description, a second pass on the same branch (`gh pr edit`, no second pull request), opening
//! that fails (the card still goes to Review, with the reason in one sentence, and Open pull request works as before),
//! a project without a link, and only QA's pass opening one. GA-369 QA: a QA run the queue held while this computer was
//! busy still opens the pull request once it starts.
// Linux and macOS only: these tests run shell scripts as fake programs, which Windows can't start.
#![cfg(unix)]
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use gizai_lib::pulls::{self, PullInfo};
use serde_json::json;

const FAKE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude.sh");
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fixtures");
const LINK: &str = "https://github.com/acme/shop";
const DESCRIPTION: &str = "Download all invoices as one CSV file.";
const CARD_TEXT: &str = "Download all invoices as one CSV file.\n\n## Acceptance criteria\n\n- [ ] One row per invoice\n\nFrom Hersir card KADE-1.";

fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").args(args).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap().trim().to_string()
}

fn has_branch(repo: &Path, branch: &str) -> bool {
    Command::new("git").args(["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).current_dir(repo).status().unwrap().success()
}

/// "GitHub" (bare repositories under tmp/github) and the project's local clone of https://github.com/acme/shop. The
/// clone's git config sends https://github.com/acme/ there and allows no transport but local files, so nothing here
/// can reach the real GitHub.
fn github(tmp: &Path) -> (PathBuf, PathBuf) {
    let src = tmp.join("src");
    std::fs::create_dir(&src).unwrap();
    git(&src, &["init", "-q", "-b", "main"]);
    git(&src, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "init"]);
    let bare = tmp.join("github/acme/shop");
    git(tmp, &["clone", "-q", "--bare", src.to_str().unwrap(), bare.to_str().unwrap()]);
    let local = tmp.join("local");
    let rewrite = format!("url.{}/.insteadOf=https://github.com/acme/", tmp.join("github/acme").display());
    git(tmp, &["clone", "-q", "-c", &rewrite, "-c", "protocol.allow=never", "-c", "protocol.file.allow=always",
               "-c", "user.email=t@t", "-c", "user.name=t", LINK, local.to_str().unwrap()]);
    (bare, local)
}

/// Writes an executable script through a child `sh`, so this process never holds it open for writing (a leaked handle
/// would make running it fail with "Text file busy").
fn write_script(path: &Path, script: &str) {
    use std::io::Write;
    let mut sh = Command::new("sh").args(["-c", r#"cat > "$1" && chmod 755 "$1""#, "sh"]).arg(path)
        .stdin(std::process::Stdio::piped()).spawn().unwrap();
    sh.stdin.take().unwrap().write_all(script.as_bytes()).unwrap();
    assert!(sh.wait().unwrap().success(), "can't write {}", path.display());
}

/// A fake gh: each call's arguments go to dir/gh-args (one line each); `pr list` answers with dir/list.json; `pr create`
/// keeps its stdin in dir/gh-stdin and fails with dir/create.err, else answers with dir/create.out; `pr edit` keeps its
/// stdin in dir/edit-stdin and fails with dir/edit.err.
fn fake_gh(dir: &Path) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let gh = dir.join("gh");
    write_script(&gh, &format!(r#"#!/bin/sh
d='{d}'
echo "$*" >> "$d/gh-args"
case "$1 $2" in
  "pr list") [ -f "$d/list.json" ] && cat "$d/list.json"; exit 0 ;;
  "pr create")
    cat > "$d/gh-stdin"
    if [ -f "$d/create.err" ]; then cat "$d/create.err" >&2; exit 1; fi
    cat "$d/create.out"; exit 0 ;;
  "pr edit")
    cat > "$d/edit-stdin"
    if [ -f "$d/edit.err" ]; then cat "$d/edit.err" >&2; exit 1; fi
    echo "https://github.com/acme/shop/pull/$3"; exit 0 ;;
esac
echo "unknown command: $*" >&2; exit 1
"#, d = dir.display()));
    gh
}

/// A fake `claude` for the QA Agent: run-ok's stream, answering qa_pass with the summary in dir/summary (plain words: no
/// quotes, slashes or ampersands).
fn qa_fake(dir: &Path) -> String {
    let path = dir.join("qa-claude.sh");
    write_script(&path, &format!(r#"#!/usr/bin/env bash
fx='{FIXTURES}'
case " $* " in *" --input-format stream-json "*)
  IFS= read -r _r; cat "$fx/models-init.jsonl"; cat > /dev/null; exit 0 ;;
esac
cat > /dev/null
summary="$(cat '{d}/summary')"
sed -e "s/ready_for_testing/qa_pass/g" -e "s/Exporter added; 12 tests pass\./$summary/g" "$fx/run-ok.jsonl"
exit 0
"#, d = dir.display()));
    path.to_string_lossy().into_owned()
}

fn gh_calls(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("gh-args")).unwrap_or_default().lines().map(String::from).collect()
}

/// The light fields Gizai asks gh for (GA-214).
const FIELDS: &str = "number,url,state,isDraft,headRefName,headRefOid,mergeCommit";

struct Card { st: gizai_lib::AppState, task: String, bare: PathBuf, wt: PathBuf, branch: String, gh: PathBuf, qa: String, qa_fake: String, tmp: PathBuf }

impl Card {
    /// The QA Agent's run on the card, ending with qa_pass and `summary`; returns once the run has ended.
    async fn qa_pass(&self, summary: &str) -> gizai_lib::runs::RunSummary {
        std::fs::write(self.tmp.join("summary"), summary).unwrap();
        let s = gizai_lib::runs::run_once(&self.st, &self.task, Some(self.qa.clone()), Some(self.qa_fake.clone())).await.unwrap();
        assert_eq!((s.status.as_str(), s.outcome.as_deref()), ("succeeded", Some("qa_pass")), "{s:?}");
        s
    }
    fn task(&self) -> gizai_core::model::Task { gizai_core::tasks::get(&self.st.db, &self.task).unwrap() }
    fn activity(&self) -> Vec<gizai_core::model::ChangeEntry> { gizai_core::tasks::activity(&self.st.db, &self.task).unwrap() }
    /// What Hersir noted on the card about its pull request (as Hersir, `pulls::note_cleanup`).
    fn notes(&self) -> Vec<String> {
        self.activity().into_iter().filter(|e| e.actor_name.is_none())
            .filter_map(|e| e.diff.get("cleanup").and_then(|c| c.as_str()).map(String::from)).collect()
    }
    fn tip(&self) -> String { git(&self.wt, &["rev-parse", "HEAD"]) }
    fn to(&self, category: &str) {
        let team = gizai_core::team::get(&self.st.db, &gizai_core::team::list(&self.st.db).unwrap()[0].id).unwrap();
        let id = team.states.iter().find(|s| s.category == category).unwrap().id.clone();
        gizai_core::tasks::move_to(&self.st.db, &self.st.you_id, &self.task, &id, "").unwrap();
    }
    /// gh's answer to `pr list`: the branch's open pull request `number`, at the branch's latest commit.
    fn open_on_github(&self, number: u64) {
        let pr = json!([{"number": number, "url": format!("{LINK}/pull/{number}"), "state": "OPEN", "isDraft": false,
                         "headRefName": self.branch, "headRefOid": self.tip()}]);
        std::fs::write(self.gh.join("list.json"), pr.to_string()).unwrap();
    }
}

/// Card KADE-1 "Export invoices as CSV" of a project linked to https://github.com/acme/shop (with `link`), with its
/// description and acceptance criteria, after the Backend Agent's run (its worktree and branch, with one commit): in
/// Testing. A QA Agent that is on no column (nothing starts it by itself); gh is the fake.
async fn tested_card(tmp: &Path, link: bool) -> Card {
    let st = gizai_lib::test_state(tmp);
    let (bare, local) = github(tmp);
    let task = gizai_lib::test_task(&st, local.to_str().unwrap(), "backend");
    let p = gizai_core::projects::list(&st.db).unwrap().into_iter().find(|p| p.key == "KADE").unwrap();
    gizai_core::projects::update(&st.db, &st.you_id, &p.id, gizai_core::model::ProjectInput { name: p.name.clone(), key: p.key.clone(),
        repo_path: p.repo_path.clone(), repo_url: link.then(|| LINK.to_string()), default_branch: Some("main".into()), ..Default::default() }).unwrap();
    gizai_core::tasks::update(&st.db, &st.you_id, &task, gizai_core::model::TaskPatch {
        description_md: Some(DESCRIPTION.into()), acceptance_md: Some("- [ ] One row per invoice".into()), ..Default::default() }).unwrap();
    let gh = tmp.join("gh");
    gizai_core::settings::set(&st.db, "gh_bin", &fake_gh(&gh).to_string_lossy().to_string()).unwrap();
    let qa_fake = qa_fake(tmp);
    // nothing reaches the real claude, whatever starts
    gizai_core::settings::set(&st.db, "claude_bin", &FAKE.to_string()).unwrap();
    gizai_lib::runs::run_once(&st, &task, None, Some(FAKE.into())).await.unwrap();
    let run = gizai_core::runs::list_for_task(&st.db, &task).unwrap().remove(0);
    let (wt, branch) = (PathBuf::from(run.worktree_path.unwrap()), run.branch.unwrap());
    std::fs::write(wt.join("invoices.csv"), "id\n").unwrap();
    git(&wt, &["add", "invoices.csv"]);
    git(&wt, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "invoices"]);
    let team = gizai_core::team::list(&st.db).unwrap()[0].id.clone();
    let qa = gizai_core::team::add_agent(&st.db, &st.you_id, &team, gizai_core::model::AgentInput { name: "QA Agent".into(),
        role_key: "qa".into(), ..Default::default() }).unwrap();
    let c = Card { st, task, bare, wt, branch, gh, qa, qa_fake, tmp: tmp.to_path_buf() };
    assert_eq!(c.task().state_category, "testing", "the Backend Agent's run sent it to Testing");
    assert!(gh_calls(&c.gh).iter().all(|l| !l.starts_with("pr create")), "{:?}", gh_calls(&c.gh));
    c
}

/// Open pull request, as a person presses it: retried while the PR check Hersir started has the card.
async fn open_by_hand(c: &Card) -> PullInfo {
    let t0 = Instant::now();
    loop {
        match pulls::open(&c.st, &c.task).await {
            Ok(pr) => return pr,
            Err(e) if e.contains("busy") && t0.elapsed() < Duration::from_secs(20) => tokio::time::sleep(Duration::from_millis(100)).await,
            Err(e) => panic!("Open pull request: {e}"),
        }
    }
}

#[tokio::test]
async fn qa_pass_opens_the_pull_request_with_qas_summary_and_a_second_pass_updates_it() {
    let tmp = real_tempdir();
    let c = tested_card(tmp.path(), true).await;
    std::fs::write(c.gh.join("create.out"), format!("{LINK}/pull/7\n")).unwrap();
    let before = gh_calls(&c.gh).len();

    // QA's first pass: once its run has ended, the card is in Review with its pull request
    c.qa_pass("QA round 1: all 14 tests pass, none skipped.").await;
    let t = c.task();
    assert_eq!((t.state_name.as_str(), t.pr_url.as_deref(), t.pr_state.as_deref()), ("Review", Some(format!("{LINK}/pull/7").as_str()), Some("open")));
    assert_eq!(git(&c.bare, &["rev-parse", &c.branch]), c.tip(), "the branch is on GitHub with its latest commit");
    assert_eq!(gh_calls(&c.gh)[before..], [
        format!("pr list --repo acme/shop --head {} --state all --limit 20 --json {FIELDS}", c.branch),
        format!("pr create --repo acme/shop --head {} --base main --title KADE-1: Export invoices as CSV --body-file -", c.branch),
    ]);
    let body1 = std::fs::read_to_string(c.gh.join("gh-stdin")).unwrap();
    assert_eq!(body1, format!("QA round 1: all 14 tests pass, none skipped.\n\n{CARD_TEXT}"), "QA's summary first, then the card's text");
    // opened by Hersir, not by you; the card says so in its activity
    let opened: Vec<_> = c.activity().into_iter().filter(|e| e.diff.get("opened").is_some()).collect();
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert_eq!((opened[0].actor_name.as_deref(), &opened[0].diff), (None, &json!({"pullRequest": format!("{LINK}/pull/7"), "prState": "open", "opened": true})));
    assert!(c.notes().is_empty(), "nothing went wrong: {:?}", c.notes());
    // the PR check follows it from now on, as it follows one an agent opened
    c.open_on_github(7);
    let checked = pulls::check_all(&c.st).await;
    assert_eq!(checked.iter().map(|k| k.task_id.as_str()).collect::<Vec<_>>(), [c.task.as_str()], "{checked:?}");
    assert_eq!(checked[0].pull.as_ref().map(|p| (p.url.clone(), p.state.clone())), Some((format!("{LINK}/pull/7"), "open".to_string())));

    // a second QA round on the same branch: the open pull request gets the new summary, no second one is opened
    c.to("testing");
    std::fs::write(c.wt.join("more.csv"), "x\n").unwrap();
    git(&c.wt, &["add", "more.csv"]);
    git(&c.wt, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "more"]);
    c.open_on_github(7);
    let before = gh_calls(&c.gh).len();
    c.qa_pass("QA round 2: all 15 tests pass, the fix works.").await;
    assert_eq!(gh_calls(&c.gh)[before..], [
        format!("pr list --repo acme/shop --head {} --state all --limit 20 --json {FIELDS}", c.branch),
        "pr edit 7 --repo acme/shop --body-file -".to_string(),
    ], "gh pr edit with the body on stdin, no --title: its title stays");
    assert_eq!(std::fs::read_to_string(c.gh.join("edit-stdin")).unwrap(), format!("QA round 2: all 15 tests pass, the fix works.\n\n{CARD_TEXT}"));
    assert_eq!(std::fs::read_to_string(c.gh.join("gh-stdin")).unwrap(), body1, "no second pr create");
    assert_eq!(gh_calls(&c.gh).iter().filter(|l| l.starts_with("pr create")).count(), 1);
    assert_eq!(git(&c.bare, &["rev-parse", &c.branch]), c.tip(), "the new commit is on GitHub too");
    let t = c.task();
    assert_eq!((t.state_name.as_str(), t.pr_url.as_deref(), t.pr_state.as_deref()), ("Review", Some(format!("{LINK}/pull/7").as_str()), Some("open")));
    assert_eq!(c.notes(), ["updated the description of pull request #7 on GitHub with QA's new summary"]);
    assert_eq!(c.activity().iter().filter(|e| e.diff.get("opened").is_some()).count(), 1, "still one pull request opened");
}

#[tokio::test]
async fn qa_pass_without_a_gh_login_still_moves_the_card_to_review_says_why_and_open_pull_request_works_as_before() {
    let tmp = real_tempdir();
    let c = tested_card(tmp.path(), true).await;
    std::fs::write(c.gh.join("create.err"), "To get started with GitHub CLI, please run:  gh auth login\n").unwrap();
    c.qa_pass("QA: all 14 tests pass.").await;
    let t = c.task();
    assert_eq!((t.state_name.as_str(), t.pr_url.as_deref()), ("Review", None), "in Review, without a pull request");
    assert_eq!(c.notes(), ["couldn't open its pull request after QA passed: The GitHub CLI isn't logged in: run gh auth login in a terminal. \
                            Open pull request tries again."]);
    assert!(!c.activity().iter().any(|e| e.diff.get("pullRequest").is_some()), "no pull request recorded");

    // you log gh in and press Open pull request: it opens one as before, as you, with the card's text
    std::fs::remove_file(c.gh.join("create.err")).unwrap();
    std::fs::write(c.gh.join("create.out"), format!("{LINK}/pull/8\n")).unwrap();
    let pr = open_by_hand(&c).await;
    assert_eq!(pr, PullInfo { url: format!("{LINK}/pull/8"), number: Some(8), state: "open".into(), note: None });
    assert_eq!(std::fs::read_to_string(c.gh.join("gh-stdin")).unwrap(), CARD_TEXT, "Open pull request's description is the card's, as before");
    let t = c.task();
    assert_eq!((t.pr_url.as_deref(), t.pr_state.as_deref()), (Some(format!("{LINK}/pull/8").as_str()), Some("open")));
    let last = c.activity().into_iter().filter(|e| e.diff.get("opened").is_some()).collect::<Vec<_>>();
    assert_eq!(last.len(), 1);
    assert!(last[0].actor_name.is_some(), "opened by you");
}

#[tokio::test]
async fn qa_pass_with_gh_gone_notes_why_on_the_card_in_one_sentence() {
    let tmp = real_tempdir();
    let c = tested_card(tmp.path(), true).await;
    gizai_core::settings::set(&c.st.db, "gh_bin", &"/nonexistent/gh".to_string()).unwrap();
    c.qa_pass("QA: all 14 tests pass.").await;
    let t = c.task();
    assert_eq!((t.state_name.as_str(), t.pr_url.as_deref()), ("Review", None));
    let notes = c.notes();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].starts_with("couldn't open its pull request after QA passed: The GitHub CLI isn't at /nonexistent/gh any more"), "{}", notes[0]);
    assert!(notes[0].ends_with(". Open pull request tries again."), "{}", notes[0]);
    assert!(!notes[0].contains('\n'), "one line: {}", notes[0]);
}

#[tokio::test]
async fn a_description_gh_refuses_to_change_keeps_the_pull_request_and_says_why() {
    let tmp = real_tempdir();
    let c = tested_card(tmp.path(), true).await;
    c.open_on_github(4); // an agent opened #4 before
    std::fs::write(c.gh.join("edit.err"), "GraphQL: Resource not accessible by integration (updatePullRequest)\n").unwrap();
    c.qa_pass("QA: all 14 tests pass.").await;
    let t = c.task();
    assert_eq!((t.state_name.as_str(), t.pr_url.as_deref(), t.pr_state.as_deref()), ("Review", Some(format!("{LINK}/pull/4").as_str()), Some("open")),
               "the open one shows on the card");
    assert!(!gh_calls(&c.gh).iter().any(|l| l.starts_with("pr create")), "no second pull request: {:?}", gh_calls(&c.gh));
    assert_eq!(c.notes(), ["couldn't update the description of pull request #4 on GitHub with QA's new summary: \
                            GraphQL: Resource not accessible by integration (updatePullRequest)"]);
    // one an agent opened is noted as seen, not as opened
    let seen: Vec<_> = c.activity().into_iter().filter(|e| e.diff.get("pullRequest").is_some()).collect();
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!((seen[0].actor_name.as_deref(), &seen[0].diff), (None, &json!({"pullRequest": format!("{LINK}/pull/4"), "prState": "open"})));
}

#[tokio::test]
async fn a_long_summary_and_card_text_are_cut_together_and_the_card_it_comes_from_stays() {
    let tmp = real_tempdir();
    let c = tested_card(tmp.path(), true).await;
    let long = "x".repeat(70_000);
    gizai_core::tasks::update(&c.st.db, &c.st.you_id, &c.task, gizai_core::model::TaskPatch { description_md: Some(long), ..Default::default() }).unwrap();
    std::fs::write(c.gh.join("create.out"), format!("{LINK}/pull/7\n")).unwrap();
    c.qa_pass("QA: all 14 tests pass.").await;
    let body = std::fs::read_to_string(c.gh.join("gh-stdin")).unwrap();
    assert!(body.starts_with("QA: all 14 tests pass.\n\nxxx"), "QA's summary first: {}", &body[..40]);
    let kept = 60_000 - "QA: all 14 tests pass.\n\n".len();
    assert_eq!(body, format!("QA: all 14 tests pass.\n\n{}…\n\nFrom Hersir card KADE-1.", "x".repeat(kept)));
    assert!(body.chars().count() < 65_536, "within GitHub's limit");
}

#[tokio::test]
async fn a_project_without_a_github_link_gets_no_pull_request_and_no_note() {
    let tmp = real_tempdir();
    let c = tested_card(tmp.path(), false).await;
    c.qa_pass("QA: all 14 tests pass.").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let t = c.task();
    assert_eq!((t.state_name.as_str(), t.pr_url.as_deref()), ("Review", None));
    assert!(gh_calls(&c.gh).is_empty(), "gh never started: {:?}", gh_calls(&c.gh));
    assert!(c.notes().is_empty(), "{:?}", c.notes());
    assert!(!has_branch(&c.bare, &c.branch), "nothing pushed to GitHub");
}

#[tokio::test]
async fn a_qa_pass_that_moves_the_card_elsewhere_than_review_opens_nothing() {
    let tmp = real_tempdir();
    let c = tested_card(tmp.path(), true).await;
    std::fs::write(c.gh.join("create.out"), format!("{LINK}/pull/7\n")).unwrap();
    // a team whose Testing column leads to Done, not Review: QA's pass moves the card there, and Hersir opens nothing
    let team = gizai_core::team::get(&c.st.db, &gizai_core::team::list(&c.st.db).unwrap()[0].id).unwrap();
    let id = |category: &str| team.states.iter().find(|s| s.category == category).unwrap().id.clone();
    gizai_core::columns::set_column(&c.st.db, &c.st.you_id, &id("testing"),
        gizai_core::columns::ColumnInput { next_state_id: Some(id("done")), ..Default::default() }).unwrap();
    c.qa_pass("QA: all 14 tests pass.").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(c.task().state_category, "done");
    assert!(!gh_calls(&c.gh).iter().any(|l| l.starts_with("pr ")), "gh not asked: {:?}", gh_calls(&c.gh));
    assert_eq!(c.task().pr_url, None);
    assert!(c.notes().is_empty(), "{:?}", c.notes());
}

/// GA-369 beside GA-368 (both changed runs.rs): while this computer is busy the queue holds the card in Testing and its
/// activity says why; once that eases the queue starts the QA Agent's run itself, and its qa_pass still opens the pull
/// request with QA's summary.
#[tokio::test]
async fn a_card_the_queue_holds_while_this_computer_is_busy_gets_its_qa_run_once_that_eases_and_its_pass_opens_the_pull_request() {
    use gizai_agents::run_cpu::Reading;
    const BUSY: &str = "Waiting: this computer is busy (CPU pressure 72% over the last minute). The run starts once that eases.";
    let tmp = real_tempdir();
    let c = tested_card(tmp.path(), true).await;
    std::fs::write(c.gh.join("create.out"), format!("{LINK}/pull/7\n")).unwrap();
    std::fs::write(c.tmp.join("summary"), "QA: all 14 tests pass, started by the queue.").unwrap();
    // the queue's own QA run: the card is the QA Agent's now, and claude is QA's fake
    gizai_core::tasks::update(&c.st.db, &c.st.you_id, &c.task, gizai_core::model::TaskPatch {
        assignee_id: Some(c.qa.clone()), ..Default::default() }).unwrap();
    gizai_core::settings::set(&c.st.db, "claude_bin", &c.qa_fake).unwrap();
    let before = gh_calls(&c.gh).len();

    // busy: no QA run, the card waits in Testing and says so once, and no pull request is opened
    c.st.run_memory.fake_cpu_reading(Some(Reading::Pressure(72.0)));
    assert!(gizai_lib::runs::pull(&c.st).await.is_empty(), "no QA run while this computer is busy");
    assert!(gizai_lib::runs::pull(&c.st).await.is_empty());
    assert_eq!(gizai_core::runs::list_for_task(&c.st.db, &c.task).unwrap().len(), 1, "only the Backend Agent's run");
    assert_eq!(c.task().state_category, "testing");
    assert_eq!(c.notes(), [BUSY]);
    assert_eq!(gh_calls(&c.gh).len(), before, "gh not asked: {:?}", gh_calls(&c.gh));

    // it eases: the next pull starts the QA run, whose pass moves the card to Review and opens its pull request
    c.st.run_memory.fake_cpu_reading(Some(Reading::Pressure(12.0)));
    let started = gizai_lib::runs::pull(&c.st).await;
    assert_eq!(started.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(), [c.task.as_str()], "the queue starts the card that waited");
    let s = started.into_iter().next().unwrap().1.await.unwrap();
    assert_eq!((s.status.as_str(), s.outcome.as_deref()), ("succeeded", Some("qa_pass")), "{s:?}");
    let runs = gizai_core::runs::list_for_task(&c.st.db, &c.task).unwrap();
    assert!(runs.iter().any(|r| r.agent_id == c.qa), "the QA Agent's run");
    let t = c.task();
    assert_eq!((t.state_name.as_str(), t.pr_url.as_deref(), t.pr_state.as_deref()), ("Review", Some(format!("{LINK}/pull/7").as_str()), Some("open")));
    assert_eq!(gh_calls(&c.gh)[before..], [
        format!("pr list --repo acme/shop --head {} --state all --limit 20 --json {FIELDS}", c.branch),
        format!("pr create --repo acme/shop --head {} --base main --title KADE-1: Export invoices as CSV --body-file -", c.branch),
    ]);
    assert_eq!(std::fs::read_to_string(c.gh.join("gh-stdin")).unwrap(), format!("QA: all 14 tests pass, started by the queue.\n\n{CARD_TEXT}"));
    assert_eq!(git(&c.bare, &["rev-parse", &c.branch]), c.tip(), "the branch is on GitHub with its latest commit");
    assert_eq!(c.notes(), [BUSY], "said once, and nothing went wrong with the pull request");
}

/// A temp folder by its real path, the way git and Gizai report it (on macOS /var/folders is /private/var/folders).
fn real_tempdir() -> tempfile::TempDir {
    let base = std::env::temp_dir().canonicalize().unwrap();
    tempfile::tempdir_in(base).unwrap()
}
