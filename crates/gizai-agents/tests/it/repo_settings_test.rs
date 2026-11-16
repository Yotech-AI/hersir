// GA-263: a repository's own settings start nothing. Hersir's own git runs none of a repository's hooks and no fsmonitor
// program, and changes no setting there; a Codex run may write only what a commit writes in the git folder; every
// Gemini run goes without extensions and other MCP servers, and a worktree with a .gemini is found.
use gizai_agents::cli::{self, CliSpec, Kind, RunFolder, TaskRun};
use gizai_agents::worktree;
use std::path::{Path, PathBuf};
use std::process::Command;

const MINUTE: std::time::Duration = std::time::Duration::from_secs(60);

fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").args(args).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap().trim().to_string()
}

/// A temp folder by its real path, the way git reports it (see worktree_test).
fn real_tempdir() -> tempfile::TempDir {
    let base = std::env::temp_dir().canonicalize().unwrap();
    let base = PathBuf::from(base.to_string_lossy().trim_start_matches(r"\\?\").to_string());
    tempfile::tempdir_in(base).unwrap()
}

/// "GitHub" (a bare repository) and the project's clone of it, with a README on main and its own identity.
fn github_and_clone(tmp: &Path) -> (PathBuf, PathBuf) {
    let src = tmp.join("src");
    std::fs::create_dir(&src).unwrap();
    git(&src, &["init", "-q", "-b", "main"]);
    std::fs::write(src.join("README.md"), "# Shop\n").unwrap();
    git(&src, &["add", "-A"]);
    git(&src, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "init"]);
    let bare = tmp.join("github.git");
    git(tmp, &["clone", "-q", "--bare", src.to_str().unwrap(), bare.to_str().unwrap()]);
    let repo = tmp.join("repo");
    git(tmp, &["clone", "-q", bare.to_str().unwrap(), repo.to_str().unwrap()]);
    git(&repo, &["config", "user.email", "t@t"]);
    git(&repo, &["config", "user.name", "t"]);
    (bare, repo)
}

// ---- Hersir's own git ----

#[cfg(unix)]
mod no_hooks {
    use super::*;

    /// Writes an executable script through a child sh, so a fork elsewhere in the test process never holds it open
    /// for writing ("Text file busy").
    fn write_script(path: &Path, script: &str) {
        use std::io::Write;
        let mut sh = Command::new("sh").args(["-c", r#"cat > "$1" && chmod 755 "$1""#, "sh"]).arg(path)
            .stdin(std::process::Stdio::piped()).spawn().unwrap();
        sh.stdin.take().unwrap().write_all(script.as_bytes()).unwrap();
        assert!(sh.wait().unwrap().success(), "can't write {}", path.display());
    }

    const HOOKS: [&str; 11] = ["pre-commit", "prepare-commit-msg", "commit-msg", "post-commit", "pre-push", "post-checkout",
                               "post-merge", "reference-transaction", "post-index-change", "pre-auto-gc", "post-rewrite"];

    /// Sets `core.hooksPath` in the repository's .git/config to a folder of hooks, and `core.fsmonitor` to a script: each
    /// writes its name to the marker file it returns. The same hooks in .git/hooks too, git's own folder for them.
    fn plant(tmp: &Path, repo: &Path) -> PathBuf {
        let marker = tmp.join("ran");
        let hooks = tmp.join("hooks");
        std::fs::create_dir(&hooks).unwrap();
        let own = repo.join(".git/hooks");
        std::fs::create_dir_all(&own).unwrap();
        for hook in HOOKS {
            let script = format!("#!/bin/sh\necho {hook} >> '{}'\n", marker.display());
            write_script(&hooks.join(hook), &script);
            write_script(&own.join(hook), &script);
        }
        // an fsmonitor hook that answers nothing: git then looks at every file itself
        let monitor = tmp.join("fsmonitor.sh");
        write_script(&monitor, &format!("#!/bin/sh\necho fsmonitor >> '{}'\nexit 1\n", marker.display()));
        git(repo, &["config", "core.hooksPath", hooks.to_str().unwrap()]);
        git(repo, &["config", "core.fsmonitor", monitor.to_str().unwrap()]);
        marker
    }

    fn none_ran(marker: &Path, after: &str) {
        assert!(!marker.exists(), "after {after}, the repository's own programs ran: {:?}", std::fs::read_to_string(marker));
    }

    #[test]
    fn hersirs_commit_stash_fetch_push_worktree_add_and_remove_run_no_hook_and_no_fsmonitor_and_still_work() {
        let tmp = real_tempdir();
        let (bare, repo) = github_and_clone(tmp.path());
        let marker = plant(tmp.path(), &repo);
        let config = std::fs::read(repo.join(".git/config")).unwrap();
        let base = tmp.path().join("wt");

        // worktree add
        let wt = worktree::ensure(&repo, &base, "KADE-7", "Export invoices", "main").unwrap();
        assert!(wt.created && wt.path.join("README.md").is_file(), "the worktree is made and checked out");
        none_ran(&marker, "worktree add");

        // fetch: main moved on GitHub
        let other = tmp.path().join("other");
        git(tmp.path(), &["clone", "-q", bare.to_str().unwrap(), other.to_str().unwrap()]);
        std::fs::write(other.join("CHANGELOG.md"), "1.1\n").unwrap();
        git(&other, &["add", "-A"]);
        git(&other, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "1.1"]);
        git(&other, &["push", "-q", "origin", "main"]);
        let start = worktree::fetch_start(&repo, Some("origin"), "", "main", MINUTE).unwrap();
        assert_eq!(git(&repo, &["rev-parse", &start]), git(&other, &["rev-parse", "HEAD"]), "the fetch got main");
        none_ran(&marker, "fetch");

        // the uncommitted files (git status), then Commit…
        std::fs::write(wt.path.join("export.csv"), "id,total\n").unwrap();
        assert_eq!(worktree::uncommitted(&wt.path).unwrap(), 1);
        let made = worktree::commit_all(&wt.path, "Export invoices as CSV").unwrap();
        assert_eq!((git(&wt.path, &["rev-parse", "HEAD"]), made.subject.as_str()), (made.sha.clone(), "Export invoices as CSV"));
        none_ran(&marker, "git status and commit");

        // stash
        std::fs::write(wt.path.join("README.md"), "# Shop\n\nwork in progress\n").unwrap();
        std::fs::write(wt.path.join("notes.txt"), "untracked\n").unwrap();
        worktree::stash_all(&wt.path, "Hersir: GA-263 test").unwrap();
        assert_eq!(worktree::uncommitted(&wt.path).unwrap(), 0, "the changes went to git's stash");
        assert!(git(&repo, &["stash", "list"]).contains("Hersir: GA-263 test"));
        none_ran(&marker, "stash");

        // push
        worktree::push_branch(&repo, "origin", &wt.branch).unwrap();
        assert_eq!(git(&bare, &["rev-parse", &format!("refs/heads/{}", wt.branch)]), made.sha, "the branch is on GitHub");
        none_ran(&marker, "push");

        // worktree remove, with its branch
        let removal = worktree::remove_card_worktree_with(&repo, &base, &tmp.path().join("run-temp"), &wt.branch, true);
        assert!(removal.removed.is_some() && removal.branch_deleted && !wt.path.exists(), "{removal:?}");
        none_ran(&marker, "worktree remove");

        // Hersir changed no setting: the repository's config is as it was
        assert_eq!(String::from_utf8_lossy(&std::fs::read(repo.join(".git/config")).unwrap()), String::from_utf8_lossy(&config));

        // and the programs are live: your own git in the checkout runs them
        git(&repo, &["status", "--short"]);
        std::fs::write(repo.join("mine.txt"), "mine\n").unwrap();
        git(&repo, &["add", "mine.txt"]);
        git(&repo, &["commit", "-q", "-m", "mine"]);
        git(&repo, &["push", "-q", "origin", "HEAD:refs/heads/mine"]);
        git(&repo, &["worktree", "add", "-q", "-b", "by-hand", tmp.path().join("by-hand").to_str().unwrap()]);
        let ran = std::fs::read_to_string(&marker).unwrap_or_default();
        for program in ["fsmonitor", "pre-commit", "commit-msg", "post-commit", "pre-push", "post-checkout"] {
            assert!(ran.lines().any(|l| l == program), "{program} didn't run in your own git: {ran}");
        }
    }

    #[test]
    fn hooks_in_gits_own_hooks_folder_dont_run_for_hersirs_commit_either() {
        let tmp = real_tempdir();
        let (_, repo) = github_and_clone(tmp.path());
        let marker = tmp.path().join("ran");
        for hook in HOOKS {
            write_script(&repo.join(".git/hooks").join(hook), &format!("#!/bin/sh\necho {hook} >> '{}'\n", marker.display()));
        }
        let base = tmp.path().join("wt");
        let wt = worktree::ensure(&repo, &base, "KADE-8", "Hooks folder", "main").unwrap();
        std::fs::write(wt.path.join("a.txt"), "a\n").unwrap();
        worktree::commit_all(&wt.path, "A").unwrap();
        none_ran(&marker, "worktree add and commit");
        git(&wt.path, &["commit", "-q", "--allow-empty", "-m", "by hand"]);
        assert!(std::fs::read_to_string(&marker).unwrap_or_default().contains("pre-commit"), "your own commit runs .git/hooks");
    }

    #[test]
    fn the_settings_are_ones_of_the_command_itself() {
        assert_eq!(worktree::NO_HOOKS, ["-c", "core.hooksPath=/dev/null", "-c", "core.fsmonitor=false"]);
        assert_eq!(worktree::no_hooks_settings(), [("core.hooksPath".to_string(), "/dev/null".to_string()),
                                                  ("core.fsmonitor".to_string(), "false".to_string())]);
        let cmd = worktree::git_command(Path::new("/some/repo"));
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(args, ["-c", "core.hooksPath=/dev/null", "-c", "core.fsmonitor=false", "-C", "/some/repo"]);
    }
}

// ---- What a Codex run may write ----

/// The folders of a repository's git folder: `common`, and the worktree's own folder in it.
fn git_dirs(wt: &Path) -> (PathBuf, PathBuf) {
    (PathBuf::from(git(wt, &["rev-parse", "--path-format=absolute", "--git-common-dir"])),
     PathBuf::from(git(wt, &["rev-parse", "--absolute-git-dir"])))
}

#[test]
fn a_commit_in_a_worktree_writes_objects_refs_logs_and_its_own_worktree_folder_only() {
    let tmp = real_tempdir();
    let (_, repo) = github_and_clone(tmp.path());
    let base = tmp.path().join("wt");
    let a = worktree::ensure(&repo, &base, "KADE-1", "One", "main").unwrap();
    let b = worktree::ensure(&repo, &base, "KADE-2", "Two", "main").unwrap();
    let (common, own) = git_dirs(&a.path);
    let (_, other) = git_dirs(&b.path);
    assert_eq!(own.parent(), Some(common.join("worktrees").as_path()));
    let dirs = worktree::commit_dirs(&a.path).unwrap();
    assert_eq!(dirs, [common.join("objects"), common.join("refs"), common.join("logs"), own]);
    for never in [common.clone(), common.join("config"), common.join("hooks"), common.join("info"), common.join("worktrees"), other] {
        assert!(!dirs.iter().any(|d| d == &never), "{} is writable: {dirs:?}", never.display());
    }
    // the main checkout has no folder of its own apart from the shared one
    assert_eq!(worktree::commit_dirs(&repo).unwrap(), [common.join("objects"), common.join("refs"), common.join("logs")]);
    assert!(worktree::commit_dirs(tmp.path()).is_err(), "no repository, no folders");
}

#[test]
fn a_repository_with_reftable_gets_its_reftable_folder_too() {
    let tmp = real_tempdir();
    let repo = tmp.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    // git before 2.45 has no reftable: nothing to check there
    if !Command::new("git").args(["init", "-q", "-b", "main", "--ref-format=reftable"]).current_dir(&repo).status().unwrap().success() {
        return;
    }
    git(&repo, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "init"]);
    let wt = worktree::ensure(&repo, &tmp.path().join("wt"), "KADE-3", "Reftable", "main").unwrap();
    let (common, own) = git_dirs(&wt.path);
    let dirs = worktree::commit_dirs(&wt.path).unwrap();
    assert!(dirs.contains(&common.join("reftable")) && dirs.contains(&common.join("objects")) && dirs.contains(&own), "{dirs:?}");
    assert!(!dirs.contains(&common) && !dirs.contains(&common.join("config")), "{dirs:?}");
}

/// Makes every folder of the git folder `common` read-only but `writable` (and what is in them), as a sandbox with only
/// those writable roots; `undo` gives the folders back.
#[cfg(unix)]
fn only_writable(common: &Path, writable: &[PathBuf], undo: bool) {
    use std::os::unix::fs::PermissionsExt;
    fn walk(dir: &Path, writable: &[PathBuf], undo: bool) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() && !p.is_symlink() && (undo || !writable.contains(&p)) {
                walk(&p, writable, undo);
            }
        }
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(if undo { 0o755 } else { 0o555 })).unwrap();
    }
    walk(common, writable, undo);
}

