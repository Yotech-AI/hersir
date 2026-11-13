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

/// This computer's memory in GB (GA-280): a save refuses a Memory for agent runs above it, and GitHub's runners have 7 GB
/// (macos-15) or 16 GB (ubuntu-24.04, windows-latest), so the saves below take their values from it.
fn computer() -> u32 {
    gizai_lib::run_memory::computer_gb().expect("this system says how much memory it has")
}

/// Memory for agent runs, a Memory per run below it, and a Memory for agent runs below that to lower it to: 16, 10 and
/// 6 GB, and on a computer with less than 16 GB its memory, 1 GB less and 2 GB less than that (7, 6 and 4 on 7 GB).
fn runs_per_run_lowered() -> (u32, u32, u32) {
    let runs = computer().min(16);
    let per_run = 10.min(runs - 1);
    (runs, per_run, 6.min(per_run - 2))
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
    // a save of Memory for agent runs takes an unset Memory per run along: 10 GB, or the computer's memory when it has less
    let gb = computer().min(10);
    let s = runs::get_settings(&st);
    runs::save_settings(&st, &with(&s, Some(gb))).unwrap();
    assert_eq!(runs::get_settings(&st).runs_memory_per_run_gb, Some(gb / 2), "{gb} GB");
    assert_eq!(saved_per_run(&st), None, "still unset: it follows Memory for agent runs");
    // the form saved as it is keeps it unset
    runs::save_settings(&st, &runs::get_settings(&st)).unwrap();
    assert_eq!(saved_per_run(&st), None);
}

#[test]
fn memory_per_run_from_2_gb_up_to_memory_for_agent_runs_is_saved_and_kept_after_a_restart_and_anything_else_is_refused_with_why() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    // Memory for agent runs 8 GB, or 1 GB less than the computer has when that is less, so a save can still raise it
    let computer = computer();
    let runs_gb = 8.min(computer - 1);
    {
        let st = gizai_lib::test_state(tmp.path());
        gizai_core::settings::set(&st.db, gizai_lib::run_memory::KEY, &runs_gb).unwrap();
        let s = runs::get_settings(&st);
        assert_eq!(s.runs_memory_per_run_gb, Some(runs_gb / 2));
        // less than 2 GB, or more than Memory for agent runs: refused with why, and nothing is saved
        for gb in [0, 1, runs_gb + 1, 64, 10_000] {
            let err = runs::save_settings(&st, &Settings { max_run_minutes: 99, ..per_run(&s, Some(gb)) }).unwrap_err();
            assert_eq!(err, format!("memory per run may be between 2 and {runs_gb} GB: memory for agent runs is {runs_gb} GB"), "{gb} GB");
            assert_eq!(saved_per_run(&st), None, "{gb} GB wasn't saved");
            assert_eq!(runs::get_settings(&st).runs_memory_per_run_gb, Some(runs_gb / 2));
            assert_ne!(runs::get_settings(&st).max_run_minutes, 99, "nor the rest of the form");
        }
        // checked against Memory for agent runs as the same save leaves it
        let err = runs::save_settings(&st, &Settings { runs_memory_gb: Some(4), runs_memory_per_run_gb: Some(6), ..s.clone() }).unwrap_err();
        assert_eq!(err, "memory per run may be between 2 and 4 GB: memory for agent runs is 4 GB");
        assert_eq!((runs::get_settings(&st).runs_memory_gb, saved_per_run(&st)), (Some(runs_gb), None), "neither was saved");
        // raised in the same save, more than it was: 12 GB with 10 per run, or the computer's memory for both
        let (raised, own) = (computer.min(12), computer.min(10));
        runs::save_settings(&st, &Settings { runs_memory_gb: Some(raised), runs_memory_per_run_gb: Some(own), ..s.clone() }).unwrap();
        assert_eq!((runs::get_settings(&st).runs_memory_gb, runs::get_settings(&st).runs_memory_per_run_gb), (Some(raised), Some(own)));
        runs::save_settings(&st, &with(&s, Some(runs_gb))).unwrap();

        // 2 GB and Memory for agent runs are the ends of the range
        for gb in [2, runs_gb, 3] {
            runs::save_settings(&st, &per_run(&runs::get_settings(&st), Some(gb))).unwrap();
            assert_eq!((runs::get_settings(&st).runs_memory_per_run_gb, saved_per_run(&st)), (Some(gb), Some(gb)));
        }
        // it no longer follows Memory for agent runs: at 4 GB an unset one would be 2
        runs::save_settings(&st, &with(&runs::get_settings(&st), Some(4))).unwrap();
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
    // 16 GB, Memory per run 10 and lowered to 6; less on a computer with less than 16 GB
    let (top, own, lowered) = runs_per_run_lowered();
    gizai_core::settings::set(&st.db, gizai_lib::run_memory::KEY, &top).unwrap();
    // as Gizai's start does, without setting the slice up (that is `slice`'s first call, which nothing here makes)
    st.run_memory.enable(&mem::slice_name(&st.data_dir), u64::from(top) * GB);
    st.run_memory.set_run_limit(u64::from(gizai_lib::run_memory::per_run_gb(&st)) * GB);
    assert_eq!(st.run_memory.run_limit(), Some(u64::from(top / 2) * GB), "half of {top} GB until it is set");
    runs::save_settings(&st, &per_run(&runs::get_settings(&st), Some(own))).unwrap();
    assert_eq!(st.run_memory.run_limit(), Some(u64::from(own) * GB), "runs that start after the save get it");

    // Memory for agent runs lowered below it, with Memory per run as the page showed it: allowed, and runs get the lower one
    runs::save_settings(&st, &with(&runs::get_settings(&st), Some(lowered))).unwrap();
    assert_eq!(st.run_memory.run_limit(), Some(u64::from(lowered) * GB));
    let s = runs::get_settings(&st);
    assert_eq!((s.runs_memory_gb, s.runs_memory_per_run_gb), (Some(lowered), Some(lowered)), "the page shows what a run gets now");
    assert_eq!(saved_per_run(&st), Some(own), "what you saved stays");
    // saved again as the page shows it, it still stays
    runs::save_settings(&st, &s).unwrap();
    assert_eq!(saved_per_run(&st), Some(own));
    // raised again, runs get Memory per run again
    runs::save_settings(&st, &with(&runs::get_settings(&st), Some(top))).unwrap();
    assert_eq!((runs::get_settings(&st).runs_memory_per_run_gb, st.run_memory.run_limit()), (Some(own), Some(u64::from(own) * GB)));
    // and a lower Memory per run counts at once
    runs::save_settings(&st, &per_run(&runs::get_settings(&st), Some(4))).unwrap();
    assert_eq!(st.run_memory.run_limit(), Some(4 * GB));
    assert!(!st.run_memory.enforced(), "nothing was set up");
}

