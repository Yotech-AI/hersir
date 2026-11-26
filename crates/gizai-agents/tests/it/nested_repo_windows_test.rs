//! GA-361, Windows: git tidies a path it is given before it matches it, and on Windows reads `\` in it as `/`. A gitlink
//! whose name in the index had a `\` would be found by Windows as the folder with `/`, and git add's
//! `:(exclude,literal)` of that name would leave out another name than the index's. git for Windows doesn't take such a
//! name into the index (git update-index ignores it, as CI showed); should it ever, Commit… asks git first, with
//! `git ls-files` and the same pathspec, and refuses before any git add. A submodule with a plain name is still left out
//! of the commit (nested_repo_test.rs, Unix only, has GA-350's tests).
#![cfg(windows)]
use std::path::{Path, PathBuf};
use std::process::Command;

use gizai_agents::{worktree, AgentError};

/// The card's sentence, word for word.
const TWO_WAYS: &str = "it has a submodule whose folder name git could read two ways, whose own settings git could run";

/// git in `dir` as a fixed person, without the environment of a git Hersir starts.
fn git_out(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git").args(["-c", "user.email=t@t", "-c", "user.name=t", "-c", "commit.gpgsign=false", "-c", "core.autocrlf=false",
                              "-c", "advice.addEmbeddedRepo=false"])
        .args(args).current_dir(dir).env_remove("GIT_DIR").env_remove("GIT_COMMON_DIR").env_remove("GIT_WORK_TREE")
        .output().unwrap()
}

fn git(dir: &Path, args: &[&str]) -> String {
    let o = git_out(dir, args);
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim_end().to_string()
}

/// A temp folder by its real path, without the \\?\ that canonicalize puts before a Windows drive (see worktree_test).
fn real_tempdir() -> tempfile::TempDir {
    let base = std::env::temp_dir().canonicalize().unwrap();
    let base = PathBuf::from(base.to_string_lossy().trim_start_matches(r"\\?\").to_string());
    tempfile::tempdir_in(base).unwrap()
}

/// The card KADE-1's worktree of a repository with a.txt committed on main, made the way a card's start makes it.
fn card(tmp: &Path) -> PathBuf {
    let repo = tmp.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("a.txt"), "one\n").unwrap();
    git(&repo, &["add", "a.txt"]);
    git(&repo, &["commit", "-q", "-m", "init"]);
    worktree::ensure(&repo, &tmp.join("data/worktrees/KADE"), "KADE-1", "Probe", "main").unwrap().path
}

/// A repository of its own at `nested` with n.txt committed there; its commit's id.
fn nested_repository(nested: &Path) -> String {
    std::fs::create_dir_all(nested).unwrap();
    git(nested, &["init", "-q", "-b", "main"]);
    std::fs::write(nested.join("n.txt"), "one\n").unwrap();
    git(nested, &["add", "n.txt"]);
    git(nested, &["commit", "-q", "-m", "nested"]);
    git(nested, &["rev-parse", "HEAD"])
}

/// `git update-index --add -z --index-info` in `dir` with one entry, "<mode> <id>\t<path>".
fn index_info(dir: &Path, entry: &str) {
    use std::io::Write;
    let mut child = Command::new("git").args(["update-index", "--add", "-z", "--index-info"]).current_dir(dir)
        .env_remove("GIT_DIR").env_remove("GIT_COMMON_DIR").env_remove("GIT_WORK_TREE")
        .stdin(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(format!("{entry}\0").as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    assert!(o.status.success(), "git update-index: {}", String::from_utf8_lossy(&o.stderr));
}

/// The text of a git refusal (`AgentError::Git`), panicking on anything else.
fn git_refusal<T: std::fmt::Debug>(r: Result<T, AgentError>) -> String {
    match r {
        Err(AgentError::Git(m)) => m,
        other => panic!("expected a git refusal: {other:?}"),
    }
}

#[test]
fn a_submodule_whose_name_in_the_index_has_a_backslash_never_gets_past_commit() {
    let tmp = real_tempdir();
    let wt = card(tmp.path());
    let id = nested_repository(&wt.join("a").join("b"));
    index_info(&wt, &format!("160000 {id}\ta\\b"));
    if worktree::gitlinks(&wt).unwrap().is_empty() {
        // git for Windows ignored the name: no gitlink at all is in the index
        let index = git(&wt, &["ls-files", "--stage"]);
        assert!(!index.lines().any(|l| l.starts_with("160000 ")), "{index}");
        return;
    }
    // should git ever take it: staged by the run, and committed
    git(&wt, &["commit", "-q", "-m", "a gitlink with a backslash in its name"]);
    assert_eq!(worktree::gitlinks(&wt).unwrap(), [r"a\b"], "the index holds the name with its backslash");
    assert_eq!(worktree::nested_repositories(&wt).unwrap(), [r"a\b"], "Windows finds its folder a/b");

    // git reads the pathspec's a\b as a/b, so its exclusion leaves this gitlink in
    let left_in = git(&wt, &["ls-files", "--stage", "--", ".", r":(exclude,literal)a\b"]);
    assert!(left_in.lines().any(|l| l.starts_with("160000 ")), "{left_in}");

    // the card's own change: Commit… refuses, and no git add ran
    std::fs::write(wt.join("a.txt"), "two\n").unwrap();
    let (head, tree) = (git(&wt, &["rev-parse", "HEAD"]), git(&wt, &["write-tree"]));
    assert_eq!(git_refusal(worktree::commit_all(&wt, "The card's own work")), format!("Hersir doesn't commit there: {TWO_WAYS}"));
    assert_eq!(git(&wt, &["rev-parse", "HEAD"]), head, "nothing was committed");
    assert_eq!(git(&wt, &["write-tree"]), tree, "nothing was staged");
    assert_eq!(std::fs::read_to_string(wt.join("a.txt")).unwrap(), "two\n", "the card's change stays");
}

/// Submodules with plain names, one in a subfolder and one in NFC with an accent (café), are left out of Commit… as
/// before: the card's own file goes in, and each submodule stays at its commit.
#[test]
fn commit_leaves_out_submodules_with_plain_names_as_before() {
    let tmp = real_tempdir();
    let wt = card(tmp.path());
    nested_repository(&wt.join("vendor").join("lib"));
    nested_repository(&wt.join("caf\u{e9}"));
    git(&wt, &["add", "vendor/lib", "caf\u{e9}"]);
    git(&wt, &["commit", "-q", "-m", "two nested repositories"]);
    assert_eq!(worktree::gitlinks(&wt).unwrap(), ["caf\u{e9}", "vendor/lib"]);
    let links = (git(&wt, &["rev-parse", "HEAD:vendor/lib"]), git(&wt, &["rev-parse", "HEAD:caf\u{e9}"]));

    std::fs::write(wt.join("a.txt"), "two\n").unwrap();
    std::fs::write(wt.join("vendor/lib/n.txt"), "changed inside\n").unwrap();
    let c = worktree::commit_all(&wt, "The card's own work").unwrap();
    assert_eq!(c.subject, "The card's own work");
    assert_eq!(git(&wt, &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]), "a.txt");
    assert_eq!((git(&wt, &["rev-parse", "HEAD:vendor/lib"]), git(&wt, &["rev-parse", "HEAD:caf\u{e9}"])), links,
               "the submodules' commits didn't change");
    assert_eq!(worktree::uncommitted(&wt).unwrap(), 0);
}
