//! GA-202: Memory for agent runs in the agents crate (`gizai_agents::run_memory`): the computer's memory, the slice's
//! name and limits, `RunsMemory` before Gizai's start, and on Linux with a systemd user session a real slice of the
//! test's own, named after its own temporary folder, never the slice of the Gizai the tests run in. Each test's slice is
//! stopped and reverted when it ends. Where there is no systemd user session (CI runners, another init system) the slice
//! tests say so and pass; GIZAI_TEST_SLICE=1 makes that a failure, for a run on a computer that has one.
#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::time::Duration;

use gizai_agents::run_memory::{self as mem, GB, Reading, RunsMemory};

#[test]
fn meminfo_and_cgroup_values_are_read_in_bytes() {
    let meminfo = "MemTotal:       64968196 kB\nMemFree:        29818904 kB\nMemAvailable:   47847760 kB\nHugePages_Total:       0\n";
    assert_eq!(mem::meminfo_value(meminfo, "MemTotal"), Some(64968196 * 1024));
    assert_eq!(mem::meminfo_value(meminfo, "MemAvailable"), Some(47847760 * 1024));
    // a value without kB is a count, taken as it is; a key that is only the start of another isn't it
    assert_eq!(mem::meminfo_value(meminfo, "HugePages_Total"), Some(0));
    assert_eq!(mem::meminfo_value(meminfo, "Mem"), None);
    assert_eq!(mem::meminfo_value(meminfo, "SwapTotal"), None);

    let stat = "anon 2147483648\nfile 8589934592\nkernel 1048576\nanon_thp 0\n";
    assert_eq!(mem::stat_value(stat, "anon"), Some(2 * GB), "anon, without the file cache");
    assert_eq!(mem::stat_value(stat, "file"), Some(8 * GB));
    assert_eq!(mem::stat_value(stat, "anon_thp"), Some(0));
    let events = "low 0\nhigh 12\nmax 3\noom 1\noom_kill 1\noom_group_kill 0\n";
    assert_eq!(mem::stat_value(events, "oom_kill"), Some(1));
    assert_eq!(mem::stat_value(events, "oom"), Some(1));
    assert_eq!(mem::stat_value(events, "swap"), None);
}

#[test]
fn the_computers_memory_and_what_it_has_free_are_known_and_free_is_at_most_all_of_it() {
    if !cfg!(any(target_os = "linux", target_os = "macos", windows)) {
        return;
    }
    let total = mem::total().expect("this system says how much memory it has");
    let free = mem::available().expect("this system says how much memory it has free");
    assert!(total >= GB, "{total}");
    assert!(free <= total, "{free} of {total}");
}

#[test]
fn memory_max_is_the_limit_and_memory_high_90_percent_of_it_in_whole_pages() {
    for gb in [1, 2, 3, 8, 31, 32, 64] {
        let max = gb * GB;
        let (m, h) = mem::limits_for(max);
        assert_eq!(m, max, "{gb} GB is whole pages already");
        assert_eq!(h % 4096, 0, "{gb} GB: {h}");
        assert!(h <= max / 10 * 9 && max / 10 * 9 - h < 4096, "{gb} GB: MemoryHigh {h} is 90% of {max}, rounded down to a page");
    }
    assert_eq!(mem::limits_for(GB), (1_073_741_824, 966_365_184));
    // a limit that isn't whole pages is rounded down, as the kernel keeps it
    assert_eq!(mem::limits_for(GB + 100).0, GB);
}

#[test]
fn the_slice_is_named_after_the_data_folder_so_a_test_gizai_has_its_own() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    let name = mem::slice_name(&a);
    let hex = name.strip_prefix("gizai-runs-").and_then(|n| n.strip_suffix(".slice")).unwrap_or_else(|| panic!("{name}"));
    assert!(hex.len() == 8 && hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "{name}");
    assert_eq!(mem::slice_name(&a), name, "the same name each time");
    assert_ne!(mem::slice_name(&b), name, "another data folder, another slice");
    // the same folder however it is spelled
    assert_eq!(mem::slice_name(&b.join("..").join("a")), name);
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&a, tmp.path().join("link")).unwrap();
        assert_eq!(mem::slice_name(&tmp.path().join("link")), name);
    }
}

