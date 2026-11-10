//! GA-242's lock on a repository's worktrees (`worktree::one_at_a_time`), checked by holding up the git command that
//! holds it. Until GA-263 a post-checkout hook and a `core.fsmonitor` script in the test repository held it up; Hersir's
//! own git now runs neither (`worktree::NO_HOOKS`), so a fake git first on PATH does: for Hersir's own calls, which
//! start with `NO_HOOKS` and `-C <folder>`, it first runs the lines a case wrote for that git command, then the real
//! git. A program of its own: it puts that git first on PATH in the test process's environment, so its one test runs
//! the cases in turn. While a gate holds, what takes the lock can't finish, however slow the machine is, so each case
//! fails every time without the lock, on any disk.
#![cfg(unix)]
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::ScopedJoinHandle;
use std::time::{Duration, Instant};

use gizai_agents::{copies, worktree};

/// How long a command that should wait for its turn gets to show it doesn't.
const PAUSE: Duration = Duration::from_millis(500);
/// How long a command that should go ahead may take. Shorter than the gate holds: it fails, it doesn't pass late.
const SOON: Duration = Duration::from_secs(60);
/// How long one case may take before it counts as a deadlock: far more than a slow runner needs.
const HANG: Duration = Duration::from_secs(300);

fn git(dir: &Path, args: &[&str]) { assert!(Command::new("git").args(args).current_dir(dir).status().unwrap().success()); }

fn repo(tmp: &Path) -> PathBuf {
    let repo = tmp.join("repo");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "init"]);
    repo
}

