//! GA-249: each `GIZAI_X` Hersir reads has a `HERSIR_X` name, which wins when both are set; `GIZAI_X` still works alone,
//! and an empty `HERSIR_X` gives way to a `GIZAI_X` that is set (`gizai_agents::env`). Checked on the reader itself and
//! on the readers in this crate: the fake keychain, Bitbucket's API and the slice switch. A program of its own: it sets
//! variables in the test process's environment, so its one test runs them in turn.
use std::path::Path;

use gizai_agents::secrets::Keychain;
use gizai_agents::{bitbucket, env, run_memory, secrets};

fn set(name: &str, value: impl AsRef<std::ffi::OsStr>) {
    // SAFETY: the only test in this binary, so no other thread reads the environment meanwhile.
    unsafe { std::env::set_var(name, value) };
}

fn unset(name: &str) {
    // SAFETY: as in `set`.
    unsafe { std::env::remove_var(name) };
}

fn unset_both(name: &str) {
    unset(&format!("HERSIR_{name}"));
    unset(&format!("GIZAI_{name}"));
}

/// The secret `key` as the keychain file at `path` holds it (`FileKeychain`), read on its own.
fn in_file(path: &Path, key: &str) -> Option<String> {
    secrets::FileKeychain::new(path.to_path_buf()).get(key).unwrap()
}

#[test]
fn each_hersir_variable_wins_over_its_gizai_name_which_still_works_alone() {
    // the names
    assert_eq!(env::PREFIX, "HERSIR_");
    assert_eq!(env::OLD_PREFIX, "GIZAI_");
    assert_eq!(env::old_name("HERSIR_DATA_DIR").as_deref(), Some("GIZAI_DATA_DIR"));
    assert_eq!(env::old_name("HERSIR_PREVIEW_PORT").as_deref(), Some("GIZAI_PREVIEW_PORT"));
    assert_eq!(env::old_name("GIZAI_DATA_DIR"), None, "only a HERSIR_ name has an old one");
    assert_eq!(env::old_name("PATH"), None);

    // the reader, on a name no other code reads
    let name = "HERSIR_QA_249_PROBE";
    unset_both("QA_249_PROBE");
    assert_eq!(env::var(name), None, "neither set");
    assert_eq!(env::var_os(name), None);
    set("GIZAI_QA_249_PROBE", "old");
    assert_eq!(env::var(name).as_deref(), Some("old"), "GIZAI_ alone still works");
    set(name, "new");
    assert_eq!(env::var(name).as_deref(), Some("new"), "HERSIR_ wins when both are set");
    assert_eq!(env::var_os(name).as_deref(), Some(std::ffi::OsStr::new("new")));
    set(name, "");
    assert_eq!(env::var(name).as_deref(), Some("old"), "an empty HERSIR_ gives way to a GIZAI_ that is set");
    unset("GIZAI_QA_249_PROBE");
    assert_eq!(env::var(name).as_deref(), Some(""), "an empty HERSIR_ alone is what it holds");
    set(name, "only-new");
    assert_eq!(env::var(name).as_deref(), Some("only-new"), "HERSIR_ alone");
    unset_both("QA_249_PROBE");

    // which variables are Hersir's own, as the terminal leaves them out: both prefixes, in any case
    for own in ["HERSIR_DATA_DIR", "GIZAI_DATA_DIR", "hersir_dev", "gizai_dev", "Hersir_X", "GIZAI_RESULT"] {
        assert!(env::is_own(own.as_ref()), "{own}");
    }
    for other in ["NOT_GIZAI_X", "NOT_HERSIR_X", "HERSIRX", "PATH", "TERM_PROGRAM", "HERSIR"] {
        assert!(!env::is_own(other.as_ref()), "{other}");
    }

    // the fake keychain: HERSIR_FAKE_KEYCHAIN, else GIZAI_FAKE_KEYCHAIN
    let tmp = tempfile::tempdir().unwrap();
    let (old_file, new_file) = (tmp.path().join("gizai-keychain.json"), tmp.path().join("hersir-keychain.json"));
    unset_both("FAKE_KEYCHAIN");
    set("GIZAI_FAKE_KEYCHAIN", &old_file);
    secrets::from_env().set("qa/one", "1").unwrap();
    assert_eq!(in_file(&old_file, "qa/one").as_deref(), Some("1"), "GIZAI_FAKE_KEYCHAIN alone still works");
    set("HERSIR_FAKE_KEYCHAIN", &new_file);
    secrets::from_env().set("qa/two", "2").unwrap();
    assert_eq!(in_file(&new_file, "qa/two").as_deref(), Some("2"), "HERSIR_FAKE_KEYCHAIN wins");
    assert_eq!(in_file(&old_file, "qa/two"), None, "and the GIZAI_ one's file isn't touched");
    assert_eq!(secrets::from_env().get("qa/one").unwrap(), None, "a FileKeychain has nothing behind it");
    unset_both("FAKE_KEYCHAIN");

    // Bitbucket's API: HERSIR_BITBUCKET_API, else GIZAI_BITBUCKET_API, else bitbucket.org's
    unset_both("BITBUCKET_API");
    let real = bitbucket::api();
    assert!(real.starts_with("https://"), "{real}");
    set("GIZAI_BITBUCKET_API", "http://127.0.0.1:9001/");
    assert_eq!(bitbucket::api(), "http://127.0.0.1:9001", "GIZAI_BITBUCKET_API alone still works");
    set("HERSIR_BITBUCKET_API", "http://127.0.0.1:9002");
    assert_eq!(bitbucket::api(), "http://127.0.0.1:9002", "HERSIR_BITBUCKET_API wins");
    set("HERSIR_BITBUCKET_API", "  ");
    assert_eq!(bitbucket::api(), real, "a blank HERSIR_BITBUCKET_API counts as set, but blank: the real API");
    unset_both("BITBUCKET_API");
    assert_eq!(bitbucket::api(), real);

    // the slice of agent runs: HERSIR_RUNS_SLICE=off, or GIZAI_RUNS_SLICE=off alone, switches it off before anything
    // is asked of systemd
    let name = run_memory::slice_name(tmp.path());
    unset_both("RUNS_SLICE");
    set("GIZAI_RUNS_SLICE", "off");
    assert!(matches!(run_memory::Slice::set_up(&name, run_memory::GB), Err(e) if e.contains("RUNS_SLICE is off")), "GIZAI_RUNS_SLICE=off alone");
    assert!(run_memory::end_leftovers_in(&name).is_empty());
    set("HERSIR_RUNS_SLICE", "off");
    unset("GIZAI_RUNS_SLICE");
    assert!(matches!(run_memory::Slice::set_up(&name, run_memory::GB), Err(e) if e == "HERSIR_RUNS_SLICE is off"), "HERSIR_RUNS_SLICE=off");
    assert!(run_memory::end_leftovers_in(&name).is_empty());
    unset_both("RUNS_SLICE");
}