/// The sandbox's writable roots, written the way Codex's `-c` takes them: a TOML list of strings.
fn toml_list(paths: &[String]) -> String {
    let quoted: Vec<String> = paths.iter().map(|p| format!("\"{}\"", p.replace('\\', "\\\\").replace('"', "\\\""))).collect();
    format!("sandbox_workspace_write.writable_roots=[{}]", quoted.join(","))
}

#[cfg(unix)]
#[test]
fn with_only_the_commit_folders_writable_a_worktrees_git_add_commit_and_push_still_work() {
    let tmp = real_tempdir();
    let (bare, repo) = github_and_clone(tmp.path());
    let wt = worktree::ensure(&repo, &tmp.path().join("wt"), "KADE-4", "Sandboxed", "main").unwrap();
    let (common, _) = git_dirs(&wt.path);
    let writable = worktree::commit_dirs(&wt.path).unwrap();
    only_writable(&common, &writable, false);
    struct Undo<'a>(&'a Path);
    impl Drop for Undo<'_> {
        fn drop(&mut self) { only_writable(self.0, &[], true); }
    }
    let _undo = Undo(&common);
    // as root every folder stays writable: nothing to check
    if std::fs::write(common.join("probe"), "").is_ok() {
        return;
    }
    // a run's own git: Hersir turns rerere off in Codex's sandbox (its rr-cache would be in the git folder itself)
    let run_git = |args: &[&str]| -> std::process::Output {
        Command::new("git").args(["-c", "rerere.enabled=false"]).args(args).current_dir(&wt.path).output().unwrap()
    };
    std::fs::write(wt.path.join("export.csv"), "id,total\n").unwrap();
    let add = run_git(&["add", "export.csv"]);
    assert!(add.status.success(), "git add: {}", String::from_utf8_lossy(&add.stderr));
    let commit = run_git(&["commit", "-q", "-m", "Export"]);
    assert!(commit.status.success(), "git commit: {}", String::from_utf8_lossy(&commit.stderr));
    let push = run_git(&["push", "-q", "origin", "HEAD"]);
    assert!(push.status.success(), "git push: {}", String::from_utf8_lossy(&push.stderr));
    assert_eq!(git(&bare, &["rev-parse", &format!("refs/heads/{}", wt.branch)]), git(&wt.path, &["rev-parse", "HEAD"]));
    // and nothing can go in the git folder itself: no new hook, no setting
    assert!(std::fs::write(common.join("hooks").join("pre-commit"), "#!/bin/sh\n").is_err());
    assert!(std::fs::write(common.join("config.lock"), "").is_err());
}

