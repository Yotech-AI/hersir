//! GA-350 (GA-326 #1, CWE-78): a run commits a nested repository into its worktree (a folder with a `.git` of its own,
//! a gitlink to git) whose own config has a clean filter that touches a file, then changes a file in it. Git's look
//! inside a submodule is a git status there with that repository's settings, so plain git status in the worktree runs
//! the filter, as you (each test checks that last, so its probe is a real one). Hersir's own git never does: its counts
//! of uncommitted files, a card's Changes, the Editor's status and Update's check leave submodules out
//! (`worktree::IGNORE_SUBMODULES`), and Commit… (GA-367), the stash and the worktree's removal leave such a worktree
//! alone (`worktree::nested_repositories`). A project with a real submodule (`.gitmodules`) still shows a card's own
//! changes; those inside the submodule don't.
#![cfg(unix)]
use std::path::{Path, PathBuf};
use std::process::Command;

use gizai_agents::{changes, checkout, editor, worktree};

const BRANCH: &str = "hersir/kade-1-probe";

/// git in `dir` as a fixed person, without the environment of a git Hersir starts.
fn git_out(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git").args(["-c", "user.email=t@t", "-c", "user.name=t", "-c", "commit.gpgsign=false", "-c", "core.autocrlf=false",
                              "-c", "advice.addEmbeddedRepo=false", "-c", "protocol.file.allow=always"])
        .args(args).current_dir(dir).env_remove("GIT_DIR").env_remove("GIT_COMMON_DIR").env_remove("GIT_WORK_TREE")
        .output().unwrap()
}

fn git(dir: &Path, args: &[&str]) -> String {
    let o = git_out(dir, args);
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim_end().to_string()
}

/// A temp folder by its real path (see worktree_test).
fn real_tempdir() -> tempfile::TempDir {
    let base = std::env::temp_dir().canonicalize().unwrap();
    tempfile::tempdir_in(base).unwrap()
}

/// A repository at `dir` with a.txt committed on main.
fn repo_at(dir: &Path) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    git(dir, &["add", "a.txt"]);
    git(dir, &["commit", "-q", "-m", "init"]);
    dir.to_path_buf()
}

/// The project's repository and card KADE-1's worktree, made the way a card's start makes it.
fn card(tmp: &Path) -> (PathBuf, PathBuf) {
    let repo = repo_at(&tmp.join("repo"));
    let wt = worktree::ensure(&repo, &tmp.join("data/worktrees/KADE"), "KADE-1", "Probe", "main").unwrap().path;
    (repo, wt)
}

/// What the run does in `dir`: commits `rel` there, a repository of its own with n.txt (every file through filter x),
/// as a gitlink; then gives that repository's config a clean filter x that touches `probe`, and changes n.txt. A git
/// status inside `rel` now runs the filter.
fn plant_nested(dir: &Path, rel: &str, probe: &Path) {
    let nested = dir.join(rel);
    std::fs::create_dir_all(&nested).unwrap();
    git(&nested, &["init", "-q", "-b", "main"]);
    std::fs::write(nested.join("n.txt"), "one\n").unwrap();
    std::fs::write(nested.join(".gitattributes"), "* filter=x\n").unwrap();
    git(&nested, &["add", "n.txt", ".gitattributes"]);
    git(&nested, &["commit", "-q", "-m", "nested"]);
    git(dir, &["add", rel]);
    git(dir, &["commit", "-q", "-m", &format!("a nested repository in {rel}")]);
    assert_eq!(git(dir, &["ls-files", "--stage", rel]).split(' ').next(), Some("160000"), "{rel} is a gitlink");
    let config = std::fs::read_to_string(nested.join(".git/config")).unwrap();
    std::fs::write(nested.join(".git/config"),
                   format!("{config}[filter \"x\"]\n\tclean = touch {} && cat\n\tsmudge = cat\n", probe.display())).unwrap();
    std::fs::write(nested.join("n.txt"), "two\n").unwrap();
}

/// The control: plain git status in `dir` looks inside the nested repository and runs its filter.
fn control(dir: &Path, probe: &Path) {
    assert!(!probe.exists(), "Hersir's git ran the nested repository's filter");
    git(dir, &["status", "--porcelain"]);
    assert!(probe.exists(), "plain git status didn't run the nested repository's filter, so this probe proves nothing");
}

fn paths(files: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut v: Vec<String> = files.into_iter().collect();
    v.sort();
    v
}

#[test]
fn hersirs_status_changes_and_editor_in_a_card_worktree_never_run_a_nested_repositorys_filter() {
    let tmp = real_tempdir();
    let (repo, wt) = card(tmp.path());
    let probe = tmp.path().join("probe-ran");
    plant_nested(&wt, "nested", &probe);
    // the card's own changes, not committed yet
    std::fs::write(wt.join("a.txt"), "two\n").unwrap();
    std::fs::write(wt.join("b.txt"), "b\n").unwrap();
    let main = git(&repo, &["rev-parse", "main"]);

    // the counts of uncommitted files: the card's own, the nested repository left out
    assert_eq!(worktree::uncommitted(&wt).unwrap(), 2);
    let files = worktree::uncommitted_files(&wt).unwrap();
    assert_eq!(paths(files.into_iter().map(|f| f.path)), ["a.txt", "b.txt"]);
    assert!(!probe.exists(), "uncommitted or uncommitted_files ran the filter");

    // the card's Changes: its own files, uncommitted ones marked; no gitlink (neither its new commit nor what's inside)
    let changed = changes::of_worktree(&wt, &main).unwrap();
    assert_eq!(paths(changed.iter().map(|f| f.path.clone())), ["a.txt", "b.txt"], "{changed:?}");
    assert!(changed.iter().find(|f| f.path == "a.txt").unwrap().uncommitted);
    let diff = changes::file_diff(&repo, Some(wt.as_path()), "a.txt", None, &main, None).unwrap();
    assert_eq!(diff.new_text.as_deref(), Some("two\n"));
    let _ = changes::file_diff(&repo, Some(wt.as_path()), "nested", None, &main, None);
    assert!(!probe.exists(), "the card's Changes ran the filter");

    // the Editor: its status, the folder's top level and the nested folder itself
    let status = editor::status(&wt);
    assert_eq!(status.branch.as_deref(), Some(BRANCH));
    assert_eq!(status.files.keys().cloned().collect::<Vec<_>>(), ["a.txt", "b.txt"]);
    assert_eq!(status.uncommitted, 2);
    let top = editor::list(&wt, "").unwrap();
    let nested = top.iter().find(|e| e.path == "nested").expect("the nested folder shows");
    assert!(nested.dir && nested.git.is_none(), "{nested:?}");
    assert!(top.iter().find(|e| e.path == "a.txt").unwrap().git.is_some());
    let inside = editor::list(&wt, "nested").unwrap();
    assert!(inside.iter().any(|e| e.path == "nested/n.txt" && e.git.is_none()), "{inside:?}");
    assert!(!probe.exists(), "the Editor ran the filter");

    // the helpers that find it, from the index and the folder alone
    assert_eq!(worktree::gitlinks(&wt).unwrap(), ["nested"]);
    assert_eq!(worktree::nested_repositories(&wt).unwrap(), ["nested"]);
    // Remove anyway's stash: git stash looks inside submodules, so Hersir doesn't stash there
    let why = worktree::stash_all(&wt, "KADE-1 leftover").unwrap_err().to_string();
    assert!(why.contains("Hersir doesn't stash there: it has a nested repository in nested"), "{why}");
    assert!(git(&repo, &["stash", "list"]).is_empty(), "nothing was stashed");

    control(&wt, &probe);
    // and the flag is what keeps it out: Hersir's own git there (GIT_DIR pinned, GA-322) without it runs the filter too
    std::fs::remove_file(&probe).unwrap();
    let out = worktree::git_command(&wt).args(["status", "--porcelain"]).output().unwrap();
    assert!(out.status.success());
    assert!(probe.exists(), "git status without --ignore-submodules=all didn't run the filter");
}

#[test]
fn removing_a_card_worktree_with_a_nested_repository_keeps_it_and_runs_no_filter() {
    let tmp = real_tempdir();
    let (repo, wt) = card(tmp.path());
    let probe = tmp.path().join("probe-ran");
    plant_nested(&wt, "nested", &probe);
    let base = tmp.path().join("data/worktrees");
    let temp = tmp.path().join("run-temp");

    // after a merge, or in Settings → Data: kept with a sentence, with its branch
    let r = worktree::remove_card_worktree_with(&repo, &base, &temp, BRANCH, true);
    assert_eq!(r.removed, None, "{r:?}");
    assert_eq!(r.kept.as_ref().map(|(p, _)| p.as_path()), Some(wt.as_path()), "{r:?}");
    assert!(!r.branch_deleted, "a kept worktree keeps its branch: {r:?}");
    assert_eq!(r.phrases(BRANCH), [format!("kept its worktree {} (it has a nested repository in nested, whose own settings git could run)",
                                           wt.display())]);
    assert_eq!(worktree::remove_card_worktree(&repo, &base, BRANCH, false).kept.map(|(p, _)| p), Some(wt.clone()));
    assert!(wt.join("nested/n.txt").is_file());
    // nor does a new card take the worktree over
    let err = worktree::reuse_on(&repo, &wt, &base.join("KADE"), "KADE-2", "hersir/kade-2-next", "main", &[]).unwrap_err().to_string();
    assert!(err.contains("has a nested repository in nested, whose own settings git could run"), "{err}");
    assert!(wt.is_dir());

    // two of them, one deeper down: the sentence names the first
    plant_nested(&wt, "vendor/lib", &probe);
    std::fs::remove_file(&probe).ok();
    let r = worktree::remove_card_worktree_with(&repo, &base, &temp, BRANCH, true);
    assert_eq!(r.phrases(BRANCH), [format!("kept its worktree {} (it has nested repositories in nested and 1 more, whose own settings git could run)",
                                           wt.display())]);
    assert_eq!(worktree::nested_repositories(&wt).unwrap(), ["nested", "vendor/lib"]);
    control(&wt, &probe);
    std::fs::remove_file(&probe).unwrap();

    // a gitlink whose folder holds no repository (a submodule never checked out) is no problem: it goes as before
    std::fs::remove_dir_all(wt.join("nested")).unwrap();
    std::fs::create_dir(wt.join("nested")).unwrap();
    std::fs::remove_dir_all(wt.join("vendor/lib")).unwrap();
    std::fs::create_dir(wt.join("vendor/lib")).unwrap();
    assert_eq!(worktree::gitlinks(&wt).unwrap(), ["nested", "vendor/lib"]);
    assert!(worktree::nested_repositories(&wt).unwrap().is_empty());
    let r = worktree::remove_card_worktree_with(&repo, &base, &temp, BRANCH, false);
    assert_eq!((r.removed.as_deref(), r.kept.as_ref()), (Some(wt.as_path()), None), "{r:?}");
    assert!(!wt.exists());
    assert!(!probe.exists());
}

