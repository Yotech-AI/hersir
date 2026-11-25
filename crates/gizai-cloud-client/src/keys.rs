//! Cloud Backup's keys (docs/backup-format.md, For Hersir Cloud; research 3.2):
//!
//! - the **account key**: 32 random bytes, made by the account's first computer, kept in each signed-in computer's
//!   keychain and never on the server as it is. Each key has an epoch (the first is 1), the byte after `HC1` in every
//!   object;
//! - from it, with HKDF-SHA256, the **data key** (objects are sealed with it), the **id key** (objects are named with
//!   it) and the 16-byte **key id** (on the server, to tell a wrong recovery code from the right one);
//! - the **recovery code**: 128 random bits, shown as `HC-` and 27 characters in groups of four (the last group three),
//!   the last one a check character. Typed back, it is read whatever the case, spaces and dashes are ignored, and a
//!   single wrong character fails the check. It is never stored on a computer;
//! - the account key **sealed by the code** (`sealed_by_code`), which the server keeps; making a new recovery code seals
//!   the same account key again, and the old code stops working once the new one is stored;
//! - this computer's own **Ed25519** (signing) and **X25519** (box) keys, whose public halves go to `POST /devices`.
//!
//! Keys are wiped from memory when dropped, and never appear in Debug output or errors.
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use ed25519_dalek::{Signer, SigningKey};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::{Zeroize, Zeroizing};

use crate::api::KeyRecord;
use crate::util::{b64, hex, random, unb64};
use crate::{Error, Result};

/// The first account key's epoch.
pub const FIRST_EPOCH: u8 = 1;
/// HKDF-SHA256's `info` for the data key (32 bytes)…
pub const DATA_KEY_INFO: &[u8] = b"hersir-cloud/data-key/1";
/// …the id key (32 bytes)…
pub const ID_KEY_INFO: &[u8] = b"hersir-cloud/id-key/1";
/// …the key id (16 bytes)…
pub const KEY_ID_INFO: &[u8] = b"hersir-cloud/key-id/1";
/// …and the key that seals the account key, from the recovery code's 16 bytes (32 bytes).
pub const CODE_KEY_INFO: &[u8] = b"hersir-cloud/recovery-code/1";
/// The first bytes of the account key sealed by the recovery code.
pub const SEALED_MAGIC: &[u8; 4] = b"HCR1";
/// A sealed account key: magic (4) ‖ epoch (1) ‖ nonce (24) ‖ the key, encrypted (32) ‖ tag (16).
pub const SEALED_LEN: usize = 4 + 1 + 24 + 32 + 16;
/// The recovery code's characters: Crockford's base32 (no I, L, O or U).
pub const CODE_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// What the recovery code is shown with: `HC-`.
pub const CODE_PREFIX: &str = "HC";
/// The recovery code's characters: 26 for its 128 bits, and the check character.
pub const CODE_CHARS: usize = 27;

/// The account key: 32 random bytes and its epoch.
#[derive(Clone)]
pub struct AccountKey {
    bytes: Zeroizing<[u8; 32]>,
    epoch: u8,
}

impl std::fmt::Debug for AccountKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountKey").field("key_id", &self.key_id()).field("epoch", &self.epoch).finish_non_exhaustive()
    }
}

impl PartialEq for AccountKey {
    fn eq(&self, other: &Self) -> bool {
        self.epoch == other.epoch && same(&self.bytes[..], &other.bytes[..])
    }
}

impl Eq for AccountKey {}

/// The account key as the keychain keeps it (`to_text`).
#[derive(Serialize, Deserialize)]
struct KeyText {
    epoch: u8,
    key: String,
}

impl AccountKey {
    /// A new account key: 32 random bytes, with `epoch` (`FIRST_EPOCH` for the account's first).
    pub fn generate(epoch: u8) -> Result<AccountKey> {
        Ok(AccountKey { bytes: Zeroizing::new(random::<32>()?), epoch })
    }

    /// An account key from its 32 bytes.
    pub fn from_bytes(bytes: &[u8], epoch: u8) -> Result<AccountKey> {
        let bytes: [u8; 32] = bytes.try_into().map_err(|_| Error::Damaged(format!("An account key has 32 bytes, not {}", bytes.len())))?;
        Ok(AccountKey { bytes: Zeroizing::new(bytes), epoch })
    }

    /// Its 32 bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }

    pub fn epoch(&self) -> u8 {
        self.epoch
    }

    /// For the keychain: `{"epoch":1,"key":"<base64 of the 32 bytes>"}`.
    pub fn to_text(&self) -> Zeroizing<String> {
        let mut t = KeyText { epoch: self.epoch, key: b64(&self.bytes[..]) };
        let text = serde_json::to_string(&t).unwrap_or_default();
        t.key.zeroize();
        Zeroizing::new(text)
    }

    /// From the keychain's text (`to_text`).
    pub fn from_text(text: &str) -> Result<AccountKey> {
        let unreadable = || Error::Damaged("The account key in the keychain can't be read".to_string());
        let mut t: KeyText = serde_json::from_str(text).map_err(|_| unreadable())?;
        let bytes = Zeroizing::new(unb64(&t.key).ok_or_else(unreadable)?);
        t.key.zeroize();
        AccountKey::from_bytes(&bytes, t.epoch).map_err(|_| unreadable())
    }

    /// HKDF-SHA256 of the account key (no salt) with `info`, `N` bytes.
    fn derive<const N: usize>(&self, info: &[u8]) -> Zeroizing<[u8; N]> {
        let mut out = Zeroizing::new([0u8; N]);
        // N is 16 or 32 here, far below HKDF-SHA256's 8,160 bytes.
        let _ = Hkdf::<Sha256>::new(None, &self.bytes[..]).expand(info, &mut out[..]);
        out
    }

    /// The data key: objects are sealed with it.
    pub fn data_key(&self) -> Zeroizing<[u8; 32]> {
        self.derive(DATA_KEY_INFO)
    }

    /// The id key: objects are named with it (HMAC-SHA256).
    pub fn id_key(&self) -> Zeroizing<[u8; 32]> {
        self.derive(ID_KEY_INFO)
    }

    /// The key id: 16 bytes, as 32 lowercase hex characters (`KeyRecord::key_id`). It tells a recovery code that opens
    /// the account's key from one that doesn't, and says nothing about the key itself.
    pub fn key_id(&self) -> String {
        hex(&self.derive::<16>(KEY_ID_INFO)[..])
    }

    /// What `PUT /keys` stores: the key id, the epoch, and the key sealed by `code`.
    pub fn record(&self, code: &RecoveryCode) -> Result<KeyRecord> {
        Ok(KeyRecord { key_id: self.key_id(), epoch: self.epoch, sealed_by_code: b64(&self.seal_by_code(code)?) })
    }

    /// The account key sealed by `code`: `HCR1` ‖ epoch ‖ nonce (24) ‖ XChaCha20-Poly1305(code key, nonce, account key),
    /// with `HCR1` ‖ epoch ‖ the key id's 16 bytes as associated data.
    pub fn seal_by_code(&self, code: &RecoveryCode) -> Result<Vec<u8>> {
        let nonce = random::<24>()?;
        let key = code.sealing_key();
        let aad = sealed_aad(self.epoch, &self.derive::<16>(KEY_ID_INFO)[..]);
        let sealed = XChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(&key[..]))
            .encrypt(XNonce::from_slice(&nonce), Payload { msg: &self.bytes[..], aad: &aad })
            .map_err(|_| Error::Local("The account key couldn't be sealed".to_string()))?;
        let mut out = Vec::with_capacity(SEALED_LEN);
        out.extend_from_slice(SEALED_MAGIC);
        out.push(self.epoch);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&sealed);
        Ok(out)
    }
}

/// The associated data of a sealed account key: `HCR1` ‖ epoch ‖ key id (16 bytes).
fn sealed_aad(epoch: u8, key_id: &[u8]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(4 + 1 + key_id.len());
    aad.extend_from_slice(SEALED_MAGIC);
    aad.push(epoch);
    aad.extend_from_slice(key_id);
    aad
}

/// Opens the account key `GET /keys` gave with the recovery code typed: `Error::WrongRecoveryCode` when the code
/// doesn't open it, or opens a key whose key id isn't the record's.
pub fn open_record(record: &KeyRecord, code: &RecoveryCode) -> Result<AccountKey> {
    let sealed = unb64(&record.sealed_by_code).ok_or_else(|| Error::Damaged("Hersir Cloud's copy of the account key can't be read".to_string()))?;
    open_sealed(&sealed, code, &record.key_id)
}

