use gizai_agents::worktree;
use std::process::Command;

/// The time limit a card start gives `fetch_start`.
const MINUTE: std::time::Duration = std::time::Duration::from_secs(60);

fn git(dir: &std::path::Path, args: &[&str]) { assert!(Command::new("git").args(args).current_dir(dir).status().unwrap().success()); }

fn repo(tmp: &std::path::Path) -> std::path::PathBuf {
    let repo = tmp.join("repo"); std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "init"]);
    repo
}

#[test]
fn creates_then_reuses_a_worktree_and_rejects_non_repos() {
    let tmp = real_tempdir();
    let repo = repo(tmp.path());
    let base = tmp.path().join("wt");
    let a = worktree::ensure(&repo, &base, "KADE-41", "Export invoices as CSV!", "main").unwrap();
    assert!(a.created && a.path.join(".git").exists());
    assert_eq!(a.branch, "gizai/kade-41-export-invoices-as-csv");
    let b = worktree::ensure(&repo, &base, "KADE-41", "Export invoices as CSV!", "main").unwrap();
    assert!(!b.created);
    assert_eq!(b.branch, a.branch);
    let not = tmp.path().join("plain"); std::fs::create_dir(&not).unwrap();
    assert!(matches!(worktree::ensure(&not, &base, "X-1", "t", "main"), Err(gizai_agents::AgentError::NotGitRepo(_))));
}

#[test]
fn reuses_an_existing_branch_after_the_worktree_was_removed() {
    let tmp = real_tempdir();
    let repo = repo(tmp.path());
    let base = tmp.path().join("wt");
    let a = worktree::ensure(&repo, &base, "GFW-7", "Mollie webhook", "main").unwrap();
    git(&repo, &["worktree", "remove", "--force", a.path.to_str().unwrap()]);
    let b = worktree::ensure(&repo, &base, "GFW-7", "Mollie webhook", "main").unwrap();
    assert!(b.created);
    assert_eq!(b.branch, "gizai/gfw-7-mollie-webhook");
}

#[test]
fn slugs_are_short_ascii_and_tidy() {
    assert_eq!(worktree::slug("  Café über   Straße -- naïve!! ", 40), "cafe-uber-strasse-naive");
    assert_eq!(worktree::slug("A very long title that goes on and on and on forever", 20), "a-very-long-title-th");
    assert_eq!(worktree::slug("!!!", 40), "");
}

#[test]
fn a_bad_default_branch_is_a_git_error() {
    let tmp = real_tempdir();
    let repo = repo(tmp.path());
    let r = worktree::ensure(&repo, &tmp.path().join("wt"), "X-2", "t", "develop");
    assert!(matches!(r, Err(gizai_agents::AgentError::Git(_))), "{r:?}");
}

fn rev(dir: &std::path::Path, r: &str) -> String {
    String::from_utf8(Command::new("git").args(["rev-parse", r]).current_dir(dir).output().unwrap().stdout).unwrap().trim().to_string()
}

/// "GitHub" (a bare repository), and a local clone that has fallen behind it, its remote named like Jeffrey's.
fn github_and_stale_clone(tmp: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let src = repo(tmp);
    let bare = tmp.join("github.git");
    git(tmp, &["clone", "-q", "--bare", src.to_str().unwrap(), bare.to_str().unwrap()]);
    let local = tmp.join("local");
    git(tmp, &["clone", "-q", "-o", "acme-labs", bare.to_str().unwrap(), local.to_str().unwrap()]);
    push_new_commit(&src, &bare, "on github");
    (src, bare, local)
}

