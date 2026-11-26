//! GA-369 QA, Windows: CPU for agents is the CPU rate cap of the one job all agents' jobs are in, and it changes at once
//! for agents' work that already runs, both ways. One test in a binary of its own, because there is one agents' job per
//! process: a cap here never slows the agents' trees of the other tests (tests/it), which run in another process.
//!
//! - A cap given before any agents' program has started holds once the first one makes the agents' job.
//! - All cores (and none) lift it at once for what runs, as a CpuRate of 10,000; Windows refuses a CpuRate of 0 ("The
//!   parameter is incorrect. (os error 87)", Windows CI on 26 Nov 2026), so every change must answer Ok.
//! - A cap given while agents' programs run holds for them at once.
//!
//! Measured with two busy agents' programs (PowerShell, one thread each) in two windows of one second, with the change
//! in between: about one core under a cap of one, about two after the lift. That is two workers for four seconds in all,
//! on Windows only, never on a developer's Linux or Mac (qa.md: never load the computer on purpose; a test that must run
//! under load runs inside Hersir's CPU cap, with a set number of workers).
#![cfg(windows)]
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gizai_agents::run_cpu as cpu;
use gizai_agents::run_memory::RunsMemory;

/// .NET's DateTime ticks (100 ns since 1 Jan 0001, UTC) at Unix time 0.
const UNIX_EPOCH_TICKS: u128 = 621_355_968_000_000_000;

fn ticks(at: SystemTime) -> u128 {
    at.duration_since(UNIX_EPOCH).unwrap().as_nanos() / 100 + UNIX_EPOCH_TICKS
}

fn sleep_until(at: SystemTime) {
    if let Ok(left) = at.duration_since(SystemTime::now()) {
        std::thread::sleep(left);
    }
}

/// Two windows, `[start, start + 1 s]` and `[start + 1.5 s, start + 2.5 s]`, in which a program is busy on one thread;
/// it prints, for each, the CPU it used and the window's length, in ms. No double quotes, so the argument reaches
/// PowerShell as it is.
fn busy_script(start: SystemTime) -> String {
    let w = |from_ms: u64, to_ms: u64| {
        format!("W {} {}", ticks(start + Duration::from_millis(from_ms)), ticks(start + Duration::from_millis(to_ms)))
    };
    format!(
        "function W($s,$e){{while([DateTime]::UtcNow.Ticks -lt $s){{Start-Sleep -Milliseconds 5}}; \
         $t=[Diagnostics.Process]::GetCurrentProcess().TotalProcessorTime; $w=[Diagnostics.Stopwatch]::StartNew(); \
         while([DateTime]::UtcNow.Ticks -lt $e){{}}; \
         $c=[Diagnostics.Process]::GetCurrentProcess().TotalProcessorTime-$t; \
         [Console]::Out.WriteLine(('{{0}} {{1}}' -f [int]($c.TotalMilliseconds), $w.ElapsedMilliseconds))}}; {}; {}",
        w(0, 1_000),
        w(1_500, 2_500)
    )
}

/// Starts two busy agents' programs (`os::spawn_agents_tree`, so in the agents' job), calls `between` in the gap between
/// the two windows, and gives the cores they used together in each window.
fn two_busy_agents(between: impl FnOnce()) -> (f64, f64) {
    // time enough for two PowerShells to start, also under a cap of one core
    let start = SystemTime::now() + Duration::from_secs(10);
    let script = busy_script(start);
    let agents: Vec<_> = (0..2)
        .map(|_| {
            let mut cmd = gizai_agents::os::command("powershell");
            cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            gizai_agents::os::spawn_agents_tree(&mut cmd).unwrap()
        })
        .collect();
    sleep_until(start + Duration::from_millis(1_250));
    between();
    let mut used = (0.0, 0.0);
    for (child, _tree) in agents {
        let out = child.wait_with_output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "{text}\n{}", String::from_utf8_lossy(&out.stderr));
        let windows: Vec<f64> = text
            .lines()
            .map(|line| {
                let (cpu_ms, wall_ms) = line.trim().split_once(' ').unwrap_or_else(|| panic!("{text}"));
                let (cpu_ms, wall_ms): (f64, f64) = (cpu_ms.parse().unwrap(), wall_ms.parse().unwrap());
                assert!(wall_ms >= 900.0, "a window of a second: {text}");
                cpu_ms / wall_ms
            })
            .collect();
        assert_eq!(windows.len(), 2, "{text}");
        used.0 += windows[0];
        used.1 += windows[1];
    }
    used
}

#[test]
fn cpu_for_agents_caps_the_agents_job_and_lifts_it_at_once_also_after_an_agents_program_has_started() {
    let all = cpu::cores();
    let m = RunsMemory::default();
    if all < 3 {
        // a cap of one core leaves too little room beside two busy programs to tell apart; the calls must still hold
        assert_eq!(m.set_cpu_cores(1), Ok(()));
        assert_eq!(m.set_cpu_cores(all), Ok(()));
        assert_eq!(m.set_cpu_cores(0), Ok(()));
        eprintln!("skipped the measurement: {all} cores");
        return;
    }

    // a cap of one core before any agents' program has started: the first one makes the agents' job with it
    assert_eq!(m.set_cpu_cores(1), Ok(()));
    let mut lifted = Ok(());
    let (capped, after_lift) = two_busy_agents(|| lifted = m.set_cpu_cores(all));
    assert_eq!(lifted, Ok(()), "all cores lifts the cap while agents' programs run");
    assert_eq!(m.cpu_cores(), Some(all));
    eprintln!("cap of 1 core given before the job was there: {capped:.2} cores; after the lift: {after_lift:.2}");
    assert!(capped < 1.5, "two busy agents' programs under a cap of one core used {capped:.2} cores");
    assert!(after_lift > 1.5, "after the lift they used {after_lift:.2} cores, not about two");

    // no cap (all cores, as the job has now), then a cap of one core given while agents' programs run
    let mut capped_now = Ok(());
    let (free, capped) = two_busy_agents(|| capped_now = m.set_cpu_cores(1));
    assert_eq!(capped_now, Ok(()), "a cap while agents' programs run");
    eprintln!("without a cap: {free:.2} cores; after a cap of 1 core while they ran: {capped:.2}");
    assert!(free > 1.5, "without a cap two busy agents' programs used {free:.2} cores, not about two");
    assert!(capped < 1.5, "a cap of one core given while they ran left them {capped:.2} cores");

    // none lifts it too, and a cap after that, and all cores again: Windows takes every one
    assert_eq!(m.set_cpu_cores(0), Ok(()), "none lifts the cap");
    assert_eq!(m.cpu_cores(), None);
    assert_eq!(m.set_cpu_cores(1), Ok(()), "a cap again");
    assert_eq!(m.set_cpu_cores(all), Ok(()), "and lifted again");
}
