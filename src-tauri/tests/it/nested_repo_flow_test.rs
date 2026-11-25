//! GA-350 (GA-326 #1, CWE-78), end to end with a fake CLI, never a real one: a Codex run commits a nested repository
//! into its worktree whose own config has a clean filter that touches a file, then changes a file in it
//! (crates/gizai-agents/tests/fake-cli-nested-repo.sh). Hersir's git at the run's end, the card's Changes, the Editor,
//! the project form's check, Settings → Data and Remove anyway never run that filter: they leave submodules out
//! (`worktree::IGNORE_SUBMODULES`), and the worktree stays with a sentence (`worktree::nested_repositories`). Plain git
//! status in that worktree does run it, so the probe is a real one.
// Linux and macOS only: these tests run shell scripts as fake programs, which Windows can't start.
#![cfg(unix)]
use std::path::{Path, PathBuf};
use std::process::Command;

use gizai_core::clis::Cli;
use gizai_core::model::AgentInput;

const FAKE_NESTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-cli-nested-repo.sh");
const LINK: &str = "https://github.com/acme/shop";

/// git in `dir` as a fixed person, without the environment of a git Hersir starts.
fn git_out(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git").args(["-c", "user.email=t@t", "-c", "user.name=t", "-c", "commit.gpgsign=false"]).args(args).current_dir(dir)
        .env_remove("GIT_DIR").env_remove("GIT_COMMON_DIR").env_remove("GIT_WORK_TREE").output().unwrap()
}

fn git(dir: &Path, args: &[&str]) -> String {
    let o = git_out(dir, args);
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

fn has_branch(repo: &Path, branch: &str) -> bool {
    git_out(repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).status.success()
}

/// Writes an executable script through a child `sh` (see push_after_run_test: no write handle left in this process).
fn write_script(path: &Path, script: &str) {
    use std::io::Write;
    let mut sh = Command::new("sh").args(["-c", r#"cat > "$1" && chmod 755 "$1""#, "sh"]).arg(path)
        .stdin(std::process::Stdio::piped()).spawn().unwrap();
    sh.stdin.take().unwrap().write_all(script.as_bytes()).unwrap();
    assert!(sh.wait().unwrap().success(), "can't write {}", path.display());
}

/// A program at `tmp/<name>` that runs the fake `script` with bash, with the same arguments and stdin.
fn fake(tmp: &Path, name: &str, script: &str) -> String {
    let path = tmp.join(name);
    write_script(&path, &format!("#!/bin/sh\nexec bash '{script}' \"$@\"\n"));
    path.display().to_string()
}

/// A repository at `dir` with a.txt committed on main.
fn repo_at(dir: &Path) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "init"]);
    dir.to_path_buf()
}

/// "GitHub" (a bare repository under tmp/github, with a.txt on main) and the project's local clone of
/// https://github.com/acme/shop, which allows no transport but local files.
fn github(tmp: &Path) -> (PathBuf, PathBuf) {
    let src = repo_at(&tmp.join("src"));
    let bare = tmp.join("github/acme/shop");
    git(tmp, &["clone", "-q", "--bare", src.to_str().unwrap(), bare.to_str().unwrap()]);
    git(&bare, &["config", "core.hooksPath", bare.join("hooks").to_str().unwrap()]);
    let local = tmp.join("local");
    let rewrite = format!("url.{}/.insteadOf=https://github.com/acme/", tmp.join("github/acme").display());
    git(tmp, &["clone", "-q", "-c", &rewrite, "-c", "protocol.allow=never", "-c", "protocol.file.allow=always",
               "-c", "user.email=t@t", "-c", "user.name=t", LINK, local.to_str().unwrap()]);
    (bare, local)
}

/// Adds a CLI in Settings → Coding CLIs and puts the Backend Agent on it.
fn backend_on_cli(st: &gizai_lib::AppState, name: &str, kind: &str, command: &str, env: &[String]) {
    let mut list: Vec<Cli> = gizai_core::clis::list(&st.db).unwrap();
    list.push(Cli { name: name.into(), kind: kind.into(), command: command.into(), env: env.to_vec(), ..Default::default() });
    let cli = gizai_lib::clis::save(st, list).unwrap().into_iter().find(|c| c.cli.name == name).unwrap().cli.id;
    let (_, agent) = gizai_core::team::all_agents(&st.db).unwrap().into_iter().find(|(_, m)| m.role_key == "backend").unwrap();
    gizai_core::team::update_agent(&st.db, &st.you_id, &agent.actor_id, AgentInput { name: agent.name.clone(), role_key: "backend".into(),
        adapter: cli, ..Default::default() }).unwrap();
}

