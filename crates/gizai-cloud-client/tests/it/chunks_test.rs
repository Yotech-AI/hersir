//! Chunks and objects (research 3.6; docs/backup-format.md, For Hersir Cloud): 256 KiB database chunks, whole blobs up
//! to 4 MiB and FastCDC above, HMAC-SHA256 ids (never a plain SHA-256), and `HC1` objects of at most 4,194,368 bytes
//! that open only as the chunk they were asked for.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use gizai_cloud_client::Error;
use gizai_cloud_client::chunks::{self, ChunkKeys, MAX_OBJECT};
use gizai_cloud_client::keys::AccountKey;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::common::{db_chunks, make_db, noise, sha256_hex};

fn keys() -> (AccountKey, ChunkKeys) {
    let key = AccountKey::generate(1).unwrap();
    let ck = ChunkKeys::new(&key);
    (key, ck)
}

fn hmac_hex(key: &[u8], bytes: &[u8]) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).unwrap();
    mac.update(bytes);
    mac.finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

fn no_read() -> impl FnMut(u64) -> gizai_cloud_client::Result<()> {
    |_| Ok(())
}

#[test]
fn the_most_an_object_may_be_is_4_194_368_bytes() {
    assert_eq!(MAX_OBJECT, 4_194_368);
    assert_eq!(chunks::DB_CHUNK, 262_144);
    assert_eq!(chunks::WHOLE_BLOB, 4_194_304);
}

#[test]
fn the_database_is_cut_into_256_kib_chunks_named_by_hmac_not_sha256() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("copy.db");
    make_db(&db, 9000, "chunk-test");
    let bytes = std::fs::read(&db).unwrap();
    assert!(bytes.len() > 3 * 256 * 1024, "a database of several chunks: {}", bytes.len());
    let (key, ck) = keys();
    let mut heard = 0u64;
    let cut = chunks::cut_db(&ck, &db, &mut |n| {
        heard += n;
        Ok(())
    }).unwrap();
    assert_eq!(heard, bytes.len() as u64, "the callback hears every byte read");
    assert_eq!(cut.bytes, bytes.len() as u64);
    assert_eq!(cut.sha256, sha256_hex(&bytes));
    let expected = db_chunks(&bytes);
    assert_eq!(cut.pieces.len(), expected.len());
    let id_key = key.id_key();
    for (i, (p, chunk)) in cut.pieces.iter().zip(&expected).enumerate() {
        assert_eq!(p.offset, (i * 256 * 1024) as u64);
        assert_eq!(p.len, chunk.len());
        if i + 1 < expected.len() {
            assert_eq!(p.len, 256 * 1024);
        }
        assert_eq!(p.id, hmac_hex(&id_key[..], chunk), "chunk {i}'s id is HMAC-SHA256(id key, its bytes)");
        assert_ne!(p.id, sha256_hex(chunk), "never its plain SHA-256");
        assert!(chunks::is_object_id(&p.id));
    }
}

#[test]
fn a_blob_up_to_4_mib_is_one_chunk_and_an_empty_one_has_none() {
    let dir = tempfile::tempdir().unwrap();
    let (key, ck) = keys();
    let small = dir.path().join("small");
    let text = b"a small attachment, said twice. a small attachment, said twice.".repeat(1000);
    std::fs::write(&small, &text).unwrap();
    let cut = chunks::cut_blob(&ck, &small, &mut no_read()).unwrap();
    assert_eq!(cut.pieces.len(), 1);
    assert_eq!(cut.pieces[0].id, hmac_hex(&key.id_key()[..], &text));
    assert_eq!(cut.sha256, sha256_hex(&text));
    assert_eq!(cut.bytes, text.len() as u64);

    // 4 MiB that zstd shrinks: still one chunk
    let four = dir.path().join("four");
    let compressible = vec![7u8; 4 * 1024 * 1024];
    std::fs::write(&four, &compressible).unwrap();
    let cut = chunks::cut_blob(&ck, &four, &mut no_read()).unwrap();
    assert_eq!(cut.pieces.len(), 1);
    assert_eq!(cut.pieces[0].len, 4 * 1024 * 1024);

    let empty = dir.path().join("empty");
    std::fs::write(&empty, b"").unwrap();
    let cut = chunks::cut_blob(&ck, &empty, &mut no_read()).unwrap();
    assert!(cut.pieces.is_empty());
    assert_eq!(cut.sha256, sha256_hex(b""));
}

