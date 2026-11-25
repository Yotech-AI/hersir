//! A cloud restore (research 3.6; docs/backup-format.md, For Hersir Cloud): a snapshot comes back from Hersir Cloud as a
//! format-1 folder, which GA-218's restore (`gizai_core::restore`) then takes as it is: the checks, today's data saved
//! first, and the swap at the next start.
//!
//! 1. **downloading**: the snapshot's manifest, opened and checked (it must be the snapshot asked for), then every
//!    object it names, once each, into `<folder>/.cloud-objects/` (an interrupted restore into the same folder picks up
//!    where it was);
//! 2. **checking**: each object is opened (its tag and its chunk's HMAC must match its id), and the chunks put back
//!    together: the database into `snapshots/<name>.db.zst` (zstd level 3, one frame that records its size, as format 1
//!    writes it), each stored blob into `files/<sha256[..2]>/<sha256>`. The database's and each blob's SHA-256 and size
//!    must be the manifest's. Then the manifest is written as format 1's (`snapshots/<name>.json`, without the chunks),
//!    with `gizai-backups.json` if the folder has none, and `.cloud-objects/` goes.
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use gizai_core::backups::{FORMAT_VERSION, MANIFEST_FORMAT, MARKER, MARKER_JSON, Manifest};
use sha2::{Digest, Sha256};

use crate::api::{Api, DownloadUrl, retried};
use crate::backup::{CloudManifest, Progress, Step};
use crate::chunks::{ChunkKeys, ZSTD_LEVEL, is_object_id, short};
use crate::keys::AccountKey;
use crate::util::{clean, is_hex, now_ms};
use crate::{Error, Result};

/// The folder in a restore's folder that holds the objects downloaded so far.
pub const OBJECTS_DIR: &str = ".cloud-objects";
/// How many tries a call or a download gets when it fails on the way (`api::retried`).
const TRIES: usize = 5;
/// Presigned downloads are asked for this many at a time (they last an hour).
const BATCH: usize = 256;

/// A snapshot that came back into a format-1 folder.
#[derive(Debug, Clone)]
pub struct Restored {
    /// The format-1 folder: give it to `gizai_core::restore::check` with `name`.
    pub dir: PathBuf,
    pub name: String,
    /// Its manifest, as written there (format 1's).
    pub manifest: Manifest,
    /// The objects the snapshot uses (its manifest included), and how many were downloaded now and their size.
    pub objects: usize,
    pub downloaded: usize,
    pub downloaded_bytes: u64,
}

