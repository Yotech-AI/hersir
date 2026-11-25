//! Hersir Cloud on this computer (GA-359; research 3.2, 3.4, 3.6 and 5.4): the app's side of `gizai_cloud_client`.
//!
//! - **Signing in** goes through the browser with `gizai_agents::oauth` as it is (`discover("<server>/api")`, then
//!   `begin` with Hersir's own client id, so no registration runs, then `finish`), then `POST /devices` registers this
//!   computer with its device id, name, platform, version and new public keys. Its token is renewed in its last 30 days.
//!   A 401 `removed` or `signed_out` signs this computer out, with the server's message as the last error.
//! - **The keychain** (service `hersir`) holds `cloud/<account>/token`, `cloud/<account>/key` (the account key) and
//!   `cloud/<account>/device` (this computer's keys). The recovery code is never stored: `make_key` keeps it in memory
//!   until `confirm_recovery`, and only the account key sealed by it goes to Hersir Cloud.
//! - **This computer's settings** (`scope::THIS_COMPUTER`, so a restore keeps them): `cloud_account` (the account and its
//!   server), `cloud_backup` (On, Every, On quit), `cloud_last_backup` and `cloud_last_error`.
//! - **Cloud Backup** runs beside the local backups (`backups`), one at a time: daily when something changed since the last
//!   cloud backup (the schedule's hourly check, `hourly`), on Quit Hersir completely with On quit on (`quit_backup`), Back
//!   up now, and before a restore (`before_restore`). The database is copied page for page (`copy_pages`), then
//!   `gizai_cloud_client::backup` chunks, seals and uploads it. A lapsed plan, no plan or a full Hersir Cloud makes none.
//! - **Restore from Hersir Cloud** downloads a backup into `<data folder>/restore/cloud/`, a format-1 folder, and from there
//!   it is GA-218's restore (`restore::start_cloud`).
//! - **The plan** comes from the signed entitlement, checked with Hersir Cloud's key before it is shown: after signing in,
//!   on each `cloud_status` and every hour.
//!
//! Each change goes out in `cloud-changed` with the status. Usable without a Tauri app (tests): the UI hears about changes
//! through `AppState::notify`, and a test points `Cloud::set_config` at the fake Hersir Cloud. Everything here blocks:
//! the commands run it off the main thread.
use std::path::PathBuf;
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use gizai_agents::oauth;
use gizai_cloud_client::Error as CloudError;
use gizai_cloud_client::api::{Api, KeyRecord, NewDevice, Snapshot};
use gizai_cloud_client::backup::{self as cloud_backup, Blob, Progress, Source, Step};
use gizai_cloud_client::entitlement;
use gizai_cloud_client::keys::{self, AccountKey, DeviceKeys, RecoveryCode};
use gizai_core::backups::{self as core_backups, Every, LastError, Reason};
use gizai_core::{files, ids, settings};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::runs::Note;

/// Hersir Cloud itself.
pub const SERVER: &str = gizai_cloud_client::api::HERSIR_CLOUD;
/// The Hersir desktop client on Hersir Cloud (Passport's public client, `passport:client --public`): not a secret.
pub const CLIENT_ID: &str = "01a21388-1d0c-7e76-9253-3af81455f4d0";
/// Hersir Cloud's entitlement key: the base64 of its Ed25519 public key, as `php artisan hersir:entitlement-key` shows it.
/// A placeholder until the real key is set here: without it the plan can't be checked, so it isn't shown (`plan` null),
/// and the server still enforces it.
pub const ENTITLEMENT_KEY: &str = "";

/// How long signing in waits for the browser.
pub const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// This computer's token is renewed in its last 30 days.
pub const RENEW_WITHIN_MS: i64 = 30 * core_backups::DAY_MS;
/// The counts of a backup's or download's step go out in `cloud-changed` at most this often; a new step at once.
pub const COUNT_EVERY: Duration = Duration::from_millis(100);
/// Inside `<data folder>/restore/`: where a backup from Hersir Cloud is downloaded (a format-1 folder).
pub const RESTORE_DIR: &str = "cloud";

/// This computer's settings (`scope::THIS_COMPUTER`): the account it signed in to…
pub const ACCOUNT_KEY: &str = "cloud_account";
/// …Cloud Backup's On, Every and On quit…
pub const BACKUP_KEY: &str = "cloud_backup";
/// …its last cloud backup…
pub const LAST_BACKUP_KEY: &str = "cloud_last_backup";
/// …and the last one that failed (or why it was signed out).
pub const LAST_ERROR_KEY: &str = "cloud_last_error";

/// The refusals, word for word (research 5.4).
pub const SIGN_IN_FIRST: &str = "Sign in to Hersir Cloud first.";
pub const WRONG_CODE: &str = "That recovery code doesn't open this account's backups.";
/// PROTOCOL.md's message for 401 `removed`, when the server gave none.
pub const REMOVED: &str = "This computer was removed from Hersir Cloud on the account page.";
/// …and for 401 `signed_out`.
pub const SIGNED_OUT: &str = "Sign in to Hersir Cloud again.";
/// Hersir Cloud couldn't be reached (the network, the server, R2): the design's words.
pub const UNREACHABLE: &str = "Hersir Cloud couldn't be reached.";
pub const WRONG_GROUP: &str = "That isn't the code's last group. Look at the code above again.";
pub const NO_PLAN: &str = "Cloud Backup needs a Hersir Cloud plan: choose one on the account page.";
pub const CODE_FIRST: &str = "Type your recovery code first.";
pub const KEY_FIRST: &str = "Save your recovery code first.";
pub const HAS_CODE: &str = "This account has a recovery code already: type it to use Cloud Backup on this computer.";
/// A backup or download that was stopped (a restore cancelled): not an error to keep.
const STOPPED: &str = "Cancelled";

// ---- where Hersir Cloud is ----

/// Which Hersir Cloud this Hersir talks to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// `https://cloud.hersir.dev`, without a trailing slash.
    pub server: String,
    /// The desktop client's id for the browser's sign-in.
    pub client_id: String,
    /// Hersir Cloud's entitlement key; None while it isn't set (`ENTITLEMENT_KEY`): then the plan isn't shown.
    pub entitlement_key: Option<[u8; 32]>,
}

impl Config {
    /// The production values, built in: Hersir Cloud, the desktop client and the entitlement key.
    pub fn production() -> Config {
        Config {
            server: SERVER.to_string(),
            client_id: CLIENT_ID.to_string(),
            entitlement_key: entitlement::public_key(ENTITLEMENT_KEY).ok(),
        }
    }

    /// `production`, with `HERSIR_CLOUD_URL`, `HERSIR_CLOUD_CLIENT_ID` and `HERSIR_CLOUD_ENTITLEMENT_KEY` from `var` in
    /// their place when `debug` (each one that is set): to point a debug build at the fake Hersir Cloud or a local one.
    /// Without `debug` the variables are never read.
    pub fn from_vars(debug: bool, var: impl Fn(&str) -> Option<String>) -> Config {
        let mut c = Config::production();
        if !debug {
            return c;
        }
        let set = |name: &str| var(name).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        if let Some(url) = set("HERSIR_CLOUD_URL") {
            c.server = url.trim_end_matches('/').to_string();
        }
        if let Some(id) = set("HERSIR_CLOUD_CLIENT_ID") {
            c.client_id = id;
        }
        if let Some(key) = set("HERSIR_CLOUD_ENTITLEMENT_KEY") {
            match entitlement::public_key(&key) {
                Ok(k) => c.entitlement_key = Some(k),
                Err(e) => eprintln!("gizai: HERSIR_CLOUD_ENTITLEMENT_KEY isn't used: {e}"),
            }
        }
        c
    }