/// Makes In progress Manual, so the queue doesn't start a card that stayed there again by itself.
fn in_progress_manual(st: &gizai_lib::AppState) {
    let team_id = gizai_core::team::list(&st.db).unwrap()[0].id.clone();
    let state = gizai_core::team::get(&st.db, &team_id).unwrap().states.into_iter().find(|s| s.category == "in_progress").unwrap().id;
    gizai_core::columns::set_column(&st.db, &st.you_id, &state, gizai_core::columns::ColumnInput { auto: Some(false), ..Default::default() }).unwrap();
}

/// Moves the card to Done.
fn finish(st: &gizai_lib::AppState, task: &str) {
    let team_id = gizai_core::team::list(&st.db).unwrap()[0].id.clone();
    let state = gizai_core::team::get(&st.db, &team_id).unwrap().states.into_iter().find(|s| s.category == "done").unwrap().id;
    gizai_core::tasks::move_to(&st.db, &st.you_id, task, &state, "").unwrap();
}

/// A run's notes from Hersir, as Show output reads them back from its log.
fn notes(st: &gizai_lib::AppState, run_id: &str) -> Vec<String> {
    gizai_lib::runs::events_for(st, run_id).into_iter()
        .filter_map(|e| match e.event { gizai_agents::stream::RunEvent::Note { text } => Some(text), _ => None }).collect()
}

#[tokio::test]
async fn a_run_that_commits_a_nested_repository_with_a_clean_filter_never_gets_it_run_by_hersirs_git() {
    let tmp = tempfile::tempdir().unwrap();
    let tmp = tmp.path().canonicalize().unwrap();
    let st = gizai_lib::test_state(&tmp);
    let (bare, local) = github(&tmp);
    let task = gizai_lib::test_task(&st, local.to_str().unwrap(), "backend");
    let p = gizai_core::projects::list(&st.db).unwrap().into_iter().find(|p| p.key == "KADE").unwrap();
    gizai_core::projects::update(&st.db, &st.you_id, &p.id, gizai_core::model::ProjectInput { name: p.name.clone(), key: p.key.clone(),
        repo_path: p.repo_path.clone(), repo_url: Some(LINK.into()), default_branch: Some("main".into()), ..Default::default() }).unwrap();
    let probe = tmp.join("probe-ran");
    let codex = fake(&tmp, "codex", FAKE_NESTED);
    backend_on_cli(&st, "Codex", "codex", &codex, &["FAKE_KIND=codex".into(), format!("FAKE_PROBE={}", probe.display())]);
    in_progress_manual(&st);

    // the run: it ends as usual, Hersir pushes its commit, and its git at the run's end ran no filter
    let s = gizai_lib::runs::run_once(&st, &task, None, None).await.unwrap();
    let run = gizai_core::runs::get(&st.db, &s.run_id).unwrap();
    assert_eq!((run.status.as_str(), s.outcome.as_deref()), ("succeeded", Some("ready_for_testing")), "{run:?}");
    let (wt, branch) = (PathBuf::from(run.worktree_path.clone().unwrap()), run.branch.clone().unwrap());
    assert_eq!(git(&wt, &["ls-files", "--stage", "nested"]).split(' ').next(), Some("160000"), "the fake committed a gitlink");
    assert_eq!(std::fs::read_to_string(wt.join("nested/n.txt")).unwrap(), "two\n", "and changed a file in it");
    assert!(has_branch(&bare, &branch), "Hersir pushed the run's branch");
    let said = notes(&st, &s.run_id);
    assert!(!said.iter().any(|n| n.contains("uncommitted")), "the change inside the nested repository isn't the card's: {said:?}");
    assert!(!probe.exists(), "Hersir's git ran the nested repository's filter at the run's end");

    // the card's Changes: its own a.txt, never the nested repository; a file's diff too
    let changes = gizai_lib::changes::of_card(&st, &task).unwrap();
    let paths: Vec<&str> = changes.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["a.txt"], "{:?}", changes.files);
    let main = git(&local, &["rev-parse", "main"]);
    let diff = gizai_lib::changes::file_of_card(&st, &task, "a.txt", None, &main, None).unwrap();
    assert_eq!(diff.new_text.as_deref(), Some("two\n"));
    assert!(!probe.exists(), "the card's Changes ran the nested repository's filter");

    // the counts of uncommitted files, the Editor's status, the project form's check
    use gizai_agents::{editor, worktree};
    assert_eq!(worktree::uncommitted(&wt).unwrap(), 0);
    assert!(worktree::uncommitted_files(&wt).unwrap().is_empty());
    let status = editor::status(&wt);
    assert_eq!((status.uncommitted, status.branch.as_deref()), (0, Some(branch.as_str())));
    assert!(editor::list(&wt, "").unwrap().iter().any(|e| e.path == "nested" && e.git.is_none()));
    assert!(!gizai_lib::git::repo_check(&wt).dirty);
    assert!(!probe.exists(), "the Editor or the project form's check ran the nested repository's filter");

    // Settings → Data lists it with nothing uncommitted; Remove anyway keeps it with a sentence, and its branch
    finish(&st, &task);
    let listed = gizai_lib::worktrees::list(&st).unwrap();
    let w = listed.iter().find(|w| w.task_id == task).expect("Settings → Data lists the worktree");
    assert_eq!(w.uncommitted, 0);
    let r = gizai_lib::worktrees::remove_anyway(&st, &task).unwrap();
    assert!(!r.removed, "{r:?}");
    assert_eq!(r.note, format!("kept its worktree {} (it has a nested repository in nested, whose own settings git could run)", wt.display()));
    let r = gizai_lib::worktrees::remove(&st, std::slice::from_ref(&task)).unwrap();
    assert!(!r[0].removed && r[0].note.contains("a nested repository in nested"), "{r:?}");
    assert!(wt.join("nested/n.txt").is_file() && has_branch(&local, &branch));
    // after a merge: the same removal, with the branch to go
    let removal = worktree::remove_card_worktree_with(&local, &st.data_dir.join("worktrees"), &st.temp_base, &branch, true);
    assert!(removal.removed.is_none() && !removal.branch_deleted, "{removal:?}");
    assert!(git(&local, &["stash", "list"]).is_empty(), "nothing went into git's stash");
    assert!(!probe.exists(), "removing the worktree ran the nested repository's filter");

    // the probe is real: plain git status in that worktree looks inside the nested repository and runs its filter
    git(&wt, &["status", "--porcelain"]);
    assert!(probe.exists(), "plain git status didn't run the nested repository's filter, so this probe proves nothing");
}

