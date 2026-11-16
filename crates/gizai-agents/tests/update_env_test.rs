//! GA-248: what the release's install.sh gets from an update's environment. Its own variables, by both names (HERSIR_,
//! and GIZAI_ from before 0.17.0), never come from Hersir's environment: the build gets none of them, and the install
//! gets the prefix alone, under both names (0.16.0's install.sh, and an older one, read GIZAI_PREFIX only). A program of
//! its own: it sets those variables for the whole process.
// Linux and macOS only: install.sh is a stub shell script here, which Windows can't start.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use gizai_agents::update::{self as up, Log, Stop};

const NAMES: [&str; 8] = ["HERSIR_REPO", "HERSIR_BRANCH", "HERSIR_PREFIX", "HERSIR_DATA_DIR",
                          "GIZAI_REPO", "GIZAI_BRANCH", "GIZAI_PREFIX", "GIZAI_DATA_DIR"];

/// A release source whose install.sh writes each of NAMES as it sees it into `<tmp>/marks/<arg>`.
fn stub_source(tmp: &Path) -> (PathBuf, PathBuf) {
    let (dir, marks) = (tmp.join("source"), tmp.join("marks"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::create_dir_all(&marks).unwrap();
    let mut script = format!("f=\"{}/$1\"; : > \"$f\"\n", marks.display());
    for name in NAMES {
        script.push_str(&format!("echo \"{name}=${{{name}:-}}\" >> \"$f\"\n"));
    }
    std::fs::write(dir.join("install.sh"), script).unwrap();
    (dir, marks)
}

#[test]
fn the_build_inherits_neither_name_and_the_install_gets_the_prefix_under_both() {
    let tmp = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    for name in NAMES {
        // SAFETY: the only test in this program, set before anything else runs in it.
        unsafe { std::env::set_var(name, format!("/from/hersirs/own/{name}")) };
    }
    let (dir, marks) = stub_source(tmp.path());
    let log = Log::create(&tmp.path().join("update.log")).unwrap();

    up::build(&dir, None, &log, &Stop::default()).unwrap();
    let built = std::fs::read_to_string(marks.join("--build-only")).unwrap();
    for name in NAMES {
        assert!(built.contains(&format!("{name}=\n")), "the build got {name}: {built}");
    }

    let prefix = tmp.path().join("home/.local");
    up::install(&dir, &prefix, None, &log, &Stop::default()).unwrap();
    let installed = std::fs::read_to_string(marks.join("--skip-build")).unwrap();
    for name in ["HERSIR_PREFIX", "GIZAI_PREFIX"] {
        assert!(installed.contains(&format!("{name}={}\n", prefix.display())), "the install's {name}: {installed}");
    }
    for name in NAMES.iter().filter(|n| !n.ends_with("_PREFIX")) {
        assert!(installed.contains(&format!("{name}=\n")), "the install got {name}: {installed}");
    }
}
