//! Each error of PROTOCOL.md's table comes back typed, with the server's `message` (and its `ids`, and 429's wait):
//! first as the fake answers it on its own, then each code as `fail_next` makes any call answer it.

use std::time::Duration;

use gizai_cloud_client::api::{DeviceChanges, NewSnapshot};
use gizai_cloud_client::fake::{Failure, Plan};
use gizai_cloud_client::{Api, Error, keys};

use crate::common::{cloud, device_id, keyed, new_device};

fn id(n: u8) -> String {
    format!("{:064x}", n)
}

fn snapshot(device: &str, name: &str, manifest: &str, objects: Vec<String>) -> NewSnapshot {
    NewSnapshot {
        name: name.into(),
        made_at: 1_795_633_200_000,
        device_id: device.into(),
        reason: "manual".into(),
        schema_version: 7,
        app_version: "0.20.1".into(),
        manifest_id: manifest.into(),
        object_ids: objects,
        bytes: 10,
    }
}

/// Uploads `bytes` as object `id` through the API and R2, as a backup does.
fn upload(api: &Api, id: &str, bytes: &[u8]) {
    let urls = api.upload_urls(&[(id.to_string(), bytes.len() as u64)]).unwrap();
    assert_eq!(urls.len(), 1);
    api.put_object(&urls[0], bytes).unwrap();
}

#[test]
fn each_code_of_the_table_comes_back_as_its_own_error_with_its_message() {
    let c = cloud();
    let table: [(u16, &str, fn(&Error) -> bool); 16] = [
        (401, "signed_out", |e| matches!(e, Error::SignedOut(_))),
        (401, "removed", |e| matches!(e, Error::Removed(_))),
        (402, "plan_required", |e| matches!(e, Error::PlanRequired(_))),
        (402, "read_only", |e| matches!(e, Error::ReadOnly(_))),
        (403, "not_yours", |e| matches!(e, Error::NotYours(_))),
        (404, "not_found", |e| matches!(e, Error::NotFound { .. })),
        (409, "claimed", |e| matches!(e, Error::Claimed(_))),
        (409, "exists", |e| matches!(e, Error::Exists(_))),
        (409, "epoch", |e| matches!(e, Error::Epoch(_))),
        (409, "no_subscription", |e| matches!(e, Error::NoSubscription(_))),
        (413, "quota", |e| matches!(e, Error::Quota(_))),
        (413, "file_too_large", |e| matches!(e, Error::FileTooLarge(_))),
        (422, "invalid", |e| matches!(e, Error::Invalid { .. })),
        (422, "missing_objects", |e| matches!(e, Error::MissingObjects { .. })),
        (426, "update_hersir", |e| matches!(e, Error::UpdateHersir(_))),
        (429, "slow_down", |e| matches!(e, Error::SlowDown { .. })),
    ];
    for (status, code, is) in table {
        let message = format!("The server's own words for {code}.");
        c.fake.fail_next(Failure::new("GET", "/snapshots", status, code, &message));
        let e = c.api.snapshots().expect_err(code);
        assert!(is(&e), "{status} {code} came back as {e:?}");
        assert_eq!(e.to_string(), message, "{code}");
        assert_eq!(e.code(), Some(code));
        assert_eq!(e.status(), Some(status));
    }
    // and the call works again after
    assert!(c.api.snapshots().unwrap().is_empty());
}

#[test]
fn slow_down_carries_its_retry_after() {
    let c = cloud();
    c.fake.fail_next(Failure::new("POST", "/objects/missing", 429, "slow_down", "Too many calls: wait a little.").retry_after(37));
    let e = c.api.missing(&[id(1)]).unwrap_err();
    assert_eq!(e, Error::SlowDown { message: "Too many calls: wait a little.".into(), retry_after: Some(Duration::from_secs(37)) });
    assert_eq!(e.retry_after(), Some(Duration::from_secs(37)));
    // without the header, no wait is made up
    c.fake.fail_next(Failure::new("GET", "/devices", 429, "slow_down", "Slow down."));
    let e = c.api.devices().unwrap_err();
    assert_eq!(e.retry_after(), None);
    assert!(matches!(e, Error::SlowDown { .. }));
    // any 429 slows down, whatever its code
    c.fake.fail_next(Failure::new("GET", "/devices", 429, "", "Rate limited.").retry_after(2));
    assert_eq!(c.api.devices().unwrap_err().retry_after(), Some(Duration::from_secs(2)));
}

#[test]
fn missing_objects_not_found_and_invalid_carry_their_ids() {
    let c = cloud();
    c.fake.fail_next(Failure::new("POST", "/snapshots", 422, "missing_objects", "Upload these first.").ids(&[id(1).to_uppercase().as_str(), id(2).as_str()]));
    let e = c.api.create_snapshot(&snapshot(&c.device_id, "s1", &id(3), vec![])).unwrap_err();
    assert_eq!(e, Error::MissingObjects { message: "Upload these first.".into(), ids: vec![id(1), id(2)] });
    assert_eq!(e.ids(), [id(1), id(2)]);
    c.fake.fail_next(Failure::new("POST", "/objects/download", 404, "not_found", "No such objects.").ids(&[id(4).as_str()]));
    assert_eq!(c.api.download_urls(&[id(4)]).unwrap_err().ids(), [id(4)]);
    c.fake.fail_next(Failure::new("POST", "/snapshots", 422, "invalid", "Too big.").ids(&[id(5).as_str()]));
    assert_eq!(c.api.create_snapshot(&snapshot(&c.device_id, "s1", &id(3), vec![])).unwrap_err().ids(), [id(5)]);
}

