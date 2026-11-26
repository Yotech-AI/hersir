// GA-359 QA: Hersir Cloud in the app (src-tauri/src/cloud.rs) against GA-358's fake Hersir Cloud. Signing in through
// oauth.rs with the built-in client id (no registration) and `POST /devices`; the token, the account key and this
// computer's keys in the keychain, never in SQLite; the first computer's key and recovery code, another computer's
// recovery code, a new code; Back up now, daily (only when something changed), on quit and before a restore, page for
// page; Restore from Hersir Cloud on clean data, with the device rule and this computer's settings kept; a removed or
// signed-out computer; a lapsed plan that stops uploads while restore still works; the contract's sentences and JSON;
// and the built-in production values a release build keeps.
#[path = "../support/data_lock.rs"]
mod data_lock;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gizai_cloud_client::fake::{self, Failure, FakeCloud, Plan};
use gizai_cloud_client::keys::RecoveryCode;
use gizai_core::backups::DAY_MS;
use gizai_lib::cloud::{self, CloudStatus, Config, SettingsChange};
use gizai_lib::{AppState, restore, runs};

const REMOVED: &str = "This computer was removed from Hersir Cloud on the account page.";
const SIGN_IN_FIRST: &str = "Sign in to Hersir Cloud first.";
const WRONG_CODE: &str = "That recovery code doesn't open this account's backups.";

/// The fake's address, the built-in client id and the fake's entitlement key.
fn config(f: &FakeCloud) -> Config {
    Config { server: f.url().to_string(), client_id: cloud::CLIENT_ID.into(), entitlement_key: Some(f.entitlement_key()) }
}

type Heard = Arc<Mutex<Vec<CloudStatus>>>;

/// Hersir on new data in `dir`, pointed at the fake; what `cloud-changed` sends is kept.
fn hersir(dir: &Path, f: &FakeCloud) -> (AppState, Heard) {
    let mut st = gizai_lib::test_state(dir);
    st.cloud.set_config(config(f));
    let heard: Heard = Arc::default();
    let h = heard.clone();
    st.notify = Arc::new(move |n| {
        if let runs::Note::CloudChanged(s) = n {
            h.lock().unwrap().push(s);
        }
    });
    (st, heard)
}

/// The browser: follows the sign-in address on a thread of its own, as a browser does while Hersir waits.
fn browser(url: &str) -> Result<(), String> {
    let url = url.to_string();
    std::thread::spawn(move || {
        let _ = fake::browse(&url);
    });
    Ok(())
}

fn sign_in(st: &AppState) -> CloudStatus {
    cloud::sign_in(st, browser, Duration::from_secs(60)).expect("signed in")
}

/// Waits up to 120 s for `ok`.
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

/// Waits until no cloud backup runs, and returns the status then.
fn backup_ended(st: &AppState) -> CloudStatus {
    until("the cloud backup ended", || {
        let s = cloud::status(st);
        s.backup.running.is_none().then_some(s)
    })
}

/// The account's first computer: signed in, the key made, the code's last group typed back (lower case, with spaces).
/// Returns the recovery code as shown.
fn first_computer(st: &AppState) -> String {
    let s = sign_in(st);
    assert!(s.signed_in, "{s:?}");
    assert_eq!(s.key, "missing");
    let code = cloud::make_key(st).unwrap().recovery_code;
    let last = code.rsplit('-').next().unwrap().to_string();
    let s = cloud::confirm_recovery(st, &format!(" {} ", last.to_lowercase())).unwrap();
    assert_eq!(s.key, "here");
    assert!(s.backup.on, "confirming the code turns Cloud Backup on");
    code
}

/// Back up now, and its end.
fn back_up_now(st: &AppState) -> CloudStatus {
    let started = cloud::back_up_now(st).unwrap();
    assert!(started.backup.running.is_some(), "it runs once Back up now returns: {started:?}");
    backup_ended(st)
}

fn snapshots(f: &FakeCloud) -> Vec<gizai_cloud_client::api::Snapshot> {
    f.snapshots(&f.account().id)
}

fn reasons(f: &FakeCloud) -> Vec<String> {
    let mut v: Vec<_> = snapshots(f).into_iter().map(|s| s.reason).collect();
    v.sort();
    v
}

fn keychain(st: &AppState, account: &str, what: &str) -> Option<String> {
    st.keychain.get(&cloud::keychain_key(account, what)).unwrap()
}

/// A new project: a `changes` row, so a daily cloud backup has something new.
fn change(st: &AppState, key: &str) -> String {
    gizai_core::projects::create(&st.db, &st.you_id, gizai_core::model::ProjectInput { name: format!("Project {key}"), key: key.into(), ..Default::default() }).unwrap()
}

fn attach(st: &AppState, name: &str, bytes: &[u8]) -> String {
    let blob = gizai_core::files::store_bytes(&st.data_dir, name, bytes).unwrap();
    st.db.write(Some(&st.you_id), |w| gizai_core::files::insert(w, Some(&st.you_id), "actor", &st.you_id, &blob)).unwrap();
    blob.sha256
}

fn files_rows(st: &AppState) -> Vec<String> {
    st.db.read(|c| {
        let mut s = c.prepare("SELECT sha256 FROM files ORDER BY sha256")?;
        let v = s.query_map([], |r| r.get(0))?.collect::<Result<Vec<String>, _>>()?;
        Ok(v)
    }).unwrap()
}

/// `devices` as (id, is_self).
fn devices_rows(st: &AppState) -> Vec<(String, bool)> {
    st.db.read(|c| {
        let mut s = c.prepare("SELECT id, is_self FROM devices ORDER BY id")?;
        let v = s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? == 1)))?.collect::<Result<Vec<_>, _>>()?;
        Ok(v)
    }).unwrap()
}

fn json_keys(v: &serde_json::Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    k.sort();
    k
}

/// Whether `needle` is anywhere in the files under `dir`.
fn in_files(dir: &Path, needle: &[u8]) -> Vec<PathBuf> {
    let mut found = vec![];
    let Ok(rd) = fs::read_dir(dir) else { return found };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            found.extend(in_files(&p, needle));
        } else if let Ok(bytes) = fs::read(&p)
            && bytes.windows(needle.len()).any(|w| w == needle)
        {
            found.push(p);
        }
    }
    found
}

// ---- the first computer: sign in, the key, Back up now ----