/// Restores snapshot `name`, whose manifest is object `manifest_id` (both as `GET /snapshots` lists them), into the
/// format-1 folder `dir` (made when missing), with the account key `key`. `progress` hears each step; an error from it
/// (`Error::Cancelled`) stops the restore there, and what was downloaded stays for the next try.
pub fn restore(api: &Api, key: &AccountKey, name: &str, manifest_id: &str, dir: &Path,
               progress: &mut dyn FnMut(Progress) -> Result<()>) -> Result<Restored> {
    if name.is_empty() || name.len() > 128 || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(Error::Local(format!("\"{}\" isn't a snapshot Hersir can restore: its name has letters, digits and dashes only", clean(name))));
    }
    let manifest_id = manifest_id.trim().to_ascii_lowercase();
    if !is_object_id(&manifest_id) {
        return Err(Error::Damaged(format!("Hersir Cloud lists snapshot {name} with a manifest id that isn't one")));
    }
    let keys = ChunkKeys::new(key);
    let cache = dir.join(OBJECTS_DIR);
    let snaps = dir.join("snapshots");
    fs::create_dir_all(&cache).and_then(|()| fs::create_dir_all(&snaps)).map_err(|e| local(dir, "can't be made", e))?;
    let mut fetched = Fetched::default();

    // 1. The manifest, then every object it names.
    progress(Progress { step: Step::Downloading, done: 0, total: 1 })?;
    let raw = fetch(api, &keys, &cache, &manifest_id, &mut fetched)?;
    let cm: CloudManifest = serde_json::from_slice(&raw).map_err(|e| Error::Damaged(format!("Snapshot {name}'s manifest can't be read: {e}")))?;
    let m = &cm.snapshot;
    if m.format != MANIFEST_FORMAT {
        return Err(Error::Damaged(format!("Snapshot {name}'s manifest isn't a snapshot's")));
    }
    if m.format_version > FORMAT_VERSION {
        return Err(Error::Local(format!("Made by a newer Hersir (backup format {}). Update Hersir first.", m.format_version)));
    }
    if m.name != name {
        return Err(Error::Damaged(format!("Hersir Cloud's manifest for {name} is another snapshot's ({})", clean(&m.name))));
    }
    let missing: HashSet<&String> = m.files_missing.iter().collect();
    let stored: Vec<(&str, i64, &Vec<String>)> = m.files.iter().filter(|f| !missing.contains(&f.sha256))
        .map(|f| {
            let ids = cm.chunks.files.get(&f.sha256)
                .ok_or_else(|| Error::Damaged(format!("Snapshot {name}'s manifest has no chunks for file {}", short(&f.sha256))))?;
            if !is_hex(&f.sha256, 64) {
                return Err(Error::Damaged(format!("Snapshot {name}'s manifest names a file \"{}\" that isn't a file's hash", clean(&f.sha256))));
            }
            Ok((f.sha256.as_str(), f.bytes, ids))
        })
        .collect::<Result<_>>()?;
    let mut all: Vec<&String> = Vec::new();
    let mut seen: HashSet<&String> = HashSet::new();
    for id in cm.chunks.db.iter().chain(stored.iter().flat_map(|(_, _, ids)| ids.iter())) {
        if !is_object_id(id) {
            return Err(Error::Damaged(format!("Snapshot {name}'s manifest names an object \"{}\" that isn't one", clean(id))));
        }
        if seen.insert(id) {
            all.push(id);
        }
    }
    if cm.chunks.db.is_empty() {
        return Err(Error::Damaged(format!("Snapshot {name}'s manifest has no database")));
    }
    let total = all.len() as u64 + 1;
    let mut done = 1u64;
    progress(Progress { step: Step::Downloading, done, total })?;
    let wanted: Vec<String> = all.iter().filter(|id| !cache.join(id.as_str()).is_file()).map(|id| id.to_string()).collect();
    done += (all.len() - wanted.len()) as u64;
    progress(Progress { step: Step::Downloading, done, total })?;
    for batch in wanted.chunks(BATCH) {
        let urls = retried(TRIES, || api.download_urls(batch))?;
        for id in batch {
            let url = urls.iter().find(|u| u.id.eq_ignore_ascii_case(id))
                .ok_or_else(|| Error::Damaged(format!("Hersir Cloud gave no download for object {}", short(id))))?;
            let object = download(api, url)?;
            fetched.objects += 1;
            fetched.bytes += object.len() as u64;
            keep(&cache, id, &object)?;
            done += 1;
            progress(Progress { step: Step::Downloading, done, total })?;
        }
    }

    // 2. Checking: each object opened, the database and the blobs put back together and checked.
    let total = m.db.db_bytes.max(0) as u64 + stored.iter().map(|(_, bytes, _)| (*bytes).max(0) as u64).sum::<u64>();
    let mut done = 0u64;
    progress(Progress { step: Step::Checking, done, total })?;
    let zst = snaps.join(format!("{name}.db.zst"));
    let zst_part = snaps.join(format!("{name}.db.zst.part"));
    let db_written = (|| -> Result<(String, u64)> {
        let out = File::create(&zst_part).map_err(|e| local(&zst_part, "can't be written", e))?;
        let mut enc = zstd::stream::write::Encoder::new(out, ZSTD_LEVEL).map_err(|e| local(&zst_part, "can't be written", e))?;
        enc.set_pledged_src_size(Some(m.db.db_bytes.max(0) as u64)).map_err(|e| local(&zst_part, "can't be written", e))?;
        enc.include_contentsize(true).map_err(|e| local(&zst_part, "can't be written", e))?;
        let mut hash = Sha256::new();
        let mut size = 0u64;
        for id in &cm.chunks.db {
            let chunk = open_kept(&keys, &cache, id)?;
            hash.update(&chunk);
            size += chunk.len() as u64;
            if size > m.db.db_bytes.max(0) as u64 {
                return Err(Error::Damaged(format!("Snapshot {name}'s database is larger than its manifest says")));
            }
            enc.write_all(&chunk).map_err(|e| local(&zst_part, "can't be written", e))?;
            done += chunk.len() as u64;
            progress(Progress { step: Step::Checking, done, total: total.max(done) })?;
        }
        if size != m.db.db_bytes.max(0) as u64 {
            return Err(Error::Damaged(format!("Snapshot {name}'s database is {size} bytes, not {} as its manifest says", m.db.db_bytes)));
        }
        let out = enc.finish().map_err(|e| local(&zst_part, "can't be written", e))?;
        out.sync_all().map_err(|e| local(&zst_part, "can't be written", e))?;
        Ok((format!("{:x}", hash.finalize()), size))
    })();
    let db_sha = match db_written {
        Ok((sha, _)) => sha,
        Err(e) => {
            let _ = fs::remove_file(&zst_part);
            return Err(e);
        }
    };
    if db_sha != m.db.sha256.to_ascii_lowercase() {
        let _ = fs::remove_file(&zst_part);
        return Err(Error::Damaged(format!("Snapshot {name}'s database doesn't match its manifest's SHA-256")));
    }
    fs::rename(&zst_part, &zst).map_err(|e| local(&zst, "can't be written", e))?;
    for (sha, bytes, ids) in &stored {
        let dest = dir.join("files").join(&sha[..2]).join(sha);
        let part = dest.with_file_name(format!("{sha}.part"));
        let written = (|| -> Result<()> {
            fs::create_dir_all(dest.parent().unwrap_or(dir)).map_err(|e| local(dir, "can't be written", e))?;
            let mut out = File::create(&part).map_err(|e| local(&part, "can't be written", e))?;
            let mut hash = Sha256::new();
            let mut size = 0u64;
            for id in ids.iter() {
                let chunk = open_kept(&keys, &cache, id)?;
                hash.update(&chunk);
                size += chunk.len() as u64;
                out.write_all(&chunk).map_err(|e| local(&part, "can't be written", e))?;
                done += chunk.len() as u64;
            progress(Progress { step: Step::Checking, done, total: total.max(done) })?;
            }
            out.sync_all().map_err(|e| local(&part, "can't be written", e))?;
            if format!("{:x}", hash.finalize()) != *sha || size as i64 != *bytes {
                return Err(Error::Damaged(format!("File {} of snapshot {name} doesn't match its manifest", short(sha))));
            }
            Ok(())
        })();
        if let Err(e) = written {
            let _ = fs::remove_file(&part);
            return Err(e);
        }
        fs::rename(&part, &dest).map_err(|e| local(&dest, "can't be written", e))?;
    }

    // The folder: format 1's marker and manifest, and the downloads go.
    let marker = dir.join(MARKER);
    if !marker.exists() {
        fs::write(&marker, MARKER_JSON).map_err(|e| local(&marker, "can't be written", e))?;
    }
    let mut manifest = cm.snapshot.clone();
    manifest.db.path = format!("snapshots/{name}.db.zst");
    manifest.db.compression = "zstd".into();
    manifest.db.bytes = fs::metadata(&zst).map(|md| md.len() as i64).map_err(|e| local(&zst, "can't be read", e))?;
    let mut json = serde_json::to_vec_pretty(&manifest).map_err(|e| Error::Local(format!("The manifest couldn't be written: {e}")))?;
    json.push(b'\n');
    let json_path = snaps.join(format!("{name}.json"));
    let json_part = snaps.join(format!("{name}.json.part"));
    fs::write(&json_part, &json).and_then(|()| fs::rename(&json_part, &json_path)).map_err(|e| local(&json_path, "can't be written", e))?;
    let _ = fs::remove_dir_all(&cache);
    Ok(Restored {
        dir: dir.to_path_buf(),
        name: name.to_string(),
        manifest,
        objects: all.len() + 1,
        downloaded: fetched.objects,
        downloaded_bytes: fetched.bytes,
    })
}

