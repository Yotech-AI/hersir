//! Memory for agent runs (GA-202, docs/PLATFORMS.md): Settings → Runs → Memory for agent runs, one limit for all agent
//! runs together. On Linux with a systemd user session the runs, chat answers, board checks and worktree preparations
//! start in this Gizai's own slice (`gizai_agents::run_memory`), which the setting limits; Gizai itself stays outside it.
//! On every system no run starts while memory is short (`wait_reason`), and a process Linux ended at the limit shows in
//! the card's activity (`ended_on_card`).
//!
//! What runs leave behind in the slice (GA-211): a run's end ends what it left in its scope and its log names it
//! (`leftovers_note`); quitting ends whatever is still in the slice (`end_leftovers`), and so does Gizai's next start,
//! after a crash, before it recovers the runs (`end_leftovers_at_start`).
//!
//! Memory per run (GA-277): Settings → Runs → Memory per run, one run's own limit (`PER_RUN_KEY`). With the slice each
//! run, chat answer, board check, preview and worktree preparation starts in a scope with that MemoryMax and no
//! MemoryHigh, so Linux ends a runaway process inside its own run at once, and the other runs go on. Why: on 11 Nov 2026
//! a hung test grew to 31 GB, the slice passed its MemoryHigh, and every run and chat in it crawled for an hour.
use std::path::Path;

use gizai_agents::run_memory::{self as mem, GB, Leftover};
use gizai_core::{ids, settings};

use crate::AppState;

/// The settings key: the limit in whole GB. Unset until someone sets it (`default_gb`).
pub const KEY: &str = "runs_memory_gb";
/// The settings key of Memory per run (GA-277): one run's own limit in whole GB. Unset until someone sets it
/// (`per_run_default_gb`).
pub const PER_RUN_KEY: &str = "runs_memory_per_run_gb";
/// The smallest limit Settings → Runs allows, in GB.
pub const MIN_GB: u32 = 2;
/// No run starts while the runs use more than this share of the limit, in percent…
pub const USED_PERCENT: u64 = 80;
/// …or while the computer has less than this free.
pub const MIN_FREE: u64 = 2 * GB;

/// This computer's memory in whole GB, as Settings → Runs shows it; None when the system doesn't say.
pub fn computer_gb() -> Option<u32> {
    mem::total().map(|t| ((t + GB / 2) / GB).max(1) as u32)
}

/// The limit until someone sets it: half of this computer's memory (64 GB → 32 GB, 16 GB → 8 GB), at least 2 GB; 8 GB
/// when the system doesn't say how much it has.
pub fn default_gb() -> u32 {
    mem::total().map(|t| ((t / 2 + GB / 2) / GB) as u32).unwrap_or(8).max(MIN_GB)
}

/// Memory for agent runs in GB: as saved, else `default_gb`.
pub fn setting_gb(st: &AppState) -> u32 {
    settings::get::<u32>(&st.db, KEY).ok().flatten().unwrap_or_else(default_gb)
}

/// The limit the runs have now, in bytes: the slice's (`RunsMemory::limit`, the setting once Gizai started), else the
/// setting.
pub fn limit(st: &AppState) -> u64 {
    st.run_memory.limit().unwrap_or_else(|| u64::from(setting_gb(st)) * GB)
}

/// Whether `gb` may be saved: 2 GB up to this computer's memory.
pub fn check(gb: u32) -> Result<(), String> {
    match computer_gb() {
        Some(c) if !(MIN_GB..=c.max(MIN_GB)).contains(&gb) =>
            Err(format!("memory for agent runs may be between {MIN_GB} and {} GB: this computer has {c} GB", c.max(MIN_GB))),
        None if gb < MIN_GB => Err(format!("memory for agent runs must be at least {MIN_GB} GB")),
        _ => Ok(()),
    }
}

/// Saves Memory for agent runs, checked with `check` first. While nothing was saved, the default stays unset, so it keeps
/// following this computer's memory. The slice gets the new limit at once (MemoryMax and MemoryHigh together); when that
/// fails the setting is saved anyway, Gizai logs why, and the next start sets it.
pub fn save(st: &AppState, gb: u32) -> Result<(), String> {
    check(gb)?;
    if settings::get::<u32>(&st.db, KEY).ok().flatten().is_some() || gb != default_gb() {
        settings::set(&st.db, KEY, &gb).map_err(|e| e.to_string())?;
    }
    if let Err(e) = st.run_memory.set_limit(u64::from(gb) * GB) {
        eprintln!("gizai: the agent runs' slice keeps its memory limit until Hersir starts again: {e}");
    }
    Ok(())
}