#[test]
fn a_nested_repositorys_dot_git_of_any_kind_counts() {
    let tmp = real_tempdir();
    let (_repo, wt) = card(tmp.path());
    let probe = tmp.path().join("probe-ran");
    plant_nested(&wt, "nested", &probe);
    // its git folder moved elsewhere, and a .git file pointing at it
    let elsewhere = tmp.path().join("elsewhere.git");
    std::fs::rename(wt.join("nested/.git"), &elsewhere).unwrap();
    std::fs::write(wt.join("nested/.git"), format!("gitdir: {}\n", elsewhere.display())).unwrap();
    assert_eq!(worktree::nested_repositories(&wt).unwrap(), ["nested"]);
    assert_eq!(worktree::uncommitted(&wt).unwrap(), 0);
    assert_eq!(editor::status(&wt).uncommitted, 0);
    control(&wt, &probe);
    // a link to a git folder
    std::fs::remove_file(wt.join("nested/.git")).unwrap();
    std::os::unix::fs::symlink(&elsewhere, wt.join("nested/.git")).unwrap();
    assert_eq!(worktree::nested_repositories(&wt).unwrap(), ["nested"]);
    // a broken link too: Hersir doesn't follow it to decide
    std::fs::remove_file(wt.join("nested/.git")).unwrap();
    std::os::unix::fs::symlink(tmp.path().join("gone"), wt.join("nested/.git")).unwrap();
    assert_eq!(worktree::nested_repositories(&wt).unwrap(), ["nested"]);
}

/// GA-367: Commit… refuses a worktree with a nested repository, with `nested_said`'s words, also when no file of the
/// card's has changed; it stages and commits nothing and runs no filter. Without one (the gitlink's folder holds no
/// repository, as a submodule never checked out) it commits the card's own files as before.
#[test]
fn commit_all_refuses_a_worktree_with_a_nested_repository_runs_no_filter_and_commits_as_before_without_one() {
    let tmp = real_tempdir();
    let (_repo, wt) = card(tmp.path());
    let probe = tmp.path().join("probe-ran");
    plant_nested(&wt, "nested", &probe);
    let (link, head) = (git(&wt, &["rev-parse", "HEAD:nested"]), git(&wt, &["rev-parse", "HEAD"]));
    let refusal = no_commit_beside("a nested repository in nested");

    // only the nested repository changed: refused all the same (it fails closed), not "nothing to commit"
    assert_eq!(git_refusal(worktree::commit_all(&wt, "nothing")), refusal);
    assert!(!probe.exists(), "Commit… ran the filter");

    // the card's own files: refused, nothing staged or committed, and the changes stay
    std::fs::write(wt.join("a.txt"), "two\n").unwrap();
    std::fs::create_dir(wt.join("src")).unwrap();
    std::fs::write(wt.join("src/b.txt"), "b\n").unwrap();
    let tree = index_tree(&wt);
    assert_eq!(git_refusal(worktree::commit_all(&wt, "The card's own work")), refusal);
    let shown = worktree::commit_all(&wt, "The card's own work").unwrap_err().to_string();
    assert!(shown.contains(&refusal), "{shown}");
    assert_eq!(git(&wt, &["rev-parse", "HEAD"]), head, "nothing was committed");
    assert_eq!(index_tree(&wt), tree, "nothing was staged");
    assert_eq!(worktree::uncommitted(&wt).unwrap(), 2, "a.txt and src/b.txt are still uncommitted");
    assert!(!probe.exists(), "Commit… ran the filter");
    control(&wt, &probe);
    // and the change inside it stays there
    assert_eq!(std::fs::read_to_string(wt.join("nested/n.txt")).unwrap(), "two\n");

    // without one: the card's own files go in, the gitlink stays at its commit
    std::fs::remove_dir_all(wt.join("nested")).unwrap();
    std::fs::create_dir(wt.join("nested")).unwrap();
    assert_eq!(worktree::gitlinks(&wt).unwrap(), ["nested"]);
    assert!(worktree::nested_repositories(&wt).unwrap().is_empty());
    let c = worktree::commit_all(&wt, "The card's own work").unwrap();
    assert_eq!(c.subject, "The card's own work");
    assert_eq!(git(&wt, &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]), "a.txt\nsrc/b.txt");
    assert_eq!(git(&wt, &["rev-parse", "HEAD:nested"]), link, "the gitlink's commit didn't change");
    assert_eq!(worktree::uncommitted(&wt).unwrap(), 0);
    assert_eq!(git_refusal(worktree::commit_all(&wt, "Again")), "There is nothing to commit: no file has changed");
}

#[test]
fn updates_check_and_the_editor_in_a_projects_own_checkout_run_no_nested_filter() {
    let tmp = real_tempdir();
    let repo = repo_at(&tmp.path().join("repo"));
    let probe = tmp.path().join("probe-ran");
    plant_nested(&repo, "nested", &probe);
    std::fs::write(repo.join("a.txt"), "changed by hand\n").unwrap();

    // Update's check: the tracked file it names, nothing of the nested repository
    match checkout::check(&repo, &repo, "main", "refs/heads/main", false, false) {
        Err(checkout::Refusal::Uncommitted(files)) => assert_eq!(files, ["a.txt"]),
        other => panic!("expected the uncommitted a.txt: {other:?}"),
    }
    std::fs::write(repo.join("a.txt"), "one\n").unwrap();
    assert!(!matches!(checkout::check(&repo, &repo, "main", "refs/heads/main", false, false), Err(checkout::Refusal::Uncommitted(_))),
            "a change inside the nested repository isn't one of the checkout's");
    assert_eq!(editor::status(&repo).uncommitted, 0);
    assert_eq!(worktree::uncommitted(&repo).unwrap(), 0);
    control(&repo, &probe);
}

/// GA-367: Commit… refuses the worktree while the submodule is checked out, and commits the card's own work once it
/// isn't (`git submodule deinit`), leaving the submodule as it is.
#[test]
fn a_project_with_a_real_submodule_shows_the_cards_own_changes_and_commit_refuses_while_it_is_checked_out() {
    let tmp = real_tempdir();
    let lib = repo_at(&tmp.path().join("lib"));
    let repo = repo_at(&tmp.path().join("repo"));
    git(&repo, &["submodule", "add", "-q", lib.to_str().unwrap(), "lib"]);
    git(&repo, &["commit", "-q", "-m", "lib as a submodule"]);
    let main = git(&repo, &["rev-parse", "main"]);
    let wt = worktree::ensure(&repo, &tmp.path().join("data/worktrees/KADE"), "KADE-1", "Probe", "main").unwrap().path;

    // a card's worktree starts without the submodule checked out: nothing to leave alone yet
    assert_eq!(worktree::gitlinks(&wt).unwrap(), ["lib"]);
    assert!(worktree::nested_repositories(&wt).unwrap().is_empty());
    assert_eq!(worktree::uncommitted(&wt).unwrap(), 0);

    // checked out, with a change inside it; and the card's own changes: a.txt, a new file, and .gitmodules saying
    // `ignore = none`, which a run can write and which beats diff.ignoreSubmodules in the config
    git(&wt, &["submodule", "update", "--init", "-q"]);
    assert_eq!(worktree::nested_repositories(&wt).unwrap(), ["lib"]);
    std::fs::write(wt.join("lib/a.txt"), "changed inside the submodule\n").unwrap();
    std::fs::write(wt.join("lib/new-in-lib.txt"), "new\n").unwrap();
    std::fs::write(wt.join("a.txt"), "the card's change\n").unwrap();
    std::fs::write(wt.join("c.txt"), "new\n").unwrap();
    let gitmodules = std::fs::read_to_string(wt.join(".gitmodules")).unwrap();
    std::fs::write(wt.join(".gitmodules"), format!("{gitmodules}\tignore = none\n")).unwrap();
    let plain = git(&wt, &["status", "--porcelain"]);
    assert!(plain.lines().any(|l| l == " M lib"), "plain git sees the change inside the submodule: {plain}");

    let own = [".gitmodules", "a.txt", "c.txt"];
    assert_eq!(worktree::uncommitted(&wt).unwrap(), 3);
    assert_eq!(paths(worktree::uncommitted_files(&wt).unwrap().into_iter().map(|f| f.path)), own);
    let changed = changes::of_worktree(&wt, &main).unwrap();
    assert_eq!(paths(changed.iter().map(|f| f.path.clone())), own, "{changed:?}");
    assert!(changed.iter().all(|f| f.uncommitted), "{changed:?}");
    let status = editor::status(&wt);
    assert_eq!(status.files.keys().cloned().collect::<Vec<_>>(), own);
    let top = editor::list(&wt, "").unwrap();
    assert!(top.iter().find(|e| e.path == "lib").is_some_and(|e| e.dir && e.git.is_none()), "{top:?}");

    // Commit… refuses while the submodule is checked out: nothing staged or committed, and the changes stay
    let (head, tree) = (git(&wt, &["rev-parse", "HEAD"]), index_tree(&wt));
    assert_eq!(git_refusal(worktree::commit_all(&wt, "The card's own work")), no_commit_beside("a nested repository in lib"));
    assert_eq!(git(&wt, &["rev-parse", "HEAD"]), head, "nothing was committed");
    assert_eq!(index_tree(&wt), tree, "nothing was staged");
    assert_eq!(paths(worktree::uncommitted_files(&wt).unwrap().into_iter().map(|f| f.path)), own);
    assert_eq!(git(&wt.join("lib"), &["status", "--porcelain"]), " M a.txt\n?? new-in-lib.txt", "the change stays inside it");

    // its removal: a worktree with a checked-out submodule stays, as git itself would keep it
    let r = worktree::remove_card_worktree_with(&repo, &tmp.path().join("data/worktrees"), &tmp.path().join("run-temp"), BRANCH, false);
    assert!(r.kept.as_ref().is_some_and(|(_, why)| why.contains("a nested repository in lib")), "{r:?}");

    // no longer checked out (its folder empty again): a commit of the card's own work leaves the submodule as it is
    git(&wt, &["submodule", "deinit", "-q", "-f", "lib"]);
    assert!(worktree::nested_repositories(&wt).unwrap().is_empty());
    worktree::commit_all(&wt, "The card's own work").unwrap();
    assert_eq!(git(&wt, &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]), own.join("\n"));
    assert_eq!(git(&wt, &["rev-parse", "HEAD:lib"]), git(&lib, &["rev-parse", "main"]));
    // the branch's Changes after the commit: still only the card's own files
    let changed = changes::of_worktree(&wt, &main).unwrap();
    assert_eq!(paths(changed.iter().map(|f| f.path.clone())), own);
    assert!(changed.iter().all(|f| !f.uncommitted), "{changed:?}");
    let head = git(&wt, &["rev-parse", "HEAD"]);
    let between = changes::between(&repo, &main, &head).unwrap();
    assert_eq!(paths(between.iter().map(|f| f.path.clone())), own);
}