#[test]
fn a_blob_over_4_mib_is_cut_by_content_and_an_edit_changes_only_the_chunks_around_it() {
    let dir = tempfile::tempdir().unwrap();
    let (_, ck) = keys();
    let big = noise(11, 9 * 1024 * 1024);
    let path = dir.path().join("big");
    std::fs::write(&path, &big).unwrap();
    let cut = chunks::cut_blob(&ck, &path, &mut no_read()).unwrap();
    assert!(cut.pieces.len() >= 4, "{} chunks", cut.pieces.len());
    let mut at = 0u64;
    for (i, p) in cut.pieces.iter().enumerate() {
        assert_eq!(p.offset, at, "the chunks follow each other");
        assert!(p.len <= chunks::CDC_MAX, "chunk {i} is {} bytes", p.len);
        if i + 1 < cut.pieces.len() {
            assert!(p.len >= chunks::CDC_MIN, "chunk {i} is {} bytes", p.len);
        }
        assert_eq!(p.id, ck.id_of(&big[p.offset as usize..p.offset as usize + p.len]));
        at += p.len as u64;
    }
    assert_eq!(at, big.len() as u64);
    assert_eq!(cut.sha256, sha256_hex(&big));

    // bytes put in near the start: the chunks after it stay the same
    let mut edited = big[..100_000].to_vec();
    edited.extend_from_slice(b"an edit in the middle of a big file");
    edited.extend_from_slice(&big[100_000..]);
    let path2 = dir.path().join("big2");
    std::fs::write(&path2, &edited).unwrap();
    let cut2 = chunks::cut_blob(&ck, &path2, &mut no_read()).unwrap();
    let before: std::collections::HashSet<&String> = cut.pieces.iter().map(|p| &p.id).collect();
    let same = cut2.pieces.iter().filter(|p| before.contains(&p.id)).count();
    assert!(same + 2 >= cut2.pieces.len(), "{same} of {} chunks kept", cut2.pieces.len());
}

#[test]
fn a_4_mib_blob_zstd_cant_shrink_is_cut_so_each_object_fits() {
    let dir = tempfile::tempdir().unwrap();
    let (_, ck) = keys();
    let data = noise(5, 4 * 1024 * 1024);
    let path = dir.path().join("random");
    std::fs::write(&path, &data).unwrap();
    let cut = chunks::cut_blob(&ck, &path, &mut no_read()).unwrap();
    assert!(cut.pieces.len() > 1, "a whole object would be over {MAX_OBJECT} bytes");
    for p in &cut.pieces {
        let chunk = &data[p.offset as usize..p.offset as usize + p.len];
        let object = ck.seal(&p.id, chunk).unwrap();
        assert!(object.len() <= MAX_OBJECT, "{} bytes", object.len());
    }
    // sealed whole, it would be refused
    let whole_id = ck.id_of(&data);
    assert!(matches!(ck.seal(&whole_id, &data), Err(Error::Local(_))));
}

#[test]
fn an_object_is_hc1_epoch_nonce_and_xchacha20_poly1305_of_zstd_with_the_id_as_associated_data() {
    let (key, ck) = keys();
    let chunk = b"the bytes of one chunk of the database, which zstd shrinks, shrinks, shrinks".repeat(50);
    let id = ck.id_of(&chunk);
    let object = ck.seal(&id, &chunk).unwrap();
    assert_eq!(&object[..3], b"HC1");
    assert_eq!(object[3], 1, "the key's epoch");
    assert!(object.len() < chunk.len(), "compressed before it is sealed");
    // opened as the doc says, without the crate
    let data_key = key.data_key();
    let compressed = XChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(&data_key[..]))
        .decrypt(XNonce::from_slice(&object[4..28]), Payload { msg: &object[28..], aad: id.as_bytes() })
        .expect("the doc's recipe opens it");
    assert_eq!(zstd::decode_all(&compressed[..]).unwrap(), chunk);
    assert_eq!(object.len(), 3 + 1 + 24 + compressed.len() + 16);
    // and by the crate
    assert_eq!(ck.open(&id, &object).unwrap(), chunk);
    // the nonce is random: the same chunk sealed twice gives two objects
    assert_ne!(ck.seal(&id, &chunk).unwrap(), object);
}

#[test]
fn an_object_opens_only_as_the_chunk_it_was_asked_for() {
    let (_, ck) = keys();
    let a = b"chunk a".repeat(100);
    let b = b"chunk b".repeat(100);
    let (ida, idb) = (ck.id_of(&a), ck.id_of(&b));
    let oa = ck.seal(&ida, &a).unwrap();
    // asked for as another chunk
    assert!(matches!(ck.open(&idb, &oa), Err(Error::Damaged(_))));
    // one changed byte anywhere after the header
    for at in [4, 20, 30, oa.len() - 1] {
        let mut changed = oa.clone();
        changed[at] ^= 1;
        assert!(matches!(ck.open(&ida, &changed), Err(Error::Damaged(_))), "byte {at}");
    }
    // another key, another epoch, not an object
    let (_, other) = keys();
    assert!(matches!(other.open(&ida, &oa), Err(Error::Damaged(_))));
    let mut epoch = oa.clone();
    epoch[3] = 2;
    assert!(matches!(ck.open(&ida, &epoch), Err(Error::Damaged(_))));
    assert!(matches!(ck.open(&ida, &oa[..40]), Err(Error::Damaged(_))));
    let mut magic = oa.clone();
    magic[0] = b'X';
    assert!(matches!(ck.open(&ida, &magic), Err(Error::Damaged(_))));
    // an object sealed under an id that isn't its chunk's HMAC doesn't open either
    let lying = ck.seal(&idb, &a).unwrap();
    assert!(matches!(ck.open(&idb, &lying), Err(Error::Damaged(_))));
}
