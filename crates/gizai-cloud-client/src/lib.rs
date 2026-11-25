//! Hersir Cloud's client (GA-358): what a computer says to Hersir Cloud (the server's PROTOCOL.md), and Cloud Backup on
//! top of it. No Tauri: the app's side (sign-in, keychain, commands, schedule) builds on this.
//!
//! - `api`: the server's calls a computer makes (devices, the entitlement, keys, objects, snapshots) and the presigned
//!   PUT and GET to R2, each error of the server typed (`Error`).
//! - `keys`: the account key, the keys derived from it, the recovery code and the account key sealed by it, and this
//!   computer's own keys.
//! - `chunks`: a backup's chunks, their keyed ids and their sealed objects.
//! - `entitlement`: the plan, checked against Hersir Cloud's signature before it is read.
//! - `backup` and `restore`: a snapshot to Hersir Cloud, and back into a format-1 folder (docs/backup-format.md) that
//!   `gizai_core::restore` takes as it is.
//! - `fake`: a fake Hersir Cloud on 127.0.0.1, in memory, for tests and local runs (`cargo run -p gizai-cloud-client
//!   --example fake-cloud`).
//!
//! The formats are in docs/backup-format.md, *For Hersir Cloud*. Keys, tokens, recovery codes and presigned URLs never
//! appear in errors or Debug output. Everything here blocks: call it off the main thread.
use std::time::Duration;

pub mod api;
pub mod backup;
pub mod chunks;
pub mod entitlement;
pub mod fake;
pub mod keys;
pub mod restore;
mod util;

pub use api::Api;

/// What went wrong: an error Hersir Cloud answered (one per code of PROTOCOL.md's table, with its `message`), or one
/// on the way or on this computer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// 401 `signed_out`: no token, or one that is unknown, expired, revoked or of the wrong kind. Sign in again.
    #[error("{0}")]
    SignedOut(String),
    /// 401 `removed`: this computer was removed (on the account page or from another computer), or signed out.
    #[error("{0}")]
    Removed(String),
    /// 402 `plan_required`: the call needs a plan the account doesn't have.
    #[error("{0}")]
    PlanRequired(String),
    /// 402 `read_only`: the plan lapsed; backups can be listed, downloaded and undeleted, not written.
    #[error("{0}")]
    ReadOnly(String),
    /// 403 `not_yours`: the thing belongs to another account.
    #[error("{0}")]
    NotYours(String),
    /// 404 `not_found`: no such thing; `ids` lists the objects the account doesn't have (`POST /objects/download`).
    #[error("{message}")]
    NotFound { message: String, ids: Vec<String> },
    /// 409 `claimed` (not used any more).
    #[error("{0}")]
    Claimed(String),
    /// 409 `exists`: the device id is on another account, or the account has another backup with this name.
    #[error("{0}")]
    Exists(String),
    /// 409 `epoch`: `PUT /keys` from an epoch that isn't the stored key's. Get the key again.
    #[error("{0}")]
    Epoch(String),
    /// 409 `no_subscription`: the device has no push subscription.
    #[error("{0}")]
    NoSubscription(String),
    /// 413 `quota`: over the 10 GB of Cloud Backup.
    #[error("{0}")]
    Quota(String),
    /// 413 `file_too_large`: a relay blob over 100 MB.
    #[error("{0}")]
    FileTooLarge(String),
    /// 422 `invalid`: the request wasn't as the protocol describes it; `ids` lists objects over 4,194,368 bytes.
    #[error("{message}")]
    Invalid { message: String, ids: Vec<String> },
    /// 422 `missing_objects`: `POST /snapshots` named objects the account doesn't have; `ids` lists them.
    #[error("{message}")]
    MissingObjects { message: String, ids: Vec<String> },
    /// 426 `update_hersir`: this Hersir is too old for the server.
    #[error("{0}")]
    UpdateHersir(String),
    /// 429 `slow_down`: too many calls; `retry_after` is the server's `Retry-After`, when it gave one.
    #[error("{message}")]
    SlowDown { message: String, retry_after: Option<Duration> },
    /// Any other answer of the server: a server error (5xx), or an error code this Hersir doesn't know.
    #[error("{message}")]
    Server { status: u16, code: Option<String>, message: String },
    /// The server or R2 couldn't be reached, or its answer couldn't be read.
    #[error("{0}")]
    Network(String),
    /// R2 refused a presigned upload or download.
    #[error("{0}")]
    Storage(String),
    /// The text typed isn't a recovery code (its length, or a character a recovery code doesn't have).
    #[error("{0}")]
    NotARecoveryCode(String),
    /// The recovery code's check character doesn't match: a character was typed wrong.
    #[error("That recovery code has a typing mistake: check each character and try again.")]
    RecoveryCodeCheck,
    /// The recovery code doesn't open the account's key (the key id doesn't match).
    #[error("That recovery code doesn't open this account's backups.")]
    WrongRecoveryCode,
    /// The entitlement isn't Hersir Cloud's (its signature fails), isn't version 1, or is another account's.
    #[error("{0}")]
    Entitlement(String),
    /// An object, the manifest or a key from Hersir Cloud doesn't open or doesn't check: changed, damaged, or made with
    /// another key.
    #[error("{0}")]
    Damaged(String),
    /// Something on this computer: a file that can't be read or written, a backup that can't be made as asked.
    #[error("{0}")]
    Local(String),
    /// The progress callback stopped the backup or restore.
    #[error("Stopped")]
    Cancelled,
}