// GA-357 (GA-353 #1, CWE-78): a submodule whose folder name isn't valid UTF-8. Read as text, its name changed, so
// GA-350's guard didn't find its `.git` and git add's pathspec didn't leave it out: Commit… and Remove anyway's stash ran
// its clean filter. Now Hersir refuses such a checkout, word for word, before any git looks inside it.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;

use gizai_agents::AgentError;

/// The card's sentence, word for word.
const NOT_UTF8: &str = "it has a submodule whose folder name isn't valid UTF-8, whose own settings git could run";

/// `sub` and the byte 0xFF: a folder name Linux can hold, which isn't valid UTF-8.
fn not_utf8_name() -> &'static OsStr {
    OsStr::from_bytes(b"sub\xff")
}

/// git in `dir` with arguments that needn't be UTF-8 (Linux: on macOS git would read them as UTF-8).
#[cfg(target_os = "linux")]
fn git_os(dir: &Path, args: &[&OsStr]) -> Vec<u8> {
    let o = Command::new("git").args(["-c", "user.email=t@t", "-c", "user.name=t", "-c", "commit.gpgsign=false",
                                      "-c", "advice.addEmbeddedRepo=false"])
        .args(args).current_dir(dir).env_remove("GIT_DIR").env_remove("GIT_COMMON_DIR").env_remove("GIT_WORK_TREE")
        .output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    o.stdout
}

