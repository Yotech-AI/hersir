//! GA-369 QA: Settings → Runs → CPU for agents, on every system: all of the computer's cores but a quarter (at least 2
//! left free, 12 of 16) until it is set, with the computer's cores next to it for the hint and how the system holds the
//! cap (Linux the slice, Windows the agents' job, macOS none); 1 up to the computer's cores is saved, given to agents'
//! work at once and kept after a restart, anything else is refused with a message. A test's Hersir has no slice, so on
//! Linux the page says the cap doesn't hold here, and names nothing missing.
#[path = "../support/data_lock.rs"]
mod data_lock;

use gizai_agents::run_cpu as cpu;
use gizai_lib::runs::{self, Settings};

fn with(s: &Settings, cores: Option<u32>) -> Settings {
    Settings { runs_cpu_cores: cores, ..s.clone() }
}

#[test]
fn cpu_for_agents_is_all_cores_but_a_quarter_until_set_1_up_to_the_cores_is_saved_given_at_once_and_kept_after_a_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let cores = cpu::cores();
    let default = cpu::default_cores(cores);
    // a low value only where it can't cap other tests' agents (Windows: one job for all agents' work in this process)
    let pick = if cfg!(windows) { cores } else { 1 };
    {
        let st = gizai_lib::test_state(tmp.path());
        let s = runs::get_settings(&st);
        assert_eq!(s.computer_cores, cores, "the hint's 'This computer has … cores'");
        assert_eq!(s.runs_cpu_cores, Some(default), "all but a quarter until it is set");
        let kind = if cfg!(windows) { "job" } else if cfg!(target_os = "macos") { "macos" } else { "no_slice" };
        assert_eq!(s.runs_cpu_cap, kind, "how the system holds the cap");
        assert!(s.runs_missing_controllers.is_empty() && s.runs_controllers_fix.is_none(), "nothing missing without a slice");
        // what the UI gets
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!((&json["runsCpuCores"], &json["computerCores"], &json["runsCpuCap"], &json["runsMissingControllers"], &json["runsControllersFix"]),
                   (&serde_json::json!(default), &serde_json::json!(cores), &serde_json::json!(kind), &serde_json::json!([]), &serde_json::Value::Null));

        // anything outside 1 to the computer's cores is refused with a message, and nothing is saved, nor the rest of the form
        for n in [0, cores + 1, 10_000] {
            let err = runs::save_settings(&st, &with(&s, Some(n))).unwrap_err();
            assert_eq!(err, format!("CPU for agents may be between 1 and {cores} cores: this computer has {cores}"), "{n} cores");
            assert_eq!(runs::get_settings(&st).runs_cpu_cores, Some(default), "{n} cores wasn't saved");
        }
        let err = runs::save_settings(&st, &Settings { max_run_minutes: 99, ..with(&s, Some(0)) }).unwrap_err();
        assert!(err.contains("CPU for agents"), "{err}");
        assert_ne!(runs::get_settings(&st).max_run_minutes, 99);

        // a save of the page as it came leaves it unset, so it keeps following the computer
        runs::save_settings(&st, &s).unwrap();
        assert_eq!(gizai_core::settings::get::<u32>(&st.db, "runs_cpu_cores").unwrap(), None);
        // a value is saved and goes to agents' work at once
        runs::save_settings(&st, &with(&s, Some(pick))).unwrap();
        assert_eq!(runs::get_settings(&st).runs_cpu_cores, Some(pick));
        assert_eq!(st.run_memory.cpu_cores(), Some(pick), "given at once");
        // a form without the field (an older page) keeps what was saved
        runs::save_settings(&st, &with(&s, None)).unwrap();
        assert_eq!(runs::get_settings(&st).runs_cpu_cores, Some(pick));
        // the computer's cores is the top of the range: no cap
        runs::save_settings(&st, &with(&s, Some(cores))).unwrap();
        assert_eq!(runs::get_settings(&st).runs_cpu_cores, Some(cores));
        runs::save_settings(&st, &with(&s, Some(pick))).unwrap();
    }
    // a restart keeps it
    data_lock::released_blocking(&data);
    let st = gizai_lib::test_state(tmp.path());
    assert_eq!(runs::get_settings(&st).runs_cpu_cores, Some(pick));
    assert_eq!(gizai_lib::run_cpu::setting(&st), pick);
}

#[test]
fn a_saved_value_bigger_than_this_computer_counts_as_all_its_cores() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    // a data folder from a bigger computer, restored here
    gizai_core::settings::set(&st.db, "runs_cpu_cores", &(cpu::cores() + 8)).unwrap();
    assert_eq!(runs::get_settings(&st).runs_cpu_cores, Some(cpu::cores()));
    assert_eq!(gizai_lib::run_cpu::check(cpu::cores()), Ok(()));
    assert!(gizai_lib::run_cpu::check(cpu::cores() + 1).is_err());
}
