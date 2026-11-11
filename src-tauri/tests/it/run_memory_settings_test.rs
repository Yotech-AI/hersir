//! GA-202: Settings → Runs → Memory for agent runs, on every system: half of the computer's memory until it is set, with
//! the computer's memory next to it for the hint; 2 GB up to the computer's memory is saved and kept after a restart, and
//! anything else is refused with a message. A test's Gizai hasn't started its slice, so the limit isn't enforced there.
#[path = "../support/data_lock.rs"]
mod data_lock;

use gizai_agents::run_memory::{self as mem, GB};
use gizai_lib::runs::{self, Settings};

fn with(s: &Settings, gb: Option<u32>) -> Settings {
    Settings { runs_memory_gb: gb, ..s.clone() }
}

#[test]
fn memory_for_agent_runs_is_half_the_computer_until_set_2_gb_up_to_the_computer_is_saved_and_kept_after_a_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let total = mem::total().expect("this system says how much memory it has");
    let computer = ((total + GB / 2) / GB) as u32;
    let half = ((total / 2 + GB / 2) / GB) as u32;
    {
        let st = gizai_lib::test_state(tmp.path());
        let s = runs::get_settings(&st);
        assert_eq!(s.computer_memory_gb, computer, "the hint's 'This computer has … GB'");
        assert_eq!(s.runs_memory_gb, Some(half.max(2)), "half of the computer's memory until it is set");
        assert!(!s.runs_memory_enforced, "a test's Gizai has no slice until it starts one");
        // what the UI gets
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!((&json["runsMemoryGb"], &json["computerMemoryGb"], &json["runsMemoryEnforced"]),
                   (&serde_json::json!(half.max(2)), &serde_json::json!(computer), &serde_json::json!(false)));

        // anything outside 2 GB to the computer's memory is refused with a message, and nothing is saved
        for gb in [0, 1, computer + 1, 10_000] {
            let err = runs::save_settings(&st, &with(&s, Some(gb))).unwrap_err();
            assert_eq!(err, format!("memory for agent runs may be between 2 and {computer} GB: this computer has {computer} GB"), "{gb} GB");
            assert_eq!(runs::get_settings(&st).runs_memory_gb, Some(half.max(2)), "{gb} GB wasn't saved");
        }
        // a refused value doesn't save the rest of the form either
        let err = runs::save_settings(&st, &Settings { max_run_minutes: 99, ..with(&s, Some(1)) }).unwrap_err();
        assert!(err.contains("memory for agent runs"), "{err}");
        assert_ne!(runs::get_settings(&st).max_run_minutes, 99);

        // 2 GB and the computer's memory are the ends of the range
        for gb in [2, computer, 3] {
            runs::save_settings(&st, &with(&s, Some(gb))).unwrap();
            assert_eq!(runs::get_settings(&st).runs_memory_gb, Some(gb));
        }
        // a form without the field (an older page) keeps what was saved
        runs::save_settings(&st, &with(&s, None)).unwrap();
        assert_eq!(runs::get_settings(&st).runs_memory_gb, Some(3));
        let from_ui: Settings = serde_json::from_value({
            let mut v = serde_json::to_value(runs::get_settings(&st)).unwrap();
            v.as_object_mut().unwrap().remove("runsMemoryGb");
            v
        }).unwrap();
        assert_eq!(from_ui.runs_memory_gb, None);
        runs::save_settings(&st, &from_ui).unwrap();
        assert_eq!(runs::get_settings(&st).runs_memory_gb, Some(3));
        // the start check and the activity line use the setting while Gizai hasn't started its slice
        assert_eq!(gizai_lib::run_memory::limit(&st), 3 * GB);
    }
    // a restart keeps it
    data_lock::released_blocking(&data);
    let st = gizai_lib::test_state(tmp.path());
    assert_eq!(runs::get_settings(&st).runs_memory_gb, Some(3));
    assert_eq!(gizai_lib::run_memory::setting_gb(&st), 3);
}

#[test]
fn the_default_follows_the_computer_until_someone_sets_another_value() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    let default = gizai_lib::run_memory::default_gb();
    let s = runs::get_settings(&st);
    // saving the form as it is (the default) keeps the default unset, so it keeps following the computer's memory
    runs::save_settings(&st, &s).unwrap();
    assert_eq!(gizai_core::settings::get::<u32>(&st.db, gizai_lib::run_memory::KEY).unwrap(), None);
    assert_eq!(runs::get_settings(&st).runs_memory_gb, Some(default));
    // another value is saved
    let other = if default > 2 { default - 1 } else { default + 1 };
    runs::save_settings(&st, &with(&s, Some(other))).unwrap();
    assert_eq!(gizai_core::settings::get::<u32>(&st.db, gizai_lib::run_memory::KEY).unwrap(), Some(other));
}