/// `git update-index --add -z --index-info` in `dir` with `entries` (each "<mode> <id> [<stage> ]\t<path>", as bytes):
/// index entries without anything on disk, so a name APFS can't hold works on macOS too.
fn index_info(dir: &Path, entries: &[Vec<u8>]) {
    use std::io::Write;
    let mut child = Command::new("git").args(["update-index", "--add", "-z", "--index-info"]).current_dir(dir)
        .env_remove("GIT_DIR").env_remove("GIT_COMMON_DIR").env_remove("GIT_WORK_TREE")
        .stdin(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap();
    let mut input = vec![];
    for e in entries {
        input.extend_from_slice(e);
        input.push(0);
    }
    child.stdin.take().unwrap().write_all(&input).unwrap();
    let o = child.wait_with_output().unwrap();
    assert!(o.status.success(), "git update-index: {}", String::from_utf8_lossy(&o.stderr));
}

/// An index entry "<mode> <id>\t<path>" for `index_info`.
fn entry(mode: &str, id: &str, path: &[u8]) -> Vec<u8> {
    let mut e = format!("{mode} {id}\t").into_bytes();
    e.extend_from_slice(path);
    e
}

/// The text of a git refusal (`AgentError::Git`), panicking on anything else.
fn git_refusal<T: std::fmt::Debug>(r: Result<T, AgentError>) -> String {
    match r {
        Err(AgentError::Git(m)) => m,
        other => panic!("expected a git refusal: {other:?}"),
    }
}

/// The tree in `dir`'s index, read from the index alone (git write-tree never looks inside a submodule).
fn index_tree(dir: &Path) -> String {
    git(dir, &["write-tree"])
}

/// Commit…, the stash of Remove anyway, the worktree's removal and its take-over by a new card each refuse the card
/// KADE-1's worktree `wt` with the sentence, and change nothing: no commit, nothing staged, nothing stashed, the
/// worktree and its branch stay where they are, and no KADE-2 worktree or branch.
fn all_four_refuse(tmp: &Path, repo: &Path, wt: &Path) {
    let (head, tree) = (git(wt, &["rev-parse", "HEAD"]), index_tree(wt));
    let base = tmp.join("data/worktrees");

    // Commit… (`commit_all`, via `add_all`)
    assert_eq!(git_refusal(worktree::commit_all(wt, "The card's own work")), format!("Hersir doesn't commit there: {NOT_UTF8}"));
    // as the user sees it in the card (`AgentError`'s text)
    let shown = worktree::commit_all(wt, "The card's own work").unwrap_err().to_string();
    assert!(shown.contains(&format!("Hersir doesn't commit there: {NOT_UTF8}")), "{shown}");
    assert_eq!(git(wt, &["rev-parse", "HEAD"]), head, "nothing was committed");
    assert_eq!(index_tree(wt), tree, "nothing was staged");

    // Remove anyway's stash (`stash_all`)
    assert_eq!(git_refusal(worktree::stash_all(wt, "KADE-1 leftover")), format!("Hersir doesn't stash there: {NOT_UTF8}"));
    assert!(git(repo, &["stash", "list"]).is_empty(), "nothing was stashed");
    assert_eq!(index_tree(wt), tree);

    // the worktree's removal (`nested_refusal`): kept with the sentence, as is, and its branch
    let r = worktree::remove_card_worktree_with(repo, &base, &tmp.join("run-temp"), BRANCH, true);
    assert_eq!(r.removed, None, "{r:?}");
    assert_eq!(r.kept, Some((wt.to_path_buf(), NOT_UTF8.to_string())), "{r:?}");
    assert!(!r.branch_deleted, "{r:?}");
    assert_eq!(r.phrases(BRANCH), [format!("kept its worktree {} ({NOT_UTF8})", wt.display())]);
    assert_eq!(worktree::remove_card_worktree(repo, &base, BRANCH, false).kept, Some((wt.to_path_buf(), NOT_UTF8.to_string())));
    assert!(wt.is_dir());
    assert_eq!(git(repo, &["rev-parse", &format!("refs/heads/{BRANCH}")]), head, "the branch stays");

    // the worktree's move, when a new card takes it over (`reuse_on`)
    let err = git_refusal(worktree::reuse_on(repo, wt, &base.join("KADE"), "KADE-2", "hersir/kade-2-next", "main", &[]));
    assert_eq!(err, format!("{}: {NOT_UTF8}", wt.display()));
    let err = git_refusal(worktree::reuse(repo, wt, &base.join("KADE"), "KADE-2", "Next", "main", &[]));
    assert_eq!(err, format!("{}: {NOT_UTF8}", wt.display()));
    assert!(wt.is_dir() && !base.join("KADE/KADE-2").exists());
    assert_eq!(git(wt, &["rev-parse", "--abbrev-ref", "HEAD"]), BRANCH, "the worktree is still on its card's branch");
    assert!(git(repo, &["branch", "--list", "hersir/kade-2*"]).is_empty(), "no branch for the new card");
    assert_eq!(git(wt, &["rev-parse", "HEAD"]), head);
}

#[test]
fn the_not_utf8_sentence_is_the_cards_word_for_word() {
    assert_eq!(worktree::NOT_UTF8_SUBMODULE, NOT_UTF8);
}

/// GA-353's proof, as a test: a nested repository at `sub` + 0xFF, committed as a gitlink, whose clean filter touches a
/// probe. Linux only: macOS's APFS can't hold such a name.
#[cfg(target_os = "linux")]
#[test]
fn a_submodule_whose_folder_name_isnt_utf8_is_refused_by_commit_stash_removal_and_move_and_runs_no_filter() {
    let tmp = real_tempdir();
    let (repo, wt) = card(tmp.path());
    let probe = tmp.path().join("probe-ran");
    let name = not_utf8_name();
    let nested = wt.join(name);
    std::fs::create_dir_all(&nested).unwrap();
    git(&nested, &["init", "-q", "-b", "main"]);
    std::fs::write(nested.join("n.txt"), "one\n").unwrap();
    std::fs::write(nested.join(".gitattributes"), "* filter=x\n").unwrap();
    git(&nested, &["add", "n.txt", ".gitattributes"]);
    git(&nested, &["commit", "-q", "-m", "nested"]);
    git_os(&wt, &[OsStr::new("add"), OsStr::new("--"), name]);
    git(&wt, &["commit", "-q", "-m", "a nested repository whose folder name isn't UTF-8"]);
    let mut wanted = b"160000 ".to_vec();
    wanted.extend_from_slice(git(&nested, &["rev-parse", "HEAD"]).as_bytes());
    wanted.extend_from_slice(b" 0\tsub\xff\0");
    assert!(git_os(&wt, &[OsStr::new("ls-files"), OsStr::new("--stage"), OsStr::new("-z")]).ends_with(&wanted),
            "{name:?} is a gitlink, with its name's real bytes in the index");
    let config = std::fs::read_to_string(nested.join(".git/config")).unwrap();
    std::fs::write(nested.join(".git/config"),
                   format!("{config}[filter \"x\"]\n\tclean = touch {} && cat\n\tsmudge = cat\n", probe.display())).unwrap();
    std::fs::write(nested.join("n.txt"), "two\n").unwrap();

    // the helpers: the sentence, whether or not git could find the folder
    assert_eq!(git_refusal(worktree::gitlinks(&wt)), NOT_UTF8);
    assert_eq!(git_refusal(worktree::nested_repositories(&wt)), NOT_UTF8);

    // what runs before Remove anyway's stash and in the card's views leaves the gitlink out, whatever its name
    assert_eq!(worktree::uncommitted(&wt).unwrap(), 0);
    assert!(worktree::uncommitted_files(&wt).unwrap().is_empty());
    assert_eq!(editor::status(&wt).uncommitted, 0);
    let main = git(&repo, &["rev-parse", "main"]);
    let _ = changes::of_worktree(&wt, &main);
    assert!(!probe.exists(), "Hersir's status ran the nested repository's filter");

    // nothing else changed: the four refuse
    all_four_refuse(tmp.path(), &repo, &wt);
    assert!(!probe.exists(), "a refusal ran the nested repository's filter");

    // the card's own changes too: a changed a.txt and a new file aren't committed, stashed or moved either
    std::fs::write(wt.join("a.txt"), "two\n").unwrap();
    std::fs::write(wt.join("b.txt"), "b\n").unwrap();
    assert_eq!(worktree::uncommitted(&wt).unwrap(), 2);
    all_four_refuse(tmp.path(), &repo, &wt);
    assert_eq!(std::fs::read_to_string(wt.join("a.txt")).unwrap(), "two\n", "the card's change stays");
    assert!(wt.join("b.txt").is_file());
    assert_eq!(std::fs::read_to_string(nested.join("n.txt")).unwrap(), "two\n", "and the change inside it");
    assert!(!probe.exists(), "a refusal ran the nested repository's filter");

    // the probe is real: plain git status in that worktree looks inside it and runs its filter
    control(&wt, &probe);
    // and so does what Commit… ran before GA-357 (GA-353's bypass): git add --all, with the name read as text left out
    // of its pathspec, which doesn't match the folder's real bytes
    std::fs::remove_file(&probe).unwrap();
    let lossy = format!(":(exclude,literal){}", String::from_utf8_lossy(b"sub\xff"));
    git(&wt, &["add", "--all", "--", ".", &lossy]);
    assert!(probe.exists(), "git add with GA-350's pathspec didn't run the filter, so the refusals above prove nothing");
}

/// The same gitlink in the index alone, with no folder on disk (a submodule never checked out): refused all the same.
/// On every Unix: the name is only in git's index.
#[test]
fn a_submodule_whose_folder_name_isnt_utf8_is_refused_when_it_is_in_the_index_only() {
    let tmp = real_tempdir();
    let (repo, wt) = card(tmp.path());
    let lib = repo_at(&tmp.path().join("lib"));
    let id = git(&lib, &["rev-parse", "HEAD"]);
    assert!(!wt.join(not_utf8_name()).exists());

    // staged only
    index_info(&wt, &[entry("160000", &id, b"sub\xff")]);
    assert_eq!(git_refusal(worktree::gitlinks(&wt)), NOT_UTF8);
    assert_eq!(git_refusal(worktree::nested_repositories(&wt)), NOT_UTF8);
    all_four_refuse(tmp.path(), &repo, &wt);

    // committed by the run, and among valid gitlinks and files: still refused, wherever it is in the index
    index_info(&wt, &[entry("160000", &id, b"a-lib"), entry("160000", &id, b"zz/lib"),
                      entry("100644", &git(&wt, &["rev-parse", "HEAD:a.txt"]), b"zz/a.txt")]);
    git(&wt, &["commit", "-q", "-m", "submodules the run committed"]);
    assert_eq!(git_refusal(worktree::gitlinks(&wt)), NOT_UTF8);
    all_four_refuse(tmp.path(), &repo, &wt);
    std::fs::write(wt.join("a.txt"), "two\n").unwrap();
    all_four_refuse(tmp.path(), &repo, &wt);

    // and a gitlink with such a name in a merge conflict (several stages of one path)
    git(&wt, &["checkout", "-q", "a.txt"]);
    let gone = b"0 0000000000000000000000000000000000000000\tsub\xff".to_vec();
    index_info(&wt, std::slice::from_ref(&gone));
    git(&wt, &["commit", "-q", "-m", "without it"]);
    assert_eq!(worktree::gitlinks(&wt).unwrap(), ["a-lib", "zz/lib"], "the valid ones, as before");
    index_info(&wt, &[gone, entry("160000", &format!("{id} 2"), b"sub\xff"), entry("160000", &format!("{id} 3"), b"sub\xff")]);
    assert_eq!(git(&wt, &["ls-files", "--stage"]).lines().filter(|l| l.starts_with("160000 ")).count(), 4, "two stages of it");
    assert_eq!(git_refusal(worktree::gitlinks(&wt)), NOT_UTF8);
}

/// Other index entries with such a name, a file's, don't matter: the gitlinks with valid names come back as before.
#[test]
fn a_file_whose_name_isnt_utf8_doesnt_matter() {
    let tmp = real_tempdir();
    let (_repo, wt) = card(tmp.path());
    let lib = repo_at(&tmp.path().join("lib"));
    let (id, blob) = (git(&lib, &["rev-parse", "HEAD"]), git(&wt, &["rev-parse", "HEAD:a.txt"]));
    index_info(&wt, &[entry("100644", &blob, b"caf\xe9.txt"), entry("100755", &blob, b"run\xff.sh"),
                      entry("120000", &blob, b"link\xfe"), entry("160000", &id, b"vendor/lib")]);
    assert_eq!(worktree::gitlinks(&wt).unwrap(), ["vendor/lib"]);
    assert!(worktree::nested_repositories(&wt).unwrap().is_empty(), "vendor/lib holds no repository");
}

/// On disk too (Linux): a file whose name isn't UTF-8 beside a nested repository with a valid name gets GA-367's refusal
/// for that repository, not GA-357's, and the filter never runs; once the gitlink's folder holds no repository, Commit…
/// commits the file under its own name.
#[cfg(target_os = "linux")]
#[test]
fn a_file_whose_name_isnt_utf8_beside_a_nested_repository_gets_its_refusal_and_is_committed_without_one() {
    let tmp = real_tempdir();
    let (_repo, wt) = card(tmp.path());
    let probe = tmp.path().join("probe-ran");
    plant_nested(&wt, "nested", &probe);
    std::fs::write(wt.join(OsStr::from_bytes(b"caf\xe9.txt")), "latin-1\n").unwrap();
    std::fs::write(wt.join("a.txt"), "two\n").unwrap();
    assert_eq!(worktree::gitlinks(&wt).unwrap(), ["nested"]);
    let head = git(&wt, &["rev-parse", "HEAD"]);
    assert_eq!(git_refusal(worktree::commit_all(&wt, "The card's own work")), no_commit_beside("a nested repository in nested"));
    assert_eq!(git(&wt, &["rev-parse", "HEAD"]), head, "nothing was committed");
    assert!(!probe.exists(), "Commit… ran the filter");
    control(&wt, &probe);

    std::fs::remove_dir_all(wt.join("nested")).unwrap();
    std::fs::create_dir(wt.join("nested")).unwrap();
    worktree::commit_all(&wt, "The card's own work").unwrap();
    let names = git_os(&wt, &[OsStr::new("diff-tree"), OsStr::new("--no-commit-id"), OsStr::new("--name-only"), OsStr::new("-z"),
                              OsStr::new("-r"), OsStr::new("HEAD")]);
    assert_eq!(names, b"a.txt\0caf\xe9.txt\0", "{}", String::from_utf8_lossy(&names));
    assert_eq!(worktree::uncommitted(&wt).unwrap(), 0);
}

/// The gitlinks with valid names: in the index's order, each once, also in a merge conflict (one entry per side).
#[test]
fn gitlinks_come_in_the_index_order_once_each() {
    let tmp = real_tempdir();
    let (_repo, wt) = card(tmp.path());
    let lib = repo_at(&tmp.path().join("lib"));
    let id = git(&lib, &["rev-parse", "HEAD"]);
    index_info(&wt, &[entry("160000", &id, "vendor/bibliothèque".as_bytes()), entry("160000", &id, b"b-lib"),
                      entry("160000", &id, b"a-lib")]);
    assert_eq!(worktree::gitlinks(&wt).unwrap(), ["a-lib", "b-lib", "vendor/bibliothèque"]);
    // b-lib in a merge conflict
    index_info(&wt, &[b"0 0000000000000000000000000000000000000000\tb-lib".to_vec()]);
    index_info(&wt, &[entry("160000", &format!("{id} 1"), b"b-lib"), entry("160000", &format!("{id} 2"), b"b-lib"),
                      entry("160000", &format!("{id} 3"), b"b-lib")]);
    let stages = git(&wt, &["ls-files", "--stage", "b-lib"]);
    assert_eq!(stages.lines().count(), 3, "{stages}");
    assert_eq!(worktree::gitlinks(&wt).unwrap(), ["a-lib", "b-lib", "vendor/bibliothèque"]);
}

/// A submodule with a valid non-ASCII name, vendor/bibliothèque, behaves as GA-350's tests expect: Commit… (GA-367), the
/// stash, the removal and the move refuse it as a nested repository, by its name.
#[test]
fn a_submodule_with_a_valid_non_ascii_name_is_refused_by_commit_stash_removal_and_move_by_its_name() {
    let tmp = real_tempdir();
    let (repo, wt) = card(tmp.path());
    let probe = tmp.path().join("probe-ran");
    let rel = "vendor/bibliothèque";
    plant_nested(&wt, rel, &probe);
    let link = git(&wt, &["rev-parse", &format!("HEAD:{rel}")]);
    let base = tmp.path().join("data/worktrees");
    assert_eq!(worktree::gitlinks(&wt).unwrap(), [rel]);
    assert_eq!(worktree::nested_repositories(&wt).unwrap(), [rel]);
    assert_eq!(worktree::uncommitted(&wt).unwrap(), 0);

    // Commit…: refused with nothing changed, and with the card's own file changed; nothing committed
    let head = git(&wt, &["rev-parse", "HEAD"]);
    assert_eq!(git_refusal(worktree::commit_all(&wt, "nothing")), no_commit_beside(&format!("a nested repository in {rel}")));
    std::fs::write(wt.join("a.txt"), "two\n").unwrap();
    assert_eq!(git_refusal(worktree::commit_all(&wt, "The card's own work")),
               no_commit_beside(&format!("a nested repository in {rel}")));
    assert_eq!(git(&wt, &["rev-parse", "HEAD"]), head, "nothing was committed");
    assert_eq!(git(&wt, &["rev-parse", &format!("HEAD:{rel}")]), link, "the submodule's commit didn't change");
    assert!(!probe.exists(), "Commit… ran the filter");
    // a.txt as it was again: the take-over says "has uncommitted changes" before the nested repository
    std::fs::write(wt.join("a.txt"), "one\n").unwrap();

    // the stash, the removal and the move: refused as a nested repository, by its name
    let why = git_refusal(worktree::stash_all(&wt, "KADE-1 leftover"));
    assert_eq!(why, format!("Hersir doesn't stash there: it has a nested repository in {rel}, whose settings git's stash would run"));
    assert!(git(&repo, &["stash", "list"]).is_empty());
    let r = worktree::remove_card_worktree_with(&repo, &base, &tmp.path().join("run-temp"), BRANCH, true);
    assert_eq!(r.phrases(BRANCH), [format!("kept its worktree {} (it has a nested repository in {rel}, whose own settings git could run)",
                                           wt.display())]);
    let err = git_refusal(worktree::reuse_on(&repo, &wt, &base.join("KADE"), "KADE-2", "hersir/kade-2-next", "main", &[]));
    assert_eq!(err, format!("{} has a nested repository in {rel}, whose own settings git could run", wt.display()));
    assert!(wt.is_dir());
    assert!(!probe.exists(), "a refusal ran the filter");
    control(&wt, &probe);
}

// GA-361 (GA-357's check, a possible High on macOS): a submodule whose folder name git could read two ways. git on macOS
// turns a path it is given into NFC (core.precomposeunicode, on there by default), so Commit…'s
// `:(exclude,literal)cafe` + U+0301 became one of `café` in NFC and missed the gitlink the index holds in NFD, while
// macOS finds its folder under either name: git add looked inside it and ran its clean filter. Now Hersir refuses a
// gitlink whose path isn't in NFC, on every system, word for word, as it refuses GA-357's; and Commit… first asks git
// (`git ls-files` with git add's own pathspec) whether a submodule stays in, and refuses then too.

/// The card's sentence, word for word.
const TWO_WAYS: &str = "it has a submodule whose folder name git could read two ways, whose own settings git could run";

/// `café` as the card writes it in NFD: `cafe` and U+0301, a combining acute accent.
const CAFE_NFD: &str = "cafe\u{301}";
/// The same name in NFC, as git on macOS reads a path it is given: `caf` and U+00E9.
const CAFE_NFC: &str = "caf\u{e9}";

/// Whether `bytes` hold `part`.
fn has(bytes: &[u8], part: &[u8]) -> bool {
    bytes.windows(part.len()).any(|w| w == part)
}

/// A repository of its own at `nested` with n.txt and a .gitattributes that sends every file through filter x, committed
/// there, and not added to the checkout around it; its commit's id.
fn nested_repository(nested: &Path) -> String {
    std::fs::create_dir_all(nested).unwrap();
    git(nested, &["init", "-q", "-b", "main"]);
    std::fs::write(nested.join("n.txt"), "one\n").unwrap();
    std::fs::write(nested.join(".gitattributes"), "* filter=x\n").unwrap();
    git(nested, &["add", "n.txt", ".gitattributes"]);
    git(nested, &["commit", "-q", "-m", "nested"]);
    git(nested, &["rev-parse", "HEAD"])
}

/// What `plant_nested` does after its commit: gives the repository at `nested` a clean filter x that touches `probe`,
/// and changes n.txt, so a git status inside it runs the filter.
fn arm(nested: &Path, probe: &Path) {
    let config = std::fs::read_to_string(nested.join(".git/config")).unwrap();
    std::fs::write(nested.join(".git/config"),
                   format!("{config}[filter \"x\"]\n\tclean = touch {} && cat\n\tsmudge = cat\n", probe.display())).unwrap();
    std::fs::write(nested.join("n.txt"), "two\n").unwrap();
}

/// `all_four_refuse`, with `sentence` in place of GA-357's: Commit…, the stash of Remove anyway, the worktree's removal
/// and its take-over by a new card each refuse KADE-1's worktree `wt` with it, and change nothing.
fn all_four_refuse_with(tmp: &Path, repo: &Path, wt: &Path, sentence: &str) {
    let (head, tree) = (git(wt, &["rev-parse", "HEAD"]), index_tree(wt));
    let base = tmp.join("data/worktrees");

    assert_eq!(git_refusal(worktree::commit_all(wt, "The card's own work")), format!("Hersir doesn't commit there: {sentence}"));
    let shown = worktree::commit_all(wt, "The card's own work").unwrap_err().to_string();
    assert!(shown.contains(&format!("Hersir doesn't commit there: {sentence}")), "{shown}");
    assert_eq!(git(wt, &["rev-parse", "HEAD"]), head, "nothing was committed");
    assert_eq!(index_tree(wt), tree, "nothing was staged");

    assert_eq!(git_refusal(worktree::stash_all(wt, "KADE-1 leftover")), format!("Hersir doesn't stash there: {sentence}"));
    assert!(git(repo, &["stash", "list"]).is_empty(), "nothing was stashed");
    assert_eq!(index_tree(wt), tree);

    let r = worktree::remove_card_worktree_with(repo, &base, &tmp.join("run-temp"), BRANCH, true);
    assert_eq!(r.removed, None, "{r:?}");
    assert_eq!(r.kept, Some((wt.to_path_buf(), sentence.to_string())), "{r:?}");
    assert!(!r.branch_deleted, "{r:?}");
    assert_eq!(r.phrases(BRANCH), [format!("kept its worktree {} ({sentence})", wt.display())]);
    assert_eq!(worktree::remove_card_worktree(repo, &base, BRANCH, false).kept, Some((wt.to_path_buf(), sentence.to_string())));
    assert!(wt.is_dir());
    assert_eq!(git(repo, &["rev-parse", &format!("refs/heads/{BRANCH}")]), head, "the branch stays");

    let err = git_refusal(worktree::reuse_on(repo, wt, &base.join("KADE"), "KADE-2", "hersir/kade-2-next", "main", &[]));
    assert_eq!(err, format!("{}: {sentence}", wt.display()));
    let err = git_refusal(worktree::reuse(repo, wt, &base.join("KADE"), "KADE-2", "Next", "main", &[]));
    assert_eq!(err, format!("{}: {sentence}", wt.display()));
    assert!(wt.is_dir() && !base.join("KADE/KADE-2").exists());
    assert_eq!(git(wt, &["rev-parse", "--abbrev-ref", "HEAD"]), BRANCH, "the worktree is still on its card's branch");
    assert!(git(repo, &["branch", "--list", "hersir/kade-2*"]).is_empty(), "no branch for the new card");
    assert_eq!(git(wt, &["rev-parse", "HEAD"]), head);
}

#[test]
fn the_two_ways_sentence_is_the_cards_word_for_word() {
    assert_eq!(worktree::TWO_WAY_SUBMODULE, TWO_WAYS);
    assert_eq!(CAFE_NFD.as_bytes(), b"cafe\xcc\x81");
    assert_eq!(CAFE_NFC.as_bytes(), b"caf\xc3\xa9");
}

/// A gitlink in NFD in the index alone, with no folder on disk: refused by the four, staged or committed, among
/// gitlinks whose names are in NFC, and in a merge conflict. On every Unix: the name is only in git's index.
#[test]
fn a_submodule_whose_folder_name_isnt_in_nfc_is_refused_when_it_is_in_the_index_only() {
    let tmp = real_tempdir();
    let (repo, wt) = card(tmp.path());
    let lib = repo_at(&tmp.path().join("lib"));
    let (id, blob) = (git(&lib, &["rev-parse", "HEAD"]), git(&wt, &["rev-parse", "HEAD:a.txt"]));

    // staged only
    index_info(&wt, &[entry("160000", &id, CAFE_NFD.as_bytes())]);
    assert!(has(&git_out(&wt, &["ls-files", "--stage", "-z"]).stdout, b"\tcafe\xcc\x81\0"), "the index holds the NFD bytes");
    assert_eq!(git_refusal(worktree::gitlinks(&wt)), TWO_WAYS);
    assert_eq!(git_refusal(worktree::nested_repositories(&wt)), TWO_WAYS);
    all_four_refuse_with(tmp.path(), &repo, &wt, TWO_WAYS);

    // committed by the run, beside the same name in NFC and other gitlinks and files: still refused
    index_info(&wt, &[entry("160000", &id, b"a-lib"), entry("160000", &id, CAFE_NFC.as_bytes()), entry("160000", &id, b"zz/lib"),
                      entry("100644", &blob, b"zz/a.txt")]);
    git(&wt, &["commit", "-q", "-m", "submodules the run committed"]);
    assert_eq!(git_refusal(worktree::gitlinks(&wt)), TWO_WAYS);
    all_four_refuse_with(tmp.path(), &repo, &wt, TWO_WAYS);
    std::fs::write(wt.join("a.txt"), "two\n").unwrap();
    all_four_refuse_with(tmp.path(), &repo, &wt, TWO_WAYS);
    git(&wt, &["checkout", "-q", "a.txt"]);

    // without it, the name in NFC is a gitlink as any other
    let gone = |name: &[u8]| entry("0", "0000000000000000000000000000000000000000", name);
    index_info(&wt, &[gone(CAFE_NFD.as_bytes())]);
    git(&wt, &["commit", "-q", "-m", "without it"]);
    assert_eq!(worktree::gitlinks(&wt).unwrap(), ["a-lib", CAFE_NFC, "zz/lib"]);

    // other names that aren't in NFC, anywhere in the path: an accent in a parent folder, the Angstrom sign (NFC: Å),
    // Hangul letters written apart (NFC: 가), and U+0958 (NFC writes it as two characters)
    for name in [format!("vendor/{CAFE_NFD}/lib"), "\u{212b}ngstrom".to_string(), "\u{1100}\u{1161}".to_string(), "\u{958}".to_string()] {
        index_info(&wt, &[entry("160000", &id, name.as_bytes())]);
        assert_eq!(git_refusal(worktree::gitlinks(&wt)), TWO_WAYS, "{name:?}");
        assert_eq!(git_refusal(worktree::commit_all(&wt, "The card's own work")), format!("Hersir doesn't commit there: {TWO_WAYS}"),
                   "{name:?}");
        index_info(&wt, &[gone(name.as_bytes())]);
    }
    // names that are in NFC, also when they hold a combining mark NFC keeps apart (U+0915 U+093C), are left alone
    index_info(&wt, &[entry("160000", &id, "\u{915}\u{93c}".as_bytes()), entry("160000", &id, "\u{ac00}".as_bytes())]);
    assert_eq!(worktree::gitlinks(&wt).unwrap(), ["a-lib", CAFE_NFC, "zz/lib", "\u{915}\u{93c}", "\u{ac00}"]);

    // and the NFD name in a merge conflict (two stages of one path)
    index_info(&wt, &[entry("160000", &format!("{id} 2"), CAFE_NFD.as_bytes()), entry("160000", &format!("{id} 3"), CAFE_NFD.as_bytes())]);
    let stages = git_out(&wt, &["ls-files", "--stage", "-z"]).stdout;
    assert_eq!(stages.split(|&b| b == 0).filter(|e| e.starts_with(b"160000 ") && e.ends_with(b"\tcafe\xcc\x81")).count(), 2,
               "two stages of it");
    assert_eq!(git_refusal(worktree::gitlinks(&wt)), TWO_WAYS);
}

/// GA-361's case on Linux, with the folder on disk under its NFD name and a nested repository in it whose clean filter
/// touches a probe: the four refuse, and no filter runs. Linux's git reads the name as given, so GA-350's exclusion did
/// leave it out there: on Linux the refusal fails closed, as on every system.
#[cfg(target_os = "linux")]
#[test]
fn a_submodule_whose_folder_name_isnt_in_nfc_is_refused_by_commit_stash_removal_and_move_and_runs_no_filter() {
    let tmp = real_tempdir();
    let (repo, wt) = card(tmp.path());
    let probe = tmp.path().join("probe-ran");
    let nested = wt.join(CAFE_NFD);
    nested_repository(&nested);
    git(&wt, &["add", CAFE_NFD]);
    git(&wt, &["commit", "-q", "-m", "a nested repository whose folder name is in NFD"]);
    let index = git_out(&wt, &["ls-files", "--stage", "-z"]).stdout;
    assert!(has(&index, b"160000 ") && has(&index, b"\tcafe\xcc\x81\0"), "a gitlink in NFD: {}", String::from_utf8_lossy(&index));
    arm(&nested, &probe);

    assert_eq!(git_refusal(worktree::gitlinks(&wt)), TWO_WAYS);
    assert_eq!(git_refusal(worktree::nested_repositories(&wt)), TWO_WAYS);
    // what runs before Remove anyway's stash and in the card's views leaves the gitlink out, whatever its name
    assert_eq!(worktree::uncommitted(&wt).unwrap(), 0);
    assert!(worktree::uncommitted_files(&wt).unwrap().is_empty());
    assert_eq!(editor::status(&wt).uncommitted, 0);
    assert!(!probe.exists(), "Hersir's status ran the nested repository's filter");

    all_four_refuse_with(tmp.path(), &repo, &wt, TWO_WAYS);
    assert!(!probe.exists(), "a refusal ran the nested repository's filter");

    // with the card's own changes too
    std::fs::write(wt.join("a.txt"), "two\n").unwrap();
    std::fs::write(wt.join("b.txt"), "b\n").unwrap();
    assert_eq!(worktree::uncommitted(&wt).unwrap(), 2);
    all_four_refuse_with(tmp.path(), &repo, &wt, TWO_WAYS);
    assert_eq!(std::fs::read_to_string(wt.join("a.txt")).unwrap(), "two\n", "the card's change stays");
    assert!(wt.join("b.txt").is_file());
    assert_eq!(std::fs::read_to_string(nested.join("n.txt")).unwrap(), "two\n", "and the change inside it");
    assert!(!probe.exists(), "a refusal ran the nested repository's filter");

    // the probe is real: plain git status there runs the filter
    control(&wt, &probe);
    // and on Linux, what Commit… ran before GA-361 left it out: git reads the NFD name as given here
    std::fs::remove_file(&probe).unwrap();
    git(&wt, &["add", "--all", "--", ".", &format!(":(exclude,literal){CAFE_NFD}")]);
    assert!(!probe.exists(), "Linux's git add didn't leave out the gitlink in NFD");
}

/// A file (or link) whose name isn't in NFC doesn't matter: the gitlinks come back as before.
#[test]
fn a_file_whose_name_isnt_in_nfc_doesnt_matter() {
    let tmp = real_tempdir();
    let (_repo, wt) = card(tmp.path());
    let lib = repo_at(&tmp.path().join("lib"));
    let (id, blob) = (git(&lib, &["rev-parse", "HEAD"]), git(&wt, &["rev-parse", "HEAD:a.txt"]));
    index_info(&wt, &[entry("100644", &blob, format!("{CAFE_NFD}.txt").as_bytes()), entry("120000", &blob, format!("link-{CAFE_NFD}").as_bytes()),
                      entry("160000", &id, b"vendor/lib")]);
    assert_eq!(worktree::gitlinks(&wt).unwrap(), ["vendor/lib"]);
    assert!(worktree::nested_repositories(&wt).unwrap().is_empty(), "vendor/lib holds no repository");
}

/// On disk too (Linux): a file whose name is in NFD beside a nested repository gets GA-367's refusal for that
/// repository, not GA-361's, and the filter never runs; once the gitlink's folder holds no repository, Commit… commits
/// the file under its NFD name.
#[cfg(target_os = "linux")]
#[test]
fn a_file_whose_name_is_in_nfd_beside_a_nested_repository_gets_its_refusal_and_is_committed_without_one() {
    let tmp = real_tempdir();
    let (_repo, wt) = card(tmp.path());
    let probe = tmp.path().join("probe-ran");
    plant_nested(&wt, "nested", &probe);
    std::fs::write(wt.join(format!("{CAFE_NFD}.txt")), "nfd\n").unwrap();
    std::fs::write(wt.join("a.txt"), "two\n").unwrap();
    let head = git(&wt, &["rev-parse", "HEAD"]);
    assert_eq!(git_refusal(worktree::commit_all(&wt, "The card's own work")), no_commit_beside("a nested repository in nested"));
    assert_eq!(git(&wt, &["rev-parse", "HEAD"]), head, "nothing was committed");
    assert!(!probe.exists(), "Commit… ran the filter");
    control(&wt, &probe);

    std::fs::remove_dir_all(wt.join("nested")).unwrap();
    std::fs::create_dir(wt.join("nested")).unwrap();
    worktree::commit_all(&wt, "The card's own work").unwrap();
    let names = git_out(&wt, &["diff-tree", "--no-commit-id", "--name-only", "-z", "-r", "HEAD"]).stdout;
    assert_eq!(names, b"a.txt\0cafe\xcc\x81.txt\0", "{}", String::from_utf8_lossy(&names));
    assert_eq!(worktree::uncommitted(&wt).unwrap(), 0);
}

/// A submodule whose name is `café` in NFC, on every Unix (macOS's git reads that name as given), is a nested
/// repository as any other: Commit… (GA-367) and the stash refuse it by its name, not with GA-361's sentence.
#[test]
fn a_submodule_whose_folder_name_is_cafe_in_nfc_is_refused_by_commit_and_stash_as_a_nested_repository() {
    let tmp = real_tempdir();
    let (repo, wt) = card(tmp.path());
    let probe = tmp.path().join("probe-ran");
    plant_nested(&wt, CAFE_NFC, &probe);
    let link = git(&wt, &["rev-parse", &format!("HEAD:{CAFE_NFC}")]);
    assert!(has(&git_out(&wt, &["ls-files", "--stage", "-z"]).stdout, b"\tcaf\xc3\xa9\0"), "the index holds it in NFC");
    assert_eq!(worktree::gitlinks(&wt).unwrap(), [CAFE_NFC]);
    assert_eq!(worktree::nested_repositories(&wt).unwrap(), [CAFE_NFC]);

    std::fs::write(wt.join("a.txt"), "two\n").unwrap();
    let head = git(&wt, &["rev-parse", "HEAD"]);
    assert_eq!(git_refusal(worktree::commit_all(&wt, "The card's own work")),
               no_commit_beside(&format!("a nested repository in {CAFE_NFC}")));
    assert_eq!(git(&wt, &["rev-parse", "HEAD"]), head, "nothing was committed");
    assert_eq!(git(&wt, &["rev-parse", &format!("HEAD:{CAFE_NFC}")]), link, "the submodule's commit didn't change");
    assert_eq!(worktree::uncommitted(&wt).unwrap(), 1, "a.txt is still uncommitted");
    assert!(!probe.exists(), "Commit… ran the filter");
    let why = git_refusal(worktree::stash_all(&wt, "KADE-1 leftover"));
    assert_eq!(why, format!("Hersir doesn't stash there: it has a nested repository in {CAFE_NFC}, whose settings git's stash would run"));
    assert!(git(&repo, &["stash", "list"]).is_empty());
    control(&wt, &probe);
}

/// The card's setup on macOS: a gitlink the index holds as `cafe` + U+0301 (NFD), written with git update-index
/// --index-info, beside a folder `café` (NFC) that holds a nested repository whose clean filter touches the probe it
/// gives back; macOS finds that folder under either name. The card's own a.txt has changed, so Commit… gets to its git add.
#[cfg(target_os = "macos")]
fn nfd_gitlink_beside_a_nested_repository(tmp: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let (repo, wt) = card(tmp);
    let probe = tmp.join("probe-ran");
    let nested = wt.join(CAFE_NFC);
    let id = nested_repository(&nested);
    index_info(&wt, &[entry("160000", &id, CAFE_NFD.as_bytes())]);
    git(&wt, &["commit", "-q", "-m", "a nested repository, as a gitlink in NFD"]);
    let index = git_out(&wt, &["ls-files", "--stage", "-z"]).stdout;
    assert!(has(&index, b"160000 ") && has(&index, b"\tcafe\xcc\x81\0") && !has(&index, b"caf\xc3\xa9"),
            "the index holds the gitlink in NFD alone: {}", String::from_utf8_lossy(&index));
    assert!(wt.join(CAFE_NFD).join(".git").is_dir(), "macOS finds the folder café under its NFD name too");
    arm(&nested, &probe);
    std::fs::write(wt.join("a.txt"), "two\n").unwrap();
    (repo, wt, probe)
}

/// GA-361's test (the card's steps 1 and 3), on macOS: Commit… refuses with the card's sentence and never runs the
/// nested repository's filter; neither do the stash, the removal and the move.
#[cfg(target_os = "macos")]
#[test]
fn on_macos_a_gitlink_stored_in_nfd_beside_a_nested_repository_is_refused_by_commit_and_runs_no_filter() {
    let tmp = real_tempdir();
    let (repo, wt, probe) = nfd_gitlink_beside_a_nested_repository(tmp.path());
    let (head, tree) = (git(&wt, &["rev-parse", "HEAD"]), index_tree(&wt));

    assert_eq!(git_refusal(worktree::gitlinks(&wt)), TWO_WAYS);
    assert_eq!(git_refusal(worktree::commit_all(&wt, "The card's own work")), format!("Hersir doesn't commit there: {TWO_WAYS}"));
    assert!(!probe.exists(), "Commit… ran the nested repository's filter");
    assert_eq!(git(&wt, &["rev-parse", "HEAD"]), head, "nothing was committed");
    assert_eq!(index_tree(&wt), tree, "nothing was staged");

    all_four_refuse_with(tmp.path(), &repo, &wt, TWO_WAYS);
    assert!(!probe.exists(), "a refusal ran the nested repository's filter");
    assert_eq!(std::fs::read_to_string(wt.join("a.txt")).unwrap(), "two\n", "the card's change stays");

    // the probe is real: plain git status there looks inside the folder by the gitlink's NFD name and runs the filter
    control(&wt, &probe);
}

/// Whether the case was real before GA-361, on macOS (the card's step 1): what Commit… ran then, git add --all with
/// GA-350's pathspec, which leaves out the gitlink by the name the index holds, runs the nested repository's filter,
/// because git reads that exclusion in NFC. Its own test, so that the result shows apart from the fix's.
#[cfg(target_os = "macos")]
#[test]
fn on_macos_commits_git_add_before_ga_361_missed_a_gitlink_stored_in_nfd_and_ran_its_filter() {
    let tmp = real_tempdir();
    let (_repo, wt, probe) = nfd_gitlink_beside_a_nested_repository(tmp.path());
    let exclusion = format!(":(exclude,literal){CAFE_NFD}");
    assert!(!probe.exists());
    git(&wt, &["add", "--all", "--", ".", &exclusion]);
    assert!(probe.exists(), "git add with GA-350's pathspec didn't run the filter: the case wasn't real on this macOS");
    // why: git ls-files, which reads a pathspec as git add does, keeps the gitlink in
    let left_in = git(&wt, &["ls-files", "--stage", "--", ".", &exclusion]);
    assert!(left_in.lines().any(|l| l.starts_with("160000 ")), "{left_in}");
}

/// GA-361's second guard, on macOS: a name in NFC that macOS's git might still read as another one (its own tables
/// are older than Unicode's), such as क and a nukta (U+0915 U+093C), which NFC keeps apart while U+0958 is the same
/// letter in one. However git reads it, Commit… refuses: with the card's sentence when git's pathspec reads it as another
/// name, else (GA-367) as a nested repository, by its name; it commits nothing and never runs the nested repository's
/// filter.
#[cfg(target_os = "macos")]
#[test]
fn on_macos_a_submodule_whose_name_is_in_nfc_never_runs_its_filter_however_git_reads_it() {
    let tmp = real_tempdir();
    let (_repo, wt) = card(tmp.path());
    let probe = tmp.path().join("probe-ran");
    let name = "\u{915}\u{93c}";
    let nested = wt.join(name);
    let id = nested_repository(&nested);
    index_info(&wt, &[entry("160000", &id, name.as_bytes())]);
    git(&wt, &["commit", "-q", "-m", "a nested repository whose name NFC keeps as two characters"]);
    arm(&nested, &probe);
    std::fs::write(wt.join("a.txt"), "two\n").unwrap();
    assert_eq!(worktree::gitlinks(&wt).unwrap(), [name]);
    let head = git(&wt, &["rev-parse", "HEAD"]);

    let m = git_refusal(worktree::commit_all(&wt, "The card's own work"));
    assert!(m == format!("Hersir doesn't commit there: {TWO_WAYS}") || m == no_commit_beside(&format!("a nested repository in {name}")),
            "{m}");
    assert_eq!(git(&wt, &["rev-parse", "HEAD"]), head, "nothing was committed");
    assert!(has(&git_out(&wt, &["ls-tree", "-z", "HEAD"]).stdout, format!("160000 commit {id}\t{name}\0").as_bytes()),
            "the submodule stays as it was");
    assert!(!probe.exists(), "Commit… ran the nested repository's filter");
    control(&wt, &probe);
}

// GA-367 (GA-361's check, a High, CWE-436): a run can write a NUL byte into a gitlink's name in its worktree's index.
// git ls-files gives that name up to the NUL, while git add matches its pathspec against all of it: so with the gitlink
// `a`, NUL, `b` beside a nested repository `a`, neither GA-350's `:(exclude,literal)a` nor GA-361's check with git
// ls-files left it out, and git add looked inside the folder `a` and ran that repository's clean filter. Now Commit…
// refuses every checkout with a nested repository (`nested_repositories`), as the stash, the removal and a new card's
// take-over do; GA-357's and GA-361's refusals stay word for word, before it.

/// Commit…'s refusal of a checkout with a nested repository, word for word: "Hersir doesn't commit there: it has " and
/// `nested` (`nested_said`'s words, such as "a nested repository in lib"), then ", whose own settings git could run".
fn no_commit_beside(nested: &str) -> String {
    format!("Hersir doesn't commit there: it has {nested}, whose own settings git could run")
}

/// SHA-1 of `data` (FIPS 180-4), for the index file's checksum, which git checks when it reads the index; the tests
/// have no SHA-1 crate. `nul_in_gitlink_name` checks it against git's own first.
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476, 0xC3D2_E1F0];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for block in msg.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(block[4 * i..4 * i + 4].try_into().unwrap());
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            (e, d, c, b, a) = (d, c, b.rotate_left(30), a, t);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 20];
    for (i, x) in h.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&x.to_be_bytes());
    }
    out
}

