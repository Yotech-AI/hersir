//! A backup to the fake Hersir Cloud and a restore from it (research 3.6): the restored format-1 folder passes GA-218's
//! `gizai_core::restore::check` and its database is the original byte for byte; a second backup of an unchanged
//! database uploads only its manifest; `missing_objects` is uploaded and registered again; the steps report progress.

use std::fs;
use std::path::Path;

use gizai_cloud_client::Error;
use gizai_cloud_client::backup::{self, BackedUp, Progress, Step};
use gizai_cloud_client::fake::Failure;
use gizai_cloud_client::restore::{self, OBJECTS_DIR};

use crate::common::{Keyed, blob, change_row, keyed, make_db, noise, quiet, record, sha256_hex, source, sub};

const NAME: &str = "gizai-manual-20261125-120000-000-9c1a07";

fn back_up(k: &Keyed, src: &backup::Source) -> BackedUp {
    backup::back_up(&k.cloud.api, &k.key, src, &mut quiet).unwrap_or_else(|e| panic!("back_up: {e:?}"))
}

/// Restores `name` as `GET /snapshots` lists it into `<dir>/restore-<name>`, checks it with format 1's restore, and
/// returns the folder.
fn restore_and_check(k: &Keyed, dir: &Path, name: &str) -> std::path::PathBuf {
    let listed = k.cloud.api.snapshots().unwrap();
    let snap = listed.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("{name} isn't listed: {listed:?}"));
    let out = dir.join(format!("restore-{name}"));
    let r = restore::restore(&k.cloud.api, &k.key, &snap.name, &snap.manifest_id, &out, &mut quiet).unwrap_or_else(|e| panic!("restore: {e:?}"));
    assert_eq!(r.dir, out);
    assert_eq!(r.name, name);
    let data_dir = dir.join(format!("data-{name}"));
    fs::create_dir_all(&data_dir).unwrap();
    let m = gizai_core::restore::check(&data_dir, &out, name, &mut |_| Ok(())).unwrap_or_else(|e| panic!("format 1's check: {e:?}"));
    assert_eq!(m, r.manifest);
    gizai_core::restore::remove_checked(&data_dir);
    out
}

fn unpacked_db(out: &Path, name: &str) -> Vec<u8> {
    let zst = fs::read(out.join("snapshots").join(format!("{name}.db.zst"))).unwrap();
    zstd::decode_all(&zst[..]).unwrap()
}

