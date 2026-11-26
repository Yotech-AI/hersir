//! CPU for agent runs (GA-369, docs/PLATFORMS.md): agents' work never starves your desktop. Why: on 26 Nov 2026 Brave
//! and Claude desktop died on a 16-core Linux PC while GA-366's QA run loaded every core (the whole test suite 90 times,
//! twice beside 16 busy-spin threads; a load average of 38 to 40 with the CPU 0% idle), and agent runs had the same CPU
//! weight as the desktop.
//!
//! - **Priority.** Agents' work (task runs, chat answers, board checks, worktree preparations, previews) starts at low
//!   CPU priority (`os::spawn_agents_tree`, `os::spawn_agents_tree_tokio`): nice 10 on Linux and macOS, below normal on
//!   Windows. On Linux with the agent runs' slice (`run_memory::Slice`) the slice, `gizai.slice` above it (which shares
//!   the CPU with your desktop's `app.slice`) and each run's scope get `CPUWeight=20` (`WEIGHT`; your apps have 100),
//!   and `IOWeight=20` when your user's systemd hands them the io controller (`Slice::missing_controllers`).
//! - **A cap.** Settings → Runs → CPU for agents, in cores (`default_cores`): `CPUQuota=` on the slice on Linux, the CPU
//!   rate cap of the job all agents' jobs are in on Windows (`os::set_agents_cpu`), none on macOS.
//! - **No new run while the computer is busy** (`reading`, `Reading::busy`): CPU pressure on Linux, else the load
//!   average against the cores; on Windows how busy the CPU was since the last look.
/// The CPU weight (and disk weight) of agents' work on Linux: your desktop's apps have 100.
pub const WEIGHT: u32 = 20;
/// Linux: busy while some task waited for a CPU in more than this share of the last minute, in percent (`some avg60` in
/// /proc/pressure/cpu).
pub const BUSY_PRESSURE: f64 = 50.0;
/// Elsewhere, and on Linux without CPU pressure: busy while the load average over the last minute is more than this
/// many times the computer's cores.
pub const BUSY_LOAD: f64 = 1.5;
/// Windows: busy while the CPU was more than this share busy since the last look, in percent.
pub const BUSY_PERCENT: f64 = 90.0;

/// The computer's cores (logical processors) that are online, at least 1.
pub fn cores() -> u32 {
    #[cfg(unix)]
    {
        // SAFETY: sysconf only reads.
        let n = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) };
        if let Ok(n) = u32::try_from(n)
            && n > 0
        {
            return n;
        }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::{ALL_PROCESSOR_GROUPS, GetActiveProcessorCount};
        // SAFETY: a plain call.
        let n = unsafe { GetActiveProcessorCount(ALL_PROCESSOR_GROUPS) };
        if n > 0 {
            return n;
        }
    }
    std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1).max(1)
}

/// CPU for agents until you set it: all `cores` but a quarter (rounded up), leaving at least 2 free, and at least 1
/// for the agents: 16 → 12, 32 → 24, 8 → 6, 6 → 4, 4 → 2, 2 → 1.
pub fn default_cores(cores: u32) -> u32 {
    let free = cores.div_ceil(4).max(2);
    cores.saturating_sub(free).max(1)
}

/// How busy the computer's CPU is, as the start check reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reading {
    /// Linux: the share of the last minute in which some task waited for a CPU, in percent (`some avg60`).
    Pressure(f64),
    /// The load average over the last minute, and the computer's cores.
    Load { avg: f64, cores: u32 },
    /// Windows: how busy the CPU was since the last look, in percent.
    Busy(f64),
}

impl Reading {
    /// Whether the computer is too busy for a new run: CPU pressure over `BUSY_PRESSURE`, a load average over
    /// `BUSY_LOAD` times the cores, a CPU over `BUSY_PERCENT` busy.
    pub fn busy(&self) -> bool {
        match *self {
            Reading::Pressure(p) => p > BUSY_PRESSURE,
            Reading::Load { avg, cores } => avg > BUSY_LOAD * f64::from(cores.max(1)),
            Reading::Busy(p) => p > BUSY_PERCENT,
        }
    }

