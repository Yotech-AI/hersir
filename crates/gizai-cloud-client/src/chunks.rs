//! A backup's chunks and the objects they become (docs/backup-format.md, For Hersir Cloud; research 3.6):
//!
//! - the database (a page-for-page copy) is cut into fixed 256 KiB chunks, the last one shorter;
//! - a blob up to 4 MiB is one chunk; a bigger one is cut by content (FastCDC 2020: at least 512 KiB, about 1 MiB, at
//!   most 2 MiB), so a changed part of a big file changes only the chunks around it;
//! - a chunk's id is HMAC-SHA256(id key, its bytes) as 64 lowercase hex characters, never its plain SHA-256: the server
//!   can't tell which known files someone has;
//! - its object is `HC1` ‖ the key's epoch (1 byte) ‖ a random 24-byte nonce ‖ XChaCha20-Poly1305(data key, nonce,
//!   zstd(chunk)) with the id's 64 characters as associated data: at most 4,194,368 bytes. An object is opened only
//!   when it is the one asked for, and its chunk's HMAC is checked again.
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::keys::AccountKey;
use crate::util::{hex, is_hex, random};
use crate::{Error, Result};

/// The database's chunks: 256 KiB (64 pages of 4 KiB).
pub const DB_CHUNK: usize = 256 * 1024;
/// A blob up to this size is one chunk.
pub const WHOLE_BLOB: u64 = 4 * 1024 * 1024;
/// A bigger blob is cut by content (FastCDC 2020, normalization level 1): chunks of at least…
pub const CDC_MIN: usize = 512 * 1024;
/// …about…
pub const CDC_AVG: usize = 1024 * 1024;
/// …and at most this size.
pub const CDC_MAX: usize = 2 * 1024 * 1024;
/// The most an object may be: 4 MiB and the overhead.
pub const MAX_OBJECT: usize = 4_194_368;
/// The most a chunk may be, and so the most an object opens to.
pub const MAX_CHUNK: usize = 4 * 1024 * 1024;
/// An object's first bytes.
pub const OBJECT_MAGIC: &[u8; 3] = b"HC1";
/// An object's bytes besides the compressed chunk: magic (3), epoch (1), nonce (24) and tag (16).
pub const OBJECT_OVERHEAD: usize = 3 + 1 + 24 + 16;
/// zstd's level for a chunk: format 1's.
pub const ZSTD_LEVEL: i32 = 3;

/// The keys a backup's chunks need: the data key and the id key of one account key (and its epoch).
pub struct ChunkKeys {
    epoch: u8,
    data: Zeroizing<[u8; 32]>,
    id: Zeroizing<[u8; 32]>,
}

impl std::fmt::Debug for ChunkKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChunkKeys").field("epoch", &self.epoch).finish_non_exhaustive()
    }
}

impl ChunkKeys {
    pub fn new(key: &AccountKey) -> ChunkKeys {
        ChunkKeys { epoch: key.epoch(), data: key.data_key(), id: key.id_key() }
    }

    pub fn epoch(&self) -> u8 {
        self.epoch
    }

    /// A chunk's id: HMAC-SHA256(id key, its bytes), 64 lowercase hex characters.
    pub fn id_of(&self, chunk: &[u8]) -> String {
        hex(&self.mac(chunk))
    }