#[test]
fn against_the_fake_sign_in_make_the_key_confirm_the_code_and_back_up_now_puts_the_snapshot_in_hersir_cloud() {
    let f = FakeCloud::start().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let (st, heard) = hersir(tmp.path(), &f);
    let account = f.account().id;
    let me = st.db.device_id().to_string();

    let out = cloud::status(&st);
    assert!(!out.signed_in);
    assert_eq!((out.email.as_deref(), out.plan.as_ref(), out.key.as_str(), out.signing_in), (None, None, "missing", false));

    // sign in: oauth.rs with the built-in client id, so no registration; then POST /devices
    let s = sign_in(&st);
    assert!(s.signed_in);
    assert_eq!(s.email.as_deref(), Some(fake::EMAIL));
    assert!(!s.signing_in);
    assert_eq!(s.key, "missing");
    let plan = s.plan.clone().expect("the checked entitlement fills the plan after sign-in");
    assert_eq!((plan.name.as_str(), plan.status.as_str(), plan.quota_bytes), ("personal", "active", fake::QUOTA));
    let calls = f.calls();
    assert!(!calls.iter().any(|c| c.path.contains("register")), "no dynamic registration: {calls:?}");
    assert!(calls.iter().any(|c| c.method == "POST" && c.path.ends_with("/devices") && c.status < 300), "{calls:?}");
    assert!(calls.iter().any(|c| c.method == "GET" && c.path.ends_with("/me/entitlements")), "{calls:?}");
    let devices = f.devices(&account);
    assert_eq!(devices.len(), 1, "{devices:?}");
    let d = &devices[0];
    assert!(d.id.eq_ignore_ascii_case(&me), "this computer's own device id: {} vs {me}", d.id);
    assert_eq!(d.kind, "desktop");
    assert_eq!(d.platform, std::env::consts::OS);
    assert_eq!(d.app_version, env!("CARGO_PKG_VERSION"));
    assert!(!d.sign_pub.is_empty() && !d.box_pub.is_empty());
    // the token and this computer's keys in the keychain (cloud/<account>/…), never in SQLite
    let token_text = keychain(&st, &account, "token").expect("the token is in the keychain");
    let token = serde_json::from_str::<serde_json::Value>(&token_text).unwrap()["token"].as_str().unwrap().to_string();
    assert!(token.len() > 10);
    assert!(keychain(&st, &account, "device").is_some());
    assert!(keychain(&st, &account, "key").is_none(), "no key before it is made");
    let in_db = in_files(&st.data_dir, token.as_bytes());
    assert!(in_db.is_empty(), "the token is in no file of the data folder: {in_db:?}");

    // the first computer: make the key; a wrong last group is refused, the right one in another case is taken
    let code = cloud::make_key(&st).unwrap().recovery_code;
    assert!(code.starts_with("HC-"), "{code}");
    assert!(f.key(&account).is_none(), "nothing goes to Hersir Cloud before the code is confirmed");
    assert_eq!(cloud::confirm_recovery(&st, "ZZZ").unwrap_err(), cloud::WRONG_GROUP);
    assert_eq!(cloud::status(&st).key, "missing");
    let last = code.rsplit('-').next().unwrap();
    let s = cloud::confirm_recovery(&st, &format!("  {}  ", last.to_lowercase())).unwrap();
    assert_eq!(s.key, "here");
    assert!(s.backup.on);
    assert_eq!(s.backup.every, "daily");
    assert!(s.recovery_made_at.is_some());
    assert!(s.backup.next.is_some(), "a daily backup is due");
    let record = f.key(&account).expect("PUT /keys stored the sealed key");
    assert_eq!(record.epoch, 1);
    assert!(keychain(&st, &account, "key").is_some(), "the account key is in the keychain");
    // a second make_key is refused: the key is here
    assert!(cloud::make_key(&st).is_err());

    // Back up now: a manual snapshot in Hersir Cloud, from this computer
    let a = attach(&st, "a.txt", b"backed up to the fake");
    heard.lock().unwrap().clear();
    let done = back_up_now(&st);
    let snaps = snapshots(&f);
    assert_eq!(snaps.len(), 1, "{snaps:?}");
    let snap = &snaps[0];
    assert_eq!(snap.reason, "manual");
    assert!(snap.device_id.eq_ignore_ascii_case(&me));
    assert!(snap.name.starts_with("gizai-manual-"), "{}", snap.name);
    let last = done.backup.last.clone().expect("the last cloud backup");
    assert_eq!(last.name, snap.name);
    assert!(last.sent_bytes > 0);
    assert_eq!(done.backup.last_error, None);
    // cloud-changed said each step, and the end
    let steps: Vec<String> = heard.lock().unwrap().iter().filter_map(|s| s.backup.running.as_ref().map(|r| r.step.clone())).collect();
    for step in ["copying", "chunking", "uploading", "registering"] {
        assert!(steps.iter().any(|s| s == step), "cloud-changed said {step}: {steps:?}");
    }
    assert_eq!(heard.lock().unwrap().last().unwrap().backup.running, None);
    // the page-for-page copy is gone from the data folder
    let left: Vec<String> = fs::read_dir(st.data_dir.join("backups")).map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(".cloud-")).collect()).unwrap_or_default();
    assert!(left.is_empty(), "{left:?}");
    assert!(f.snapshot_objects(&account, &snap.name).len() >= 2, "the manifest and the database's chunks");
    let _ = a;

    // the list, the computers and the account page
    let list = cloud::list_backups(&st).unwrap();
    assert_eq!(list.len(), 1);
    let b = &list[0];
    assert_eq!((b.name.as_str(), b.reason.as_str(), b.restorable, b.problem.as_deref()), (snap.name.as_str(), "manual", true, None));
    assert!(b.device.this);
    assert!(b.bytes > 0 && b.made_at > 0);
    let computers = cloud::computers(&st).unwrap();
    assert_eq!(computers.len(), 1);
    assert!(computers[0].this);
    assert!(!computers[0].chief, "chief is always false");
    assert_eq!(cloud::account_url(&st), format!("{}/account", f.url()));

    // the window's JSON is the contract's (research 5.4)
    let json = serde_json::to_value(cloud::status(&st)).unwrap();
    assert_eq!(json_keys(&json), ["backup", "email", "key", "plan", "recoveryMadeAt", "signedIn", "signingIn"]);
    assert_eq!(json_keys(&json["backup"]), ["every", "last", "lastError", "next", "on", "onQuit", "running"]);
    assert_eq!(json_keys(&json["backup"]["last"]), ["at", "name", "sentBytes"]);
    assert_eq!(json_keys(&json["plan"]), ["name", "quotaBytes", "readOnlyUntil", "status", "usedBytes", "validUntil"]);
    let json = serde_json::to_value(&list[0]).unwrap();
    assert_eq!(json_keys(&json), ["bytes", "device", "madeAt", "name", "problem", "reason", "restorable"]);
    assert_eq!(json_keys(&json["device"]), ["id", "name", "this"]);
    let json = serde_json::to_value(&computers[0]).unwrap();
    assert_eq!(json_keys(&json), ["appVersion", "chief", "id", "lastSeenAt", "name", "online", "platform", "this"]);
    // cloud_save_settings' argument is { on?, every?, onQuit? }
    let change: SettingsChange = serde_json::from_str(r#"{"onQuit":true,"every":"manual"}"#).unwrap();
    let s = cloud::save_settings(&st, change).unwrap();
    assert_eq!((s.backup.on, s.backup.every.as_str(), s.backup.on_quit, s.backup.next), (true, "manual", true, None));
    assert!(cloud::save_settings(&st, SettingsChange { every: Some("weekly".into()), ..Default::default() }).is_err());
    // the switches are this computer's settings
    for key in [cloud::ACCOUNT_KEY, cloud::BACKUP_KEY, cloud::LAST_BACKUP_KEY, cloud::LAST_ERROR_KEY] {
        assert!(gizai_core::scope::this_computer(key), "{key} is this computer's");
    }
}

