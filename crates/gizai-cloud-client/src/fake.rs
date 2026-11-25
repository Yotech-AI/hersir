//! A fake Hersir Cloud, in memory on 127.0.0.1, for tests and local runs (`cargo run -p gizai-cloud-client --example
//! fake-cloud`). It answers as PROTOCOL.md says the real one does:
//!
//! - the device, entitlement, key, object and snapshot calls under `/api/v1`, with their errors (401 `signed_out` and
//!   `removed`, 402 `plan_required` and `read_only`, 403 `not_yours`, 404 `not_found`, 409 `exists` and `epoch`, 413
//!   `quota`, 422 `invalid` and `missing_objects`); any error, 429 with `Retry-After` included, can be asked for once
//!   with `fail_next`;
//! - presigned URLs that point to itself (`/r2/…`): a `PUT` must carry the header its upload URL gave;
//! - its own entitlement key pair (`entitlement_key`), signing `GET /me/entitlements` as the server does;
//! - the OAuth calls `gizai_agents::oauth` makes: the two metadata documents, `/oauth/authorize`, which sends the
//!   browser straight back to its loopback `redirect_uri` with a code (no sign-in page: `browse` plays the browser), and
//!   `/oauth/token` (PKCE S256 checked).
//!
//! Every byte it receives, R2's included, is kept (`received`), so a test can look through what left the computer.
//! There is one account to begin with (`account`), with an active plan; `add_account` makes more. Nothing is pruned
//! and there is no rate limit. It stops when dropped.
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signer, SigningKey};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::api::{Account, Device, KeyRecord, SignedEntitlement, Snapshot};
use crate::chunks::MAX_OBJECT;
use crate::util::{b64, hex, now_ms, random, unb64, unpct};

/// The quota of an account: 10 GB.
pub const QUOTA: i64 = 10_000_000_000;
/// The first account's email.
pub const EMAIL: &str = "you@example.com";
/// The header a presigned upload must carry, with the value its URL gave.
pub const UPLOAD_HEADER: &str = "x-fake-r2-check";
const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;
/// The most of a request it reads.
const REQUEST_LIMIT: usize = 64 << 20;

/// An account's plan, as the entitlement shows it and the API enforces it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Uploads and new snapshots work; `valid_until` is when a cancelled plan ends (None: it renews).
    Active { valid_until: Option<i64> },
    /// Stripe is retrying a failed renewal: everything works.
    PastDue,
    /// The plan ended at `valid_until`: read only until `read_only_until`, then `plan_required` for reads too.
    Lapsed { valid_until: i64, read_only_until: i64 },
    /// No plan: `plan_required` for uploads and new snapshots.
    None,
}

/// How to start a fake.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// The port on 127.0.0.1; 0 for any free one.
    pub port: u16,
    /// The entitlement key's Ed25519 seed; None for a random one.
    pub entitlement_seed: Option<[u8; 32]>,
}

/// An error the next matching call answers (`FakeCloud::fail_next`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// The method (`POST`), and the path the call's starts with: under the API (`/snapshots`), or a full one
    /// (`/oauth/token`, `/r2/`).
    pub method: String,
    pub path: String,
    pub status: u16,
    /// The error code (`slow_down`); empty for an answer without a body of the API's form.
    pub code: String,
    pub message: String,
    /// `Retry-After`, in seconds.
    pub retry_after: Option<u64>,
    /// The error's `ids`.
    pub ids: Vec<String>,
}

impl Failure {
    pub fn new(method: &str, path: &str, status: u16, code: &str, message: &str) -> Failure {
        Failure {
            method: method.to_ascii_uppercase(),
            path: path.to_string(),
            status,
            code: code.to_string(),
            message: message.to_string(),
            retry_after: None,
            ids: vec![],
        }
    }

    pub fn retry_after(mut self, seconds: u64) -> Failure {
        self.retry_after = Some(seconds);
        self
    }

    pub fn ids(mut self, ids: &[&str]) -> Failure {
        self.ids = ids.iter().map(|s| s.to_string()).collect();
        self
    }
}

/// A call the fake answered: its method, path (without the query) and status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub method: String,
    pub path: String,
    pub status: u16,
}

/// A fake Hersir Cloud on 127.0.0.1. It stops when dropped.
pub struct FakeCloud {
    shared: Arc<Shared>,
    addr: SocketAddr,
    accept: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for FakeCloud {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeCloud").field("url", &self.shared.url).finish_non_exhaustive()
    }
}

struct Shared {
    url: String,
    signing: SigningKey,
    stop: AtomicBool,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    accounts: Vec<AccountState>,
    /// Whom `/oauth/authorize` signs in.
    signing_in: String,
    codes: HashMap<String, Code>,
    browser_tokens: HashMap<String, BrowserToken>,
    refresh_tokens: HashMap<String, Refresh>,
    device_tokens: HashMap<String, DeviceTokenState>,
    devices: Vec<DeviceState>,
    /// R2's objects, by key (`o/<account>/<id>`).
    r2: HashMap<String, Vec<u8>>,
    presigned: HashMap<String, Presigned>,
    failures: Vec<Failure>,
    /// Uploads to answer 200 and keep nothing.
    forget_uploads: usize,
    received: Vec<u8>,
    calls: Vec<Call>,
    uploads: Vec<String>,
    quota: i64,
}

struct AccountState {
    id: String,
    email: String,
    plan: Plan,
    key: Option<KeyRecord>,
    objects: HashMap<String, ObjectState>,
    snapshots: Vec<SnapshotState>,
}

struct ObjectState {
    /// The size given at `POST /objects/upload`, then its real one once stored.
    size: i64,
    stored: bool,
}

#[derive(Clone)]
struct SnapshotState {
    name: String,
    made_at: i64,
    device_id: String,
    reason: String,
    schema_version: i64,
    app_version: String,
    bytes: i64,
    manifest_id: String,
    objects: Vec<String>,
    deleted_at: Option<i64>,
}

#[derive(Clone)]
struct DeviceState {
    id: String,
    account: String,
    name: String,
    platform: String,
    app_version: String,
    kind: String,
    remote_on: bool,
    created_at: i64,
    last_seen_at: Option<i64>,
    sign_pub: String,
    box_pub: String,
    revoked_at: Option<i64>,
    lost_at: Option<i64>,
}

struct DeviceTokenState {
    account: String,
    device_id: String,
    expires_at: i64,
    revoked: bool,
}

struct BrowserToken {
    account: String,
    scopes: Vec<String>,
    expires_at: i64,
    used: bool,
    refresh: Option<String>,
}

struct Code {
    account: String,
    client_id: String,
    redirect_uri: String,
    challenge: String,
    scopes: Vec<String>,
    expires_at: i64,
}

struct Refresh {
    account: String,
    client_id: String,
    scopes: Vec<String>,
    expires_at: i64,
}

struct Presigned {
    method: &'static str,
    key: String,
    id: String,
    expires_at: i64,
    header: Option<String>,
}

impl FakeCloud {
    /// A fake on a free port, with a random entitlement key.
    pub fn start() -> io::Result<FakeCloud> {
        FakeCloud::start_with(Options::default())
    }

