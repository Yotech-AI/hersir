//! The keys (research 3.2; docs/backup-format.md, For Hersir Cloud): the account key and what is derived from it, the
//! recovery code and its check character, the account key sealed by the code, and this computer's own keys. Where the
//! doc gives a format, the test works it out from the doc on its own and compares.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use gizai_cloud_client::Error;
use gizai_cloud_client::keys::{self, AccountKey, CODE_ALPHABET, DeviceKeys, RecoveryCode};
use hkdf::Hkdf;
use sha2::Sha256;

use crate::common::{self, keyed};

fn hkdf(ikm: &[u8], info: &str, len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    Hkdf::<Sha256>::new(None, ikm).expand(info.as_bytes(), &mut out).unwrap();
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn b64(text: &str) -> Vec<u8> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(text).unwrap()
}

/// The doc's encoding of a recovery code, worked out on its own: the 16 bytes as bits, 5 per character, the 26th
/// holding the last 3 bits shifted up, then Luhn mod 32 over the 26 values.
fn doc_characters(bytes: &[u8; 16]) -> String {
    let bits: Vec<u8> = bytes.iter().flat_map(|b| (0..8).rev().map(move |i| (b >> i) & 1)).collect();
    let mut values: Vec<u32> = bits.chunks(5).map(|c| {
        let mut v = 0u32;
        for k in 0..5 {
            v = (v << 1) | u32::from(*c.get(k).unwrap_or(&0));
        }
        v
    }).collect();
    assert_eq!(values.len(), 26);
    let mut sum = 0u32;
    for (i, v) in values.iter().rev().enumerate() {
        let a = if i % 2 == 0 { let d = v * 2; d / 32 + d % 32 } else { *v };
        sum += a;
    }
    values.push((32 - sum % 32) % 32);
    values.iter().map(|v| char::from(CODE_ALPHABET[*v as usize])).collect()
}

#[test]
fn the_account_key_is_32_random_bytes_with_its_epoch() {
    let a = AccountKey::generate(keys::FIRST_EPOCH).unwrap();
    let b = AccountKey::generate(keys::FIRST_EPOCH).unwrap();
    assert_eq!(a.as_bytes().len(), 32);
    assert_eq!(a.epoch(), 1);
    assert_ne!(a.as_bytes(), b.as_bytes(), "two account keys are not the same");
    assert_ne!(a.key_id(), b.key_id());
    let (k, _, record) = keys::new_account_key().unwrap();
    assert_eq!(k.epoch(), keys::FIRST_EPOCH);
    assert_eq!(record.epoch, 1);
    assert_eq!(record.key_id, k.key_id());
}

