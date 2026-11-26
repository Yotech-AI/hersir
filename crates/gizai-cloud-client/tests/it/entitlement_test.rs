//! The entitlement (PROTOCOL.md, *Checking it*): `sig` checked over the UTF-8 bytes of `payload` with Hersir Cloud's
//! public key, and only then the payload read; a changed payload, a wrong signature, a `v` other than 1 and another
//! account are refused.

use ed25519_dalek::{Signer, SigningKey};
use gizai_cloud_client::Error;
use gizai_cloud_client::api::SignedEntitlement;
use gizai_cloud_client::entitlement::{self, Entitlement};
use gizai_cloud_client::fake::{Plan, QUOTA};

use crate::common::cloud;

fn b64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn refused(r: gizai_cloud_client::Result<Entitlement>) -> String {
    match r {
        Err(Error::Entitlement(m)) => m,
        other => panic!("refused as Error::Entitlement, not {other:?}"),
    }
}

#[test]
fn a_good_entitlement_from_the_server_passes_and_reads_as_the_plan() {
    let c = cloud();
    let signed = c.api.entitlement().unwrap();
    let key = entitlement::public_key(&c.fake.entitlement_key_b64()).unwrap();
    assert_eq!(key, c.fake.entitlement_key());
    let e = entitlement::check(&signed, &key, &c.account_id()).unwrap();
    assert_eq!(e.v, 1);
    assert_eq!(e.account, c.account_id());
    assert_eq!(e.plan, "personal");
    assert_eq!(e.status, "active");
    assert_eq!(e.features, ["backup", "remote"]);
    assert!(e.has("backup") && e.has("remote") && !e.has("sync"));
    assert_eq!(e.quota_bytes, QUOTA);
    assert_eq!(e.valid_until, None);
    assert_eq!(e.read_only_until, None);
    assert!(e.issued_at > 0);
}

#[test]
fn a_lapsed_plan_and_no_plan_read_as_the_server_signed_them() {
    let c = cloud();
    let key = c.fake.entitlement_key();
    c.fake.set_plan(&c.account_id(), Plan::Lapsed { valid_until: 1_000, read_only_until: 4_102_444_800_000 });
    let e = entitlement::check(&c.api.entitlement().unwrap(), &key, &c.account_id()).unwrap();
    assert_eq!((e.status.as_str(), e.features.len(), e.valid_until, e.read_only_until), ("lapsed", 0, Some(1_000), Some(4_102_444_800_000)));
    assert!(!e.has("backup"));
    c.fake.set_plan(&c.account_id(), Plan::None);
    let e = entitlement::check(&c.api.entitlement().unwrap(), &key, &c.account_id()).unwrap();
    assert_eq!((e.plan.as_str(), e.status.as_str()), ("none", "none"));
}

#[test]
fn a_changed_payload_is_refused() {
    let c = cloud();
    let signed = c.api.entitlement().unwrap();
    let key = c.fake.entitlement_key();
    // a bigger quota, a status, and one byte of white space
    for changed in [
        signed.payload.replace("10000000000", "90000000000"),
        signed.payload.replace("\"active\"", "\"Active\""),
        format!("{} ", signed.payload),
        signed.payload.replacen('{', "{ ", 1),
    ] {
        assert_ne!(changed, signed.payload);
        let m = refused(entitlement::check(&SignedEntitlement { payload: changed, sig: signed.sig.clone() }, &key, &c.account_id()));
        assert!(m.contains("signature"), "{m}");
    }
}

#[test]
fn a_wrong_signature_is_refused() {
    let c = cloud();
    let signed = c.api.entitlement().unwrap();
    let key = c.fake.entitlement_key();
    let account = c.account_id();
    // signed by another key
    let other = SigningKey::from_bytes(&[9u8; 32]);
    let forged = SignedEntitlement { payload: signed.payload.clone(), sig: b64(&other.sign(signed.payload.as_bytes()).to_bytes()) };
    refused(entitlement::check(&forged, &key, &account));
    // checked with another public key
    refused(entitlement::check(&signed, &other.verifying_key().to_bytes(), &account));
    // one bit of the signature changed
    use base64::Engine;
    let mut sig = base64::engine::general_purpose::STANDARD.decode(&signed.sig).unwrap();
    sig[10] ^= 4;
    refused(entitlement::check(&SignedEntitlement { payload: signed.payload.clone(), sig: b64(&sig) }, &key, &account));
    // no signature, one too short, one that isn't base64
    for sig in [String::new(), b64(&sig[..63]), "not base64!".to_string()] {
        refused(entitlement::check(&SignedEntitlement { payload: signed.payload.clone(), sig }, &key, &account));
    }
}

#[test]
fn another_accounts_entitlement_is_refused() {
    let c = cloud();
    let key = c.fake.entitlement_key();
    // this account's, checked as another's
    let signed = c.api.entitlement().unwrap();
    let m = refused(entitlement::check(&signed, &key, "2"));
    assert!(m.contains("another account"), "{m}");
    // another account's, signed by the server
    let other = c.fake.add_account("other@example.com");
    let payload = signed.payload.replace(&format!("\"account\":\"{}\"", c.account_id()), &format!("\"account\":\"{}\"", other.id));
    assert_ne!(payload, signed.payload);
    let m = refused(entitlement::check(&c.fake.sign_entitlement(&payload), &key, &c.account_id()));
    assert!(m.contains("another account"), "{m}");
}

#[test]
fn a_payload_of_another_version_is_refused() {
    let c = cloud();
    let key = c.fake.entitlement_key();
    let signed = c.api.entitlement().unwrap();
    assert!(signed.payload.starts_with("{\"v\":1,"), "{}", signed.payload);
    for payload in [signed.payload.replacen("\"v\":1", "\"v\":2", 1), signed.payload.replacen("\"v\":1", "\"v\":\"1\"", 1), signed.payload.replacen("\"v\":1,", "", 1)] {
        let m = refused(entitlement::check(&c.fake.sign_entitlement(&payload), &key, &c.account_id()));
        assert!(m.contains("version"), "{payload}: {m}");
    }
}

#[test]
fn the_signature_is_checked_before_the_payload_is_read() {
    let c = cloud();
    let key = c.fake.entitlement_key();
    // not JSON, unsigned: refused for its signature, not for its JSON
    let m = refused(entitlement::check(&SignedEntitlement { payload: "not json".into(), sig: b64(&[0u8; 64]) }, &key, "1"));
    assert!(m.contains("signature"), "{m}");
    // not JSON, signed: refused as unreadable
    let m = refused(entitlement::check(&c.fake.sign_entitlement("not json"), &key, "1"));
    assert!(m.contains("can't be read"), "{m}");
}

#[test]
fn the_public_key_is_the_base64_of_32_bytes() {
    assert!(matches!(entitlement::public_key("AAAA"), Err(Error::Local(_))));
    assert!(matches!(entitlement::public_key("not base64"), Err(Error::Local(_))));
    assert_eq!(entitlement::public_key(&b64(&[1u8; 32])).unwrap(), [1u8; 32]);
}