    pub fn start_with(options: Options) -> io::Result<FakeCloud> {
        let listener = TcpListener::bind(("127.0.0.1", options.port))?;
        let addr = listener.local_addr()?;
        let seed = match options.entitlement_seed {
            Some(seed) => seed,
            None => random::<32>().map_err(|e| io::Error::other(e.to_string()))?,
        };
        let url = format!("http://127.0.0.1:{}", addr.port());
        let mut state = State { quota: QUOTA, signing_in: "1".into(), ..State::default() };
        state.accounts.push(AccountState::new("1", EMAIL));
        let shared = Arc::new(Shared { url, signing: SigningKey::from_bytes(&seed), stop: AtomicBool::new(false), state: Mutex::new(state) });
        let accepting = shared.clone();
        let accept = thread::Builder::new().name("fake-hersir-cloud".into()).spawn(move || {
            for stream in listener.incoming() {
                if accepting.stop.load(Ordering::SeqCst) {
                    break;
                }
                if let Ok(stream) = stream {
                    let one = accepting.clone();
                    let _ = thread::Builder::new().name("fake-hersir-cloud-call".into()).spawn(move || one.serve(stream));
                }
            }
        })?;
        Ok(FakeCloud { shared, addr, accept: Some(accept) })
    }

    /// Its address, `http://127.0.0.1:<port>`: the server's address for `Api::new`.
    pub fn url(&self) -> &str {
        &self.shared.url
    }

    /// The entitlement key's public half: what Hersir checks `GET /me/entitlements` with.
    pub fn entitlement_key(&self) -> [u8; 32] {
        self.shared.signing.verifying_key().to_bytes()
    }

    /// The entitlement key's public half in base64, as `php artisan hersir:entitlement-key` shows it.
    pub fn entitlement_key_b64(&self) -> String {
        b64(&self.entitlement_key())
    }

    /// `payload` signed with the fake's entitlement key, as `GET /me/entitlements` gives one: to test a payload the fake
    /// wouldn't make (another version, another account).
    pub fn sign_entitlement(&self, payload: &str) -> SignedEntitlement {
        SignedEntitlement { payload: payload.to_string(), sig: b64(&self.shared.signing.sign(payload.as_bytes()).to_bytes()) }
    }

    /// The first account.
    pub fn account(&self) -> Account {
        let s = self.state();
        Account { id: s.accounts[0].id.clone(), email: s.accounts[0].email.clone() }
    }

    /// Another account, with an active plan.
    pub fn add_account(&self, email: &str) -> Account {
        let mut s = self.state();
        let id = (s.accounts.len() + 1).to_string();
        s.accounts.push(AccountState::new(&id, email));
        Account { id, email: email.to_string() }
    }

    /// The account `/oauth/authorize` signs in from now on (the first one to begin with).
    pub fn sign_in_as(&self, account_id: &str) {
        self.state().signing_in = account_id.to_string();
    }

    /// A browser's sign-in token for `account_id` (scope `device:register`), as `/oauth/token` gives it: it works once,
    /// for `POST /devices`.
    pub fn browser_token(&self, account_id: &str) -> String {
        let mut s = self.state();
        let token = token("hcb");
        s.browser_tokens.insert(token.clone(), BrowserToken {
            account: account_id.to_string(),
            scopes: vec!["device:register".into()],
            expires_at: now_ms() + HOUR,
            used: false,
            refresh: None,
        });
        token
    }

    pub fn set_plan(&self, account_id: &str, plan: Plan) {
        if let Some(a) = self.state().account_mut(account_id) {
            a.plan = plan;
        }
    }

    /// Every account's quota (10 GB to begin with).
    pub fn set_quota(&self, bytes: i64) {
        self.state().quota = bytes;
    }

    /// The next call that matches answers this error, once.
    pub fn fail_next(&self, failure: Failure) {
        self.state().failures.push(failure);
    }

    /// The next `n` uploads to R2 answer 200 and keep nothing: their snapshot gets `missing_objects`.
    pub fn forget_next_uploads(&self, n: usize) {
        self.state().forget_uploads = n;
    }

    /// Removes a device, as the account page's Remove… does: its token gets `401 removed` from then on.
    pub fn remove_device(&self, device_id: &str, lost: bool) {
        let mut s = self.state();
        if let Some(d) = s.devices.iter_mut().find(|d| d.id == device_id.to_ascii_lowercase()) {
            let now = now_ms();
            d.revoked_at.get_or_insert(now);
            if lost {
                d.lost_at.get_or_insert(now);
            }
        }
    }

    /// The account no longer has object `id`: not in its index, not in R2.
    pub fn lose_object(&self, account_id: &str, id: &str) {
        let mut s = self.state();
        s.r2.remove(&format!("o/{account_id}/{id}"));
        if let Some(a) = s.account_mut(account_id) {
            a.objects.remove(id);
        }
    }

    /// Every byte the fake and its R2 received so far, in the order it came.
    pub fn received(&self) -> Vec<u8> {
        self.state().received.clone()
    }

    /// The calls it answered so far.
    pub fn calls(&self) -> Vec<Call> {
        self.state().calls.clone()
    }

    /// The object ids uploaded to its R2 so far, in order (one per `PUT` that it kept).
    pub fn uploads(&self) -> Vec<String> {
        self.state().uploads.clone()
    }

    /// An account's stored objects' ids (pending ones aren't).
    pub fn objects(&self, account_id: &str) -> Vec<String> {
        let s = self.state();
        let mut ids: Vec<String> = s.account(account_id).map(|a| a.objects.iter().filter(|(_, o)| o.stored).map(|(id, _)| id.clone()).collect())
            .unwrap_or_default();
        ids.sort();
        ids
    }

    /// An object's bytes in R2.
    pub fn object(&self, account_id: &str, id: &str) -> Option<Vec<u8>> {
        self.state().r2.get(&format!("o/{account_id}/{id}")).cloned()
    }

    /// Puts bytes in R2 under an object's key, as if they had been uploaded: to test a changed object.
    pub fn replace_object(&self, account_id: &str, id: &str, bytes: Vec<u8>) {
        self.state().r2.insert(format!("o/{account_id}/{id}"), bytes);
    }

    /// An account's snapshots, as `GET /snapshots` lists them (deleted ones in their 7 days included).
    pub fn snapshots(&self, account_id: &str) -> Vec<Snapshot> {
        let s = self.state();
        s.snapshots_of(account_id)
    }

    /// An account's sealed key, as `GET /keys` gives it.
    pub fn key(&self, account_id: &str) -> Option<KeyRecord> {
        self.state().account(account_id).and_then(|a| a.key.clone())
    }

    /// An account's devices, as `GET /devices` lists them (`this` false).
    pub fn devices(&self, account_id: &str) -> Vec<Device> {
        let s = self.state();
        s.devices.iter().filter(|d| d.account == account_id).map(|d| d.json(None)).collect()
    }

    /// Whether a device was removed as lost or stolen.
    pub fn lost(&self, device_id: &str) -> bool {
        self.state().devices.iter().any(|d| d.id == device_id.to_ascii_lowercase() && d.lost_at.is_some())
    }

    /// The objects a snapshot registered (its manifest included), in the order it gave them.
    pub fn snapshot_objects(&self, account_id: &str, name: &str) -> Vec<String> {
        let s = self.state();
        s.account(account_id).and_then(|a| a.snapshots.iter().find(|x| x.name == name)).map(|x| x.objects.clone()).unwrap_or_default()
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.shared.state()
    }
}

