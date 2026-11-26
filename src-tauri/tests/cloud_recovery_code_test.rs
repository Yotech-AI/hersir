//! GA-359 QA: after setting up Cloud Backup, the recovery code is in no keychain entry, no file and no log.
//!
//! A child process of this test program (this same program, run again with QA_CLOUD_CODE_CHILD set), as in
//! `setup_secret_log_test.rs`: `logs::start` points the child's fds 1 and 2 at Hersir's log, as in a release build. In it,
//! computer A signs in to the fake Hersir Cloud, makes the key, types a wrong last group and then the right one, backs
//! up, and makes a new recovery code and types its last group back (GA-370); computer B signs in, types a wrong code and
//! then the new one, and restores A's backup. Both keep their keychain in a file (`FileKeychain`), and the child writes
//! what `cloud-changed` sent and every byte the fake received into files of its own. The codes it was shown go into `codes.txt`, the only file the parent
//! doesn't search. The parent then searches every other file in the folder (both data folders, their databases and
//! backups, Hersir's log, both keychains, the events and what reached Hersir Cloud) for each code: as shown, without
//! dashes, in lower case, its 16 bytes, and their hex and base64. A program of its own: `start` points the whole
//! process's output at the file.
#![cfg(unix)]
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gizai_cloud_client::fake::{self, FakeCloud};
use gizai_cloud_client::keys::RecoveryCode;
use gizai_lib::{AppState, cloud, logs, restore, runs};

const CHILD: &str = "QA_CLOUD_CODE_CHILD";
const DIR: &str = "QA_CLOUD_CODE_DIR";

fn until<T>(what: &str, mut ok: impl FnMut() -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(120);
    loop {
        if let Some(t) = ok() {
            return t;
        }
        assert!(Instant::now() < end, "timed out: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Hersir on new data in `dir/<name>`, its keychain in `dir/keychain-<name>.json`, pointed at the fake; what
/// `cloud-changed` sends goes into `dir/events-<name>.jsonl`.
fn hersir(dir: &Path, name: &str, f: &FakeCloud) -> AppState {
    let mut st = gizai_lib::test_state(&dir.join(name));
    st.keychain = Arc::new(gizai_agents::secrets::FileKeychain::new(dir.join(format!("keychain-{name}.json"))));
    st.cloud.set_config(cloud::Config { server: f.url().into(), client_id: cloud::CLIENT_ID.into(), entitlement_key: Some(f.entitlement_key()) });
    let events = dir.join(format!("events-{name}.jsonl"));
    st.notify = Arc::new(move |n| {
        if let runs::Note::CloudChanged(s) = n {
            let mut file = fs::OpenOptions::new().create(true).append(true).open(&events).unwrap();
            writeln!(file, "{}", serde_json::to_string(&s).unwrap()).unwrap();
        }
    });
    st
}

fn browser(url: &str) -> Result<(), String> {
    let url = url.to_string();
    std::thread::spawn(move || {
        let _ = fake::browse(&url);
    });
    Ok(())
}

fn backup_ended(st: &AppState) {
    until("the cloud backup ended", || cloud::status(st).backup.running.is_none().then_some(()));
}

/// The setup the parent checks. Without QA_CLOUD_CODE_CHILD (the test program's own run) it does nothing.
#[test]
fn child_case() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    let dir = PathBuf::from(std::env::var_os(DIR).unwrap());
    logs::start(&dir.join("a").join("data"), true).unwrap();
    let f = FakeCloud::start().unwrap();
    let mut codes = fs::File::create(dir.join("codes.txt")).unwrap();

    // computer A: the first computer
    let a = hersir(&dir, "a", &f);
    cloud::sign_in(&a, browser, Duration::from_secs(60)).unwrap();
    let code = cloud::make_key(&a).unwrap();
    writeln!(codes, "{}", code.recovery_code).unwrap();
    eprintln!("QA: the code shown: {code:?}");
    assert!(cloud::confirm_recovery(&a, "zzz").is_err());
    let last = code.recovery_code.rsplit('-').next().unwrap().to_string();
    cloud::confirm_recovery(&a, &last).unwrap();
    cloud::back_up_now(&a).unwrap();
    backup_ended(&a);
    // a new code: the old one stops working once its last group is typed back (GA-370)
    let new = cloud::new_recovery(&a).unwrap();
    writeln!(codes, "{}", new.recovery_code).unwrap();
    assert!(cloud::confirm_recovery(&a, "zzz").is_err());
    cloud::confirm_recovery(&a, new.recovery_code.rsplit('-').next().unwrap()).unwrap();
    cloud::back_up_now(&a).unwrap();
    backup_ended(&a);
    assert!(cloud::status(&a).backup.last_error.is_none(), "{:?}", cloud::status(&a));

    // computer B: another computer, which restores
    let b = hersir(&dir, "b", &f);
    cloud::sign_in(&b, browser, Duration::from_secs(60)).unwrap();
    assert!(cloud::enter_recovery(&b, &code.recovery_code).is_err(), "the old code stopped working");
    cloud::enter_recovery(&b, &new.recovery_code.to_lowercase()).unwrap();
    let name = cloud::list_backups(&b).unwrap()[0].name.clone();
    cloud::start_restore(&b, &name).unwrap();
    let done = until("the restore ended", || {
        let s = restore::status(&b);
        matches!(s.state.as_str(), "ready" | "failed" | "none").then_some(s)
    });
    assert_eq!(done.state, "ready", "{done:?}");
    fs::write(dir.join("received.bin"), f.received()).unwrap();
    drop(codes);
    logs::quit();
    std::process::exit(0);
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Base64 without padding (the standard alphabet, or the URL-safe one).
fn b64(bytes: &[u8], url: bool) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (*b as u32) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            let c = B64[((n >> (18 - 6 * i)) & 63) as usize];
            out.push(match (url, c) {
                (true, b'+') => '-',
                (true, b'/') => '_',
                _ => c as char,
            });
        }
    }
    out
}

