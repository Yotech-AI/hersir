//! GA-277: a Gizai started with HERSIR_RUNS_SLICE=off sets up no slice: its runs start as before, where Gizai runs and
//! with no limit per run, and Memory per run is still checked, saved and kept. One test in a binary of its own: it sets
//! HERSIR_RUNS_SLICE in the test process's environment.
#![cfg(target_os = "linux")]
use std::path::{Path, PathBuf};

use gizai_agents::run_memory::GB;
use gizai_lib::runs;

const FAKE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude.sh");

fn cgroup_in(file: &Path) -> PathBuf {
    let text = std::fs::read_to_string(file).unwrap();
    Path::new("/sys/fs/cgroup").join(text.lines().find_map(|l| l.strip_prefix("0::")).unwrap().trim().trim_start_matches('/'))
}

#[tokio::test]
async fn with_hersir_runs_slice_off_runs_start_as_before_and_memory_per_run_still_saves() {
    // SAFETY: the only test in this binary, before it starts anything that reads the environment.
    unsafe { std::env::set_var("HERSIR_RUNS_SLICE", "off") };
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    gizai_core::settings::set(&st.db, gizai_lib::run_memory::KEY, &8u32).unwrap();
    // as Gizai's start does
    gizai_lib::run_memory::start(&st);
    assert!(st.run_memory.slice().is_none(), "no slice with HERSIR_RUNS_SLICE=off");
    assert!(!st.run_memory.enforced());
    let s = runs::get_settings(&st);
    assert_eq!((s.runs_memory_gb, s.runs_memory_per_run_gb, s.runs_memory_enforced), (Some(8), Some(4), false));

    // a run starts as before, where Gizai runs (here: the test's own cgroup)
    let repo = tmp.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    for a in [&["init", "-q", "-b", "main"][..], &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "i"][..]] {
        assert!(std::process::Command::new("git").args(a).current_dir(&repo).status().unwrap().success());
    }
    let out = tmp.path().join("cgroup");
    let bin = tmp.path().join("claude.sh");
    let script = format!("#!/usr/bin/env bash\ncat /proc/$$/cgroup > '{}'\nexec '{FAKE}' \"$@\"\n", out.display());
    assert!(std::process::Command::new("sh").args(["-c", "printf '%s' \"$1\" > \"$2\" && chmod +x \"$2\"", "sh", &script])
        .arg(&bin).status().unwrap().success());
    run_where_gizai_runs(&st, &repo, &bin, &out).await;

    // Memory per run is still checked and saved
    let err = runs::save_settings(&st, &runs::Settings { runs_memory_per_run_gb: Some(9), ..runs::get_settings(&st) }).unwrap_err();
    assert_eq!(err, "memory per run may be between 2 and 8 GB: memory for agent runs is 8 GB");
    runs::save_settings(&st, &runs::Settings { runs_memory_per_run_gb: Some(3), ..runs::get_settings(&st) }).unwrap();
    assert_eq!(runs::get_settings(&st).runs_memory_per_run_gb, Some(3));
    assert_eq!(gizai_core::settings::get::<u32>(&st.db, gizai_lib::run_memory::PER_RUN_KEY).unwrap(), Some(3));
    assert_eq!(st.run_memory.run_limit(), Some(3 * GB), "noted, though no scope gets it");
    assert!(st.run_memory.slice().is_none());
    // and a run after the save starts as before too
    run_where_gizai_runs(&st, &repo, &bin, &out).await;
}

/// Runs a new card with `bin` (which writes its cgroup to `out`) and checks it ran where Gizai runs.
async fn run_where_gizai_runs(st: &gizai_lib::AppState, repo: &Path, bin: &Path, out: &Path) {
    let task = gizai_lib::test_task(st, repo.to_str().unwrap(), "backend");
    let summary = runs::run_once(st, &task, None, Some(bin.to_string_lossy().into_owned())).await.unwrap();
    assert_eq!(summary.status, "succeeded", "{summary:?}");
    assert_eq!(cgroup_in(out), cgroup_in(Path::new("/proc/self/cgroup")), "the run ran where Gizai runs, as before");
}
