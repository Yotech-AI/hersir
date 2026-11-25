//! Nothing unencrypted leaves the computer: every byte the fake Hersir Cloud and its fake R2 received during a backup
//! is read, and none of the database's or the blobs' content is in it, nor a key or the recovery code; object ids are
//! HMACs, never a chunk's plain SHA-256. What R2 keeps opens only with the data key, as the doc says.

use std::collections::HashSet;
use std::fs;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use gizai_cloud_client::backup::{self, CloudManifest};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::common::{blob, contains, db_chunks, keyed, make_db, quiet, sha256_hex, source, sub};

const MARKER: &str = "zebra-quartz-plaintext";
const BLOB_TEXT: &str = "an attachment's words: violet aqueduct, umber kite, nineteen lanterns";

fn hmac_hex(key: &[u8], bytes: &[u8]) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).unwrap();
    mac.update(bytes);
    mac.finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

fn b64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 16-byte pieces of `bytes` that say something (at least 6 different bytes), every 16 bytes.
fn pieces(bytes: &[u8]) -> HashSet<[u8; 16]> {
    bytes.chunks_exact(16).filter(|w| w.iter().collect::<HashSet<_>>().len() >= 6).map(|w| w.try_into().unwrap()).collect()
}

/// Opens an object as docs/backup-format.md says, without the crate.
fn open_by_hand(data_key: &[u8], id: &str, object: &[u8]) -> Vec<u8> {
    assert_eq!(&object[..3], b"HC1", "{id}");
    let compressed = XChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(data_key))
        .decrypt(XNonce::from_slice(&object[4..28]), Payload { msg: &object[28..], aad: id.as_bytes() })
        .unwrap_or_else(|_| panic!("object {id} doesn't open with the data key"));
    zstd::decode_all(&compressed[..]).unwrap()
}

#[test]
fn nothing_unencrypted_leaves_the_computer_and_object_ids_arent_plain_sha256() {
    let k = keyed();
    let dir = tempfile::tempdir().unwrap();
    let data = sub(dir.path(), "data");
    let db = dir.path().join("copy.db");
    make_db(&db, 9000, MARKER);
    let db_bytes = fs::read(&db).unwrap();
    let text_blob = blob(&data, BLOB_TEXT.repeat(400).as_bytes());
    let blob_bytes = fs::read(&text_blob.path).unwrap();
    let before = k.cloud.fake.received().len();
    let done = backup::back_up(&k.cloud.api, &k.key, &source(&k.cloud, "gizai-manual-1", &db, vec![text_blob.clone()]), &mut quiet).unwrap();
    let all = k.cloud.fake.received();
    let sent = &all[before..];
    assert!(sent.len() > 10_000, "the backup went through the fake: {} bytes", sent.len());

    // the content, as it is
    for needle in [MARKER.as_bytes(), BLOB_TEXT.as_bytes(), b"SQLite format 3\0", b"CREATE TABLE notes", b"harbour lantern", b"violet aqueduct"] {
        assert!(!contains(sent, needle), "{:?} left the computer", String::from_utf8_lossy(needle));
    }
    // any 16 bytes of the database or the blob that say something
    let mut wanted = pieces(&db_bytes);
    wanted.extend(pieces(&blob_bytes));
    assert!(wanted.len() > 1000, "{} pieces", wanted.len());
    let found = sent.windows(16).filter(|w| wanted.contains(&<[u8; 16]>::try_from(*w).unwrap())).count();
    assert_eq!(found, 0, "pieces of the database or a blob left the computer");
    // nor compressed and unencrypted: zstd of a chunk isn't there either
    let first_zstd = zstd::bulk::compress(db_chunks(&db_bytes)[1], 3).unwrap();
    assert!(!contains(sent, &first_zstd[8..40]));

    // the keys and the recovery code
    for secret in [
        k.key.as_bytes().to_vec(),
        b64(k.key.as_bytes()).into_bytes(),
        hex(k.key.as_bytes()).into_bytes(),
        k.key.data_key().to_vec(),
        hex(&k.key.data_key()[..]).into_bytes(),
        k.key.id_key().to_vec(),
        hex(&k.key.id_key()[..]).into_bytes(),
        k.code.as_bytes().to_vec(),
        k.code.characters().as_bytes().to_vec(),
        k.code.shown().as_bytes().to_vec(),
    ] {
        assert!(!contains(&all, &secret), "a key or the recovery code left the computer");
    }

    // hashes that would tell what the content is: a chunk's or a blob's plain SHA-256, the database's
    let mut plain: Vec<String> = db_chunks(&db_bytes).iter().map(|c| sha256_hex(c)).collect();
    plain.push(sha256_hex(&db_bytes));
    plain.push(sha256_hex(&blob_bytes));
    assert_eq!(text_blob.sha256, sha256_hex(&blob_bytes));
    for h in &plain {
        assert!(!contains(sent, h.as_bytes()), "a plain SHA-256 left the computer: {h}");
    }
    let account = k.cloud.account_id();
    let stored = k.cloud.fake.objects(&account);
    assert_eq!(stored.len(), done.objects);
    for id in &stored {
        assert!(!plain.contains(id), "object id {id} is a plain SHA-256");
    }
    // the ids are HMAC-SHA256(id key, chunk), in the manifest's order
    let id_key = k.key.id_key();
    let expected: Vec<String> = db_chunks(&db_bytes).iter().map(|c| hmac_hex(&id_key[..], c)).collect();
    assert_eq!(done.manifest.chunks.db, expected);
    assert_eq!(done.manifest.chunks.files[&text_blob.sha256], [hmac_hex(&id_key[..], &blob_bytes)]);

    // what R2 keeps: HC1 objects that open with the data key into the chunks their ids name
    let data_key = k.key.data_key();
    for id in &stored {
        let object = k.cloud.fake.object(&account, id).unwrap();
        assert!(object.len() <= 4_194_368);
        let chunk = open_by_hand(&data_key[..], id, &object);
        assert_eq!(&hmac_hex(&id_key[..], &chunk), id);
    }
    // the manifest too: compact JSON with format 1's fields and the chunks
    let manifest = open_by_hand(&data_key[..], &done.manifest_id, &k.cloud.fake.object(&account, &done.manifest_id).unwrap());
    assert!(!manifest.contains(&b'\n'), "compact JSON");
    let read: CloudManifest = serde_json::from_slice(&manifest).unwrap();
    assert_eq!(read, done.manifest);
    assert_eq!(hmac_hex(&id_key[..], &manifest), done.manifest_id);
}
