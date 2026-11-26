//! The entitlement: what the account's plan allows, signed by Hersir Cloud (PROTOCOL.md, *The entitlement* and
//! *Checking it*). The server enforces the plan; Hersir only shows it, and checks the signature first:
//!
//! 1. take `payload` exactly as received, a string, and its UTF-8 bytes;
//! 2. check `sig` (base64 of a 64-byte Ed25519 signature) over those bytes with Hersir Cloud's public key: one changed
//!    byte fails;
//! 3. only then parse `payload` as JSON, and check `v` is 1 and `account` is the signed-in account.
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::SignedEntitlement;
use crate::util::{clean, unb64};
use crate::{Error, Result};

/// The payload's version this Hersir reads.
pub const VERSION: i64 = 1;

/// The plan, from a checked entitlement. The fields in the payload's order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entitlement {
    /// 1.
    pub v: i64,
    /// The account's id, a string.
    pub account: String,
    /// `"personal"` or `"none"`.
    pub plan: String,
    /// `"active"`, `"past_due"`, `"lapsed"` or `"none"`.
    pub status: String,
    /// `["backup","remote"]` while the plan is active or past due, otherwise empty.
    pub features: Vec<String>,
    /// The plan's storage: 10,000,000,000 bytes.
    pub quota_bytes: i64,
    /// The account's Cloud Backup use.
    pub used_bytes: i64,
    /// When the plan ends (ms), or None when it has no end.
    pub valid_until: Option<i64>,
    /// A lapsed plan's last moment of reading its backups (ms); None for every other plan.
    pub read_only_until: Option<i64>,
    /// When this entitlement was made (ms).
    pub issued_at: i64,
}

impl Entitlement {
    /// Whether the plan has `feature` (`"backup"`, `"remote"`).
    pub fn has(&self, feature: &str) -> bool {
        self.features.iter().any(|f| f == feature)
    }
}

/// Hersir Cloud's public key from its base64 (32 bytes), as `php artisan hersir:entitlement-key` shows it.
pub fn public_key(b64: &str) -> Result<[u8; 32]> {
    unb64(b64).and_then(|k| <[u8; 32]>::try_from(k.as_slice()).ok())
        .ok_or_else(|| Error::Local("Hersir Cloud's entitlement key isn't the base64 of 32 bytes".to_string()))
}

/// Checks `signed` with Hersir Cloud's public key `key` for `account` (the signed-in account's id), and reads it.
/// `Error::Entitlement` when the signature fails (a changed payload, a wrong signature or another key), when the payload
/// isn't version 1, or when it is another account's.
pub fn check(signed: &SignedEntitlement, key: &[u8; 32], account: &str) -> Result<Entitlement> {
    let refused = |why: String| Error::Entitlement(why);
    let key = VerifyingKey::from_bytes(key).map_err(|_| refused("Hersir Cloud's entitlement key isn't a valid Ed25519 key".to_string()))?;
    let sig = unb64(&signed.sig).and_then(|s| <[u8; 64]>::try_from(s.as_slice()).ok())
        .ok_or_else(|| refused("The plan from Hersir Cloud has no valid signature".to_string()))?;
    key.verify_strict(signed.payload.as_bytes(), &Signature::from_bytes(&sig))
        .map_err(|_| refused("The plan from Hersir Cloud isn't signed by Hersir Cloud: its signature doesn't match".to_string()))?;
    let doc: Value = serde_json::from_str(&signed.payload).map_err(|e| refused(format!("The plan from Hersir Cloud can't be read: {e}")))?;
    let v = doc.get("v").and_then(Value::as_i64);
    if v != Some(VERSION) {
        let shown = doc.get("v").map(|v| clean(&v.to_string())).unwrap_or_else(|| "none".to_string());
        return Err(refused(format!("The plan from Hersir Cloud is version {shown}, which this Hersir can't read: update Hersir")));
    }
    let theirs = doc.get("account").and_then(Value::as_str).unwrap_or_default();
    if theirs != account {
        return Err(refused("The plan from Hersir Cloud is another account's".to_string()));
    }
    serde_json::from_value(doc).map_err(|e| refused(format!("The plan from Hersir Cloud can't be read: {e}")))
}