// ---- GA-277, QA round 2: Memory per run as the page showed it (`runsMemoryPerRunShownGb`) ----

/// The JSON a page sends: `s` with `drop` left out (an older page that doesn't know those fields).
fn from_page(s: &Settings, drop: &[&str]) -> Settings {
    let mut v = serde_json::to_value(s).unwrap();
    for k in drop {
        v.as_object_mut().unwrap().remove(*k);
    }
    serde_json::from_value(v).unwrap()
}

#[test]
fn get_settings_gives_memory_per_run_as_shown_next_to_it_also_when_memory_for_agent_runs_is_lowered_below_a_saved_one() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    let (top, own, lowered) = runs_per_run_lowered();
    gizai_core::settings::set(&st.db, gizai_lib::run_memory::KEY, &top).unwrap();
    let json = serde_json::to_value(runs::get_settings(&st)).unwrap();
    assert_eq!((&json["runsMemoryPerRunGb"], &json["runsMemoryPerRunShownGb"]), (&serde_json::json!(top / 2), &serde_json::json!(top / 2)), "unset: half of {top}");
    runs::save_settings(&st, &per_run(&runs::get_settings(&st), Some(own))).unwrap();
    runs::save_settings(&st, &with(&runs::get_settings(&st), Some(lowered))).unwrap();
    assert_eq!(saved_per_run(&st), Some(own));
    let json = serde_json::to_value(runs::get_settings(&st)).unwrap();
    assert_eq!((&json["runsMemoryPerRunGb"], &json["runsMemoryPerRunShownGb"]), (&serde_json::json!(lowered), &serde_json::json!(lowered)), "capped at Memory for agent runs");
}

#[test]
fn the_ga202_tests_saves_of_its_first_read_leave_memory_per_run_unset() {
    // as memory_for_agent_runs_is_half_the_computer_until_set_…: copies of the first read, saved at 2 GB, the computer's
    // memory and 3 GB, then without Memory for agent runs
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    let computer = gizai_lib::run_memory::computer_gb().unwrap();
    let s = runs::get_settings(&st);
    assert_eq!(s.runs_memory_per_run_shown_gb, s.runs_memory_per_run_gb);
    for gb in [2, computer, 3] {
        runs::save_settings(&st, &with(&s, Some(gb))).unwrap();
        assert_eq!(saved_per_run(&st), None, "{gb} GB: Memory per run stays unset");
        assert_eq!(runs::get_settings(&st).runs_memory_per_run_gb, Some(gizai_lib::run_memory::per_run_default_gb(gb)), "{gb} GB: it follows");
    }
    runs::save_settings(&st, &with(&s, None)).unwrap();
    assert_eq!((runs::get_settings(&st).runs_memory_gb, saved_per_run(&st)), (Some(3), None));
    assert_eq!(runs::get_settings(&st).runs_memory_per_run_gb, Some(2));
}