// ---- when a cloud backup is made ----

#[test]
fn daily_backs_up_only_when_a_day_passed_and_something_changed_on_quit_with_on_quit_on_and_never_with_cloud_backup_off() {
    let f = FakeCloud::start().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let (st, _heard) = hersir(tmp.path(), &f);
    first_computer(&st);
    change(&st, "ONE");
    let now = gizai_core::ids::now_ms();

    // the first daily one: none made yet
    let made = cloud::hourly(&st, now).expect("a daily backup is due").expect("made");
    assert_eq!(reasons(&f), ["daily"]);
    assert!(made.name.starts_with("gizai-daily-"), "{}", made.name);
    // not again the same day
    assert!(cloud::hourly(&st, now + 3_600_000).is_none());
    // a day later, with nothing changed since: none
    assert!(cloud::hourly(&st, now + DAY_MS + 3_600_000).is_none(), "nothing changed since the last cloud backup");
    assert_eq!(snapshots(&f).len(), 1);
    // a day later, after a change: one
    change(&st, "TWO");
    assert!(cloud::hourly(&st, now + DAY_MS + 3_600_000).is_some_and(|r| r.is_ok()));
    assert_eq!(reasons(&f), ["daily", "daily"]);
    // Only when I ask: no daily one
    change(&st, "THREE");
    cloud::save_settings(&st, SettingsChange { every: Some("manual".into()), ..Default::default() }).unwrap();
    assert!(cloud::hourly(&st, now + 3 * DAY_MS).is_none());

    // on quit, with On quit off: none; with it on: a quit backup
    cloud::quit_backup(&st, Instant::now() + Duration::from_secs(60));
    assert_eq!(snapshots(&f).len(), 2);
    cloud::save_settings(&st, SettingsChange { on_quit: Some(true), ..Default::default() }).unwrap();
    cloud::quit_backup(&st, Instant::now() + Duration::from_secs(60));
    assert_eq!(reasons(&f), ["daily", "daily", "quit"]);

    // Cloud Backup off: no daily, no quit backup
    cloud::save_settings(&st, SettingsChange { on: Some(false), every: Some("daily".into()), ..Default::default() }).unwrap();
    change(&st, "FOUR");
    assert!(cloud::hourly(&st, now + 5 * DAY_MS).is_none());
    cloud::quit_backup(&st, Instant::now() + Duration::from_secs(60));
    assert_eq!(snapshots(&f).len(), 3);
    // and Back up now still works with it off
    back_up_now(&st);
    assert_eq!(reasons(&f), ["daily", "daily", "manual", "quit"]);
}

#[test]
fn the_copy_is_page_for_page_so_free_pages_keep_their_place_unlike_vacuum_into() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    // free pages in the middle of the database: VACUUM INTO would drop them and move the pages after them
    st.db.write(None, |w| {
        w.conn().execute_batch("CREATE TABLE qa_pad(x BLOB); CREATE TABLE qa_after(x BLOB);")?;
        for _ in 0..64 {
            w.conn().execute("INSERT INTO qa_pad(x) VALUES (zeroblob(4000))", [])?;
        }
        w.conn().execute("INSERT INTO qa_after(x) VALUES (randomblob(20000))", [])?;
        w.conn().execute("DELETE FROM qa_pad", [])?;
        Ok(())
    }).unwrap();
    let free: i64 = st.db.read(|c| Ok(c.query_row("PRAGMA freelist_count", [], |r| r.get(0))?)).unwrap();
    assert!(free > 10, "the test made free pages: {free}");
    let dest = tmp.path().join("copy.db");
    let mut heard = vec![];
    let copy = gizai_core::backups::copy_pages(&st.data_dir, st.db.device_id(), &dest, &mut |done, total| {
        heard.push((done, total));
        Ok(())
    }).unwrap();
    assert_eq!(copy.path, dest);
    assert_eq!(copy.device.id, st.db.device_id());
    assert_eq!(copy.schema_version, gizai_core::db::SCHEMA_VERSION);
    assert_eq!(copy.last_change.as_ref().map(|c| c.id.clone()), gizai_core::backups::newest_change_id(&st.db));
    let (done, total) = *heard.last().unwrap();
    assert!(total > 0 && done == total, "{heard:?}");
    // one file, nothing beside it
    for end in ["-wal", "-shm", "-journal"] {
        assert!(!tmp.path().join(format!("copy.db{end}")).exists(), "{end}");
    }
    // the same pages in the same places: compare with the database itself, its WAL folded in
    // (Hersir closed: its last connection folds the WAL into the database)
    let data = st.data_dir.clone();
    drop(st);
    data_lock::released_blocking(&data);
    let wal = fs::metadata(data.join("gizai.db-wal")).map(|m| m.len()).unwrap_or(0);
    assert_eq!(wal, 0, "nothing left in the WAL");
    let src = fs::read(data.join("gizai.db")).unwrap();
    let dst = fs::read(&dest).unwrap();
    let page = u16::from_be_bytes([src[16], src[17]]) as usize;
    let page = if page == 1 { 65536 } else { page };
    assert_eq!(src.len(), dst.len(), "as many pages: free pages kept");
    assert_eq!(&dst[36..40], &src[36..40], "the same freelist count");
    let differ: Vec<usize> = (1..src.len() / page).filter(|i| src[i * page..(i + 1) * page] != dst[i * page..(i + 1) * page]).collect();
    assert!(differ.is_empty(), "every page after the first is the same, at the same number: {differ:?}");
    // the copy opens as a database in journal mode DELETE (bytes 18 and 19 are 1: not WAL)
    assert_eq!((dst[18], dst[19]), (1, 1));
    gizai_core::backups::remove_copy(&dest);
    assert!(!dest.exists());
}

// ---- Restore from Hersir Cloud on clean data ----