#[test]
fn the_check_allows_2_gb_up_to_the_computers_memory() {
    let computer = gizai_lib::run_memory::computer_gb().unwrap();
    assert!(gizai_lib::run_memory::check(2).is_ok());
    assert!(gizai_lib::run_memory::check(computer).is_ok());
    assert!(gizai_lib::run_memory::check(1).is_err());
    assert!(gizai_lib::run_memory::check(0).is_err());
    assert!(gizai_lib::run_memory::check(computer + 1).is_err());
}

// ---- GA-277: Settings → Runs → Memory per run ----

fn per_run(s: &Settings, gb: Option<u32>) -> Settings {
    Settings { runs_memory_per_run_gb: gb, ..s.clone() }
}

/// What is saved under Memory per run's key: None while it isn't set.
fn saved_per_run(st: &gizai_lib::AppState) -> Option<u32> {
    gizai_core::settings::get::<u32>(&st.db, gizai_lib::run_memory::PER_RUN_KEY).unwrap()
}

#[test]
fn memory_per_run_until_set_is_half_of_memory_for_agent_runs_rounded_down_and_at_least_2_gb() {
    use gizai_lib::run_memory::per_run_default_gb;
    assert_eq!(gizai_lib::run_memory::PER_RUN_KEY, "runs_memory_per_run_gb");
    for (runs, half) in [(64, 32), (32, 16), (31, 15), (16, 8), (5, 2), (4, 2), (3, 2), (2, 2)] {
        assert_eq!(per_run_default_gb(runs), half, "{runs} GB");
    }
    // Settings → Runs shows it, and follows Memory for agent runs while it isn't set
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    let default = gizai_lib::run_memory::default_gb();
    assert_eq!(runs::get_settings(&st).runs_memory_per_run_gb, Some(per_run_default_gb(default)), "half of the default");
    for (runs, half) in [(32u32, 16u32), (31, 15), (3, 2)] {
        gizai_core::settings::set(&st.db, gizai_lib::run_memory::KEY, &runs).unwrap();
        let s = runs::get_settings(&st);
        assert_eq!((s.runs_memory_gb, s.runs_memory_per_run_gb), (Some(runs), Some(half)), "{runs} GB");
        // what the UI gets
        assert_eq!(serde_json::to_value(&s).unwrap()["runsMemoryPerRunGb"], serde_json::json!(half));
    }
    // a save of Memory for agent runs takes an unset Memory per run along
    let s = runs::get_settings(&st);
    runs::save_settings(&st, &with(&s, Some(10))).unwrap();
    assert_eq!(runs::get_settings(&st).runs_memory_per_run_gb, Some(5));
    assert_eq!(saved_per_run(&st), None, "still unset: it follows Memory for agent runs");
    // the form saved as it is keeps it unset
    runs::save_settings(&st, &runs::get_settings(&st)).unwrap();
    assert_eq!(saved_per_run(&st), None);
}