    /// This build's: a debug build reads the variables (`from_vars`); a release build ignores them.
    pub fn from_env() -> Config {
        #[cfg(debug_assertions)]
        {
            Config::from_vars(true, gizai_agents::env::var)
        }
        #[cfg(not(debug_assertions))]
        {
            Config::production()
        }
    }
}

// ---- what the window gets (research 5.4, word for word) ----

/// The plan, from the checked entitlement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudPlan {
    /// "personal" or "none".
    pub name: String,
    /// "active", "past_due", "lapsed" or "none".
    pub status: String,
    pub valid_until: Option<i64>,
    pub read_only_until: Option<i64>,
    pub quota_bytes: i64,
    pub used_bytes: i64,
}

/// A cloud backup's or download's step: "copying", "chunking", "uploading", "registering", "downloading" or "checking".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudRunning {
    pub step: String,
    pub done: u64,
    pub total: u64,
}

/// This computer's last cloud backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudLast {
    pub at: i64,
    pub name: String,
    /// What it uploaded (sealed objects), not the backup's size.
    pub sent_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudBackupStatus {
    pub on: bool,
    /// "daily" or "manual".
    pub every: String,
    pub on_quit: bool,
    pub running: Option<CloudRunning>,
    pub last: Option<CloudLast>,
    /// When the next daily backup is due (it is made only when something changed); None with Only when I ask, Off or
    /// signed out.
    pub next: Option<i64>,
    pub last_error: Option<LastError>,
}

/// What Settings → Hersir Cloud shows (`CloudStatus`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudStatus {
    pub signed_in: bool,
    pub email: Option<String>,
    pub signing_in: bool,
    /// From the signed entitlement; None while signed out, or while it couldn't be checked.
    pub plan: Option<CloudPlan>,
    /// "missing": the account has no key yet; "here": this computer has it; "needed": the account has one, this
    /// computer doesn't.
    pub key: String,
    pub recovery_made_at: Option<i64>,
    pub backup: CloudBackupStatus,
}

/// A backup in Hersir Cloud (`CloudBackup`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudBackup {
    pub name: String,
    pub made_at: i64,
    pub device: CloudDevice,
    pub reason: String,
    pub bytes: i64,
    pub restorable: bool,
    pub problem: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudDevice {
    pub id: String,
    pub name: String,
    pub this: bool,
}

/// A computer of the account (`CloudComputer`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudComputer {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub app_version: String,
    pub last_seen_at: Option<i64>,
    pub online: bool,
    pub this: bool,
    /// Always false: it belonged to Round 2's sync.
    pub chief: bool,
}

/// A recovery code, shown once (`cloud_make_key`, `cloud_new_recovery`).
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryShown {
    pub recovery_code: String,
}

impl std::fmt::Debug for RecoveryShown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryShown(•••)")
    }
}

/// `cloud_save_settings`' changes: each field that is set.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsChange {
    pub on: Option<bool>,
    pub every: Option<String>,
    pub on_quit: Option<bool>,
}

// ---- this computer's settings ----

/// `cloud_account`: the account this computer signed in to, on which server.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AccountSetting {
    pub id: String,
    pub email: String,
    pub server: String,
    /// When this computer made the recovery code (the account's first, or a new one).
    pub recovery_made_at: Option<i64>,
}

/// `cloud_backup`: Cloud Backup on this computer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BackupSetting {
    pub on: bool,
    /// "daily" or "manual".
    pub every: String,
    pub on_quit: bool,
}

impl Default for BackupSetting {
    fn default() -> Self {
        BackupSetting { on: false, every: "daily".into(), on_quit: false }
    }
}

/// `cloud_last_backup`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct LastBackupSetting {
    at: i64,
    name: String,
    sent_bytes: u64,
    /// The newest `changes` row the backup held: the next daily one waits until there is a newer one.
    change_id: Option<String>,
}

fn account_setting(st: &AppState) -> Option<AccountSetting> {
    settings::get::<Option<AccountSetting>>(&st.db, ACCOUNT_KEY).ok().flatten().flatten().filter(|a| !a.id.trim().is_empty())
}

/// The account this computer is signed in to on `cfg`'s server.
fn account(st: &AppState, cfg: &Config) -> Option<AccountSetting> {
    account_setting(st).filter(|a| a.server == cfg.server)
}

fn write_account(st: &AppState, a: Option<&AccountSetting>) -> Result<(), String> {
    settings::set(&st.db, ACCOUNT_KEY, &a).map_err(|e| e.to_string())
}

/// Cloud Backup's settings on this computer: Off, Daily and On quit off where none are saved.
pub fn backup_setting(st: &AppState) -> BackupSetting {
    let mut b: BackupSetting = settings::get(&st.db, BACKUP_KEY).ok().flatten().unwrap_or_default();
    if b.every != "manual" {
        b.every = "daily".into();
    }
    b
}

fn last_backup(st: &AppState) -> Option<LastBackupSetting> {
    settings::get::<Option<LastBackupSetting>>(&st.db, LAST_BACKUP_KEY).ok().flatten().flatten()
}

fn last_error(st: &AppState) -> Option<LastError> {
    settings::get::<Option<LastError>>(&st.db, LAST_ERROR_KEY).ok().flatten().flatten()
}

fn set_last_error(st: &AppState, message: Option<&str>) {
    let e = message.map(|m| LastError { at: ids::now_ms(), message: m.to_string() });
    if let Err(err) = settings::set(&st.db, LAST_ERROR_KEY, &e) {
        eprintln!("gizai: keeping Cloud Backup's last error failed: {err}");
    }
}

// ---- in memory ----

/// Hersir Cloud in memory: the sign-in that waits, the key made but not confirmed, the backup or download that runs,
/// and what the server said last.
#[derive(Default)]
pub struct Cloud {
    /// None: `Config::from_env` at first use.
    config: Mutex<Option<Config>>,
    sign_in: Mutex<Option<SignIn>>,
    pending: Mutex<Option<PendingKey>>,
    running: Mutex<Option<CloudRunning>>,
    /// Woken when a backup or download ends.
    ended: Condvar,
    seen: Mutex<Seen>,
    /// One look at the token (and its renewal) at a time.
    token: Mutex<()>,
    /// When `cloud-changed` last went out for a count.
    said: Mutex<Option<Instant>>,
}

/// A sign-in waiting for the browser.
#[derive(Default)]
struct SignIn {
    cancel: Option<oauth::CancelHandle>,
    cancelled: bool,
}

/// The account key and recovery code `make_key` made, in memory only until `confirm_recovery`.
struct PendingKey {
    account: String,
    key: AccountKey,
    code: RecoveryCode,
    record: KeyRecord,
}