#[test]
fn restore_on_clean_data_signs_in_takes_the_recovery_code_restores_and_after_the_restart_the_data_is_back_with_the_device_rule_and_this_computers_settings() {
    let f = FakeCloud::start().unwrap();
    // computer A backs up its data
    let tmp_a = tempfile::tempdir().unwrap();
    let (a, _) = hersir(tmp_a.path(), &f);
    let code = first_computer(&a);
    change(&a, "KADE");
    let blob = attach(&a, "from-a.txt", b"made on computer A");
    gizai_core::settings::set(&a.db, "max_concurrent_runs", &2).unwrap();
    gizai_core::settings::set(&a.db, "qa_shared_setting", &"from A").unwrap();
    back_up_now(&a);
    let snap = snapshots(&f).remove(0);
    let a_device = a.db.device_id().to_string();

    // computer B: a new install, a scratch data folder
    let tmp_b = tempfile::tempdir().unwrap();
    let (b, heard) = hersir(tmp_b.path(), &f);
    let b_device = b.db.device_id().to_string();
    assert_ne!(a_device, b_device);
    gizai_core::settings::set(&b.db, "max_concurrent_runs", &7).unwrap();
    let pending_before = gizai_core::onboarding::pending(&b.db).unwrap();
    assert!(files_rows(&b).is_empty());
    let s = sign_in(&b);
    assert_eq!(s.key, "needed", "the account has a key, this computer doesn't");
    // without the key: no Cloud Backup, no Back up now, no restore
    assert_eq!(cloud::save_settings(&b, SettingsChange { on: Some(true), ..Default::default() }).unwrap_err(), cloud::CODE_FIRST);
    assert_eq!(cloud::back_up_now(&b).unwrap_err(), cloud::CODE_FIRST);
    assert_eq!(cloud::start_restore(&b, &snap.name).unwrap_err(), cloud::CODE_FIRST);
    assert!(cloud::make_key(&b).is_err(), "the account has a key: no second one");
    // a wrong code (a real one, of no account), then the right one as typed: lower case, no dashes
    let wrong = RecoveryCode::generate().unwrap().shown().to_string();
    assert_eq!(cloud::enter_recovery(&b, &wrong).unwrap_err(), WRONG_CODE);
    assert_eq!(cloud::status(&b).key, "needed");
    let typed = code.to_lowercase().replace('-', " ");
    let s = cloud::enter_recovery(&b, &typed).unwrap();
    assert_eq!(s.key, "here");
    assert!(!s.backup.on, "entering the code doesn't turn Cloud Backup on");

    // the list: A's backup, restorable
    let list = cloud::list_backups(&b).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, snap.name);
    assert!(!list[0].device.this);
    assert!(list[0].device.id.eq_ignore_ascii_case(&a_device));
    assert!(list[0].restorable);
    // refused for a backup that isn't there
    assert!(cloud::start_restore(&b, "gizai-manual-nothing").is_err());

    let started = cloud::start_restore(&b, &snap.name).unwrap();
    assert_eq!(started.state, "checking");
    assert_eq!(started.step.as_deref(), Some(restore::DOWNLOADING_STEP));
    assert_eq!(started.folder.as_deref(), Some(restore::cloud_dir(&b).to_string_lossy().as_ref()));
    let done = until("the restore ended", || {
        let s = restore::status(&b);
        matches!(s.state.as_str(), "ready" | "failed" | "none").then_some(s)
    });
    assert_eq!(done.state, "ready", "{done:?}");
    assert_eq!(done.snapshot.as_ref().map(|e| e.name.as_str()), Some(snap.name.as_str()));
    // today's data was saved first, in the data folder's own backups (no folder in Settings)
    let saved = done.saved_today.clone().expect("today's data saved first");
    assert!(saved.starts_with("gizai-before-restore-"), "{saved}");
    // Cloud Backup is off on B: no before-restore backup to Hersir Cloud
    assert_eq!(reasons(&f), ["manual"]);
    // cloud-changed said the download's steps
    let steps: Vec<String> = heard.lock().unwrap().iter().filter_map(|s| s.backup.running.as_ref().map(|r| r.step.clone())).collect();
    assert!(steps.iter().any(|s| s == "downloading"), "{steps:?}");
    assert!(steps.iter().any(|s| s == "checking"), "{steps:?}");
    assert_eq!(cloud::status(&b).backup.running, None);
    // today's data keeps working until the restart
    assert!(files_rows(&b).is_empty());

    // the restart
    let data = b.data_dir.clone();
    let b_account = gizai_core::settings::get::<serde_json::Value>(&b.db, cloud::ACCOUNT_KEY).unwrap();
    drop(b);
    data_lock::released_blocking(&data);
    let b = gizai_lib::test_state(tmp_b.path());
    // the data is back
    assert_eq!(files_rows(&b), vec![blob.clone()]);
    assert!(gizai_core::files::blob_path(&b.data_dir, &blob).is_file());
    assert!(gizai_core::projects::list(&b.db).unwrap().iter().any(|p| p.key == "KADE"));
    assert_eq!(gizai_core::settings::get::<String>(&b.db, "qa_shared_setting").unwrap().as_deref(), Some("from A"));
    // the device rule: this computer keeps its own id, A stays as another computer
    assert_eq!(b.db.device_id(), b_device);
    let rows = devices_rows(&b);
    assert!(rows.iter().any(|(id, me)| id == &b_device && *me), "{rows:?}");
    assert!(rows.iter().any(|(id, me)| id == &a_device && !*me), "{rows:?}");
    assert_eq!(rows.iter().filter(|(_, me)| *me).count(), 1, "{rows:?}");
    // this computer's settings are kept: B's runs limit and its Hersir Cloud account
    assert_eq!(gizai_core::settings::get::<i64>(&b.db, "max_concurrent_runs").unwrap(), Some(7));
    assert_eq!(gizai_core::settings::get::<serde_json::Value>(&b.db, cloud::ACCOUNT_KEY).unwrap(), b_account);
    let result = restore::status(&b).result.expect("the restore's result");
    assert!(result.ok, "{result:?}");
    assert_eq!(result.snapshot, snap.name);
    // the download went with the restore, and the first start's onboarding no longer waits
    assert!(!restore::cloud_dir(&b).exists());
    assert!(!gizai_core::onboarding::pending(&b.db).unwrap(), "onboarding skipped after the restore (it waited before: {pending_before})");
}

#[test]
fn with_cloud_backup_on_a_restore_sends_todays_data_to_hersir_cloud_first() {
    let f = FakeCloud::start().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let (st, _) = hersir(tmp.path(), &f);
    first_computer(&st);
    back_up_now(&st);
    let name = snapshots(&f)[0].name.clone();
    change(&st, "LATER");
    cloud::start_restore(&st, &name).unwrap();
    let done = until("the restore ended", || {
        let s = restore::status(&st);
        matches!(s.state.as_str(), "ready" | "failed" | "none").then_some(s)
    });
    assert_eq!(done.state, "ready", "{done:?}");
    assert_eq!(reasons(&f), ["before-restore", "manual"], "today's data went to Hersir Cloud too");
    restore::cancel(&st).unwrap();
    assert!(!restore::cloud_dir(&st).exists());
}

// ---- a removed or signed-out computer ----