impl Drop for FakeCloud {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        // wakes the accept loop
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_secs(1));
        if let Some(t) = self.accept.take() {
            let _ = t.join();
        }
    }
}

/// What a browser does with a sign-in address: opens it, and follows the fake's answer back to Hersir's loopback
/// `redirect_uri`. The page Hersir showed there, or why it went wrong.
pub fn browse(authorize_url: &str) -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).max_redirects(0).proxy(None)
        .timeout_global(Some(Duration::from_secs(60))).build().into();
    let first = agent.get(authorize_url).call().map_err(|e| e.to_string())?;
    let status = first.status().as_u16();
    let Some(location) = first.headers().get("location").and_then(|v| v.to_str().ok()).map(String::from) else {
        let mut first = first;
        let body = first.body_mut().read_to_string().unwrap_or_default();
        return Err(format!("the sign-in answered {status} without sending the browser back: {body}"));
    };
    let mut back = agent.get(&location).call().map_err(|e| e.to_string())?;
    let body = back.body_mut().read_to_string().map_err(|e| e.to_string())?;
    Ok(body)
}

impl AccountState {
    fn new(id: &str, email: &str) -> AccountState {
        AccountState {
            id: id.to_string(),
            email: email.to_string(),
            plan: Plan::Active { valid_until: None },
            key: None,
            objects: HashMap::new(),
            snapshots: vec![],
        }
    }

    /// Its use: the real size of each stored object, once.
    fn used(&self) -> i64 {
        self.objects.values().filter(|o| o.stored).map(|o| o.size).sum()
    }
}

impl DeviceState {
    fn json(&self, this: Option<&str>) -> Device {
        Device {
            id: self.id.clone(),
            name: self.name.clone(),
            platform: self.platform.clone(),
            app_version: self.app_version.clone(),
            kind: self.kind.clone(),
            remote_on: self.remote_on,
            created_at: self.created_at,
            last_seen_at: self.last_seen_at,
            online: false,
            this: this == Some(self.id.as_str()),
            sign_pub: self.sign_pub.clone(),
            box_pub: self.box_pub.clone(),
            revoked_at: self.revoked_at,
        }
    }
}

impl SnapshotState {
    fn json(&self, device_name: &str) -> Snapshot {
        Snapshot {
            name: self.name.clone(),
            made_at: self.made_at,
            device_id: self.device_id.clone(),
            device_name: device_name.to_string(),
            reason: self.reason.clone(),
            schema_version: self.schema_version,
            app_version: self.app_version.clone(),
            bytes: self.bytes,
            manifest_id: self.manifest_id.clone(),
            deleted_at: self.deleted_at,
        }
    }
}

impl State {
    fn account(&self, id: &str) -> Option<&AccountState> {
        self.accounts.iter().find(|a| a.id == id)
    }

    fn account_mut(&mut self, id: &str) -> Option<&mut AccountState> {
        self.accounts.iter_mut().find(|a| a.id == id)
    }

    fn device_name(&self, id: &str) -> String {
        self.devices.iter().find(|d| d.id == id).map(|d| d.name.clone()).unwrap_or_default()
    }

    /// An account's snapshots, newest first.
    fn snapshots_of(&self, account_id: &str) -> Vec<Snapshot> {
        let Some(a) = self.account(account_id) else { return vec![] };
        let mut all: Vec<Snapshot> = a.snapshots.iter().map(|s| s.json(&self.device_name(&s.device_id))).collect();
        all.sort_by(|x, y| y.made_at.cmp(&x.made_at).then_with(|| y.name.cmp(&x.name)));
        all
    }
}

/// A request as the fake read it.
struct Request {
    method: String,
    path: String,
    query: HashMap<String, String>,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    fn bearer(&self) -> Option<&str> {
        let auth = self.header("authorization")?.trim();
        let (kind, token) = auth.split_once(' ')?;
        kind.eq_ignore_ascii_case("bearer").then(|| token.trim())
    }

    fn json(&self) -> Result<Value, Reply> {
        if self.body.iter().all(u8::is_ascii_whitespace) {
            return Ok(json!({}));
        }
        serde_json::from_slice(&self.body).map_err(|_| Reply::error(422, "invalid", "The request's body isn't JSON."))
    }

    fn form(&self) -> HashMap<String, String> {
        query_params(&String::from_utf8_lossy(&self.body))
    }
}

/// An answer.
struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Reply {
    fn json(status: u16, v: &Value) -> Reply {
        Reply { status, headers: vec![("Content-Type".into(), "application/json".into())], body: v.to_string().into_bytes() }
    }

    fn of<T: Serialize>(status: u16, v: &T) -> Reply {
        Reply::json(status, &serde_json::to_value(v).unwrap_or(Value::Null))
    }

    fn empty(status: u16) -> Reply {
        Reply { status, headers: vec![], body: vec![] }
    }

    /// The API's error: `{ "error": code, "message": … }`.
    fn error(status: u16, code: &str, message: &str) -> Reply {
        Reply::json(status, &json!({ "error": code, "message": message }))
    }

    fn error_ids(status: u16, code: &str, message: &str, ids: &[String]) -> Reply {
        Reply::json(status, &json!({ "error": code, "message": message, "ids": ids }))
    }

    /// Passport's OAuth error.
    fn oauth(status: u16, error: &str, description: &str) -> Reply {
        Reply::json(status, &json!({ "error": error, "error_description": description, "message": description }))
    }

    /// R2's error: XML.
    fn storage(status: u16, code: &str, message: &str) -> Reply {
        let body = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><Error><Code>{code}</Code><Message>{message}</Message></Error>");
        Reply { status, headers: vec![("Content-Type".into(), "application/xml".into())], body: body.into_bytes() }
    }

    fn redirect(location: &str) -> Reply {
        Reply { status: 302, headers: vec![("Location".into(), location.to_string())], body: vec![] }
    }
}

/// The 401 for no token or a bad one.
fn signed_out() -> Reply {
    Reply::error(401, "signed_out", "Sign in to Hersir Cloud again.")
}

fn removed() -> Reply {
    Reply::error(401, "removed", "This computer was removed from Hersir Cloud on the account page.")
}

fn invalid(message: &str) -> Reply {
    Reply::error(422, "invalid", message)
}

