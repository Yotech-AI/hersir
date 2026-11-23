//! GA-322 (GA-270 #2): Hersir's git in a card's worktree takes its git folders from the project's repository
//! (`worktree::git_command`: GIT_DIR, GIT_COMMON_DIR and GIT_WORK_TREE), never from the worktree's `.git` file or the
//! `commondir` file in the worktree's own git folder, which a Codex run may write. Probes plant a filter that touches a
//! file, the way the audit did: a `commondir` pointed at a folder of the run's own, a `config.worktree`, a `.git` file
//! pointed elsewhere. Hersir's git, its Changes and `git_folder_problem` are checked against each, and plain git in the
//! same worktree runs the planted filter, so each probe is a real one. Your editor and a terminal don't open there either.
use std::path::{Path, PathBuf};
use std::process::Command;

use gizai_agents::worktree::{self, CardGit};

/// git in `dir` as a fixed person, without the environment of a git Hersir starts.
fn git_out(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git").args(["-c", "user.email=t@t", "-c", "user.name=t", "-c", "commit.gpgsign=false", "-c", "core.autocrlf=false"])
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
    let base = PathBuf::from(base.to_string_lossy().trim_start_matches(r"\\?\").to_string());
    tempfile::tempdir_in(base).unwrap()
}

/// A project's repository with a.txt committed, and card KADE-1's worktree in `tmp/data/worktrees/KADE`, made the way a
/// card's start makes it (`worktree::ensure`, which registers that folder as the project's card folder). Returns the
/// repository, the worktree and the worktree's own folder in the repository's git folder.
fn card(tmp: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let repo = tmp.join("repo");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("a.txt"), "one\n").unwrap();
    git(&repo, &["add", "a.txt"]);
    git(&repo, &["commit", "-q", "-m", "init"]);
    let base = tmp.join("data").join("worktrees").join("KADE");
    let wt = worktree::ensure(&repo, &base, "KADE-1", "Probe", "main").unwrap().path;
    let admin = PathBuf::from(git(&wt, &["rev-parse", "--absolute-git-dir"]));
    assert_eq!(admin, repo.join(".git").join("worktrees").join("KADE-1"));
    (repo, wt, admin)
}

/// The value `cmd` sets for `key`, None when it sets none.
fn env_of(cmd: &Command, key: &str) -> Option<PathBuf> {
    cmd.get_envs().find(|(k, _)| *k == key).and_then(|(_, v)| v.map(PathBuf::from))
}

/// Copies the folder `from` to `to`, whole.
fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to.join(e.file_name()));
        } else {
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

/// A clean filter `x` that touches `probe`, as lines of a git config file.
fn filter(probe: &Path) -> String {
    format!("[filter \"x\"]\n\tclean = touch {} && cat\n\tsmudge = cat\n", probe.display())
}

/// What a run plants in the worktree itself for a filter: every file goes through `x`, and a.txt changed, so `git diff`
/// and `git status` clean it.
fn plant_in_worktree(wt: &Path) {
    std::fs::write(wt.join(".gitattributes"), "* filter=x\n").unwrap();
    std::fs::write(wt.join("a.txt"), "two\n").unwrap();
}

/// Hersir's git diff and git status in `wt`: (whether each succeeded, what git diff said).
fn hersirs_git(wt: &Path) -> (bool, bool, String) {
    let diff = worktree::git_command(wt).args(["diff"]).output().unwrap();
    let status = worktree::git_command(wt).args(["status", "--porcelain"]).output().unwrap();
    (diff.status.success(), status.status.success(), String::from_utf8_lossy(&diff.stdout).into_owned())
}

#[cfg(unix)]
#[test]
fn hersirs_git_in_a_card_worktree_takes_its_folders_from_the_projects_repository() {
    let tmp = real_tempdir();
    let (repo, wt, admin) = card(tmp.path());
    let common = repo.join(".git");
    // in the worktree, and in a folder of it
    std::fs::create_dir(wt.join("src")).unwrap();
    for dir in [wt.clone(), wt.join("src")] {
        let cmd = worktree::git_command(&dir);
        assert_eq!(env_of(&cmd, "GIT_DIR").as_deref(), Some(admin.as_path()), "{}", dir.display());
        assert_eq!(env_of(&cmd, "GIT_COMMON_DIR").as_deref(), Some(common.as_path()), "{}", dir.display());
        assert_eq!(env_of(&cmd, "GIT_WORK_TREE").as_deref(), Some(wt.as_path()), "{}", dir.display());
        assert!(worktree::checked_git_command(&dir).is_ok());
    }
    assert_eq!(worktree::card_git(&repo, &wt).unwrap(), CardGit { git_dir: admin.clone(), common_dir: common.clone(), work_tree: wt.clone() });
    assert_eq!(worktree::git_folder_problem(&repo, &wt), None, "a worktree as Hersir made it is fine");
    assert_eq!(worktree::card_problem(&wt), None);
    // and it works as before: the card's branch, its status, a commit
    let head = worktree::git_command(&wt).args(["rev-parse", "--abbrev-ref", "HEAD"]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&head.stdout).trim(), "hersir/kade-1-probe");
    std::fs::write(wt.join("b.txt"), "b\n").unwrap();
    let status = worktree::git_command(&wt).args(["status", "--porcelain"]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&status.stdout).trim(), "?? b.txt");
    let top = worktree::git_command(&wt.join("src")).args(["rev-parse", "--show-toplevel"]).output().unwrap();
    assert_eq!(PathBuf::from(String::from_utf8_lossy(&top.stdout).trim()), wt, "the worktree is the work tree, also from a folder in it");

    // the main checkout and a folder outside the card folders: git as before, without these variables
    let outside = tmp.path().join("other");
    std::fs::create_dir(&outside).unwrap();
    for dir in [repo.clone(), outside] {
        let cmd = worktree::git_command(&dir);
        for key in ["GIT_DIR", "GIT_COMMON_DIR", "GIT_WORK_TREE"] {
            assert!(cmd.get_envs().all(|(k, _)| k != key), "{key} in {}", dir.display());
        }
        assert_eq!(worktree::card_problem(&dir), None);
    }
}