#[test]
fn before_gizais_start_nothing_is_enforced_or_read_and_a_fake_reading_stands_in() {
    let m = RunsMemory::default();
    assert!(m.slice().is_none(), "a test's Gizai keeps its processes outside systemd");
    assert!(!m.enforced());
    assert_eq!(m.limit(), None);
    assert_eq!(m.reading(), Reading::default(), "its runs never wait for the test machine's memory");
    assert_eq!(m.set_limit(4 * GB), Ok(()));
    assert_eq!(m.limit(), None, "a new limit before the start changes nothing");
    let short = Reading { used: Some(7 * GB), available: Some(GB) };
    m.fake_reading(Some(short));
    assert_eq!(m.reading(), short);
    m.fake_reading(None);
    assert_eq!(m.reading(), Reading::default());
}

#[cfg(not(target_os = "linux"))]
#[test]
fn only_linux_has_a_slice_elsewhere_it_says_why() {
    let tmp = tempfile::tempdir().unwrap();
    let why = mem::Slice::set_up(&mem::slice_name(tmp.path()), 2 * GB).unwrap_err();
    assert!(why.contains("only Linux"), "{why}");
}

// ---- Linux with a systemd user session: a slice of the test's own ----

/// The test's own slice, stopped and reverted when it goes. None where this Linux has no systemd user session.
#[cfg(target_os = "linux")]
struct Own(mem::Slice);

#[cfg(target_os = "linux")]
impl Drop for Own {
    fn drop(&mut self) {
        let _ = self.0.remove();
    }
}

#[cfg(target_os = "linux")]
fn own_slice(dir: &Path, max: u64) -> Option<Own> {
    match mem::Slice::set_up(&mem::slice_name(dir), max) {
        Ok(s) => Some(Own(s)),
        Err(why) => {
            assert!(std::env::var_os("GIZAI_TEST_SLICE").is_none(), "GIZAI_TEST_SLICE is set, but there is no slice: {why}");
            eprintln!("skipped: no slice for agent runs on this Linux: {why}");
            None
        }
    }
}

/// The cgroup folder the process `pid` runs in.
#[cfg(target_os = "linux")]
fn cgroup_of(pid: u32) -> PathBuf {
    let text = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).unwrap();
    Path::new("/sys/fs/cgroup").join(text.lines().find_map(|l| l.strip_prefix("0::")).unwrap().trim().trim_start_matches('/'))
}

/// `/proc/<pid>/stat`'s fields after the program's name: state, ppid, pgrp, … (nice is the 17th of these).
#[cfg(target_os = "linux")]
fn stat_fields(pid: u32) -> Vec<String> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    text[text.rfind(')').unwrap() + 2..].split_whitespace().map(String::from).collect()
}

