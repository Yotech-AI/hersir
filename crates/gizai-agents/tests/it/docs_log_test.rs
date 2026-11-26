//! The docs page's commit list and diffs: `worktree::log_name_status`, main's newest commits along its first parents with
//! the files each changed (a merge: its pull request's files); `worktree::show_patch`, one commit's changes against its
//! first parent; a rev that reads as an option, or a commit id that isn't one, refused.
use std::path::Path;
use std::process::Command;

use gizai_agents::worktree;

/// git in `dir` as a fixed person, whatever this computer's git config says about line ends and signing.
fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").args(["-c", "user.email=t@t", "-c", "user.name=Tess", "-c", "core.autocrlf=false", "-c", "commit.gpgsign=false"])
        .args(args).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim_end().to_string()
}

/// Writes `text` to `path` in `dir`, commits it as `message` and returns the commit's short id.
fn commit(dir: &Path, path: &str, text: &str, message: &str) -> String {
    std::fs::write(dir.join(path), text).unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    git(dir, &["rev-parse", "--short", "HEAD"])
}

/// A new repository whose branch is main.
fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    dir
}

#[test]
fn the_log_lists_main_newest_first_along_its_first_parents_with_each_commits_files() {
    let dir = repo();
    let r = dir.path();
    commit(r, "a.ts", "one\n", "First");
    git(r, &["switch", "-q", "-c", "feature"]);
    commit(r, "f.ts", "f\n", "On the branch");
    git(r, &["switch", "-q", "main"]);
    let second = commit(r, "a.ts", "two\n", "Second");
    git(r, &["merge", "-q", "--no-ff", "-m", "Merge feature", "feature"]);

    let log = worktree::log_name_status(r, "main", 50).unwrap();
    let records: Vec<&str> = log.split('\x1e').filter(|s| !s.trim().is_empty()).collect();
    let heads: Vec<Vec<&str>> = records.iter().map(|rec| rec.lines().next().unwrap().split('\x1f').collect()).collect();
    assert_eq!(heads.iter().map(|h| h[3]).collect::<Vec<_>>(), ["Merge feature", "Second", "First"], "the branch's own commit is inside its merge");
    assert_eq!((heads[1][0], heads[1][2]), (second.as_str(), "Tess"));
    assert!(heads[1][1].starts_with("20") && heads[1][1].contains('T'), "an ISO date: {}", heads[1][1]);
    assert!(records[0].contains("A\tf.ts"), "a merge lists its pull request's files: {:?}", records[0]);
    assert!(records[1].contains("M\ta.ts") && records[2].contains("A\ta.ts"));
    assert_eq!(worktree::log_name_status(r, "main", 1).unwrap().matches('\x1e').count(), 1, "at most the limit");
}

#[test]
fn a_commits_patch_is_against_its_first_parent() {
    let dir = repo();
    let r = dir.path();
    commit(r, "a.ts", "one\n", "First");
    let second = commit(r, "a.ts", "two\n", "Second");
    let patch = worktree::show_patch(r, &second).unwrap();
    assert!(patch.starts_with("diff --git a/a.ts b/a.ts"), "{patch}");
    assert!(patch.contains("-one\n+two"), "{patch}");
}

#[test]
fn a_rev_that_reads_as_an_option_or_isnt_a_commit_is_refused() {
    let dir = repo();
    commit(dir.path(), "a.ts", "one\n", "First");
    assert!(worktree::log_name_status(dir.path(), "--output=x", 5).is_err());
    assert!(!dir.path().join("x").exists(), "no option reached git");
    assert!(worktree::log_name_status(dir.path(), "nope", 5).is_err());
    for bad in ["--stat", "HEAD", "abc", "zzzzzzz"] {
        assert!(worktree::show_patch(dir.path(), bad).is_err(), "{bad}");
    }
}

/// GA-362 QA: a commit opened on the Docs page starts no program of the repository's git config: not a textconv driver
/// (`diff.<driver>.textconv` through .gitattributes), not an external diff driver (`diff.<driver>.command`), not
/// `diff.external`. Plain `git show` in the same repository does run them, so the test can tell.
#[cfg(unix)]
#[test]
fn opening_a_commit_runs_no_textconv_driver_or_external_diff_of_the_repository() {
    use std::os::unix::fs::PermissionsExt;
    let dir = repo();
    let r = dir.path();
    let outside = tempfile::tempdir().unwrap();
    let ran = outside.path().join("ran");
    let script = outside.path().join("driver.sh");
    std::fs::write(&script, format!("#!/bin/sh\necho \"$0 $*\" >> '{}'\necho CONVERTED-BY-THE-REPOSITORY\n", ran.display())).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let script = script.to_string_lossy().into_owned();
    git(r, &["config", "diff.conv.textconv", &script]);
    git(r, &["config", "diff.ext.command", &script]);
    std::fs::write(r.join(".gitattributes"), "*.ts diff=conv\n*.php diff=ext\n").unwrap();
    std::fs::write(r.join("b.php"), "<?php // one\n").unwrap();
    commit(r, "a.ts", "one\n", "First");
    std::fs::write(r.join("b.php"), "<?php // two\n").unwrap();
    let second = commit(r, "a.ts", "two\n", "Second");

    let patch = worktree::show_patch(r, &second).unwrap();
    assert!(!ran.exists(), "a textconv or external diff driver ran: {}", std::fs::read_to_string(&ran).unwrap_or_default());
    assert!(patch.contains("-one\n+two") && patch.contains("-<?php // one\n+<?php // two"), "the plain lines: {patch}");
    assert!(!patch.contains("CONVERTED-BY-THE-REPOSITORY"), "{patch}");

    // and with diff.external set for every diff
    git(r, &["config", "diff.external", &script]);
    let patch = worktree::show_patch(r, &second).unwrap();
    assert!(!ran.exists(), "diff.external ran: {}", std::fs::read_to_string(&ran).unwrap_or_default());
    assert!(patch.contains("-one\n+two"), "{patch}");

    // plain git show runs them: the drivers are set up so a run would show
    git(r, &["show", &second]);
    assert!(ran.exists(), "the drivers didn't run for plain git show either, so this test can't tell");
}
