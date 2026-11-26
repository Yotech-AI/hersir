//! A cloud backup (research 3.6; docs/backup-format.md, For Hersir Cloud): a snapshot of the database and the blobs it
//! names, in chunks, each sealed on this computer and uploaded straight to R2, then registered with Hersir Cloud.
//!
//! 1. **chunking**: the database copy (page for page, which the caller makes with SQLite's backup API, so unchanged
//!    pages stay where they were) is cut into 256 KiB chunks, and each blob as `chunks::cut_blob` says; each chunk is
//!    named by its keyed hash. A blob that isn't where it should be, or no longer matches its hash, goes into the
//!    manifest's `files_missing`, as format 1 does.
//! 2. **uploading**: the manifest (format 1's, plus the ordered chunk ids of the database and of each blob) is sealed
//!    like a chunk; `POST /objects/missing` says which objects the account doesn't have, and only those are sealed and
//!    uploaded, a batch at a time (`POST /objects/upload`, then a `PUT` of each to its presigned URL). A second backup
//!    of an unchanged database uploads only its manifest.
//! 3. **registering**: `POST /snapshots` with every object the snapshot uses. On 422 `missing_objects` those objects
//!    are uploaded again, and the snapshot registered again.
//!
//! A 429, a server error or a network failure on the way is waited out and tried again a few times; any other error
//! stops the backup. The progress callback hears each step (step, done, total); an error from it stops the backup.
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::path::PathBuf;

use gizai_core::backups::{DbFile, Device, FORMAT_VERSION, FileEntry, LastChange, MANIFEST_FORMAT, Manifest};
use serde::{Deserialize, Serialize};

use crate::api::{Api, NewSnapshot, Snapshot, UploadUrl, retried};
use crate::chunks::{self, ChunkKeys, MAX_CHUNK, Piece, short};
use crate::keys::AccountKey;
use crate::util::{clean, is_hex, now_ms};
use crate::{Error, Result};

/// The reasons Hersir Cloud takes for a snapshot.
pub const REASONS: [&str; 4] = ["daily", "manual", "quit", "before-restore"];
/// How many tries a call or an upload gets when it fails on the way (`api::retried`).
const TRIES: usize = 5;
/// Objects are uploaded in batches of at most this many…
const BATCH_OBJECTS: usize = 256;
/// …and at most this many bytes of chunks: a batch's presigned URLs last 10 minutes.
const BATCH_BYTES: u64 = 32 << 20;
/// How many times a snapshot is registered again after `missing_objects`.
const REGISTER_ROUNDS: usize = 3;

/// A step of a cloud backup or restore, as the progress callback hears it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Step {
    /// A backup cuts the database and the blobs into chunks: `done` and `total` count their bytes read.
    Chunking,
    /// A backup uploads the objects the account doesn't have: `done` and `total` count their chunks' bytes.
    Uploading,
    /// A backup registers the snapshot: 0 of 1, then 1 of 1.
    Registering,
    /// A restore downloads the manifest and the snapshot's objects: `done` and `total` count objects.
    Downloading,
    /// A restore opens and checks each object and writes the database and the blobs: `done` and `total` count their
    /// bytes.
    Checking,
}

impl Step {
    /// `chunking`, `uploading`, `registering`, `downloading` or `checking`.
    pub fn as_str(self) -> &'static str {
        match self {
            Step::Chunking => "chunking",
            Step::Uploading => "uploading",
            Step::Registering => "registering",
            Step::Downloading => "downloading",
            Step::Checking => "checking",
        }
    }
}

/// Where a backup or restore is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub step: Step,
    pub done: u64,
    pub total: u64,
}

/// What a backup sends: format 1's fields for the manifest, the database copy, and the blobs it names.
#[derive(Debug, Clone)]
pub struct Source {
    /// The snapshot's name, as format 1 makes it (`gizai_core::backups::snapshot_name`): letters, digits and dashes.
    pub name: String,
    /// `daily`, `manual`, `quit` or `before-restore`.
    pub reason: String,
    /// When the database was copied (ms).
    pub made_at: i64,
    pub app_version: String,
    /// The copy's `PRAGMA user_version`.
    pub schema_version: i64,
    /// This computer: its `devices` row with `is_self = 1`.
    pub device: Device,
    /// The copy's newest `changes` row, None when it has none.
    pub last_change: Option<LastChange>,
    /// The page-for-page copy of the database (SQLite's backup API), one file, nothing beside it.
    pub db: PathBuf,
    /// Every blob the copy names, as format 1's `files` lists them.
    pub files: Vec<Blob>,
}