fn push_new_commit(src: &std::path::Path, bare: &std::path::Path, msg: &str) {
    git(src, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", msg]);
    git(src, &["push", "-q", bare.to_str().unwrap(), "main"]);
}

#[test]
fn a_new_card_starts_from_main_fetched_from_github_not_the_stale_local_main() {
    let tmp = real_tempdir();
    let (src, bare, local) = github_and_stale_clone(tmp.path());
    assert_ne!(rev(&local, "main"), rev(&bare, "main"), "the local main is behind");
    assert_eq!(worktree::remotes(&local).unwrap(), vec![("acme-labs".to_string(), bare.to_string_lossy().to_string())]);
    let start = worktree::fetch_start(&local, Some("acme-labs"), bare.to_str().unwrap(), "main", MINUTE).unwrap();
    assert_eq!(start, "refs/remotes/acme-labs/main");
    let wt = worktree::ensure(&local, &tmp.path().join("wt"), "SH-1", "Wireframes", &start).unwrap();
    assert_eq!(rev(&wt.path, "HEAD"), rev(&bare, "main"));
    let upstream = Command::new("git").args(["rev-parse", "--abbrev-ref", "@{upstream}"]).current_dir(&wt.path).output().unwrap();
    assert!(!upstream.status.success(), "no upstream, so a push can't land on main by accident");
    assert_eq!(worktree::behind(&wt.path, &start).unwrap(), 0);
    // GitHub moves on: the card's branch is now one commit behind
    push_new_commit(&src, &bare, "later on github");
    let start = worktree::fetch_start(&local, Some("acme-labs"), bare.to_str().unwrap(), "main", MINUTE).unwrap();
    assert_eq!(worktree::behind(&wt.path, &start).unwrap(), 1);
}

#[test]
fn without_a_matching_remote_the_link_is_fetched_into_a_hidden_ref() {
    let tmp = real_tempdir();
    let (_src, bare, _local) = github_and_stale_clone(tmp.path());
    let plain = repo(&tmp.path().join("p").tap_mkdir());
    let start = worktree::fetch_start(&plain, None, bare.to_str().unwrap(), "main", MINUTE).unwrap();
    assert_eq!(start, "refs/gizai/base/main");
    assert_eq!(rev(&plain, &start), rev(&bare, "main"));
    let branches = String::from_utf8(Command::new("git").args(["branch", "-a"]).current_dir(&plain).output().unwrap().stdout).unwrap();
    assert!(!branches.contains("gizai/base"), "not in your branch list: {branches}");
}

#[test]
fn an_unreachable_repository_is_a_clear_error() {
    let tmp = real_tempdir();
    let local = repo(tmp.path());
    let e = worktree::fetch_start(&local, None, "/nonexistent/github.git", "main", MINUTE).unwrap_err().to_string();
    assert!(e.contains("Couldn't fetch main from /nonexistent/github.git"), "{e}");
}

trait TapMkdir { fn tap_mkdir(self) -> Self; }
impl TapMkdir for std::path::PathBuf { fn tap_mkdir(self) -> Self { std::fs::create_dir_all(&self).unwrap(); self } }

// ---- Review on GitHub: pushing a card's branch, and the clean-up after its merge ----

fn commit_file(dir: &std::path::Path, name: &str, text: &str) {
    std::fs::write(dir.join(name), text).unwrap();
    git(dir, &["add", name]);
    git(dir, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", name]);
}

fn has_branch(repo: &std::path::Path, branch: &str) -> bool {
    Command::new("git").args(["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).current_dir(repo).status().unwrap().success()
}

#[test]
fn pushing_a_cards_branch_puts_its_commits_on_github_and_never_forces() {
    let tmp = real_tempdir();
    let (_src, bare, local) = github_and_stale_clone(tmp.path());
    let wt = worktree::ensure(&local, &tmp.path().join("wt"), "SH-1", "Export", "main").unwrap();
    commit_file(&wt.path, "export.csv", "a,b\n");
    worktree::push_branch(&local, "acme-labs", &wt.branch).unwrap();
    assert_eq!(rev(&bare, &wt.branch), rev(&wt.path, "HEAD"), "through the remote's name");
    commit_file(&wt.path, "more.csv", "c\n");
    worktree::push_branch(&local, bare.to_str().unwrap(), &wt.branch).unwrap();
    assert_eq!(rev(&bare, &wt.branch), rev(&wt.path, "HEAD"), "or to the link itself");
    // rewritten history is refused, not forced over GitHub's copy
    let on_github = rev(&bare, &wt.branch);
    git(&wt.path, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--amend", "-m", "rewritten"]);
    let e = worktree::push_branch(&local, bare.to_str().unwrap(), &wt.branch).unwrap_err().to_string();
    assert!(e.contains(&format!("Couldn't push {} to {}", wt.branch, bare.display())), "{e}");
    assert_eq!(rev(&bare, &wt.branch), on_github);
    // nowhere to push to
    let e = worktree::push_branch(&local, "/nonexistent/github.git", &wt.branch).unwrap_err().to_string();
    assert!(e.contains("Couldn't push"), "{e}");
}

#[test]
fn a_clean_card_worktree_is_removed_and_its_branch_deleted_when_asked() {
    let tmp = real_tempdir();
    let repo = repo(tmp.path());
    let base = tmp.path().join("worktrees");
    let a = worktree::ensure(&repo, &base.join("SH"), "SH-1", "Export", "main").unwrap();
    commit_file(&a.path, "a.txt", "a");
    let r = worktree::remove_card_worktree(&repo, &base, &a.branch, true);
    assert_eq!((r.removed.as_deref(), r.kept.is_none(), r.branch_deleted, r.branch_kept.is_none()), (Some(a.path.as_path()), true, true, true), "{r:?}");
    assert!(!a.path.exists() && !has_branch(&repo, &a.branch));
    assert_eq!(r.phrases(&a.branch), ["removed its worktree".to_string(), format!("deleted branch {}", a.branch)]);
    // without delete_branch the branch stays, and nothing says it was refused
    let b = worktree::ensure(&repo, &base.join("SH"), "SH-2", "Import", "main").unwrap();
    let r = worktree::remove_card_worktree(&repo, &base, &b.branch, false);
    assert!(r.removed.is_some() && !r.branch_deleted && r.branch_kept.is_none(), "{r:?}");
    assert!(!b.path.exists() && has_branch(&repo, &b.branch));
    assert_eq!(r.phrases(&b.branch), ["removed its worktree"]);
    // a branch no worktree has checked out any more is just deleted
    let r = worktree::remove_card_worktree(&repo, &base, &b.branch, true);
    assert!(r.removed.is_none() && r.branch_deleted, "{r:?}");
    assert!(!has_branch(&repo, &b.branch));
}

#[test]
fn a_card_worktree_with_uncommitted_work_is_kept_with_its_branch() {
    let tmp = real_tempdir();
    let repo = repo(tmp.path());
    let base = tmp.path().join("worktrees");
    let a = worktree::ensure(&repo, &base, "SH-1", "Export", "main").unwrap();
    assert_eq!(worktree::uncommitted(&a.path).unwrap(), 0);
    std::fs::write(a.path.join("notes.txt"), "not committed").unwrap();
    assert_eq!(worktree::uncommitted(&a.path).unwrap(), 1, "an untracked file counts");
    let r = worktree::remove_card_worktree(&repo, &base, &a.branch, true);
    assert_eq!(r.kept, Some((a.path.clone(), "it has uncommitted changes".to_string())), "{r:?}");
    assert!(r.removed.is_none() && !r.branch_deleted);
    assert!(a.path.join("notes.txt").exists() && has_branch(&repo, &a.branch), "nothing lost");
    // the path as git gives it (with / on Windows)
    assert_eq!(r.phrases(&a.branch), [format!("kept its worktree {} (it has uncommitted changes)", git_path(&a.path))]);
}

#[test]
fn only_gizais_own_worktrees_are_removed() {
    let tmp = real_tempdir();
    let repo = repo(tmp.path());
    let base = tmp.path().join("worktrees");
    std::fs::create_dir_all(&base).unwrap();
    // your own checkout of the branch, next to the repository
    let mine = tmp.path().join("mine");
    git(&repo, &["worktree", "add", "-q", "-b", "gizai/sh-1-export", mine.to_str().unwrap()]);
    let r = worktree::remove_card_worktree(&repo, &base, "gizai/sh-1-export", true);
    assert_eq!(r.kept, Some((mine.clone(), "it isn't one of Hersir's worktrees".to_string())), "{r:?}");
    assert!(mine.exists() && has_branch(&repo, "gizai/sh-1-export") && !r.branch_deleted);
    // the main checkout itself on the branch
    let r = worktree::remove_card_worktree(&repo, &base, "main", true);
    assert!(r.kept.is_some() && r.removed.is_none() && !r.branch_deleted, "{r:?}");
    assert!(repo.join(".git").exists() && has_branch(&repo, "main"));
    // the worktrees folder itself is not "inside" it
    let r = worktree::remove_card_worktree(&repo, &repo, "main", false);
    assert!(r.kept.is_some() && r.removed.is_none(), "{r:?}");
}

#[test]
fn a_card_worktree_whose_folder_is_gone_only_leaves_gits_list() {
    let tmp = real_tempdir();
    let repo = repo(tmp.path());
    let base = tmp.path().join("worktrees");
    let a = worktree::ensure(&repo, &base.join("SH"), "SH-1", "Export", "main").unwrap();
    std::fs::remove_dir_all(&a.path).unwrap();
    let r = worktree::remove_card_worktree(&repo, &base, &a.branch, true);
    assert!(r.removed.is_some() && r.branch_deleted, "{r:?}");
    assert_eq!(worktree::worktree_of(&repo, &a.branch).unwrap(), None);
    assert!(!has_branch(&repo, &a.branch));
    // a branch that doesn't exist says why it stayed
    let r = worktree::remove_card_worktree(&repo, &base, "gizai/nope", true);
    assert!(!r.branch_deleted && r.branch_kept.is_some(), "{r:?}");
}

// ---- Taking over a finished card's worktree (GA-30) ----

fn worktree_list(repo: &std::path::Path) -> String {
    String::from_utf8(Command::new("git").args(["worktree", "list", "--porcelain"]).current_dir(repo).output().unwrap().stdout).unwrap()
}

/// A repository that ignores the folders a warm worktree keeps, with a finished card's worktree that has a commit
/// of its own, build output (target/, node_modules/, .env) and an ignored file of the card that isn't on the keep list.
fn finished_card_worktree(tmp: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf, worktree::Worktree) {
    let repo = repo(tmp);
    commit_file(&repo, ".gitignore", ".env\ntarget/\nnode_modules/\n*.log\n");
    let base = tmp.join("wt");
    let a = worktree::ensure(&repo, &base, "KADE-1", "Old card", "main").unwrap();
    commit_file(&a.path, "old-card.txt", "done\n");
    std::fs::create_dir_all(a.path.join("target/debug")).unwrap();
    std::fs::write(a.path.join("target/debug/warm.rlib"), "compiled").unwrap();
    std::fs::create_dir_all(a.path.join("node_modules/left-pad")).unwrap();
    std::fs::write(a.path.join(".env"), "APP_KEY=1\n").unwrap();
    std::fs::write(a.path.join("debug.log"), "the old card's log\n").unwrap();
    (repo, base, a)
}

#[test]
fn a_new_card_takes_over_a_finished_cards_worktree_on_its_own_clean_branch() {
    let tmp = real_tempdir();
    let (repo, base, a) = finished_card_worktree(tmp.path());
    let old_head = rev(&a.path, "HEAD");
    let keep = [".env".to_string(), "target/".into(), "node_modules/".into()];
    let b = worktree::reuse(&repo, &a.path, &base, "KADE-2", "Next card", "main", &keep).unwrap();

    assert_eq!(b.path, base.join("KADE-2"));
    assert_eq!(b.branch, "gizai/kade-2-next-card");
    assert!(b.created, "a taken-over worktree counts as new for the card: it is prepared");
    assert_eq!(b.reused, Some(worktree::Reused { from: a.path.clone(), head: old_head }));
    assert!(!a.path.exists(), "the finished card's folder moved");
    let list = worktree_list(&repo);
    assert!(list.contains(&format!("worktree {}", git_path(&b.path))) && !list.contains(&format!("worktree {}\n", git_path(&a.path))), "{list}");
    // its own branch, from main: nothing of the finished card is left
    assert_eq!(rev(&b.path, "HEAD"), rev(&repo, "main"));
    assert_eq!(String::from_utf8(Command::new("git").args(["branch", "--show-current"]).current_dir(&b.path).output().unwrap().stdout).unwrap().trim(), b.branch);
    assert!(!b.path.join("old-card.txt").exists(), "the finished card's commit isn't on the new branch");
    assert!(!b.path.join("debug.log").exists(), "ignored files that aren't kept go");
    assert_eq!(worktree::uncommitted(&b.path).unwrap(), 0);
    // the warm folders stay
    assert_eq!(std::fs::read_to_string(b.path.join("target/debug/warm.rlib")).unwrap(), "compiled");
    assert!(b.path.join("node_modules/left-pad").is_dir());
    assert_eq!(std::fs::read_to_string(b.path.join(".env")).unwrap(), "APP_KEY=1\n");
    // the finished card's branch, with its work, is still there
    assert!(has_branch(&repo, &a.branch));
}

#[test]
fn a_card_that_already_has_a_branch_takes_over_on_that_branch() {
    let tmp = real_tempdir();
    let (repo, base, a) = finished_card_worktree(tmp.path());
    git(&repo, &["branch", "gizai/kade-2-next-card", "main"]);
    commit_file(&repo, "main-only.txt", "x\n");
    let b = worktree::reuse(&repo, &a.path, &base, "KADE-2", "Next card", "main", &[]).unwrap();
    assert_eq!(rev(&b.path, "HEAD"), rev(&repo, "gizai/kade-2-next-card"), "its own branch as it was, not main");
    assert!(!b.path.join("target").exists(), "nothing kept when the keep list is empty");
}

#[test]
fn a_worktree_with_uncommitted_work_is_never_taken_over() {
    let tmp = real_tempdir();
    let (repo, base, a) = finished_card_worktree(tmp.path());
    std::fs::write(a.path.join("draft.md"), "unsaved\n").unwrap();
    let r = worktree::reuse(&repo, &a.path, &base, "KADE-2", "Next card", "main", &[]);
    assert!(matches!(&r, Err(gizai_agents::AgentError::Git(m)) if m.contains("uncommitted")), "{r:?}");
    assert!(a.path.join("draft.md").is_file() && !base.join("KADE-2").exists(), "left as it was");
    // nor a folder that isn't a worktree of this repository
    let plain = tmp.path().join("plain");
    std::fs::create_dir(&plain).unwrap();
    assert!(worktree::reuse(&repo, &plain, &base, "KADE-3", "Other", "main", &[]).is_err());
}

#[test]
fn a_failed_switch_puts_the_worktree_back() {
    let tmp = real_tempdir();
    let (repo, base, a) = finished_card_worktree(tmp.path());
    let r = worktree::reuse(&repo, &a.path, &base, "KADE-2", "Next card", "no-such-branch", &[]);
    assert!(r.is_err(), "{r:?}");
    assert!(a.path.join("old-card.txt").is_file() && !base.join("KADE-2").exists());
    assert!(worktree_list(&repo).contains(&format!("worktree {}", git_path(&a.path))));
}

#[test]
fn the_still_to_prepare_note_lives_out_of_git_status_sight() {
    let tmp = real_tempdir();
    let repo = repo(tmp.path());
    let a = worktree::ensure(&repo, &tmp.path().join("wt"), "KADE-1", "Card", "main").unwrap();
    assert_eq!(worktree::unprepared(&a.path), None);
    worktree::mark_unprepared(&a.path, Some("abc123")).unwrap();
    assert_eq!(worktree::unprepared(&a.path), Some(worktree::Unprepared { since: Some("abc123".into()) }));
    assert_eq!(worktree::uncommitted(&a.path).unwrap(), 0);
    worktree::mark_unprepared(&a.path, None).unwrap();
    assert_eq!(worktree::unprepared(&a.path), Some(worktree::Unprepared { since: None }));
    worktree::mark_prepared(&a.path).unwrap();
    assert_eq!(worktree::unprepared(&a.path), None);
    worktree::mark_prepared(&a.path).unwrap();
}

// GA-14: the commits a run made, from the commit it started at to the one it ended at.

fn commit(dir: &std::path::Path, subject: &str) {
    git(dir, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", subject]);
}

#[test]
fn the_commits_between_two_ids_are_listed_oldest_first_with_their_subjects() {
    let tmp = real_tempdir();
    let repo = repo(tmp.path());
    let wt = worktree::ensure(&repo, &tmp.path().join("wt"), "KADE-14", "Record where a run ended", "main").unwrap();
    let base = rev(&wt.path, "HEAD");
    commit(&wt.path, "First change");
    commit(&wt.path, "Second change\n\nWith a body that is not its subject.");
    let head = rev(&wt.path, "HEAD");
    let list = worktree::commits(&wt.path, &base, &head).unwrap();
    assert_eq!(list.iter().map(|c| c.subject.as_str()).collect::<Vec<_>>(), ["First change", "Second change"]);
    assert_eq!(list[1].sha, head, "full commit ids");
    assert_eq!(list[0].sha, rev(&wt.path, "HEAD~1"));
    assert_eq!(worktree::commits(&repo, &base, &head).unwrap(), list, "the main checkout shares the worktree's commits");
    assert!(worktree::commits(&wt.path, &head, &head).unwrap().is_empty(), "a run that didn't commit");
}

#[test]
fn merging_main_in_counts_as_one_commit_not_all_of_mains() {
    let tmp = real_tempdir();
    let repo = repo(tmp.path());
    let wt = worktree::ensure(&repo, &tmp.path().join("wt"), "KADE-15", "Merge main", "main").unwrap();
    let base = rev(&wt.path, "HEAD");
    commit(&wt.path, "Own change");
    commit(&repo, "On main 1");
    commit(&repo, "On main 2");
    git(&wt.path, &["-c", "user.email=t@t", "-c", "user.name=t", "merge", "-q", "--no-ff", "-m", "Merge main", "main"]);
    let list = worktree::commits(&wt.path, &base, &rev(&wt.path, "HEAD")).unwrap();
    assert_eq!(list.iter().map(|c| c.subject.as_str()).collect::<Vec<_>>(), ["Own change", "Merge main"]);
}

#[test]
fn only_commit_ids_are_read_never_options_or_names() {
    let tmp = real_tempdir();
    let repo = repo(tmp.path());
    let head = rev(&repo, "HEAD");
    for bad in ["", "HEAD", "main", "--all", "-p", "abc def", "../x"] {
        assert!(matches!(worktree::commits(&repo, bad, &head), Err(gizai_agents::AgentError::Git(_))), "base {bad:?}");
        assert!(matches!(worktree::commits(&repo, &head, bad), Err(gizai_agents::AgentError::Git(_))), "head {bad:?}");
    }
    assert!(worktree::commits(&repo, &head, "0123456789abcdef0123456789abcdef01234567").is_err(), "an unknown commit is an error");
}

// GA-48: the run's temp folder at the worktree root.

fn status(dir: &std::path::Path) -> String {
    String::from_utf8(Command::new("git").args(["status", "--porcelain", "--untracked-files=all"]).current_dir(dir).output().unwrap().stdout).unwrap()
}

fn exclude_of(repo: &std::path::Path) -> String {
    std::fs::read_to_string(repo.join(".git/info/exclude")).unwrap_or_default()
}

#[test]
fn the_temp_folder_is_made_empty_and_ignored_by_every_checkout_without_touching_gitignore() {
    let tmp = real_tempdir();
    let repo = repo(tmp.path());
    std::fs::write(repo.join(".gitignore"), "target/\n").unwrap();
    git(&repo, &["add", ".gitignore"]);
    git(&repo, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "ignore"]);
    let base = tmp.path().join("wt");
    let a = worktree::ensure(&repo, &base, "KADE-1", "One", "main").unwrap();
    let b = worktree::ensure(&repo, &base, "KADE-2", "Two", "main").unwrap();

    let dir = worktree::prepare_temp(&a.path).unwrap();
    assert_eq!(dir, a.path.join(".gizai-tmp"));
    assert!(dir.is_dir());
    std::fs::create_dir_all(dir.join("deep/er")).unwrap();
    std::fs::write(dir.join("deep/er/x.db"), "x").unwrap();
    assert_eq!(status(&a.path), "", "the worktree's git status doesn't show it");
    // again, from another worktree of the same repository: one line in the shared info/exclude
    worktree::prepare_temp(&b.path).unwrap();
    worktree::prepare_temp(&a.path).unwrap();
    assert_eq!(exclude_of(&repo).lines().filter(|l| *l == "/.gizai-tmp/").count(), 1, "{}", exclude_of(&repo));
    assert!(std::fs::read_dir(&dir).unwrap().next().is_none(), "a run starts with it empty");
    assert_eq!(std::fs::read_to_string(a.path.join(".gitignore")).unwrap(), "target/\n");
    assert_eq!(std::fs::read_to_string(repo.join(".gitignore")).unwrap(), "target/\n", ".gitignore untouched");
    // the main checkout ignores one of its own as well
    std::fs::create_dir_all(repo.join(".gizai-tmp")).unwrap();
    std::fs::write(repo.join(".gizai-tmp/y"), "y").unwrap();
    for d in [&repo, &a.path, &b.path] {
        assert_eq!(status(d), "", "{}", d.display());
    }
    // a file in a subfolder named like it is not hidden: the line only matches the root
    std::fs::create_dir_all(a.path.join("src/.gizai-tmp")).unwrap();
    std::fs::write(a.path.join("src/.gizai-tmp/z"), "z").unwrap();
    assert!(status(&a.path).contains("src/.gizai-tmp/z"), "{}", status(&a.path));
}

#[test]
fn the_exclude_line_goes_on_a_line_of_its_own_and_only_once() {
    let tmp = real_tempdir();
    let common = tmp.path().join("git");
    // no info folder yet
    worktree::exclude_temp(&common).unwrap();
    assert_eq!(std::fs::read_to_string(common.join("info/exclude")).unwrap(), "/.gizai-tmp/\n");
    // a file without a newline at its end
    std::fs::write(common.join("info/exclude"), "*.swp").unwrap();
    worktree::exclude_temp(&common).unwrap();
    worktree::exclude_temp(&common).unwrap();
    assert_eq!(std::fs::read_to_string(common.join("info/exclude")).unwrap(), "*.swp\n/.gizai-tmp/\n");
    // a line someone wrote with spaces around it counts
    std::fs::write(common.join("info/exclude"), "  /.gizai-tmp/  \n").unwrap();
    worktree::exclude_temp(&common).unwrap();
    assert_eq!(std::fs::read_to_string(common.join("info/exclude")).unwrap(), "  /.gizai-tmp/  \n");
}

// Linux and macOS only: Unix symlinks (a symlink on Windows needs Developer Mode or an administrator).
#[cfg(unix)]
#[test]
fn emptying_the_temp_folder_removes_links_and_never_follows_them() {
    let tmp = real_tempdir();
    let repo = repo(tmp.path());
    let wt = worktree::ensure(&repo, &tmp.path().join("wt"), "KADE-3", "Three", "main").unwrap().path;
    // nothing to empty yet
    worktree::empty_temp(&wt).unwrap();
    let dir = worktree::prepare_temp(&wt).unwrap();
    let outside = tmp.path().join("outside");
    std::fs::create_dir_all(outside.join("sub")).unwrap();
    std::fs::write(outside.join("sub/keep.txt"), "mine").unwrap();
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    std::fs::create_dir_all(dir.join("b/c")).unwrap();
    std::fs::write(dir.join("b/c/d.txt"), "d").unwrap();
    std::os::unix::fs::symlink(&outside, dir.join("link-to-dir")).unwrap();
    std::os::unix::fs::symlink(outside.join("sub/keep.txt"), dir.join("link-to-file")).unwrap();
    worktree::empty_temp(&wt).unwrap();
    assert!(dir.is_dir() && std::fs::read_dir(&dir).unwrap().next().is_none(), "emptied, the folder stays");
    assert_eq!(std::fs::read_to_string(outside.join("sub/keep.txt")).unwrap(), "mine", "links aren't followed");

    // a link in the folder's place: prepare removes the link and makes a real folder
    std::fs::remove_dir(&dir).unwrap();
    std::os::unix::fs::symlink(&outside, &dir).unwrap();
    let made = worktree::prepare_temp(&wt).unwrap();
    assert!(!std::fs::symlink_metadata(&made).unwrap().file_type().is_symlink() && made.is_dir());
    assert!(outside.join("sub/keep.txt").is_file(), "the link's target is left alone");
    // and emptying removes one (a file too) without following it
    std::fs::remove_dir(&dir).unwrap();
    std::os::unix::fs::symlink(&outside, &dir).unwrap();
    worktree::empty_temp(&wt).unwrap();
    assert!(std::fs::symlink_metadata(&dir).is_err() && outside.join("sub/keep.txt").is_file());
}

#[test]
fn the_temp_folder_goes_with_the_worktree() {
    let tmp = real_tempdir();
    let repo = repo(tmp.path());
    let base = tmp.path().join("wt");
    let w = worktree::ensure(&repo, &base, "KADE-4", "Four", "main").unwrap();
    let dir = worktree::prepare_temp(&w.path).unwrap();
    std::fs::create_dir_all(dir.join("left/over")).unwrap();
    std::fs::write(dir.join("left/over/db.sqlite"), "x").unwrap();
    // never by force: the folder is ignored, not untracked, so git removes the worktree with it
    let out = worktree::remove_card_worktree(&repo, &base, &w.branch, false);
    assert_eq!(out.removed.as_deref(), Some(w.path.as_path()), "{:?}", out.kept);
    assert!(!w.path.exists() && !dir.exists());
}

#[test]
fn the_temp_environment_is_tmpdir_tmp_and_temp() {
    let env = worktree::temp_env(std::path::Path::new("/w/KADE-5/.gizai-tmp"));
    assert_eq!(env, [("TMPDIR", "/w/KADE-5/.gizai-tmp"), ("TMP", "/w/KADE-5/.gizai-tmp"), ("TEMP", "/w/KADE-5/.gizai-tmp")]
        .map(|(k, v)| (k.to_string(), v.to_string())));
}

// ---- GA-138: a finished worktree's uncommitted files (Settings → Data), and Remove anyway's stash ----

fn git_out(dir: &std::path::Path, args: &[&str]) -> String {
    String::from_utf8(Command::new("git").args(args).current_dir(dir).output().unwrap().stdout).unwrap().trim().to_string()
}

fn commit_all(dir: &std::path::Path, msg: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", msg]);
}

#[test]
fn git_status_lines_are_read_as_new_changed_or_deleted_files() {
    use worktree::{FileChangeKind::*, UncommittedFile};
    let f = |path: &str, change| UncommittedFile { path: path.into(), change };
    // as `git status --porcelain -z` prints them: no quotes around a path, and a rename's or copy's old path as one more field
    let out = "?? notes/\0 M src/app.rs\0 D old.txt\0A  added.rs\0R  new name.rs\0old name.rs\0AM staged then changed.rs\0\
               UU conflict.rs\0MD gone.rs\0 T link\0C  copy.rs\0orig.rs\0?? ünï cödé.md\0";
    assert_eq!(worktree::parse_status(out), [
        f("notes/", New), f("src/app.rs", Changed), f("old.txt", Deleted), f("added.rs", New), f("new name.rs", Changed),
        f("staged then changed.rs", New), f("conflict.rs", Changed), f("gone.rs", Deleted), f("link", Changed),
        f("copy.rs", Changed), f("ünï cödé.md", New),
    ]);
    assert_eq!(worktree::parse_status(""), []);
    // as the app reads them
    assert_eq!(serde_json::to_value(f("a.rs", Deleted)).unwrap(), serde_json::json!({"path": "a.rs", "change": "deleted"}));
}

#[test]
fn a_checkouts_uncommitted_files_come_from_git_status_as_many_as_it_counts() {
    let tmp = real_tempdir();
    let repo = repo(tmp.path());
    for (name, text) in [("keep.txt", "a\n"), ("gone.txt", "b\n"), ("old name.txt", "c\n")] {
        std::fs::write(repo.join(name), text).unwrap();
    }
    commit_all(&repo, "files");
    let w = worktree::ensure(&repo, &tmp.path().join("wt"), "KADE-6", "Six", "main").unwrap();
    assert_eq!(worktree::uncommitted_files(&w.path).unwrap(), []);

    std::fs::write(w.path.join("keep.txt"), "changed\n").unwrap();
    std::fs::remove_file(w.path.join("gone.txt")).unwrap();
    git(&w.path, &["mv", "old name.txt", "new name.txt"]);
    std::fs::write(w.path.join("ünï cödé.md"), "x\n").unwrap();
    std::fs::create_dir_all(w.path.join("drafts/deep")).unwrap();
    std::fs::write(w.path.join("drafts/deep/a.md"), "x\n").unwrap();
    std::fs::write(w.path.join("drafts/b.md"), "x\n").unwrap();
    let files = worktree::uncommitted_files(&w.path).unwrap();
    let got: Vec<(&str, worktree::FileChangeKind)> = files.iter().map(|f| (f.path.as_str(), f.change)).collect();
    use worktree::FileChangeKind::*;
    assert_eq!(got, [("gone.txt", Deleted), ("keep.txt", Changed), ("new name.txt", Changed), ("drafts/", New), ("ünï cödé.md", New)],
               "git status's order, a renamed file by its new name, a new folder as one entry and a path as it is");
    assert_eq!(files.len(), worktree::uncommitted(&w.path).unwrap(), "one per line uncommitted counts");
}

#[test]
fn stash_all_puts_every_uncommitted_change_in_the_repositorys_stash_with_its_message() {
    let tmp = real_tempdir();
    let repo = repo(tmp.path());
    std::fs::write(repo.join("app.rs"), "fn main() {}\n").unwrap();
    commit_all(&repo, "app");
    let w = worktree::ensure(&repo, &tmp.path().join("wt"), "KADE-7", "Seven", "main").unwrap();
    std::fs::write(w.path.join("app.rs"), "fn main() { todo!() }\n").unwrap();
    std::fs::write(w.path.join("draft.md"), "unsaved\n").unwrap();

    worktree::stash_all(&w.path, "KADE-7 leftover, removed in Settings").unwrap();
    assert_eq!(worktree::uncommitted(&w.path).unwrap(), 0, "nothing uncommitted is left");
    assert_eq!(git_out(&repo, &["stash", "list", "--format=%gs"]), format!("On {}: KADE-7 leftover, removed in Settings", w.branch),
               "the main checkout's git stash list shows it");
    assert_eq!(git_out(&repo, &["show", "stash@{0}:app.rs"]), "fn main() { todo!() }");
    assert_eq!(git_out(&repo, &["show", "stash@{0}^3:draft.md"]), "unsaved", "the untracked file too");

    // nothing left to stash: nothing happens
    worktree::stash_all(&w.path, "KADE-7 again").unwrap();
    assert_eq!(git_out(&repo, &["stash", "list"]).lines().count(), 1);
}

#[test]
fn a_stash_git_refuses_says_why_in_one_line_and_changes_nothing() {
    let tmp = real_tempdir();
    let repo = repo(tmp.path());
    let w = worktree::ensure(&repo, &tmp.path().join("wt"), "KADE-8", "Eight", "main").unwrap();
    std::fs::write(w.path.join("draft.md"), "unsaved\n").unwrap();
    std::fs::write(repo.join(".git/refs/stash.lock"), "").unwrap();

    let err = worktree::stash_all(&w.path, "KADE-8 leftover, removed in Settings").unwrap_err();
    let gizai_agents::AgentError::Git(why) = &err else { panic!("{err:?}") };
    // git 2.55 says only "Cannot save the current status" here; older ones name the lock first
    assert!((why == "Cannot save the current status" || why.contains("stash.lock")) && !why.contains('\n'), "git's own line: {why}");
    assert_eq!(std::fs::read_to_string(w.path.join("draft.md")).unwrap(), "unsaved\n");
    assert_eq!(git_out(&repo, &["stash", "list"]), "");
    assert!(matches!(worktree::stash_all(&tmp.path().join("gone"), "x"), Err(gizai_agents::AgentError::Git(_))),
            "a folder that isn't there is a git error too");
}

/// A temp folder by its real path, the way git and Gizai report it: on macOS /var/folders is /private/var/folders, and on
/// Windows TEMP can be a short name (RUNNER~1) that git gives in full. Without the \\?\ that canonicalize puts before a
/// Windows drive.
fn real_tempdir() -> tempfile::TempDir {
    let base = std::env::temp_dir().canonicalize().unwrap();
    let base = std::path::PathBuf::from(base.to_string_lossy().trim_start_matches(r"\\?\").to_string());
    tempfile::tempdir_in(base).unwrap()
}

/// A path as `git worktree list` prints it: with / between folders, also on Windows.
fn git_path(p: &std::path::Path) -> String {
    p.display().to_string().replace('\\', "/")
}

// ---- Cards of one repository that start together (GA-242) ----

/// How long one test may take before it counts as a deadlock: far more than a slow runner needs.
const HANG: std::time::Duration = std::time::Duration::from_secs(300);

/// Runs a test's `body` on a thread of its own, so a lock taken twice fails the test instead of hanging it.
fn no_hang(body: impl FnOnce() + Send + 'static) {
    let (done, finished) = std::sync::mpsc::channel();
    let test = std::thread::spawn(move || { body(); let _ = done.send(()); });
    if let Err(std::sync::mpsc::RecvTimeoutError::Timeout) = finished.recv_timeout(HANG) {
        panic!("not done after {HANG:?}: a deadlock");
    }
    if let Err(panicked) = test.join() { std::panic::resume_unwind(panicked); }
}

/// Runs `job` for 0..n at one moment, each on a thread of its own, and gives their results in that order.
fn together<T: Send>(n: usize, job: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let start = std::sync::Barrier::new(n);
    let (start, job) = (&start, &job);
    std::thread::scope(|s| {
        let threads: Vec<_> = (0..n).map(|i| s.spawn(move || { start.wait(); job(i) })).collect();
        threads.into_iter().map(|t| t.join().unwrap()).collect()
    })
}

/// The card's own test. Without the lock it fails only now and then on a fast disk (`failed to read
/// .git/worktrees/…/commondir`): the tests in `one_at_a_time` fail every time.
#[test]
fn cards_of_one_repository_that_start_at_the_same_moment_each_get_their_own_worktree_and_branch() {
    const CARDS: usize = 8;
    const ROUNDS: usize = 20;
    no_hang(|| {
        let tmp = real_tempdir();
        let repo = repo(tmp.path());
        let base = tmp.path().join("wt");
        let (mut paths, mut branches) = (std::collections::HashSet::new(), std::collections::HashSet::new());
        for round in 0..ROUNDS {
            let started = together(CARDS, |card| worktree::ensure(&repo, &base, &format!("CARD-{round}-{card}"), "Side by side", "main"));
            for (card, w) in started.into_iter().enumerate() {
                let w = w.unwrap_or_else(|e| panic!("round {round}, card {card}: {e}"));
                assert!(w.created && w.path.join(".git").exists(), "round {round}, card {card}: {w:?}");
                assert!(paths.insert(w.path.clone()) && branches.insert(w.branch.clone()), "taken already: {w:?}");
            }
        }
        // git says the same
        let list = worktree_list(&repo);
        assert_eq!(list.lines().filter(|l| l.starts_with("worktree ")).count(), CARDS * ROUNDS + 1, "{list}");
        assert!(paths.iter().all(|p| list.contains(&format!("worktree {}\n", git_path(p)))), "{list}");
        assert!(branches.iter().all(|b| list.contains(&format!("branch refs/heads/{b}\n"))), "{list}");
    });
}

#[test]
fn one_card_started_from_several_threads_gets_one_worktree() {
    no_hang(|| {
        let tmp = real_tempdir();
        let repo = repo(tmp.path());
        let base = tmp.path().join("wt");
        let start = || together(8, |_| worktree::ensure(&repo, &base, "KADE-1", "One card", "main").unwrap());
        // a new one: one thread makes it and the others find it, so none of them decides on what it read before the add
        let first = start();
        assert_eq!(first.iter().filter(|w| w.created).count(), 1, "{first:?}");
        // an existing one
        let again = start();
        assert!(again.iter().all(|w| !w.created), "{again:?}");
        assert!(first.iter().chain(&again).all(|w| w.path == base.join("KADE-1") && w.branch == "gizai/kade-1-one-card"), "{first:?} {again:?}");
    });
}

#[test]
fn one_card_takes_over_a_finished_worktree_and_the_others_get_their_own() {
    no_hang(|| {
        let tmp = real_tempdir();
        let (repo, base, a) = finished_card_worktree(tmp.path());
        // as a card opens its worktree (`open_worktree_now`): take one over, or else make one
        let cards = together(8, |i| {
            let id = format!("CARD-{i}");
            worktree::reuse(&repo, &a.path, &base, &id, "Side by side", "main", &[]).unwrap_or_else(|e| {
                assert!(matches!(&e, gizai_agents::AgentError::Git(m) if m.contains("is not a worktree of this repository")), "{e:?}");
                worktree::ensure(&repo, &base, &id, "Side by side", "main").unwrap()
            })
        });
        assert_eq!(cards.iter().filter(|w| w.reused.is_some()).count(), 1, "{cards:?}");
        for (i, w) in cards.iter().enumerate() {
            assert_eq!(w.path, base.join(format!("CARD-{i}")));
            assert_eq!(w.branch, format!("gizai/card-{i}-side-by-side"));
            assert_eq!(rev(&w.path, "HEAD"), rev(&repo, "main"));
        }
        assert!(!a.path.exists(), "the finished card's folder moved");
    });
}

#[test]
fn a_failed_take_over_from_several_threads_puts_the_worktree_back_each_time() {
    no_hang(|| {
        let tmp = real_tempdir();
        let (repo, base, a) = finished_card_worktree(tmp.path());
        let failed = together(8, |i| worktree::reuse(&repo, &a.path, &base, &format!("CARD-{i}"), "Side by side", "no-such-branch", &[]));
        for (i, r) in failed.iter().enumerate() {
            // git's refusal of the switch: each found the worktree in its place, none found it moved away by another
            assert!(matches!(r, Err(gizai_agents::AgentError::Git(m)) if m.contains("no-such-branch")), "{r:?}");
            assert!(!base.join(format!("CARD-{i}")).exists());
        }
        assert!(a.path.join("old-card.txt").is_file());
        assert!(worktree_list(&repo).contains(&format!("worktree {}\n", git_path(&a.path))));
        // and the next ones go ahead
        assert!(worktree::reuse(&repo, &a.path, &base, "CARD-9", "Side by side", "main", &[]).unwrap().reused.is_some());
        assert!(worktree::ensure(&repo, &base, "CARD-10", "Side by side", "main").unwrap().created);
    });
}

#[test]
fn a_refused_command_leaves_the_repository_free_for_the_next() {
    no_hang(|| {
        let tmp = real_tempdir();
        let (repo, base, a) = finished_card_worktree(tmp.path());
        let plain = tmp.path().join("plain");
        std::fs::create_dir(&plain).unwrap();
        std::fs::create_dir_all(base.join("TAKEN")).unwrap();
        std::fs::write(a.path.join("draft.md"), "unsaved\n").unwrap();
        let refused = [
            ("a folder that isn't a worktree", worktree::ensure(&repo, &base, "TAKEN", "t", "main").map(drop)),
            ("no such start", worktree::ensure(&repo, &base, "X-1", "t", "develop").map(drop)),
            ("a folder in the way", worktree::reuse(&repo, &a.path, &base, "TAKEN", "t", "main", &[]).map(drop)),
            ("uncommitted work", worktree::reuse(&repo, &a.path, &base, "X-2", "t", "main", &[]).map(drop)),
            ("not one of its worktrees", worktree::reuse(&repo, &plain, &base, "X-3", "t", "main", &[]).map(drop)),
            ("removing a plain folder", worktree::remove_worktree(&repo, &plain)),
            ("removing uncommitted work", worktree::remove_worktree(&repo, &a.path)),
            ("listing a folder without git", worktree::worktree_of(&plain, "x").map(drop)),
            ("listing a folder that isn't there", worktree::worktree_of(&tmp.path().join("gone"), "x").map(drop)),
        ];
        for (what, r) in refused {
            assert!(r.is_err(), "{what}");
        }
        assert_eq!(worktree::worktree_of(&repo, &a.branch).unwrap().map(|p| git_path(&p)), Some(git_path(&a.path)));
        assert!(worktree::ensure(&repo, &base, "X-4", "t", "main").unwrap().created);
    });
}

#[test]
fn every_worktree_command_at_once_on_one_repository() {
    use gizai_agents::copies;
    no_hang(|| {
        let tmp = real_tempdir();
        let repo = repo(tmp.path());
        let (base, code) = (tmp.path().join("wt"), tmp.path().join("code"));
        std::fs::create_dir(&code).unwrap();
        for round in 0..20 {
            let to_take = worktree::ensure(&repo, &base, &format!("TAKE-{round}"), "t", "main").unwrap();
            let to_remove = worktree::ensure(&repo, &base, &format!("GO-{round}"), "t", "main").unwrap();
            let old_copy = code.join(format!("OLD-{round}"));
            copies::sync(&repo, &old_copy, "main").unwrap();
            // deleted by hand: still in git's list, so its add fails and `sync` prunes
            let deleted_copy = code.join(format!("DELETED-{round}"));
            copies::sync(&repo, &deleted_copy, "main").unwrap();
            std::fs::remove_dir_all(&deleted_copy).unwrap();
            let done = together(9, |job| match job {
                0 | 1 => worktree::ensure(&repo, &base, &format!("NEW-{round}-{job}"), "t", "main").map(drop),
                2 => worktree::worktree_of(&repo, "x").map(drop),
                3 => worktree::checked_out(&repo).map(drop),
                4 => worktree::reuse(&repo, &to_take.path, &base, &format!("TOOK-{round}"), "t", "main", &[]).map(drop),
                5 => worktree::remove_worktree(&repo, &to_remove.path),
                6 => copies::sync(&repo, &code.join(format!("NEW-{round}")), "main").map(drop),
                7 => copies::sync(&repo, &deleted_copy, "main").map(drop),
                _ => copies::remove(&old_copy, &code),
            });
            for (job, r) in done.into_iter().enumerate() {
                r.unwrap_or_else(|e| panic!("round {round}, job {job}: {e}"));
            }
            assert!(!to_remove.path.exists() && !to_take.path.exists() && !old_copy.exists() && deleted_copy.join(".git").exists(), "round {round}");
        }
    });
}

/// These tests hold the lock open with a shell script that git runs, so they fail every time without it, on any disk.
/// While the script waits, what takes the lock can't finish, however slow the machine is.
#[cfg(unix)]
mod one_at_a_time {
    use super::*;
    use gizai_agents::copies;
    use gizai_agents::prepare::{self, Prepare};
    use std::path::Path;
    use std::thread::ScopedJoinHandle;
    use std::time::{Duration, Instant};

    /// How long a command that should wait for its turn gets to show it doesn't.
    const PAUSE: Duration = Duration::from_millis(500);
    /// How long a command that should go ahead may take. Shorter than the gate holds: it fails, it doesn't pass late.
    const SOON: Duration = Duration::from_secs(60);

    /// Writes an executable script through a child sh, so a fork elsewhere in the test process never holds it open
    /// for writing ("Text file busy").
    fn write_script(path: &Path, script: &str) {
        use std::io::Write;
        let mut sh = Command::new("sh").args(["-c", r#"cat > "$1" && chmod 755 "$1""#, "sh"]).arg(path)
            .stdin(std::process::Stdio::piped()).spawn().unwrap();
        sh.stdin.take().unwrap().write_all(script.as_bytes()).unwrap();
        assert!(sh.wait().unwrap().success(), "can't write {}", path.display());
    }

    /// Shell lines that hold up what runs them until the test opens the gate (at most 2 minutes), and files to say so.
    struct Gate(std::path::PathBuf);

    impl Gate {
        fn new(tmp: &Path) -> Gate {
            let dir = tmp.join("gate");
            std::fs::create_dir(&dir).unwrap();
            Gate(dir)
        }

        fn lines(&self) -> String {
            let d = self.0.display();
            format!("touch '{d}/held'\ni=0\nwhile [ ! -e '{d}/go' ] && [ $i -lt 2400 ]; do sleep 0.05; i=$((i+1)); done\n")
        }

        /// In the post-checkout hook, which `git worktree add` and a branch switch run: the last thing they do.
        fn on_checkout(tmp: &Path, repo: &Path) -> Gate {
            let gate = Gate::new(tmp);
            write_hook(repo, &gate.lines());
            gate
        }

        /// In the `git clean` of a take-over to the folder `wt`. Git runs the `core.fsmonitor` program when a command reads
        /// the index, also a clean; the switch before it runs the post-checkout hook, and the clean is the next one there.
        fn on_clean(tmp: &Path, repo: &Path, wt: &Path) -> Gate {
            let gate = Gate::new(tmp);
            let (switched, monitor) = (gate.0.join("switched"), gate.0.join("fsmonitor"));
            write_hook(repo, &format!("touch '{}'\n", switched.display()));
            write_script(&monitor, &format!("#!/bin/sh\nif [ -e '{}' ] && [ \"$(pwd -P)\" = '{}' ]; then\n{}fi\nexit 1\n",
                                            switched.display(), wt.display(), gate.lines()));
            git(repo, &["config", "core.fsmonitor", monitor.to_str().unwrap()]);
            gate
        }

        fn wait_until_held(&self) {
            let since = Instant::now();
            while !self.0.join("held").exists() {
                assert!(since.elapsed() < SOON, "nothing reached the gate");
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        fn open(&self) {
            std::fs::write(self.0.join("go"), "").unwrap();
        }
    }

    impl Drop for Gate {
        fn drop(&mut self) {
            let _ = std::fs::write(self.0.join("go"), "");
        }
    }

    fn write_hook(repo: &Path, lines: &str) {
        std::fs::create_dir_all(repo.join(".git/hooks")).unwrap();
        write_script(&repo.join(".git/hooks/post-checkout"), &format!("#!/bin/sh\n{lines}"));
    }

    fn finishes_soon<T>(thread: &ScopedJoinHandle<T>) -> bool {
        let since = Instant::now();
        while !thread.is_finished() && since.elapsed() < SOON {
            std::thread::sleep(Duration::from_millis(10));
        }
        thread.is_finished()
    }

    #[test]
    fn worktree_adds_and_branch_switches_of_one_repository_never_overlap() {
        no_hang(|| {
            let tmp = real_tempdir();
            let (repo, base, _) = finished_card_worktree(tmp.path());
            let finished: Vec<_> = (0..4).map(|i| worktree::ensure(&repo, &base, &format!("DONE-{i}"), "t", "main").unwrap().path).collect();
            let log = tmp.path().join("turns");
            write_hook(&repo, &format!("echo start >> '{l}'\nsleep 0.05\necho end >> '{l}'\n", l = log.display()));
            let started = together(8, |i| match finished.get(i) {
                Some(from) => worktree::reuse(&repo, from, &base, &format!("TOOK-{i}"), "t", "main", &[]),
                None => worktree::ensure(&repo, &base, &format!("NEW-{i}"), "t", "main"),
            });
            for s in started {
                s.unwrap();
            }
            assert_eq!(std::fs::read_to_string(&log).unwrap(), "start\nend\n".repeat(8));
        });
    }

    #[test]
    fn every_worktree_command_waits_for_the_add_that_is_running_by_any_spelling_of_the_repository() {
        no_hang(|| {
            let tmp = real_tempdir();
            let (repo, base, finished) = finished_card_worktree(tmp.path());
            let card = worktree::ensure(&repo, &base, "KADE-2", "To remove", "main").unwrap();
            let other = worktree::ensure(&repo, &base, "KADE-3", "Stays", "main").unwrap();
            let code = tmp.path().join("code");
            std::fs::create_dir(&code).unwrap();
            let copy = code.join("OLD");
            copies::sync(&repo, &copy, "main").unwrap();
            let (dots, link) = (repo.join("..").join("repo"), tmp.path().join("link"));
            std::os::unix::fs::symlink(&repo, &link).unwrap();
            let gate = Gate::on_checkout(tmp.path(), &repo);
            std::thread::scope(|s| {
                let holder = s.spawn(|| worktree::ensure(&repo, &base, "HOLD-1", "Holds the lock", "main"));
                gate.wait_until_held();
                let waiting: Vec<(&str, ScopedJoinHandle<Result<(), gizai_agents::AgentError>>)> = vec![
                    ("worktree_of", s.spawn(|| worktree::worktree_of(&repo, "x").map(drop))),
                    ("checked_out", s.spawn(|| worktree::checked_out(&repo).map(drop))),
                    ("ensure", s.spawn(|| worktree::ensure(&repo, &base, "KADE-4", "t", "main").map(drop))),
                    ("reuse", s.spawn(|| worktree::reuse(&repo, &finished.path, &base, "KADE-5", "t", "main", &[]).map(drop))),
                    ("remove_worktree", s.spawn(|| worktree::remove_worktree(&repo, &card.path))),
                    ("copies::sync", s.spawn(|| copies::sync(&repo, &code.join("NEW"), "main").map(drop))),
                    // lists from the copy's own folder, so this is one of the repository's worktrees asking too
                    ("copies::remove", s.spawn(|| copies::remove(&copy, &code))),
                    ("through ..", s.spawn(|| worktree::worktree_of(&dots, "x").map(drop))),
                    ("through a link", s.spawn(|| worktree::worktree_of(&link, "x").map(drop))),
                    ("from a worktree", s.spawn(|| worktree::worktree_of(&other.path, "x").map(drop))),
                ];
                std::thread::sleep(PAUSE);
                let early: Vec<&str> = waiting.iter().filter(|(_, t)| t.is_finished()).map(|(what, _)| *what).collect();
                let holder_early = holder.is_finished();
                gate.open();
                holder.join().unwrap().unwrap();
                for (what, t) in waiting {
                    t.join().unwrap().unwrap_or_else(|e| panic!("{what}: {e}"));
                }
                assert!(early.is_empty() && !holder_early, "didn't wait for another card's worktree add: {early:?}");
            });
        });
    }

    #[test]
    fn a_take_overs_branch_switch_holds_the_lock() {
        no_hang(|| {
            let tmp = real_tempdir();
            let (repo, base, a) = finished_card_worktree(tmp.path());
            let gate = Gate::on_checkout(tmp.path(), &repo);
            std::thread::scope(|s| {
                let holder = s.spawn(|| worktree::reuse(&repo, &a.path, &base, "KADE-2", "Next card", "main", &[]));
                gate.wait_until_held();
                let list = s.spawn(|| worktree::worktree_of(&repo, "x"));
                std::thread::sleep(PAUSE);
                let early = list.is_finished();
                gate.open();
                assert!(holder.join().unwrap().unwrap().reused.is_some());
                list.join().unwrap().unwrap();
                assert!(!early, "a list went ahead during a take-over's branch switch");
            });
        });
    }

    #[test]
    fn a_copys_worktree_add_holds_the_lock() {
        no_hang(|| {
            let tmp = real_tempdir();
            let repo = repo(tmp.path());
            let gate = Gate::on_checkout(tmp.path(), &repo);
            std::thread::scope(|s| {
                let holder = s.spawn(|| copies::sync(&repo, &tmp.path().join("code").join("NEW"), "main"));
                gate.wait_until_held();
                let list = s.spawn(|| worktree::worktree_of(&repo, "x"));
                std::thread::sleep(PAUSE);
                let early = list.is_finished();
                gate.open();
                holder.join().unwrap().unwrap();
                list.join().unwrap().unwrap();
                assert!(!early, "a list went ahead during a copy's worktree add");
            });
        });
    }

    #[test]
    fn cards_of_another_repository_do_not_wait() {
        no_hang(|| {
            let tmp = real_tempdir();
            let (one, two) = (tmp.path().join("one"), tmp.path().join("two"));
            std::fs::create_dir(&one).unwrap();
            std::fs::create_dir(&two).unwrap();
            let busy = repo(&one);
            let (free, base, finished) = finished_card_worktree(&two);
            let card = worktree::ensure(&free, &base, "KADE-2", "To remove", "main").unwrap();
            let code = two.join("code");
            std::fs::create_dir(&code).unwrap();
            let copy = code.join("OLD");
            copies::sync(&free, &copy, "main").unwrap();
            let gate = Gate::on_checkout(&one, &busy);
            std::thread::scope(|s| {
                let holder = s.spawn(|| worktree::ensure(&busy, &one.join("wt"), "HOLD-1", "Holds the lock", "main"));
                gate.wait_until_held();
                let others = s.spawn(|| {
                    assert!(worktree::ensure(&free, &base, "KADE-3", "t", "main").unwrap().created);
                    assert!(worktree::reuse(&free, &finished.path, &base, "KADE-4", "t", "main", &[]).unwrap().reused.is_some());
                    worktree::checked_out(&free).unwrap();
                    worktree::worktree_of(&free, "x").unwrap();
                    worktree::remove_worktree(&free, &card.path).unwrap();
                    copies::sync(&free, &code.join("NEW"), "main").unwrap();
                    copies::remove(&copy, &code).unwrap();
                });
                let finished_first = finishes_soon(&others);
                let holder_early = holder.is_finished();
                gate.open();
                holder.join().unwrap().unwrap();
                others.join().unwrap();
                assert!(finished_first, "a card of another repository waited for this one's worktree add");
                assert!(!holder_early, "the worktree add didn't wait for the gate");
            });
        });
    }

    #[test]
    fn a_take_overs_git_clean_runs_outside_the_lock() {
        no_hang(|| {
            let tmp = real_tempdir();
            let (repo, base, a) = finished_card_worktree(tmp.path());
            let wt = base.join("KADE-2");
            let gate = Gate::on_clean(tmp.path(), &repo, &wt);
            std::thread::scope(|s| {
                let taking = s.spawn(|| worktree::reuse(&repo, &a.path, &base, "KADE-2", "Next card", "main", &[]));
                gate.wait_until_held();
                let others = s.spawn(|| {
                    assert!(worktree::ensure(&repo, &base, "KADE-3", "t", "main").unwrap().created);
                    worktree::worktree_of(&repo, "x").unwrap();
                });
                let finished_first = finishes_soon(&others);
                // the clean hadn't run yet: the last card's ignored files are still there
                let log_still_there = wt.join("debug.log").is_file();
                gate.open();
                assert!(taking.join().unwrap().unwrap().reused.is_some());
                others.join().unwrap();
                assert!(finished_first, "another card waited for a take-over's git clean");
                assert!(log_still_there && !wt.join("debug.log").exists(), "the gate didn't hold up the clean");
            });
        });
    }

    #[test]
    fn a_worktrees_preparation_runs_outside_the_lock() {
        no_hang(|| {
            let tmp = real_tempdir();
            let repo = repo(tmp.path());
            let base = tmp.path().join("wt");
            let a = worktree::ensure(&repo, &base, "KADE-1", "Slow setup", "main").unwrap();
            let gate = Gate::new(tmp.path());
            let plan = Prepare { copy: vec![], install: false, setup: Some(gate.lines()) };
            std::thread::scope(|s| {
                let preparing = s.spawn(|| prepare::prepare(&repo, &a.path, &plan, None, None));
                gate.wait_until_held();
                let others = s.spawn(|| {
                    assert!(worktree::ensure(&repo, &base, "KADE-2", "t", "main").unwrap().created);
                    worktree::worktree_of(&repo, "x").unwrap();
                });
                let finished_first = finishes_soon(&others);
                let preparing_early = preparing.is_finished();
                gate.open();
                preparing.join().unwrap().unwrap();
                others.join().unwrap();
                assert!(finished_first, "another card waited for this worktree's setup command");
                assert!(!preparing_early, "the setup command didn't wait for the gate");
            });
        });
    }
}
