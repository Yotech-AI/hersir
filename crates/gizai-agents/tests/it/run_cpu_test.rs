//! GA-369 QA: CPU for agents in the agents crate (`gizai_agents::run_cpu`, the CPU and disk weights and the CPU cap of
//! `run_memory::Slice`, `os::spawn_agents_tree`).
//! - CPU for agents until it is set: all cores but a quarter, at least 2 left free (12 of 16).
//! - The busy check: CPU pressure (`some avg60` over 50%), else the load average over 1.5 times the cores, on Windows a
//!   CPU over 90% busy; and what it says. A test's `RunsMemory` never reads the computer's CPU until `watch_cpu`.
//! - Agents' work starts at nice 10 on Linux and macOS (`spawn_agents_tree`, `spawn_agents_tree_tokio`); other trees don't.
//! - The one command that turns the cpu and io controllers on (`DELEGATE_FIX`) writes the drop-in it says.
//! - Linux with a systemd user session, in a slice of the test's own (never the slice of the Hersir the tests run in):
//!   the slice, `gizai.slice` and each scope get CPUWeight 20 and IOWeight 20, a scope's program nice 10, and io.weight
//!   20 when the io controller is there; CPU for agents is the slice's cpu.max at once, also for what already runs in it,
//!   and through `RunsMemory` too. Two busy processes on one shared core: a desktop program (weight 100) keeps most of it
//!   beside an agent's. That is one core for about a second and a half, never more (qa.md: never load the computer on
//!   purpose); the cap check runs two busy processes inside a cap of one core for a second.
//!   Where there is no slice, or your user's systemd doesn't hand it the cpu controller, those tests say so and pass.
use gizai_agents::run_cpu::{self as cpu, Reading};
use gizai_agents::run_memory::{self as mem, RunsMemory};

#[test]
fn cpu_for_agents_is_all_cores_but_a_quarter_with_at_least_2_free_12_of_16() {
    for (cores, agents) in [(16, 12), (32, 24), (64, 48), (12, 9), (8, 6), (6, 4), (5, 3), (4, 2), (3, 1), (2, 1), (1, 1)] {
        assert_eq!(cpu::default_cores(cores), agents, "{cores} cores");
    }
    for cores in 3..=256 {
        let agents = cpu::default_cores(cores);
        assert!(agents >= 1 && cores - agents >= 2, "{cores} cores: {agents} for the agents leaves at least 2 free");
        assert!(cores - agents >= cores.div_ceil(4), "{cores} cores: a quarter stays free");
    }
    assert!(cpu::cores() >= 1);
}

#[test]
fn cpu_pressure_is_some_avg60_and_over_50_percent_is_busy() {
    let text = "some avg10=12.50 avg60=72.31 avg300=40.02 total=123456\nfull avg10=0.00 avg60=80.00 avg300=0.00 total=0\n";
    assert_eq!(cpu::pressure_some_avg60(text), Some(72.31));
    // the full line isn't it, nor avg10 or avg300; something unreadable is no reading
    assert_eq!(cpu::pressure_some_avg60("full avg10=0.00 avg60=80.00 avg300=0.00 total=0\n"), None);
    assert_eq!(cpu::pressure_some_avg60("some avg10=99.00 avg300=99.00 total=1\n"), None);
    assert_eq!(cpu::pressure_some_avg60(""), None);
    assert_eq!(cpu::pressure_some_avg60("some avg60=nan total=1\n"), None);
    assert_eq!(cpu::pressure_some_avg60("some avg60=-3.00 total=1\n"), None);
    assert_eq!(cpu::BUSY_PRESSURE, 50.0);
    assert!(Reading::Pressure(72.31).busy());
    assert!(Reading::Pressure(50.01).busy());
    assert!(!Reading::Pressure(50.0).busy(), "over 50%, not at it");
    assert!(!Reading::Pressure(0.16).busy());
    assert_eq!(Reading::Pressure(72.31).text(), "CPU pressure 72% over the last minute");
}