fn commit_file(dir: &Path, name: &str, text: &str) {
    std::fs::write(dir.join(name), text).unwrap();
    git(dir, &["add", name]);
    git(dir, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", name]);
}

/// A repository that ignores build folders, with a finished card's worktree that has a commit of its own and an
/// ignored `debug.log`, which a take-over's `git clean` removes.
fn finished_card_worktree(tmp: &Path) -> (PathBuf, PathBuf, worktree::Worktree) {
    let repo = repo(tmp);
    commit_file(&repo, ".gitignore", ".env\ntarget/\nnode_modules/\n*.log\n");
    let base = tmp.join("wt");
    let a = worktree::ensure(&repo, &base, "KADE-1", "Old card", "main").unwrap();
    commit_file(&a.path, "old-card.txt", "done\n");
    std::fs::create_dir_all(a.path.join("target/debug")).unwrap();
    std::fs::write(a.path.join("target/debug/warm.rlib"), "compiled").unwrap();
    std::fs::write(a.path.join("debug.log"), "the old card's log\n").unwrap();
    (repo, base, a)
}

/// A temporary folder by its real path, as git prints it and as the fake git compares it.
fn real_tempdir() -> tempfile::TempDir {
    tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap()
}

/// Runs a case's `body` on a thread of its own, so a lock taken twice fails the case instead of hanging the test.
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

fn finishes_soon<T>(thread: &ScopedJoinHandle<T>) -> bool {
    let since = Instant::now();
    while !thread.is_finished() && since.elapsed() < SOON {
        std::thread::sleep(Duration::from_millis(10));
    }
    thread.is_finished()
}

/// Writes an executable script through a child sh, so a fork elsewhere in the test process never holds it open
/// for writing ("Text file busy").
fn write_script(path: &Path, script: &str) {
    use std::io::Write;
    let mut sh = Command::new("sh").args(["-c", r#"cat > "$1" && chmod 755 "$1""#, "sh"]).arg(path)
        .stdin(std::process::Stdio::piped()).spawn().unwrap();
    sh.stdin.take().unwrap().write_all(script.as_bytes()).unwrap();
    assert!(sh.wait().unwrap().success(), "can't write {}", path.display());
}

/// The git first on PATH, and the folder of its rules: one file of shell lines per git command (`worktree-add` for
/// `git worktree add`), which run first in Hersir's own calls of that command, with `$dir` the folder of `-C`.
/// Lines that end in `exit` run the real git themselves (`"$real" "$@"`). Git's own child processes find the real
/// git on the exec path it puts first, and they get the settings in their environment, not as `-c` arguments.
#[derive(Clone)]
struct FakeGit {
    rules: PathBuf,
}

impl FakeGit {
    fn install(tmp: &Path) -> FakeGit {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let real = std::env::split_paths(&path).map(|d| d.join("git")).find(|g| g.is_file()).expect("git on PATH");
        let (bin, rules) = (tmp.join("bin"), tmp.join("rules"));
        std::fs::create_dir(&bin).unwrap();
        std::fs::create_dir(&rules).unwrap();
        let mut prefix = String::new();
        for (i, s) in worktree::NO_HOOKS.iter().chain(["-C"].iter()).enumerate() {
            prefix.push_str(&format!("[ \"${}\" = '{s}' ] && ", i + 1));
        }
        write_script(&bin.join("git"), &format!(
            "#!/bin/sh\nreal='{real}'\nif {prefix}true; then\n  dir=$6\n  what=$7\n  if [ \"$7\" = worktree ]; then what=\"worktree-$8\"; fi\n  \
             if [ -f '{rules}/'\"$what\" ]; then . '{rules}/'\"$what\"; fi\nfi\nexec \"$real\" \"$@\"\n",
            real = real.display(), rules = rules.display()));
        let mut on_path = vec![bin];
        on_path.extend(std::env::split_paths(&path));
        // SAFETY: the only test in this binary sets it before it starts a thread, so no other thread reads the
        // environment meanwhile.
        unsafe { std::env::set_var("PATH", std::env::join_paths(on_path).unwrap()) };
        FakeGit { rules }
    }

    /// `lines` run first in each of Hersir's own `git <command>` (`worktree-add`, `checkout`, `clean`).
    fn on(&self, command: &str, lines: &str) {
        std::fs::write(self.rules.join(command), lines).unwrap();
    }

    /// `gate` holds up Hersir's own `git <command>` in the folder `dir`, before it does anything.
    fn gate(&self, command: &str, dir: &Path, gate: &Gate) {
        self.on(command, &format!("if [ \"$dir\" = '{}' ]; then\n{}fi\n", dir.display(), gate.lines()));
    }

    fn clear(&self) {
        for rule in std::fs::read_dir(&self.rules).unwrap() {
            std::fs::remove_file(rule.unwrap().path()).unwrap();
        }
    }
}

/// Shell lines that hold up what runs them until the case opens the gate (at most 2 minutes), and files to say so.
struct Gate(PathBuf);

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

fn worktree_adds_and_branch_switches_of_one_repository_never_overlap(fake: &FakeGit) {
    let tmp = real_tempdir();
    let (repo, base, _) = finished_card_worktree(tmp.path());
    let finished: Vec<_> = (0..4).map(|i| worktree::ensure(&repo, &base, &format!("DONE-{i}"), "t", "main").unwrap().path).collect();
    let log = tmp.path().join("turns");
    let turn = format!("echo start >> '{l}'\n\"$real\" \"$@\"\ns=$?\nsleep 0.05\necho end >> '{l}'\nexit $s\n", l = log.display());
    // the four adds and the four take-overs' branch switches, as the post-checkout hook saw them before GA-263
    fake.on("worktree-add", &turn);
    fake.on("checkout", &turn);
    let started = together(8, |i| match finished.get(i) {
        Some(from) => worktree::reuse(&repo, from, &base, &format!("TOOK-{i}"), "t", "main", &[]),
        None => worktree::ensure(&repo, &base, &format!("NEW-{i}"), "t", "main"),
    });
    for s in started {
        s.unwrap();
    }
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "start\nend\n".repeat(8));
}

fn every_worktree_command_waits_for_the_add_that_is_running_by_any_spelling_of_the_repository(fake: &FakeGit) {
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
    let gate = Gate::new(tmp.path());
    fake.gate("worktree-add", &repo, &gate);
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
}

fn a_take_overs_branch_switch_holds_the_lock(fake: &FakeGit) {
    let tmp = real_tempdir();
    let (repo, base, a) = finished_card_worktree(tmp.path());
    let gate = Gate::new(tmp.path());
    // the worktree has moved to the new card's folder when it switches branch
    fake.gate("checkout", &base.join("KADE-2"), &gate);
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
}

fn a_copys_worktree_add_holds_the_lock(fake: &FakeGit) {
    let tmp = real_tempdir();
    let repo = repo(tmp.path());
    let gate = Gate::new(tmp.path());
    fake.gate("worktree-add", &repo, &gate);
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
}

fn cards_of_another_repository_do_not_wait(fake: &FakeGit) {
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
    let gate = Gate::new(&one);
    fake.gate("worktree-add", &busy, &gate);
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
}

fn a_take_overs_git_clean_runs_outside_the_lock(fake: &FakeGit) {
    let tmp = real_tempdir();
    let (repo, base, a) = finished_card_worktree(tmp.path());
    let wt = base.join("KADE-2");
    let gate = Gate::new(tmp.path());
    fake.gate("clean", &wt, &gate);
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
}

#[test]
fn worktree_commands_of_one_repository_take_turns_and_other_work_does_not_wait() {
    let tmp = real_tempdir();
    let fake = FakeGit::install(tmp.path());
    let cases: [(&str, fn(&FakeGit)); 6] = [
        ("worktree_adds_and_branch_switches_of_one_repository_never_overlap", worktree_adds_and_branch_switches_of_one_repository_never_overlap),
        ("every_worktree_command_waits_for_the_add_that_is_running_by_any_spelling_of_the_repository",
         every_worktree_command_waits_for_the_add_that_is_running_by_any_spelling_of_the_repository),
        ("a_take_overs_branch_switch_holds_the_lock", a_take_overs_branch_switch_holds_the_lock),
        ("a_copys_worktree_add_holds_the_lock", a_copys_worktree_add_holds_the_lock),
        ("cards_of_another_repository_do_not_wait", cards_of_another_repository_do_not_wait),
        ("a_take_overs_git_clean_runs_outside_the_lock", a_take_overs_git_clean_runs_outside_the_lock),
    ];
    let mut failed = vec![];
    for (name, case) in cases {
        fake.clear();
        let fake = fake.clone();
        if std::panic::catch_unwind(|| no_hang(move || case(&fake))).is_err() {
            failed.push(name);
        }
    }
    assert!(failed.is_empty(), "failed (each one's panic is above): {failed:?}");
}
