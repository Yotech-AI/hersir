//! GA-350 (GA-326 #1, CWE-78): a run commits a nested repository into its worktree (a folder with a `.git` of its own,
//! a gitlink to git) whose own config has a clean filter that touches a file, then changes a file in it. Git's look
//! inside a submodule is a git status there with that repository's settings, so plain git status in the worktree runs
//! the filter, as you (each test checks that last, so its probe is a real one). Hersir's own git never does: its counts
//! of uncommitted files, a card's Changes, the Editor's status, Commit…, Update's check and the worktree's removal leave
//! submodules out (`worktree::IGNORE_SUBMODULES`), or leave such a worktree alone (`worktree::nested_repositories`).
//! A project with a real submodule (`.gitmodules`) still shows a card's own changes; those inside the submodule don't.
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

#[test]
fn commit_all_commits_the_cards_own_files_without_the_nested_repository_and_runs_no_filter() {
    let tmp = real_tempdir();
    let (_repo, wt) = card(tmp.path());
    let probe = tmp.path().join("probe-ran");
    plant_nested(&wt, "nested", &probe);
    let link = git(&wt, &["rev-parse", "HEAD:nested"]);

    // only the nested repository changed: nothing to commit, and no git commit that prints a status
    let err = worktree::commit_all(&wt, "nothing").unwrap_err().to_string();
    assert!(err.contains("There is nothing to commit: no file has changed"), "{err}");
    assert!(!probe.exists(), "Commit… ran the filter");

    // the card's own files go in; the gitlink stays at its commit
    std::fs::write(wt.join("a.txt"), "two\n").unwrap();
    std::fs::create_dir(wt.join("src")).unwrap();
    std::fs::write(wt.join("src/b.txt"), "b\n").unwrap();
    let c = worktree::commit_all(&wt, "The card's own work").unwrap();
    assert_eq!(c.subject, "The card's own work");
    assert_eq!(git(&wt, &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]), "a.txt\nsrc/b.txt");
    assert_eq!(git(&wt, &["rev-parse", "HEAD:nested"]), link, "the nested repository's commit didn't change");
    assert_eq!(worktree::uncommitted(&wt).unwrap(), 0);
    assert!(!probe.exists(), "Commit… ran the filter");

    control(&wt, &probe);
    // and the change inside it stays there
    assert_eq!(std::fs::read_to_string(wt.join("nested/n.txt")).unwrap(), "two\n");
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

#[test]
fn a_project_with_a_real_submodule_shows_the_cards_own_changes_and_none_inside_the_submodule() {
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

    // a commit of the card's own work leaves the submodule as it is
    worktree::commit_all(&wt, "The card's own work").unwrap();
    assert_eq!(git(&wt, &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]), own.join("\n"));
    assert_eq!(git(&wt, &["rev-parse", "HEAD:lib"]), git(&lib, &["rev-parse", "main"]));
    assert_eq!(git(&wt.join("lib"), &["status", "--porcelain"]), " M a.txt\n?? new-in-lib.txt", "the change stays inside it");
    // the branch's Changes after the commit: still only the card's own files
    let changed = changes::of_worktree(&wt, &main).unwrap();
    assert_eq!(paths(changed.iter().map(|f| f.path.clone())), own);
    assert!(changed.iter().all(|f| !f.uncommitted), "{changed:?}");
    let head = git(&wt, &["rev-parse", "HEAD"]);
    let between = changes::between(&repo, &main, &head).unwrap();
    assert_eq!(paths(between.iter().map(|f| f.path.clone())), own);

    // its removal: a worktree with a checked-out submodule stays, as git itself would keep it
    let r = worktree::remove_card_worktree_with(&repo, &tmp.path().join("data/worktrees"), &tmp.path().join("run-temp"), BRANCH, false);
    assert!(r.kept.as_ref().is_some_and(|(_, why)| why.contains("a nested repository in lib")), "{r:?}");
}