#[test]
fn a_backup_restores_into_a_format_1_folder_whose_database_matches_byte_for_byte() {
    let k = keyed();
    let dir = tempfile::tempdir().unwrap();
    let data = sub(dir.path(), "data");
    let db = dir.path().join("copy.db");
    make_db(&db, 14_000, "byte-for-byte");
    let original = fs::read(&db).unwrap();
    assert!(original.len() > 4 * 256 * 1024, "several chunks: {}", original.len());
    let small = blob(&data, b"a small attachment".repeat(300).as_slice());
    let big = blob(&data, &noise(3, 6 * 1024 * 1024));
    let edge = blob(&data, &noise(4, 4 * 1024 * 1024));
    let empty = blob(&data, b"");
    let files = vec![small.clone(), big.clone(), edge.clone(), empty.clone(), small.clone()];
    let src = source(&k.cloud, NAME, &db, files);
    let done = back_up(&k, &src);

    assert_eq!(done.snapshot.name, NAME);
    assert_eq!(done.snapshot.manifest_id, done.manifest_id);
    assert_eq!(done.snapshot.device_id, k.cloud.device_id);
    assert_eq!(done.snapshot.reason, "manual");
    assert_eq!(done.uploaded, done.objects, "a first backup uploads every object");
    assert_eq!(k.cloud.fake.uploads().len(), done.objects);
    let m = &done.manifest;
    assert_eq!(m.snapshot.db.db_bytes, original.len() as i64);
    assert_eq!(m.snapshot.db.sha256, sha256_hex(&original));
    assert_eq!(m.snapshot.db.bytes, 0, "in the cloud the database isn't one compressed file");
    assert_eq!(m.snapshot.files.len(), 4, "each blob once");
    assert!(m.snapshot.files_missing.is_empty());
    assert_eq!(m.chunks.db.len(), original.len().div_ceil(256 * 1024));
    assert_eq!(m.chunks.files[&small.sha256].len(), 1);
    assert!(m.chunks.files[&big.sha256].len() >= 3);
    assert!(m.chunks.files[&edge.sha256].len() >= 2, "an incompressible 4 MiB blob is cut so each object fits");
    assert!(m.chunks.files[&empty.sha256].is_empty());
    // every object on the fake's R2 fits
    for id in k.cloud.fake.objects(&k.cloud.account_id()) {
        assert!(k.cloud.fake.object(&k.cloud.account_id(), &id).unwrap().len() <= 4_194_368);
    }

    let out = restore_and_check(&k, dir.path(), NAME);
    assert_eq!(unpacked_db(&out, NAME), original, "the database byte for byte");
    for b in [&small, &big, &edge, &empty] {
        let restored = fs::read(out.join("files").join(&b.sha256[..2]).join(&b.sha256)).unwrap();
        assert_eq!(restored, fs::read(&b.path).unwrap(), "blob {}", &b.sha256[..12]);
    }
    // format 1's folder: its marker, the manifest without chunks, and no downloads left
    assert!(out.join("gizai-backups.json").is_file());
    assert!(!out.join(OBJECTS_DIR).exists());
    let json: serde_json::Value = serde_json::from_slice(&fs::read(out.join("snapshots").join(format!("{NAME}.json"))).unwrap()).unwrap();
    assert!(json.get("chunks").is_none(), "{json}");
    assert_eq!(json["format"], "gizai-backup");
    assert_eq!(json["db"]["path"], format!("snapshots/{NAME}.db.zst"));
    assert_eq!(json["db"]["bytes"].as_u64().unwrap(), fs::metadata(out.join("snapshots").join(format!("{NAME}.db.zst"))).unwrap().len());
    let names: Vec<String> = fs::read_dir(out.join("snapshots")).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    assert!(names.iter().all(|n| !n.ends_with(".part")), "{names:?}");
}

#[test]
fn a_second_backup_of_an_unchanged_database_uploads_nothing_but_the_manifest() {
    let k = keyed();
    let dir = tempfile::tempdir().unwrap();
    let data = sub(dir.path(), "data");
    let db = dir.path().join("copy.db");
    make_db(&db, 8000, "second-backup");
    let files = vec![blob(&data, b"a screenshot, as bytes".repeat(100).as_slice()), blob(&data, &noise(8, 5 * 1024 * 1024))];
    let first = back_up(&k, &source(&k.cloud, "gizai-manual-1", &db, files.clone()));
    let before = k.cloud.fake.uploads();
    assert_eq!(before.len(), first.objects);

    let second = back_up(&k, &source(&k.cloud, "gizai-manual-2", &db, files));
    let after = k.cloud.fake.uploads();
    assert_eq!(second.uploaded, 1, "only the manifest");
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(after.last().unwrap(), &second.manifest_id);
    assert_ne!(second.manifest_id, first.manifest_id);
    assert_eq!(second.objects, first.objects);
    assert_eq!(second.manifest.chunks, first.manifest.chunks, "the same chunks, by the same ids");
    // the presigned PUTs to R2: one more, for the manifest
    let puts = |calls: &[gizai_cloud_client::fake::Call]| calls.iter().filter(|c| c.method == "PUT" && c.path.starts_with("/r2/")).count();
    assert_eq!(puts(&k.cloud.fake.calls()), first.objects + 1);
    // both restore
    let out = restore_and_check(&k, dir.path(), "gizai-manual-2");
    assert_eq!(unpacked_db(&out, "gizai-manual-2"), fs::read(&db).unwrap());
}