/// A blob the database names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blob {
    /// Its name: the SHA-256 of its content, 64 hex.
    pub sha256: String,
    /// The size the database gives it (a `files` row's `size_bytes`): what the manifest says for a missing one.
    pub bytes: Option<i64>,
    /// Where it is: `<data folder>/files/<sha256[..2]>/<sha256>` (`gizai_core::files::blob_path`).
    pub path: PathBuf,
}

/// A cloud snapshot's manifest: format 1's (docs/backup-format.md), and the chunks of its database and blobs. In the
/// cloud, `db.bytes` is 0 (the database isn't one compressed file there); a restore writes the `.db.zst` and its size.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CloudManifest {
    #[serde(flatten)]
    pub snapshot: Manifest,
    #[serde(default)]
    pub chunks: Chunks,
}

/// The chunks of a snapshot, by their ids, in order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Chunks {
    /// The database's: 256 KiB each, the last one shorter. Put together, they are the database.
    pub db: Vec<String>,
    /// Each stored blob's, by its sha256 (the blobs in `files_missing` have none). An empty blob has an empty list.
    pub files: BTreeMap<String, Vec<String>>,
}

/// A backup that was made.
#[derive(Debug, Clone)]
pub struct BackedUp {
    /// The snapshot as Hersir Cloud registered it.
    pub snapshot: Snapshot,
    pub manifest: CloudManifest,
    pub manifest_id: String,
    /// The objects the snapshot uses, its manifest included.
    pub objects: usize,
    /// The objects uploaded now, its manifest included, and their size.
    pub uploaded: usize,
    pub uploaded_bytes: u64,
}

/// Where a chunk's bytes are: in which file (0 is the database, then the blobs), at which place.
#[derive(Debug, Clone)]
struct Place {
    file: usize,
    piece: Piece,
}

/// The chunks a backup can upload, by id, and the files they are in.
struct Sources<'a> {
    keys: &'a ChunkKeys,
    files: Vec<PathBuf>,
    places: HashMap<String, Place>,
    manifest_id: String,
    manifest_object: Vec<u8>,
    manifest_len: u64,
}

impl Sources<'_> {
    /// The size of a chunk (or the manifest) before it is sealed.
    fn len(&self, id: &str) -> u64 {
        if id == self.manifest_id { self.manifest_len } else { self.places.get(id).map_or(0, |p| p.piece.len as u64) }
    }

    /// The object for `id`, sealed now (the manifest was sealed already).
    fn object(&self, id: &str, open: &mut HashMap<usize, File>) -> Result<Vec<u8>> {
        if id == self.manifest_id {
            return Ok(self.manifest_object.clone());
        }
        let place = self.places.get(id).ok_or_else(|| Error::Local(format!("Hersir Cloud asked for an object this backup doesn't have ({})", short(id))))?;
        let file = match open.entry(place.file) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => {
                let path = &self.files[place.file];
                e.insert(File::open(path).map_err(|err| Error::Local(format!("{} can't be read: {err}", path.display())))?)
            }
        };
        let chunk = chunks::read_piece(self.keys, file, &place.piece)?;
        self.keys.seal(id, &chunk)
    }
}