#[test]
fn elsewhere_a_load_over_one_and_a_half_times_the_cores_is_busy_and_on_windows_a_cpu_over_90_percent() {
    assert_eq!(cpu::loadavg_1("38.21 30.02 12.00 3/2825 1234\n"), Some(38.21));
    assert_eq!(cpu::loadavg_1(""), None);
    assert_eq!(cpu::loadavg_1("busy 1.0 2.0"), None);
    let on16 = |avg| Reading::Load { avg, cores: 16 };
    // the load of 26 Nov on this 16-core PC
    assert!(on16(38.2).busy());
    assert!(on16(24.01).busy());
    assert!(!on16(24.0).busy(), "over 1.5 times the cores, not at it");
    assert!(!on16(6.63).busy());
    assert_eq!(on16(38.21).text(), "a load of 38.2 on 16 cores over the last minute");
    assert_eq!(Reading::Load { avg: 2.0, cores: 1 }.text(), "a load of 2.0 on 1 core over the last minute");
    // a count of no cores counts as one
    assert!(Reading::Load { avg: 1.6, cores: 0 }.busy());
    assert!(Reading::Busy(97.4).busy());
    assert!(!Reading::Busy(90.0).busy());
    assert_eq!(Reading::Busy(97.4).text(), "the CPU 97% busy");
}

#[test]
fn this_computer_says_how_busy_its_cpu_is() {
    #[cfg(target_os = "linux")]
    match cpu::reading() {
        Some(Reading::Pressure(p)) => assert!((0.0..=100.0).contains(&p), "{p}"),
        Some(Reading::Load { avg, cores }) => assert!(avg >= 0.0 && cores == cpu::cores(), "{avg} {cores}"),
        other => panic!("Linux reads CPU pressure or its load average: {other:?}"),
    }
    #[cfg(target_os = "macos")]
    assert!(matches!(cpu::reading(), Some(Reading::Load { avg, cores }) if avg >= 0.0 && cores == cpu::cores()));
    #[cfg(windows)]
    {
        // the first reading only starts counting; one a second later says how busy the CPU was meanwhile
        let _ = cpu::reading();
        std::thread::sleep(std::time::Duration::from_millis(1_100));
        assert!(matches!(cpu::reading(), Some(Reading::Busy(p)) if (0.0..=100.0).contains(&p)));
    }
}

#[test]
fn a_tests_runs_memory_never_reads_the_cpu_until_watch_cpu_and_a_fake_reading_stands_in() {
    let m = RunsMemory::default();
    assert_eq!(m.cpu_reading(), None, "a test's runs never wait for the test machine's CPU");
    m.fake_cpu_reading(Some(Reading::Pressure(72.0)));
    assert_eq!(m.cpu_reading(), Some(Reading::Pressure(72.0)));
    m.fake_cpu_reading(None);
    assert_eq!(m.cpu_reading(), None);
    // Hersir's own start
    m.watch_cpu();
    if cfg!(any(target_os = "linux", target_os = "macos")) {
        assert!(m.cpu_reading().is_some(), "it reads this computer's CPU now");
    }
    // a fake reading still comes first
    m.fake_cpu_reading(Some(Reading::Load { avg: 40.0, cores: 16 }));
    assert_eq!(m.cpu_reading(), Some(Reading::Load { avg: 40.0, cores: 16 }));
}

#[test]
fn cpu_for_agents_is_kept_in_cores_and_all_of_them_or_none_is_no_cap() {
    let m = RunsMemory::default();
    assert_eq!(m.cpu_cores(), None, "nothing until Hersir's start gives it");
    // Without a slice nothing is capped yet (all cores, so a Windows test never caps the other tests' agents' job)
    assert_eq!(m.set_cpu_cores(cpu::cores()), Ok(()));
    assert_eq!(m.cpu_cores(), Some(cpu::cores()));
    assert_eq!(m.set_cpu_cores(0), Ok(()));
    assert_eq!(m.cpu_cores(), None);
    assert!(m.missing_controllers().is_empty(), "nothing is missing without a slice");
}

#[test]
fn a_cgroups_cpu_max_is_read_in_cores() {
    assert_eq!(mem::cpu_max_cores("1200000 100000\n"), Some(12.0));
    assert_eq!(mem::cpu_max_cores("50000 100000"), Some(0.5));
    assert_eq!(mem::cpu_max_cores("200000"), Some(2.0), "the period is 100 ms when it isn't there");
    assert_eq!(mem::cpu_max_cores("max 100000\n"), None, "no cap");
    assert_eq!(mem::cpu_max_cores(""), None);
}