impl Error {
    /// The server's error code (`signed_out`, `quota`, …) for an error the server answered with one.
    pub fn code(&self) -> Option<&str> {
        Some(match self {
            Error::SignedOut(_) => "signed_out",
            Error::Removed(_) => "removed",
            Error::PlanRequired(_) => "plan_required",
            Error::ReadOnly(_) => "read_only",
            Error::NotYours(_) => "not_yours",
            Error::NotFound { .. } => "not_found",
            Error::Claimed(_) => "claimed",
            Error::Exists(_) => "exists",
            Error::Epoch(_) => "epoch",
            Error::NoSubscription(_) => "no_subscription",
            Error::Quota(_) => "quota",
            Error::FileTooLarge(_) => "file_too_large",
            Error::Invalid { .. } => "invalid",
            Error::MissingObjects { .. } => "missing_objects",
            Error::UpdateHersir(_) => "update_hersir",
            Error::SlowDown { .. } => "slow_down",
            Error::Server { code, .. } => return code.as_deref(),
            _ => return None,
        })
    }

    /// The HTTP status the server answered with, for an error the server answered.
    pub fn status(&self) -> Option<u16> {
        Some(match self {
            Error::SignedOut(_) | Error::Removed(_) => 401,
            Error::PlanRequired(_) | Error::ReadOnly(_) => 402,
            Error::NotYours(_) => 403,
            Error::NotFound { .. } => 404,
            Error::Claimed(_) | Error::Exists(_) | Error::Epoch(_) | Error::NoSubscription(_) => 409,
            Error::Quota(_) | Error::FileTooLarge(_) => 413,
            Error::Invalid { .. } | Error::MissingObjects { .. } => 422,
            Error::UpdateHersir(_) => 426,
            Error::SlowDown { .. } => 429,
            Error::Server { status, .. } => *status,
            _ => return None,
        })
    }

    /// The ids an error names: the objects of `missing_objects`, `not_found` and `invalid`.
    pub fn ids(&self) -> &[String] {
        match self {
            Error::NotFound { ids, .. } | Error::Invalid { ids, .. } | Error::MissingObjects { ids, .. } => ids,
            _ => &[],
        }
    }

    /// How long to wait before trying again: a 429's `Retry-After`.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Error::SlowDown { retry_after, .. } => *retry_after,
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Local(e.to_string())
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