fn not_found(message: &str) -> Reply {
    Reply::error(404, "not_found", message)
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// One connection: one request, one answer, then it closes.
    fn serve(&self, mut stream: TcpStream) {
        if self.stop.load(Ordering::SeqCst) {
            return;
        }
        let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
        let mut raw = Vec::new();
        let request = read_request(&mut stream, &mut raw);
        self.state().received.extend_from_slice(&raw);
        let Some(request) = request else { return };
        let reply = self.answer(&request);
        self.state().calls.push(Call { method: request.method.clone(), path: request.path.clone(), status: reply.status });
        write_reply(&mut stream, &reply);
    }

    fn answer(&self, r: &Request) -> Reply {
        let p = r.path.as_str();
        if let Some(f) = self.failure(r) {
            let mut reply = if f.code.is_empty() {
                Reply::json(f.status, &json!({ "message": f.message }))
            } else if f.ids.is_empty() {
                Reply::error(f.status, &f.code, &f.message)
            } else {
                Reply::error_ids(f.status, &f.code, &f.message, &f.ids)
            };
            if let Some(s) = f.retry_after {
                reply.headers.push(("Retry-After".into(), s.to_string()));
            }
            return reply;
        }
        match (r.method.as_str(), p) {
            ("GET", "/.well-known/oauth-protected-resource" | "/.well-known/oauth-protected-resource/api") => Reply::json(200, &json!({
                "resource": format!("{}/api", self.url),
                "authorization_servers": [self.url],
                "scopes_supported": ["device:register"],
                "resource_name": "Hersir Cloud",
            })),
            ("GET", "/.well-known/oauth-authorization-server") => Reply::json(200, &json!({
                "issuer": self.url,
                "authorization_endpoint": format!("{}/oauth/authorize", self.url),
                "token_endpoint": format!("{}/oauth/token", self.url),
                "code_challenge_methods_supported": ["S256"],
                "response_types_supported": ["code"],
                "grant_types_supported": ["authorization_code", "refresh_token"],
            })),
            ("GET", "/oauth/authorize") => self.authorize(r),
            ("POST", "/oauth/token") => self.token(r),
            (_, p) if p.starts_with("/api/v1/") => self.api(r, &p["/api/v1".len()..]),
            ("PUT" | "GET", p) if p.starts_with("/r2/") => self.r2(r, &p["/r2/".len()..]),
            _ => not_found("There is nothing here."),
        }
    }

    /// The first failure asked for that matches the request: taken, so it answers once.
    fn failure(&self, r: &Request) -> Option<Failure> {
        let mut s = self.state();
        let at = s.failures.iter().position(|f| {
            let path = if f.path.starts_with("/api/") || f.path.starts_with("/r2/") || f.path.starts_with("/oauth/") || f.path.starts_with("/.well-known/") {
                f.path.clone()
            } else {
                format!("/api/v1{}", f.path)
            };
            f.method.eq_ignore_ascii_case(&r.method) && r.path.starts_with(&path)
        })?;
        Some(s.failures.remove(at))
    }

    // ---- OAuth ----

    fn authorize(&self, r: &Request) -> Reply {
        let q = &r.query;
        let get = |k: &str| q.get(k).map(String::as_str).unwrap_or_default();
        let redirect_uri = get("redirect_uri");
        if !loopback_callback(redirect_uri) {
            return Reply::oauth(400, "invalid_request", "redirect_uri must be http://127.0.0.1:<port>/callback or http://[::1]:<port>/callback.");
        }
        if get("code_challenge_method") != "S256" || get("code_challenge").is_empty() {
            return Reply::json(400, &json!({
                "error": "invalid_request",
                "error_description": "The request is missing a required parameter, includes an invalid parameter value, includes a parameter more than once, or is otherwise malformed.",
                "hint": "Hersir Cloud only accepts PKCE with code_challenge_method=S256.",
                "message": "The request is missing a required parameter, includes an invalid parameter value, includes a parameter more than once, or is otherwise malformed.",
            }));
        }
        if get("response_type") != "code" || get("client_id").is_empty() {
            return Reply::oauth(400, "invalid_request", "response_type must be code, with a client_id.");
        }
        let state = get("state");
        let back = |params: &str| {
            let sep = if redirect_uri.contains('?') { '&' } else { '?' };
            Reply::redirect(&format!("{redirect_uri}{sep}{params}&state={}", crate::util::pct(state)))
        };
        let asked: Vec<&str> = get("scope").split_whitespace().collect();
        if asked.iter().any(|s| !matches!(*s, "device:register" | "cloud")) {
            return back("error=invalid_scope");
        }
        // `device:register` is the browser's only scope: asked for, or no scope at all; `cloud` is dropped.
        let scopes = if asked.is_empty() || asked.contains(&"device:register") { vec!["device:register".to_string()] } else { vec![] };
        let mut s = self.state();
        let code = token("hcc");
        let account = s.signing_in.clone();
        s.codes.insert(code.clone(), Code {
            account,
            client_id: get("client_id").to_string(),
            redirect_uri: redirect_uri.to_string(),
            challenge: get("code_challenge").to_string(),
            scopes,
            expires_at: now_ms() + 10 * MINUTE,
        });
        back(&format!("code={code}"))
    }

    fn token(&self, r: &Request) -> Reply {
        let f = r.form();
        let get = |k: &str| f.get(k).map(String::as_str).unwrap_or_default();
        let mut s = self.state();
        let (account, client_id, scopes) = match get("grant_type") {
            "authorization_code" => {
                let Some(code) = s.codes.remove(get("code")) else {
                    return Reply::oauth(400, "invalid_grant", "The authorization code is unknown, used or expired.");
                };
                let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(get("code_verifier").as_bytes()));
                if code.expires_at < now_ms() || code.client_id != get("client_id") || code.redirect_uri != get("redirect_uri") || challenge != code.challenge {
                    return Reply::oauth(400, "invalid_grant", "The authorization code doesn't match the request (client, redirect_uri or code_verifier).");
                }
                (code.account, code.client_id, code.scopes)
            }
            "refresh_token" => {
                let Some(old) = s.refresh_tokens.remove(get("refresh_token")) else {
                    return Reply::oauth(400, "invalid_grant", "The refresh token is unknown, used or expired.");
                };
                if old.expires_at < now_ms() || old.client_id != get("client_id") {
                    return Reply::oauth(400, "invalid_grant", "The refresh token doesn't match the request.");
                }
                (old.account, old.client_id, old.scopes)
            }
            _ => return Reply::oauth(400, "unsupported_grant_type", "The grant type isn't supported."),
        };
        let access = token("hcb");
        let refresh = token("hcr");
        s.browser_tokens.insert(access.clone(), BrowserToken {
            account: account.clone(),
            scopes: scopes.clone(),
            expires_at: now_ms() + HOUR,
            used: false,
            refresh: Some(refresh.clone()),
        });
        s.refresh_tokens.insert(refresh.clone(), Refresh { account, client_id, scopes, expires_at: now_ms() + DAY });
        Reply::json(200, &json!({ "token_type": "Bearer", "expires_in": 3600, "access_token": access, "refresh_token": refresh }))
    }

    // ---- the API ----

    fn api(&self, r: &Request, path: &str) -> Reply {
        let parts: Vec<String> = path.trim_start_matches('/').split('/').map(unpct).collect();
        let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
        if r.method == "POST" && parts == ["devices"] {
            return self.register(r);
        }
        let (account, device) = match self.device_auth(r) {
            Ok(who) => who,
            Err(reply) => return reply,
        };
        let body = match r.method.as_str() {
            "POST" | "PUT" | "DELETE" => match r.json() {
                Ok(v) => v,
                Err(reply) => return reply,
            },
            _ => Value::Null,
        };
        match (r.method.as_str(), parts.as_slice()) {
            ("POST", ["devices", "me", "token"]) => self.renew(r, &account, &device),
            ("GET", ["devices"]) => {
                let s = self.state();
                let all: Vec<Device> = s.devices.iter().filter(|d| d.account == account).map(|d| d.json(Some(device.as_str()))).collect();
                Reply::of(200, &all)
            }
            ("PUT", ["devices", "me"]) => self.update_device(&device, &body),
            ("DELETE", ["devices", id]) => self.remove(&account, &device, id, &body),
            ("GET", ["me", "entitlements"]) => self.entitlement(&account),
            ("GET", ["keys"]) => match self.state().account(&account).and_then(|a| a.key.clone()) {
                Some(k) => Reply::of(200, &k),
                None => not_found("This account has no key yet."),
            },
            ("PUT", ["keys"]) => self.put_key(&account, &body),
            ("POST", ["objects", "missing"]) => self.missing(&account, &body),
            ("POST", ["objects", "upload"]) => self.upload(&account, &body),
            ("POST", ["objects", "download"]) => self.download(&account, &body),
            ("POST", ["snapshots"]) => self.create_snapshot(&account, &body),
            ("GET", ["snapshots"]) => {
                let s = self.state();
                if let Some(refused) = s.account(&account).and_then(|a| read_refused(&a.plan)) {
                    return refused;
                }
                Reply::of(200, &s.snapshots_of(&account))
            }
            ("DELETE", ["snapshots", name]) => {
                let mut s = self.state();
                let Some(snap) = s.account_mut(&account).and_then(|a| a.snapshots.iter_mut().find(|x| x.name == *name)) else {
                    return not_found("This account has no backup with that name.");
                };
                snap.deleted_at.get_or_insert(now_ms());
                Reply::empty(204)
            }
            ("POST", ["snapshots", name, "undelete"]) => {
                let mut s = self.state();
                if let Some(refused) = s.account(&account).and_then(|a| read_refused(&a.plan)) {
                    return refused;
                }
                let Some(snap) = s.account_mut(&account).and_then(|a| a.snapshots.iter_mut().find(|x| x.name == *name)) else {
                    return not_found("This account has no backup with that name.");
                };
                snap.deleted_at = None;
                let snap = snap.clone();
                Reply::of(200, &snap.json(&s.device_name(&snap.device_id)))
            }
            _ => not_found("There is no such call."),
        }
    }

    /// The computer's token: its account and device, or the 401 to answer.
    fn device_auth(&self, r: &Request) -> Result<(String, String), Reply> {
        let token = r.bearer().ok_or_else(signed_out)?;
        let mut s = self.state();
        let (account, device_id, expired, revoked) = match s.device_tokens.get(token) {
            Some(t) => (t.account.clone(), t.device_id.clone(), t.expires_at < now_ms(), t.revoked),
            None => return Err(signed_out()),
        };
        let Some(d) = s.devices.iter_mut().find(|d| d.id == device_id) else { return Err(signed_out()) };
        if d.revoked_at.is_some() {
            return Err(removed());
        }
        if expired || revoked {
            return Err(signed_out());
        }
        d.last_seen_at = Some(now_ms());
        Ok((account, device_id))
    }

    /// `POST /devices`, with the browser's token.
    fn register(&self, r: &Request) -> Reply {
        let Some(token) = r.bearer().map(String::from) else { return signed_out() };
        let body = match r.json() {
            Ok(v) => v,
            Err(reply) => return reply,
        };
        let mut s = self.state();
        let account = match s.browser_tokens.get(&token) {
            Some(t) if !t.used && t.expires_at >= now_ms() && t.scopes.iter().any(|x| x == "device:register") => t.account.clone(),
            _ => return signed_out(),
        };
        let text = |k: &str| body.get(k).and_then(Value::as_str).map(str::to_string);
        let Some(device_id) = text("device_id").map(|d| d.to_ascii_lowercase()).filter(|d| is_uuid(d)) else {
            return invalid("device_id must be a UUID.");
        };
        let Some(name) = text("name").filter(|n| (1..=255).contains(&n.chars().count())) else {
            return invalid("name must be 1 to 255 characters.");
        };
        let (Some(platform), Some(app_version)) = (text("platform"), text("app_version")) else {
            return invalid("platform and app_version are needed.");
        };
        if platform.chars().count() > 64 || app_version.chars().count() > 64 {
            return invalid("platform and app_version may be up to 64 characters.");
        }
        let (Some(sign_pub), Some(box_pub)) = (text("sign_pub").filter(|k| is_key(k)), text("box_pub").filter(|k| is_key(k))) else {
            return invalid("sign_pub and box_pub must be the base64 of 32 bytes.");
        };
        let kind = match body.get("kind") {
            None | Some(Value::Null) => "desktop".to_string(),
            Some(Value::String(k)) if k == "desktop" || k == "web" => k.clone(),
            _ => return invalid("kind must be \"desktop\" or \"web\"."),
        };
        if s.devices.iter().any(|d| d.id == device_id && d.account != account) {
            return Reply::error(409, "exists", "This computer is on another Hersir Cloud account.");
        }
        let now = now_ms();
        match s.devices.iter_mut().find(|d| d.id == device_id) {
            Some(d) => {
                d.name = name;
                d.platform = platform;
                d.app_version = app_version;
                d.sign_pub = sign_pub;
                d.box_pub = box_pub;
                d.kind = kind;
                d.revoked_at = None;
                d.lost_at = None;
                d.remote_on = false;
                d.last_seen_at = Some(now);
            }
            None => s.devices.push(DeviceState {
                id: device_id.clone(),
                account: account.clone(),
                name,
                platform,
                app_version,
                kind,
                remote_on: false,
                created_at: now,
                last_seen_at: Some(now),
                sign_pub,
                box_pub,
                revoked_at: None,
                lost_at: None,
            }),
        }
        for t in s.device_tokens.values_mut().filter(|t| t.device_id == device_id) {
            t.revoked = true;
        }
        let refresh = s.browser_tokens.get_mut(&token).and_then(|t| {
            t.used = true;
            t.refresh.take()
        });
        if let Some(refresh) = refresh {
            s.refresh_tokens.remove(&refresh);
        }
        let device_token = token_for(&mut s, &account, &device_id);
        let expires_at = s.device_tokens[&device_token].expires_at;
        let email = s.account(&account).map(|a| a.email.clone()).unwrap_or_default();
        Reply::json(201, &json!({ "account": { "id": account, "email": email }, "device_token": device_token, "expires_at": expires_at }))
    }

    /// `POST /devices/me/token`.
    fn renew(&self, r: &Request, account: &str, device: &str) -> Reply {
        let mut s = self.state();
        if let Some(old) = r.bearer().and_then(|t| s.device_tokens.get_mut(t)) {
            old.revoked = true;
        }
        let device_token = token_for(&mut s, account, device);
        let expires_at = s.device_tokens[&device_token].expires_at;
        Reply::json(200, &json!({ "device_token": device_token, "expires_at": expires_at }))
    }

    /// `PUT /devices/me`.
    fn update_device(&self, device: &str, body: &Value) -> Reply {
        let mut s = self.state();
        let Some(d) = s.devices.iter_mut().find(|d| d.id == device) else { return signed_out() };
        let mut changed = d.clone();
        match body.get("name") {
            None => {}
            Some(Value::String(n)) if (1..=255).contains(&n.chars().count()) => changed.name = n.clone(),
            _ => return invalid("name must be 1 to 255 characters."),
        }
        match body.get("app_version") {
            None => {}
            Some(Value::String(v)) if v.chars().count() <= 64 => changed.app_version = v.clone(),
            _ => return invalid("app_version may be up to 64 characters."),
        }
        match body.get("remote_on") {
            None => {}
            Some(Value::Bool(on)) => changed.remote_on = *on,
            _ => return invalid("remote_on must be true or false."),
        }
        *d = changed;
        Reply::of(200, &d.json(Some(device)))
    }

    /// `DELETE /devices/{id}`.
    fn remove(&self, account: &str, device: &str, id: &str, body: &Value) -> Reply {
        let lost = match body.get("lost") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(l)) => *l,
            Some(_) => return invalid("lost must be true or false."),
        };
        let id = if id == "me" { device.to_string() } else { id.to_ascii_lowercase() };
        let mut s = self.state();
        let Some(d) = s.devices.iter_mut().find(|d| d.id == id) else { return not_found("There is no such device.") };
        if d.account != account {
            return Reply::error(403, "not_yours", "This device is another account's.");
        }
        let now = now_ms();
        d.revoked_at.get_or_insert(now);
        if lost {
            d.lost_at.get_or_insert(now);
        }
        Reply::empty(204)
    }

    /// `GET /me/entitlements`: the payload in PROTOCOL.md's order, signed.
    fn entitlement(&self, account: &str) -> Reply {
        #[derive(Serialize)]
        struct Payload<'a> {
            v: i64,
            account: &'a str,
            plan: &'a str,
            status: &'a str,
            features: Vec<&'a str>,
            quota_bytes: i64,
            used_bytes: i64,
            valid_until: Option<i64>,
            read_only_until: Option<i64>,
            issued_at: i64,
        }
        let s = self.state();
        let Some(a) = s.account(account) else { return signed_out() };
        let both = vec!["backup", "remote"];
        let (plan, status, features, valid_until, read_only_until) = match &a.plan {
            Plan::Active { valid_until } => ("personal", "active", both, *valid_until, None),
            Plan::PastDue => ("personal", "past_due", both, None, None),
            Plan::Lapsed { valid_until, read_only_until } => ("personal", "lapsed", vec![], Some(*valid_until), Some(*read_only_until)),
            Plan::None => ("none", "none", vec![], None, None),
        };
        let payload = serde_json::to_string(&Payload {
            v: 1,
            account,
            plan,
            status,
            features,
            quota_bytes: s.quota,
            used_bytes: a.used(),
            valid_until,
            read_only_until,
            issued_at: now_ms(),
        })
        .unwrap_or_default();
        let sig = self.signing.sign(payload.as_bytes()).to_bytes();
        Reply::json(200, &json!({ "payload": payload, "sig": b64(&sig) }))
    }

    /// `PUT /keys`.
    fn put_key(&self, account: &str, body: &Value) -> Reply {
        let Some(key_id) = body.get("key_id").and_then(Value::as_str).filter(|k| !k.is_empty() && k.chars().count() <= 255) else {
            return invalid("key_id must be a string of up to 255 characters.");
        };
        let Some(epoch) = body.get("epoch").and_then(Value::as_u64).filter(|e| *e <= 255) else {
            return invalid("epoch must be an integer from 0 to 255.");
        };
        let Some(sealed) = body.get("sealed_by_code").and_then(Value::as_str).filter(|b| unb64(b).is_some_and(|k| !k.is_empty() && k.len() <= 8192)) else {
            return invalid("sealed_by_code must be base64 of at most 8,192 bytes.");
        };
        let if_epoch = match body.get("if_epoch") {
            None | Some(Value::Null) => None,
            Some(v) => match v.as_u64().filter(|e| *e <= 255) {
                Some(e) => Some(e as u8),
                None => return invalid("if_epoch must be null or an integer from 0 to 255."),
            },
        };
        let key = KeyRecord { key_id: key_id.to_string(), epoch: epoch as u8, sealed_by_code: sealed.to_string() };
        let mut s = self.state();
        let Some(a) = s.account_mut(account) else { return signed_out() };
        if a.key.as_ref() == Some(&key) {
            return Reply::of(200, &key);
        }
        if a.key.as_ref().map(|k| k.epoch) != if_epoch {
            return Reply::error(409, "epoch", "Another computer changed this account's key: get it again.");
        }
        let first = a.key.is_none();
        a.key = Some(key.clone());
        Reply::of(if first { 201 } else { 200 }, &key)
    }

    /// `POST /objects/missing`.
    fn missing(&self, account: &str, body: &Value) -> Reply {
        let ids = match id_list(body.get("ids")) {
            Ok(ids) => ids,
            Err(reply) => return reply,
        };
        let s = self.state();
        let Some(a) = s.account(account) else { return signed_out() };
        let mut out: Vec<String> = Vec::new();
        for id in ids {
            if !a.objects.get(&id).is_some_and(|o| o.stored) && !out.contains(&id) {
                out.push(id);
            }
        }
        Reply::json(200, &json!({ "missing": out }))
    }

    /// `POST /objects/upload`.
    fn upload(&self, account: &str, body: &Value) -> Reply {
        let Some(list) = body.get("objects").and_then(Value::as_array).filter(|l| l.len() <= 1000) else {
            return invalid("objects must be a list of at most 1,000.");
        };
        let mut asked: Vec<(String, i64)> = Vec::new();
        for o in list {
            let id = o.get("id").and_then(Value::as_str).map(str::to_ascii_lowercase).filter(|id| crate::util::is_hex(id, 64));
            let bytes = o.get("bytes").and_then(Value::as_i64).filter(|b| (1..=MAX_OBJECT as i64).contains(b));
            match (id, bytes) {
                (Some(id), Some(bytes)) if !asked.iter().any(|(x, _)| *x == id) => asked.push((id, bytes)),
                _ => return invalid("Each object needs an id of 64 hex characters, once, and bytes from 1 to 4,194,368."),
            }
        }
        let mut s = self.state();
        let quota = s.quota;
        let Some(a) = s.account(account) else { return signed_out() };
        if let Some(refused) = write_refused(&a.plan) {
            return refused;
        }
        let used = a.used();
        let adding: i64 = asked.iter().filter(|(id, _)| !a.objects.get(id).is_some_and(|o| o.stored)).map(|(_, b)| *b).sum();
        if used + adding > quota {
            return quota_full(used, quota);
        }
        let mut urls = Vec::new();
        for (id, bytes) in asked {
            let a = s.account_mut(account).expect("the account is there");
            if a.objects.get(&id).is_some_and(|o| o.stored) {
                continue;
            }
            a.objects.insert(id.clone(), ObjectState { size: bytes, stored: false });
            let t = token("put");
            let check = token("chk");
            let expires_at = now_ms() + 10 * MINUTE;
            s.presigned.insert(t.clone(), Presigned {
                method: "PUT",
                key: format!("o/{account}/{id}"),
                id: id.clone(),
                expires_at,
                header: Some(check.clone()),
            });
            urls.push(json!({
                "id": id,
                "url": format!("{}/r2/hersir-cloud-objects/o/{account}/{id}?X-Amz-Signature={t}", self.url),
                "headers": { UPLOAD_HEADER: check },
                "expires_at": expires_at,
            }));
        }
        Reply::json(200, &json!({ "urls": urls }))
    }

    /// `POST /objects/download`.
    fn download(&self, account: &str, body: &Value) -> Reply {
        let ids = match id_list(body.get("ids")) {
            Ok(ids) => ids,
            Err(reply) => return reply,
        };
        let mut s = self.state();
        let Some(a) = s.account(account) else { return signed_out() };
        if let Some(refused) = read_refused(&a.plan) {
            return refused;
        }
        let mut unique: Vec<String> = Vec::new();
        for id in ids {
            if !unique.contains(&id) {
                unique.push(id);
            }
        }
        let lacking: Vec<String> = unique.iter().filter(|id| !a.objects.get(*id).is_some_and(|o| o.stored)).cloned().collect();
        if !lacking.is_empty() {
            return Reply::error_ids(404, "not_found", "This account doesn't have some of these objects.", &lacking);
        }
        let mut urls = Vec::new();
        for id in unique {
            let t = token("get");
            let expires_at = now_ms() + HOUR;
            s.presigned.insert(t.clone(), Presigned { method: "GET", key: format!("o/{account}/{id}"), id: id.clone(), expires_at, header: None });
            urls.push(json!({
                "id": id,
                "url": format!("{}/r2/hersir-cloud-objects/o/{account}/{id}?X-Amz-Signature={t}", self.url),
                "expires_at": expires_at,
            }));
        }
        Reply::json(200, &json!({ "urls": urls }))
    }

    /// `POST /snapshots`.
    fn create_snapshot(&self, account: &str, body: &Value) -> Reply {
        let text = |k: &str| body.get(k).and_then(Value::as_str).map(str::to_string);
        let Some(name) = text("name").filter(|n| snapshot_name(n)) else {
            return invalid("name must be 1 to 128 letters, digits, dots, dashes and underscores, from a letter or digit.");
        };
        let Some(made_at) = body.get("made_at").and_then(Value::as_i64).filter(|t| *t <= now_ms() + DAY) else {
            return invalid("made_at must be a time in ms, at most a day ahead.");
        };
        let Some(device_id) = text("device_id").map(|d| d.to_ascii_lowercase()) else { return invalid("device_id is needed.") };
        let Some(reason) = text("reason").filter(|r| crate::backup::REASONS.contains(&r.as_str())) else {
            return invalid("reason must be daily, manual, quit or before-restore.");
        };
        let Some(schema_version) = body.get("schema_version").and_then(Value::as_i64).filter(|v| *v >= 0) else {
            return invalid("schema_version must be an integer from 0.");
        };
        let Some(app_version) = text("app_version").filter(|v| v.chars().count() <= 64) else {
            return invalid("app_version may be up to 64 characters.");
        };
        let Some(manifest_id) = text("manifest_id").map(|m| m.to_ascii_lowercase()).filter(|m| crate::util::is_hex(m, 64)) else {
            return invalid("manifest_id must be 64 hex characters.");
        };
        let Some(bytes) = body.get("bytes").and_then(Value::as_i64).filter(|b| *b >= 0) else {
            return invalid("bytes must be an integer from 0.");
        };
        let ids = match body.get("object_ids").and_then(Value::as_array).filter(|l| l.len() <= 100_000) {
            Some(list) => {
                let ids: Option<Vec<String>> = list.iter().map(|v| v.as_str().map(str::to_ascii_lowercase).filter(|id| crate::util::is_hex(id, 64))).collect();
                match ids {
                    Some(ids) => ids,
                    None => return invalid("object_ids must be ids of 64 hex characters."),
                }
            }
            None => return invalid("object_ids must be a list of at most 100,000."),
        };
        let mut s = self.state();
        let quota = s.quota;
        let Some(a) = s.account(account) else { return signed_out() };
        if let Some(refused) = write_refused(&a.plan) {
            return refused;
        }
        match s.devices.iter().find(|d| d.id == device_id) {
            None => return invalid("device_id isn't a device on Hersir Cloud."),
            Some(d) if d.account != account => return Reply::error(403, "not_yours", "That device is another account's."),
            Some(_) => {}
        }
        if let Some(same) = a.snapshots.iter().find(|x| x.name == name) {
            if same.manifest_id == manifest_id && same.device_id == device_id && same.made_at == made_at {
                let same = same.clone();
                return Reply::of(201, &same.json(&s.device_name(&same.device_id)));
            }
            return Reply::error(409, "exists", "This account has another backup with this name.");
        }
        let mut all: Vec<String> = Vec::new();
        for id in ids.into_iter().chain(std::iter::once(manifest_id.clone())) {
            if !all.contains(&id) {
                all.push(id);
            }
        }
        let (mut missing, mut too_big, mut becoming) = (Vec::new(), Vec::new(), Vec::new());
        for id in &all {
            match a.objects.get(id) {
                Some(o) if o.stored => {}
                Some(_) => match s.r2.get(&format!("o/{account}/{id}")) {
                    Some(bytes) if bytes.len() > MAX_OBJECT => too_big.push(id.clone()),
                    Some(bytes) => becoming.push((id.clone(), bytes.len() as i64)),
                    None => missing.push(id.clone()),
                },
                None => missing.push(id.clone()),
            }
        }
        if !missing.is_empty() {
            return Reply::error_ids(422, "missing_objects", "Some objects of this backup aren't uploaded: upload them, then register it again.", &missing);
        }
        if !too_big.is_empty() {
            return Reply::error_ids(422, "invalid", "Some objects are over 4,194,368 bytes.", &too_big);
        }
        let used = a.used();
        if used + becoming.iter().map(|(_, b)| b).sum::<i64>() > quota {
            return quota_full(used, quota);
        }
        let device_name = s.device_name(&device_id);
        let a = s.account_mut(account).expect("the account is there");
        for (id, size) in becoming {
            a.objects.insert(id, ObjectState { size, stored: true });
        }
        let snap = SnapshotState { name, made_at, device_id, reason, schema_version, app_version, bytes, manifest_id, objects: all, deleted_at: None };
        a.snapshots.push(snap.clone());
        Reply::of(201, &snap.json(&device_name))
    }

    // ---- R2 ----

    /// A presigned `PUT` or `GET`: `/r2/hersir-cloud-objects/o/<account>/<id>?X-Amz-Signature=<token>`.
    fn r2(&self, r: &Request, path: &str) -> Reply {
        let key = path.strip_prefix("hersir-cloud-objects/").unwrap_or(path).to_string();
        let t = r.query.get("X-Amz-Signature").cloned().unwrap_or_default();
        let mut s = self.state();
        let allowed = s.presigned.get(&t).filter(|p| p.method == r.method && p.key == key && p.expires_at >= now_ms());
        let Some(p) = allowed else {
            return Reply::storage(403, "AccessDenied", "Request has expired or its signature doesn't match.");
        };
        if let Some(check) = &p.header
            && r.header(UPLOAD_HEADER) != Some(check.as_str())
        {
            return Reply::storage(403, "SignatureDoesNotMatch", "The headers signed with the URL weren't sent.");
        }
        let id = p.id.clone();
        if r.method == "PUT" {
            if r.body.len() > MAX_OBJECT + 1024 {
                return Reply::storage(400, "EntityTooLarge", "Your proposed upload exceeds the maximum allowed size.");
            }
            if s.forget_uploads > 0 {
                s.forget_uploads -= 1;
            } else {
                s.r2.insert(key, r.body.clone());
                s.uploads.push(id);
            }
            return Reply::empty(200);
        }
        match s.r2.get(&key) {
            Some(bytes) => Reply { status: 200, headers: vec![("Content-Type".into(), "application/octet-stream".into())], body: bytes.clone() },
            None => Reply::storage(404, "NoSuchKey", "The specified key does not exist."),
        }
    }
}