/// The index file of the checkout at `dir`, where git keeps it (a card worktree's is in its repository's .git).
fn index_file(dir: &Path) -> PathBuf {
    let path = PathBuf::from(git(dir, &["rev-parse", "--git-path", "index"]));
    if path.is_absolute() { path } else { dir.join(path) }
}

/// What a run can do to the index file itself (GA-361's check): the gitlink `a!b`, staged with `index_info`, becomes
/// `a`, NUL, `b`. The index is written in version 2 first, where an entry's name is its own bytes after its flags,
/// whose last 12 bits are its length (3, which stays); the file's trailing SHA-1 is computed again, so git reads it.
fn nul_in_gitlink_name(dir: &Path) {
    git(dir, &["update-index", "--index-version", "2"]);
    let file = index_file(dir);
    let mut index = std::fs::read(&file).unwrap();
    assert_eq!(&index[..8], b"DIRC\0\0\0\x02", "an index in version 2");
    let body = index.len() - 20;
    assert_eq!(sha1(&index[..body]), index[body..], "this SHA-1 gives the checksum git wrote");
    let at: Vec<usize> = (0..body - 5).filter(|&i| index[i..i + 5] == *b"\0\x03a!b").collect();
    assert_eq!(at.len(), 1, "the entry a!b, its flags (stage 0, length 3) and then its name, once in the index");
    index[at[0] + 3] = 0;
    let sum = sha1(&index[..body]);
    index[body..].copy_from_slice(&sum);
    std::fs::write(&file, &index).unwrap();
}

