//! Hersir Cloud's API as a computer calls it (PROTOCOL.md): JSON over HTTPS at `<server>/api/v1`, with this computer's
//! token (scope `cloud`) on every call but `POST /devices`, which takes the browser's sign-in token once. Each error of
//! the server comes back as its own `Error` variant with the server's `message`, and 429 with its `Retry-After`.
//!
//! The objects themselves go straight to R2 with the presigned URLs the server gives (`put_object`, `get_object`),
//! without the token. Only https is used, except for addresses on this computer (127.0.0.1, localhost, [::1]), such as
//! the fake Hersir Cloud's. Lists of ids go in batches of 1,000, the most the server takes in one call.
use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use zeroize::Zeroizing;

use crate::chunks::MAX_OBJECT;
use crate::util::{clean, host_of, is_loopback, pct, safe_url, shown};
use crate::{Error, Result};

/// Hersir Cloud itself.
pub const HERSIR_CLOUD: &str = "https://cloud.hersir.dev";
/// The API's path on the server.
pub const API_PATH: &str = "/api/v1";
/// The most ids one call takes (`POST /objects/missing`, `/objects/upload`, `/objects/download`).
pub const BATCH: usize = 1000;
/// How long one call to the API may take…
const API_LIMIT: Duration = Duration::from_secs(30);
/// …and one object's upload or download to R2 (at most 4 MiB).
const OBJECT_LIMIT: Duration = Duration::from_secs(5 * 60);
/// The most of an API answer Hersir reads (a list of 100,000 snapshots' worth is far below it).
const ANSWER_LIMIT: u64 = 64 << 20;

/// This computer, as `POST /devices` records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewDevice {
    /// The app's own device id (a UUID): `devices.id` of the row with `is_self = 1`.
    pub device_id: String,
    /// 1 to 255 characters.
    pub name: String,
    /// `macos`, `windows`, `linux`: up to 64 characters.
    pub platform: String,
    pub app_version: String,
    /// Base64 of this computer's 32-byte Ed25519 public key (`keys::DeviceKeys::sign_pub_b64`).
    pub sign_pub: String,
    /// Base64 of this computer's 32-byte X25519 public key (`keys::DeviceKeys::box_pub_b64`).
    pub box_pub: String,
    /// `"desktop"` (a computer) or `"web"` (the PWA).
    pub kind: String,
}

/// The account a token is for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Account {
    /// Hersir Cloud's account id, as a string (`"42"`).
    pub id: String,
    pub email: String,
}

/// `POST /devices`' answer: the account, and this computer's own token.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registered {
    pub account: Account,
    pub device_token: String,
    /// When the token runs out (ms): a year after it was made.
    pub expires_at: i64,
}

impl std::fmt::Debug for Registered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registered").field("account", &self.account).field("device_token", &format_args!("•••"))
            .field("expires_at", &self.expires_at).finish()
    }
}

/// `POST /devices/me/token`'s answer: this computer's new token (the old one is revoked).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceToken {
    pub device_token: String,
    pub expires_at: i64,
}

impl std::fmt::Debug for DeviceToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceToken").field("device_token", &format_args!("•••")).field("expires_at", &self.expires_at).finish()
    }
}

/// A computer or device of the account, as `GET /devices` lists it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub app_version: String,
    /// `"desktop"` or `"web"`.
    pub kind: String,
    pub remote_on: bool,
    pub created_at: i64,
    pub last_seen_at: Option<i64>,
    pub online: bool,
    /// The device making the call.
    pub this: bool,
    pub sign_pub: String,
    pub box_pub: String,
    /// When it was removed or signed out; None while it is signed in.
    pub revoked_at: Option<i64>,
}

/// What `PUT /devices/me` changes: each field that is set.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceChanges {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_on: Option<bool>,
}

/// `GET /me/entitlements`' answer, as received: check it with `entitlement::check` before reading it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedEntitlement {
    /// The payload's JSON text, exactly as received.
    pub payload: String,
    /// Base64 of the 64-byte Ed25519 signature over the payload's UTF-8 bytes.
    pub sig: String,
}