    fn mac(&self, chunk: &[u8]) -> [u8; 32] {
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&self.id[..]).expect("HMAC takes a key of any size");
        mac.update(chunk);
        mac.finalize().into_bytes().into()
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(&self.data[..]))
    }

    /// Seals chunk `chunk` whose id is `id` into its object: `HC1` ‖ epoch ‖ nonce ‖ XChaCha20-Poly1305(data key, nonce,
    /// zstd(chunk), associated data: the id's 64 characters). An error when the object would be over `MAX_OBJECT`.
    pub fn seal(&self, id: &str, chunk: &[u8]) -> Result<Vec<u8>> {
        let compressed = zstd::bulk::compress(chunk, ZSTD_LEVEL).map_err(|e| Error::Local(format!("A chunk couldn't be compressed: {e}")))?;
        if compressed.len() + OBJECT_OVERHEAD > MAX_OBJECT {
            return Err(Error::Local(format!(
                "A chunk of {} bytes would be {} bytes sealed, over the {MAX_OBJECT} an object may be",
                chunk.len(),
                compressed.len() + OBJECT_OVERHEAD
            )));
        }
        let nonce = random::<24>()?;
        let sealed = self.cipher().encrypt(XNonce::from_slice(&nonce), Payload { msg: &compressed, aad: id.as_bytes() })
            .map_err(|_| Error::Local("A chunk couldn't be encrypted".to_string()))?;
        let mut object = Vec::with_capacity(4 + 24 + sealed.len());
        object.extend_from_slice(OBJECT_MAGIC);
        object.push(self.epoch);
        object.extend_from_slice(&nonce);
        object.extend_from_slice(&sealed);
        Ok(object)
    }

    /// Opens object `object`, which should be chunk `id`: its header and epoch, its tag (with `id` as associated data),
    /// zstd (at most `MAX_CHUNK` bytes), and the chunk's HMAC, which must be `id`. `Error::Damaged` when any fails.
    pub fn open(&self, id: &str, object: &[u8]) -> Result<Vec<u8>> {
        let damaged = |why: &str| Error::Damaged(format!("Object {} from Hersir Cloud {why}", short(id)));
        if object.len() < OBJECT_OVERHEAD || &object[..3] != OBJECT_MAGIC {
            return Err(damaged("isn't one of Hersir's objects"));
        }
        if object[3] != self.epoch {
            return Err(damaged(&format!("was made with another account key (epoch {}, not {})", object[3], self.epoch)));
        }
        let compressed = self.cipher().decrypt(XNonce::from_slice(&object[4..28]), Payload { msg: &object[28..], aad: id.as_bytes() })
            .map_err(|_| damaged("doesn't open: it was changed, or isn't the object asked for"))?;
        let chunk = zstd::bulk::decompress(&compressed, MAX_CHUNK).map_err(|_| damaged("opens to something that can't be decompressed"))?;
        if !same(hex(&self.mac(&chunk)).as_bytes(), id.as_bytes()) {
            return Err(damaged("doesn't hold the chunk its id names"));
        }
        Ok(chunk)
    }
}

/// The first 12 characters of an id, for a message.
pub(crate) fn short(id: &str) -> String {
    id.chars().take(12).collect()
}

/// Whether two byte strings are equal, compared in constant time.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

/// One chunk of a file: its id, and where its bytes are in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub id: String,
    pub offset: u64,
    pub len: usize,
}

/// A file cut into chunks: its chunks in order, and the whole file's SHA-256 (64 lowercase hex) and size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cut {
    pub pieces: Vec<Piece>,
    pub sha256: String,
    pub bytes: u64,
}

/// Cuts the database copy at `path` into 256 KiB chunks. `read` hears the size of each part read.
pub fn cut_db(keys: &ChunkKeys, path: &Path, read: &mut dyn FnMut(u64) -> Result<()>) -> Result<Cut> {
    let mut file = File::open(path).map_err(|e| local(path, "can't be read", e))?;
    let mut hash = Sha256::new();
    let mut pieces = Vec::new();
    let mut offset = 0u64;
    let mut buf = vec![0u8; DB_CHUNK];
    loop {
        let n = fill(&mut file, &mut buf).map_err(|e| local(path, "can't be read", e))?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        pieces.push(Piece { id: keys.id_of(&buf[..n]), offset, len: n });
        offset += n as u64;
        read(n as u64)?;
        if n < DB_CHUNK {
            break;
        }
    }
    Ok(Cut { pieces, sha256: format!("{:x}", hash.finalize()), bytes: offset })
}