#[test]
fn memory_per_run_from_2_gb_up_to_memory_for_agent_runs_is_saved_and_kept_after_a_restart_and_anything_else_is_refused_with_why() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    {
        let st = gizai_lib::test_state(tmp.path());
        gizai_core::settings::set(&st.db, gizai_lib::run_memory::KEY, &8u32).unwrap();
        let s = runs::get_settings(&st);
        assert_eq!(s.runs_memory_per_run_gb, Some(4));
        // less than 2 GB, or more than Memory for agent runs: refused with why, and nothing is saved
        for gb in [0, 1, 9, 64, 10_000] {
            let err = runs::save_settings(&st, &Settings { max_run_minutes: 99, ..per_run(&s, Some(gb)) }).unwrap_err();
            assert_eq!(err, "memory per run may be between 2 and 8 GB: memory for agent runs is 8 GB", "{gb} GB");
            assert_eq!(saved_per_run(&st), None, "{gb} GB wasn't saved");
            assert_eq!(runs::get_settings(&st).runs_memory_per_run_gb, Some(4));
            assert_ne!(runs::get_settings(&st).max_run_minutes, 99, "nor the rest of the form");
        }
        // checked against Memory for agent runs as the same save leaves it
        let err = runs::save_settings(&st, &Settings { runs_memory_gb: Some(4), runs_memory_per_run_gb: Some(6), ..s.clone() }).unwrap_err();
        assert_eq!(err, "memory per run may be between 2 and 4 GB: memory for agent runs is 4 GB");
        assert_eq!((runs::get_settings(&st).runs_memory_gb, saved_per_run(&st)), (Some(8), None), "neither was saved");
        runs::save_settings(&st, &Settings { runs_memory_gb: Some(12), runs_memory_per_run_gb: Some(10), ..s.clone() }).unwrap();
        assert_eq!((runs::get_settings(&st).runs_memory_gb, runs::get_settings(&st).runs_memory_per_run_gb), (Some(12), Some(10)));
        runs::save_settings(&st, &with(&s, Some(8))).unwrap();

        // 2 GB and Memory for agent runs are the ends of the range
        for gb in [2, 8, 3] {
            runs::save_settings(&st, &per_run(&runs::get_settings(&st), Some(gb))).unwrap();
            assert_eq!((runs::get_settings(&st).runs_memory_per_run_gb, saved_per_run(&st)), (Some(gb), Some(gb)));
        }
        // it no longer follows Memory for agent runs
        runs::save_settings(&st, &with(&runs::get_settings(&st), Some(16))).unwrap();
        assert_eq!(runs::get_settings(&st).runs_memory_per_run_gb, Some(3));
        // a form without the field (an older page) keeps what was saved
        runs::save_settings(&st, &per_run(&runs::get_settings(&st), None)).unwrap();
        let from_ui: Settings = serde_json::from_value({
            let mut v = serde_json::to_value(runs::get_settings(&st)).unwrap();
            v.as_object_mut().unwrap().remove("runsMemoryPerRunGb");
            v
        }).unwrap();
        assert_eq!(from_ui.runs_memory_per_run_gb, None);
        runs::save_settings(&st, &from_ui).unwrap();
        assert_eq!(saved_per_run(&st), Some(3));
    }
    // a restart keeps it
    data_lock::released_blocking(&data);
    let st = gizai_lib::test_state(tmp.path());
    assert_eq!(runs::get_settings(&st).runs_memory_per_run_gb, Some(3));
    assert_eq!(gizai_lib::run_memory::per_run_gb(&st), 3);
}

#[test]
fn a_run_never_gets_more_than_memory_for_agent_runs_also_when_that_is_lowered_below_memory_per_run_later() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    gizai_core::settings::set(&st.db, gizai_lib::run_memory::KEY, &16u32).unwrap();
    // as Gizai's start does, without setting the slice up (that is `slice`'s first call, which nothing here makes)
    st.run_memory.enable(&mem::slice_name(&st.data_dir), 16 * GB);
    st.run_memory.set_run_limit(u64::from(gizai_lib::run_memory::per_run_gb(&st)) * GB);
    assert_eq!(st.run_memory.run_limit(), Some(8 * GB), "half of 16 GB until it is set");
    runs::save_settings(&st, &per_run(&runs::get_settings(&st), Some(10))).unwrap();
    assert_eq!(st.run_memory.run_limit(), Some(10 * GB), "runs that start after the save get it");

    // Memory for agent runs lowered to 6 GB, with Memory per run as the page showed it (10): allowed, and runs get 6 GB
    runs::save_settings(&st, &with(&runs::get_settings(&st), Some(6))).unwrap();
    assert_eq!(st.run_memory.run_limit(), Some(6 * GB));
    let s = runs::get_settings(&st);
    assert_eq!((s.runs_memory_gb, s.runs_memory_per_run_gb), (Some(6), Some(6)), "the page shows what a run gets now");
    assert_eq!(saved_per_run(&st), Some(10), "what you saved stays");
    // saved again as the page shows it, it still stays
    runs::save_settings(&st, &s).unwrap();
    assert_eq!(saved_per_run(&st), Some(10));
    // raised again, runs get Memory per run again
    runs::save_settings(&st, &with(&runs::get_settings(&st), Some(20))).unwrap();
    assert_eq!((runs::get_settings(&st).runs_memory_per_run_gb, st.run_memory.run_limit()), (Some(10), Some(10 * GB)));
    // and a lower Memory per run counts at once
    runs::save_settings(&st, &per_run(&runs::get_settings(&st), Some(4))).unwrap();
    assert_eq!(st.run_memory.run_limit(), Some(4 * GB));
    assert!(!st.run_memory.enforced(), "nothing was set up");
}
