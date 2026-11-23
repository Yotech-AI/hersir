//! GA-318 (GA-270 #1, CWE-88): every git command Hersir runs with a remote's name or a project's link has
//! `--end-of-options` right before it, so git reads it as an address whatever it is; and an address that starts with "-"
//! is refused before git starts. A git first on PATH writes down the arguments of each of Hersir's own git commands (the
//! ones after `NO_HOOKS` and `-C <folder>`), then runs the real git. "GitHub" is a local bare repository: nothing here
//! leaves the computer.
// Linux and macOS only: the fake git is a shell script.
#![cfg(unix)]
// A test program of its own, with one test: it puts the fake git first on PATH for the whole process.
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use gizai_agents::connection::PushOver;
use gizai_agents::worktree::{self, Pushed};

const LIMIT: Duration = Duration::from_secs(60);
const END: &str = "--end-of-options";

fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").args(["-c", "user.email=t@t", "-c", "user.name=t"]).args(args).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap().trim().to_string()
}

/// The arguments of each of Hersir's own git commands, one per line, each command ended by a line `@@`.
struct Log(PathBuf);

impl Log {
    /// Puts a git first on PATH that writes to the log, then runs the real git.
    fn install(tmp: &Path) -> Log {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let real = std::env::split_paths(&path).map(|d| d.join("git")).find(|g| g.is_file()).expect("git on PATH");
        let (bin, log) = (tmp.join("bin"), tmp.join("git.log"));
        std::fs::create_dir(&bin).unwrap();
        let script = format!("#!/bin/sh\n{{ for a in \"$@\"; do printf '%s\\n' \"$a\"; done; printf '%s\\n' '@@'; }} >> '{}'\nexec '{}' \"$@\"\n",
                             log.display(), real.display());
        std::fs::write(bin.join("git"), script).unwrap();
        std::fs::set_permissions(bin.join("git"), std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut on_path = vec![bin];
        on_path.extend(std::env::split_paths(&path));
        // SAFETY: the only test in this program sets it before it starts a thread or a process.
        unsafe { std::env::set_var("PATH", std::env::join_paths(on_path).unwrap()) };
        Log(log)
    }

    /// Hersir's own git commands since the last call, from their command on: after `NO_HOOKS`, `-C <folder>` and any
    /// `-c <setting>` the call adds.
    fn take(&self) -> Vec<Vec<String>> {
        let text = std::fs::read_to_string(&self.0).unwrap_or_default();
        std::fs::write(&self.0, "").unwrap();
        let mut out = vec![];
        for cmd in text.split("@@\n").filter(|c| !c.is_empty()) {
            let args: Vec<String> = cmd.lines().map(str::to_string).collect();
            if args.len() < 6 || args[..4] != worktree::NO_HOOKS || args[4] != "-C" {
                continue;
            }
            let mut rest = &args[6..];
            while rest.first().is_some_and(|a| a == "-c") {
                rest = &rest[2..];
            }
            out.push(rest.to_vec());
        }
        out
    }
}

/// `cmds` has the git command `want` (word for word), and in every one of them each of `addresses` comes right after
/// `--end-of-options`.
fn ran(what: &str, cmds: &[Vec<String>], want: &[&str], addresses: &[&str]) {
    assert!(cmds.iter().any(|c| c == want), "{what}: no git {want:?} in {cmds:?}");
    for c in cmds {
        for (i, a) in c.iter().enumerate() {
            if addresses.contains(&a.as_str()) {
                assert!(i > 0 && c[i - 1] == END, "{what}: {a} without {END} right before it: {c:?}");
            }
        }
    }
}

#[test]
fn every_git_command_with_a_link_or_remote_has_end_of_options_before_it_and_a_link_that_starts_with_a_dash_never_reaches_git() {
    let tmp = tempfile::tempdir().unwrap();
    let log = Log::install(tmp.path());
    // "GitHub" (bare, fed from src) and the project's repository, a clone of it (its remote origin)
    let src = tmp.path().join("src");
    std::fs::create_dir(&src).unwrap();
    git(&src, &["init", "-q", "-b", "main"]);
    git(&src, &["commit", "-q", "--allow-empty", "-m", "init"]);
    let bare = tmp.path().join("github.git");
    git(tmp.path(), &["clone", "-q", "--bare", src.to_str().unwrap(), bare.to_str().unwrap()]);
    let repo = tmp.path().join("repo");
    git(tmp.path(), &["clone", "-q", bare.to_str().unwrap(), repo.to_str().unwrap()]);
    let base = git(&repo, &["rev-parse", "HEAD"]);
    for b in ["feature-a", "feature-b", "feature-c"] {
        git(&repo, &["switch", "-q", "-c", b, &base]);
        git(&repo, &["commit", "-q", "--allow-empty", "-m", b]);
    }
    git(&repo, &["switch", "-q", "main"]);
    let url = bare.to_str().unwrap();
    let both = [url, "origin"];
    log.take();

    // fetch: from the link itself, and through the repository's remote for it
    assert_eq!(worktree::fetch_start(&repo, None, url, "main", LIMIT).unwrap(), "refs/gizai/base/main");
    ran("fetch_start (link)", &log.take(), &["fetch", "--quiet", "--no-tags", END, url, "+refs/heads/main:refs/gizai/base/main"], &both);
    assert_eq!(worktree::fetch_start(&repo, Some("origin"), url, "main", LIMIT).unwrap(), "refs/remotes/origin/main");
    ran("fetch_start (remote)", &log.take(), &["fetch", "--quiet", "--no-tags", END, "origin", "+refs/heads/main:refs/remotes/origin/main"], &both);
    // ls-remote --symref: the remote's main branch
    assert_eq!(worktree::remote_head(&repo, "origin", LIMIT).unwrap().as_deref(), Some("main"));
    ran("remote_head", &log.take(), &["ls-remote", "--symref", END, "origin", "HEAD"], &both);
    // push, over SSH and to Bitbucket (which reads the remote's push addresses first)
    worktree::push_branch(&repo, url, "feature-a").unwrap();
    ran("push_branch", &log.take(), &["push", "--quiet", END, url, "refs/heads/feature-a:refs/heads/feature-a"], &both);
    worktree::push_branch_over(&repo, "origin", "feature-b", &PushOver::Bitbucket).unwrap();
    ran("push_branch_over", &log.take(), &["push", "--quiet", END, "origin", "refs/heads/feature-b:refs/heads/feature-b"], &both);
    // the push after a run: ls-remote, then the push
    assert!(matches!(worktree::push_new_commits(&repo, url, "feature-c", &base, &PushOver::Ssh).unwrap(), Pushed::Commits(1)));
    let cmds = log.take();
    ran("push_new_commits (ls-remote)", &cmds, &["ls-remote", "--quiet", END, url, "refs/heads/feature-c"], &both);
    ran("push_new_commits (push)", &cmds, &["push", "--quiet", END, url, "refs/heads/feature-c:refs/heads/feature-c"], &both);
    // ls-remote for the branches of pull requests
    let listed = worktree::ls_remote(&repo, url, &["refs/heads/main".into()], &PushOver::Ssh, LIMIT).unwrap();
    assert_eq!(listed.len(), 1, "{listed:?}");
    ran("ls_remote", &log.take(), &["ls-remote", "--quiet", END, url, "refs/heads/main"], &both);
    // the connection check
    worktree::can_push(&repo, url, &PushOver::Ssh, LIMIT).unwrap_or_else(|p| panic!("{}", p.what));
    ran("can_push", &log.take(), &["push", "--dry-run", "--quiet", "--no-verify", END, url, "HEAD:refs/heads/gizai-connection-check"], &both);
    // where a push goes: a remote's name, and an address
    assert_eq!(worktree::push_url(&repo, "origin", &PushOver::Ssh).as_deref(), Some(url));
    ran("push_url (remote)", &log.take(), &["remote", "get-url", "--push", END, "origin"], &both);
    assert_eq!(worktree::push_url(&repo, url, &PushOver::Ssh).as_deref(), Some(url));
    ran("push_url (link)", &log.take(), &["ls-remote", "--get-url", END, url], &both);

    // A link that starts with "-" (one saved before Hersir refused such links): each refuses it with a sentence before git
    // starts, so git never runs it as an option and the probe file never appears.
    let probe = tmp.path().join("PWNED");
    let links = [format!("--upload-pack=touch${{IFS}}{};#@x:y", probe.display()), format!("-oProxyCommand=touch {}", probe.display()),
                 "-".to_string()];
    // the first is live: as git got it before the fix (before the refspec, with no --end-of-options), git runs it
    let control = tmp.path().join("control");
    git(tmp.path(), &["clone", "-q", url, control.to_str().unwrap()]);
    let live = links[0].replace("PWNED", "CONTROL");
    let _ = Command::new("git").current_dir(&control).args(["fetch", "--quiet", "--no-tags", &live, "+refs/heads/main:refs/gizai/base/main"]).output();
    assert!(tmp.path().join("CONTROL").exists(), "the audit's link no longer runs a command in this git: the probe below proves nothing");
    log.take();
    let said = |e: String, link: &str| assert!(e.contains(&format!("{link} starts with \"-\", which git reads as an option")), "{link}: {e}");
    for link in &links {
        let l = link.as_str();
        said(worktree::fetch_start(&repo, None, l, "main", LIMIT).unwrap_err().to_string(), l);
        said(worktree::fetch_start(&repo, Some(l), url, "main", LIMIT).unwrap_err().to_string(), l);
        said(worktree::remote_head(&repo, l, LIMIT).unwrap_err(), l);
        said(worktree::push_branch(&repo, l, "feature-a").unwrap_err().to_string(), l);
        said(worktree::push_branch_over(&repo, l, "feature-a", &PushOver::Bitbucket).unwrap_err().to_string(), l);
        said(worktree::push_new_commits(&repo, l, "feature-a", &base, &PushOver::Ssh).unwrap_err().to_string(), l);
        said(worktree::ls_remote(&repo, l, &["refs/heads/main".into()], &PushOver::Ssh, LIMIT).unwrap_err().to_string(), l);
        said(worktree::can_push(&repo, l, &PushOver::Ssh, LIMIT).unwrap_err().what, l);
        assert_eq!(worktree::push_url(&repo, l, &PushOver::Ssh), None, "{l}");
        let cmds = log.take();
        assert!(cmds.iter().all(|c| !c.iter().any(|a| a == l)), "git started with {l}: {cmds:?}");
        assert!(!probe.exists(), "{l} ran a command");
    }
}