#[test]
fn the_data_key_id_key_and_key_id_are_hkdf_sha256_with_the_documented_info_strings() {
    let key = AccountKey::from_bytes(&[42u8; 32], 1).unwrap();
    assert_eq!(&key.data_key()[..], &hkdf(&[42u8; 32], "hersir-cloud/data-key/1", 32)[..]);
    assert_eq!(&key.id_key()[..], &hkdf(&[42u8; 32], "hersir-cloud/id-key/1", 32)[..]);
    assert_eq!(key.key_id(), hex(&hkdf(&[42u8; 32], "hersir-cloud/key-id/1", 16)));
    assert_eq!(key.key_id().len(), 32);
    assert!(key.key_id().chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_ne!(&key.data_key()[..], &key.id_key()[..], "the data key and the id key differ");
    // an account key of another length isn't one
    assert!(matches!(AccountKey::from_bytes(&[1u8; 31], 1), Err(Error::Damaged(_))));
}

#[test]
fn the_account_key_goes_to_the_keychain_and_back_and_is_never_in_debug_output() {
    let key = AccountKey::generate(3).unwrap();
    let text = key.to_text();
    let back = AccountKey::from_text(&text).unwrap();
    assert_eq!(back, key);
    assert_eq!(back.epoch(), 3);
    let shown = format!("{key:?}");
    assert!(!shown.contains(&text[..]), "{shown}");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let raw = v["key"].as_str().unwrap();
    assert!(!shown.contains(raw), "Debug shows the key: {shown}");
    assert!(shown.contains(&key.key_id()), "Debug may show the key id: {shown}");
    assert!(matches!(AccountKey::from_text("{\"epoch\":1,\"key\":\"AAAA\"}"), Err(Error::Damaged(_))));
}

#[test]
fn a_recovery_code_is_shown_as_hc_and_27_characters_in_groups_of_four_the_last_three() {
    for _ in 0..50 {
        let code = RecoveryCode::generate().unwrap();
        let shown = code.shown();
        let groups: Vec<&str> = shown.split('-').collect();
        assert_eq!(groups[0], "HC", "{}", &shown[..]);
        let lens: Vec<usize> = groups[1..].iter().map(|g| g.len()).collect();
        assert_eq!(lens, [4, 4, 4, 4, 4, 4, 3], "{}", &shown[..]);
        let chars = code.characters();
        assert_eq!(chars.len(), 27);
        assert_eq!(groups[1..].concat(), &chars[..]);
        assert!(chars.bytes().all(|c| CODE_ALPHABET.contains(&c)), "only Crockford's base32: {}", &chars[..]);
        assert!(!chars.contains(['I', 'L', 'O', 'U']));
        assert_eq!(&code.last_group()[..], &chars[24..]);
    }
    assert_eq!(&CODE_ALPHABET[..], b"0123456789ABCDEFGHJKMNPQRSTVWXYZ");
}

#[test]
fn a_recovery_codes_characters_are_the_documented_base32_and_luhn_mod_32_check() {
    let fixed: [[u8; 16]; 4] = [[0u8; 16], [0xff; 16], *b"0123456789abcdef", [0x9c, 0x1a, 0x07, 0x42, 0xde, 0xad, 0xbe, 0xef, 0x01, 0x80, 0x55, 0xaa, 0x10, 0x20, 0x30, 0x40]];
    for bytes in fixed {
        let code = RecoveryCode::from_bytes(bytes);
        assert_eq!(&code.characters()[..], doc_characters(&bytes), "{bytes:?}");
    }
    for _ in 0..200 {
        let code = RecoveryCode::generate().unwrap();
        assert_eq!(&code.characters()[..], doc_characters(code.as_bytes()));
    }
    // 128 bits: all zeros is 26 zeros and a check of 0
    assert_eq!(&RecoveryCode::from_bytes([0u8; 16]).shown()[..], "HC-0000-0000-0000-0000-0000-0000-000");
}

#[test]
fn a_typed_recovery_code_is_read_whatever_its_case_spaces_and_dashes() {
    let code = RecoveryCode::generate().unwrap();
    let shown = code.shown().to_string();
    let chars = code.characters().to_string();
    let spaced: String = chars.chars().flat_map(|c| [c, ' ']).collect();
    let typed = [
        shown.clone(),
        shown.to_lowercase(),
        chars.clone(),
        chars.to_lowercase(),
        format!("  {spaced}  "),
        format!("hc {}", chars.to_lowercase()),
        shown.replace('-', " "),
        shown.replace('-', "\u{2013}"),
        shown.replace('-', "\u{2010}"),
        shown.replace('-', "--"),
        format!("HC-{}", chars.chars().collect::<Vec<_>>().chunks(3).map(|g| g.iter().collect::<String>()).collect::<Vec<_>>().join("-")),
    ];
    for t in &typed {
        assert_eq!(RecoveryCode::parse(t).unwrap_or_else(|e| panic!("{t:?}: {e}")), code, "{t:?}");
    }
}

#[test]
fn o_is_read_as_zero_and_i_and_l_as_one() {
    let mut found = 0;
    for _ in 0..400 {
        let code = RecoveryCode::generate().unwrap();
        let chars = code.characters().to_string();
        if chars.contains('0') && chars.contains('1') {
            let typed = chars.replace('0', "O").replacen('1', "I", 1).replace('1', "l");
            assert_eq!(RecoveryCode::parse(&typed).unwrap(), code, "{typed}");
            found += 1;
        }
    }
    assert!(found > 0);
}

#[test]
fn a_recovery_code_with_any_one_wrong_character_fails_its_check_character() {
    for _ in 0..8 {
        let code = RecoveryCode::generate().unwrap();
        let chars: Vec<u8> = code.characters().bytes().collect();
        for at in 0..chars.len() {
            for &c in CODE_ALPHABET.iter().filter(|c| **c != chars[at]) {
                let mut typed = chars.clone();
                typed[at] = c;
                let typed = String::from_utf8(typed).unwrap();
                match RecoveryCode::parse(&format!("HC-{typed}")) {
                    Err(Error::RecoveryCodeCheck) => {}
                    other => panic!("{} changed at {at} to {}: {other:?}", &code.characters()[..], c as char),
                }
            }
        }
    }
    assert_eq!(Error::RecoveryCodeCheck.to_string(), "That recovery code has a typing mistake: check each character and try again.");
}

#[test]
fn text_that_isnt_a_recovery_code_is_refused_as_such() {
    let code = RecoveryCode::generate().unwrap();
    let chars = code.characters().to_string();
    // too short, too long, and counted after HC when it is typed with it
    match RecoveryCode::parse(&chars[..26]) {
        Err(Error::NotARecoveryCode(m)) => assert!(m.contains("27") && m.contains("26"), "{m}"),
        other => panic!("{other:?}"),
    }
    match RecoveryCode::parse(&format!("HC-{chars}7")) {
        Err(Error::NotARecoveryCode(m)) => assert!(m.contains("has 28"), "{m}"),
        other => panic!("{other:?}"),
    }
    assert!(matches!(RecoveryCode::parse(""), Err(Error::NotARecoveryCode(_))));
    // U isn't in the alphabet (Crockford leaves it out)
    let with_u = format!("U{}", &chars[1..]);
    match RecoveryCode::parse(&with_u) {
        Err(Error::NotARecoveryCode(m)) => assert!(m.contains('U'), "{m}"),
        other => panic!("{other:?}"),
    }
    assert!(matches!(RecoveryCode::parse(&format!("{}!", &chars[..26])), Err(Error::NotARecoveryCode(_))));
}

#[test]
fn setup_checks_the_last_group_as_typed() {
    let code = RecoveryCode::generate().unwrap();
    let last = code.last_group().to_string();
    assert_eq!(last.len(), 3);
    assert!(code.last_group_matches(&last));
    assert!(code.last_group_matches(&last.to_lowercase()));
    assert!(code.last_group_matches(&format!(" {} ", last.chars().map(String::from).collect::<Vec<_>>().join(" "))));
    assert!(code.last_group_matches(&format!("-{last}")));
    let wrong: String = last.chars().take(2).chain(std::iter::once(if last.ends_with('A') { 'B' } else { 'A' })).collect();
    assert!(!code.last_group_matches(&wrong));
    assert!(!code.last_group_matches(&last[..2]));
    assert!(!code.last_group_matches(""));
}

#[test]
fn the_account_key_sealed_by_the_code_is_77_bytes_as_the_doc_says_and_opens_by_hand() {
    let (key, code, record) = keys::new_account_key().unwrap();
    let sealed = b64(&record.sealed_by_code);
    assert_eq!(sealed.len(), 77);
    assert_eq!(sealed.len(), keys::SEALED_LEN);
    assert_eq!(&sealed[..4], b"HCR1");
    assert_eq!(sealed[4], key.epoch());
    // opened as the doc says, without the crate
    let code_key = hkdf(code.as_bytes(), "hersir-cloud/recovery-code/1", 32);
    let key_id = hkdf(key.as_bytes(), "hersir-cloud/key-id/1", 16);
    let mut aad = b"HCR1".to_vec();
    aad.push(sealed[4]);
    aad.extend_from_slice(&key_id);
    let opened = XChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(&code_key))
        .decrypt(XNonce::from_slice(&sealed[5..29]), Payload { msg: &sealed[29..], aad: &aad })
        .expect("the doc's recipe opens it");
    assert_eq!(&opened[..], &key.as_bytes()[..]);
    // and by the crate
    assert_eq!(keys::open_record(&record, &code).unwrap(), key);
    // the nonce is random: the same key sealed twice differs
    assert_ne!(key.seal_by_code(&code).unwrap(), key.seal_by_code(&code).unwrap());
}