#[test]
fn a_backup_after_a_small_change_uploads_only_the_chunks_that_changed() {
    let k = keyed();
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("copy.db");
    make_db(&db, 12_000, "small-change");
    let first = back_up(&k, &source(&k.cloud, "gizai-daily-1", &db, vec![]));
    change_row(&db, 6000, "a note changed since the last backup");
    let second = back_up(&k, &source(&k.cloud, "gizai-daily-2", &db, vec![]));
    assert!(second.uploaded >= 2 && second.uploaded <= 3, "the changed page's chunk, the header's and the manifest: {}", second.uploaded);
    assert!(second.uploaded < first.uploaded);
    let out = restore_and_check(&k, dir.path(), "gizai-daily-2");
    assert_eq!(unpacked_db(&out, "gizai-daily-2"), fs::read(&db).unwrap());
    // the first one still restores as it was
    let out1 = restore_and_check(&k, dir.path(), "gizai-daily-1");
    assert_ne!(unpacked_db(&out1, "gizai-daily-1"), fs::read(&db).unwrap());
}

#[test]
fn a_backup_reports_chunking_uploading_and_registering_and_a_restore_downloading_and_checking() {
    let k = keyed();
    let dir = tempfile::tempdir().unwrap();
    let data = sub(dir.path(), "data");
    let db = dir.path().join("copy.db");
    make_db(&db, 6000, "progress");
    let b = blob(&data, b"progress blob".repeat(50).as_slice());
    let total_bytes = fs::metadata(&db).unwrap().len() + fs::metadata(&b.path).unwrap().len();
    let mut heard: Vec<Progress> = Vec::new();
    backup::back_up(&k.cloud.api, &k.key, &source(&k.cloud, NAME, &db, vec![b]), &mut record(&mut heard)).unwrap();
    let steps: Vec<Step> = heard.iter().map(|p| p.step).fold(Vec::new(), |mut v, s| {
        if v.last() != Some(&s) {
            v.push(s);
        }
        v
    });
    assert_eq!(steps, [Step::Chunking, Step::Uploading, Step::Registering], "{heard:?}");
    assert!(heard.iter().all(|p| p.done <= p.total), "{heard:?}");
    let last_chunking = heard.iter().rev().find(|p| p.step == Step::Chunking).unwrap();
    assert_eq!((last_chunking.done, last_chunking.total), (total_bytes, total_bytes));
    let last_upload = heard.iter().rev().find(|p| p.step == Step::Uploading).unwrap();
    assert_eq!(last_upload.done, last_upload.total);
    assert_eq!(heard.last().unwrap(), &Progress { step: Step::Registering, done: 1, total: 1 });
    // each step's done only grows
    for w in heard.windows(2) {
        if w[0].step == w[1].step {
            assert!(w[1].done >= w[0].done, "{w:?}");
        }
    }
    assert_eq!(Step::Chunking.as_str(), "chunking");
    assert_eq!(serde_json::to_string(&Step::Registering).unwrap(), "\"registering\"");

    let snap = &k.cloud.api.snapshots().unwrap()[0];
    let mut heard: Vec<Progress> = Vec::new();
    restore::restore(&k.cloud.api, &k.key, NAME, &snap.manifest_id, &dir.path().join("out"), &mut record(&mut heard)).unwrap();
    let steps: Vec<Step> = heard.iter().map(|p| p.step).fold(Vec::new(), |mut v, s| {
        if v.last() != Some(&s) {
            v.push(s);
        }
        v
    });
    assert_eq!(steps, [Step::Downloading, Step::Checking], "{heard:?}");
    assert!(heard.iter().all(|p| p.done <= p.total), "{heard:?}");
    let last = heard.last().unwrap();
    assert_eq!((last.step, last.done), (Step::Checking, last.total));
    assert_eq!(Step::Downloading.as_str(), "downloading");
    assert_eq!(Step::Checking.as_str(), "checking");
}

#[test]
fn the_progress_callback_can_stop_a_backup() {
    let k = keyed();
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("copy.db");
    make_db(&db, 3000, "stopped");
    for stop_at in [Step::Chunking, Step::Uploading, Step::Registering] {
        let r = backup::back_up(&k.cloud.api, &k.key, &source(&k.cloud, NAME, &db, vec![]), &mut |p: Progress| {
            if p.step == stop_at { Err(Error::Cancelled) } else { Ok(()) }
        });
        assert_eq!(r.unwrap_err(), Error::Cancelled, "{stop_at:?}");
    }
    assert!(k.cloud.api.snapshots().unwrap().is_empty(), "nothing registered");
}

