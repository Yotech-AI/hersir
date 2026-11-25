//! What the tests share: a fake Hersir Cloud with this computer signed in, an account key, a SQLite database copy and
//! blob files to back up.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gizai_cloud_client::api::{NewDevice, Registered};
use gizai_cloud_client::backup::{Blob, Progress, Source};
use gizai_cloud_client::fake::FakeCloud;
use gizai_cloud_client::keys::{self, AccountKey, DeviceKeys, RecoveryCode};
use gizai_cloud_client::Api;
use sha2::{Digest, Sha256};

/// The schema version the test databases carry (`PRAGMA user_version`): at most gizai-core's, so format 1's restore
/// takes them.
pub const SCHEMA: i64 = 7;

/// A device id (a UUID) no other test in this process uses.
pub fn device_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    format!("0b5e1c2d-7a3f-4c8e-9d1b-{n:012x}")
}

/// This computer, as `POST /devices` takes it.
pub fn new_device(id: &str, name: &str) -> NewDevice {
    let keys = DeviceKeys::generate().expect("device keys");
    NewDevice {
        device_id: id.to_string(),
        name: name.to_string(),
        platform: "linux".into(),
        app_version: "0.20.1".into(),
        sign_pub: keys.sign_pub_b64(),
        box_pub: keys.box_pub_b64(),
        kind: "desktop".into(),
    }
}

/// A fake Hersir Cloud with one computer of its first account signed in.
pub struct Cloud {
    pub fake: FakeCloud,
    pub api: Api,
    pub device_id: String,
    pub registered: Registered,
}

impl Cloud {
    pub fn account_id(&self) -> String {
        self.registered.account.id.clone()
    }
}

/// Starts a fake and registers a computer on its first account with a browser token.
pub fn cloud() -> Cloud {
    let fake = FakeCloud::start().expect("the fake Hersir Cloud starts");
    let account = fake.account();
    let device_id = device_id();
    let registered = Api::register(fake.url(), &fake.browser_token(&account.id), &new_device(&device_id, "QA laptop"))
        .expect("POST /devices");
    let api = Api::new(fake.url(), &registered.device_token).expect("the API");
    Cloud { fake, api, device_id, registered }
}

/// A signed-in fake whose account has a key: the account key, the recovery code, and the API.
pub struct Keyed {
    pub cloud: Cloud,
    pub key: AccountKey,
    pub code: RecoveryCode,
}

pub fn keyed() -> Keyed {
    let cloud = cloud();
    let (key, code, record) = keys::new_account_key().expect("a new account key");
    let (stored, first) = cloud.api.put_key(&record, None).expect("PUT /keys");
    assert!(first, "the account's first key is created (201)");
    assert_eq!(stored, record);
    Keyed { cloud, key, code }
}

/// Deterministic bytes that zstd can't shrink (xorshift64*).
pub fn noise(seed: u64, len: usize) -> Vec<u8> {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut out = Vec::with_capacity(len + 8);
    while out.len() < len {
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        out.extend_from_slice(&x.wrapping_mul(0x2545_F491_4F6C_DD1D).to_le_bytes());
    }
    out.truncate(len);
    out
}

/// Text that reads like notes, each row different, with `marker` in each.
pub fn note(marker: &str, i: usize) -> String {
    const WORDS: [&str; 12] = ["harbour", "lantern", "meadow", "copper", "willow", "ember", "quarry", "saffron", "tundra", "orchid", "falcon", "basalt"];
    let mut s = format!("{marker} row {i}:");
    for k in 0..24 {
        s.push(' ');
        s.push_str(WORDS[(i * 7 + k * 5 + i / 3) % WORDS.len()]);
    }
    s
}

/// A closed SQLite database (journal mode DELETE, `user_version` `SCHEMA`) of about `rows` notes with `marker` in each,
/// as the caller's page-for-page copy would be.
pub fn make_db(path: &Path, rows: usize, marker: &str) {
    let conn = rusqlite::Connection::open(path).expect("open the database");
    let mode: String = conn.query_row("PRAGMA journal_mode=DELETE", [], |r| r.get(0)).expect("journal mode");
    assert_eq!(mode, "delete");
    conn.pragma_update(None, "user_version", SCHEMA).expect("user_version");
    conn.execute_batch("CREATE TABLE notes(id INTEGER PRIMARY KEY, body TEXT NOT NULL);").expect("the table");
    let tx = conn.unchecked_transaction().expect("a transaction");
    {
        let mut st = tx.prepare("INSERT INTO notes(body) VALUES (?1)").expect("insert");
        for i in 0..rows {
            st.execute([note(marker, i)]).expect("a row");
        }
    }
    tx.commit().expect("commit");
    conn.close().expect("close");
}

/// Changes one row of a closed database in place (the same pages, one of them changed).
pub fn change_row(path: &Path, id: i64, body: &str) {
    let conn = rusqlite::Connection::open(path).expect("open the database");
    conn.execute("UPDATE notes SET body = ?1 WHERE id = ?2", rusqlite::params![body, id]).expect("update");
    conn.close().expect("close");
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Writes a blob where the data folder keeps it, `<dir>/files/<sha[..2]>/<sha>`, and returns it as a backup names it.
pub fn blob(data_dir: &Path, bytes: &[u8]) -> Blob {
    let sha = sha256_hex(bytes);
    let path = data_dir.join("files").join(&sha[..2]).join(&sha);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, bytes).unwrap();
    Blob { sha256: sha, bytes: Some(bytes.len() as i64), path }
}

/// A backup of `db` and `files` by this computer, named `name`.
pub fn source(cloud: &Cloud, name: &str, db: &Path, files: Vec<Blob>) -> Source {
    Source {
        name: name.to_string(),
        reason: "manual".into(),
        made_at: 1_795_633_200_000,
        app_version: "0.20.1".into(),
        schema_version: SCHEMA,
        device: gizai_core::backups::Device { id: cloud.device_id.clone(), name: "QA laptop".into() },
        last_change: None,
        db: db.to_path_buf(),
        files,
    }
}

/// A progress callback that keeps what it hears.
pub fn record(into: &mut Vec<Progress>) -> impl FnMut(Progress) -> gizai_cloud_client::Result<()> + '_ {
    move |p| {
        into.push(p);
        Ok(())
    }
}

pub fn quiet(_: Progress) -> gizai_cloud_client::Result<()> {
    Ok(())
}

/// The database's 256 KiB chunks, as format 1's cloud backup cuts them.
pub fn db_chunks(bytes: &[u8]) -> Vec<&[u8]> {
    bytes.chunks(256 * 1024).collect()
}

/// A folder under the test's temp folder.
pub fn sub(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// Whether `needle` is somewhere in `hay`.
pub fn contains(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}