/// Opens an account key sealed by `code` (`AccountKey::seal_by_code`) whose key id should be `key_id` (32 hex).
pub fn open_sealed(sealed: &[u8], code: &RecoveryCode, key_id: &str) -> Result<AccountKey> {
    if sealed.len() != SEALED_LEN || &sealed[..4] != SEALED_MAGIC {
        return Err(Error::Damaged("Hersir Cloud's copy of the account key isn't one Hersir can read".to_string()));
    }
    let id = key_id.trim().to_ascii_lowercase();
    let id_bytes = (id.len() == 32).then(|| (0..16).map(|i| u8::from_str_radix(&id[2 * i..2 * i + 2], 16).ok()).collect::<Option<Vec<u8>>>())
        .flatten()
        .ok_or(Error::WrongRecoveryCode)?;
    let epoch = sealed[4];
    let key = code.sealing_key();
    let opened = XChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(&key[..]))
        .decrypt(XNonce::from_slice(&sealed[5..29]), Payload { msg: &sealed[29..], aad: &sealed_aad(epoch, &id_bytes) })
        .map(Zeroizing::new)
        .map_err(|_| Error::WrongRecoveryCode)?;
    let ak = AccountKey::from_bytes(&opened[..], epoch).map_err(|_| Error::WrongRecoveryCode)?;
    if !same(ak.key_id().as_bytes(), id.as_bytes()) {
        return Err(Error::WrongRecoveryCode);
    }
    Ok(ak)
}

/// A new account key (epoch 1) and recovery code, and what `PUT /keys` stores: the account's first computer makes them.
pub fn new_account_key() -> Result<(AccountKey, RecoveryCode, KeyRecord)> {
    let ak = AccountKey::generate(FIRST_EPOCH)?;
    let code = RecoveryCode::generate()?;
    let record = ak.record(&code)?;
    Ok((ak, code, record))
}

/// A new recovery code for the same account key, and what `PUT /keys` stores (with `if_epoch` the key's epoch): once it
/// is stored, the old code no longer opens it.
pub fn new_recovery_code(ak: &AccountKey) -> Result<(RecoveryCode, KeyRecord)> {
    let code = RecoveryCode::generate()?;
    let record = ak.record(&code)?;
    Ok((code, record))
}

/// The recovery code: 128 random bits.
#[derive(Clone)]
pub struct RecoveryCode {
    bytes: Zeroizing<[u8; 16]>,
}

impl std::fmt::Debug for RecoveryCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryCode(•••)")
    }
}

impl PartialEq for RecoveryCode {
    fn eq(&self, other: &Self) -> bool {
        same(&self.bytes[..], &other.bytes[..])
    }
}

impl Eq for RecoveryCode {}

impl RecoveryCode {
    /// A new recovery code: 128 random bits.
    pub fn generate() -> Result<RecoveryCode> {
        Ok(RecoveryCode { bytes: Zeroizing::new(random::<16>()?) })
    }

    /// A recovery code from its 16 bytes.
    pub fn from_bytes(bytes: [u8; 16]) -> RecoveryCode {
        RecoveryCode { bytes: Zeroizing::new(bytes) }
    }