#[test]
fn this_sha1_is_sha1() {
    let hex = |d: [u8; 20]| d.iter().map(|b| format!("{b:02x}")).collect::<String>();
    assert_eq!(hex(sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    assert_eq!(hex(sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
    assert_eq!(hex(sha1(&[b'a'; 1000])), "291e9a6c66994949b57ba5e650361e98fc36b1ba");
}

/// The card's case, in a card's worktree: the gitlink `a`, NUL, `b`, staged by the run, beside a nested repository `a`
/// that isn't in the index, whose clean filter touches a probe; the card's own a.txt has changed. Commit… refuses with
/// "Hersir doesn't commit there: " and `nested_said`'s words, stages and commits nothing, and the filter never runs;
/// nor do the stash, the removal and the take-over. The control: what main's Commit… ran (GA-361's check, then git add
/// with its pathspec) lets the gitlink through and runs the filter.
#[test]
fn a_gitlink_whose_name_in_the_index_holds_a_nul_byte_beside_a_nested_repository_is_refused_by_commit_and_runs_no_filter() {
    let tmp = real_tempdir();
    let (repo, wt) = card(tmp.path());
    let probe = tmp.path().join("probe-ran");
    let nested = wt.join("a");
    let id = nested_repository(&nested);
    arm(&nested, &probe);
    index_info(&wt, &[entry("160000", &id, b"a!b")]);
    nul_in_gitlink_name(&wt);
    std::fs::write(wt.join("a.txt"), "two\n").unwrap();
    assert!(!probe.exists());

    // git reads the index: git ls-files gives the gitlink's name up to the NUL, and the folder git opens is `a`
    let listed = git_out(&wt, &["ls-files", "--stage", "-z"]);
    assert!(listed.status.success(), "{}", String::from_utf8_lossy(&listed.stderr));
    assert!(has(&listed.stdout, format!("160000 {id} 0\ta\0").as_bytes()), "{}", String::from_utf8_lossy(&listed.stdout));
    assert_eq!(worktree::gitlinks(&wt).unwrap(), ["a"]);
    assert_eq!(worktree::nested_repositories(&wt).unwrap(), ["a"]);
    let (head, index) = (git(&wt, &["rev-parse", "HEAD"]), std::fs::read(index_file(&wt)).unwrap());

    // Commit… refuses, word for word, and as the user sees it
    let refusal = no_commit_beside("a nested repository in a");
    assert_eq!(refusal, "Hersir doesn't commit there: it has a nested repository in a, whose own settings git could run");
    assert_eq!(git_refusal(worktree::commit_all(&wt, "The card's own work")), refusal);
    let shown = worktree::commit_all(&wt, "The card's own work").unwrap_err().to_string();
    assert!(shown.contains(&refusal), "{shown}");
    assert!(!probe.exists(), "Commit… ran the nested repository's filter");
    assert_eq!(git(&wt, &["rev-parse", "HEAD"]), head, "nothing was committed");
    assert_eq!(std::fs::read(index_file(&wt)).unwrap(), index, "nothing was staged: the index is as it was, byte for byte");
    assert_eq!(std::fs::read_to_string(wt.join("a.txt")).unwrap(), "two\n", "the card's change stays");

    // the stash, the removal and a new card's take-over refuse it as before
    assert_eq!(git_refusal(worktree::stash_all(&wt, "KADE-1 leftover")),
               "Hersir doesn't stash there: it has a nested repository in a, whose settings git's stash would run");
    assert!(git(&repo, &["stash", "list"]).is_empty(), "nothing was stashed");
    let base = tmp.path().join("data/worktrees");
    let r = worktree::remove_card_worktree_with(&repo, &base, &tmp.path().join("run-temp"), BRANCH, true);
    assert_eq!(r.phrases(BRANCH), [format!("kept its worktree {} (it has a nested repository in a, whose own settings git could run)",
                                           wt.display())]);
    // the take-over: its check for uncommitted changes (a.txt) comes first, else the nested repository's
    let err = git_refusal(worktree::reuse_on(&repo, &wt, &base.join("KADE"), "KADE-2", "hersir/kade-2-next", "main", &[]));
    assert!([format!("{} has uncommitted changes", wt.display()),
             format!("{} has a nested repository in a, whose own settings git could run", wt.display())].contains(&err), "{err}");
    assert!(wt.is_dir() && !base.join("KADE/KADE-2").exists());
    assert!(!probe.exists(), "a refusal ran the nested repository's filter");
    assert!(has(&git_out(&wt, &["ls-files", "--stage", "-z"]).stdout, format!("160000 {id} 0\ta\0").as_bytes()),
            "the gitlink is still in the index");
    assert_eq!(git(&wt, &["rev-parse", "HEAD"]), head, "nothing was committed");

    // the control, with Hersir's own git there: GA-361's check (git ls-files with git add's pathspec) leaves no gitlink
    // in, so main's Commit… went on, and its git add --all with that pathspec looked inside `a` and ran the filter
    let pathspec = ["--", ".", ":(exclude,literal)a"];
    let left_in = worktree::git_command(&wt).args(["ls-files", "--stage", "-z"]).args(pathspec).output().unwrap();
    assert!(left_in.status.success());
    assert!(!left_in.stdout.split(|&b| b == 0).any(|e| e.starts_with(b"160000 ")), "{}", String::from_utf8_lossy(&left_in.stdout));
    let added = worktree::git_command(&wt).args(["add", "--all"]).args(pathspec).output().unwrap();
    assert!(added.status.success(), "{}", String::from_utf8_lossy(&added.stderr));
    assert!(probe.exists(), "main's git add didn't run the nested repository's filter, so the refusal above proves nothing");
}

/// Commit…'s order with a nested repository (GA-367): an empty message is still said first; GA-357's and GA-361's
/// sentences come before the new one, word for word, in their own cases; the new one is said also when no file has
/// changed; and two repositories give `nested_said`'s "and 1 more". No refusal runs a filter.
#[test]
fn commit_all_says_the_message_and_the_names_first_and_refuses_a_nested_repository_before_nothing_to_commit() {
    let tmp = real_tempdir();
    let (_repo, wt) = card(tmp.path());
    let probe = tmp.path().join("probe-ran");
    plant_nested(&wt, "nested", &probe);
    plant_nested(&wt, "vendor/lib", &probe);
    std::fs::remove_file(&probe).ok();
    let head = git(&wt, &["rev-parse", "HEAD"]);
    assert_eq!(worktree::nested_repositories(&wt).unwrap(), ["nested", "vendor/lib"]);

    assert_eq!(git_refusal(worktree::commit_all(&wt, "  \n")), "Write a commit message first");
    assert_eq!(git_refusal(worktree::commit_all(&wt, "nothing changed")), no_commit_beside("nested repositories in nested and 1 more"));
    std::fs::write(wt.join("a.txt"), "two\n").unwrap();
    assert_eq!(git_refusal(worktree::commit_all(&wt, "The card's own work")),
               no_commit_beside("nested repositories in nested and 1 more"));

    // a gitlink whose name isn't UTF-8 (GA-357), then one not in NFC (GA-361), staged beside them: their sentences
    let gone = |name: &[u8]| entry("0", "0000000000000000000000000000000000000000", name);
    index_info(&wt, &[entry("160000", &id_of(&wt.join("nested")), b"sub\xff")]);
    assert_eq!(git_refusal(worktree::commit_all(&wt, "The card's own work")), format!("Hersir doesn't commit there: {NOT_UTF8}"));
    index_info(&wt, &[gone(b"sub\xff"), entry("160000", &id_of(&wt.join("nested")), CAFE_NFD.as_bytes())]);
    assert_eq!(git_refusal(worktree::commit_all(&wt, "The card's own work")), format!("Hersir doesn't commit there: {TWO_WAYS}"));
    index_info(&wt, &[gone(CAFE_NFD.as_bytes())]);
    assert_eq!(git_refusal(worktree::commit_all(&wt, "The card's own work")),
               no_commit_beside("nested repositories in nested and 1 more"));

    assert_eq!(git(&wt, &["rev-parse", "HEAD"]), head, "nothing was committed");
    assert_eq!(std::fs::read_to_string(wt.join("a.txt")).unwrap(), "two\n");
    assert!(!probe.exists(), "Commit… ran a nested repository's filter");
    control(&wt, &probe);
}

/// The commit a repository at `dir` is at.
fn id_of(dir: &Path) -> String {
    git(dir, &["rev-parse", "HEAD"])
}