#[test]
fn a_wrong_recovery_code_is_refused() {
    let (_, code, record) = keys::new_account_key().unwrap();
    let other = RecoveryCode::generate().unwrap();
    assert_ne!(other, code);
    assert_eq!(keys::open_record(&record, &other), Err(Error::WrongRecoveryCode));
    // a code with one bit changed, as if mistyped past its check character
    let mut bytes = *code.as_bytes();
    bytes[15] ^= 0x80;
    assert_eq!(keys::open_record(&record, &RecoveryCode::from_bytes(bytes)), Err(Error::WrongRecoveryCode));
    assert_eq!(Error::WrongRecoveryCode.to_string(), "That recovery code doesn't open this account's backups.");
}

#[test]
fn a_code_that_opens_a_key_whose_key_id_isnt_the_records_is_refused() {
    let (key, code, mut record) = keys::new_account_key().unwrap();
    let another = AccountKey::generate(1).unwrap();
    // the server's key id is another key's: the key id doesn't match
    record.key_id = another.key_id();
    assert_eq!(keys::open_record(&record, &code), Err(Error::WrongRecoveryCode));
    // and one that isn't a key id at all
    record.key_id = "not hex".into();
    assert_eq!(keys::open_record(&record, &code), Err(Error::WrongRecoveryCode));
    // the right one opens it, whatever its case
    record.key_id = key.key_id().to_uppercase();
    assert_eq!(keys::open_record(&record, &code).unwrap(), key);
}