/// GA-357's sentence, word for word.
const NOT_UTF8: &str = "it has a submodule whose folder name isn't valid UTF-8, whose own settings git could run";

/// GA-357 (GA-353 #1, CWE-78): a card done by hand (Open in editor) whose worktree gets a submodule whose folder name
/// isn't valid UTF-8 (`sub` and the byte 0xFF). Commit… in the card bar and Settings → Data's Remove and Remove anyway
/// say the sentence where you read it, and nothing is committed, stashed or removed. On Linux its folder is there, a
/// repository whose clean filter touches a probe that never runs; on macOS (APFS can't hold such a name) the gitlink is
/// in the index only.
#[tokio::test]
async fn a_submodule_whose_folder_name_isnt_utf8_is_refused_with_the_sentence_by_commit_and_settings_data() {
    use std::os::unix::ffi::OsStrExt;
    let tmp = tempfile::tempdir().unwrap();
    let tmp = tmp.path().canonicalize().unwrap();
    let st = gizai_lib::test_state(&tmp);
    gizai_core::settings::set(&st.db, "claude_bin", &concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude.sh").to_string()).unwrap();
    let (_bare, local) = github(&tmp);
    let task = gizai_lib::test_task(&st, local.to_str().unwrap(), "backend");
    let p = gizai_core::projects::list(&st.db).unwrap().into_iter().find(|p| p.key == "KADE").unwrap();
    gizai_core::projects::update(&st.db, &st.you_id, &p.id, gizai_core::model::ProjectInput { name: p.name.clone(), key: p.key.clone(),
        repo_path: p.repo_path.clone(), repo_url: Some(LINK.into()), default_branch: Some("main".into()), ..Default::default() }).unwrap();
    gizai_core::tasks::update(&st.db, &st.you_id, &task, gizai_core::model::TaskPatch { assignee_id: Some(st.you_id.clone()),
        ..Default::default() }).unwrap();
    let hand = gizai_lib::hand::start(&st, &task).await.unwrap();
    let wt = st.data_dir.join("worktrees/KADE/KADE-1");
    assert!(wt.join(".git").exists());
    let branch = hand.branch;
    let probe = tmp.join("probe-ran");
    let name = std::ffi::OsStr::from_bytes(b"sub\xff");

    // the run's commit of the submodule
    #[cfg(target_os = "linux")]
    {
        let nested = wt.join(name);
        std::fs::create_dir(&nested).unwrap();
        git(&nested, &["init", "-q", "-b", "main"]);
        std::fs::write(nested.join("n.txt"), "one\n").unwrap();
        std::fs::write(nested.join(".gitattributes"), "* filter=x\n").unwrap();
        git(&nested, &["add", "n.txt", ".gitattributes"]);
        git(&nested, &["commit", "-q", "-m", "nested"]);
        let o = Command::new("git").args(["-c", "advice.addEmbeddedRepo=false", "add", "--"]).arg(name).current_dir(&wt).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let config = std::fs::read_to_string(nested.join(".git/config")).unwrap();
        std::fs::write(nested.join(".git/config"),
                       format!("{config}[filter \"x\"]\n\tclean = touch {} && cat\n\tsmudge = cat\n", probe.display())).unwrap();
        std::fs::write(nested.join("n.txt"), "two\n").unwrap();
    }
    #[cfg(not(target_os = "linux"))]
    {
        use std::io::Write;
        let id = git(&local, &["rev-parse", "main"]);
        let mut child = Command::new("git").args(["update-index", "--add", "-z", "--index-info"]).current_dir(&wt)
            .stdin(std::process::Stdio::piped()).spawn().unwrap();
        let mut entry = format!("160000 {id}\t").into_bytes();
        entry.extend_from_slice(name.as_bytes());
        entry.push(0);
        child.stdin.take().unwrap().write_all(&entry).unwrap();
        assert!(child.wait().unwrap().success(), "git update-index failed");
    }
    git(&wt, &["commit", "-q", "-m", "a submodule whose folder name isn't UTF-8"]);
    let head = git(&wt, &["rev-parse", "HEAD"]);

    // Commit… in the card bar, with the card's own change
    std::fs::write(wt.join("a.txt"), "the card's change\n").unwrap();
    assert_eq!(gizai_lib::hand::commit(&st, &task, "Mine").await.unwrap_err(), format!("Hersir doesn't commit there: {NOT_UTF8}"));
    assert_eq!(git(&wt, &["rev-parse", "HEAD"]), head, "nothing was committed");
    assert!(!probe.exists(), "Commit… ran the submodule's filter");

    // Settings → Data: Remove anyway's stash refuses, and the worktree and its change stay
    finish(&st, &task);
    let listed = gizai_lib::worktrees::list(&st).unwrap();
    let w = listed.iter().find(|w| w.task_id == task).expect("Settings → Data lists the worktree");
    assert!(w.uncommitted > 0, "{w:?}");
    let r = gizai_lib::worktrees::remove_anyway(&st, &task).unwrap();
    assert!(!r.removed, "{r:?}");
    assert!(r.note.starts_with("kept its worktree: git couldn't put its uncommitted "), "{}", r.note);
    assert!(r.note.ends_with(&format!(" in the stash, so nothing was removed (Hersir doesn't stash there: {NOT_UTF8})")), "{}", r.note);
    assert!(git(&local, &["stash", "list"]).is_empty(), "nothing went into git's stash");
    assert_eq!(std::fs::read_to_string(wt.join("a.txt")).unwrap(), "the card's change\n");

    // with nothing uncommitted: Remove and Remove anyway keep it at the removal, with the sentence as is
    git(&wt, &["checkout", "-q", "--", "a.txt"]);
    let kept = format!("kept its worktree {} ({NOT_UTF8})", wt.display());
    let r = gizai_lib::worktrees::remove_anyway(&st, &task).unwrap();
    assert_eq!((r.removed, r.note.as_str()), (false, kept.as_str()));
    let r = gizai_lib::worktrees::remove(&st, std::slice::from_ref(&task)).unwrap();
    assert_eq!((r[0].removed, r[0].note.as_str()), (false, kept.as_str()));
    // after a merge: the same removal, with the branch to go
    let removal = gizai_agents::worktree::remove_card_worktree_with(&local, &st.data_dir.join("worktrees"), &st.temp_base, &branch, true);
    assert_eq!(removal.phrases(&branch), [kept.clone()]);
    assert!(wt.is_dir() && has_branch(&local, &branch));
    assert!(!probe.exists(), "Settings → Data ran the submodule's filter");

    // on Linux the probe is real: plain git status in that worktree looks inside the submodule and runs its filter
    #[cfg(target_os = "linux")]
    {
        git(&wt, &["status", "--porcelain"]);
        assert!(probe.exists(), "plain git status didn't run the submodule's filter, so this probe proves nothing");
    }
    gizai_lib::runs::stop_all(&st, std::time::Duration::from_secs(10)).await;
}