#[test]
fn a_page_older_than_another_save_never_saves_nor_is_refused_for_a_memory_per_run_it_only_read_but_an_edit_is_checked() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    // 32 GB, or the computer's memory when it has less; then lowered to 6 GB, or 1 GB less than the half the old page read
    // (32, 16 and 6 GB; 16, 8 and 6 on 16 GB; 7, 3 and 2 on 7 GB)
    let top = computer().min(32);
    let lowered = 6.min(top / 2 - 1);
    let follows = gizai_lib::run_memory::per_run_default_gb(lowered);
    let within = 5.min(lowered);
    gizai_core::settings::set(&st.db, gizai_lib::run_memory::KEY, &top).unwrap();
    let old = runs::get_settings(&st);
    assert_eq!((old.runs_memory_per_run_gb, old.runs_memory_per_run_shown_gb), (Some(top / 2), Some(top / 2)));
    // another page lowers Memory for agent runs below that: an unset Memory per run follows to half of it
    runs::save_settings(&st, &with(&runs::get_settings(&st), Some(lowered))).unwrap();
    assert_eq!(runs::get_settings(&st).runs_memory_per_run_gb, Some(follows));

    // the old page saves what it read, without Memory for agent runs (it stays lowered): its half is neither refused nor saved
    let older = from_page(&old, &["runsMemoryGb"]);
    runs::save_settings(&st, &Settings { max_run_minutes: 77, ..older.clone() }).unwrap();
    assert_eq!((saved_per_run(&st), runs::get_settings(&st).runs_memory_per_run_gb), (None, Some(follows)), "still unset, it follows");
    assert_eq!(runs::get_settings(&st).max_run_minutes, 77, "the rest of the form is saved");
    // with an edit, Memory per run is checked against Memory for agent runs as the save leaves it (lowered), and refused
    let err = runs::save_settings(&st, &Settings { max_run_minutes: 88, ..per_run(&older, Some(10)) }).unwrap_err();
    assert_eq!(err, format!("memory per run may be between 2 and {lowered} GB: memory for agent runs is {lowered} GB"));
    assert_eq!((saved_per_run(&st), runs::get_settings(&st).max_run_minutes), (None, 77), "nothing was saved");
    // an edit within it is saved
    runs::save_settings(&st, &per_run(&older, Some(within))).unwrap();
    assert_eq!((saved_per_run(&st), runs::get_settings(&st).runs_memory_per_run_gb), (Some(within), Some(within)));
    // the old page again, as it read it: what someone saved since stays
    runs::save_settings(&st, &older).unwrap();
    assert_eq!(saved_per_run(&st), Some(within));
    // the old page with its own Memory for agent runs puts that back, and Memory per run stays
    runs::save_settings(&st, &old).unwrap();
    assert_eq!((runs::get_settings(&st).runs_memory_gb, saved_per_run(&st), runs::get_settings(&st).runs_memory_per_run_gb), (Some(top), Some(within), Some(within)));
}

#[test]
fn without_the_shown_value_memory_per_run_is_compared_with_what_settings_shows_now() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    gizai_core::settings::set(&st.db, gizai_lib::run_memory::KEY, &32u32).unwrap();
    let old = from_page(&runs::get_settings(&st), &["runsMemoryPerRunShownGb"]);
    assert_eq!((old.runs_memory_per_run_gb, old.runs_memory_per_run_shown_gb), (Some(16), None));
    // as Settings → Runs shows it now: unchanged, still unset (saved without its Memory for agent runs: the 32 GB above is
    // set straight in the database, and a save refuses more than this computer has)
    runs::save_settings(&st, &with(&old, None)).unwrap();
    assert_eq!(saved_per_run(&st), None);
    // after another save lowered Memory for agent runs to 6 GB, 16 is a change, and more than 6
    runs::save_settings(&st, &with(&runs::get_settings(&st), Some(6))).unwrap();
    let err = runs::save_settings(&st, &with(&old, None)).unwrap_err();
    assert_eq!(err, "memory per run may be between 2 and 6 GB: memory for agent runs is 6 GB");
    assert_eq!(saved_per_run(&st), None);
    // 3, as it shows now, is unchanged
    runs::save_settings(&st, &per_run(&with(&old, None), Some(3))).unwrap();
    assert_eq!(saved_per_run(&st), None);
    // and 4 is saved
    runs::save_settings(&st, &per_run(&with(&old, None), Some(4))).unwrap();
    assert_eq!(saved_per_run(&st), Some(4));
}
