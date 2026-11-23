//! Tokens for the MCP socket. Mostly short-lived ones: one per chat turn, so the tools know which agent acts and nothing
//! else on the machine can borrow them; the token itself lives in the turn's 0600 MCP config file and Claude Code's
//! environment, and dies with the turn (revoked or expired). GA-338: a lasting one for Claude Desktop (`mint_lasting`),
//! with no expiry, until it is revoked. Only the sha256 is stored.
use rusqlite::OptionalExtension;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::db::Db;
use crate::{Error, Result, ids};

#[derive(Debug, Clone, PartialEq)]
pub struct TokenGrant {
    pub actor_id: String,
    pub scope: Value,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// sha256 of a string, as 64 hex characters.
pub fn sha256_hex(s: &str) -> String {
    hex(&Sha256::digest(s.as_bytes()))
}

fn hash(token: &str) -> String {
    sha256_hex(token)
}

/// 64 hex characters: 32 bytes from the system's random source.
fn random_token() -> Result<String> {
    let mut raw = [0u8; 32];
    getrandom::fill(&mut raw).map_err(|e| Error::Invalid(format!("Couldn't make a random token: {e}")))?;
    Ok(hex(&raw))
}

fn insert(c: &rusqlite::Connection, token: &str, actor_id: &str, scope: &Value, now: i64, expires_at: Option<i64>) -> Result<()> {
    c.execute(
        "INSERT INTO api_tokens(id, created_at, actor_id, token_sha256, scopes_json, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![ids::new_id(), now, actor_id, hash(token), scope.to_string(), expires_at],
    )?;
    Ok(())
}

/// A new random token (64 hex characters, 32 bytes from the system's random source) for `actor_id`, valid for `ttl_ms`.
pub fn mint(db: &Db, actor_id: &str, scope: Value, ttl_ms: i64) -> Result<String> {
    let token = random_token()?;
    let now = ids::now_ms();
    db.write(None, |w| insert(w.conn(), &token, actor_id, &scope, now, Some(now + ttl_ms)))?;
    Ok(token)
}

/// GA-338: a new lasting token for `actor_id`: no expiry (`expires_at` NULL), so `verify` takes it and `prune` keeps it
/// until it is revoked. The only valid token with exactly this `scope`: the earlier ones are revoked in the same write,
/// first. Only its sha256 is stored, like every token's: the caller gets the token once, here.
pub fn mint_lasting(db: &Db, actor_id: &str, scope: Value) -> Result<String> {
    let token = random_token()?;
    let now = ids::now_ms();
    db.write(None, |w| {
        revoke_all(w.conn(), &scope, now)?;
        insert(w.conn(), &token, actor_id, &scope, now, None)
    })?;
    Ok(token)
}

/// When the valid token with exactly `scope` was made (ms), the newest if there are more; None when there is none.
pub fn made_at(db: &Db, scope: &Value) -> Result<Option<i64>> {
    let now = ids::now_ms();
    db.read(|c| {
        Ok(c.query_row(
            "SELECT max(created_at) FROM api_tokens WHERE scopes_json=?1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > ?2)",
            rusqlite::params![scope.to_string(), now], |r| r.get(0))?)
    })
}

fn revoke_all(c: &rusqlite::Connection, scope: &Value, now: i64) -> Result<usize> {
    Ok(c.execute("UPDATE api_tokens SET revoked_at=?2 WHERE scopes_json=?1 AND revoked_at IS NULL", rusqlite::params![scope.to_string(), now])?)
}

/// GA-338: revokes every token with exactly `scope` that isn't revoked yet. Returns how many.
pub fn revoke_scope(db: &Db, scope: &Value) -> Result<usize> {
    db.write(None, |w| revoke_all(w.conn(), scope, ids::now_ms()))
}

/// Who the token speaks for, if it exists, isn't revoked and hasn't expired.
pub fn verify(db: &Db, token: &str) -> Result<Option<TokenGrant>> {
    let now = ids::now_ms();
    db.read(|c| {
        let row: Option<(String, String)> = c.query_row(
            "SELECT actor_id, scopes_json FROM api_tokens WHERE token_sha256=?1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > ?2)",
            rusqlite::params![hash(token), now], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        Ok(row.map(|(actor_id, scope)| TokenGrant { actor_id, scope: serde_json::from_str(&scope).unwrap_or(Value::Null) }))
    })
}

pub fn revoke(db: &Db, token: &str) -> Result<()> {
    db.write(None, |w| {
        w.conn().execute("UPDATE api_tokens SET revoked_at=?2 WHERE token_sha256=?1 AND revoked_at IS NULL", rusqlite::params![hash(token), ids::now_ms()])?;
        Ok(())
    })
}

/// A token that expired or was revoked stays this long, to look into a turn that went wrong; then `prune` removes it.
pub const KEEP_DEAD_MS: i64 = 24 * 60 * 60 * 1000;

/// Removes the tokens that expired or were revoked more than `KEEP_DEAD_MS` before `now_ms`: nothing can use them any
/// more. Valid tokens and recently dead ones stay; a lasting one (`mint_lasting`) never expires, so it stays until it is
/// revoked. Returns how many went.
pub fn prune(db: &Db, now_ms: i64) -> Result<usize> {
    db.write(None, |w| {
        Ok(w.conn().execute("DELETE FROM api_tokens WHERE revoked_at < ?1 OR expires_at < ?1", [now_ms - KEEP_DEAD_MS])?)
    })
}
