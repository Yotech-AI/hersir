// GA-338 QA: the lasting token for Claude Desktop (`tokens::mint_lasting`). It has no expiry, so `verify` takes it and
// `prune` keeps it until it is revoked; a new one with the same scope revokes the earlier ones in the same write, and
// tokens with another scope are left alone. Only the sha256 is stored.
use gizai_core::db::Db;
use gizai_core::{ids, seed, tokens};
use serde_json::{Value, json};

const HOUR: i64 = 60 * 60 * 1000;

fn desktop() -> Value {
    json!({"client": "claude-desktop"})
}

/// The api_tokens row with this token's sha256: (actor_id, scopes_json, expires_at, revoked_at).
fn row(db: &Db, token: &str) -> Option<(String, String, Option<i64>, Option<i64>)> {
    db.read(|c| {
        let mut q = c.prepare("SELECT actor_id, scopes_json, expires_at, revoked_at FROM api_tokens WHERE token_sha256=?1")?;
        let mut rows = q.query_map([tokens::sha256_hex(token)], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        Ok(rows.next().transpose()?)
    }).unwrap()
}

fn count(db: &Db) -> i64 {
    db.read(|c| Ok(c.query_row("SELECT count(*) FROM api_tokens", [], |r| r.get(0))?)).unwrap()
}

/// Every text in api_tokens, joined: to look for the token itself.
fn all_text(db: &Db) -> String {
    db.read(|c| {
        let mut q = c.prepare("SELECT id || ' ' || actor_id || ' ' || token_sha256 || ' ' || scopes_json FROM api_tokens")?;
        let rows = q.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows.join("\n"))
    }).unwrap()
}

#[test]
fn a_lasting_token_has_no_expiry_is_stored_only_as_its_sha256_and_speaks_for_its_actor() {
    let db = Db::open_in_memory().unwrap();
    let you = seed::ensure_seed(&db, "Jeffrey").unwrap().you_id;
    let before = ids::now_ms();
    let token = tokens::mint_lasting(&db, &you, desktop()).unwrap();
    assert_eq!(token.len(), 64, "{token}");
    assert!(token.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "{token}");
    let (actor, scope, expires, revoked) = row(&db, &token).expect("a row with the token's sha256");
    assert_eq!(actor, you);
    assert_eq!(scope, r#"{"client":"claude-desktop"}"#);
    assert_eq!(expires, None, "no expiry");
    assert_eq!(revoked, None);
    assert!(!all_text(&db).contains(&token), "only the sha256 is stored");
    let grant = tokens::verify(&db, &token).unwrap().expect("valid");
    assert_eq!(grant.actor_id, you);
    assert_eq!(grant.scope, desktop());
    let made = tokens::made_at(&db, &desktop()).unwrap().expect("made");
    assert!(made >= before && made <= ids::now_ms(), "{made}");
    // two lasting tokens are never the same
    assert_ne!(tokens::mint_lasting(&db, &you, desktop()).unwrap(), token);
}

#[test]
fn a_new_lasting_token_revokes_the_earlier_one_with_its_scope_and_leaves_every_other_token_alone() {
    let db = Db::open_in_memory().unwrap();
    let you = seed::ensure_seed(&db, "Jeffrey").unwrap().you_id;
    let chat = tokens::mint(&db, &you, json!({"chat": "T1"}), HOUR).unwrap();
    let empty = tokens::mint(&db, &you, json!({}), HOUR).unwrap();
    let other_client = tokens::mint_lasting(&db, &you, json!({"client": "something-else"})).unwrap();
    let first = tokens::mint_lasting(&db, &you, desktop()).unwrap();
    let second = tokens::mint_lasting(&db, &you, desktop()).unwrap();
    assert!(tokens::verify(&db, &first).unwrap().is_none(), "the earlier one is revoked");
    assert!(row(&db, &first).unwrap().3.is_some(), "revoked, not deleted");
    assert!(tokens::verify(&db, &second).unwrap().is_some(), "the new one is valid");
    for (what, t) in [("a chat turn's", &chat), ("a token with {}", &empty), ("another client's", &other_client)] {
        assert!(tokens::verify(&db, t).unwrap().is_some(), "{what} token stays valid");
    }
    let valid: i64 = db.read(|c| Ok(c.query_row(
        "SELECT count(*) FROM api_tokens WHERE scopes_json=?1 AND revoked_at IS NULL", [desktop().to_string()], |r| r.get(0))?)).unwrap();
    assert_eq!(valid, 1, "only one Claude Desktop token is valid at a time");
    // made_at gives the valid one's time
    let (made, second_made): (Option<i64>, i64) = (tokens::made_at(&db, &desktop()).unwrap(), db.read(|c| Ok(c.query_row(
        "SELECT created_at FROM api_tokens WHERE token_sha256=?1", [tokens::sha256_hex(&second)], |r| r.get(0))?)).unwrap());
    assert_eq!(made, Some(second_made));
}

#[test]
fn revoke_scope_ends_every_valid_token_with_that_scope_and_made_at_is_then_none() {
    let db = Db::open_in_memory().unwrap();
    let you = seed::ensure_seed(&db, "Jeffrey").unwrap().you_id;
    assert_eq!(tokens::made_at(&db, &desktop()).unwrap(), None, "none yet");
    assert_eq!(tokens::revoke_scope(&db, &desktop()).unwrap(), 0, "nothing to revoke");
    let chat = tokens::mint(&db, &you, json!({"chat": "T1"}), HOUR).unwrap();
    let token = tokens::mint_lasting(&db, &you, desktop()).unwrap();
    assert_eq!(tokens::revoke_scope(&db, &desktop()).unwrap(), 1);
    assert!(tokens::verify(&db, &token).unwrap().is_none());
    assert_eq!(tokens::made_at(&db, &desktop()).unwrap(), None);
    assert!(tokens::verify(&db, &chat).unwrap().is_some(), "a chat turn's token isn't touched");
    assert_eq!(tokens::revoke_scope(&db, &desktop()).unwrap(), 0, "an already revoked one isn't counted again");
}

#[test]
fn prune_keeps_a_valid_lasting_token_for_ever_and_removes_a_revoked_one_after_keep_dead_ms() {
    let db = Db::open_in_memory().unwrap();
    let you = seed::ensure_seed(&db, "Jeffrey").unwrap().you_id;
    let now = ids::now_ms();
    let kept = tokens::mint_lasting(&db, &you, json!({"client": "kept"})).unwrap();
    let revoked = tokens::mint_lasting(&db, &you, desktop()).unwrap();
    tokens::revoke_scope(&db, &desktop()).unwrap();
    assert_eq!(count(&db), 2);
    // within KEEP_DEAD_MS the revoked one stays, to look into
    assert_eq!(tokens::prune(&db, now + tokens::KEEP_DEAD_MS - HOUR).unwrap(), 0);
    assert!(row(&db, &revoked).is_some());
    // after it, the revoked one goes; the valid lasting one stays, also years on
    assert_eq!(tokens::prune(&db, now + tokens::KEEP_DEAD_MS + HOUR).unwrap(), 1);
    assert!(row(&db, &revoked).is_none());
    assert_eq!(tokens::prune(&db, now + 10 * 365 * 24 * HOUR).unwrap(), 0);
    assert!(tokens::verify(&db, &kept).unwrap().is_some(), "a valid lasting token stays until it is revoked");
    assert_eq!(count(&db), 1);
}