#[test]
fn a_removed_computer_shows_as_signed_out_with_the_protocols_message_and_forgets_its_keys() {
    let f = FakeCloud::start().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let (st, heard) = hersir(tmp.path(), &f);
    let account = f.account().id;
    first_computer(&st);
    f.remove_device(st.db.device_id(), false);

    let s = cloud::refresh(&st);
    assert!(!s.signed_in, "{s:?}");
    assert_eq!(s.email, None);
    assert_eq!(s.plan, None);
    assert_eq!(s.backup.last_error.as_ref().map(|e| e.message.as_str()), Some(REMOVED));
    assert!(heard.lock().unwrap().last().is_some_and(|s| !s.signed_in), "cloud-changed said so");
    for what in ["token", "key", "device"] {
        assert!(keychain(&st, &account, what).is_none(), "{what} forgotten");
    }
    assert_eq!(cloud::make_key(&st).unwrap_err(), SIGN_IN_FIRST);
    assert_eq!(cloud::back_up_now(&st).unwrap_err(), SIGN_IN_FIRST);
    // the message stays until it signs in again
    assert_eq!(cloud::status(&st).backup.last_error.map(|e| e.message), Some(REMOVED.to_string()));
    let s = sign_in(&st);
    assert!(s.signed_in);
    assert_eq!(s.backup.last_error, None);
    assert_eq!(s.key, "needed", "the key was forgotten here, the account still has it");
}

#[test]
fn removed_during_a_backup_signs_out_too() {
    let f = FakeCloud::start().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let (st, _) = hersir(tmp.path(), &f);
    first_computer(&st);
    f.fail_next(Failure::new("POST", "/snapshots", 401, "removed", REMOVED));
    let s = back_up_now(&st);
    assert!(!s.signed_in, "{s:?}");
    assert_eq!(s.backup.last_error.map(|e| e.message), Some(REMOVED.to_string()));
    assert!(snapshots(&f).is_empty());
}

#[test]
fn a_signed_out_computer_forgets_only_its_token_and_keeps_the_key_for_the_next_sign_in() {
    let f = FakeCloud::start().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let (st, _) = hersir(tmp.path(), &f);
    let account = f.account().id;
    first_computer(&st);
    f.fail_next(Failure::new("GET", "/me/entitlements", 401, "signed_out", "Sign in to Hersir Cloud again."));
    let s = cloud::refresh(&st);
    assert!(!s.signed_in);
    assert_eq!(s.backup.last_error.map(|e| e.message), Some("Sign in to Hersir Cloud again.".to_string()));
    assert!(keychain(&st, &account, "token").is_none());
    assert!(keychain(&st, &account, "key").is_some(), "the key stays");
    let s = sign_in(&st);
    assert_eq!(s.key, "here");
}

#[test]
fn sign_out_removes_this_computer_on_hersir_cloud_and_forgets_the_token_and_keys() {
    let f = FakeCloud::start().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let (st, _) = hersir(tmp.path(), &f);
    let account = f.account().id;
    first_computer(&st);
    let s = cloud::sign_out(&st).unwrap();
    assert!(!s.signed_in);
    assert_eq!(s.backup.last_error, None);
    assert!(f.calls().iter().any(|c| c.method == "DELETE" && c.path.ends_with("/devices/me")), "{:?}", f.calls());
    for what in ["token", "key", "device"] {
        assert!(keychain(&st, &account, what).is_none(), "{what} forgotten");
    }
    assert!(f.devices(&account).iter().all(|d| d.revoked_at.is_some()));
}

#[test]
fn removing_another_computer_lists_the_rest_and_that_computer_hears_it_was_removed() {
    let f = FakeCloud::start().unwrap();
    let (tmp_a, tmp_b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (a, _) = hersir(tmp_a.path(), &f);
    let (b, _) = hersir(tmp_b.path(), &f);
    first_computer(&a);
    sign_in(&b);
    let all = cloud::computers(&a).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all.iter().filter(|c| c.this).count(), 1);
    assert!(all.iter().all(|c| !c.chief));
    let other = all.iter().find(|c| !c.this).unwrap().id.clone();
    assert!(other.eq_ignore_ascii_case(b.db.device_id()));
    let left = cloud::remove_computer(&a, &other, true).unwrap();
    assert_eq!(left.len(), 1);
    assert!(left[0].this);
    assert!(f.lost(&other));
    let s = cloud::refresh(&b);
    assert!(!s.signed_in);
    assert_eq!(s.backup.last_error.map(|e| e.message), Some(REMOVED.to_string()));
    // removing this computer itself signs it out
    assert_eq!(cloud::remove_computer(&a, a.db.device_id(), false).unwrap(), vec![]);
    assert!(!cloud::status(&a).signed_in);
}

// ---- the plan ----

#[test]
fn a_lapsed_plan_stops_uploads_and_restore_still_works() {
    let f = FakeCloud::start().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let (st, _) = hersir(tmp.path(), &f);
    let account = f.account().id;
    first_computer(&st);
    back_up_now(&st);
    let name = snapshots(&f)[0].name.clone();
    // ended 9 Dec 2026, read only until 7 Feb 2027 (noon UTC: the same day in local time)
    f.set_plan(&account, Plan::Lapsed { valid_until: 1_796_817_600_000, read_only_until: 1_802_001_600_000 });
    change(&st, "AFTER");

    let s = cloud::refresh(&st);
    assert_eq!(s.plan.as_ref().map(|p| p.status.as_str()), Some("lapsed"));
    assert_eq!(cloud::back_up_now(&st).unwrap_err(), "Your plan ended on 9 Dec 2026: backups can be restored until 7 Feb 2027.");
    assert!(cloud::hourly(&st, gizai_core::ids::now_ms() + 2 * DAY_MS).is_none());
    cloud::save_settings(&st, SettingsChange { on_quit: Some(true), ..Default::default() }).unwrap();
    cloud::quit_backup(&st, Instant::now() + Duration::from_secs(30));
    assert_eq!(snapshots(&f).len(), 1, "no upload with a lapsed plan");
    // and when the plan isn't known here (no entitlement key built in), Hersir Cloud's 402 stops it
    st.cloud.set_config(Config { entitlement_key: None, ..config(&f) });
    let s = back_up_now(&st);
    assert!(s.backup.last_error.is_some(), "{s:?}");
    assert_eq!(snapshots(&f).len(), 1);
    st.cloud.set_config(config(&f));

    // restore still works
    assert_eq!(cloud::list_backups(&st).unwrap().len(), 1);
    cloud::start_restore(&st, &name).unwrap();
    let done = until("the restore ended", || {
        let s = restore::status(&st);
        matches!(s.state.as_str(), "ready" | "failed" | "none").then_some(s)
    });
    assert_eq!(done.state, "ready", "{done:?}");
    restore::cancel(&st).unwrap();
}