/// A new device token for `device` (a year).
fn token_for(s: &mut State, account: &str, device: &str) -> String {
    let t = token("hcd");
    s.device_tokens.insert(t.clone(), DeviceTokenState {
        account: account.to_string(),
        device_id: device.to_string(),
        expires_at: now_ms() + 365 * DAY,
        revoked: false,
    });
    t
}

/// Uploads and new snapshots need an active (or past due) plan.
fn write_refused(plan: &Plan) -> Option<Reply> {
    match plan {
        Plan::Active { .. } | Plan::PastDue => None,
        Plan::Lapsed { .. } => Some(Reply::error(402, "read_only", "Your plan ended: backups can be restored, not made.")),
        Plan::None => Some(Reply::error(402, "plan_required", "Cloud Backup needs Hersir Personal.")),
    }
}

/// Lists, downloads and undeletes work until a lapsed plan's `read_only_until`.
fn read_refused(plan: &Plan) -> Option<Reply> {
    match plan {
        Plan::Lapsed { read_only_until, .. } if *read_only_until < now_ms() => {
            Some(Reply::error(402, "plan_required", "Your plan ended more than 60 days ago: its backups can't be read any more."))
        }
        _ => None,
    }
}

fn quota_full(used: i64, quota: i64) -> Reply {
    let gb = |b: i64| b as f64 / 1e9;
    Reply::error(413, "quota", &format!(
        "Hersir Cloud is full: {:.1} of {} GB. Remove old backups or files you no longer need.",
        gb(used),
        (gb(quota) * 10.0).round() / 10.0
    ))
}