/// The account key as Hersir Cloud keeps it: sealed by the recovery code (`keys`), which it never sees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyRecord {
    /// The key's id: 32 lowercase hex characters (`keys::AccountKey::key_id`).
    pub key_id: String,
    /// The key's epoch, 0 to 255: the byte after `HC1` in every object.
    pub epoch: u8,
    /// Base64 of the account key sealed by the recovery code (docs/backup-format.md).
    pub sealed_by_code: String,
}

/// A presigned upload: one `PUT` of the object's bytes to `url`, with every header in `headers`, until `expires_at`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadUrl {
    pub id: String,
    pub url: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    pub expires_at: i64,
}

impl std::fmt::Debug for UploadUrl {
    // Not the URL's query: it is the upload's key.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UploadUrl").field("id", &self.id).field("url", &shown(&self.url)).field("headers", &self.headers.keys())
            .field("expires_at", &self.expires_at).finish()
    }
}

/// A presigned download: one `GET` of the object from `url`, until `expires_at`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadUrl {
    pub id: String,
    pub url: String,
    pub expires_at: i64,
}

impl std::fmt::Debug for DownloadUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DownloadUrl").field("id", &self.id).field("url", &shown(&self.url)).field("expires_at", &self.expires_at).finish()
    }
}

/// A snapshot as `POST /snapshots` registers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewSnapshot {
    /// 1 to 128 letters, digits, dots, dashes and underscores, starting with a letter or digit; unique on the account.
    pub name: String,
    /// When the database was copied (ms).
    pub made_at: i64,
    /// The computer that made it.
    pub device_id: String,
    /// `daily`, `manual`, `quit` or `before-restore`.
    pub reason: String,
    pub schema_version: i64,
    pub app_version: String,
    /// The id of the snapshot's sealed manifest, an object like the others.
    pub manifest_id: String,
    /// Every object the snapshot uses (the manifest counts whether or not it is listed).
    pub object_ids: Vec<String>,
    /// The snapshot's size for people.
    pub bytes: i64,
}

/// A snapshot as `GET /snapshots` lists it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Snapshot {
    pub name: String,
    pub made_at: i64,
    pub device_id: String,
    pub device_name: String,
    pub reason: String,
    pub schema_version: i64,
    pub app_version: String,
    pub bytes: i64,
    pub manifest_id: String,
    /// Set on a snapshot deleted by hand, listed for its 7 more days while it can be undeleted.
    pub deleted_at: Option<i64>,
}

/// Hersir Cloud's API, with this computer's token.
#[derive(Clone)]
pub struct Api {
    server: String,
    token: Zeroizing<String>,
    agent: ureq::Agent,
}

impl std::fmt::Debug for Api {
    // Never the token.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Api").field("server", &self.server).finish_non_exhaustive()
    }
}

impl Api {
    /// The API of the Hersir Cloud at `server` (`https://cloud.hersir.dev`, or the fake's `http://127.0.0.1:<port>`),
    /// with this computer's token.
    pub fn new(server: &str, device_token: &str) -> Result<Api> {
        let server = server_url(server)?;
        let agent = agent(&server, API_LIMIT);
        Ok(Api { server, token: Zeroizing::new(device_token.trim().to_string()), agent })
    }

    /// The server's address, as given (without a trailing slash).
    pub fn server(&self) -> &str {
        &self.server
    }

    /// `POST /devices` with the browser's sign-in token (scope `device:register`), which works once: records this
    /// computer on the account and gives it its own token. Use that token from then on (`Api::new`).
    pub fn register(server: &str, browser_token: &str, device: &NewDevice) -> Result<Registered> {
        Api::new(server, browser_token)?.send("POST", "/devices", Some(&json!(device))).and_then(parse)
    }

    /// `POST /devices/me/token`: a new token for this computer; this one is revoked at once.
    pub fn renew_token(&self) -> Result<DeviceToken> {
        self.send("POST", "/devices/me/token", Some(&json!({}))).and_then(parse)
    }

