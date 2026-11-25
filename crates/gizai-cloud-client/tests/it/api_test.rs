//! The calls a computer makes (PROTOCOL.md), against the fake: devices and their tokens, objects (with batches of
//! 1,000 ids), and snapshots listed, deleted and undeleted.

use gizai_cloud_client::api::{DeviceChanges, NewSnapshot};
use gizai_cloud_client::backup;
use gizai_cloud_client::{Api, Error};

use crate::common::{cloud, device_id, keyed, make_db, new_device, quiet, source};

fn id(n: u32) -> String {
    format!("{:064x}", n)
}

#[test]
fn a_computer_registers_lists_renames_and_signs_out() {
    let c = cloud();
    assert_eq!(c.registered.account.id, c.fake.account().id);
    let devices = c.api.devices().unwrap();
    assert_eq!(devices.len(), 1);
    let me = &devices[0];
    assert_eq!((me.id.as_str(), me.name.as_str(), me.kind.as_str(), me.platform.as_str()), (c.device_id.as_str(), "QA laptop", "desktop", "linux"));
    assert!(me.this);
    assert_eq!(me.revoked_at, None);
    assert_eq!(me.sign_pub.len(), 44);

    let renamed = c.api.update_device(&DeviceChanges { name: Some("Desk".into()), app_version: Some("0.21.0".into()), remote_on: Some(true) }).unwrap();
    assert_eq!((renamed.name.as_str(), renamed.app_version.as_str(), renamed.remote_on), ("Desk", "0.21.0", true));
    // only what is set changes
    let again = c.api.update_device(&DeviceChanges { remote_on: Some(false), ..Default::default() }).unwrap();
    assert_eq!((again.name.as_str(), again.remote_on), ("Desk", false));

    // another computer of the account, removed as lost from this one
    let other = device_id();
    let r2 = Api::register(c.fake.url(), &c.fake.browser_token(&c.account_id()), &new_device(&other, "Old laptop")).unwrap();
    let other_api = Api::new(c.fake.url(), &r2.device_token).unwrap();
    assert_eq!(c.api.devices().unwrap().len(), 2);
    c.api.remove_device(&other, true).unwrap();
    assert!(c.fake.lost(&other));
    assert!(matches!(other_api.devices(), Err(Error::Removed(_))));
    let listed = c.api.devices().unwrap();
    assert!(listed.iter().find(|d| d.id == other).unwrap().revoked_at.is_some(), "removed ones are listed too");

    // signing this computer out
    c.api.remove_device("me", false).unwrap();
    assert!(matches!(c.api.devices(), Err(Error::Removed(_))));
    assert!(!c.fake.lost(&c.device_id));
}

#[test]
fn a_new_token_revokes_the_old_one() {
    let c = cloud();
    let fresh = c.api.renew_token().unwrap();
    assert_ne!(fresh.device_token, c.registered.device_token);
    assert!(fresh.expires_at > 0);
    assert!(matches!(c.api.devices(), Err(Error::SignedOut(_))));
    let api = Api::new(c.fake.url(), &fresh.device_token).unwrap();
    assert_eq!(api.devices().unwrap().len(), 1);
    assert!(!format!("{fresh:?}").contains(&fresh.device_token));
}

#[test]
fn registering_this_computer_again_gives_it_a_new_token_and_revokes_the_old() {
    let c = cloud();
    let again = Api::register(c.fake.url(), &c.fake.browser_token(&c.account_id()), &new_device(&c.device_id, "QA laptop, again")).unwrap();
    assert!(matches!(c.api.devices(), Err(Error::SignedOut(_))));
    let api = Api::new(c.fake.url(), &again.device_token).unwrap();
    let devices = api.devices().unwrap();
    assert_eq!(devices.len(), 1, "the same computer");
    assert_eq!(devices[0].name, "QA laptop, again");
}

