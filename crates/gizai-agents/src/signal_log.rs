//! A log of every process Hersir ends (GA-369, docs/PLATFORMS.md). Why: on 26 Nov 2026 Brave and Claude desktop died
//! on Jefsev's PC while a QA run loaded every core, and nothing said what had ended them. Each signal Hersir sends, and
//! each process it ends another way (a cgroup's `cgroup.kill`, a Windows job, a program of its own it gives up on),
//! writes one line to Hersir's log (stderr, which a release build points at its log file, `src-tauri/src/logs.rs`):
//!
//! `2026-11-26T10:59:12Z gizai: SIGTERM to pid 12345 (chromium) in /user.slice/…/gizai-runs-1a2b3c4d.slice/gizai-0123456789abcdef.scope: a run left it running in its scope`
//!
//! The time (UTC), the signal, the pid or process group, the process's name (of a group: its leader's), where it ran
//! (Linux: its cgroup, which names the scope or the app it belongs to) and why. Only what reached a process is logged:
//! a signal to a group that has already ended isn't. `recent` keeps the latest lines in memory too, for tests.
use std::collections::VecDeque;
use std::sync::Mutex;

/// How many of the latest lines `recent` keeps.
const KEEP: usize = 2000;

static RECENT: Mutex<VecDeque<Sent>> = Mutex::new(VecDeque::new());

/// One signal Hersir sent, or one process it ended another way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sent {
    /// When, in Unix ms.
    pub at_ms: i64,
    /// The process, or with `group` the process group it leads.
    pub pid: u32,
    pub group: bool,
    /// Its name as the system keeps it ("chromium", "node"; on Linux at most 15 characters), read just before it was
    /// signalled; "unnamed" when it couldn't be read.
    pub name: String,
    /// Linux: its cgroup as /proc/<pid>/cgroup says ("/user.slice/user-1000.slice/user@1000.service/…"), read just
    /// before; None elsewhere, or when it couldn't be read.
    pub cgroup: Option<String>,
    /// "SIGTERM", "SIGKILL", "SIGKILL (cgroup.kill)", "TerminateJobObject", "kill".
    pub signal: String,
    /// Why Hersir ended it, in plain words.
    pub why: String,
}

impl Sent {
    /// Its line in Hersir's log.
    pub fn line(&self) -> String {
        let target = if self.group { "process group" } else { "pid" };
        let place = self.cgroup.as_deref().map(|c| format!(" in {c}")).unwrap_or_default();
        format!("{} gizai: {} to {target} {} ({}){place}: {}", utc(self.at_ms), self.signal, self.pid, self.name, self.why)
    }
}

/// What a process is, read before it is signalled: its name and, on Linux, its cgroup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct About {
    pub pid: u32,
    pub name: String,
    pub cgroup: Option<String>,
}

/// Reads what `pid` is now: its name (`os::process_name`) and, on Linux, its cgroup.
pub fn about(pid: u32) -> About {
    About { pid, name: crate::os::process_name(pid).unwrap_or_else(|| "unnamed".into()), cgroup: cgroup(pid) }
}

/// The cgroup `pid` runs in, as /proc/<pid>/cgroup says (cgroup v2's `0::` line); None elsewhere.
fn cgroup(pid: u32) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
        text.lines().find_map(|l| l.strip_prefix("0::")).map(|c| c.trim().to_string()).filter(|c| !c.is_empty())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        None
    }
}

/// Logs that `signal` reached the process `about` (or the group it leads, with `group`), because of `why`: one line in
/// Hersir's log, kept for `recent` too.
pub fn record(about: &About, group: bool, signal: &str, why: &str) {
    let sent = Sent {
        at_ms: now_ms(), pid: about.pid, group, name: about.name.clone(), cgroup: about.cgroup.clone(), signal: signal.to_string(),
        why: why.to_string(),
    };
    eprintln!("{}", sent.line());
    let mut recent = RECENT.lock().unwrap_or_else(|e| e.into_inner());
    if recent.len() >= KEEP {
        recent.pop_front();
    }
    recent.push_back(sent);
}

/// The latest lines (at most 2,000), oldest first: tests check what Hersir signalled, and what it never did.
pub fn recent() -> Vec<Sent> {
    RECENT.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect()
}

/// A signal's name: "SIGTERM" for 15.
#[cfg(unix)]
pub fn signal_name(signal: libc::c_int) -> String {
    match signal {
        libc::SIGINT => "SIGINT".into(),
        libc::SIGTERM => "SIGTERM".into(),
        libc::SIGKILL => "SIGKILL".into(),
        libc::SIGHUP => "SIGHUP".into(),
        libc::SIGCONT => "SIGCONT".into(),
        other => format!("signal {other}"),
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Unix ms as ISO 8601 UTC, to the second: "2026-11-26T10:59:12Z".
pub fn utc(ms: i64) -> String {
    // Howard Hinnant's civil_from_days
    let z = ms.div_euclid(86_400_000) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    let s = ms.div_euclid(1000).rem_euclid(86_400);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", s / 3600, s / 60 % 60, s % 60)
}