    /// What it read, in words: "CPU pressure 72% over the last minute", "a load of 38.2 on 16 cores over the last
    /// minute", "the CPU 97% busy".
    pub fn text(&self) -> String {
        match *self {
            Reading::Pressure(p) => format!("CPU pressure {p:.0}% over the last minute"),
            Reading::Load { avg, cores } => format!("a load of {avg:.1} on {cores} {} over the last minute", if cores == 1 { "core" } else { "cores" }),
            Reading::Busy(p) => format!("the CPU {p:.0}% busy"),
        }
    }
}

/// The share of the last minute in which some task waited for a CPU, in percent: `some avg60` in /proc/pressure/cpu's
/// text ("some avg10=0.00 avg60=72.31 avg300=40.02 total=123"). Plain text work, the same on every system.
pub fn pressure_some_avg60(text: &str) -> Option<f64> {
    let some = text.lines().find_map(|l| l.strip_prefix("some "))?;
    some.split_whitespace().find_map(|w| w.strip_prefix("avg60="))?.parse().ok().filter(|p: &f64| p.is_finite() && *p >= 0.0)
}

/// The load average over the last minute: the first number in /proc/loadavg's text ("38.21 30.02 12.00 3/2825 1234").
pub fn loadavg_1(text: &str) -> Option<f64> {
    text.split_whitespace().next()?.parse().ok().filter(|l: &f64| l.is_finite() && *l >= 0.0)
}

/// How busy the CPU is now: Linux reads /proc/pressure/cpu, else (a kernel without CPU pressure) /proc/loadavg; macOS
/// its load average (getloadavg); Windows how busy the CPU was since the last reading, at least a second ago
/// (GetSystemTimes): the first reading only starts counting. None when the system doesn't say.
pub fn reading() -> Option<Reading> {
    #[cfg(target_os = "linux")]
    {
        if let Some(p) = std::fs::read_to_string("/proc/pressure/cpu").ok().and_then(|t| pressure_some_avg60(&t)) {
            return Some(Reading::Pressure(p));
        }
        std::fs::read_to_string("/proc/loadavg").ok().and_then(|t| loadavg_1(&t)).map(|avg| Reading::Load { avg, cores: cores() })
    }
    #[cfg(target_os = "macos")]
    {
        let mut avg = [0f64; 3];
        // SAFETY: getloadavg writes at most 3 values into the buffer.
        let n = unsafe { libc::getloadavg(avg.as_mut_ptr(), 3) };
        (n >= 1 && avg[0].is_finite()).then(|| Reading::Load { avg: avg[0], cores: cores() })
    }
    #[cfg(windows)]
    {
        windows_busy().map(Reading::Busy)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    None
}

/// Windows: how busy the CPU was, in percent, between the last two readings at least a second apart.
#[cfg(windows)]
fn windows_busy() -> Option<f64> {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::GetSystemTimes;
    // when, the idle and the total time then (100 ns), and the share busy since the reading before
    static LAST: Mutex<Option<(Instant, u64, u64, Option<f64>)>> = Mutex::new(None);
    let (mut idle, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
    // SAFETY: a plain call with three valid pointers.
    if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
        return None;
    }
    let n = |f: FILETIME| (u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime);
    // the kernel time includes the idle time
    let (idle, total) = (n(idle), n(kernel).saturating_add(n(user)));
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    match *last {
        Some((at, _, _, busy)) if now.duration_since(at) < Duration::from_secs(1) => busy,
        Some((_, idle0, total0, _)) => {
            let (di, dt) = (idle.saturating_sub(idle0), total.saturating_sub(total0));
            let busy = (dt > 0).then(|| 100.0 * dt.saturating_sub(di) as f64 / dt as f64);
            *last = Some((now, idle, total, busy));
            busy
        }
        None => {
            *last = Some((now, idle, total, None));
            None
        }
    }
}