    /// Its 16 bytes.
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.bytes
    }

    /// The code's 27 characters, without `HC-` and groups: the 128 bits as 26 characters of `CODE_ALPHABET` (5 bits
    /// each, the first bit first, the last character's 2 lowest bits 0), then the check character.
    pub fn characters(&self) -> Zeroizing<String> {
        let mut values = Zeroizing::new(Vec::with_capacity(CODE_CHARS));
        let mut acc: u32 = 0;
        let mut bits = 0;
        for b in self.bytes.iter() {
            acc = (acc << 8) | u32::from(*b);
            bits += 8;
            while bits >= 5 {
                bits -= 5;
                values.push(((acc >> bits) & 31) as u8);
            }
            acc &= (1 << bits) - 1;
        }
        // 128 bits: 25 characters and 3 bits left over, shifted up into the 26th.
        values.push(((acc << (5 - bits)) & 31) as u8);
        acc.zeroize();
        let check = check_value(&values);
        values.push(check);
        Zeroizing::new(values.iter().map(|v| char::from(CODE_ALPHABET[usize::from(*v)])).collect())
    }

    /// The code as it is shown: `HC-` and the 27 characters in groups of four, the last group three, joined by dashes:
    /// `HC-7K2Q-9MXA-4T8R-ZP3N-KD6W-1BVG-H2C`.
    pub fn shown(&self) -> Zeroizing<String> {
        let chars = self.characters();
        let groups: Vec<&str> = chars.as_bytes().chunks(4).map(|g| std::str::from_utf8(g).unwrap_or_default()).collect();
        Zeroizing::new(format!("{CODE_PREFIX}-{}", groups.join("-")))
    }

    /// The last group (three characters, the check character last): what setup asks to type back.
    pub fn last_group(&self) -> Zeroizing<String> {
        let chars = self.characters();
        Zeroizing::new(chars[24..].to_string())
    }

    /// Whether `typed` is the code's last group, read as `parse` reads a code: any case, spaces and dashes ignored, O as
    /// 0, I and L as 1.
    pub fn last_group_matches(&self, typed: &str) -> bool {
        let typed: Zeroizing<String> = Zeroizing::new(
            typed.chars().filter(|c| !c.is_whitespace() && !is_dash(*c))
                .map(|c| match c.to_ascii_uppercase() {
                    'O' => '0',
                    'I' | 'L' => '1',
                    c => c,
                })
                .collect(),
        );
        same(typed.as_bytes(), self.last_group().as_bytes())
    }

    /// Reads a typed recovery code: any case, spaces and dashes ignored, with or without `HC` in front; O is read as 0,
    /// and I and L as 1. `Error::NotARecoveryCode` for the wrong length or another character, and
    /// `Error::RecoveryCodeCheck` when the check character doesn't match (a character typed wrong).
    pub fn parse(typed: &str) -> Result<RecoveryCode> {
        let mut chars: Zeroizing<Vec<char>> = Zeroizing::new(
            typed.chars().filter(|c| !c.is_whitespace() && !is_dash(*c)).map(|c| c.to_ascii_uppercase()).collect(),
        );
        if chars.len() == CODE_CHARS + CODE_PREFIX.len() && chars.starts_with(&['H', 'C']) {
            chars.drain(..CODE_PREFIX.len());
        }
        if chars.len() != CODE_CHARS {
            // counted after HC, when it is typed with it
            let typed = if chars.len() > CODE_CHARS && chars.starts_with(&['H', 'C']) { chars.len() - CODE_PREFIX.len() } else { chars.len() };
            return Err(Error::NotARecoveryCode(format!(
                "A recovery code has {CODE_CHARS} characters after {CODE_PREFIX}-, and this has {typed}."
            )));
        }
        let mut values = Zeroizing::new(Vec::with_capacity(CODE_CHARS));
        for c in chars.iter() {
            let c = match c {
                'O' => '0',
                'I' | 'L' => '1',
                c => *c,
            };
            match CODE_ALPHABET.iter().position(|a| char::from(*a) == c) {
                Some(v) => values.push(v as u8),
                None => {
                    return Err(Error::NotARecoveryCode(format!("A recovery code has only the letters and digits it was shown with, not {c}.")));
                }
            }
        }
        let check = values.pop().unwrap_or_default();
        // the 26th character carries 3 bits: its 2 lowest are 0 in every code
        if check_value(&values) != check || values[25] & 3 != 0 {
            return Err(Error::RecoveryCodeCheck);
        }
        let mut bytes = Zeroizing::new([0u8; 16]);
        let mut acc: u32 = 0;
        let mut bits = 0;
        let mut n = 0;
        for v in values.iter() {
            acc = (acc << 5) | u32::from(*v);
            bits += 5;
            if bits >= 8 {
                bits -= 8;
                if n < 16 {
                    bytes[n] = ((acc >> bits) & 0xff) as u8;
                    n += 1;
                }
                acc &= (1 << bits) - 1;
            }
        }
        acc.zeroize();
        Ok(RecoveryCode { bytes })
    }

    /// The key that seals the account key: HKDF-SHA256 of the code's 16 bytes (no salt), `CODE_KEY_INFO`.
    fn sealing_key(&self) -> Zeroizing<[u8; 32]> {
        let mut out = Zeroizing::new([0u8; 32]);
        let _ = Hkdf::<Sha256>::new(None, &self.bytes[..]).expand(CODE_KEY_INFO, &mut out[..]);
        out
    }
}

/// Dashes people type or paste: the hyphen-minus and Unicode's hyphens and dashes.
fn is_dash(c: char) -> bool {
    matches!(c, '-' | '\u{2010}'..='\u{2015}' | '\u{2212}')
}

