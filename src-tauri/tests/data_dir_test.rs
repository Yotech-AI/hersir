//! GA-51: Hersir's data lives in each system's own place, and a variable overrides it. GA-249: a new install's folder is
//! `hersir` (`Hersir` on macOS and Windows); an install from before 0.17.0 with its data (gizai.db) in the `gizai` folder
//! keeps using that one, and nothing is moved. HERSIR_DATA_DIR overrides it and wins over GIZAI_DATA_DIR, which still
//! works alone. One test in its own binary, as it changes the environment.
use std::path::{Path, PathBuf};

use gizai_lib::{data_dir, data_dir_names, default_data_dir};

fn set(name: &str, value: impl AsRef<std::ffi::OsStr>) {
    // SAFETY: the only test in this binary, so no other thread reads the environment meanwhile.
    unsafe { std::env::set_var(name, value) };
}

fn unset(name: &str) {
    // SAFETY: as in `set`.
    unsafe { std::env::remove_var(name) };
}

/// Every file and folder under `dir`, relative to it, sorted: what "nothing moved" compares.
fn listing(dir: &Path) -> Vec<String> {
    let mut all = vec![];
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            all.push(p.strip_prefix(dir).unwrap().display().to_string());
            if p.is_dir() {
                stack.push(p);
            }
        }
    }
    all.sort();
    all
}

#[test]
fn the_data_folder_is_each_systems_own_and_hersir_data_dir_or_gizai_data_dir_overrides_it() {
    unset("HERSIR_DATA_DIR");
    unset("GIZAI_DATA_DIR");
    #[cfg(target_os = "linux")]
    {
        set("XDG_DATA_HOME", "/srv/me/data");
        assert_eq!(data_dir_names(), [PathBuf::from("/srv/me/data/hersir"), PathBuf::from("/srv/me/data/gizai")]);
        assert_eq!(default_data_dir(), PathBuf::from("/srv/me/data/hersir"), "a new install's folder is hersir");
        unset("XDG_DATA_HOME");
        set("HOME", "/home/me");
        assert_eq!(data_dir_names(), [PathBuf::from("/home/me/.local/share/hersir"), PathBuf::from("/home/me/.local/share/gizai")]);
        assert_eq!(default_data_dir(), PathBuf::from("/home/me/.local/share/hersir"));
    }
    #[cfg(target_os = "macos")]
    {
        set("HOME", "/Users/me");
        set("XDG_DATA_HOME", "/Users/me/.xdg");
        assert_eq!(data_dir_names(), [
            PathBuf::from("/Users/me/Library/Application Support/Hersir"),
            PathBuf::from("/Users/me/Library/Application Support/Gizai"),
        ]);
        assert_eq!(default_data_dir(), PathBuf::from("/Users/me/Library/Application Support/Hersir"));
    }
    #[cfg(windows)]
    {
        set("APPDATA", r"C:\Users\me\AppData\Roaming");
        assert_eq!(data_dir_names(), [PathBuf::from(r"C:\Users\me\AppData\Roaming\Hersir"), PathBuf::from(r"C:\Users\me\AppData\Roaming\Gizai")]);
        assert_eq!(default_data_dir(), PathBuf::from(r"C:\Users\me\AppData\Roaming\Hersir"));
        // without APPDATA: the same place in your profile folder (Windows has no HOME)
        unset("APPDATA");
        set("USERPROFILE", r"C:\Users\other");
        assert_eq!(default_data_dir(), PathBuf::from(r"C:\Users\other\AppData\Roaming\Hersir"));
    }

    // Which folder, in a scratch place: the base of both names on this system
    let tmp = tempfile::tempdir().unwrap();
    #[cfg(target_os = "linux")]
    set("XDG_DATA_HOME", tmp.path());
    #[cfg(target_os = "macos")]
    set("HOME", tmp.path());
    #[cfg(windows)]
    set("APPDATA", tmp.path());
    let [new, old] = data_dir_names();
    assert!(new.starts_with(tmp.path()) && old.starts_with(tmp.path()), "{new:?} {old:?}");
    assert_eq!(default_data_dir(), new, "neither folder: a new hersir folder");
    assert!(!new.exists() && !old.exists(), "only picked, nothing made");

    // a gizai folder without Hersir's data in it isn't taken
    std::fs::create_dir_all(old.join("worktrees")).unwrap();
    assert_eq!(default_data_dir(), new, "a gizai folder without gizai.db");
    std::fs::create_dir(old.join("gizai.db")).unwrap();
    assert_eq!(default_data_dir(), new, "gizai.db must be a file");
    std::fs::remove_dir(old.join("gizai.db")).unwrap();

    // an install from before 0.17.0: its gizai folder holds gizai.db, so it is used where it is, and nothing moves
    std::fs::write(old.join("gizai.db"), b"SQLite format 3\0").unwrap();
    std::fs::write(old.join("gizai.lock"), b"").unwrap();
    let before = listing(tmp.path());
    assert_eq!(default_data_dir(), old, "the gizai folder that holds gizai.db");
    assert_eq!(data_dir(), old, "without a variable, data_dir is that folder too");
    assert_eq!(listing(tmp.path()), before, "nothing moved, copied or made");
    assert!(!new.exists());

    // once a hersir folder exists it wins, even beside an old gizai folder with data
    std::fs::create_dir_all(&new).unwrap();
    assert_eq!(default_data_dir(), new, "the hersir folder when it exists");
    assert_eq!(std::fs::read(old.join("gizai.db")).unwrap(), b"SQLite format 3\0", "the gizai folder's data stays as it was");
    std::fs::remove_dir(&new).unwrap();
    assert_eq!(default_data_dir(), old);

    // the variables: GIZAI_DATA_DIR alone, HERSIR_DATA_DIR alone, both (HERSIR_ wins), an empty HERSIR_ one gives way
    assert_eq!(data_dir(), default_data_dir());
    let gizai_own = std::env::temp_dir().join("gizai-own-data");
    let hersir_own = std::env::temp_dir().join("hersir-own-data");
    set("GIZAI_DATA_DIR", &gizai_own);
    assert_eq!(data_dir(), gizai_own, "GIZAI_DATA_DIR alone still overrides it on every system");
    set("HERSIR_DATA_DIR", &hersir_own);
    assert_eq!(data_dir(), hersir_own, "HERSIR_DATA_DIR wins over GIZAI_DATA_DIR");
    set("HERSIR_DATA_DIR", "");
    assert_eq!(data_dir(), gizai_own, "an empty HERSIR_DATA_DIR gives way to GIZAI_DATA_DIR");
    unset("GIZAI_DATA_DIR");
    assert_eq!(data_dir(), default_data_dir(), "both empty or unset: the usual folder");
    set("HERSIR_DATA_DIR", &hersir_own);
    assert_eq!(data_dir(), hersir_own, "HERSIR_DATA_DIR alone");
    unset("HERSIR_DATA_DIR");
    assert_eq!(listing(tmp.path()), before, "still nothing moved");
}