#[test]
fn a_server_error_or_an_unknown_code_is_a_server_error_with_its_status() {
    let c = cloud();
    c.fake.fail_next(Failure::new("GET", "/devices", 500, "", "Something broke."));
    let e = c.api.devices().unwrap_err();
    assert_eq!(e, Error::Server { status: 500, code: None, message: "Something broke.".into() });
    c.fake.fail_next(Failure::new("GET", "/devices", 409, "brand_new", "A code this Hersir doesn't know."));
    let e = c.api.devices().unwrap_err();
    assert_eq!(e.status(), Some(409));
    assert_eq!(e.code(), Some("brand_new"));
    assert_eq!(e.to_string(), "A code this Hersir doesn't know.");
    // a code with the wrong status isn't taken for that code
    c.fake.fail_next(Failure::new("GET", "/devices", 400, "quota", "Odd."));
    assert!(matches!(c.api.devices().unwrap_err(), Error::Server { status: 400, .. }));
}

#[test]
fn the_fake_answers_signed_out_for_a_bad_token_and_removed_for_a_removed_computer() {
    let c = cloud();
    let stranger = Api::new(c.fake.url(), "hcd_not_a_token").unwrap();
    match stranger.devices() {
        Err(Error::SignedOut(m)) => assert!(!m.is_empty()),
        other => panic!("{other:?}"),
    }
    // the browser's token works once, for POST /devices only
    let browser = c.fake.browser_token(&c.account_id());
    assert!(matches!(Api::new(c.fake.url(), &browser).unwrap().devices(), Err(Error::SignedOut(_))));
    Api::register(c.fake.url(), &browser, &new_device(&device_id(), "Second")).unwrap();
    assert!(matches!(Api::register(c.fake.url(), &browser, &new_device(&device_id(), "Third")), Err(Error::SignedOut(_))));
    // removed on the account page
    c.fake.remove_device(&c.device_id, false);
    match c.api.devices() {
        Err(e @ Error::Removed(_)) => assert_eq!(e.status(), Some(401)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_fake_answers_plan_required_and_read_only_by_the_plan() {
    let c = cloud();
    c.fake.set_plan(&c.account_id(), Plan::None);
    assert!(matches!(c.api.upload_urls(&[(id(1), 10)]), Err(Error::PlanRequired(_))));
    assert!(matches!(c.api.create_snapshot(&snapshot(&c.device_id, "s", &id(1), vec![])), Err(Error::PlanRequired(_))));
    c.fake.set_plan(&c.account_id(), Plan::Lapsed { valid_until: 1, read_only_until: 4_102_444_800_000 });
    assert!(matches!(c.api.upload_urls(&[(id(1), 10)]), Err(Error::ReadOnly(_))));
    // still readable
    assert!(c.api.snapshots().is_ok());
    // past its 60 days, not even that
    c.fake.set_plan(&c.account_id(), Plan::Lapsed { valid_until: 1, read_only_until: 2 });
    assert!(matches!(c.api.snapshots(), Err(Error::PlanRequired(_))));
}

#[test]
fn the_fake_answers_quota_missing_objects_exists_not_found_and_not_yours() {
    let k = keyed();
    let c = &k.cloud;
    // quota: over the account's storage
    c.fake.set_quota(100);
    match c.api.upload_urls(&[(id(1), 101)]) {
        Err(e @ Error::Quota(_)) => assert_eq!(e.status(), Some(413)),
        other => panic!("{other:?}"),
    }
    c.fake.set_quota(10_000_000_000);
    // missing_objects: a snapshot of objects never uploaded
    match c.api.create_snapshot(&snapshot(&c.device_id, "s1", &id(9), vec![id(1), id(2)])) {
        Err(Error::MissingObjects { ids, .. }) => assert_eq!(ids, [id(1), id(2), id(9)]),
        other => panic!("{other:?}"),
    }
    // registered once its objects are there
    for n in [1, 2, 9] {
        upload(&c.api, &id(n), b"HC1 sealed bytes");
    }
    let s = c.api.create_snapshot(&snapshot(&c.device_id, "s1", &id(9), vec![id(1), id(2)])).unwrap();
    assert_eq!(s.name, "s1");
    assert_eq!(s.device_name, "QA laptop");
    // registering the same again answers the one registered
    assert_eq!(c.api.create_snapshot(&snapshot(&c.device_id, "s1", &id(9), vec![id(1), id(2)])).unwrap(), s);
    // exists: another backup with this name
    assert!(matches!(c.api.create_snapshot(&snapshot(&c.device_id, "s1", &id(1), vec![])), Err(Error::Exists(_))));
    // not_found: objects the account doesn't have
    match c.api.download_urls(&[id(1), id(7)]) {
        Err(Error::NotFound { ids, .. }) => assert_eq!(ids, [id(7)]),
        other => panic!("{other:?}"),
    }
    assert!(matches!(c.api.delete_snapshot("no-such"), Err(Error::NotFound { .. })));
    // not_yours: another account's device
    let other = c.fake.add_account("other@example.com");
    let theirs = device_id();
    Api::register(c.fake.url(), &c.fake.browser_token(&other.id), &new_device(&theirs, "Theirs")).unwrap();
    assert!(matches!(c.api.remove_device(&theirs, false), Err(Error::NotYours(_))));
    // exists: this computer's id on another account
    assert!(matches!(Api::register(c.fake.url(), &c.fake.browser_token(&other.id), &new_device(&c.device_id, "Mine")), Err(Error::Exists(_))));
    // epoch: PUT /keys from an epoch that isn't the stored key's
    let (_, record) = keys::new_recovery_code(&k.key).unwrap();
    assert!(matches!(c.api.put_key(&record, Some(9)), Err(Error::Epoch(_))));
}

#[test]
fn the_fake_answers_invalid_for_a_request_the_protocol_doesnt_describe() {
    let c = cloud();
    assert!(matches!(c.api.missing(&["not an id".into()]), Err(Error::Invalid { .. })));
    assert!(matches!(c.api.upload_urls(&[(id(1), 4_194_369)]), Err(Error::Invalid { .. })));
    assert!(matches!(c.api.create_snapshot(&snapshot(&c.device_id, "-bad", &id(1), vec![])), Err(Error::Invalid { .. })));
    assert!(matches!(c.api.update_device(&DeviceChanges { name: Some(String::new()), ..Default::default() }), Err(Error::Invalid { .. })));
}

#[test]
fn r2_refusing_an_upload_or_download_is_a_storage_error() {
    let c = cloud();
    let urls = c.api.upload_urls(&[(id(1), 5)]).unwrap();
    // without the header the URL was signed with
    let mut bare = urls[0].clone();
    bare.headers.clear();
    match c.api.put_object(&bare, b"bytes") {
        Err(Error::Storage(m)) => {
            assert!(m.contains("SignatureDoesNotMatch"), "{m}");
            assert!(!m.contains("X-Amz-Signature"), "a presigned URL's key isn't in a message: {m}");
        }
        other => panic!("{other:?}"),
    }
    c.fake.fail_next(Failure::new("PUT", "/r2/", 503, "", "R2 is down."));
    assert!(matches!(c.api.put_object(&urls[0], b"bytes"), Err(Error::Storage(_))));
    c.api.put_object(&urls[0], b"bytes").unwrap();
    // a URL that isn't the one presigned
    let mut wrong = c.api.download_urls(&[]).unwrap_or_default();
    assert!(wrong.is_empty());
    let snap = c.api.create_snapshot(&snapshot(&c.device_id, "one", &id(1), vec![])).unwrap();
    wrong = c.api.download_urls(&[snap.manifest_id.clone()]).unwrap();
    assert_eq!(c.api.get_object(&wrong[0]).unwrap(), b"bytes");
    wrong[0].url = wrong[0].url.replace("X-Amz-Signature=", "X-Amz-Signature=x");
    assert!(matches!(c.api.get_object(&wrong[0]), Err(Error::Storage(_))));
}

#[test]
fn errors_and_debug_output_never_show_a_token() {
    let c = cloud();
    let shown = format!("{:?} {:?}", c.api, c.registered);
    assert!(!shown.contains(&c.registered.device_token), "{shown}");
    let urls = c.api.upload_urls(&[(id(1), 5)]).unwrap();
    let shown = format!("{:?}", urls[0]);
    assert!(!shown.contains("X-Amz-Signature"), "{shown}");
    assert!(!shown.contains(urls[0].headers.values().next().unwrap()), "{shown}");
}

#[test]
fn hersir_only_talks_to_hersir_cloud_over_https_or_on_this_computer() {
    assert!(matches!(Api::new("http://cloud.hersir.dev", "t"), Err(Error::Local(_))));
    assert!(matches!(Api::new("ftp://cloud.hersir.dev", "t"), Err(Error::Local(_))));
    assert!(matches!(Api::new("https://cloud.hersir.dev/?x=1", "t"), Err(Error::Local(_))));
    assert_eq!(Api::new("https://cloud.hersir.dev/", "t").unwrap().server(), "https://cloud.hersir.dev");
    assert!(Api::new("http://127.0.0.1:9", "t").is_ok());
    assert!(Api::new("http://localhost:9", "t").is_ok());
    // nothing listens: a network error, not a panic
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    match Api::new(&format!("http://127.0.0.1:{port}"), "t").unwrap().devices() {
        Err(Error::Network(m)) => assert!(m.contains("127.0.0.1"), "{m}"),
        other => panic!("{other:?}"),
    }
}