#[test]
fn the_plan_is_shown_only_from_an_entitlement_that_checks_and_is_asked_for_again_each_hour() {
    let f = FakeCloud::start().unwrap();
    let other = FakeCloud::start().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let (st, _) = hersir(tmp.path(), &f);
    let account = f.account().id;
    // the wrong key: no plan
    st.cloud.set_config(Config { entitlement_key: Some(other.entitlement_key()), ..config(&f) });
    let s = sign_in(&st);
    assert!(s.signed_in);
    assert_eq!(s.plan, None, "an entitlement that doesn't check fills no plan");
    // the placeholder (no key built in): no plan either
    st.cloud.set_config(Config { entitlement_key: None, ..config(&f) });
    assert_eq!(cloud::refresh(&st).plan, None);
    // the right key
    st.cloud.set_config(config(&f));
    assert_eq!(cloud::refresh(&st).plan.map(|p| p.status), Some("active".to_string()));
    // the hourly check asks again
    f.set_plan(&account, Plan::PastDue);
    cloud::hourly(&st, gizai_core::ids::now_ms());
    assert_eq!(cloud::status(&st).plan.map(|p| p.status), Some("past_due".to_string()));
}

#[test]
fn a_full_hersir_cloud_says_so_with_the_real_numbers() {
    let f = FakeCloud::start().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let (st, _) = hersir(tmp.path(), &f);
    first_computer(&st);
    f.fail_next(Failure::new("POST", "/objects/upload", 413, "quota", "Hersir Cloud is full."));
    let s = back_up_now(&st);
    let message = s.backup.last_error.map(|e| e.message).unwrap_or_default();
    assert!(message.starts_with("Hersir Cloud is full: ") && message.ends_with(" of 10 GB."), "{message}");
    assert!(snapshots(&f).is_empty());
}

// ---- the token ----

#[test]
fn the_token_is_renewed_in_its_last_30_days_and_not_before() {
    let f = FakeCloud::start().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let (st, _) = hersir(tmp.path(), &f);
    let account = f.account().id;
    sign_in(&st);
    let renewals = || f.calls().iter().filter(|c| c.method == "POST" && c.path.ends_with("/devices/me/token")).count();
    cloud::refresh(&st);
    assert_eq!(renewals(), 0, "a new token isn't renewed");
    // 10 days left
    let text = keychain(&st, &account, "token").unwrap();
    let mut t: serde_json::Value = serde_json::from_str(&text).unwrap();
    let old = t["token"].as_str().unwrap().to_string();
    t["expires_at"] = (gizai_core::ids::now_ms() + 10 * DAY_MS).into();
    st.keychain.set(&cloud::keychain_key(&account, "token"), &t.to_string()).unwrap();
    let s = cloud::refresh(&st);
    assert!(s.signed_in);
    assert_eq!(renewals(), 1);
    let t: serde_json::Value = serde_json::from_str(&keychain(&st, &account, "token").unwrap()).unwrap();
    assert_ne!(t["token"].as_str().unwrap(), old, "the new token is kept");
    assert!(t["expires_at"].as_i64().unwrap() > gizai_core::ids::now_ms() + 30 * DAY_MS);
    // and the new one works
    assert!(cloud::computers(&st).is_ok());
}

// ---- the keys ----

/// The `PUT /keys` calls the fake answered.
fn key_puts(f: &FakeCloud) -> usize {
    f.calls().iter().filter(|c| c.method == "PUT" && c.path.ends_with("/keys")).count()
}

/// A recovery code's last group, as shown.
fn last_group(code: &str) -> &str {
    code.rsplit('-').next().unwrap()
}

/// What a new recovery code must leave as it is until its last group is typed back: the sign-in, this computer's key,
/// when the code was made, Cloud Backup's settings and the plan (`next` follows the clock, so it isn't compared).
fn kept(s: &CloudStatus) -> (bool, String, Option<i64>, bool, String, bool, Option<cloud::CloudPlan>) {
    (s.signed_in, s.key.clone(), s.recovery_made_at, s.backup.on, s.backup.every.clone(), s.backup.on_quit, s.plan.clone())
}

// GA-370: a new recovery code is kept in memory until its last group is typed back (`confirm_recovery`); only then does
// `PUT /keys` store the key sealed by it, from the key's epoch. Until then the old code keeps working.
#[test]
fn a_new_recovery_code_replaces_the_old_one_only_once_its_last_group_is_typed_back() {
    let f = FakeCloud::start().unwrap();
    let (tmp_a, tmp_b, tmp_c) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (a, heard) = hersir(tmp_a.path(), &f);
    let account = f.account().id;
    let old = first_computer(&a);
    // Cloud Backup's settings of this computer's own: a new code leaves them as they are
    cloud::save_settings(&a, SettingsChange { every: Some("manual".into()), on_quit: Some(true), ..Default::default() }).unwrap();
    let before = cloud::status(&a);
    let made = before.recovery_made_at.unwrap();
    let record = f.key(&account).unwrap();
    let key_here = keychain(&a, &account, "key").unwrap();
    std::thread::sleep(Duration::from_millis(5));

    // made and shown: nothing changes yet
    let new = cloud::new_recovery(&a).unwrap().recovery_code;
    assert!(new.starts_with("HC-") && new != old, "{new}");
    assert_eq!(key_puts(&f), 1, "nothing goes to Hersir Cloud before its last group is typed back");
    assert_eq!(f.key(&account).as_ref(), Some(&record));
    assert_eq!(kept(&cloud::status(&a)), kept(&before), "nothing changes on this computer either");
    // another computer: the old code still works, the new one doesn't yet
    let (b, _) = hersir(tmp_b.path(), &f);
    sign_in(&b);
    assert_eq!(cloud::enter_recovery(&b, &new).unwrap_err(), WRONG_CODE, "the new code doesn't work yet");
    assert_eq!(cloud::enter_recovery(&b, &old).unwrap().key, "here", "the old code keeps working");

    // a wrong last group (also the old code's) is refused and changes nothing: the code waits on
    assert_eq!(cloud::confirm_recovery(&a, "ZZZ").unwrap_err(), cloud::WRONG_GROUP);
    if last_group(&old) != last_group(&new) {
        assert_eq!(cloud::confirm_recovery(&a, last_group(&old)).unwrap_err(), cloud::WRONG_GROUP);
    }
    assert_eq!(key_puts(&f), 1);
    assert_eq!(f.key(&account).as_ref(), Some(&record));
    assert_eq!(kept(&cloud::status(&a)), kept(&before));

    // its last group, in another case and with spaces: stored from the key's epoch, the same account key
    heard.lock().unwrap().clear();
    let s = cloud::confirm_recovery(&a, &format!("  {} ", last_group(&new).to_lowercase())).unwrap();
    assert_eq!(key_puts(&f), 2);
    let stored = f.key(&account).unwrap();
    assert_eq!((stored.key_id.as_str(), stored.epoch), (record.key_id.as_str(), record.epoch), "the same account key");
    // stored over the account's key: the fake answers 409 epoch to a PUT /keys whose if_epoch isn't the stored key's epoch
    // (null included), so it went with if_epoch, the key's epoch
    assert_ne!(stored.sealed_by_code, record.sealed_by_code, "now sealed by the new code");
    assert!(s.recovery_made_at.unwrap() > made, "recoveryMadeAt is set");
    assert_eq!(s.key, "here");
    assert_eq!((s.backup.on, s.backup.every.as_str(), s.backup.on_quit), (before.backup.on, "manual", true), "Cloud Backup's settings stay");
    assert_eq!(keychain(&a, &account, "key").as_deref(), Some(key_here.as_str()), "this computer keeps its key");
    assert_eq!(heard.lock().unwrap().last(), Some(&s), "cloud-changed went out");
    // it is forgotten: typed again, there is nothing to confirm
    assert_eq!(cloud::confirm_recovery(&a, last_group(&new)).unwrap_err(), cloud::MAKE_CODE_FIRST);

    // a third computer: the old code stopped working, the new one works
    let (c, _) = hersir(tmp_c.path(), &f);
    sign_in(&c);
    assert_eq!(cloud::enter_recovery(&c, &old).unwrap_err(), WRONG_CODE, "the old code stopped working");
    assert_eq!(cloud::enter_recovery(&c, &new).unwrap().key, "here");
    // all three have the same key: B and C list A's backup
    back_up_now(&a);
    assert_eq!(cloud::list_backups(&b).unwrap().len(), 1);
    assert_eq!(cloud::list_backups(&c).unwrap().len(), 1);
}

