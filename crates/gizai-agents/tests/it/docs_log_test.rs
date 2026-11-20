//! Docs → Commits: `worktree::log`, main's newest commits along its first parents, newest first, each with its first
//! parent, author, time and subject; a rev that reads as an option, or isn't there, refused; and a first commit's changes
//! measured from git's empty tree (`changes::empty_tree`).
use std::path::Path;
use std::process::Command;

use gizai_agents::changes::{self, FileStatus};
use gizai_agents::worktree;

/// git in `dir` as a fixed person, whatever this computer's git config says about line ends and signing.
fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").args(["-c", "user.email=t@t", "-c", "user.name=Tess", "-c", "core.autocrlf=false", "-c", "commit.gpgsign=false"])
        .args(args).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim_end().to_string()
}

/// Writes `text` to `path` in `dir`, commits it as `message` and returns the commit's id.
fn commit(dir: &Path, path: &str, text: &str, message: &str) -> String {
    std::fs::write(dir.join(path), text).unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    git(dir, &["rev-parse", "HEAD"])
}

/// A new repository whose branch is main.
fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    dir
}

#[test]
fn log_lists_main_newest_first_along_its_first_parents() {
    let dir = repo();
    let r = dir.path();
    let first = commit(r, "a.txt", "one\n", "First");
    commit(r, "a.txt", "two\n", "Second");
    git(r, &["switch", "-q", "-c", "feature"]);
    commit(r, "f.txt", "f\n", "On the branch");
    git(r, &["switch", "-q", "main"]);
    let third = commit(r, "c.txt", "c\n", "Third");
    git(r, &["merge", "-q", "--no-ff", "-m", "Merge feature", "feature"]);
    let merge = git(r, &["rev-parse", "HEAD"]);

    let log = worktree::log(r, "main", 50).unwrap();
    let subjects: Vec<&str> = log.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(subjects, ["Merge feature", "Third", "Second", "First"], "the branch's own commit is inside its merge");
    assert_eq!((log[0].sha.as_str(), log[0].parent.as_deref()), (merge.as_str(), Some(third.as_str())), "a merge's first parent is main before it");
    assert_eq!((log[3].sha.as_str(), log[3].parent.as_deref()), (first.as_str(), None), "the first commit has no parent");
    assert_eq!(log[1].author, "Tess");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    assert!((now - log[1].at).abs() < 3_600_000, "the commit's time, in milliseconds: {}", log[1].at);
    assert_eq!(worktree::log(r, "main", 2).unwrap().len(), 2, "at most the limit");
}

#[test]
fn log_refuses_a_rev_that_reads_as_an_option_or_isnt_there() {
    let dir = repo();
    commit(dir.path(), "a.txt", "one\n", "First");
    assert!(worktree::log(dir.path(), "--output=x", 5).is_err());
    assert!(!dir.path().join("x").exists(), "no option reached git");
    assert!(worktree::log(dir.path(), "nope", 5).is_err());
}

#[test]
fn a_first_commits_changes_are_measured_from_the_empty_tree() {
    let dir = repo();
    let r = dir.path();
    let first = commit(r, "a.txt", "one\ntwo\n", "First");
    let empty = changes::empty_tree(r).unwrap();
    assert!(changes::is_commit_id(&empty), "{empty}");
    let files = changes::between(r, &empty, &first).unwrap();
    assert_eq!(files.iter().map(|f| (f.path.as_str(), f.status, f.additions)).collect::<Vec<_>>(), [("a.txt", FileStatus::Added, Some(2))]);
    let d = changes::file_diff(r, None, "a.txt", None, &empty, Some(&first)).unwrap();
    assert_eq!((d.old_text, d.new_text.as_deref()), (None, Some("one\ntwo\n")));
}