#[test]
fn a_codex_runs_writable_roots_are_what_a_commit_writes_its_change_folders_and_temp_never_the_git_folder() {
    let tmp = real_tempdir();
    let (_, repo) = github_and_clone(tmp.path());
    let wt = worktree::ensure(&repo, &tmp.path().join("wt"), "KADE-5", "Codex", "main").unwrap();
    let (common, own) = git_dirs(&wt.path);
    let s = |p: PathBuf| p.to_string_lossy().into_owned();
    let run = TaskRun {
        prompt: "Do the card".into(),
        writable_dirs: worktree::commit_dirs(&wt.path).unwrap().into_iter().map(s).collect(),
        folders: vec![RunFolder { path: "/home/u/notes".into(), change: true }, RunFolder { path: "/home/u/docs".into(), change: false }],
        temp_dir: Some("/var/tmp/run-1".into()),
        ..Default::default()
    };
    let spec = CliSpec { kind: Kind::Codex, bin: "/bin/fake".into(), env: vec![], args: String::new() };
    let e = cli::task_exec(&spec, &run);
    let want = toml_list(&[s(common.join("objects")), s(common.join("refs")), s(common.join("logs")), s(own),
                           "/home/u/notes".into(), "/var/tmp/run-1".into()]);
    let configs: Vec<&String> = e.args.windows(2).filter(|w| w[0] == "-c").map(|w| &w[1]).collect();
    let roots = configs.iter().find(|c| c.starts_with("sandbox_workspace_write.writable_roots=")).expect("writable roots");
    assert_eq!(**roots, want);
    assert!(configs.iter().any(|c| *c == r#"sandbox_mode="workspace-write""#), "the worktree itself is Codex's workspace: {configs:?}");
    for never in [common.clone(), common.join("config"), common.join("hooks")] {
        assert!(!roots.contains(&format!("\"{}\"", never.display())), "{} is writable: {roots}", never.display());
    }
    assert!(!roots.contains("/home/u/docs"), "a read folder stays read only: {roots}");
}

// ---- Gemini ----

fn gemini(run: TaskRun) -> Vec<String> {
    let spec = CliSpec { kind: Kind::Gemini, bin: "/bin/fake".into(), env: vec![], args: String::new() };
    cli::task_exec(&spec, &TaskRun { prompt: "Do the card".into(), session_id: "S-1".into(), ..run }).args
}

/// The value after the flag `flag`.
fn after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).map(String::as_str)
}

