//! GA-369 QA: the log of every process Hersir ends (`gizai_agents::signal_log`). One line each: the time (UTC), the
//! signal, the pid or process group, the process's name, its cgroup on Linux, and why; only what reached a process.
//! `Tree::end_for` and `kill_child` write it; a tree's own end (`Tree::end`) says why too. What the slice's cleanup
//! ends, and that it never signals a program outside Hersir, run_leftovers_test.rs checks in a slice of its own.
use gizai_agents::signal_log::{self, About, Sent};

#[test]
fn the_time_is_utc_to_the_second() {
    assert_eq!(signal_log::utc(0), "1970-01-01T00:00:00Z");
    assert_eq!(signal_log::utc(1_795_690_752_000), "2026-11-26T10:59:12Z");
    assert_eq!(signal_log::utc(1_795_690_752_999), "2026-11-26T10:59:12Z", "to the second");
    assert_eq!(signal_log::utc(951_868_799_000), "2000-02-29T23:59:59Z", "a leap day");
    assert_eq!(signal_log::utc(-1_000), "1969-12-31T23:59:59Z");
}

#[test]
fn a_line_says_when_what_to_which_process_where_and_why() {
    let sent = Sent {
        at_ms: 1_795_690_752_000, pid: 12345, group: false, name: "chromium".into(),
        cgroup: Some("/user.slice/user-1000.slice/user@1000.service/gizai.slice/gizai-runs.slice/gizai-runs-1a2b3c4d.slice/gizai-0123456789abcdef.scope".into()),
        signal: "SIGTERM".into(), why: "its run had ended, and it still ran in the run's scope".into(),
    };
    assert_eq!(sent.line(), "2026-11-26T10:59:12Z gizai: SIGTERM to pid 12345 (chromium) in /user.slice/user-1000.slice/user@1000.service/gizai.slice/gizai-runs.slice/gizai-runs-1a2b3c4d.slice/gizai-0123456789abcdef.scope: its run had ended, and it still ran in the run's scope");
    // a process group, named after its leader; no cgroup where the system has none
    let group = Sent { group: true, cgroup: None, name: "claude".into(), signal: "SIGINT".into(), why: "its run was stopped (Stop, or Hersir quitting)".into(), ..sent };
    assert_eq!(group.line(), "2026-11-26T10:59:12Z gizai: SIGINT to process group 12345 (claude): its run was stopped (Stop, or Hersir quitting)");
}

#[test]
fn record_keeps_the_line_for_recent() {
    // a pid no process has, so no other test's line is mistaken for it
    let about = About { pid: u32::MAX - 7, name: "unnamed".into(), cgroup: None };
    signal_log::record(&about, false, "SIGKILL", "a test's own line");
    let mine: Vec<Sent> = signal_log::recent().into_iter().filter(|s| s.pid == about.pid).collect();
    assert_eq!(mine.len(), 1);
    assert_eq!((mine[0].signal.as_str(), mine[0].why.as_str(), mine[0].group), ("SIGKILL", "a test's own line", false));
    assert!(mine[0].line().ends_with(&format!("gizai: SIGKILL to pid {} (unnamed): a test's own line", about.pid)), "{}", mine[0].line());
}

#[cfg(unix)]
mod unix {
    use std::time::{Duration, Instant};

    use gizai_agents::os::{self, End, Tree};
    use gizai_agents::signal_log::{self, Sent};

    fn lines_for(pid: u32) -> Vec<Sent> {
        signal_log::recent().into_iter().filter(|s| s.pid == pid).collect()
    }

