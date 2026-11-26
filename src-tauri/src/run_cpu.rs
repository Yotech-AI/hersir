//! CPU for agent runs (GA-369, docs/PLATFORMS.md): agents never over-tax this computer.
//!
//! - **Settings → Runs → CPU for agents** (`KEY`): at most this many of the computer's cores for all agents' work
//!   together, all but a quarter (and at least 2 left free) until you set it (`default_cores`: 12 of 16). Linux caps
//!   the agent runs' slice (`CPUQuota=`), at once for the runs at work too; Windows caps the job all agents' jobs are in;
//!   macOS has no cap. Agents' work starts at low CPU priority everywhere (`gizai_agents::run_cpu`).
//! - **Disk priority:** the slice gets `IOWeight=20` when your user's systemd hands it the io controller; otherwise
//!   Settings → Runs names what is missing, with the one command that turns it on (`missing`, `DELEGATE_FIX`).
//! - **No new run while the computer is busy** (`busy_reason`): a card the queue would start waits, and its activity
//!   says "Waiting: this computer is busy" once (`note_waiting`), until the CPU pressure eases; the queue tries again
//!   when a run ends and once a minute. Your own Run and Continue, the Chief's, Hersir's nudge after a run without its
//!   result (which nobody would start again), and chats aren't held.
use gizai_agents::run_cpu as cpu;
use gizai_agents::run_memory as mem;
use gizai_core::{ids, settings};

use crate::AppState;

/// The settings key: CPU for agents, in whole cores. Unset until someone sets it (`default_cores`).
pub const KEY: &str = "runs_cpu_cores";

/// This computer's cores, as Settings → Runs shows them.
pub fn computer_cores() -> u32 {
    cpu::cores()
}

/// CPU for agents until someone sets it: all of this computer's cores but a quarter, at least 2 left free (16 → 12).
pub fn default_cores() -> u32 {
    cpu::default_cores(cpu::cores())
}

/// CPU for agents in cores: as saved, else `default_cores`; never more than this computer has (a saved value from a
/// bigger computer, a data folder restored here).
pub fn setting(st: &AppState) -> u32 {
    let cores = computer_cores();
    settings::get::<u32>(&st.db, KEY).ok().flatten().unwrap_or_else(default_cores).clamp(1, cores.max(1))
}

/// Whether `cores` may be saved: 1 up to this computer's cores.
pub fn check(cores: u32) -> Result<(), String> {
    let most = computer_cores().max(1);
    if (1..=most).contains(&cores) {
        return Ok(());
    }
    Err(format!("CPU for agents may be between 1 and {most} cores: this computer has {most}"))
}

/// Saves CPU for agents, checked with `check` first. While nothing was saved the default stays unset, so it keeps
/// following this computer's cores. The cap changes at once, for the runs at work too; when the system refuses it the
/// setting is saved anyway, Hersir logs why, and the next start sets it.
pub fn save(st: &AppState, cores: u32) -> Result<(), String> {
    check(cores)?;
    if settings::get::<u32>(&st.db, KEY).ok().flatten().is_some() || cores != default_cores() {
        settings::set(&st.db, KEY, &cores).map_err(|e| e.to_string())?;
    }
    if let Err(e) = st.run_memory.set_cpu_cores(cores) {
        eprintln!("gizai: CPU for agents is saved, but doesn't hold until Hersir starts again: {e}");
    }
    Ok(())
}

/// Gizai's start (`run_memory::start`, before the slice is set up): agents' work gets CPU for agents as its cap.
pub fn start(st: &AppState) {
    if let Err(e) = st.run_memory.set_cpu_cores(setting(st)) {
        eprintln!("gizai: agents' work starts without a CPU cap: {e}");
    }
}

/// How the system holds CPU for agents now, for Settings → Runs: "slice" (Linux, the agent runs' slice), "job"
/// (Windows, the agents' job), or why there is no cap: "macos", "no_slice" (Linux without a systemd user session, or
/// before Hersir has set the slice up), "no_cpu" (the slice lacks the cpu controller).
pub fn cap_kind(st: &AppState) -> &'static str {
    if cfg!(windows) {
        return "job";
    }
    if cfg!(target_os = "macos") {
        return "macos";
    }
    if !st.run_memory.enforced() {
        return "no_slice";
    }
    if st.run_memory.missing_controllers().contains(&"cpu") { "no_cpu" } else { "slice" }
}

/// The controllers the agent runs' slice lacks (GA-369): "cpu" for agents' CPU weight and cap, "io" for their disk
/// priority. Empty without a slice, or when it has both.
pub fn missing(st: &AppState) -> Vec<String> {
    st.run_memory.missing_controllers().into_iter().map(String::from).collect()
}

/// The one command that turns the missing controllers on (`mem::DELEGATE_FIX`), when the slice lacks one.
pub fn fix(st: &AppState) -> Option<String> {
    (!missing(st).is_empty()).then(|| mem::DELEGATE_FIX.to_string())
}

/// The start check of a card the queue would start (GA-369): why it waits because this computer is
/// busy (`gizai_agents::run_cpu::Reading::busy`), or None. Nothing until Hersir's start enables the readings
/// (`RunsMemory::cpu_reading`), so a test's runs never wait for the test machine. Remembers since when it has been busy
/// (the board check), and forgets the cards it told when it eases, so a busy spell later tells them again.
pub fn busy_reason(st: &AppState) -> Option<String> {
    let why = st.run_memory.cpu_reading().filter(cpu::Reading::busy)
        .map(|r| format!("Waiting: this computer is busy ({}). The run starts once that eases.", r.text()));
    let mut waiting = st.runs.busy_wait.lock().unwrap_or_else(|e| e.into_inner());
    let since = waiting.as_ref().map(|(since, _)| *since).unwrap_or_else(ids::now_ms);
    *waiting = why.clone().map(|w| (since, w));
    if why.is_none() {
        st.runs.busy_noted.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
    why
}

/// Since when the start check has held cards because this computer is busy, and why (the board check).
pub fn waiting_since(st: &AppState) -> Option<(i64, String)> {
    st.runs.busy_wait.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The card `task_id` waits because this computer is busy: its activity says so, as Hersir, once while the computer
/// stays busy (`busy_reason` forgets it when it eases).
pub fn note_waiting(st: &AppState, task_id: &str, why: &str) {
    let first = st.runs.busy_noted.lock().unwrap_or_else(|e| e.into_inner()).insert(task_id.to_string());
    if first && gizai_core::pulls::note_cleanup(&st.db, task_id, why).is_ok() {
        (st.notify)(crate::runs::Note::RowsChanged("tasks"));
    }
}