#[test]
fn every_gemini_run_goes_without_extensions_and_with_only_its_own_mcp_servers() {
    let a = gemini(TaskRun { mcp_servers: vec!["otus".into(), "figma".into()], ..Default::default() });
    assert_eq!(after(&a, "--extensions"), Some("none"), "{a:?}");
    assert_eq!(after(&a, "--allowed-mcp-server-names"), Some("otus,figma"), "{a:?}");
    // none of its own: one empty name, which no server has (Gemini allows every server for an empty list)
    for run in [TaskRun::default(), TaskRun { resume: true, ..Default::default() },
                TaskRun { permission_mode: "plan".into(), model: Some("gemini-2.5-pro".into()), ..Default::default() }] {
        let a = gemini(run);
        assert_eq!(after(&a, "--extensions"), Some("none"), "{a:?}");
        assert_eq!(after(&a, "--allowed-mcp-server-names"), Some(""), "{a:?}");
        assert_eq!(a.iter().filter(|x| *x == "--allowed-mcp-server-names").count(), 1, "{a:?}");
    }
    // Claude Code and Codex don't get Gemini's flags
    for kind in [Kind::ClaudeCode, Kind::Codex] {
        let spec = CliSpec { kind, bin: "/bin/fake".into(), env: vec![], args: String::new() };
        let a = cli::task_exec(&spec, &TaskRun { mcp_servers: vec!["otus".into()], ..Default::default() }).args.join(" ");
        assert!(!a.contains("--extensions") && !a.contains("--allowed-mcp-server-names"), "{a}");
    }
}