#[cfg(unix)]
#[test]
fn a_commondir_pointed_at_the_runs_own_folder_gets_no_filter_run_by_hersirs_git() {
    let tmp = real_tempdir();
    let (repo, wt, admin) = card(tmp.path());
    let probe = tmp.path().join("probe-ran");
    // what the audit's probe did: a copy of the repository's git folder with a filter in its config, and commondir at it
    let evil = tmp.path().join("evil-common");
    copy_dir(&repo.join(".git"), &evil);
    let config = std::fs::read_to_string(evil.join("config")).unwrap();
    std::fs::write(evil.join("config"), format!("{config}{}", filter(&probe))).unwrap();
    std::fs::write(admin.join("commondir"), format!("{}\n", evil.display())).unwrap();
    plant_in_worktree(&wt);

    let (diff_ok, status_ok, diff) = hersirs_git(&wt);
    assert!(diff_ok && status_ok, "Hersir's git still works there, from the project's repository");
    assert!(diff.contains("+two"), "{diff}");
    let changed = gizai_agents::changes::of_worktree(&wt, &git(&repo, &["rev-parse", "main"])).unwrap();
    assert!(changed.iter().any(|f| f.path == "a.txt"), "{changed:?}");
    assert!(!probe.exists(), "Hersir's git ran the planted filter");

    let why = worktree::git_folder_problem(&repo, &wt).expect("a changed commondir is a problem");
    assert!(why.contains(&admin.join("commondir").display().to_string()) && why.contains("no longer says ../..")
            && why.contains("take the hold off"), "{why}");
    assert_eq!(worktree::card_problem(&wt.join("src")).as_deref(), Some(why.as_str()), "a folder in it counts too");

    // the probe is real: plain git there follows commondir and runs the filter, as you
    git(&wt, &["diff"]);
    assert!(probe.exists(), "plain git didn't run the planted filter, so this probe proves nothing");

    // set right again (../..): no problem, and the run can go on
    std::fs::remove_file(&probe).unwrap();
    std::fs::write(admin.join("commondir"), "../..\n").unwrap();
    assert_eq!(worktree::git_folder_problem(&repo, &wt), None);
}