/// Memory per run until someone sets it (GA-277): half of Memory for agent runs `runs_gb`, rounded down, at least 2 GB
/// (32 → 16, 31 → 15, 3 → 2).
pub fn per_run_default_gb(runs_gb: u32) -> u32 {
    (runs_gb / 2).max(MIN_GB)
}

/// Memory per run as saved, else `per_run_default_gb` of Memory for agent runs, which it then follows. It may be more
/// than a Memory for agent runs that was lowered later; a run never gets more than that (`RunsMemory::run_limit`).
fn per_run_setting_gb(st: &AppState) -> u32 {
    settings::get::<u32>(&st.db, PER_RUN_KEY).ok().flatten().unwrap_or_else(|| per_run_default_gb(setting_gb(st)))
}

/// Memory per run in GB as Settings → Runs shows it: what a run that starts now gets, `per_run_setting_gb` and never
/// more than Memory for agent runs.
pub fn per_run_gb(st: &AppState) -> u32 {
    per_run_setting_gb(st).min(setting_gb(st))
}

/// Whether Memory per run may be `gb` with Memory for agent runs at `runs_gb`: 2 GB up to Memory for agent runs.
pub fn check_per_run(gb: u32, runs_gb: u32) -> Result<(), String> {
    let most = runs_gb.max(MIN_GB);
    if (MIN_GB..=most).contains(&gb) {
        return Ok(());
    }
    Err(format!("memory per run may be between {MIN_GB} and {most} GB: memory for agent runs is {most} GB"))
}

/// The Memory per run a save of Settings → Runs gives, checked before anything is saved: Some(gb) to save, None to keep
/// what was saved (unset, it goes on following Memory for agent runs). Left out (`gb` None), or as Settings → Runs
/// shows it now (`per_run_gb`), it stays as it was, also when Memory for agent runs is lowered below it in the same
/// save: a run never gets more than Memory for agent runs anyway. Another value must be 2 GB up to Memory for agent
/// runs as this save leaves it (`runs_gb`).
pub fn per_run_change(st: &AppState, gb: Option<u32>, runs_gb: u32) -> Result<Option<u32>, String> {
    match gb {
        Some(gb) if gb != per_run_gb(st) => check_per_run(gb, runs_gb).map(|_| Some(gb)),
        _ => Ok(None),
    }
}

/// Saves Memory per run when `change` (from `per_run_change`) has a new value, and gives it to the scopes that start
/// from now on: as saved, or half of Memory for agent runs while unset (`RunsMemory::set_run_limit`). Scopes that already
/// run keep the limit they started with. After a save of Memory for agent runs too, as an unset Memory per run follows it.
pub fn save_per_run(st: &AppState, change: Option<u32>) -> Result<(), String> {
    if let Some(gb) = change {
        settings::set(&st.db, PER_RUN_KEY, &gb).map_err(|e| e.to_string())?;
    }
    st.run_memory.set_run_limit(u64::from(per_run_setting_gb(st)) * GB);
    Ok(())
}

/// Gizai's start: its agent processes start in its own slice from now on (`mem::slice_name` of its data folder), with
/// the limit from Settings → Runs, each in a scope with Memory per run as its own limit. The slice is set up on a thread
/// of its own now, so the first run rarely waits for it; when it can't be made, processes start as before and Gizai
/// logs why, once. Tests that want the slice call this too.
pub fn start(st: &AppState) {
    st.run_memory.enable(&mem::slice_name(&st.data_dir), u64::from(setting_gb(st)) * GB);
    st.run_memory.set_run_limit(u64::from(per_run_setting_gb(st)) * GB);
    let st = st.clone();
    std::thread::spawn(move || {
        let _ = st.run_memory.slice();
    });
}

/// The start check, on every system: why a run may not start now for want of memory, or None. A run waits while the
/// agent runs use more than 80% of the limit (Linux with the slice: its memory.stat `anon`, without the file cache) or
/// the computer has less than 2 GB free. The queue tries again as it does: when a run ends, and once a minute.
pub fn wait_reason(st: &AppState) -> Option<String> {
    let limit = limit(st);
    let why = match st.run_memory.reading() {
        mem::Reading { used: Some(used), .. } if u128::from(used) * 100 > u128::from(limit) * u128::from(USED_PERCENT) =>
            Some(format!("Waiting for memory: agent runs use {} of {} GB (Settings → Runs)", tenths(used), whole(limit))),
        mem::Reading { available: Some(free), .. } if free < MIN_FREE =>
            Some(format!("Waiting for memory: this computer has {} GB free, less than {} GB (Settings → Runs)", tenths(free), whole(MIN_FREE))),
        _ => None,
    };
    let mut waiting = st.runs.memory_wait.lock().unwrap();
    let since = waiting.as_ref().map(|(since, _)| *since).unwrap_or_else(ids::now_ms);
    *waiting = why.clone().map(|w| (since, w));
    why
}