/// What Hersir Cloud said last about the signed-in account.
#[derive(Debug, Default)]
struct Seen {
    account: String,
    plan: Option<CloudPlan>,
    /// Whether the account has a key on the server; None: not asked yet.
    has_key: Option<bool>,
    /// Whether this computer's keychain has it; None: not looked yet.
    key_here: Option<bool>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Cloud {
    /// Which Hersir Cloud this Hersir talks to.
    pub fn config(&self) -> Config {
        lock(&self.config).get_or_insert_with(Config::from_env).clone()
    }

    /// Points this Hersir at another Hersir Cloud (tests: the fake). What was seen of the old one is forgotten.
    pub fn set_config(&self, config: Config) {
        *lock(&self.config) = Some(config);
        *lock(&self.seen) = Seen::default();
    }

    /// The backup or download that runs now.
    pub fn running(&self) -> Option<CloudRunning> {
        lock(&self.running).clone()
    }
}

fn seen<T>(st: &AppState, account: &str, f: impl FnOnce(&mut Seen) -> T) -> T {
    let mut s = lock(&st.cloud.seen);
    if s.account != account {
        *s = Seen { account: account.to_string(), ..Seen::default() };
    }
    f(&mut s)
}

// ---- the keychain ----

/// `cloud/<account>/<what>`: `token`, `key` or `device`.
pub fn keychain_key(account: &str, what: &str) -> String {
    format!("cloud/{account}/{what}")
}

/// This computer's token, as the keychain keeps it.
#[derive(Serialize, Deserialize)]
struct TokenText {
    token: String,
    expires_at: i64,
}

fn keychain_problem(e: String) -> String {
    format!("Hersir couldn't use the keychain for Hersir Cloud: {e}")
}

fn read_token(st: &AppState, account: &str) -> Result<Option<TokenText>, String> {
    let Some(text) = st.keychain.get(&keychain_key(account, "token")).map_err(keychain_problem)? else { return Ok(None) };
    Ok(serde_json::from_str::<TokenText>(&text).ok().filter(|t| !t.token.trim().is_empty()))
}

fn save_token(st: &AppState, account: &str, token: &str, expires_at: i64) -> Result<(), String> {
    let text = serde_json::to_string(&TokenText { token: token.to_string(), expires_at }).map_err(|e| e.to_string())?;
    st.keychain.set(&keychain_key(account, "token"), &text).map_err(keychain_problem)
}

/// The account key in this computer's keychain.
fn load_key(st: &AppState, account: &str) -> Result<Option<AccountKey>, String> {
    let Some(text) = st.keychain.get(&keychain_key(account, "key")).map_err(keychain_problem)? else { return Ok(None) };
    AccountKey::from_text(&text).map(Some).map_err(|e| e.to_string())
}

/// Whether this computer has the account key: as seen last, else from the keychain.
fn key_here(st: &AppState, account: &str) -> bool {
    if let Some(here) = seen(st, account, |s| s.key_here) {
        return here;
    }
    let here = st.keychain.get(&keychain_key(account, "key")).ok().flatten().is_some();
    seen(st, account, |s| s.key_here = Some(here));
    here
}

/// Signs this computer out here: its token goes from the keychain, and with `all` the account key and its own keys too
/// (Sign out, a removed computer); the account, the last backup and what was seen go. Cloud Backup's switches stay.
fn forget(st: &AppState, account: &AccountSetting, all: bool) {
    let mut whats = vec!["token"];
    if all {
        whats.extend(["key", "device"]);
    }
    for what in whats {
        if let Err(e) = st.keychain.delete(&keychain_key(&account.id, what)) {
            eprintln!("gizai: removing Hersir Cloud's {what} from the keychain failed: {e}");
        }
    }
    if account_setting(st).is_some_and(|a| a.id == account.id)
        && let Err(e) = write_account(st, None)
    {
        eprintln!("gizai: forgetting the Hersir Cloud account failed: {e}");
    }
    let _ = settings::set(&st.db, LAST_BACKUP_KEY, &None::<LastBackupSetting>);
    *lock(&st.cloud.seen) = Seen::default();
    *lock(&st.cloud.pending) = None;
}

// ---- a session: the account and its API, the token renewed when due ----

struct Session {
    cfg: Config,
    account: AccountSetting,
    api: Api,
}

/// The signed-in account and Hersir Cloud's API with this computer's token, renewed in its last 30 days (not while a
/// backup or download runs: it uses the token it has). "Sign in to Hersir Cloud first." while signed out; a token the
/// keychain no longer has signs this computer out.
fn session(st: &AppState) -> Result<Session, String> {
    let cfg = st.cloud.config();
    let Some(account) = account(st, &cfg) else { return Err(SIGN_IN_FIRST.into()) };
    let one = lock(&st.cloud.token);
    let token = match read_token(st, &account.id)? {
        Some(t) => t,
        None => {
            drop(one);
            forget(st, &account, false);
            changed(st);
            return Err(SIGN_IN_FIRST.into());
        }
    };
    let api = Api::new(&cfg.server, &token.token).map_err(|e| e.to_string())?;
    if token.expires_at - ids::now_ms() > RENEW_WITHIN_MS || st.cloud.running().is_some() {
        return Ok(Session { cfg, account, api });
    }
    match api.renew_token() {
        Ok(new) => {
            save_token(st, &account.id, &new.device_token, new.expires_at)?;
            let api = Api::new(&cfg.server, &new.device_token).map_err(|e| e.to_string())?;
            Ok(Session { cfg, account, api })
        }
        Err(e @ (CloudError::SignedOut(_) | CloudError::Removed(_))) => {
            drop(one);
            let s = Session { cfg, account, api };
            Err(failed(st, &s, e))
        }
        Err(e) => {
            eprintln!("gizai: renewing this computer's Hersir Cloud token failed, so it goes on with the one it has: {e}");
            Ok(Session { cfg, account, api })
        }
    }
}

/// What a refusal of Hersir Cloud means here, as a sentence; a 401 signs this computer out first (`removed` forgets the
/// account key too), keeping the server's message as the last error.
fn failed(st: &AppState, s: &Session, e: CloudError) -> String {
    match e {
        CloudError::Removed(m) => {
            let message = or(m, REMOVED);
            forget(st, &s.account, true);
            set_last_error(st, Some(&message));
            changed(st);
            message
        }
        CloudError::SignedOut(m) => {
            let message = or(m, SIGNED_OUT);
            forget(st, &s.account, false);
            set_last_error(st, Some(&message));
            changed(st);
            message
        }
        CloudError::Network(why) | CloudError::Storage(why) => {
            eprintln!("gizai: Hersir Cloud: {why}");
            UNREACHABLE.into()
        }
        CloudError::Server { status, message, .. } if status >= 500 => {
            eprintln!("gizai: Hersir Cloud answered {status}: {message}");
            UNREACHABLE.into()
        }
        CloudError::ReadOnly(m) => {
            let _ = refresh_plan(st, s);
            match plan(st, &s.account.id) {
                Some(p) if p.status == "lapsed" => ended_line(&p),
                _ => m,
            }
        }
        CloudError::PlanRequired(_) => NO_PLAN.into(),
        CloudError::Quota(m) => {
            let _ = refresh_plan(st, s);
            match plan(st, &s.account.id) {
                Some(p) => quota_line(&p),
                None => m.split_inclusive(". ").next().unwrap_or(&m).trim().to_string(),
            }
        }
        CloudError::WrongRecoveryCode => WRONG_CODE.into(),
        CloudError::Cancelled => STOPPED.into(),
        e => e.to_string(),
    }
}

fn or(message: String, otherwise: &str) -> String {
    if message.trim().is_empty() { otherwise.to_string() } else { message }
}

// ---- the plan ----

/// The plan as seen last for `account`.
fn plan(st: &AppState, account: &str) -> Option<CloudPlan> {
    seen(st, account, |s| s.plan.clone())
}

/// `GET /me/entitlements`, checked with Hersir Cloud's key before the plan is filled; one that doesn't check leaves no
/// plan. Without the key (not set yet) nothing is asked.
fn refresh_plan(st: &AppState, s: &Session) -> Result<(), String> {
    let Some(key) = s.cfg.entitlement_key else { return Ok(()) };
    let signed = s.api.entitlement().map_err(|e| failed(st, s, e))?;
    let checked = entitlement::check(&signed, &key, &s.account.id);
    let plan = checked.as_ref().ok().map(|e| CloudPlan {
        name: e.plan.clone(),
        status: e.status.clone(),
        valid_until: e.valid_until,
        read_only_until: e.read_only_until,
        quota_bytes: e.quota_bytes,
        used_bytes: e.used_bytes,
    });
    let moved = seen(st, &s.account.id, |seen| {
        let moved = seen.plan != plan;
        seen.plan = plan;
        moved
    });
    if moved {
        changed(st);
    }
    checked.map(drop).map_err(|e| {
        eprintln!("gizai: Hersir Cloud's plan isn't shown: {e}");
        e.to_string()
    })
}

/// Why the plan takes no new backups now, as a sentence: it lapsed, there is none, or Hersir Cloud is full. None when
/// it takes them, or the plan isn't known.
fn cannot_upload(st: &AppState, account: &str) -> Option<String> {
    let p = plan(st, account)?;
    match p.status.as_str() {
        "lapsed" => Some(ended_line(&p)),
        _ if p.status == "none" || p.name == "none" => Some(NO_PLAN.into()),
        _ if p.quota_bytes > 0 && p.used_bytes >= p.quota_bytes => Some(quota_line(&p)),
        _ => None,
    }
}

/// "Your plan ended on 9 Dec 2026: backups can be restored until 7 Feb 2027."
fn ended_line(p: &CloudPlan) -> String {
    match (p.valid_until, p.read_only_until) {
        (Some(ended), Some(until)) => format!("Your plan ended on {}: backups can be restored until {}.", day(ended), day(until)),
        (Some(ended), None) => format!("Your plan ended on {}.", day(ended)),
        _ => "Your plan ended.".to_string(),
    }
}

/// "Hersir Cloud is full: 10.0 of 10 GB.", in the quota's own unit (10 GB is 10 × 1000³ bytes).
fn quota_line(p: &CloudPlan) -> String {
    let gb = if p.quota_bytes > 0 && p.quota_bytes % 1_000_000_000 == 0 { 1e9 } else { 1024f64.powi(3) };
    let quota = p.quota_bytes as f64 / gb;
    let quota = if quota.fract() == 0.0 { format!("{quota:.0}") } else { format!("{quota:.1}") };
    format!("Hersir Cloud is full: {:.1} of {quota} GB.", p.used_bytes as f64 / gb)
}

/// A day as people read it, in local time as the window shows it: "9 Dec 2026".
fn day(ms: i64) -> String {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    // "20261209-…": the date in local time
    let stamp = gizai_core::db::stamp(ms);
    let (y, m, d) = (&stamp[..4], stamp[4..6].parse::<usize>().unwrap_or(1), stamp[6..8].parse::<u32>().unwrap_or(1));
    format!("{d} {} {y}", MONTHS[m.clamp(1, 12) - 1])
}

// ---- the status ----

/// Sends `cloud-changed` with the status.
pub fn changed(st: &AppState) {
    (st.notify)(Note::CloudChanged(status(st)));
}

/// What Settings → Hersir Cloud shows, from what is known now (no call to Hersir Cloud: `refresh` asks it).
pub fn status(st: &AppState) -> CloudStatus {
    let cfg = st.cloud.config();
    let b = backup_setting(st);
    let signing_in = lock(&st.cloud.sign_in).is_some();
    let running = st.cloud.running();
    let mut backup = CloudBackupStatus {
        on: b.on,
        every: b.every.clone(),
        on_quit: b.on_quit,
        running,
        last: None,
        next: None,
        last_error: last_error(st),
    };
    let Some(a) = account(st, &cfg) else {
        return CloudStatus { signed_in: false, email: None, signing_in, plan: None, key: "missing".into(), recovery_made_at: None, backup };
    };
    let here = key_here(st, &a.id);
    let (plan, has_key) = seen(st, &a.id, |s| (s.plan.clone(), s.has_key));
    let key = if here { "here" } else if has_key == Some(true) { "needed" } else { "missing" };
    let last = last_backup(st);
    backup.last = last.as_ref().map(|l| CloudLast { at: l.at, name: l.name.clone(), sent_bytes: l.sent_bytes });
    if b.on && b.every == "daily" && here {
        backup.next = core_backups::next_due(Every::Daily, last.map(|l| l.at), ids::now_ms());
    }
    CloudStatus {
        signed_in: true,
        email: Some(a.email.clone()),
        signing_in,
        plan,
        key: key.into(),
        recovery_made_at: a.recovery_made_at,
        backup,
    }
}

/// The status after asking Hersir Cloud (`cloud_status`): the plan (the entitlement), and whether the account has a key
/// this computer lacks. When it can't be reached, what was known stays.
pub fn refresh(st: &AppState) -> CloudStatus {
    if let Ok(s) = session(st) {
        let _ = refresh_plan(st, &s);
        let _ = refresh_key(st, &s);
    }
    status(st)
}

/// Whether this computer has the account key (the keychain), and else whether the account has one (`GET /keys`).
fn refresh_key(st: &AppState, s: &Session) -> Result<(), String> {
    let here = st.keychain.get(&keychain_key(&s.account.id, "key")).map_err(keychain_problem)?.is_some();
    let has = if here { Some(true) } else { Some(s.api.key().map_err(|e| failed(st, s, e))?.is_some()) };
    let moved = seen(st, &s.account.id, |seen| {
        let moved = seen.key_here != Some(here) || seen.has_key != has;
        seen.key_here = Some(here);
        seen.has_key = has;
        moved
    });
    if moved {
        changed(st);
    }
    Ok(())
}

// ---- signing in and out ----

/// Signs in through the browser (`open` opens the sign-in page; it waits up to `timeout`), registers this computer
/// (`POST /devices`) and keeps its token and keys in the keychain. Then the plan and the key are asked for. One sign-in at a
/// time; `cancel_sign_in` ends the wait.
pub fn sign_in(st: &AppState, open: impl Fn(&str) -> Result<(), String>, timeout: Duration) -> Result<CloudStatus, String> {
    let cfg = st.cloud.config();
    {
        let mut s = lock(&st.cloud.sign_in);
        if s.is_some() {
            return Err("Hersir is waiting for you to sign in to Hersir Cloud in your browser already.".into());
        }
        *s = Some(SignIn::default());
    }
    changed(st);
    let out = sign_in_now(st, &cfg, &open, timeout);
    *lock(&st.cloud.sign_in) = None;
    match out {
        Ok(()) => {
            let s = refresh(st);
            (st.notify)(Note::CloudChanged(s.clone()));
            Ok(s)
        }
        Err(e) => {
            changed(st);
            Err(e)
        }
    }
}

fn sign_in_now(st: &AppState, cfg: &Config, open: &dyn Fn(&str) -> Result<(), String>, timeout: Duration) -> Result<(), String> {
    let cancelled = || "The sign-in was cancelled".to_string();
    let d = oauth::discover(&format!("{}/api", cfg.server), None).map_err(|e| format!("Couldn't sign in to Hersir Cloud: {e}"))?;
    let pending = oauth::begin(&d, Some(&cfg.client_id))?;
    match lock(&st.cloud.sign_in).as_mut() {
        Some(s) if !s.cancelled => s.cancel = Some(pending.cancel_handle()),
        _ => return Err(cancelled()),
    }
    open(&pending.authorize_url)?;
    let saved = oauth::finish(pending, timeout)?;
    register(st, cfg, &saved.access_token)
}

/// Ends the wait of a sign-in (it returns "The sign-in was cancelled").
pub fn cancel_sign_in(st: &AppState) {
    if let Some(s) = lock(&st.cloud.sign_in).as_mut() {
        s.cancelled = true;
        if let Some(c) = &s.cancel {
            c.cancel();
        }
    }
}

/// `POST /devices` with the browser's token: this computer's id, name, platform, version and new keys. Its own token and
/// keys go into the keychain, the account into this computer's settings.
fn register(st: &AppState, cfg: &Config, browser_token: &str) -> Result<(), String> {
    let keys = DeviceKeys::generate().map_err(|e| e.to_string())?;
    let device = NewDevice {
        device_id: st.db.device_id().to_string(),
        name: computer_name(st),
        platform: std::env::consts::OS.to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        sign_pub: keys.sign_pub_b64(),
        box_pub: keys.box_pub_b64(),
        kind: "desktop".into(),
    };
    let r = Api::register(&cfg.server, browser_token, &device).map_err(|e| match e {
        CloudError::Network(why) | CloudError::Storage(why) => {
            eprintln!("gizai: Hersir Cloud: {why}");
            UNREACHABLE.to_string()
        }
        e => e.to_string(),
    })?;
    save_token(st, &r.account.id, &r.device_token, r.expires_at)?;
    st.keychain.set(&keychain_key(&r.account.id, "device"), &keys.to_text()).map_err(keychain_problem)?;
    let before = account_setting(st);
    let recovery_made_at = before.filter(|b| b.id == r.account.id && b.server == cfg.server).and_then(|b| b.recovery_made_at);
    write_account(st, Some(&AccountSetting { id: r.account.id.clone(), email: r.account.email.clone(), server: cfg.server.clone(), recovery_made_at }))?;
    set_last_error(st, None);
    *lock(&st.cloud.seen) = Seen { account: r.account.id, ..Seen::default() };
    Ok(())
}

/// This computer's name as Hersir knows it (its `devices` row), 1 to 255 characters.
fn computer_name(st: &AppState) -> String {
    let id = st.db.device_id().to_string();
    let name = st.db.read(|c| Ok(c.query_row("SELECT name FROM devices WHERE id = ?1", [&id], |r| r.get::<_, String>(0))?)).unwrap_or_default();
    let name: String = name.trim().chars().take(255).collect();
    if name.is_empty() { format!("Hersir on {}", platform_name(std::env::consts::OS)) } else { name }
}

/// A platform as people read it: "Linux", "macOS", "Windows".
fn platform_name(p: &str) -> String {
    match p.trim() {
        "linux" => "Linux".into(),
        "macos" => "macOS".into(),
        "windows" => "Windows".into(),
        "web" => "Web".into(),
        other => other.to_string(),
    }
}

/// Signs this computer out: `DELETE /devices/me` (when Hersir Cloud can't be reached, only this computer forgets), then
/// its token, the account key and its own keys go from the keychain, and the account from its settings.
pub fn sign_out(st: &AppState) -> Result<CloudStatus, String> {
    let cfg = st.cloud.config();
    if let Some(a) = account_setting(st) {
        if a.server == cfg.server
            && let Ok(Some(t)) = read_token(st, &a.id)
            && let Ok(api) = Api::new(&cfg.server, &t.token)
            && let Err(e) = api.remove_device("me", false)
        {
            eprintln!("gizai: signing out on Hersir Cloud failed, so only this computer forgets the sign-in: {e}");
        }
        forget(st, &a, true);
    }
    set_last_error(st, None);
    let out = status(st);
    (st.notify)(Note::CloudChanged(out.clone()));
    Ok(out)
}

// ---- the keys ----

/// The account's first computer: a new account key and its recovery code, kept in memory only until `confirm_recovery`.
/// Refused when the account has a key already (this computer then types its code: `enter_recovery`).
pub fn make_key(st: &AppState) -> Result<RecoveryShown, String> {
    let s = session(st)?;
    if key_here(st, &s.account.id) {
        return Err("This computer has the account's key already.".into());
    }
    match s.api.key() {
        Ok(Some(_)) => {
            seen(st, &s.account.id, |seen| seen.has_key = Some(true));
            changed(st);
            return Err(HAS_CODE.into());
        }
        Ok(None) => {}
        Err(e) => return Err(failed(st, &s, e)),
    }
    let (key, code, record) = keys::new_account_key().map_err(|e| e.to_string())?;
    let shown = RecoveryShown { recovery_code: code.shown().to_string() };
    *lock(&st.cloud.pending) = Some(PendingKey { account: s.account.id.clone(), key, code, record });
    Ok(shown)
}

/// The recovery code's last group typed back (any case, spaces and dashes ignored): Hersir Cloud keeps the account key
/// sealed by the code (`PUT /keys`), this computer keeps the key, and Cloud Backup is turned on. The code is forgotten.
pub fn confirm_recovery(st: &AppState, last_group: &str) -> Result<CloudStatus, String> {
    let s = session(st)?;
    let mut pending = lock(&st.cloud.pending);
    let Some(p) = pending.as_ref().filter(|p| p.account == s.account.id) else {
        return Err("Make the recovery code first: Hersir shows it once.".into());
    };
    if !p.code.last_group_matches(last_group) {
        return Err(WRONG_GROUP.into());
    }
    match s.api.put_key(&p.record, None) {
        Ok(_) => {}
        Err(CloudError::Epoch(_)) => {
            *pending = None;
            drop(pending);
            seen(st, &s.account.id, |seen| seen.has_key = Some(true));
            changed(st);
            return Err("Another computer made this account's recovery code meanwhile: type that code instead.".into());
        }
        Err(e) => {
            drop(pending);
            return Err(failed(st, &s, e));
        }
    }
    let Some(p) = pending.take() else { return Err("Make the recovery code first: Hersir shows it once.".into()) };
    drop(pending);
    seen(st, &s.account.id, |seen| seen.has_key = Some(true));
    st.keychain.set(&keychain_key(&s.account.id, "key"), &p.key.to_text())
        .map_err(|e| format!("{} Type your recovery code to use Cloud Backup on this computer.", keychain_problem(e)))?;
    seen(st, &s.account.id, |seen| seen.key_here = Some(true));
    let mut a = s.account.clone();
    a.recovery_made_at = Some(ids::now_ms());
    write_account(st, Some(&a))?;
    let mut b = backup_setting(st);
    b.on = true;
    settings::set(&st.db, BACKUP_KEY, &b).map_err(|e| e.to_string())?;
    let out = status(st);
    (st.notify)(Note::CloudChanged(out.clone()));
    Ok(out)
}

/// Another computer of the account: the recovery code typed opens the account key from Hersir Cloud (`GET /keys`), and
/// this computer keeps it. "That recovery code doesn't open this account's backups." when it doesn't.
pub fn enter_recovery(st: &AppState, code: &str) -> Result<CloudStatus, String> {
    let s = session(st)?;
    let code = RecoveryCode::parse(code).map_err(|e| e.to_string())?;
    let record = match s.api.key() {
        Ok(Some(r)) => r,
        Ok(None) => {
            seen(st, &s.account.id, |seen| seen.has_key = Some(false));
            changed(st);
            return Err("This account has no recovery code yet: make one in Settings → Hersir Cloud.".into());
        }
        Err(e) => return Err(failed(st, &s, e)),
    };
    let key = keys::open_record(&record, &code).map_err(|e| match e {
        CloudError::WrongRecoveryCode => WRONG_CODE.to_string(),
        e => e.to_string(),
    })?;
    st.keychain.set(&keychain_key(&s.account.id, "key"), &key.to_text()).map_err(keychain_problem)?;
    seen(st, &s.account.id, |seen| {
        seen.key_here = Some(true);
        seen.has_key = Some(true);
    });
    set_last_error(st, None);
    let out = status(st);
    (st.notify)(Note::CloudChanged(out.clone()));
    Ok(out)
}

/// A new recovery code for the same account key, stored with `PUT /keys` from the key's epoch: the old code stops working.
pub fn new_recovery(st: &AppState) -> Result<RecoveryShown, String> {
    let s = session(st)?;
    let key = load_key(st, &s.account.id)?.ok_or_else(|| CODE_FIRST.to_string())?;
    let (code, record) = keys::new_recovery_code(&key).map_err(|e| e.to_string())?;
    match s.api.put_key(&record, Some(key.epoch())) {
        Ok(_) => {}
        Err(CloudError::Epoch(_)) => return Err("The account's key changed on another computer: sign out and in again, then try again.".into()),
        Err(e) => return Err(failed(st, &s, e)),
    }
    let mut a = s.account.clone();
    a.recovery_made_at = Some(ids::now_ms());
    write_account(st, Some(&a))?;
    changed(st);
    Ok(RecoveryShown { recovery_code: code.shown().to_string() })
}

// ---- Cloud Backup's settings ----

/// Saves Cloud Backup's switches (On, Every: "daily" or "manual", On quit). Turning it on needs the account and its key
/// on this computer.
pub fn save_settings(st: &AppState, change: SettingsChange) -> Result<CloudStatus, String> {
    let mut b = backup_setting(st);
    if let Some(every) = change.every {
        if every != "daily" && every != "manual" {
            return Err(format!("Cloud Backup backs up daily or manual, not \"{every}\""));
        }
        b.every = every;
    }
    if let Some(on_quit) = change.on_quit {
        b.on_quit = on_quit;
    }
    if let Some(on) = change.on {
        if on && !b.on {
            let Some(a) = account(st, &st.cloud.config()) else { return Err(SIGN_IN_FIRST.into()) };
            if !key_here(st, &a.id) {
                let needed = seen(st, &a.id, |s| s.has_key == Some(true));
                return Err(if needed { CODE_FIRST } else { KEY_FIRST }.into());
            }
        }
        b.on = on;
    }
    settings::set(&st.db, BACKUP_KEY, &b).map_err(|e| e.to_string())?;
    let out = status(st);
    (st.notify)(Note::CloudChanged(out.clone()));
    Ok(out)
}

// ---- backing up ----

/// The one slot for a cloud backup or download, given back when dropped (also after a panic), which `cloud-changed` says.
struct Slot {
    st: AppState,
}

impl Drop for Slot {
    fn drop(&mut self) {
        *lock(&self.st.cloud.running) = None;
        self.st.cloud.ended.notify_all();
        changed(&self.st);
    }
}

/// Takes the slot at `step`, which `cloud-changed` says. While a backup or download runs: refused at once (`until`
/// None), or waits for it to end until `until`.
fn claim(st: &AppState, step: &str, until: Option<Instant>) -> Result<Slot, String> {
    let busy = || "A cloud backup is running already".to_string();
    let mut running = lock(&st.cloud.running);
    while running.is_some() {
        let left = until.ok_or_else(busy)?.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(busy());
        }
        running = st.cloud.ended.wait_timeout(running, left).unwrap_or_else(|p| p.into_inner()).0;
    }
    *running = Some(CloudRunning { step: step.to_string(), done: 0, total: 0 });
    drop(running);
    *lock(&st.cloud.said) = Some(Instant::now());
    changed(st);
    Ok(Slot { st: st.clone() })
}