#[cfg(unix)]
#[test]
fn a_config_worktree_keeps_hersirs_git_from_starting_there_at_all() {
    let tmp = real_tempdir();
    let (repo, wt, admin) = card(tmp.path());
    let probe = tmp.path().join("probe-ran");
    // a repository that reads config.worktree (sparse-checkout's `init` turns this on)
    git(&repo, &["config", "core.repositoryformatversion", "1"]);
    git(&repo, &["config", "extensions.worktreeConfig", "true"]);
    std::fs::write(admin.join("config.worktree"), filter(&probe)).unwrap();
    plant_in_worktree(&wt);

    let why = worktree::checked_git_command(&wt).expect_err("no git with a config.worktree");
    assert!(why.starts_with(&format!("Hersir doesn't run git in {}", wt.display())) && why.contains("config.worktree"), "{why}");
    let (diff_ok, status_ok, _) = hersirs_git(&wt);
    assert!(!diff_ok && !status_ok, "git_command doesn't start a git that reads a config.worktree");
    let err = gizai_agents::changes::of_worktree(&wt, &git(&repo, &["rev-parse", "main"])).unwrap_err().to_string();
    assert!(err.contains("config.worktree"), "the Changes view says why: {err}");
    assert!(worktree::stash_all(&wt, "x").unwrap_err().to_string().contains("config.worktree"));
    let search = gizai_agents::editor::search(&wt, "two", gizai_agents::editor::Find::default()).unwrap_err();
    assert!(search.starts_with("Search failed: Hersir doesn't run git in") && search.contains("config.worktree"),
            "the Editor's Search says why: {search}");
    assert!(!probe.exists(), "Hersir's git ran the planted filter");
    let why = worktree::git_folder_problem(&repo, &wt).expect("a config.worktree is a problem");
    assert!(why.contains(&admin.join("config.worktree").display().to_string()) && why.contains("Delete that file"), "{why}");

    // the probe is real: plain git reads config.worktree and runs the filter
    git(&wt, &["diff"]);
    assert!(probe.exists(), "plain git didn't run the planted filter, so this probe proves nothing");

    // a config.worktree of any kind counts, a folder too; gone, git runs again
    std::fs::remove_file(admin.join("config.worktree")).unwrap();
    std::fs::create_dir(admin.join("config.worktree")).unwrap();
    assert!(worktree::checked_git_command(&wt).is_err());
    std::fs::remove_dir(admin.join("config.worktree")).unwrap();
    assert!(worktree::checked_git_command(&wt).is_ok());
    assert_eq!(worktree::git_folder_problem(&repo, &wt), None);
}

#[cfg(unix)]
#[test]
fn a_dot_git_file_pointed_elsewhere_is_a_problem_and_hersirs_git_doesnt_follow_it() {
    let tmp = real_tempdir();
    let (repo, wt, admin) = card(tmp.path());
    let probe = tmp.path().join("probe-ran");
    let evil = tmp.path().join("evil-git");
    copy_dir(&repo.join(".git"), &evil);
    let config = std::fs::read_to_string(evil.join("config")).unwrap();
    std::fs::write(evil.join("config"), format!("{config}{}", filter(&probe))).unwrap();
    std::fs::write(wt.join(".git"), format!("gitdir: {}\n", evil.display())).unwrap();
    plant_in_worktree(&wt);

    let (diff_ok, _, diff) = hersirs_git(&wt);
    assert!(diff_ok && diff.contains("+two"), "{diff}");
    assert!(!probe.exists(), "Hersir's git followed the worktree's .git file");
    let why = worktree::git_folder_problem(&repo, &wt).expect("a .git pointed elsewhere is a problem");
    assert!(why.contains(&format!("{} no longer points at {}", wt.join(".git").display(), admin.display()))
            && why.contains("git worktree repair"), "{why}");

    git(&wt, &["diff"]);
    assert!(probe.exists(), "plain git didn't run the planted filter, so this probe proves nothing");

    // a .git that is a folder of its own isn't the worktree's either
    std::fs::remove_file(wt.join(".git")).unwrap();
    std::fs::create_dir(wt.join(".git")).unwrap();
    assert!(worktree::git_folder_problem(&repo, &wt).is_some_and(|w| w.contains("no longer points at")));
}