/// The files under `dir` (but `skip`) that hold `needle`.
fn in_files(dir: &Path, skip: &Path, needle: &[u8]) -> Vec<PathBuf> {
    let mut found = vec![];
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p == skip {
            continue;
        }
        if p.is_dir() {
            found.extend(in_files(&p, skip, needle));
        } else if let Ok(bytes) = fs::read(&p)
            && bytes.windows(needle.len()).any(|w| w == needle)
        {
            found.push(p);
        }
    }
    found
}

fn all_files(dir: &Path) -> Vec<PathBuf> {
    let mut v = vec![];
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            v.extend(all_files(&p));
        } else {
            v.push(p);
        }
    }
    v
}

#[test]
fn after_setup_the_recovery_code_is_in_no_keychain_entry_file_or_log() {
    let tmp = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let dir = tmp.path();
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_case", "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .env(DIR, dir)
        .env("RUST_BACKTRACE", "0")
        .output()
        .unwrap();
    let logs: Vec<PathBuf> = fs::read_dir(logs::folder(&dir.join("a").join("data"))).map(|d| d.flatten().map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "log")).collect()).unwrap_or_default();
    assert_eq!(logs.len(), 1, "one log: {logs:?}\n{}", String::from_utf8_lossy(&out.stderr));
    let log = fs::read_to_string(&logs[0]).unwrap();
    assert!(out.status.success(), "the child's setup worked: {:?}\n{log}", out.status);
    // the log holds the child's output (the redirect worked)
    assert!(log.contains("QA: the code shown: RecoveryShown(•••)"), "{log}");

    let codes_file = dir.join("codes.txt");
    let codes: Vec<String> = fs::read_to_string(&codes_file).unwrap().lines().map(String::from).collect();
    assert_eq!(codes.len(), 2, "{codes:?}");
    // what was searched: both keychains with their cloud entries, both databases, the backups, the log, the events,
    // and what Hersir Cloud received
    let files = all_files(dir);
    for (what, ok) in [
        ("A's keychain with its cloud key", fs::read_to_string(dir.join("keychain-a.json")).is_ok_and(|t| t.contains("cloud/1/key") && t.contains("cloud/1/token"))),
        ("B's keychain with its cloud key", fs::read_to_string(dir.join("keychain-b.json")).is_ok_and(|t| t.contains("cloud/1/key"))),
        ("A's database", dir.join("a/data/gizai.db").is_file()),
        ("B's database", dir.join("b/data/gizai.db").is_file()),
        ("the events", dir.join("events-a.jsonl").is_file() && dir.join("events-b.jsonl").is_file()),
        ("what Hersir Cloud received", fs::metadata(dir.join("received.bin")).is_ok_and(|m| m.len() > 1000)),
        ("a before-restore snapshot", files.iter().any(|p| p.to_string_lossy().contains("gizai-before-restore-"))),
    ] {
        assert!(ok, "{what} was searched: {files:?}");
    }
    for code in &codes {
        let parsed = RecoveryCode::parse(code).unwrap();
        let chars = code.trim_start_matches("HC-").replace('-', "");
        let bytes = parsed.as_bytes().to_vec();
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let needles: Vec<(&str, Vec<u8>)> = vec![
            ("as shown", code.as_bytes().to_vec()),
            ("in lower case", code.to_lowercase().into_bytes()),
            ("without dashes", chars.clone().into_bytes()),
            ("without dashes, in lower case", chars.to_lowercase().into_bytes()),
            ("its 16 bytes", bytes.clone()),
            ("its hex", hex.clone().into_bytes()),
            ("its hex, upper case", hex.to_uppercase().into_bytes()),
            ("its base64", b64(&bytes, false).into_bytes()),
            ("its URL-safe base64", b64(&bytes, true).into_bytes()),
        ];
        for (how, needle) in needles {
            let found = in_files(dir, &codes_file, &needle);
            assert!(found.is_empty(), "the recovery code ({how}) is in {found:?}");
        }
    }
}
