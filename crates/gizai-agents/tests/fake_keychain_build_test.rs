//! GA-370 QA: HERSIR_FAKE_KEYCHAIN (or GIZAI_FAKE_KEYCHAIN) puts a file in the keychain's place only in a debug build,
//! for headless UI tests and QA runs: `secrets::fake_file` reads it under `cfg(debug_assertions)`, and `from_env` and
//! src-tauri's `test_state` ask `fake_file`. A release build (this test built with --release, as GA-363 did for
//! HERSIR_CLOUD_* in src-tauri/tests/cloud_vars_test.rs) never reads either name, so it always uses the system's
//! keychain. The release build doesn't open that keychain here: that would touch this computer's real one. And no other
//! code reads the variable. A program of its own: it sets variables in the test process's environment.
use std::path::{Path, PathBuf};

use gizai_agents::secrets::{self, Keychain};

fn set(name: &str, value: impl AsRef<std::ffi::OsStr>) {
    // SAFETY: the only test in this binary, so no other thread reads the environment meanwhile.
    unsafe { std::env::set_var(name, value) };
}

fn unset_both() {
    // SAFETY: as in `set`.
    unsafe {
        std::env::remove_var("HERSIR_FAKE_KEYCHAIN");
        std::env::remove_var("GIZAI_FAKE_KEYCHAIN");
    }
}

/// The Rust files under `dir`.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(rust_files(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
    out
}

/// The body of `fn <name>` in `src`, up to the first line that closes it at four spaces' indent or none.
fn body<'a>(src: &'a str, name: &str) -> &'a str {
    let start = src.find(&format!("fn {name}(")).unwrap_or_else(|| panic!("fn {name}"));
    let rest = &src[start..];
    let end = rest.find("\n}\n").or_else(|| rest.find("\n    }\n")).unwrap();
    &rest[..end]
}

#[test]
fn only_a_debug_build_puts_a_file_in_the_keychains_place() {
    let tmp = tempfile::tempdir().unwrap();
    let (new_file, old_file) = (tmp.path().join("hersir-keychain.json"), tmp.path().join("gizai-keychain.json"));
    unset_both();
    assert_eq!(secrets::fake_file(), None);
    set("HERSIR_FAKE_KEYCHAIN", &new_file);
    if cfg!(debug_assertions) {
        assert_eq!(secrets::fake_file(), Some(new_file.clone()), "a debug build reads HERSIR_FAKE_KEYCHAIN");
        secrets::from_env().set("qa/one", "1").unwrap();
        assert_eq!(secrets::FileKeychain::new(new_file.clone()).get("qa/one").unwrap().as_deref(), Some("1"), "from_env uses the file");
        unset_both();
        set("GIZAI_FAKE_KEYCHAIN", &old_file);
        assert_eq!(secrets::fake_file(), Some(old_file.clone()), "and its old name");
        set("GIZAI_FAKE_KEYCHAIN", "");
        assert_eq!(secrets::fake_file(), None, "an empty one is no file");
    } else {
        assert_eq!(secrets::fake_file(), None, "a release build ignores HERSIR_FAKE_KEYCHAIN");
        set("GIZAI_FAKE_KEYCHAIN", &old_file);
        assert_eq!(secrets::fake_file(), None, "and GIZAI_FAKE_KEYCHAIN");
        assert!(!new_file.exists() && !old_file.exists());
    }
    unset_both();

    // the source: fake_file reads the variable only under debug_assertions, and from_env and test_state ask it
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let secrets_rs = std::fs::read_to_string(root.join("crates/gizai-agents/src/secrets.rs")).unwrap().replace("\r\n", "\n");
    let fake_file = body(&secrets_rs, "fake_file");
    let release_arm = &fake_file[fake_file.find("#[cfg(not(debug_assertions))]").expect("a release arm")..];
    let debug_arm = &fake_file[fake_file.find("#[cfg(debug_assertions)]").expect("a debug arm")..fake_file.find("#[cfg(not(debug_assertions))]").unwrap()];
    assert!(debug_arm.contains("\"HERSIR_FAKE_KEYCHAIN\""), "{fake_file}");
    assert!(release_arm.contains("None") && !release_arm.contains("var"), "{release_arm}");
    let from_env = body(&secrets_rs, "from_env");
    assert!(from_env.contains("fake_file()") && !from_env.contains("var"), "{from_env}");
    let lib_rs = std::fs::read_to_string(root.join("src-tauri/src/lib.rs")).unwrap().replace("\r\n", "\n");
    let test_state = body(&lib_rs, "test_state");
    assert!(test_state.contains("secrets::fake_file()"), "{test_state}");
    // and no other code reads it: the only line with FAKE_KEYCHAIN outside comments and tests is fake_file's
    let mut readers = vec![];
    for dir in ["crates", "src-tauri/src"] {
        for file in rust_files(&root.join(dir)) {
            let path = file.to_string_lossy().replace('\\', "/");
            if path.contains("/tests/") || path.contains("/target/") {
                continue;
            }
            let text = std::fs::read_to_string(&file).unwrap();
            for line in text.lines().filter(|l| l.contains("FAKE_KEYCHAIN") && !l.trim_start().starts_with("//")) {
                readers.push(format!("{path}: {}", line.trim()));
            }
        }
    }
    assert_eq!(readers.len(), 1, "{readers:#?}");
    assert!(readers[0].ends_with("crate::env::var_os(\"HERSIR_FAKE_KEYCHAIN\").filter(|path| !path.is_empty()).map(PathBuf::from)"), "{readers:#?}");
}