/// Backs up `src` to Hersir Cloud with the account key `key`: see the module. `progress` hears each step; an error from
/// it (`Error::Cancelled`) stops the backup there.
pub fn back_up(api: &Api, key: &AccountKey, src: &Source, progress: &mut dyn FnMut(Progress) -> Result<()>) -> Result<BackedUp> {
    check_source(src)?;
    let keys = ChunkKeys::new(key);

    // 1. Chunking: the database, then each blob once, by hash.
    let mut blobs: BTreeMap<String, &Blob> = BTreeMap::new();
    for b in &src.files {
        blobs.entry(b.sha256.trim().to_ascii_lowercase()).or_insert(b);
    }
    let size = |p: &PathBuf| fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    let total = size(&src.db) + blobs.values().map(|b| size(&b.path)).sum::<u64>();
    let mut done = 0u64;
    progress(Progress { step: Step::Chunking, done, total })?;
    let mut read = |n: u64| {
        done += n;
        progress(Progress { step: Step::Chunking, done, total: total.max(done) })
    };
    let db = chunks::cut_db(&keys, &src.db, &mut read)?;
    if db.bytes == 0 {
        return Err(Error::Local(format!("The database copy {} is empty", src.db.display())));
    }
    let mut files = Vec::with_capacity(blobs.len());
    let mut files_missing = Vec::new();
    let mut stored: Vec<(String, chunks::Cut, PathBuf)> = Vec::new();
    for (sha, b) in &blobs {
        let cut = if b.path.is_file() { Some(chunks::cut_blob(&keys, &b.path, &mut read)?) } else { None };
        match cut {
            Some(cut) if cut.sha256 == *sha => {
                files.push(FileEntry { sha256: sha.clone(), bytes: cut.bytes as i64 });
                stored.push((sha.clone(), cut, b.path.clone()));
            }
            _ => {
                files.push(FileEntry { sha256: sha.clone(), bytes: b.bytes.unwrap_or(0) });
                files_missing.push(sha.clone());
            }
        }
    }
    drop(read);

    // 2. The manifest, sealed like a chunk.
    let mut chunk_files = BTreeMap::new();
    for (sha, cut, _) in &stored {
        chunk_files.insert(sha.clone(), cut.pieces.iter().map(|p| p.id.clone()).collect::<Vec<_>>());
    }
    let manifest = CloudManifest {
        snapshot: Manifest {
            format: MANIFEST_FORMAT.into(),
            format_version: FORMAT_VERSION,
            name: src.name.clone(),
            reason: src.reason.clone(),
            made_at: src.made_at,
            app_version: src.app_version.clone(),
            schema_version: src.schema_version,
            device: src.device.clone(),
            db: DbFile {
                path: format!("snapshots/{}.db.zst", src.name),
                compression: "zstd".into(),
                bytes: 0,
                sha256: db.sha256.clone(),
                db_bytes: db.bytes as i64,
            },
            last_change: src.last_change.clone(),
            files,
            files_missing,
        },
        chunks: Chunks { db: db.pieces.iter().map(|p| p.id.clone()).collect(), files: chunk_files },
    };
    let json = serde_json::to_vec(&manifest).map_err(|e| Error::Local(format!("The backup's manifest couldn't be written: {e}")))?;
    if json.len() > MAX_CHUNK {
        return Err(Error::Local(format!("The backup's manifest is {} bytes, more than the {MAX_CHUNK} one object may hold", json.len())));
    }
    let manifest_id = keys.id_of(&json);
    let manifest_object = keys.seal(&manifest_id, &json)?;

    // Every chunk once, where it first is.
    let mut paths = vec![src.db.clone()];
    let mut places: HashMap<String, Place> = HashMap::new();
    let mut ids: Vec<String> = Vec::new();
    let mut add = |file: usize, pieces: &[Piece], places: &mut HashMap<String, Place>| {
        for p in pieces {
            if !places.contains_key(&p.id) {
                ids.push(p.id.clone());
                places.insert(p.id.clone(), Place { file, piece: p.clone() });
            }
        }
    };
    add(0, &db.pieces, &mut places);
    for (_, cut, path) in &stored {
        paths.push(path.clone());
        add(paths.len() - 1, &cut.pieces, &mut places);
    }
    if !places.contains_key(&manifest_id) {
        ids.push(manifest_id.clone());
    }
    let sources = Sources { keys: &keys, files: paths, places, manifest_id: manifest_id.clone(), manifest_object, manifest_len: json.len() as u64 };

    // 3. Uploading: only what the account doesn't have.
    let missing = retried(TRIES, || api.missing(&ids))?;
    let known: HashSet<&String> = ids.iter().collect();
    let mut wanted: Vec<String> = Vec::new();
    for id in missing {
        if known.contains(&id) && !wanted.contains(&id) {
            wanted.push(id);
        }
    }
    let total = wanted.iter().map(|id| sources.len(id)).sum::<u64>();
    let mut sent = Sent::default();
    progress(Progress { step: Step::Uploading, done: 0, total })?;
    upload(api, &sources, &wanted, &mut sent, &mut |done| progress(Progress { step: Step::Uploading, done, total: total.max(done) }))?;

    // 4. Registering, uploading again what the server says it doesn't have.
    let stored_bytes: u64 = stored.iter().map(|(_, cut, _)| cut.bytes).sum();
    let new = NewSnapshot {
        name: src.name.clone(),
        made_at: src.made_at,
        device_id: src.device.id.clone(),
        reason: src.reason.clone(),
        schema_version: src.schema_version,
        app_version: src.app_version.clone(),
        manifest_id: manifest_id.clone(),
        object_ids: ids.clone(),
        bytes: (db.bytes + stored_bytes) as i64,
    };
    progress(Progress { step: Step::Registering, done: 0, total: 1 })?;
    let mut rounds = 0;
    let snapshot = loop {
        match retried(TRIES, || api.create_snapshot(&new)) {
            Ok(s) => break s,
            Err(Error::MissingObjects { ids: again, message }) if rounds < REGISTER_ROUNDS => {
                rounds += 1;
                let again: Vec<String> = again.into_iter().filter(|id| known.contains(id)).collect();
                if again.is_empty() {
                    return Err(Error::MissingObjects { message, ids: vec![] });
                }
                let total = again.iter().map(|id| sources.len(id)).sum::<u64>();
                upload(api, &sources, &again, &mut sent, &mut |done| progress(Progress { step: Step::Uploading, done, total: total.max(done) }))?;
                progress(Progress { step: Step::Registering, done: 0, total: 1 })?;
            }
            Err(e) => return Err(e),
        }
    };
    progress(Progress { step: Step::Registering, done: 1, total: 1 })?;
    Ok(BackedUp { snapshot, manifest, manifest_id, objects: ids.len(), uploaded: sent.objects, uploaded_bytes: sent.bytes })
}