/// The step the backup or download is at: a new step goes out at once, its counts at most every COUNT_EVERY.
fn step(st: &AppState, step: &str, done: u64, total: u64) {
    let moved = match lock(&st.cloud.running).as_mut() {
        Some(r) => {
            let moved = r.step != step;
            *r = CloudRunning { step: step.to_string(), done, total };
            moved
        }
        None => return,
    };
    let mut said = lock(&st.cloud.said);
    if moved || done >= total || said.is_none_or(|t| t.elapsed() >= COUNT_EVERY) {
        *said = Some(Instant::now());
        drop(said);
        changed(st);
    }
}

/// Whether this computer backs up to Hersir Cloud now: signed in, with the key, and a plan that takes backups (as far
/// as it is known). The account when it does.
fn ready_to_back_up(st: &AppState) -> Option<AccountSetting> {
    let a = account(st, &st.cloud.config())?;
    (key_here(st, &a.id) && cannot_upload(st, &a.id).is_none()).then_some(a)
}

/// Back up now: starts a `manual` cloud backup on a thread of its own and returns the status at once, with it running.
/// Refused while signed out, without the key, with a lapsed plan, no plan or a full Hersir Cloud (as Hersir Cloud says
/// it now), and while a cloud backup or a restore runs.
pub fn back_up_now(st: &AppState) -> Result<CloudStatus, String> {
    let s = session(st)?;
    if !key_here(st, &s.account.id) {
        return Err(if seen(st, &s.account.id, |x| x.has_key == Some(true)) { CODE_FIRST } else { KEY_FIRST }.into());
    }
    if crate::restore::running(st) {
        return Err("A restore is running: back up once it is done.".into());
    }
    let _ = refresh_plan(st, &s);
    if let Some(why) = cannot_upload(st, &s.account.id) {
        return Err(why);
    }
    drop(s);
    let slot = claim(st, "copying", None)?;
    let out = status(st);
    std::thread::Builder::new()
        .name("gizai-cloud-backup".into())
        .spawn(move || {
            let _ = run(slot, Reason::Manual, &|| false);
        })
        .map_err(|e| format!("Couldn't start the backup: {e}"))?;
    Ok(out)
}