/// A list of object ids (at most 1,000, each 64 hex), in lower case.
fn id_list(v: Option<&Value>) -> Result<Vec<String>, Reply> {
    let refused = || invalid("ids must be a list of at most 1,000 ids of 64 hex characters.");
    let list = v.and_then(Value::as_array).filter(|l| l.len() <= 1000).ok_or_else(refused)?;
    list.iter().map(|v| v.as_str().map(str::to_ascii_lowercase).filter(|id| crate::util::is_hex(id, 64)).ok_or_else(refused)).collect()
}

/// A random token with a prefix.
fn token(prefix: &str) -> String {
    format!("{prefix}_{}", hex(&random::<20>().unwrap_or_else(|_| [7u8; 20])))
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36
        && s.char_indices().all(|(i, c)| if matches!(i, 8 | 13 | 18 | 23) { c == '-' } else { c.is_ascii_hexdigit() })
}

/// Base64, with padding, of exactly 32 bytes.
fn is_key(s: &str) -> bool {
    s.len() == 44 && unb64(s).is_some_and(|k| k.len() == 32)
}

/// 1 to 128 letters, digits, dots, dashes and underscores, from a letter or digit.
fn snapshot_name(s: &str) -> bool {
    (1..=128).contains(&s.len())
        && s.starts_with(|c: char| c.is_ascii_alphanumeric())
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// `http://127.0.0.1:<port>/callback` or `http://[::1]:<port>/callback`.
fn loopback_callback(uri: &str) -> bool {
    let rest = uri.strip_prefix("http://127.0.0.1:").or_else(|| uri.strip_prefix("http://[::1]:"));
    rest.and_then(|r| r.split_once('/')).is_some_and(|(port, path)| port.parse::<u16>().is_ok() && path == "callback")
}

/// A query's parameters, decoded; the first of each name counts.
fn query_params(query: &str) -> HashMap<String, String> {
    let mut all = HashMap::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        all.entry(unpct(name)).or_insert_with(|| unpct(value));
    }
    all
}