/// Waits until the process `pid` runs `comm` (systemd-run made its scope and became the program).
#[cfg(target_os = "linux")]
fn wait_until_runs(pid: u32, comm: &str) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default().trim() != comm {
        assert!(std::time::Instant::now() < until, "{pid} never ran {comm}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[cfg(target_os = "linux")]
fn alive(pid: u32) -> bool {
    // a zombie has ended; only its parent's wait is missing
    std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|_| stat_fields(pid)[0] != "Z")
}

#[cfg(target_os = "linux")]
fn systemctl_show(name: &str) -> String {
    let out = std::process::Command::new("systemctl").args(["--user", "show", name, "-p", "MemoryMax", "-p", "MemoryHigh", "-p", "MemorySwapMax"])
        .output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[cfg(target_os = "linux")]
#[test]
fn a_slice_of_its_own_gets_memory_max_memory_high_at_90_percent_and_no_swap_and_a_new_limit_changes_both_at_once() {
    let tmp = tempfile::tempdir().unwrap();
    let Some(s) = own_slice(tmp.path(), GB) else { return };
    let (max, high) = mem::limits_for(GB);
    assert_eq!(s.0.name(), mem::slice_name(tmp.path()));
    assert!(s.0.dir().ends_with(s.0.name()) && s.0.dir().starts_with("/sys/fs/cgroup/user.slice"), "{}", s.0.dir().display());
    assert_eq!(s.0.limits(), (Some(max), Some(high)), "the kernel's memory.max and memory.high");
    assert_eq!(std::fs::read_to_string(s.0.dir().join("memory.swap.max")).unwrap().trim(), "0", "no swap");
    let shown = systemctl_show(s.0.name());
    for line in [format!("MemoryMax={max}"), format!("MemoryHigh={high}"), "MemorySwapMax=0".into()] {
        assert!(shown.lines().any(|l| l == line), "systemd has {line}: {shown}");
    }
    // the slice's processes use nothing yet
    assert_eq!(s.0.used(), Some(0));

    // a new limit: both change at once, without a new slice
    s.0.set_limit(3 * GB).unwrap();
    let (max3, high3) = mem::limits_for(3 * GB);
    assert_eq!(s.0.limits(), (Some(max3), Some(high3)));
    let shown = systemctl_show(s.0.name());
    assert!(shown.lines().any(|l| l == format!("MemoryMax={max3}")) && shown.lines().any(|l| l == format!("MemoryHigh={high3}")), "{shown}");
}

#[cfg(target_os = "linux")]
#[test]
fn each_process_starts_in_a_scope_of_its_own_in_the_slice_with_its_pid_process_group_and_priority_and_the_test_stays_outside() {
    let tmp = tempfile::tempdir().unwrap();
    let Some(s) = own_slice(tmp.path(), 2 * GB) else { return };
    let start = || {
        let mut cmd = s.0.command_for("sleep".as_ref(), None).expect("sleep is installed");
        cmd.arg("30").stdin(std::process::Stdio::null());
        // as a worktree's preparation starts its commands: a process group of its own, at low priority
        gizai_agents::os::spawn_tree(&mut cmd, true).unwrap()
    };
    let (mut one, tree_one) = start();
    let (mut two, tree_two) = start();
    for pid in [one.id(), two.id()] {
        wait_until_runs(pid, "sleep");
    }
    let (at_one, at_two) = (cgroup_of(one.id()), cgroup_of(two.id()));
    for at in [&at_one, &at_two] {
        assert_eq!(at.parent(), Some(s.0.dir()), "a scope right inside the slice: {}", at.display());
        assert!(at.extension().is_some_and(|e| e == "scope"), "{}", at.display());
    }
    assert_ne!(at_one, at_two, "a scope each");
    // the same process: its pid leads its own process group, and keeps the nice 10 it was started with
    let own_nice: i64 = stat_fields(std::process::id())[16].parse().unwrap();
    for pid in [one.id(), two.id()] {
        let f = stat_fields(pid);
        assert_eq!(f[2], pid.to_string(), "its own process group");
        assert_eq!(f[16].parse::<i64>().unwrap(), (own_nice + 10).min(19), "low priority kept");
    }
    // what the slice's processes use is read from its memory.stat
    assert!(s.0.used().is_some());
    // the test itself (Gizai's own process in a real Gizai) stays outside the slice
    assert!(!cgroup_of(std::process::id()).starts_with(s.0.dir()));
    // ending a tree ends its scope too
    tree_one.end(gizai_agents::os::End::Kill);
    one.wait().unwrap();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while at_one.exists() {
        assert!(std::time::Instant::now() < until, "the scope stays after its process ended: {}", at_one.display());
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(alive(two.id()), "the other process keeps running");
    tree_two.end(gizai_agents::os::End::Kill);
    two.wait().unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn arguments_and_environment_reach_a_program_in_the_slice_unchanged() {
    // A run's arguments (Claude Code's system prompt, another CLI's prompt with the card's text, a setup command for
    // bash -c) must reach it as they are, as outside the slice: `$`, `${…}` and `%` included.
    let tmp = tempfile::tempdir().unwrap();
    let Some(s) = own_slice(tmp.path(), 2 * GB) else { return };
    let args = ["a$$b", "echo $$", "${HOME}", "$HOME", "${GIZAI_TEST_UNSET_VARIABLE}x", "cost: $5", "100%", "%h %u", "two words",
                "line one\nline two", "", "ünïcødé ✓"];
    let show = "import json, os, sys; print(json.dumps([sys.argv[1:], os.environ.get('GIZAI_TEST_VALUE')]))";
    let run = |mut cmd: std::process::Command| {
        let out = cmd.arg("-c").arg(show).args(args).env("GIZAI_TEST_VALUE", "a $b ${c} %d").output().unwrap();
        assert!(out.status.success(), "{out:?}");
        serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap()
    };
    let outside = run(std::process::Command::new("python3"));
    assert_eq!(outside, serde_json::json!([args, "a $b ${c} %d"]), "as a program gets them outside the slice");
    let inside = run(s.0.command_for("python3".as_ref(), None).unwrap());
    assert_eq!(inside, outside, "systemd-run changed the arguments");
}

/// A program that asks for 2 GB at once and touches all of it (a big build).
#[cfg(target_os = "linux")]
const ASKS_2_GB: &str = "b = b'x' * (2 << 30)";

#[cfg(target_os = "linux")]
#[test]
fn at_a_1_gb_limit_a_command_that_asks_for_2_gb_is_ended_the_watch_sees_it_and_the_other_process_and_the_test_keep_running() {
    let tmp = tempfile::tempdir().unwrap();
    let Some(s) = own_slice(tmp.path(), GB) else { return };
    // another run in the slice
    let mut other = s.0.command_for("sleep".as_ref(), None).unwrap();
    let (mut other, other_tree) = gizai_agents::os::spawn_tree(other.arg("600").stdin(std::process::Stdio::null()), false).unwrap();
    wait_until_runs(other.id(), "sleep");

    let mut big = s.0.command_for("python3".as_ref(), None).expect("python3 is installed");
    big.args(["-c", ASKS_2_GB]).stdin(std::process::Stdio::null());
    let (mut big, _tree) = gizai_agents::os::spawn_tree(&mut big, false).unwrap();
    let heard = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u64>::new()));
    let h2 = heard.clone();
    let watch = mem::OomWatch::start(&s.0, big.id(), move |n| h2.lock().unwrap().push(n));
    let status = big.wait().unwrap();
    use std::os::unix::process::ExitStatusExt;
    assert_eq!(status.signal(), Some(libc::SIGKILL), "Linux ended it at the limit: {status:?}");
    assert_eq!(watch.finish(), 1, "one process ended at the limit");
    assert_eq!(*heard.lock().unwrap(), [1], "heard as it happened");
    assert!(alive(other.id()), "the other process in the slice keeps running");
    // Linux frees the ended process's memory a moment after the kill, so wait for it (up to 5 s)
    let until = std::time::Instant::now() + Duration::from_secs(5);
    while !s.0.used().is_some_and(|u| u < GB / 2) && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(s.0.used().is_some_and(|u| u < GB / 2), "its memory is free again: {:?}", s.0.used());
    other_tree.end(gizai_agents::os::End::Kill);
    other.wait().unwrap();
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn a_cli_started_in_the_slice_runs_there_and_its_events_say_when_linux_ended_a_process_in_it_at_the_limit() {
    use gizai_agents::process::{AFTER_RESULT, Caps, ClaudeLines, Exec, OOM_KILL, spawn_exec};
    use gizai_agents::stream::RunEvent;
    let tmp = tempfile::tempdir().unwrap();
    let Some(s) = own_slice(tmp.path(), GB) else { return };
    // a CLI whose command (a build) asks for 2 GB; the CLI itself goes on, like Claude Code after a failed Bash call
    let cli = tmp.path().join("cli.sh");
    let fake = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fake-claude.sh");
    let script = format!("#!/usr/bin/env bash\ncat /proc/$$/cgroup > '{}'\npython3 -c \"{ASKS_2_GB}\"\necho \"build: $?\" >&2\nexec '{fake}' \"$@\"\n",
                         tmp.path().join("cgroup").display());
    // written by a child, so this process never holds the script open for writing (ETXTBSY in other tests' children)
    assert!(std::process::Command::new("sh").args(["-c", "printf '%s' \"$1\" > \"$2\" && chmod +x \"$2\"", "sh", &script])
        .arg(&cli).status().unwrap().success());
    let exec = Exec { bin: cli, args: vec!["-p".into()], env: vec![], stdin: "p".into(), slice: Some(s.0.clone()) };
    let log = tmp.path().join("r.jsonl");
    let mut h = spawn_exec::<RunEvent, _>(&exec, tmp.path(), &log, Caps { max_time: Duration::from_secs(120), max_tool_calls: 80, after_result: AFTER_RESULT }, ClaudeLines).unwrap();
    let mut evs = vec![];
    while let Some(e) = h.events.recv().await {
        evs.push(e);
    }
    let cgroup = std::fs::read_to_string(tmp.path().join("cgroup")).unwrap();
    let at = Path::new("/sys/fs/cgroup").join(cgroup.lines().find_map(|l| l.strip_prefix("0::")).unwrap().trim().trim_start_matches('/'));
    assert_eq!(at.parent(), Some(s.0.dir()), "the CLI ran in a scope in the slice: {cgroup}");
    let others: Vec<&str> = evs.iter().filter_map(|e| match e { RunEvent::Other { raw_type } => Some(raw_type.as_str()), _ => None }).collect();
    assert!(others.contains(&format!("{OOM_KILL}1").as_str()), "{others:?}");
    assert_eq!(others.last(), Some(&"exit:0"), "the CLI itself finished: {others:?}");
    assert!(evs.iter().any(|e| matches!(e, RunEvent::Result { .. })), "{evs:?}");
    assert!(std::fs::read_to_string(log.with_extension("stderr.log")).unwrap().contains("build: 137"), "the build was killed");
}

#[cfg(target_os = "linux")]
#[test]
fn a_worktrees_setup_command_runs_in_the_slice_and_a_failure_at_the_limit_says_so() {
    use gizai_agents::prepare::{Prepare, prepare_in};
    let tmp = tempfile::tempdir().unwrap();
    let Some(s) = own_slice(tmp.path(), GB) else { return };
    let (main, wt) = (tmp.path().join("main"), tmp.path().join("wt"));
    std::fs::create_dir_all(&main).unwrap();
    std::fs::create_dir_all(&wt).unwrap();
    let out = tmp.path().join("setup-cgroup");
    let p = Prepare { copy: vec![], install: false, setup: Some(format!("cat /proc/self/cgroup > '{}'", out.display())) };
    prepare_in(&main, &wt, &p, None, None, Some(&s.0)).unwrap();
    let cgroup = std::fs::read_to_string(&out).unwrap();
    let at = Path::new("/sys/fs/cgroup").join(cgroup.lines().find_map(|l| l.strip_prefix("0::")).unwrap().trim().trim_start_matches('/'));
    assert_eq!(at.parent(), Some(s.0.dir()), "{cgroup}");
    // without the slice it runs where the caller runs, as before
    prepare_in(&main, &wt, &p, None, None, None).unwrap();
    assert!(!std::fs::read_to_string(&out).unwrap().contains(s.0.name()));

    let p = Prepare { copy: vec![], install: false, setup: Some(format!("python3 -c \"{ASKS_2_GB}\"")) };
    let failed = prepare_in(&main, &wt, &p, None, None, Some(&s.0)).unwrap_err();
    assert!(failed.why.contains("Linux ended a process in it at the memory limit for agent runs (Settings → Runs)"), "{failed:?}");
}

// ---- GA-277: Memory per run, a MemoryMax of its own for each scope ----

#[test]
fn memory_per_run_is_none_until_gizais_start_and_never_more_than_the_limit_of_all_runs_also_after_that_is_lowered() {
    let m = RunsMemory::default();
    assert_eq!(m.run_limit(), None);
    m.set_run_limit(2 * GB);
    assert_eq!(m.run_limit(), None, "nothing before Gizai's start: a test's Gizai keeps its processes outside systemd");
    // enable only notes the slice's name and limit; nothing is set up until `slice`
    let tmp = tempfile::tempdir().unwrap();
    m.enable(&mem::slice_name(tmp.path()), 8 * GB);
    assert_eq!(m.run_limit(), Some(2 * GB));
    m.set_run_limit(12 * GB);
    assert_eq!(m.run_limit(), Some(8 * GB), "never more than the limit of all runs together");
    m.set_limit(6 * GB).unwrap();
    assert_eq!(m.run_limit(), Some(6 * GB), "also after that is lowered");
    m.set_run_limit(3 * GB);
    assert_eq!(m.run_limit(), Some(3 * GB));
    m.set_run_limit(0);
    assert_eq!(m.run_limit(), None, "0 is none");
    assert!(!m.enforced(), "nothing was set up");
}

#[test]
fn an_oom_kill_marker_gives_its_count_and_whether_the_runs_own_limit_ended_the_process() {
    use gizai_agents::process::{OOM_KILL, PER_RUN, oom_kill};
    use gizai_agents::run_memory::OomKill;
    assert_eq!((OOM_KILL, PER_RUN), ("oom_kill:", " per_run"));
    assert_eq!(oom_kill("oom_kill:1"), Some(OomKill { count: 1, own_limit: false }), "all runs together (GA-202), as before");
    assert_eq!(oom_kill("oom_kill:3 per_run"), Some(OomKill { count: 3, own_limit: true }), "the run's own limit");
    for not in ["oom_kill:", "oom_kill: per_run", "oom_kill:x", "oom_kill:1 per run", "exit:0", "leftover:12 sleep", "", "oom_kill:-1"] {
        assert_eq!(oom_kill(not), None, "{not:?}");
    }
}

/// A shell command that adds its scope's cgroup folder, memory.max and memory.high, as a process in the scope reads
/// them, to `out`.
#[cfg(target_os = "linux")]
fn record_scope(out: &Path) -> String {
    format!("c=$(sed -n 's/^0:://p' /proc/self/cgroup); echo \"$c $(cat /sys/fs/cgroup$c/memory.max) $(cat /sys/fs/cgroup$c/memory.high)\" >> '{}'",
            out.display())
}

/// The lines `record_scope` wrote: the cgroup folder, memory.max and memory.high.
#[cfg(target_os = "linux")]
fn recorded(out: &Path) -> Vec<(PathBuf, String, String)> {
    std::fs::read_to_string(out).unwrap_or_default().lines().map(|l| {
        let f: Vec<&str> = l.split(' ').collect();
        (Path::new("/sys/fs/cgroup").join(f[0].trim_start_matches('/')), f[1].to_string(), f[2].to_string())
    }).collect()
}

#[cfg(target_os = "linux")]
fn read_trim(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default().trim().to_string()
}

#[cfg(target_os = "linux")]
#[test]
fn with_a_limit_per_run_each_new_scope_gets_it_as_memory_max_in_whole_pages_without_memory_high_and_the_slice_keeps_its_own() {
    let tmp = tempfile::tempdir().unwrap();
    let Some(s) = own_slice(tmp.path(), 4 * GB) else { return };
    let (max, high) = mem::limits_for(4 * GB);
    let shown = systemctl_show(s.0.name());
    assert_eq!(s.0.run_limit(), None, "a slice straight from set_up gives its scopes no limit of their own");
    let sleep = |slice: &mem::Slice| {
        let (mut cmd, scope) = slice.scoped_for("sleep".as_ref(), None).expect("sleep is installed");
        let (child, tree) = gizai_agents::os::spawn_tree(cmd.arg("60").stdin(std::process::Stdio::null()), false).unwrap();
        wait_until_runs(child.id(), "sleep");
        assert_eq!(cgroup_of(child.id()), scope.dir(), "it runs in the scope Gizai named");
        (child, tree, scope)
    };
    // a scope from before, without a limit of its own
    let (mut before, before_tree, before_scope) = sleep(&s.0);
    assert_eq!((read_trim(&before_scope.dir().join("memory.max")), read_trim(&before_scope.dir().join("memory.high"))), ("max".into(), "max".into()));

    // 1 GB and 100 bytes: whole 4 KiB pages, as limits_for rounds
    let per_run = s.0.with_run_limit(GB + 100);
    assert_eq!(per_run.run_limit(), Some(GB));
    assert_eq!((per_run.name(), per_run.dir()), (s.0.name(), s.0.dir()), "the same slice");
    assert_eq!(s.0.run_limit(), None, "the slice it came from is unchanged");
    let (mut one, one_tree, one_scope) = sleep(&per_run);
    assert_eq!(read_trim(&one_scope.dir().join("memory.max")), GB.to_string(), "Memory per run is the scope's MemoryMax");
    assert_eq!(read_trim(&one_scope.dir().join("memory.high")), "max", "and it has no MemoryHigh of its own");
    // and a process in it reads the same
    let out = tmp.path().join("scope");
    let mut sh = per_run.command_for("sh".as_ref(), None).unwrap();
    assert!(sh.arg("-c").arg(record_scope(&out)).status().unwrap().success());
    let seen = recorded(&out);
    assert_eq!((seen.len(), seen[0].1.as_str(), seen[0].2.as_str()), (1, GB.to_string().as_str(), "max"), "{seen:?}");
    assert_eq!(seen[0].0.parent(), Some(s.0.dir()));

    // another limit per run: new scopes get it, the ones that already run keep what they had
    let (mut two, two_tree, two_scope) = sleep(&s.0.with_run_limit(2 * GB));
    assert_eq!(read_trim(&two_scope.dir().join("memory.max")), (2 * GB).to_string());
    assert_eq!(read_trim(&one_scope.dir().join("memory.max")), GB.to_string());
    assert_eq!(read_trim(&before_scope.dir().join("memory.max")), "max");

    // the slice keeps its MemoryMax, MemoryHigh (90%) and no swap
    assert_eq!(s.0.limits(), (Some(max), Some(high)));
    assert_eq!(read_trim(&s.0.dir().join("memory.swap.max")), "0");
    assert_eq!(systemctl_show(s.0.name()), shown);
    for (child, tree) in [(&mut before, before_tree), (&mut one, one_tree), (&mut two, two_tree)] {
        tree.end(gizai_agents::os::End::Kill);
        child.wait().unwrap();
    }
}

/// A program that asks for 1 GB at once and touches all of it.
#[cfg(target_os = "linux")]
const ASKS_1_GB: &str = "b = b'x' * (1 << 30)";

#[cfg(target_os = "linux")]
#[test]
fn a_process_that_grows_past_its_limit_per_run_is_ended_within_seconds_its_parent_and_another_scope_run_on_and_the_watch_says_it_was_its_own() {
    use std::os::unix::process::ExitStatusExt;
    let tmp = tempfile::tempdir().unwrap();
    // all runs together 4 GB (MemoryHigh 3.6 GB), each run 512 MB: one runaway process never gets near the slice's
    // MemoryHigh, where Linux would slow every run down
    let Some(s) = own_slice(tmp.path(), 4 * GB) else { return };
    let per_run = s.0.with_run_limit(GB / 2);
    // another run in the slice
    let mut other = per_run.command_for("sleep".as_ref(), None).unwrap();
    let (mut other, other_tree) = gizai_agents::os::spawn_tree(other.arg("600").stdin(std::process::Stdio::null()), false).unwrap();
    wait_until_runs(other.id(), "sleep");

    // a run: a shell (its agent) that starts a command asking for 1 GB, and goes on after it
    let out = tmp.path().join("after");
    let script = format!("python3 -c \"{ASKS_1_GB}\"; echo $? > '{}'; sleep 1; echo done >> '{}'", out.display(), out.display());
    let mut run = per_run.command_for("bash".as_ref(), None).unwrap();
    run.args(["-c", &script]).stdin(std::process::Stdio::null());
    let started = std::time::Instant::now();
    let (mut run, _tree) = gizai_agents::os::spawn_tree(&mut run, false).unwrap();
    let heard = std::sync::Arc::new(std::sync::Mutex::new(Vec::<mem::OomKill>::new()));
    let h2 = heard.clone();
    let watch = mem::OomWatch::start_by_limit(&per_run, run.id(), move |k| h2.lock().unwrap().push(k));
    let until = started + Duration::from_secs(30);
    while read_trim(&out).is_empty() {
        assert!(std::time::Instant::now() < until, "the command was never ended");
        std::thread::sleep(Duration::from_millis(20));
    }
    let ended_after = started.elapsed();
    assert_eq!(read_trim(&out), "137", "Linux ended the command (SIGKILL)");
    assert!(ended_after < Duration::from_secs(10), "ended within seconds, not slowed down: {ended_after:?}");
    // its parent, the run's own program, goes on (OOMPolicy=continue) and finishes as usual
    let status = run.wait().unwrap();
    assert_eq!((status.code(), status.signal()), (Some(0), None), "{status:?}");
    assert_eq!(read_trim(&out), "137\ndone");
    assert_eq!(watch.finish(), 1, "one process ended");
    assert_eq!(*heard.lock().unwrap(), [mem::OomKill { count: 1, own_limit: true }], "at the run's own limit, heard as it happened");
    // the slice's own limit was never reached: no oom at its level
    let local = std::fs::read_to_string(s.0.dir().join("memory.events.local")).unwrap();
    assert_eq!(mem::stat_value(&local, "oom"), Some(0), "{local}");
    // the other run keeps running
    assert!(alive(other.id()), "the process in the other scope keeps running");
    other_tree.end(gizai_agents::os::End::Kill);
    other.wait().unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn when_the_limit_of_all_runs_together_ends_a_process_the_watch_says_it_wasnt_the_runs_own() {
    use std::os::unix::process::ExitStatusExt;
    let tmp = tempfile::tempdir().unwrap();
    // a scope whose own limit (2 GB) is above the slice's (1 GB): the slice's is reached first
    let Some(s) = own_slice(tmp.path(), GB) else { return };
    let per_run = s.0.with_run_limit(2 * GB);
    let mut big = per_run.command_for("python3".as_ref(), None).unwrap();
    big.args(["-c", ASKS_2_GB]).stdin(std::process::Stdio::null());
    let (mut big, _tree) = gizai_agents::os::spawn_tree(&mut big, false).unwrap();
    let heard = std::sync::Arc::new(std::sync::Mutex::new(Vec::<mem::OomKill>::new()));
    let h2 = heard.clone();
    let watch = mem::OomWatch::start_by_limit(&per_run, big.id(), move |k| h2.lock().unwrap().push(k));
    let status = big.wait().unwrap();
    assert_eq!(status.signal(), Some(libc::SIGKILL), "{status:?}");
    assert_eq!(watch.finish(), 1);
    assert_eq!(*heard.lock().unwrap(), [mem::OomKill { count: 1, own_limit: false }], "the limit of all runs together");
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn a_cli_with_a_limit_per_run_runs_in_a_scope_with_it_and_its_events_say_linux_ended_a_process_at_the_runs_own_limit() {
    use gizai_agents::process::{AFTER_RESULT, Caps, ClaudeLines, Exec, oom_kill, spawn_exec};
    use gizai_agents::stream::RunEvent;
    let tmp = tempfile::tempdir().unwrap();
    let Some(s) = own_slice(tmp.path(), 4 * GB) else { return };
    let per_run = s.0.with_run_limit(GB / 2);
    // a CLI whose command (a test that hangs and grows) asks for 1 GB; the CLI itself goes on
    let cli = tmp.path().join("cli.sh");
    let fake = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fake-claude.sh");
    let script = format!("#!/usr/bin/env bash\n{}\npython3 -c \"{ASKS_1_GB}\"\necho \"build: $?\" >&2\nexec '{fake}' \"$@\"\n",
                         record_scope(&tmp.path().join("scope")));
    assert!(std::process::Command::new("sh").args(["-c", "printf '%s' \"$1\" > \"$2\" && chmod +x \"$2\"", "sh", &script])
        .arg(&cli).status().unwrap().success());
    let exec = Exec { bin: cli, args: vec!["-p".into()], env: vec![], stdin: "p".into(), slice: Some(per_run) };
    let log = tmp.path().join("r.jsonl");
    let mut h = spawn_exec::<RunEvent, _>(&exec, tmp.path(), &log, Caps { max_time: Duration::from_secs(120), max_tool_calls: 80, after_result: AFTER_RESULT }, ClaudeLines).unwrap();
    let mut evs = vec![];
    while let Some(e) = h.events.recv().await {
        evs.push(e);
    }
    let seen = recorded(&tmp.path().join("scope"));
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(seen[0].0.parent(), Some(s.0.dir()), "in a scope in the slice");
    assert_eq!((seen[0].1.as_str(), seen[0].2.as_str()), ((GB / 2).to_string().as_str(), "max"), "with Memory per run and no MemoryHigh");
    let others: Vec<&str> = evs.iter().filter_map(|e| match e { RunEvent::Other { raw_type } => Some(raw_type.as_str()), _ => None }).collect();
    assert!(others.contains(&"oom_kill:1 per_run"), "{others:?}");
    assert_eq!(others.iter().filter_map(|o| oom_kill(o)).map(|k| (k.count, k.own_limit)).collect::<Vec<_>>(), [(1, true)], "{others:?}");
    assert_eq!(others.last(), Some(&"exit:0"), "the CLI itself finished: {others:?}");
    assert!(std::fs::read_to_string(log.with_extension("stderr.log")).unwrap().contains("build: 137"));
}

#[cfg(target_os = "linux")]
#[test]
fn a_worktrees_setup_command_gets_the_limit_per_run_and_a_failure_there_says_it_was_its_own() {
    use gizai_agents::prepare::{Prepare, prepare_in};
    let tmp = tempfile::tempdir().unwrap();
    let Some(s) = own_slice(tmp.path(), 4 * GB) else { return };
    let per_run = s.0.with_run_limit(GB / 2);
    let (main, wt) = (tmp.path().join("main"), tmp.path().join("wt"));
    std::fs::create_dir_all(&main).unwrap();
    std::fs::create_dir_all(&wt).unwrap();
    let out = tmp.path().join("setup-scope");
    let p = Prepare { copy: vec![], install: false, setup: Some(record_scope(&out)) };
    prepare_in(&main, &wt, &p, None, None, Some(&per_run)).unwrap();
    let seen = recorded(&out);
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(seen[0].0.parent(), Some(s.0.dir()));
    assert_eq!((seen[0].1.as_str(), seen[0].2.as_str()), ((GB / 2).to_string().as_str(), "max"));

    let p = Prepare { copy: vec![], install: false, setup: Some(format!("python3 -c \"{ASKS_1_GB}\"")) };
    let failed = prepare_in(&main, &wt, &p, None, None, Some(&per_run)).unwrap_err();
    assert!(failed.why.contains(": Linux ended a process in it at its memory limit per run (Settings → Runs → Memory per run)"), "{failed:?}");
    assert!(!failed.why.contains("memory limit for agent runs"), "{failed:?}");
}