/// The schedule's hourly check (`lib.rs`, on the ticks of `backups`): the plan is asked for again, and with Cloud Backup
/// on and Daily, a `daily` backup is made on this thread when the last one is a day old and something changed since.
/// Passes quietly while signed out, without the key, with a plan that takes no backups, or while one runs.
pub fn hourly(st: &AppState, now_ms: i64) -> Option<Result<CloudLast, String>> {
    let b = backup_setting(st);
    let a = account(st, &st.cloud.config())?;
    if st.cloud.running().is_none()
        && let Ok(s) = session(st)
    {
        let _ = refresh_plan(st, &s);
    }
    if !b.on || b.every != "daily" || st.cloud.running().is_some() || crate::restore::running(st) {
        return None;
    }
    if ready_to_back_up(st).is_none_or(|r| r.id != a.id) {
        return None;
    }
    let last = last_backup(st);
    if !core_backups::is_due(Every::Daily, last.as_ref().map(|l| l.at), now_ms) {
        return None;
    }
    // nothing changed since the last cloud backup: nothing to do
    if let Some(l) = &last
        && l.change_id.is_some()
        && l.change_id == core_backups::newest_change_id(&st.db)
    {
        return None;
    }
    let slot = claim(st, "copying", None).ok()?;
    Some(run(slot, Reason::Daily, &|| false))
}