    fn until(what: &str, mut ok: impl FnMut() -> bool) {
        let t0 = Instant::now();
        while !ok() {
            assert!(t0.elapsed() < Duration::from_secs(10), "waited 10 s for {what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[cfg(target_os = "linux")]
    fn cgroup_line(pid: u32) -> Option<String> {
        let text = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
        text.lines().find_map(|l| l.strip_prefix("0::")).map(|c| c.trim().to_string())
    }

    #[test]
    fn ending_a_tree_logs_one_line_for_its_group_with_its_leaders_name_and_cgroup_and_why_and_an_ended_one_none() {
        let mut cmd = std::process::Command::new("sleep");
        cmd.arg("30");
        let (mut child, tree) = os::spawn_agents_tree(&mut cmd).unwrap();
        let pid = child.id();
        until("sleep runs", || os::process_name(pid).as_deref() == Some("sleep"));
        #[cfg(target_os = "linux")]
        let cgroup = cgroup_line(pid);
        tree.end_for(End::Terminate, "a worktree's setup command reached its time limit");
        child.wait().unwrap();
        let lines = lines_for(pid);
        assert_eq!(lines.len(), 1, "{lines:?}");
        let l = &lines[0];
        assert_eq!((l.group, l.name.as_str(), l.signal.as_str(), l.why.as_str()),
                   (true, "sleep", "SIGTERM", "a worktree's setup command reached its time limit"));
        #[cfg(target_os = "linux")]
        assert_eq!(l.cgroup, cgroup, "where it ran, read just before");
        #[cfg(target_os = "macos")]
        assert_eq!(l.cgroup, None);
        assert!(l.line().contains(&format!(" gizai: SIGTERM to process group {pid} (sleep)")), "{}", l.line());
        // the group has ended: a second signal reaches nothing and writes nothing
        tree.end_for(End::Kill, "again");
        assert_eq!(lines_for(pid).len(), 1);
    }

    #[test]
    fn a_trees_plain_end_says_why_too_and_each_signal_of_a_stop_has_its_line() {
        let mut cmd = std::process::Command::new("sh");
        // a program that ignores SIGINT, as a CLI busy with a tool can
        cmd.args(["-c", "trap '' INT; exec sleep 30"]);
        let (mut child, tree) = os::spawn_agents_tree(&mut cmd).unwrap();
        let pid = child.id();
        until("sleep runs", || os::process_name(pid).as_deref() == Some("sleep"));
        tree.end_for(End::Interrupt, "its run was stopped (Stop, or Hersir quitting)");
        std::thread::sleep(Duration::from_millis(100));
        tree.end(End::Kill);
        child.wait().unwrap();
        let lines: Vec<(String, String)> = lines_for(pid).into_iter().map(|s| (s.signal, s.why)).collect();
        assert_eq!(lines, [
            ("SIGINT".to_string(), "its run was stopped (Stop, or Hersir quitting)".to_string()),
            ("SIGKILL".to_string(), "Hersir ended a program it started, with everything that program started".to_string()),
        ]);
    }

    #[test]
    fn a_program_hersir_gives_up_on_is_logged_once_and_one_that_has_ended_isnt() {
        let mut child = os::command("sleep").arg("30").spawn().unwrap();
        let pid = child.id();
        until("sleep runs", || os::process_name(pid).as_deref() == Some("sleep"));
        assert!(os::kill_child(&mut child, "git gave the Editor no answer in time"));
        child.wait().unwrap();
        let lines = lines_for(pid);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!((lines[0].group, lines[0].name.as_str(), lines[0].signal.as_str(), lines[0].why.as_str()),
                   (false, "sleep", "SIGKILL", "git gave the Editor no answer in time"));
        // it has ended (and been waited for): nothing is sent, nothing logged
        assert!(!os::kill_child(&mut child, "again"));
        assert_eq!(lines_for(pid).len(), 1);
    }

    #[tokio::test]
    async fn tokios_child_hersir_gives_up_on_is_logged_too() {
        let mut child = tokio::process::Command::new("sleep").arg("30").spawn().unwrap();
        let pid = child.id().unwrap();
        until("sleep runs", || os::process_name(pid).as_deref() == Some("sleep"));
        os::start_kill_child(&mut child, "it couldn't be put in its Job Object");
        let _ = child.wait().await;
        let lines = lines_for(pid);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!((lines[0].signal.as_str(), lines[0].why.as_str()), ("SIGKILL", "it couldn't be put in its Job Object"));
        os::start_kill_child(&mut child, "again");
        assert_eq!(lines_for(pid).len(), 1);
    }

    #[test]
    fn a_tree_for_a_pid_of_1_or_hersirs_own_group_is_never_signalled() {
        // Tree::led_by is how a start after a crash ends what a previous Hersir left (end_orphan_group)
        for pid in [0, 1, std::process::id()] {
            Tree::led_by(pid).end_for(End::Terminate, "a test: never sent");
            assert!(lines_for(pid).iter().all(|s| s.why != "a test: never sent"), "{pid}");
        }
        // SAFETY: getpgrp only reads.
        let own_group = unsafe { libc::getpgrp() } as u32;
        Tree::led_by(own_group).end_for(End::Terminate, "a test: never sent");
        assert!(lines_for(own_group).iter().all(|s| s.why != "a test: never sent"));
    }

    #[test]
    fn an_orphan_group_is_ended_only_in_its_worktree_and_a_program_elsewhere_is_never_signalled() {
        let (worktree, elsewhere) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        // a program outside Hersir (a browser you started) in a group of its own, and a run a previous Hersir left
        let start = |dir: &std::path::Path| {
            use std::os::unix::process::CommandExt;
            let mut c = std::process::Command::new("sleep");
            c.arg("30").current_dir(dir).process_group(0);
            c.spawn().unwrap()
        };
        let (mut browser, mut left) = (start(elsewhere.path()), start(worktree.path()));
        let (b, l) = (browser.id(), left.id());
        until("both run sleep", || os::process_name(b).as_deref() == Some("sleep") && os::process_name(l).as_deref() == Some("sleep"));
        assert!(!gizai_agents::process::end_orphan_group(b, worktree.path()), "it works in another folder");
        assert!(gizai_agents::process::end_orphan_group(l, worktree.path()));
        left.wait().unwrap();
        let lines = lines_for(l);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!((lines[0].group, lines[0].signal.as_str(), lines[0].why.as_str()),
                   (true, "SIGTERM", "a previous Hersir left this run running (it crashed or was killed)"));
        assert!(browser.try_wait().unwrap().is_none(), "the program outside Hersir runs on");
        assert!(lines_for(b).is_empty(), "and was never signalled: {:?}", lines_for(b));
        let _ = browser.kill();
        browser.wait().unwrap();
    }
}