/// The one command Settings → Runs shows as Run this for me: sudo, a drop-in for user@.service with one Delegate= line for
/// cpu and io (beside pids and memory, which Hersir's memory limit needs), and a daemon-reload. Its script, with the
/// drop-in's folder in a temporary one and without sudo and the reload, writes exactly that file.
#[cfg(unix)]
#[test]
fn the_command_that_turns_the_cpu_and_io_controllers_on_writes_one_delegate_line_for_user_service() {
    let fix = mem::DELEGATE_FIX;
    assert!(fix.starts_with("sudo sh -c '") && fix.ends_with('\''), "{fix}");
    assert!(fix.contains("/etc/systemd/system/user@.service.d/hersir-delegate.conf"), "{fix}");
    assert!(fix.ends_with("&& systemctl daemon-reload'"), "{fix}");
    let script = &fix["sudo sh -c '".len()..fix.len() - 1];
    assert!(!script.contains('\''), "one quoted script: {script}");
    let tmp = tempfile::tempdir().unwrap();
    let here = script.replace("/etc/systemd/system", &tmp.path().to_string_lossy()).replace(" && systemctl daemon-reload", "");
    assert!(!here.contains("systemctl") && !here.contains("/etc/"), "{here}");
    let out = std::process::Command::new("sh").arg("-c").arg(&here).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let written = std::fs::read_to_string(tmp.path().join("user@.service.d/hersir-delegate.conf")).unwrap();
    assert_eq!(written, "[Service]\nDelegate=pids memory cpu io\n");
    // and the command as a whole is one valid shell line
    assert!(std::process::Command::new("sh").args(["-n", "-c", fix]).status().unwrap().success());
}

// ---- nice 10 for agents' work (Linux and macOS) ----

#[cfg(unix)]
fn nice_of(pid: u32) -> i32 {
    // SAFETY: errno cleared, then a plain call that only reads; -1 is a valid nice value, so errno tells failure apart.
    unsafe {
        #[cfg(target_os = "linux")]
        {
            *libc::__errno_location() = 0;
        }
        #[cfg(target_os = "macos")]
        {
            *libc::__error() = 0;
        }
        let n = libc::getpriority(libc::PRIO_PROCESS, pid as libc::id_t);
        assert!(n != -1 || std::io::Error::last_os_error().raw_os_error() == Some(0), "can't read the nice value of {pid}");
        n
    }
}

#[cfg(unix)]
fn own_nice() -> i32 {
    nice_of(std::process::id())
}