/// Quit Hersir completely, with Cloud Backup and On quit on: a `quit` backup on this thread, waiting for one that runs
/// until `until` (the quit waits at most 5 minutes for both backups: `backups::quit_backup`).
pub fn quit_backup(st: &AppState, until: Instant) {
    let b = backup_setting(st);
    if !b.on || !b.on_quit || ready_to_back_up(st).is_none() {
        return;
    }
    crate::runs::mark_closing(st);
    if let Ok(slot) = claim(st, "copying", Some(until)) {
        let _ = run(slot, Reason::Quit, &|| false);
    }
}

/// Before a restore (`restore::save_today`), with Cloud Backup on: today's data to Hersir Cloud as `before-restore`, beside
/// the local snapshot, waiting for a backup that runs until `until`. `stop` ends it (a cancelled restore). None when it
/// makes none.
pub fn before_restore(st: &AppState, until: Instant, stop: &dyn Fn() -> bool) -> Option<Result<CloudLast, String>> {
    if !backup_setting(st).on || ready_to_back_up(st).is_none() {
        return None;
    }
    let slot = claim(st, "copying", Some(until)).ok()?;
    Some(run(slot, Reason::BeforeRestore, stop))
}

/// Makes the backup `slot` was taken for, on this thread: each step goes out in `cloud-changed`, a failure is kept as the
/// last error (one that worked clears it), and the slot is given back at the end.
fn run(slot: Slot, reason: Reason, stop: &dyn Fn() -> bool) -> Result<CloudLast, String> {
    let st = &slot.st;
    let out = back_up(st, reason, stop);
    match &out {
        Ok(last) => {
            let _ = settings::set(&st.db, LAST_BACKUP_KEY, &Some(last));
            set_last_error(st, None);
        }
        Err(message) if message == STOPPED => {}
        Err(message) => {
            eprintln!("gizai: the {} backup to Hersir Cloud failed: {message}", reason.as_str());
            if account(st, &st.cloud.config()).is_some() {
                set_last_error(st, Some(message));
            }
        }
    }
    drop(slot);
    out.map(|l| CloudLast { at: l.at, name: l.name, sent_bytes: l.sent_bytes })
}