/// Reads one request: its head, and its body by `Content-Length` or chunked. Every byte read goes into `raw`.
fn read_request(stream: &mut TcpStream, raw: &mut Vec<u8>) -> Option<Request> {
    let head_end = loop {
        if let Some(i) = find(raw, b"\r\n\r\n") {
            break i + 4;
        }
        if raw.len() > 64 * 1024 || !more(stream, raw) {
            return None;
        }
    };
    let head = String::from_utf8_lossy(&raw[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split_whitespace();
    let (method, target) = (first.next()?.to_ascii_uppercase(), first.next()?.to_string());
    let headers: Vec<(String, String)> = lines.filter_map(|l| l.split_once(':')).map(|(n, v)| (n.trim().to_string(), v.trim().to_string())).collect();
    let header = |name: &str| headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone());
    let body = if header("transfer-encoding").is_some_and(|t| t.to_ascii_lowercase().contains("chunked")) {
        let mut body = Vec::new();
        let mut at = head_end;
        loop {
            let line_end = loop {
                if let Some(i) = find(&raw[at..], b"\r\n") {
                    break at + i;
                }
                if raw.len() > REQUEST_LIMIT || !more(stream, raw) {
                    return None;
                }
            };
            let size_text = String::from_utf8_lossy(&raw[at..line_end]).into_owned();
            let size = usize::from_str_radix(size_text.split(';').next().unwrap_or_default().trim(), 16).ok()?;
            at = line_end + 2;
            if size == 0 {
                // the trailer, up to its blank line
                while find(&raw[at..], b"\r\n").is_none() {
                    if !more(stream, raw) {
                        break;
                    }
                }
                break;
            }
            while raw.len() < at + size + 2 {
                if raw.len() > REQUEST_LIMIT || !more(stream, raw) {
                    return None;
                }
            }
            body.extend_from_slice(&raw[at..at + size]);
            at += size + 2;
        }
        body
    } else {
        let len: usize = header("content-length").and_then(|l| l.parse().ok()).unwrap_or(0);
        if len > REQUEST_LIMIT {
            return None;
        }
        while raw.len() < head_end + len {
            if !more(stream, raw) {
                return None;
            }
        }
        raw[head_end..head_end + len].to_vec()
    };
    let (path, query) = target.split_once('?').unwrap_or((target.as_str(), ""));
    Some(Request { method, path: path.to_string(), query: query_params(query), headers, body })
}

/// Reads more of the request into `raw`; false when the connection ended or failed.
fn more(stream: &mut TcpStream, raw: &mut Vec<u8>) -> bool {
    let mut chunk = [0u8; 64 * 1024];
    match stream.read(&mut chunk) {
        Ok(0) | Err(_) => false,
        Ok(n) => {
            raw.extend_from_slice(&chunk[..n]);
            true
        }
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn write_reply(stream: &mut TcpStream, reply: &Reply) {
    let reason = match reply.status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        302 => "Found",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        413 => "Content Too Large",
        422 => "Unprocessable Content",
        426 => "Upgrade Required",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "Status",
    };
    let mut head = format!("HTTP/1.1 {} {reason}\r\n", reply.status);
    for (name, value) in &reply.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!("Content-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n", reply.body.len()));
    let _ = stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(&reply.body)).and_then(|()| stream.flush());
    let _ = stream.shutdown(std::net::Shutdown::Write);
}