#[cfg(unix)]
#[test]
fn hersir_doesnt_run_git_in_a_worktree_the_repository_doesnt_vouch_for() {
    let tmp = real_tempdir();
    let (repo, wt, admin) = card(tmp.path());
    let worktrees = repo.join(".git").join("worktrees");

    // the worktree's own folder now says it belongs to another folder
    let gitdir = std::fs::read_to_string(admin.join("gitdir")).unwrap();
    std::fs::write(admin.join("gitdir"), format!("{}\n", tmp.path().join("elsewhere/.git").display())).unwrap();
    let why = worktree::checked_git_command(&wt).expect_err("no folder points at the worktree");
    assert!(why.contains(&format!("no folder in {} points at it", worktrees.display())), "{why}");
    assert!(!worktree::git_command(&wt).args(["status"]).output().unwrap().status.success(), "git refuses at once");
    let held = worktree::git_folder_problem(&repo, &wt).expect("a problem");
    assert!(held.starts_with(&format!("Hersir doesn't run git in this card's worktree {}", wt.display())) && held.contains("git worktree repair"), "{held}");

    // two folders point at it: Hersir doesn't pick one
    std::fs::write(admin.join("gitdir"), &gitdir).unwrap();
    copy_dir(&admin, &worktrees.join("KADE-1-copy"));
    let why = worktree::checked_git_command(&wt).expect_err("two folders point at the worktree");
    assert!(why.contains("more than one folder in") && why.contains("KADE-1-copy"), "{why}");
    assert!(worktree::git_folder_problem(&repo, &wt).is_some_and(|w| w.contains("more than one folder")));
    std::fs::remove_dir_all(worktrees.join("KADE-1-copy")).unwrap();
    assert!(worktree::checked_git_command(&wt).is_ok());
}

#[cfg(unix)]
#[test]
fn your_editor_and_a_terminal_dont_open_in_a_worktree_whose_git_folder_a_run_changed() {
    let tmp = real_tempdir();
    let (_repo, wt, admin) = card(tmp.path());
    let missing = tmp.path().join("bin/code");
    let path = std::ffi::OsString::from("/usr/bin:/bin");
    use gizai_agents::editor::{self, AppId};
    // a worktree as Hersir made it: the editor is started (here one that isn't there)
    let err = editor::open_in(AppId::VsCode, &missing, &wt, None, None, &path).unwrap_err();
    assert!(err.starts_with("Hersir couldn't start"), "{err}");

    std::fs::write(admin.join("commondir"), format!("{}\n", tmp.path().join("evil").display())).unwrap();
    let why = worktree::card_problem(&wt).expect("a problem");
    assert_eq!(editor::open_in(AppId::VsCode, &missing, &wt, None, None, &path).unwrap_err(), why);
    assert_eq!(editor::open_in(AppId::Zed, &missing, &wt.join("src"), None, None, &path).unwrap_err(), why);

    let terms = gizai_agents::terminal::Terminals::default();
    let start = gizai_agents::terminal::Start {
        folder: wt.display().to_string(), folder_path: wt.display().to_string(), folder_name: "KADE-1".into(), task_id: None,
        program: PathBuf::from("/bin/sh"), args: vec![], env: vec![], cols: 80, rows: 24, name: None,
    };
    let changed: gizai_agents::terminal::Changed = std::sync::Arc::new(|_| {});
    let refused = terms.open(start, changed).map(|_| ()).unwrap_err();
    assert_eq!(refused, why);
    assert!(terms.list().is_empty(), "no terminal opened");
}