/// What a restore downloaded: objects and their size.
#[derive(Debug, Default)]
struct Fetched {
    objects: usize,
    bytes: u64,
}

/// One object, opened: from the folder's downloads when it is there (and opens), else downloaded now and kept.
fn fetch(api: &Api, keys: &ChunkKeys, cache: &Path, id: &str, fetched: &mut Fetched) -> Result<Vec<u8>> {
    if cache.join(id).is_file()
        && let Ok(chunk) = open_kept(keys, cache, id)
    {
        return Ok(chunk);
    }
    let urls = retried(TRIES, || api.download_urls(&[id.to_string()]))?;
    let url = urls.iter().find(|u| u.id.eq_ignore_ascii_case(id))
        .ok_or_else(|| Error::Damaged(format!("Hersir Cloud gave no download for object {}", short(id))))?;
    let object = download(api, url)?;
    fetched.objects += 1;
    fetched.bytes += object.len() as u64;
    let chunk = keys.open(id, &object)?;
    keep(cache, id, &object)?;
    Ok(chunk)
}

/// One object from its presigned URL, trying again when it fails on the way, with a new URL when this one has run out
/// (or is about to) or R2 refused it.
fn download(api: &Api, url: &DownloadUrl) -> Result<Vec<u8>> {
    let mut url = url.clone();
    let mut left = TRIES;
    loop {
        if url.expires_at - 10_000 < now_ms() {
            let fresh = retried(TRIES, || api.download_urls(std::slice::from_ref(&url.id)))?;
            url = fresh.into_iter().find(|u| u.id.eq_ignore_ascii_case(&url.id))
                .ok_or_else(|| Error::Damaged(format!("Hersir Cloud gave no download for object {}", short(&url.id))))?;
        }
        match api.get_object(&url) {
            Ok(object) => return Ok(object),
            Err(e @ (Error::Network(_) | Error::Storage(_))) if left > 1 => {
                left -= 1;
                // a refused download may be a URL that ran out: the next try asks for a new one
                if matches!(e, Error::Storage(_)) {
                    url.expires_at = 0;
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            Err(e) => return Err(e),
        }
    }
}

/// Keeps a downloaded object in the folder's downloads: through a `.part` file, so only whole ones are there.
fn keep(cache: &Path, id: &str, object: &[u8]) -> Result<()> {
    let path = cache.join(id);
    let part = cache.join(format!("{id}.part"));
    fs::write(&part, object).and_then(|()| fs::rename(&part, &path)).map_err(|e| local(&path, "can't be written", e))
}

/// Opens a downloaded object. One that doesn't open is removed, so the next try downloads it again.
fn open_kept(keys: &ChunkKeys, cache: &Path, id: &str) -> Result<Vec<u8>> {
    let path = cache.join(id);
    let object = fs::read(&path).map_err(|e| local(&path, "can't be read", e))?;
    keys.open(id, &object).inspect_err(|_| {
        let _ = fs::remove_file(&path);
    })
}

fn local(path: &Path, what: &str, e: io::Error) -> Error {
    Error::Local(format!("{} {what}: {e}", path.display()))
}