    /// `GET /devices`: all of the account's computers and devices, removed ones included, oldest first.
    pub fn devices(&self) -> Result<Vec<Device>> {
        self.send("GET", "/devices", None).and_then(parse)
    }

    /// `PUT /devices/me`: changes this computer's name, app version or Remote switch.
    pub fn update_device(&self, changes: &DeviceChanges) -> Result<Device> {
        self.send("PUT", "/devices/me", Some(&json!(changes))).and_then(parse)
    }

    /// `DELETE /devices/{id}`: removes a device of the account; `"me"` signs this computer out. `lost`: it was lost or
    /// stolen.
    pub fn remove_device(&self, id: &str, lost: bool) -> Result<()> {
        self.send("DELETE", &format!("/devices/{}", pct(id.trim())), Some(&json!({ "lost": lost }))).map(drop)
    }

    /// `GET /me/entitlements`: the plan, signed; check it with `entitlement::check` before reading it.
    pub fn entitlement(&self) -> Result<SignedEntitlement> {
        self.send("GET", "/me/entitlements", None).and_then(parse)
    }

    /// `GET /keys`: the account key, sealed by the recovery code; None while the account has none.
    pub fn key(&self) -> Result<Option<KeyRecord>> {
        match self.send("GET", "/keys", None) {
            Ok(v) => parse(v).map(Some),
            Err(Error::NotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// `PUT /keys`: stores the account key sealed by the recovery code, from epoch `if_epoch` (None: the account has no
    /// key yet). The key as stored, and whether it is the account's first (`201`). `Error::Epoch` when another computer
    /// changed the key meanwhile: get it again.
    pub fn put_key(&self, key: &KeyRecord, if_epoch: Option<u8>) -> Result<(KeyRecord, bool)> {
        let body = json!({ "key_id": key.key_id, "epoch": key.epoch, "sealed_by_code": key.sealed_by_code, "if_epoch": if_epoch });
        let (status, v) = self.call("PUT", "/keys", Some(&body))?;
        Ok((parse(v)?, status == 201))
    }

    /// `POST /objects/missing`: of `ids`, the ones the account has no stored object for, each once, in the order asked.
    pub fn missing(&self, ids: &[String]) -> Result<Vec<String>> {
        let mut out = Vec::new();
        for batch in ids.chunks(BATCH) {
            #[derive(Deserialize)]
            struct Missing {
                missing: Vec<String>,
            }
            let m: Missing = self.send("POST", "/objects/missing", Some(&json!({ "ids": batch }))).and_then(parse)?;
            out.extend(m.missing.into_iter().map(|id| id.to_ascii_lowercase()));
        }
        Ok(out)
    }

    /// `POST /objects/upload`: a presigned `PUT` for each object (its id and size in bytes). An object the account has
    /// stored already gets none: it needs no upload. `Error::PlanRequired`, `ReadOnly` or `Quota` when it can't take them.
    pub fn upload_urls(&self, objects: &[(String, u64)]) -> Result<Vec<UploadUrl>> {
        let mut out = Vec::new();
        for batch in objects.chunks(BATCH) {
            #[derive(Deserialize)]
            struct Urls {
                urls: Vec<UploadUrl>,
            }
            let list: Vec<Value> = batch.iter().map(|(id, bytes)| json!({ "id": id, "bytes": bytes })).collect();
            let u: Urls = self.send("POST", "/objects/upload", Some(&json!({ "objects": list }))).and_then(parse)?;
            out.extend(u.urls);
        }
        Ok(out)
    }

    /// `POST /objects/download`: a presigned `GET` for each id (valid an hour). `Error::NotFound` with the `ids` the
    /// account has no stored object for.
    pub fn download_urls(&self, ids: &[String]) -> Result<Vec<DownloadUrl>> {
        let mut out = Vec::new();
        for batch in ids.chunks(BATCH) {
            #[derive(Deserialize)]
            struct Urls {
                urls: Vec<DownloadUrl>,
            }
            let u: Urls = self.send("POST", "/objects/download", Some(&json!({ "ids": batch }))).and_then(parse)?;
            out.extend(u.urls);
        }
        Ok(out)
    }

    /// `PUT`s an object's bytes to its presigned URL, with the headers the server gave, without the token.
    pub fn put_object(&self, to: &UploadUrl, bytes: &[u8]) -> Result<()> {
        if !safe_url(&to.url) {
            return Err(Error::Storage(format!("Hersir Cloud gave an upload address that isn't secure (https): {}", shown(&to.url))));
        }
        let mut request = object_agent(&to.url).put(&to.url);
        for (name, value) in &to.headers {
            request = request.header(name.as_str(), value.as_str());
        }
        let mut response = request.send(bytes).map_err(|e| network(&to.url, e))?;
        let status = response.status().as_u16();
        if (200..300).contains(&status) {
            return Ok(());
        }
        let body = response.body_mut().with_config().limit(64 * 1024).lossy_utf8(true).read_to_string().unwrap_or_default();
        Err(Error::Storage(format!("Hersir Cloud's storage refused an upload (HTTP status {status}{})", storage_code(&body))))
    }

    /// `GET`s an object from its presigned URL, without the token: at most 4,194,368 bytes.
    pub fn get_object(&self, from: &DownloadUrl) -> Result<Vec<u8>> {
        if !safe_url(&from.url) {
            return Err(Error::Storage(format!("Hersir Cloud gave a download address that isn't secure (https): {}", shown(&from.url))));
        }
        let mut response = object_agent(&from.url).get(&from.url).call().map_err(|e| network(&from.url, e))?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            let body = response.body_mut().with_config().limit(64 * 1024).lossy_utf8(true).read_to_string().unwrap_or_default();
            return Err(Error::Storage(format!("Hersir Cloud's storage refused a download (HTTP status {status}{})", storage_code(&body))));
        }
        response.body_mut().with_config().limit(MAX_OBJECT as u64).read_to_vec().map_err(|e| network(&from.url, e))
    }

    /// `POST /snapshots`: registers a snapshot whose objects are all uploaded. `Error::MissingObjects` with the ids to
    /// upload (then register again); registering the same snapshot again answers the one registered already.
    pub fn create_snapshot(&self, snapshot: &NewSnapshot) -> Result<Snapshot> {
        self.send("POST", "/snapshots", Some(&json!(snapshot))).and_then(parse)
    }

    /// `GET /snapshots`: the account's kept snapshots, newest first, deleted ones in their 7 days included.
    pub fn snapshots(&self) -> Result<Vec<Snapshot>> {
        self.send("GET", "/snapshots", None).and_then(parse)
    }

    /// `DELETE /snapshots/{name}`: it is kept 7 more days, and can be undeleted until then.
    pub fn delete_snapshot(&self, name: &str) -> Result<()> {
        self.send("DELETE", &format!("/snapshots/{}", pct(name)), None).map(drop)
    }

    /// `POST /snapshots/{name}/undelete`: brings a snapshot deleted by hand back within its 7 days.
    pub fn undelete_snapshot(&self, name: &str) -> Result<Snapshot> {
        self.send("POST", &format!("/snapshots/{}/undelete", pct(name)), Some(&json!({}))).and_then(parse)
    }

    /// A call whose status doesn't matter beyond success: its answer's JSON (Null when it had none).
    fn send(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value> {
        self.call(method, path, body).map(|(_, v)| v)
    }

    /// One call to the API: its status and JSON answer, or the error the server answered (`error`).
    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<(u16, Value)> {
        let url = format!("{}{API_PATH}{path}", self.server);
        let bearer = Zeroizing::new(format!("Bearer {}", self.token.as_str()));
        let accept = "application/json";
        let sent = match (method, body) {
            ("GET", _) => self.agent.get(&url).header("Authorization", bearer.as_str()).header("Accept", accept).call(),
            ("POST", Some(b)) => self.agent.post(&url).header("Authorization", bearer.as_str()).header("Accept", accept).send_json(b),
            ("POST", None) => self.agent.post(&url).header("Authorization", bearer.as_str()).header("Accept", accept).send_empty(),
            ("PUT", Some(b)) => self.agent.put(&url).header("Authorization", bearer.as_str()).header("Accept", accept).send_json(b),
            ("PUT", None) => self.agent.put(&url).header("Authorization", bearer.as_str()).header("Accept", accept).send_empty(),
            ("DELETE", Some(b)) => self.agent.delete(&url).header("Authorization", bearer.as_str()).header("Accept", accept)
                .force_send_body().send_json(b),
            ("DELETE", None) => self.agent.delete(&url).header("Authorization", bearer.as_str()).header("Accept", accept).call(),
            _ => return Err(Error::Local(format!("Hersir doesn't call {method} on Hersir Cloud"))),
        };
        let mut response = sent.map_err(|e| network(&url, e))?;
        let status = response.status().as_u16();
        let retry_after = response.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(parse_retry_after);
        let raw = response.body_mut().with_config().limit(ANSWER_LIMIT).read_to_vec().map_err(|e| network(&url, e))?;
        let doc: Option<Value> = serde_json::from_slice(&raw).ok();
        if (200..300).contains(&status) {
            return match doc {
                Some(v) => Ok((status, v)),
                None if raw.iter().all(u8::is_ascii_whitespace) => Ok((status, Value::Null)),
                None => Err(Error::Network(format!("Hersir Cloud's answer to {method} {path} wasn't JSON"))),
            };
        }
        Err(api_error(status, doc.as_ref(), retry_after))
    }
}

/// The error a status and answer `{ "error": code, "message": … }` stand for.
pub(crate) fn api_error(status: u16, doc: Option<&Value>, retry_after: Option<Duration>) -> Error {
    let text = |key: &str| doc.and_then(|d| d.get(key)).and_then(Value::as_str).map(clean).filter(|s| !s.is_empty());
    let code = text("error").map(|c| c.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').take(64).collect::<String>());
    let message = text("message").unwrap_or_else(|| match status {
        500.. => format!("Hersir Cloud ran into an error (HTTP status {status}). Try again later."),
        _ => format!("Hersir Cloud refused the request (HTTP status {status})."),
    });
    let ids = || -> Vec<String> {
        doc.and_then(|d| d.get("ids")).and_then(Value::as_array)
            .map(|all| all.iter().filter_map(Value::as_str).map(|s| s.to_ascii_lowercase()).collect())
            .unwrap_or_default()
    };
    match (status, code.as_deref()) {
        (401, Some("signed_out")) => Error::SignedOut(message),
        (401, Some("removed")) => Error::Removed(message),
        (402, Some("plan_required")) => Error::PlanRequired(message),
        (402, Some("read_only")) => Error::ReadOnly(message),
        (403, Some("not_yours")) => Error::NotYours(message),
        (404, Some("not_found")) => Error::NotFound { message, ids: ids() },
        (409, Some("claimed")) => Error::Claimed(message),
        (409, Some("exists")) => Error::Exists(message),
        (409, Some("epoch")) => Error::Epoch(message),
        (409, Some("no_subscription")) => Error::NoSubscription(message),
        (413, Some("quota")) => Error::Quota(message),
        (413, Some("file_too_large")) => Error::FileTooLarge(message),
        (422, Some("invalid")) => Error::Invalid { message, ids: ids() },
        (422, Some("missing_objects")) => Error::MissingObjects { message, ids: ids() },
        (426, Some("update_hersir")) => Error::UpdateHersir(message),
        (429, _) => Error::SlowDown { message, retry_after },
        _ => Error::Server { status, code, message },
    }
}

/// `Retry-After`: whole seconds (what Hersir Cloud sends).
fn parse_retry_after(value: &str) -> Option<Duration> {
    value.trim().parse::<u64>().ok().map(Duration::from_secs)
}

/// R2's error code in its XML answer (`<Code>AccessDenied</Code>`), for a message: ", AccessDenied".
fn storage_code(body: &str) -> String {
    let code = body.split_once("<Code>").and_then(|(_, rest)| rest.split_once("</Code>")).map(|(code, _)| code);
    code.map(|c| c.chars().filter(|c| c.is_ascii_alphanumeric()).take(64).collect::<String>())
        .filter(|c| !c.is_empty()).map(|c| format!(", {c}")).unwrap_or_default()
}

/// An answer's JSON as `T`.
fn parse<T: DeserializeOwned>(v: Value) -> Result<T> {
    serde_json::from_value(v).map_err(|e| Error::Network(format!("Hersir Cloud's answer wasn't what Hersir expects: {e}")))
}

/// The server's address without a trailing slash: https, or http on this computer.
fn server_url(server: &str) -> Result<String> {
    let s = server.trim().trim_end_matches('/');
    if host_of(s).is_none() {
        return Err(Error::Local(format!("{} isn't a web address (https://…)", shown(s))));
    }
    if !safe_url(s) {
        return Err(Error::Local(format!("{} isn't secure (http): Hersir only talks to Hersir Cloud over https", shown(s))));
    }
    if s.contains(['?', '#']) {
        return Err(Error::Local(format!("{} should be the server's address alone, without a query", shown(s))));
    }
    Ok(s.to_string())
}

/// An agent for one address: HTTP errors as answers (each is turned into an `Error`), no redirects (a token or an
/// object's bytes go only where they are sent), and no proxy for this computer's own addresses.
fn agent(url: &str, limit: Duration) -> ureq::Agent {
    let mut config = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(limit))
        .max_redirects(0)
        .user_agent(format!("Hersir/{}", env!("CARGO_PKG_VERSION")));
    if host_of(url).is_some_and(|h| is_loopback(&h)) {
        config = config.proxy(None);
    }
    config.build().into()
}

/// The agent for a presigned URL.
fn object_agent(url: &str) -> ureq::Agent {
    agent(url, OBJECT_LIMIT)
}

/// A request that failed on the way, in plain words.
fn network(url: &str, e: ureq::Error) -> Error {
    let why = match &e {
        ureq::Error::Timeout(_) => "it gave no answer in time".to_string(),
        ureq::Error::Io(io) if io.kind() == ErrorKind::TimedOut => "it gave no answer in time".to_string(),
        ureq::Error::Io(io) if io.kind() == ErrorKind::ConnectionRefused => "it refused the connection".to_string(),
        ureq::Error::HostNotFound => "the address wasn't found".to_string(),
        ureq::Error::ConnectionFailed => "the connection failed".to_string(),
        ureq::Error::BodyExceedsLimit(_) => "its answer was too large".to_string(),
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) => format!("the secure connection (TLS) failed: {e}"),
        _ => e.to_string(),
    };
    let host = host_of(url).unwrap_or_else(|| "Hersir Cloud".to_string());
    Error::Network(format!("Couldn't get an answer from {host}: {why}"))
}

/// Whether an error is worth trying again after a pause: the server asked to slow down, it had an error of its own, or
/// it couldn't be reached.
pub(crate) fn passing(e: &Error) -> bool {
    matches!(e, Error::SlowDown { .. } | Error::Network(_) | Error::Storage(_)) || matches!(e, Error::Server { status: 500.., .. })
}

/// Tries `f` up to `tries` times while it fails with a passing error (`passing`), waiting the server's `Retry-After`
/// (at most 2 minutes; 2 seconds when it gave none) in between.
pub(crate) fn retried<T>(tries: usize, mut f: impl FnMut() -> Result<T>) -> Result<T> {
    let mut left = tries.max(1);
    loop {
        match f() {
            Err(e) if left > 1 && passing(&e) => {
                left -= 1;
                let wait = e.retry_after().unwrap_or(Duration::from_secs(2)).min(Duration::from_secs(120));
                std::thread::sleep(wait);
            }
            other => return other,
        }
    }
}