#[test]
fn objects_the_server_says_are_missing_are_uploaded_again_and_the_snapshot_registered() {
    let k = keyed();
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("copy.db");
    make_db(&db, 6000, "missing-objects");
    // R2 answers 200 for two uploads and keeps nothing: POST /snapshots says 422 missing_objects
    k.cloud.fake.forget_next_uploads(2);
    let done = back_up(&k, &source(&k.cloud, NAME, &db, vec![]));
    assert_eq!(done.uploaded, done.objects + 2, "two uploaded again");
    let registers: Vec<u16> = k.cloud.fake.calls().iter().filter(|c| c.method == "POST" && c.path == "/api/v1/snapshots").map(|c| c.status).collect();
    assert_eq!(registers, [422, 201]);
    let out = restore_and_check(&k, dir.path(), NAME);
    assert_eq!(unpacked_db(&out, NAME), fs::read(&db).unwrap());
}

#[test]
fn a_missing_objects_answer_naming_objects_already_stored_registers_again() {
    let k = keyed();
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("copy.db");
    make_db(&db, 2000, "missing-again");
    let first = back_up(&k, &source(&k.cloud, "gizai-manual-1", &db, vec![]));
    let one = first.manifest.chunks.db[0].clone();
    k.cloud.fake.fail_next(Failure::new("POST", "/snapshots", 422, "missing_objects", "Upload these first.").ids(&[one.as_str()]));
    let second = back_up(&k, &source(&k.cloud, "gizai-manual-2", &db, vec![]));
    assert_eq!(second.snapshot.name, "gizai-manual-2");
    // one it doesn't know of isn't uploaded, and the backup says so
    let other = format!("{:064x}", 7);
    k.cloud.fake.fail_next(Failure::new("POST", "/snapshots", 422, "missing_objects", "Upload these first.").ids(&[other.as_str()]));
    match backup::back_up(&k.cloud.api, &k.key, &source(&k.cloud, "gizai-manual-3", &db, vec![]), &mut quiet) {
        Err(Error::MissingObjects { .. }) => {}
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_slow_down_and_a_refused_upload_on_the_way_are_waited_out() {
    let k = keyed();
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("copy.db");
    make_db(&db, 2000, "waited-out");
    k.cloud.fake.fail_next(Failure::new("POST", "/objects/missing", 429, "slow_down", "Slow down.").retry_after(1));
    k.cloud.fake.fail_next(Failure::new("PUT", "/r2/", 503, "", "R2 is busy."));
    let done = back_up(&k, &source(&k.cloud, NAME, &db, vec![]));
    assert_eq!(done.snapshot.name, NAME);
    let missing: Vec<u16> = k.cloud.fake.calls().iter().filter(|c| c.path == "/api/v1/objects/missing").map(|c| c.status).collect();
    assert_eq!(missing, [429, 200]);
    let out = restore_and_check(&k, dir.path(), NAME);
    assert_eq!(unpacked_db(&out, NAME), fs::read(&db).unwrap());
}

#[test]
fn an_error_that_isnt_passing_stops_the_backup() {
    let k = keyed();
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("copy.db");
    make_db(&db, 1000, "quota");
    k.cloud.fake.set_quota(1000);
    assert!(matches!(backup::back_up(&k.cloud.api, &k.key, &source(&k.cloud, NAME, &db, vec![]), &mut quiet), Err(Error::Quota(_))));
    k.cloud.fake.set_quota(10_000_000_000);
    k.cloud.fake.set_plan(&k.cloud.account_id(), gizai_cloud_client::fake::Plan::None);
    assert!(matches!(backup::back_up(&k.cloud.api, &k.key, &source(&k.cloud, NAME, &db, vec![]), &mut quiet), Err(Error::PlanRequired(_))));
}

#[test]
fn a_blob_that_isnt_there_or_changed_goes_into_files_missing_as_format_1_does() {
    let k = keyed();
    let dir = tempfile::tempdir().unwrap();
    let data = sub(dir.path(), "data");
    let db = dir.path().join("copy.db");
    make_db(&db, 1000, "files-missing");
    let kept = blob(&data, b"kept");
    let gone = blob(&data, b"gone");
    fs::remove_file(&gone.path).unwrap();
    let changed = blob(&data, b"before");
    fs::write(&changed.path, b"after!").unwrap();
    let done = back_up(&k, &source(&k.cloud, NAME, &db, vec![kept.clone(), gone.clone(), changed.clone()]));
    let mut missing = done.manifest.snapshot.files_missing.clone();
    missing.sort();
    let mut expected = vec![gone.sha256.clone(), changed.sha256.clone()];
    expected.sort();
    assert_eq!(missing, expected);
    assert!(done.manifest.chunks.files.contains_key(&kept.sha256));
    assert!(!done.manifest.chunks.files.contains_key(&gone.sha256));
    let gone_entry = done.manifest.snapshot.files.iter().find(|f| f.sha256 == gone.sha256).unwrap();
    assert_eq!(gone_entry.bytes, 4, "the size the database gives it");
    // it restores, and format 1's restore refuses it as it does a backups folder's
    let snap = &k.cloud.api.snapshots().unwrap()[0];
    let out = dir.path().join("out");
    restore::restore(&k.cloud.api, &k.key, NAME, &snap.manifest_id, &out, &mut quiet).unwrap();
    let data_dir = sub(dir.path(), "app");
    let e = gizai_core::restore::check(&data_dir, &out, NAME, &mut |_| Ok(())).unwrap_err();
    assert!(e.to_string().contains("missing"), "{e}");
}

#[test]
fn a_changed_object_another_key_or_another_snapshots_manifest_is_refused_on_restore() {
    let k = keyed();
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("copy.db");
    make_db(&db, 6000, "damaged");
    let done = back_up(&k, &source(&k.cloud, NAME, &db, vec![]));
    let account = k.cloud.account_id();

    // another account key
    let other = gizai_cloud_client::keys::AccountKey::generate(1).unwrap();
    let r = restore::restore(&k.cloud.api, &other, NAME, &done.manifest_id, &dir.path().join("a"), &mut quiet);
    assert!(matches!(r, Err(Error::Damaged(_))), "{r:?}");
    // the manifest asked for under another snapshot's name
    let r = restore::restore(&k.cloud.api, &k.key, "gizai-manual-other", &done.manifest_id, &dir.path().join("b"), &mut quiet);
    match r {
        Err(Error::Damaged(m)) => assert!(m.contains("another snapshot"), "{m}"),
        other => panic!("{other:?}"),
    }
    // a database chunk changed on R2
    let id = done.manifest.chunks.db[1].clone();
    let mut bytes = k.cloud.fake.object(&account, &id).unwrap();
    let at = bytes.len() / 2;
    bytes[at] ^= 0x20;
    k.cloud.fake.replace_object(&account, &id, bytes);
    let out = dir.path().join("c");
    let r = restore::restore(&k.cloud.api, &k.key, NAME, &done.manifest_id, &out, &mut quiet);
    assert!(matches!(r, Err(Error::Damaged(_))), "{r:?}");
    assert!(!out.join("snapshots").join(format!("{NAME}.json")).exists(), "no manifest for a restore that failed");
    assert!(!out.join("snapshots").join(format!("{NAME}.db.zst")).exists());
    assert!(!out.join("snapshots").join(format!("{NAME}.db.zst.part")).exists());
    // one object put where another should be
    let first = done.manifest.chunks.db[0].clone();
    let swapped = k.cloud.fake.object(&account, &done.manifest.chunks.db[2]).unwrap();
    k.cloud.fake.replace_object(&account, &first, swapped);
    let r = restore::restore(&k.cloud.api, &k.key, NAME, &done.manifest_id, &dir.path().join("d"), &mut quiet);
    assert!(matches!(r, Err(Error::Damaged(_))), "{r:?}");
}

#[test]
fn an_interrupted_restore_picks_up_without_downloading_again() {
    let k = keyed();
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("copy.db");
    make_db(&db, 6000, "picks-up");
    let done = back_up(&k, &source(&k.cloud, NAME, &db, vec![]));
    let out = dir.path().join("out");
    let r = restore::restore(&k.cloud.api, &k.key, NAME, &done.manifest_id, &out, &mut |p: Progress| {
        if p.step == Step::Checking { Err(Error::Cancelled) } else { Ok(()) }
    });
    assert_eq!(r.unwrap_err(), Error::Cancelled);
    let kept = fs::read_dir(out.join(OBJECTS_DIR)).unwrap().count();
    assert_eq!(kept, done.objects, "every object downloaded so far stays");
    let gets_before = k.cloud.fake.calls().iter().filter(|c| c.method == "GET" && c.path.starts_with("/r2/")).count();
    let again = restore::restore(&k.cloud.api, &k.key, NAME, &done.manifest_id, &out, &mut quiet).unwrap();
    assert_eq!(again.downloaded, 0);
    assert_eq!(again.objects, done.objects);
    let gets_after = k.cloud.fake.calls().iter().filter(|c| c.method == "GET" && c.path.starts_with("/r2/")).count();
    assert_eq!(gets_after, gets_before);
    let data_dir = sub(dir.path(), "app");
    gizai_core::restore::check(&data_dir, &out, NAME, &mut |_| Ok(())).unwrap();
    assert_eq!(unpacked_db(&out, NAME), fs::read(&db).unwrap());
}

#[test]
fn a_restore_waits_out_a_refused_download() {
    let k = keyed();
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("copy.db");
    make_db(&db, 2000, "refused-download");
    let done = back_up(&k, &source(&k.cloud, NAME, &db, vec![]));
    k.cloud.fake.fail_next(Failure::new("GET", "/r2/", 403, "", "Request has expired."));
    k.cloud.fake.fail_next(Failure::new("POST", "/objects/download", 429, "slow_down", "Slow down.").retry_after(1));
    let out = dir.path().join("out");
    let r = restore::restore(&k.cloud.api, &k.key, NAME, &done.manifest_id, &out, &mut quiet).unwrap();
    assert_eq!(r.objects, done.objects);
    assert_eq!(unpacked_db(&out, NAME), fs::read(&db).unwrap());
}

#[test]
fn a_backup_refuses_a_source_hersir_cloud_or_format_1_wouldnt_take() {
    let k = keyed();
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("copy.db");
    make_db(&db, 10, "refused");
    let good = source(&k.cloud, NAME, &db, vec![]);
    let mut bad = Vec::new();
    for name in ["", "-starts-with-a-dash", "has_underscore", "has.dot", "has space", &"a".repeat(129)] {
        bad.push(backup::Source { name: name.to_string(), ..good.clone() });
    }
    bad.push(backup::Source { reason: "weekly".into(), ..good.clone() });
    bad.push(backup::Source { device: gizai_core::backups::Device { id: " ".into(), name: "x".into() }, ..good.clone() });
    bad.push(backup::Source { db: dir.path().join("no-such.db"), ..good.clone() });
    bad.push(backup::Source { files: vec![backup::Blob { sha256: "not-a-hash".into(), bytes: None, path: dir.path().join("x") }], ..good.clone() });
    let empty = dir.path().join("empty.db");
    fs::write(&empty, b"").unwrap();
    bad.push(backup::Source { db: empty, ..good.clone() });
    for src in &bad {
        match backup::back_up(&k.cloud.api, &k.key, src, &mut quiet) {
            Err(Error::Local(_)) => {}
            other => panic!("{:?} / {:?}: {other:?}", src.name, src.reason),
        }
    }
    assert!(k.cloud.fake.uploads().is_empty(), "nothing uploaded");
    // and a restore refuses a name that isn't a snapshot's, or a manifest id that isn't one
    assert!(matches!(restore::restore(&k.cloud.api, &k.key, "../x", &format!("{:064x}", 1), &dir.path().join("o"), &mut quiet), Err(Error::Local(_))));
    assert!(matches!(restore::restore(&k.cloud.api, &k.key, NAME, "abc", &dir.path().join("o"), &mut quiet), Err(Error::Damaged(_))));
}