/// A page-for-page copy of the database, chunked, sealed and uploaded with the blobs it names, then registered.
fn back_up(st: &AppState, reason: Reason, stop: &dyn Fn() -> bool) -> Result<LastBackupSetting, String> {
    let s = session(st)?;
    let key = load_key(st, &s.account.id)?.ok_or_else(|| KEY_FIRST.to_string())?;
    let device_id = st.db.device_id().to_string();
    let made_at = ids::now_ms();
    let name = core_backups::snapshot_name(reason, made_at, &core_backups::device_tag(&device_id));
    let dir = st.data_dir.join("backups");
    remove_old_copies(&dir);
    let dest = dir.join(format!(".cloud-{name}.db"));
    let copy = core_backups::copy_pages(&st.data_dir, &device_id, &dest, &mut |done, total| {
        if stop() {
            return Err(gizai_core::Error::Invalid(STOPPED.into()));
        }
        step(st, "copying", done, total);
        Ok(())
    });
    let copy = match copy {
        Ok(c) => c,
        Err(_) if stop() => return Err(STOPPED.into()),
        Err(e) => return Err(format!("The database couldn't be copied for the backup: {e}")),
    };
    let _gone = Gone(copy.path.clone());
    let src = Source {
        name,
        reason: reason.as_str().into(),
        made_at,
        app_version: env!("CARGO_PKG_VERSION").into(),
        schema_version: copy.schema_version,
        device: copy.device.clone(),
        last_change: copy.last_change.clone(),
        db: copy.path.clone(),
        files: copy.files.iter().map(|(sha, size)| Blob { sha256: sha.clone(), bytes: *size, path: files::blob_path(&st.data_dir, sha) }).collect(),
    };
    let made = cloud_backup::back_up(&s.api, &key, &src, &mut |p: Progress| {
        if stop() {
            return Err(CloudError::Cancelled);
        }
        step(st, p.step.as_str(), p.done, p.total);
        Ok(())
    })
    .map_err(|e| failed(st, &s, e))?;
    // the account's use changed
    let _ = refresh_plan(st, &s);
    Ok(LastBackupSetting {
        at: made.snapshot.made_at,
        name: made.snapshot.name,
        sent_bytes: made.uploaded_bytes,
        change_id: copy.last_change.map(|c| c.id),
    })
}

/// Removes a page-for-page copy when dropped.
struct Gone(PathBuf);

impl Drop for Gone {
    fn drop(&mut self) {
        core_backups::remove_copy(&self.0);
    }
}

