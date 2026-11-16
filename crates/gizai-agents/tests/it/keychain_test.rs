//! GA-249: Hersir keeps its secrets under the keychain service `hersir`, with `gizai` (before 0.17.0) behind it
//! (`secrets::WithOld`): a secret found only under `gizai` is used and saved under `hersir` too, the `gizai` entry stays
//! for an older Hersir after a downgrade, saving saves under `hersir`, and deleting deletes both. Here with keychains
//! in memory in the two services' places.
use std::sync::Mutex;

use gizai_agents::secrets::{self, Keychain, MemoryKeychain, WithOld};

fn both() -> WithOld<MemoryKeychain, MemoryKeychain> {
    WithOld::default()
}

#[test]
fn the_service_is_hersir_and_the_one_before_0_17_is_gizai() {
    assert_eq!(secrets::SERVICE, "hersir");
    assert_eq!(secrets::OLD_SERVICE, "gizai");
}

#[test]
fn a_secret_saved_by_0_16_under_gizai_is_found_and_then_also_under_hersir_while_the_gizai_one_stays() {
    let k = both();
    k.old.set("mcp/srv1/oauth", "token-from-0.16").unwrap();
    assert_eq!(k.new.get("mcp/srv1/oauth").unwrap(), None, "before: only under gizai");

    assert_eq!(k.get("mcp/srv1/oauth").unwrap().as_deref(), Some("token-from-0.16"), "found after the update");
    assert_eq!(k.new.get("mcp/srv1/oauth").unwrap().as_deref(), Some("token-from-0.16"), "and now under hersir too");
    assert_eq!(k.old.get("mcp/srv1/oauth").unwrap().as_deref(), Some("token-from-0.16"), "the gizai entry stays, for 0.16.0");

    // found again: from hersir now, the same value
    assert_eq!(k.get("mcp/srv1/oauth").unwrap().as_deref(), Some("token-from-0.16"));
}

#[test]
fn a_secret_under_hersir_wins_over_the_one_under_gizai_and_nothing_is_none() {
    let k = both();
    k.old.set("bitbucket/login", "old").unwrap();
    k.new.set("bitbucket/login", "new").unwrap();
    assert_eq!(k.get("bitbucket/login").unwrap().as_deref(), Some("new"));
    assert_eq!(k.old.get("bitbucket/login").unwrap().as_deref(), Some("old"), "the gizai one is left as it is");

    assert_eq!(k.get("mcp/none/oauth").unwrap(), None);
    assert_eq!((k.new.get("mcp/none/oauth").unwrap(), k.old.get("mcp/none/oauth").unwrap()), (None, None), "nothing written");
}

#[test]
fn saving_saves_under_hersir_only() {
    let k = both();
    k.set("mcp/srv2/env/API_KEY", "v1").unwrap();
    assert_eq!(k.new.get("mcp/srv2/env/API_KEY").unwrap().as_deref(), Some("v1"));
    assert_eq!(k.old.get("mcp/srv2/env/API_KEY").unwrap(), None, "nothing new goes under gizai");
    // a new value over one that came from gizai: hersir has the new one, the gizai one stays as it was
    k.old.set("mcp/srv3/oauth", "from-gizai").unwrap();
    k.set("mcp/srv3/oauth", "changed").unwrap();
    assert_eq!(k.get("mcp/srv3/oauth").unwrap().as_deref(), Some("changed"));
    assert_eq!(k.old.get("mcp/srv3/oauth").unwrap().as_deref(), Some("from-gizai"));
}

#[test]
fn deleting_a_secret_removes_it_under_both_services() {
    let k = both();
    k.old.set("mcp/srv4/oauth", "x").unwrap();
    k.new.set("mcp/srv4/oauth", "x").unwrap();
    k.delete("mcp/srv4/oauth").unwrap();
    assert_eq!((k.new.get("mcp/srv4/oauth").unwrap(), k.old.get("mcp/srv4/oauth").unwrap()), (None, None));
    assert_eq!(k.get("mcp/srv4/oauth").unwrap(), None, "and it doesn't come back from gizai");

    // only under gizai (never read since the update), or under neither: deleted, without an error
    k.old.set("mcp/srv5/oauth", "y").unwrap();
    k.delete("mcp/srv5/oauth").unwrap();
    assert_eq!(k.get("mcp/srv5/oauth").unwrap(), None);
    k.delete("mcp/never/oauth").unwrap();
}

/// A keychain whose writes (or deletes) fail, with what it holds in memory.
#[derive(Default)]
struct Failing {
    inner: MemoryKeychain,
    set_fails: bool,
    delete_fails: bool,
    deleted: Mutex<Vec<String>>,
}

impl Keychain for Failing {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        self.inner.get(key)
    }
    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        if self.set_fails { Err("The keychain is locked".into()) } else { self.inner.set(key, value) }
    }
    fn delete(&self, key: &str) -> Result<(), String> {
        self.deleted.lock().unwrap().push(key.into());
        if self.delete_fails { Err("The keychain is locked".into()) } else { self.inner.delete(key) }
    }
}

#[test]
fn when_saving_under_hersir_fails_the_secret_from_gizai_still_works() {
    let k = WithOld { new: Failing { set_fails: true, ..Default::default() }, old: MemoryKeychain::default() };
    k.old.set("mcp/srv6/oauth", "still-here").unwrap();
    assert_eq!(k.get("mcp/srv6/oauth").unwrap().as_deref(), Some("still-here"));
    assert_eq!(k.get("mcp/srv6/oauth").unwrap().as_deref(), Some("still-here"), "and again next time");
    assert_eq!(k.old.get("mcp/srv6/oauth").unwrap().as_deref(), Some("still-here"));
}

#[test]
fn a_delete_that_fails_under_one_service_still_deletes_under_the_other_and_says_so() {
    let k = WithOld { new: Failing { delete_fails: true, ..Default::default() }, old: MemoryKeychain::default() };
    k.old.set("mcp/srv7/oauth", "z").unwrap();
    assert!(k.delete("mcp/srv7/oauth").is_err(), "the failure is reported");
    assert_eq!(k.old.get("mcp/srv7/oauth").unwrap(), None, "the gizai one went anyway");
    assert_eq!(*k.new.deleted.lock().unwrap(), ["mcp/srv7/oauth"]);

    let k = WithOld { new: MemoryKeychain::default(), old: Failing { delete_fails: true, ..Default::default() } };
    k.new.set("mcp/srv8/oauth", "z").unwrap();
    assert!(k.delete("mcp/srv8/oauth").is_err());
    assert_eq!(k.new.get("mcp/srv8/oauth").unwrap(), None, "the hersir one went anyway");
}