#[test]
fn a_worktree_with_a_gemini_folder_file_or_link_is_found() {
    let tmp = real_tempdir();
    let wt = tmp.path();
    assert!(!cli::gemini_folder(wt));
    std::fs::create_dir_all(wt.join("src/.gemini")).unwrap();
    std::fs::write(wt.join(".geminiignore"), "target/\n").unwrap();
    std::fs::write(wt.join("GEMINI.md"), "# Notes\n").unwrap();
    assert!(!cli::gemini_folder(wt), "only a .gemini at the worktree's root");
    std::fs::create_dir(wt.join(".gemini")).unwrap();
    std::fs::write(wt.join(".gemini/settings.json"), r#"{"mcpServers": {"x": {"command": "touch"}}}"#).unwrap();
    assert!(cli::gemini_folder(wt));
    std::fs::remove_dir_all(wt.join(".gemini")).unwrap();
    std::fs::write(wt.join(".gemini"), "").unwrap();
    assert!(cli::gemini_folder(wt), "a file by that name");
    #[cfg(unix)]
    {
        std::fs::remove_file(wt.join(".gemini")).unwrap();
        std::os::unix::fs::symlink(tmp.path().join("gone"), wt.join(".gemini")).unwrap();
        assert!(cli::gemini_folder(wt), "a link, also to nothing");
    }
}

#[test]
fn a_gemini_run_on_such_a_worktree_is_held_with_the_cards_sentence() {
    assert_eq!(cli::GEMINI_FOLDER, "This repository has a .gemini folder, whose settings could start programs outside the agent's \
                                   allowed commands: remove it from the branch, or run the card on Claude Code or Codex.");
}