/// The check character's value: Luhn mod 32 over the code's character values (from the right, every second value
/// doubled, its two base-32 digits added), so that any one wrong character changes it.
fn check_value(values: &[u8]) -> u8 {
    let mut sum: u32 = 0;
    for (i, v) in values.iter().rev().enumerate() {
        let mut addend = u32::from(*v);
        if i % 2 == 0 {
            addend *= 2;
            addend = addend / 32 + addend % 32;
        }
        sum += addend;
    }
    ((32 - sum % 32) % 32) as u8
}

/// Whether two byte strings are equal, compared in constant time.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

/// This computer's own keys: an Ed25519 signing key and an X25519 box key. Their public halves go to `POST /devices`.
pub struct DeviceKeys {
    signing: SigningKey,
    boxing: x25519_dalek::StaticSecret,
}

impl std::fmt::Debug for DeviceKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceKeys").field("sign_pub", &self.sign_pub_b64()).field("box_pub", &self.box_pub_b64()).finish_non_exhaustive()
    }
}

/// The computer's keys as the keychain keeps them (`to_text`).
#[derive(Serialize, Deserialize)]
struct DeviceText {
    sign: String,
    #[serde(rename = "box")]
    boxing: String,
}

impl DeviceKeys {
    /// New keys for this computer.
    pub fn generate() -> Result<DeviceKeys> {
        let (seed, secret) = (Zeroizing::new(random::<32>()?), Zeroizing::new(random::<32>()?));
        Ok(DeviceKeys::from_secrets(&seed, &secret))
    }

    /// The keys from their secret halves: the Ed25519 seed and the X25519 secret, 32 bytes each.
    pub fn from_secrets(sign_seed: &[u8; 32], box_secret: &[u8; 32]) -> DeviceKeys {
        DeviceKeys { signing: SigningKey::from_bytes(sign_seed), boxing: x25519_dalek::StaticSecret::from(*box_secret) }
    }

    /// The Ed25519 public key.
    pub fn sign_pub(&self) -> [u8; 32] {
        self.signing.verifying_key().to_bytes()
    }

    /// The X25519 public key.
    pub fn box_pub(&self) -> [u8; 32] {
        x25519_dalek::PublicKey::from(&self.boxing).to_bytes()
    }

    /// The Ed25519 public key in base64, as `POST /devices` takes it.
    pub fn sign_pub_b64(&self) -> String {
        b64(&self.sign_pub())
    }

    /// The X25519 public key in base64, as `POST /devices` takes it.
    pub fn box_pub_b64(&self) -> String {
        b64(&self.box_pub())
    }

    /// An Ed25519 signature by this computer.
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.signing.sign(message).to_bytes()
    }

    /// X25519 with another device's public key: the shared secret.
    pub fn agree(&self, their_box_pub: &[u8; 32]) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(self.boxing.diffie_hellman(&x25519_dalek::PublicKey::from(*their_box_pub)).to_bytes())
    }

    /// For the keychain: `{"sign":"<base64 Ed25519 seed>","box":"<base64 X25519 secret>"}`.
    pub fn to_text(&self) -> Zeroizing<String> {
        let mut t = DeviceText { sign: b64(&self.signing.to_bytes()), boxing: b64(self.boxing.as_bytes()) };
        let text = serde_json::to_string(&t).unwrap_or_default();
        t.sign.zeroize();
        t.boxing.zeroize();
        Zeroizing::new(text)
    }

    /// From the keychain's text (`to_text`).
    pub fn from_text(text: &str) -> Result<DeviceKeys> {
        let unreadable = || Error::Damaged("This computer's keys in the keychain can't be read".to_string());
        let mut t: DeviceText = serde_json::from_str(text).map_err(|_| unreadable())?;
        let sign = Zeroizing::new(unb64(&t.sign).ok_or_else(unreadable)?);
        let boxing = Zeroizing::new(unb64(&t.boxing).ok_or_else(unreadable)?);
        t.sign.zeroize();
        t.boxing.zeroize();
        let sign: Zeroizing<[u8; 32]> = Zeroizing::new(sign.as_slice().try_into().map_err(|_| unreadable())?);
        let boxing: Zeroizing<[u8; 32]> = Zeroizing::new(boxing.as_slice().try_into().map_err(|_| unreadable())?);
        Ok(DeviceKeys::from_secrets(&sign, &boxing))
    }
}