#[test]
fn a_damaged_sealed_key_is_refused() {
    let (key, code, mut record) = keys::new_account_key().unwrap();
    let mut sealed = b64(&record.sealed_by_code);
    sealed[40] ^= 1;
    assert_eq!(keys::open_sealed(&sealed, &code, &key.key_id()), Err(Error::WrongRecoveryCode));
    assert!(matches!(keys::open_sealed(&sealed[..76], &code, &key.key_id()), Err(Error::Damaged(_))));
    record.sealed_by_code = "@@@".into();
    assert!(matches!(keys::open_record(&record, &code), Err(Error::Damaged(_))));
}

#[test]
fn a_new_recovery_code_seals_the_same_account_key_and_the_old_code_stops_working() {
    let k = keyed();
    let api = &k.cloud.api;
    let (new_code, record) = keys::new_recovery_code(&k.key).unwrap();
    assert_ne!(new_code, k.code);
    assert_eq!(record.key_id, k.key.key_id(), "the same account key");
    assert_eq!(record.epoch, k.key.epoch());
    // from the key's epoch: stored, not a first key
    let (stored, first) = api.put_key(&record, Some(k.key.epoch())).unwrap();
    assert!(!first);
    assert_eq!(stored, record);
    let got = api.key().unwrap().expect("GET /keys");
    assert_eq!(got, record);
    assert_eq!(keys::open_record(&got, &new_code).unwrap(), k.key);
    assert_eq!(keys::open_record(&got, &k.code), Err(Error::WrongRecoveryCode));
}

#[test]
fn putting_a_key_from_the_wrong_epoch_gets_epoch() {
    let k = keyed();
    let (_, record) = keys::new_recovery_code(&k.key).unwrap();
    // as if no key were there yet: another computer made one meanwhile
    match k.cloud.api.put_key(&record, None) {
        Err(Error::Epoch(m)) => assert!(!m.is_empty()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_account_without_a_key_has_none() {
    let c = common::cloud();
    assert_eq!(c.api.key().unwrap(), None);
}

#[test]
fn this_computers_ed25519_and_x25519_keys_work_and_go_to_the_keychain_and_back() {
    let a = DeviceKeys::generate().unwrap();
    let b = DeviceKeys::generate().unwrap();
    // Ed25519: a signature checks with the public half
    let sig = a.sign(b"hello");
    let vk = VerifyingKey::from_bytes(&a.sign_pub()).unwrap();
    assert!(vk.verify(b"hello", &Signature::from_bytes(&sig)).is_ok());
    assert!(vk.verify(b"hellp", &Signature::from_bytes(&sig)).is_err());
    // X25519: both sides agree
    assert_eq!(&a.agree(&b.box_pub())[..], &b.agree(&a.box_pub())[..]);
    assert_ne!(a.sign_pub(), b.sign_pub());
    // public halves in base64 of 32 bytes, as POST /devices takes them
    assert_eq!(b64(&a.sign_pub_b64()), a.sign_pub());
    assert_eq!(b64(&a.box_pub_b64()), a.box_pub());
    // the keychain's text, and Debug without the secrets
    let text = a.to_text();
    let back = DeviceKeys::from_text(&text).unwrap();
    assert_eq!(back.sign_pub(), a.sign_pub());
    assert_eq!(back.box_pub(), a.box_pub());
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let shown = format!("{a:?}");
    assert!(!shown.contains(v["sign"].as_str().unwrap()) && !shown.contains(v["box"].as_str().unwrap()), "{shown}");
    assert!(matches!(DeviceKeys::from_text("{}"), Err(Error::Damaged(_))));
    // a recovery code is never in Debug output
    let code = RecoveryCode::generate().unwrap();
    assert!(!format!("{code:?}").contains(&code.characters()[..4]));
}