/// Copies an earlier cloud backup left in the data folder's `backups/` (a Hersir that quit or crashed meanwhile): one
/// backup runs at a time, so none is in use.
fn remove_old_copies(dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        if e.file_name().to_string_lossy().starts_with(".cloud-gizai-") {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

// ---- the backups in Hersir Cloud, and restoring one ----

/// The account's backups in Hersir Cloud, newest first (`GET /snapshots`), without the ones deleted on the account page.
pub fn list_backups(st: &AppState) -> Result<Vec<CloudBackup>, String> {
    let s = session(st)?;
    let all = s.api.snapshots().map_err(|e| failed(st, &s, e))?;
    let me = st.db.device_id().to_string();
    Ok(all.into_iter().filter(|x| x.deleted_at.is_none()).map(|x| to_backup(x, &me)).collect())
}

fn to_backup(x: Snapshot, me: &str) -> CloudBackup {
    let problem = (x.schema_version > gizai_core::db::SCHEMA_VERSION)
        .then(|| format!("Made by a newer Hersir (database version {}). Update Hersir first.", x.schema_version));
    CloudBackup {
        device: CloudDevice { this: x.device_id.eq_ignore_ascii_case(me), id: x.device_id, name: x.device_name },
        name: x.name,
        made_at: x.made_at,
        reason: x.reason,
        bytes: x.bytes,
        restorable: problem.is_none(),
        problem,
    }
}

/// Restore from Hersir Cloud: backup `name` is downloaded into `<data folder>/restore/cloud/` on the restore's own thread,
/// then GA-218's restore takes it from there (`restore::start_cloud`): the checks, today's data saved first, the swap at
/// the next start. Returns the restore's status at once. Refused while signed out, without the key, for a backup this
/// Hersir can't restore, and while a cloud backup, a backup or another restore runs.
pub fn start_restore(st: &AppState, name: &str) -> Result<gizai_core::restore::RestoreStatus, String> {
    let s = session(st)?;
    if !key_here(st, &s.account.id) {
        return Err(CODE_FIRST.into());
    }
    let snapshot = s.api.snapshots().map_err(|e| failed(st, &s, e))?
        .into_iter().find(|x| x.name == name && x.deleted_at.is_none())
        .ok_or_else(|| format!("There is no backup {name} in Hersir Cloud."))?;
    if let Some(problem) = to_backup(snapshot.clone(), st.db.device_id()).problem {
        return Err(problem);
    }
    if st.cloud.running().is_some() {
        return Err("A cloud backup is running: restore once it is done.".into());
    }
    crate::restore::start_cloud(st, name, Box::new(move |st: &AppState| download(st, snapshot)))
}

/// The download of a restore from Hersir Cloud, on the restore's thread: a format-1 folder, `<data>/restore/cloud/`.
/// Its steps go out in `cloud-changed` (`downloading` counts the backup's bytes, `checking` the bytes put back together);
/// a cancelled restore stops it.
fn download(st: &AppState, snapshot: Snapshot) -> Result<PathBuf, String> {
    let slot = claim(st, "downloading", Some(Instant::now() + crate::restore::SAVE_WAIT))?;
    // after the slot: the token isn't renewed while a download runs
    let s = session(st)?;
    let key = load_key(st, &s.account.id)?.ok_or_else(|| CODE_FIRST.to_string())?;
    let dir = gizai_core::restore::dir(&st.data_dir).join(RESTORE_DIR);
    let bytes = snapshot.bytes.max(0) as u64;
    let out = gizai_cloud_client::restore::restore(&s.api, &key, &snapshot.name, &snapshot.manifest_id, &dir, &mut |p: Progress| {
        if crate::restore::cancelled(st) {
            return Err(CloudError::Cancelled);
        }
        // the download counts objects: as the backup's bytes, as the window shows them
        let (done, total) = match p.step {
            Step::Downloading if p.total > 0 => (bytes.saturating_mul(p.done) / p.total, bytes),
            Step::Downloading => (0, bytes),
            _ => (p.done, p.total),
        };
        step(st, p.step.as_str(), done, total);
        Ok(())
    });
    drop(slot);
    match out {
        Ok(r) => Ok(r.dir),
        Err(e) => Err(failed(st, &s, e)),
    }
}

// ---- the account's computers ----

/// The account's computers and devices that are signed in (`GET /devices`), oldest first.
pub fn computers(st: &AppState) -> Result<Vec<CloudComputer>, String> {
    let s = session(st)?;
    let all = s.api.devices().map_err(|e| failed(st, &s, e))?;
    Ok(all
        .into_iter()
        .filter(|d| d.revoked_at.is_none())
        .map(|d| CloudComputer {
            id: d.id,
            name: d.name,
            platform: platform_name(&d.platform),
            app_version: d.app_version,
            last_seen_at: d.last_seen_at,
            online: d.online,
            this: d.this,
            chief: false,
        })
        .collect())
}

/// Removes a computer of the account (`DELETE /devices/{id}`; `lost`: it was lost or stolen) and lists them again. This
/// computer's own id signs it out.
pub fn remove_computer(st: &AppState, id: &str, lost: bool) -> Result<Vec<CloudComputer>, String> {
    let s = session(st)?;
    let id = id.trim();
    if id.is_empty() {
        return Err("Which computer? Its id is missing.".into());
    }
    s.api.remove_device(id, lost).map_err(|e| failed(st, &s, e))?;
    if id == "me" || id.eq_ignore_ascii_case(st.db.device_id()) {
        forget(st, &s.account, true);
        changed(st);
        return Ok(vec![]);
    }
    computers(st)
}

/// The account page: `<server>/account`.
pub fn account_url(st: &AppState) -> String {
    format!("{}/account", st.cloud.config().server)
}

// ---- the commands (research 5.4: `src/api.ts` names them) ----

type R<T> = Result<T, String>;

/// Runs `f` on a thread that may wait (the network, the keychain, the database), never on the main thread.
async fn off<T: Send + 'static>(st: &AppState, f: impl FnOnce(&AppState) -> R<T> + Send + 'static) -> R<T> {
    let st = st.clone();
    tauri::async_runtime::spawn_blocking(move || f(&st)).await.map_err(|e| e.to_string())?
}

/// The status, after asking Hersir Cloud for the plan and the key.
#[tauri::command]
pub async fn cloud_status(st: tauri::State<'_, AppState>) -> R<CloudStatus> {
    off(&st, |st| Ok(refresh(st))).await
}

/// Opens the browser to sign in and waits for it, up to 10 minutes.
#[tauri::command]
pub async fn cloud_sign_in(app: tauri::AppHandle, st: tauri::State<'_, AppState>) -> R<CloudStatus> {
    use tauri_plugin_opener::OpenerExt;
    off(&st, move |st| {
        sign_in(st, |url| app.opener().open_url(url, None::<&str>).map_err(|e| format!("Couldn't open your browser: {e}")), SIGN_IN_TIMEOUT)
    })
    .await
}

/// Memory only.
#[tauri::command]
pub async fn cloud_cancel_sign_in(st: tauri::State<'_, AppState>) -> R<()> {
    cancel_sign_in(&st);
    Ok(())
}

#[tauri::command]
pub async fn cloud_sign_out(st: tauri::State<'_, AppState>) -> R<CloudStatus> {
    off(&st, sign_out).await
}

#[tauri::command]
pub async fn cloud_make_key(st: tauri::State<'_, AppState>) -> R<RecoveryShown> {
    off(&st, make_key).await
}

#[tauri::command]
pub async fn cloud_confirm_recovery(st: tauri::State<'_, AppState>, last_group: String) -> R<CloudStatus> {
    off(&st, move |st| confirm_recovery(st, &last_group)).await
}

#[tauri::command]
pub async fn cloud_enter_recovery(st: tauri::State<'_, AppState>, code: String) -> R<CloudStatus> {
    off(&st, move |st| enter_recovery(st, &code)).await
}

#[tauri::command]
pub async fn cloud_new_recovery(st: tauri::State<'_, AppState>) -> R<RecoveryShown> {
    off(&st, new_recovery).await
}

#[tauri::command]
pub async fn cloud_save_settings(st: tauri::State<'_, AppState>, s: SettingsChange) -> R<CloudStatus> {
    off(&st, move |st| save_settings(st, s)).await
}

#[tauri::command]
pub async fn cloud_back_up_now(st: tauri::State<'_, AppState>) -> R<CloudStatus> {
    off(&st, back_up_now).await
}

#[tauri::command]
pub async fn cloud_list_backups(st: tauri::State<'_, AppState>) -> R<Vec<CloudBackup>> {
    off(&st, list_backups).await
}

#[tauri::command]
pub async fn cloud_start_restore(st: tauri::State<'_, AppState>, name: String) -> R<gizai_core::restore::RestoreStatus> {
    off(&st, move |st| start_restore(st, &name)).await
}

#[tauri::command]
pub async fn cloud_computers(st: tauri::State<'_, AppState>) -> R<Vec<CloudComputer>> {
    off(&st, computers).await
}

#[tauri::command]
pub async fn cloud_remove_computer(st: tauri::State<'_, AppState>, id: String, lost: bool) -> R<Vec<CloudComputer>> {
    off(&st, move |st| remove_computer(st, &id, lost)).await
}

/// Opens the account page in the browser.
#[tauri::command]
pub async fn cloud_open_account(app: tauri::AppHandle, st: tauri::State<'_, AppState>) -> R<()> {
    use tauri_plugin_opener::OpenerExt;
    let url = account_url(&st);
    app.opener().open_url(url, None::<&str>).map_err(|e| format!("Couldn't open your browser: {e}"))
}