/// Cuts the blob at `path`: one chunk when it is up to 4 MiB (and its object fits in `MAX_OBJECT`, which only a few
/// bytes short of 4 MiB that zstd can't shrink don't), else by content (FastCDC). An empty blob has no chunks. `read`
/// hears the size of each part read.
pub fn cut_blob(keys: &ChunkKeys, path: &Path, read: &mut dyn FnMut(u64) -> Result<()>) -> Result<Cut> {
    let mut file = File::open(path).map_err(|e| local(path, "can't be read", e))?;
    let size = file.metadata().map_err(|e| local(path, "can't be read", e))?.len();
    if size <= WHOLE_BLOB {
        let mut data = Vec::with_capacity(size as usize);
        // at most one byte more than the size: a file that grew meanwhile is seen as changed (its hash won't match)
        (&mut file).take(WHOLE_BLOB + 1).read_to_end(&mut data).map_err(|e| local(path, "can't be read", e))?;
        read(data.len() as u64)?;
        let sha256 = format!("{:x}", Sha256::digest(&data));
        let bytes = data.len() as u64;
        if data.is_empty() {
            return Ok(Cut { pieces: vec![], sha256, bytes });
        }
        if data.len() as u64 <= WHOLE_BLOB && fits_whole(&data)? {
            return Ok(Cut { pieces: vec![Piece { id: keys.id_of(&data), offset: 0, len: data.len() }], sha256, bytes });
        }
        let pieces = fastcdc::v2020::FastCDC::new(&data, CDC_MIN, CDC_AVG, CDC_MAX)
            .map(|c| Piece { id: keys.id_of(&data[c.offset..c.offset + c.length]), offset: c.offset as u64, len: c.length })
            .collect();
        return Ok(Cut { pieces, sha256, bytes });
    }
    let mut hashing = HashingReader { inner: file, hash: Sha256::new(), bytes: 0 };
    let mut pieces = Vec::new();
    for chunk in fastcdc::v2020::StreamCDC::new(&mut hashing, CDC_MIN, CDC_AVG, CDC_MAX) {
        let chunk = chunk.map_err(|e| local(path, "can't be read", io::Error::from(e)))?;
        read(chunk.length as u64)?;
        pieces.push(Piece { id: keys.id_of(&chunk.data), offset: chunk.offset, len: chunk.length });
    }
    Ok(Cut { pieces, sha256: format!("{:x}", hashing.hash.finalize()), bytes: hashing.bytes })
}

/// Whether a chunk of these bytes surely fits in one object: zstd adds at most 3 bytes per 128 KiB block and its frame
/// header when it can't shrink them, so only a chunk within about a KiB of 4 MiB needs compressing to know.
fn fits_whole(data: &[u8]) -> Result<bool> {
    let n = data.len();
    let worst = n + 3 * (n / (128 * 1024) + 1) + 22 + 1024;
    if worst + OBJECT_OVERHEAD <= MAX_OBJECT {
        return Ok(true);
    }
    let compressed = zstd::bulk::compress(data, ZSTD_LEVEL).map_err(|e| Error::Local(format!("A file couldn't be compressed: {e}")))?;
    Ok(compressed.len() + OBJECT_OVERHEAD <= MAX_OBJECT)
}

/// Reads one piece's bytes back from the file it was cut from, and checks they are still the chunk its id names.
pub fn read_piece(keys: &ChunkKeys, file: &mut File, piece: &Piece) -> Result<Vec<u8>> {
    let mut data = vec![0u8; piece.len];
    file.seek(SeekFrom::Start(piece.offset))?;
    file.read_exact(&mut data)?;
    if keys.id_of(&data) != piece.id {
        return Err(Error::Local("A file changed while it was being backed up: back up again".to_string()));
    }
    Ok(data)
}

/// Reads until `buf` is full or the file ends: how much was read.
fn fill(file: &mut File, buf: &mut [u8]) -> io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match file.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(n)
}

/// Reads through to `inner`, hashing (SHA-256) and counting what passes.
struct HashingReader<R> {
    inner: R,
    hash: Sha256,
    bytes: u64,
}

impl<R: Read> Read for HashingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.hash.update(&buf[..n]);
        self.bytes += n as u64;
        Ok(n)
    }
}

fn local(path: &Path, what: &str, e: io::Error) -> Error {
    Error::Local(format!("{} {what}: {e}", path.display()))
}

/// Whether `id` looks like an object id (64 lowercase hex).
pub fn is_object_id(id: &str) -> bool {
    is_hex(id, 64)
}