#[test]
fn objects_missing_is_asked_in_batches_of_1000_and_answers_each_once_in_order() {
    let c = cloud();
    let ids: Vec<String> = (0..2500).map(id).collect();
    let mut asked = ids.clone();
    asked.push(id(3));
    asked.push(id(3).to_uppercase());
    let missing = c.api.missing(&asked).unwrap();
    // every id is missing; each batch answers each once, lowercased
    assert_eq!(&missing[..2500], &ids[..]);
    let batches = c.fake.calls().iter().filter(|x| x.path == "/api/v1/objects/missing").count();
    assert_eq!(batches, 3);
}

#[test]
fn an_object_stored_already_gets_no_upload_url() {
    let c = cloud();
    let urls = c.api.upload_urls(&[(id(1), 3), (id(2), 3)]).unwrap();
    assert_eq!(urls.len(), 2);
    assert!(urls.iter().all(|u| u.url.starts_with(c.fake.url()) && u.url.contains("/r2/")), "presigned URLs point to the fake");
    assert!(urls.iter().all(|u| u.headers.contains_key(gizai_cloud_client::fake::UPLOAD_HEADER)));
    for u in &urls {
        c.api.put_object(u, b"HC1").unwrap();
    }
    c.api.create_snapshot(&NewSnapshot {
        name: "one".into(),
        made_at: 1,
        device_id: c.device_id.clone(),
        reason: "daily".into(),
        schema_version: 1,
        app_version: "0.20.1".into(),
        manifest_id: id(1),
        object_ids: vec![id(2)],
        bytes: 6,
    }).unwrap();
    assert_eq!(c.api.missing(&[id(1), id(2), id(3)]).unwrap(), [id(3)]);
    let urls = c.api.upload_urls(&[(id(1), 3), (id(3), 3)]).unwrap();
    assert_eq!(urls.iter().map(|u| u.id.as_str()).collect::<Vec<_>>(), [id(3).as_str()]);
    let downloads = c.api.download_urls(&[id(1), id(2), id(1)]).unwrap();
    assert_eq!(downloads.len(), 2, "each once");
    assert_eq!(c.api.get_object(&downloads[0]).unwrap(), b"HC1");
}

#[test]
fn snapshots_are_listed_newest_first_and_deleted_ones_can_be_undeleted() {
    let k = keyed();
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("copy.db");
    make_db(&db, 200, "listing");
    for (name, at) in [("gizai-daily-1", 1_000), ("gizai-daily-2", 3_000), ("gizai-daily-3", 2_000)] {
        let src = backup::Source { made_at: at, reason: "daily".into(), ..source(&k.cloud, name, &db, vec![]) };
        backup::back_up(&k.cloud.api, &k.key, &src, &mut quiet).unwrap();
    }
    let api = &k.cloud.api;
    let names: Vec<String> = api.snapshots().unwrap().into_iter().map(|s| s.name).collect();
    assert_eq!(names, ["gizai-daily-2", "gizai-daily-3", "gizai-daily-1"]);
    let s = &api.snapshots().unwrap()[0];
    assert_eq!((s.device_id.as_str(), s.device_name.as_str(), s.reason.as_str(), s.schema_version), (k.cloud.device_id.as_str(), "QA laptop", "daily", crate::common::SCHEMA));
    assert!(s.bytes > 0);

    api.delete_snapshot("gizai-daily-3").unwrap();
    let listed = api.snapshots().unwrap();
    assert!(listed.iter().find(|s| s.name == "gizai-daily-3").unwrap().deleted_at.is_some(), "listed in its 7 days");
    let back = api.undelete_snapshot("gizai-daily-3").unwrap();
    assert_eq!(back.name, "gizai-daily-3");
    assert_eq!(back.deleted_at, None);
    assert!(api.snapshots().unwrap().iter().all(|s| s.deleted_at.is_none()));
    assert!(matches!(api.undelete_snapshot("no-such"), Err(Error::NotFound { .. })));
}