#[cfg(unix)]
#[test]
fn agents_work_starts_at_nice_10_and_other_trees_at_hersirs_own() {
    use gizai_agents::os::{self, End};
    assert_eq!(os::LOW_NICE, 10);
    let low = (own_nice() + 10).min(19);
    let mut cmd = std::process::Command::new("sleep");
    cmd.arg("30");
    let (mut agents, tree) = os::spawn_agents_tree(&mut cmd).unwrap();
    let mut cmd = std::process::Command::new("sleep");
    cmd.arg("30");
    let (mut normal, normal_tree) = os::spawn_tree(&mut cmd, false).unwrap();
    assert_eq!(nice_of(agents.id()), low, "a worktree's preparation, a preview");
    assert_eq!(nice_of(normal.id()), own_nice(), "a tree that isn't agents' work keeps Hersir's own");
    tree.end(End::Kill);
    normal_tree.end(End::Kill);
    agents.wait().unwrap();
    normal.wait().unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn an_agents_cli_starts_at_nice_10_and_claude_code_asked_for_its_tools_at_hersirs_own() {
    use gizai_agents::os::{self, End};
    let low = (own_nice() + 10).min(19);
    let mut cmd = tokio::process::Command::new("sleep");
    cmd.arg("30").kill_on_drop(true);
    let (mut agents, tree) = os::spawn_agents_tree_tokio(&mut cmd).unwrap();
    let mut cmd = tokio::process::Command::new("sleep");
    cmd.arg("30").kill_on_drop(true);
    let (mut normal, normal_tree) = os::spawn_tree_tokio(&mut cmd).unwrap();
    let (a, n) = (agents.id().unwrap(), normal.id().unwrap());
    assert_eq!(nice_of(a), low, "a task run, a chat answer, a board check");
    assert_eq!(nice_of(n), own_nice());
    #[cfg(target_os = "linux")]
    {
        // a session of its own, as before: the CLI leads it
        let sid = |pid: u32| {
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
            stat[stat.rfind(')').unwrap() + 2..].split_whitespace().nth(3).unwrap().to_string()
        };
        assert_eq!(sid(a), a.to_string());
    }
    tree.end(End::Kill);
    normal_tree.end(End::Kill);
    let _ = agents.wait().await;
    let _ = normal.wait().await;
}

// ---- Linux with a systemd user session: a slice of the test's own ----

#[cfg(target_os = "linux")]
mod linux {
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    use gizai_agents::os::{self, End, Tree};
    use gizai_agents::run_cpu::{self as cpu, WEIGHT};
    use gizai_agents::run_memory::{self as mem, GB, PARENT_SLICE, RunsMemory, Slice};

    /// The test's own slice, stopped and reverted when it goes.
    struct Own(Slice);

    impl Drop for Own {
        fn drop(&mut self) {
            let _ = self.0.remove();
        }
    }

    /// A slice of the test's own for the data folder `dir`, with the cpu controller; None (the test says why and passes)
    /// where this Linux has no systemd user session or doesn't hand the slice the cpu controller.
    fn own_slice(dir: &Path) -> Option<Own> {
        match Slice::set_up(&mem::slice_name(dir), 2 * GB) {
            Ok(s) => {
                assert!(!cgroup_of(std::process::id()).starts_with(s.dir()), "never the slice the tests run in");
                let s = Own(s);
                if s.0.missing_controllers().contains(&"cpu") {
                    eprintln!("skipped: your user's systemd doesn't hand {} the cpu controller", s.0.name());
                    return None;
                }
                Some(s)
            }
            Err(why) => {
                assert!(std::env::var_os("GIZAI_TEST_SLICE").is_none(), "GIZAI_TEST_SLICE is set, but there is no slice: {why}");
                eprintln!("skipped: no slice for agent runs on this Linux: {why}");
                None
            }
        }
    }

    fn cgroup_of(pid: u32) -> PathBuf {
        let text = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).unwrap();
        Path::new("/sys/fs/cgroup").join(text.lines().find_map(|l| l.strip_prefix("0::")).unwrap().trim().trim_start_matches('/'))
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())).trim().to_string()
    }

    fn systemctl_show(unit: &str, props: &[&str]) -> Vec<String> {
        let mut cmd = Command::new("systemctl");
        cmd.args(["--user", "show", unit]);
        for p in props {
            cmd.args(["-p", p]);
        }
        String::from_utf8_lossy(&cmd.output().unwrap().stdout).lines().map(String::from).collect()
    }

    fn comm(pid: u32) -> String {
        std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default().trim().to_string()
    }

    fn until(what: &str, secs: u64, mut ok: impl FnMut() -> bool) {
        let t0 = Instant::now();
        while !ok() {
            assert!(t0.elapsed() < Duration::from_secs(secs), "waited {secs} s for {what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// `program args` as agents' work starts it in a scope of `slice` (`os::spawn_agents_tree`), on CPU `pin` when given.
    fn in_scope(slice: &Slice, program: &str, args: &[&str], pin: Option<usize>) -> (Child, Tree, mem::Scope) {
        let (mut cmd, scope) = slice.scoped(program.as_ref());
        cmd.args(args).stdin(Stdio::null()).stdout(Stdio::null());
        if let Some(cpu) = pin {
            pin_to(&mut cmd, cpu);
        }
        let (child, tree) = os::spawn_agents_tree(&mut cmd).unwrap();
        (child, tree, scope)
    }

    /// The command runs on CPU `cpu` only (sched_setaffinity before exec: systemd-run then execs the program in the
    /// same process, which keeps it).
    fn pin_to(cmd: &mut Command, cpu: usize) {
        use std::os::unix::process::CommandExt;
        // SAFETY: sched_setaffinity is a plain syscall, safe between fork and exec; the set is built before the fork.
        unsafe {
            let mut set: libc::cpu_set_t = std::mem::zeroed();
            libc::CPU_SET(cpu, &mut set);
            cmd.pre_exec(move || {
                if libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    /// The last CPU this test may run on.
    fn last_cpu() -> usize {
        // SAFETY: a plain syscall into a zeroed set of the size given.
        unsafe {
            let mut set: libc::cpu_set_t = std::mem::zeroed();
            assert_eq!(libc::sched_getaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mut set), 0);
            (0..libc::CPU_SETSIZE as usize).rev().find(|&c| libc::CPU_ISSET(c, &set)).unwrap()
        }
    }

    /// The time `pid` has had on a CPU, in ns (/proc/<pid>/schedstat).
    fn on_cpu_ns(pid: u32) -> u64 {
        read(Path::new(&format!("/proc/{pid}/schedstat"))).split_whitespace().next().unwrap().parse().unwrap()
    }

    fn nice_of(pid: u32) -> i64 {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
        stat[stat.rfind(')').unwrap() + 2..].split_whitespace().nth(16).unwrap().parse().unwrap()
    }

    /// Ends what the test started when it goes, also when an assert fails first.
    struct Ends(Vec<(Child, Option<Tree>)>);

    impl Drop for Ends {
        fn drop(&mut self) {
            for (c, tree) in &mut self.0 {
                match tree {
                    Some(t) => t.end(End::Kill),
                    None => { let _ = c.kill(); }
                }
                let _ = c.wait();
            }
        }
    }

    const BUSY: &str = "while :; do :; done";

    #[test]
    fn the_slice_gizai_slice_and_each_scope_get_cpu_weight_20_its_program_nice_10_and_io_weight_20_when_the_io_controller_is_there() {
        let tmp = tempfile::tempdir().unwrap();
        let Some(s) = own_slice(tmp.path()) else { return };
        assert_eq!(WEIGHT, 20);
        let gizai = s.0.dir().parent().and_then(Path::parent).unwrap();
        assert!(gizai.ends_with(PARENT_SLICE), "{}", gizai.display());
        // as systemd has them: your desktop's apps have 100
        for unit in [s.0.name(), PARENT_SLICE] {
            let shown = systemctl_show(unit, &["CPUWeight", "IOWeight"]);
            assert!(shown.contains(&"CPUWeight=20".to_string()) && shown.contains(&"IOWeight=20".to_string()), "{unit}: {shown:?}");
        }
        assert_eq!(read(&s.0.dir().join("cpu.weight")), "20");
        assert_eq!(read(&gizai.join("cpu.weight")), "20");

        let (child, tree, scope) = in_scope(&s.0, "sleep", &["30"], None);
        let pid = child.id();
        let ends = Ends(vec![(child, Some(tree))]);
        until("sleep runs in its scope", 10, || comm(pid) == "sleep");
        assert_eq!(cgroup_of(pid), scope.dir(), "a scope of its own in the slice");
        assert_eq!(read(&scope.dir().join("cpu.weight")), "20", "the scope's cpu.weight");
        let own = nice_of(std::process::id());
        assert_eq!(nice_of(pid), (own + 10).min(19), "nice 10");
        let unit = scope.dir().file_name().unwrap().to_string_lossy().into_owned();
        let shown = systemctl_show(&unit, &["CPUWeight", "IOWeight"]);
        assert!(shown.contains(&"CPUWeight=20".to_string()) && shown.contains(&"IOWeight=20".to_string()), "{unit}: {shown:?}");

        // the disk: io.weight 20 where your user's systemd hands the slice the io controller; else the slice says it lacks it
        let controllers = read(&s.0.dir().join("cgroup.controllers"));
        let has_io = controllers.split_whitespace().any(|c| c == "io");
        assert_eq!(s.0.missing_controllers().contains(&"io"), !has_io, "{controllers}");
        if has_io {
            assert_eq!(read(&scope.dir().join("io.weight")), "default 20");
            assert_eq!(read(&s.0.dir().join("io.weight")), "default 20");
        } else {
            assert!(!scope.dir().join("io.weight").exists());
            eprintln!("this Linux doesn't hand the slice the io controller ({controllers}): Settings → Runs shows the command");
        }
        drop(ends);
    }

    /// GA-369's first criterion, measured: an agent's busy process (in its scope in gizai.slice, weight 20) and a desktop
    /// program's (in a cgroup of its own beside gizai.slice and app.slice, with systemd's default weight of 100, as your
    /// desktop's apps have) share one core for 1.5 s. With the weights the desktop's gets about five sixths of it; with
    /// the same weight each would get half. One core, never the whole computer: on 26 Nov it took 16 busy threads on 16
    /// cores, and the weights share each core the same way.
    ///
    /// Not app.slice itself: a cgroup's weight is shared over all its busy programs on every core, so on a desktop where
    /// app.slice has more at work (a browser, Claude desktop) its program on this one core gets less (63% in a QA run on
    /// 26 Nov), while app.slice as a whole still gets five sixths of what both want.
    #[test]
    fn on_a_shared_core_a_desktop_program_keeps_most_of_it_beside_an_agents_busy_process() {
        let tmp = tempfile::tempdir().unwrap();
        let Some(s) = own_slice(tmp.path()) else { return };
        let user = s.0.dir().ancestors().nth(3).unwrap().to_path_buf();
        assert!(user.file_name().unwrap().to_string_lossy().starts_with("user@"), "{}", user.display());
        let core = last_cpu();

        let (agent, agent_tree, _scope) = in_scope(&s.0, "sh", &["-c", BUSY], Some(core));
        let mut ends = Ends(vec![(agent, Some(agent_tree))]);
        // a desktop program as your desktop starts it, in a slice of the test's own right beside app.slice (no dash in its
        // name: a dash would put it inside another slice), stopped when the test ends
        let hex = s.0.name().trim_start_matches("gizai-runs-").trim_end_matches(".slice").to_string();
        let desk_slice = Desk(format!("hersirqadesk{hex}.slice"));
        let mut desk = Command::new("systemd-run");
        desk.args(["--user", "--scope", "--collect", "--quiet"]).arg(format!("--slice={}", desk_slice.0)).args(["--", "sh", "-c", BUSY])
            .stdin(Stdio::null()).stdout(Stdio::null());
        pin_to(&mut desk, core);
        ends.0.push((desk.spawn().unwrap(), None));
        let (a, d) = (ends.0[0].0.id(), ends.0[1].0.id());
        until("both run sh", 10, || comm(a) == "sh" && comm(d) == "sh");
        assert_eq!(cgroup_of(d).parent().and_then(Path::parent), Some(user.as_path()), "beside gizai.slice and app.slice: {}", cgroup_of(d).display());
        let Ok(weight) = std::fs::read_to_string(user.join(&desk_slice.0).join("cpu.weight")) else {
            eprintln!("skipped: the desktop's slice has no cpu.weight here");
            return;
        };
        assert_eq!(weight.trim(), "100", "systemd's default weight, as app.slice has");

        // a moment to settle, then 1.5 s measured
        std::thread::sleep(Duration::from_millis(300));
        let (a0, d0) = (on_cpu_ns(a), on_cpu_ns(d));
        std::thread::sleep(Duration::from_millis(1_500));
        let (a1, d1) = (on_cpu_ns(a), on_cpu_ns(d));
        drop(ends);
        let (agent_ns, desk_ns) = ((a1 - a0) as f64, (d1 - d0) as f64);
        let share = desk_ns / (agent_ns + desk_ns);
        eprintln!("one shared core for 1.5 s: the desktop program had {:.0} ms, the agent {:.0} ms ({:.0}% for the desktop)",
                  desk_ns / 1e6, agent_ns / 1e6, share * 100.0);
        assert!(desk_ns > 0.0, "the desktop program ran");
        assert!(share >= 0.75, "the desktop program keeps most of the core beside the agent: {:.0}% (same weights: 50%)", share * 100.0);
    }

    /// The desktop program's slice of the test's own, stopped when the test ends.
    struct Desk(String);

    impl Drop for Desk {
        fn drop(&mut self) {
            let _ = Command::new("systemctl").args(["--user", "stop", &self.0]).status();
        }
    }

    fn usage_usec(dir: &Path) -> u64 {
        read(&dir.join("cpu.stat")).lines().find_map(|l| l.strip_prefix("usage_usec ")).unwrap().parse().unwrap()
    }

    #[test]
    fn cpu_for_agents_is_the_slices_cpu_max_at_once_also_for_what_already_runs_in_it_and_none_lifts_it() {
        let tmp = tempfile::tempdir().unwrap();
        let Some(s) = own_slice(tmp.path()) else { return };
        assert_eq!(read(&s.0.dir().join("cpu.max")), "max 100000", "no cap until it is given");
        assert_eq!(s.0.cpu_cap(), None);
        // two busy processes already at work in the slice, then a cap of one core
        let (one, t1, _) = in_scope(&s.0, "sh", &["-c", BUSY], None);
        let mut ends = Ends(vec![(one, Some(t1))]);
        let (two, t2, _) = in_scope(&s.0, "sh", &["-c", BUSY], None);
        ends.0.push((two, Some(t2)));
        let (a, b) = (ends.0[0].0.id(), ends.0[1].0.id());
        until("both run sh", 10, || comm(a) == "sh" && comm(b) == "sh");
        s.0.set_cpu_cap(Some(1)).unwrap();
        assert_eq!(read(&s.0.dir().join("cpu.max")), "100000 100000");
        assert_eq!(s.0.cpu_cap(), Some(1.0));
        assert!(systemctl_show(s.0.name(), &["CPUQuotaPerSecUSec"]).contains(&"CPUQuotaPerSecUSec=1s".to_string()));
        std::thread::sleep(Duration::from_millis(200));
        let (u0, t0) = (usage_usec(s.0.dir()), Instant::now());
        std::thread::sleep(Duration::from_millis(1_000));
        let (u1, took) = (usage_usec(s.0.dir()), t0.elapsed());
        drop(ends);
        let cores = (u1 - u0) as f64 / took.as_micros() as f64;
        eprintln!("two busy processes under a cap of 1 core used {cores:.2} cores");
        assert!(cores <= 1.2, "the cap holds for what already ran: {cores:.2} cores");
        // another value holds at once; none lifts it
        s.0.set_cpu_cap(Some(12)).unwrap();
        assert_eq!(read(&s.0.dir().join("cpu.max")), "1200000 100000");
        assert_eq!(s.0.cpu_cap(), Some(12.0));
        s.0.set_cpu_cap(None).unwrap();
        assert_eq!(read(&s.0.dir().join("cpu.max")), "max 100000");
        assert_eq!(s.0.cpu_cap(), None);
    }

    #[test]
    fn runs_memory_gives_the_slice_cpu_for_agents_when_it_is_set_up_and_a_change_at_once_and_all_cores_is_no_cap() {
        let tmp = tempfile::tempdir().unwrap();
        let name = mem::slice_name(tmp.path());
        let m = RunsMemory::default();
        m.enable(&name, 2 * GB);
        let cores = cpu::cores();
        let want = cpu::default_cores(cores).min(3);
        // as Hersir's start: CPU for agents first, then the slice
        assert_eq!(m.set_cpu_cores(want), Ok(()));
        let Some(slice) = m.slice() else {
            eprintln!("skipped: no slice for agent runs on this Linux");
            return;
        };
        let own = Own(slice);
        if own.0.missing_controllers().contains(&"cpu") {
            eprintln!("skipped: your user's systemd doesn't hand {} the cpu controller", own.0.name());
            return;
        }
        if want < cores {
            assert_eq!(read(&own.0.dir().join("cpu.max")), format!("{} 100000", u64::from(want) * 100_000));
        }
        assert_eq!(m.missing_controllers(), own.0.missing_controllers());
        if cores > 2 {
            assert_eq!(m.set_cpu_cores(2), Ok(()));
            assert_eq!(read(&own.0.dir().join("cpu.max")), "200000 100000", "at once");
            assert_eq!(m.cpu_cores(), Some(2));
        }
        // all of this computer's cores: no cap
        assert_eq!(m.set_cpu_cores(cores), Ok(()));
        assert_eq!(read(&own.0.dir().join("cpu.max")), "max 100000");
    }
}