// GA-370: a new code that isn't typed back changes nothing. Another new code takes its place, and signing out drops it.
#[test]
fn a_new_recovery_code_not_typed_back_changes_nothing_and_another_one_or_signing_out_drops_it() {
    let f = FakeCloud::start().unwrap();
    let (tmp_a, tmp_b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (a, _) = hersir(tmp_a.path(), &f);
    let account = f.account().id;
    let old = first_computer(&a);
    let record = f.key(&account).unwrap();
    let before = cloud::status(&a);

    let first = cloud::new_recovery(&a).unwrap().recovery_code;
    let second = cloud::new_recovery(&a).unwrap().recovery_code;
    assert_ne!(first, second);
    if last_group(&first) != last_group(&second) {
        assert_eq!(cloud::confirm_recovery(&a, last_group(&first)).unwrap_err(), cloud::WRONG_GROUP, "the second code took the first one's place");
    }
    assert_eq!(kept(&cloud::status(&a)), kept(&before), "nothing changed on this computer");
    // signed out and in again: the second one is gone too
    cloud::sign_out(&a).unwrap();
    assert_eq!(sign_in(&a).key, "needed");
    assert_eq!(cloud::confirm_recovery(&a, last_group(&second)).unwrap_err(), cloud::MAKE_CODE_FIRST);
    assert_eq!(key_puts(&f), 1, "neither went to Hersir Cloud");
    assert_eq!(f.key(&account), Some(record));
    // the old code still opens the account's backups, here and on another computer; the new ones don't
    let (b, _) = hersir(tmp_b.path(), &f);
    sign_in(&b);
    for code in [&first, &second] {
        assert_eq!(cloud::enter_recovery(&b, code).unwrap_err(), WRONG_CODE);
    }
    assert_eq!(cloud::enter_recovery(&b, &old).unwrap().key, "here");
    assert_eq!(cloud::enter_recovery(&a, &old).unwrap().key, "here");
}

// GA-370: a new code typed back after the account's key changed on another computer (`PUT /keys` answers 409 epoch) is
// dropped with its sentence, and the old code keeps working. When Hersir Cloud can't take it, the code waits on.
#[test]
fn a_new_recovery_code_over_a_changed_key_is_dropped_and_one_hersir_cloud_refused_can_be_typed_again() {
    let f = FakeCloud::start().unwrap();
    let (tmp_a, tmp_b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (a, _) = hersir(tmp_a.path(), &f);
    let account = f.account().id;
    let old = first_computer(&a);
    let record = f.key(&account).unwrap();
    let before = cloud::status(&a);

    let new = cloud::new_recovery(&a).unwrap().recovery_code;
    f.fail_next(Failure::new("PUT", "/keys", 409, "epoch", "Another computer changed this account's key: get it again."));
    assert_eq!(cloud::confirm_recovery(&a, last_group(&new)).unwrap_err(), cloud::KEY_CHANGED);
    assert_eq!(cloud::KEY_CHANGED, "The account's key changed on another computer: sign out and in again, then try again.");
    assert_eq!(cloud::confirm_recovery(&a, last_group(&new)).unwrap_err(), cloud::MAKE_CODE_FIRST, "the new code is dropped");
    assert_eq!(cloud::MAKE_CODE_FIRST, "Make the recovery code first: Hersir shows it once.");
    assert_eq!(f.key(&account).as_ref(), Some(&record));
    assert_eq!(kept(&cloud::status(&a)), kept(&before), "this computer's key and settings stay");

    // Hersir Cloud fails: the code waits on, and its last group typed again stores it
    let again = cloud::new_recovery(&a).unwrap().recovery_code;
    f.fail_next(Failure::new("PUT", "/keys", 500, "", "Server Error"));
    assert!(cloud::confirm_recovery(&a, last_group(&again)).is_err());
    assert_eq!(f.key(&account).as_ref(), Some(&record));
    assert!(cloud::confirm_recovery(&a, last_group(&again)).unwrap().recovery_made_at > before.recovery_made_at);
    let (b, _) = hersir(tmp_b.path(), &f);
    sign_in(&b);
    assert_eq!(cloud::enter_recovery(&b, &old).unwrap_err(), WRONG_CODE);
    assert_eq!(cloud::enter_recovery(&b, &new).unwrap_err(), WRONG_CODE, "the dropped code never worked");
    assert_eq!(cloud::enter_recovery(&b, &again).unwrap().key, "here");
}

// ---- signing in ----

#[test]
fn one_sign_in_at_a_time_and_cancel_ends_the_wait() {
    let f = FakeCloud::start().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let (st, _) = hersir(tmp.path(), &f);
    let st2 = st.clone();
    // a browser that never comes back
    let waiting = std::thread::spawn(move || cloud::sign_in(&st2, |_| Ok(()), Duration::from_secs(60)));
    until("signing in", || cloud::status(&st).signing_in.then_some(()));
    // wait until the sign-in can be cancelled (its address made)
    std::thread::sleep(Duration::from_millis(300));
    assert!(cloud::sign_in(&st, |_| Ok(()), Duration::from_secs(1)).is_err(), "a second sign-in is refused");
    let asked = Instant::now();
    cloud::cancel_sign_in(&st);
    let out = waiting.join().unwrap();
    assert!(out.is_err(), "cancelled");
    assert!(asked.elapsed() < Duration::from_secs(10));
    let s = cloud::status(&st);
    assert!(!s.signed_in && !s.signing_in);
}

// ---- the refusals ----

#[test]
fn signed_out_every_command_says_sign_in_to_hersir_cloud_first() {
    let f = FakeCloud::start().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let (st, _) = hersir(tmp.path(), &f);
    assert_eq!(cloud::make_key(&st).unwrap_err(), SIGN_IN_FIRST);
    assert_eq!(cloud::confirm_recovery(&st, "ABC").unwrap_err(), SIGN_IN_FIRST);
    assert_eq!(cloud::enter_recovery(&st, "HC-0000").unwrap_err(), SIGN_IN_FIRST);
    assert_eq!(cloud::new_recovery(&st).unwrap_err(), SIGN_IN_FIRST);
    assert_eq!(cloud::back_up_now(&st).unwrap_err(), SIGN_IN_FIRST);
    assert_eq!(cloud::list_backups(&st).unwrap_err(), SIGN_IN_FIRST);
    assert_eq!(cloud::start_restore(&st, "x").unwrap_err(), SIGN_IN_FIRST);
    assert_eq!(cloud::computers(&st).unwrap_err(), SIGN_IN_FIRST);
    assert_eq!(cloud::remove_computer(&st, "x", false).unwrap_err(), SIGN_IN_FIRST);
    assert_eq!(cloud::save_settings(&st, SettingsChange { on: Some(true), ..Default::default() }).unwrap_err(), SIGN_IN_FIRST);
    // the other switches can be saved signed out
    assert!(cloud::save_settings(&st, SettingsChange { on_quit: Some(true), ..Default::default() }).unwrap().backup.on_quit);
    assert!(cloud::sign_out(&st).is_ok());
    assert!(cloud::hourly(&st, gizai_core::ids::now_ms()).is_none());
    assert!(f.calls().is_empty(), "nothing asked Hersir Cloud: {:?}", f.calls());
}

// ---- where Hersir Cloud is ----

#[test]
fn release_builds_ignore_the_hersir_cloud_variables_and_use_the_production_values() {
    let p = Config::production();
    assert_eq!(p.server, "https://cloud.hersir.dev");
    assert_eq!(p.client_id, "01a21388-1d0c-7e76-9253-3af81455f4d0");
    assert_eq!(cloud::SERVER, "https://cloud.hersir.dev");
    assert_eq!(cloud::CLIENT_ID, "01a21388-1d0c-7e76-9253-3af81455f4d0");
    // a release build: no variable is read
    let release = Config::from_vars(false, |name| panic!("a release build read {name}"));
    assert_eq!(release, p);
    // a debug build: each variable that is set
    let f = FakeCloud::start().unwrap();
    let key = f.entitlement_key_b64();
    let vars = |name: &str| match name {
        "HERSIR_CLOUD_URL" => Some(format!("{}/", f.url())),
        "HERSIR_CLOUD_CLIENT_ID" => Some("qa-client".to_string()),
        "HERSIR_CLOUD_ENTITLEMENT_KEY" => Some(key.clone()),
        _ => None,
    };
    let debug = Config::from_vars(true, vars);
    assert_eq!(debug.server, f.url());
    assert_eq!(debug.client_id, "qa-client");
    assert_eq!(debug.entitlement_key, Some(f.entitlement_key()));
    // unset or empty: production's
    assert_eq!(Config::from_vars(true, |_| Some(" ".to_string())), p);
    assert_eq!(Config::from_vars(true, |_| None), p);
    // the source: from_env reads them only under debug_assertions
    let src = include_str!("../../src/cloud.rs").replace("\r\n", "\n");
    let from_env = &src[src.find("pub fn from_env()").unwrap()..];
    let from_env = &from_env[..from_env.find("\n    }\n").unwrap()];
    assert!(from_env.contains("#[cfg(debug_assertions)]") && from_env.contains("#[cfg(not(debug_assertions))]"), "{from_env}");
    let release_arm = &from_env[from_env.find("#[cfg(not(debug_assertions))]").unwrap()..];
    assert!(release_arm.contains("Config::production()") && !release_arm.contains("var"), "{release_arm}");
}

// GA-370: Hersir Cloud's entitlement key is built in (from `php artisan hersir:entitlement-key` on cloud.hersir.dev, the
// Chief's comment of 26 Nov), so the production config, and a release build's, has it. It is a valid Ed25519 key that
// refuses an entitlement signed by another key, the fake Hersir Cloud's: such a plan isn't shown.
#[test]
fn hersir_clouds_entitlement_key_is_built_in_and_refuses_a_plan_signed_by_another_key() {
    use gizai_cloud_client::entitlement;
    assert_eq!(cloud::ENTITLEMENT_KEY, "9BNGtlbwshLo9NUH9IIg+N5rkR8KmDu6O3dEHr/kg6g=");
    let key = Config::production().entitlement_key.expect("the production config has Hersir Cloud's entitlement key");
    assert_eq!(key, entitlement::public_key(cloud::ENTITLEMENT_KEY).unwrap());
    assert_eq!(Config::from_vars(false, |name| panic!("a release build read {name}")).entitlement_key, Some(key));

    let f = FakeCloud::start().unwrap();
    assert_ne!(f.entitlement_key(), key);
    let account = f.account().id;
    let payload = serde_json::json!({ "v": 1, "account": account, "plan": "personal", "status": "active", "features": ["backup", "remote"],
        "quota_bytes": 10_000_000_000_i64, "used_bytes": 0, "valid_until": null, "read_only_until": null, "issued_at": gizai_core::ids::now_ms() })
        .to_string();
    let signed = f.sign_entitlement(&payload);
    assert_eq!(entitlement::check(&signed, &f.entitlement_key(), &account).unwrap().status, "active", "the fake's own key takes it");
    let refused = entitlement::check(&signed, &key, &account).unwrap_err().to_string();
    // refused for its signature, so the built-in key is a valid Ed25519 key
    assert_eq!(refused, "The plan from Hersir Cloud isn't signed by Hersir Cloud: its signature doesn't match");

    // in the app: Hersir Cloud's key against the fake's entitlement shows no plan; the fake's own key does
    let tmp = tempfile::tempdir().unwrap();
    let (st, _) = hersir(tmp.path(), &f);
    st.cloud.set_config(Config { entitlement_key: Config::production().entitlement_key, ..config(&f) });
    let s = sign_in(&st);
    assert!(s.signed_in);
    assert_eq!(s.plan, None, "a plan signed by another key isn't shown");
    assert_eq!(cloud::refresh(&st).plan, None);
    st.cloud.set_config(config(&f));
    assert_eq!(cloud::refresh(&st).plan.map(|p| p.status), Some("active".to_string()));
}