/// What a backup uploaded: objects and their sealed size.
#[derive(Debug, Default)]
struct Sent {
    objects: usize,
    bytes: u64,
}

/// Seals and uploads the objects `ids`, a batch at a time: each batch sealed, its presigned URLs asked for, then each
/// object `PUT`. An object the account has stored already gets no URL and is passed over. `done` hears how many of the
/// chunks' bytes are done.
fn upload(api: &Api, sources: &Sources, ids: &[String], sent: &mut Sent, done: &mut dyn FnMut(u64) -> Result<()>) -> Result<()> {
    let mut so_far = 0u64;
    let mut open: HashMap<usize, File> = HashMap::new();
    let mut i = 0;
    while i < ids.len() {
        // a batch: up to BATCH_OBJECTS objects and BATCH_BYTES of chunks (at least one)
        let mut batch: Vec<(String, Vec<u8>)> = Vec::new();
        let mut bytes = 0u64;
        while i < ids.len() && batch.len() < BATCH_OBJECTS && (batch.is_empty() || bytes + sources.len(&ids[i]) <= BATCH_BYTES) {
            bytes += sources.len(&ids[i]);
            batch.push((ids[i].clone(), sources.object(&ids[i], &mut open)?));
            i += 1;
        }
        let asked: Vec<(String, u64)> = batch.iter().map(|(id, o)| (id.clone(), o.len() as u64)).collect();
        let urls = retried(TRIES, || api.upload_urls(&asked))?;
        let mut by_id: HashMap<String, UploadUrl> = urls.into_iter().map(|u| (u.id.to_ascii_lowercase(), u)).collect();
        for (id, object) in &batch {
            if let Some(url) = by_id.remove(id) {
                put(api, url, id, object)?;
                sent.objects += 1;
                sent.bytes += object.len() as u64;
            }
            so_far += sources.len(id);
            done(so_far)?;
        }
    }
    Ok(())
}

/// `PUT`s one object, trying again when it fails on the way, with a new URL when this one has run out (or is about to).
fn put(api: &Api, mut url: UploadUrl, id: &str, object: &[u8]) -> Result<()> {
    let mut left = TRIES;
    loop {
        if url.expires_at - 10_000 < now_ms() {
            let fresh = retried(TRIES, || api.upload_urls(&[(id.to_string(), object.len() as u64)]))?;
            match fresh.into_iter().find(|u| u.id.eq_ignore_ascii_case(id)) {
                Some(u) => url = u,
                // stored meanwhile: nothing to upload
                None => return Ok(()),
            }
        }
        match api.put_object(&url, object) {
            Ok(()) => return Ok(()),
            Err(e @ (Error::Network(_) | Error::Storage(_))) if left > 1 => {
                left -= 1;
                // a refused upload may be a URL that ran out: the next try asks for a new one
                if matches!(e, Error::Storage(_)) {
                    url.expires_at = 0;
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            Err(e) => return Err(e),
        }
    }
}

/// The source is one Hersir Cloud and format 1 take: a name of letters, digits and dashes (at most 128, from a letter
/// or digit), a reason the server takes, a device, and blobs named by their sha256.
fn check_source(src: &Source) -> Result<()> {
    let name = &src.name;
    if name.is_empty() || name.len() > 128 || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        || !name.starts_with(|c: char| c.is_ascii_alphanumeric()) {
        return Err(Error::Local(format!("\"{}\" can't be a cloud snapshot's name: letters, digits and dashes only", clean(name))));
    }
    if !REASONS.contains(&src.reason.as_str()) {
        return Err(Error::Local(format!("Hersir Cloud doesn't take \"{}\" as a reason for a snapshot", clean(&src.reason))));
    }
    if src.device.id.trim().is_empty() {
        return Err(Error::Local("A snapshot needs this computer's device id".to_string()));
    }
    if let Some(b) = src.files.iter().find(|b| !is_hex(&b.sha256.trim().to_ascii_lowercase(), 64)) {
        return Err(Error::Local(format!("\"{}\" isn't a file's hash", clean(&b.sha256))));
    }
    if !src.db.is_file() {
        return Err(Error::Local(format!("The database copy {} isn't there", src.db.display())));
    }
    Ok(())
}