/// Since when the start check has been waiting for memory, and why (the board check).
pub fn waiting_since(st: &AppState) -> Option<(i64, String)> {
    st.runs.memory_wait.lock().unwrap().clone()
}

/// Linux ended a process in `agent`'s run on the card `task_id` at a limit: the card's activity says so, as Gizai, with
/// the limit and where to change it. `per_run`: the run's own limit in bytes, when that one ended it (GA-277); None when
/// the limit of all agent runs together did.
pub fn ended_on_card(st: &AppState, task_id: &str, agent: &str, per_run: Option<u64>) {
    let said = match per_run {
        Some(own) => format!("saw Linux end a process in {agent}'s run: the run reached its memory limit of {} GB \
                              (Settings → Runs → Memory per run)", whole(own)),
        None => format!("saw Linux end a process in {agent}'s run: the agent runs reached their memory limit of {} GB together \
                         (Settings → Runs → Memory for agent runs)", whole(limit(st))),
    };
    if gizai_core::pulls::note_cleanup(&st.db, task_id, &said).is_ok() {
        (st.notify)(crate::runs::Note::RowsChanged("tasks"));
    }
}

/// The run's own note at its end, when Linux ended `count` processes in it at a limit: its own of `per_run` bytes
/// (GA-277), or with None the limit of all agent runs together.
pub fn ended_note(st: &AppState, count: u64, per_run: Option<u64>) -> String {
    let what = if count == 1 { "a process".to_string() } else { format!("{count} processes") };
    match per_run {
        Some(own) => format!("Linux ended {what} in this run: it reached its memory limit of {} GB per run. \
                              Change it in Settings → Runs → Memory per run.", whole(own)),
        None => format!("Linux ended {what} in this run: Hersir's agent runs reached their memory limit of {} GB together. \
                         Change it in Settings → Runs → Memory for agent runs.", whole(limit(st))),
    }
}

/// The run's own note at its end, when Gizai ended processes it left running in its scope, outside its process group
/// (and its preview in the preview's scope): "Ended 1 process this run left running: chromium (pid 12345).", each
/// process once.
pub fn leftovers_note(left: &[Leftover]) -> String {
    format!("Ended {}.", mem::leftovers_text(left, "this run"))
}

/// Gizai quits, once its agents have ended (`quit`): whatever still runs in its slice ends too, SIGTERM and SIGKILL 3 s
/// later, so nothing outlives Gizai: a browser a run started detached, a run that didn't end in time. Only the slice this
/// Gizai set up (none in a test's Gizai, unless the test enabled it); Gizai itself is outside it. Returns what it
/// ended, which Gizai's log names.
pub fn end_leftovers(st: &AppState) -> Vec<Leftover> {
    let Some(slice) = st.run_memory.made() else { return vec![] };
    let left = slice.end_leftovers();
    if !left.is_empty() {
        eprintln!("gizai: quitting ended {}", mem::leftovers_text(&left, "the agent runs"));
    }
    left
}

/// Gizai's start, before it recovers the runs a previous Gizai left (`open_state`): whatever that Gizai left running in
/// the slice of the data folder `data_dir` ends (it crashed, or was killed), its runs' CLIs and what they started, SIGTERM
/// and SIGKILL 3 s later. A Gizai with another data folder has another slice, which this never touches. Linux with a
/// systemd user session; nothing with `HERSIR_RUNS_SLICE=off` (or `GIZAI_RUNS_SLICE=off`). Returns what it ended, which
/// Gizai's log names.
pub fn end_leftovers_at_start(data_dir: &Path) -> Vec<Leftover> {
    let left = mem::end_leftovers_in(&mem::slice_name(data_dir));
    if !left.is_empty() {
        eprintln!("gizai: ended {}", mem::leftovers_text(&left, "a previous Hersir"));
    }
    left
}

/// Bytes as GB with one decimal ("27.3").
fn tenths(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / GB as f64)
}

/// Bytes as whole GB, rounded ("32").
fn whole(bytes: u64) -> u64 {
    (bytes + GB / 2) / GB
}
