//! docs/backup-format.md, *For Hersir Cloud*, is what the PWA will read the formats from: it must say what the code
//! does. Its constants, its example recovery code and its example manifest are checked against the crate.

use std::collections::BTreeSet;

use gizai_cloud_client::backup::CloudManifest;
use gizai_cloud_client::chunks;
use gizai_cloud_client::keys::{self, RecoveryCode};
use serde_json::Value;

const DOC: &str = include_str!("../../../../docs/backup-format.md");

/// The doc's section for Hersir Cloud, up to the next `## ` heading.
fn section() -> &'static str {
    let start = DOC.find("\n## For Hersir Cloud").expect("a section For Hersir Cloud");
    let rest = &DOC[start + 1..];
    let end = rest[3..].find("\n## ").map_or(rest.len(), |i| i + 3);
    &rest[..end]
}

fn keys_of(v: &Value) -> BTreeSet<String> {
    v.as_object().unwrap_or_else(|| panic!("an object: {v}")).keys().cloned().collect()
}

#[test]
fn the_doc_names_the_hkdf_info_strings_the_alphabet_and_the_sizes_the_code_uses() {
    let s = section();
    for info in [keys::DATA_KEY_INFO, keys::ID_KEY_INFO, keys::KEY_ID_INFO, keys::CODE_KEY_INFO] {
        let info = std::str::from_utf8(info).unwrap();
        assert!(s.contains(&format!("`{info}`")), "the doc names {info}");
    }
    assert!(s.contains(&format!("`{}`", std::str::from_utf8(keys::CODE_ALPHABET).unwrap())));
    assert!(s.contains("Luhn mod 32"));
    assert!(s.contains(&format!("{} bytes", keys::SEALED_LEN)), "sealed_by_code's size");
    assert!(s.contains("\"HCR1\" (4) ‖ epoch (1) ‖ nonce (24)"));
    assert!(s.contains("\"HC1\" (3) ‖ epoch (1) ‖ nonce (24)"));
    assert_eq!(chunks::MAX_OBJECT, 4_194_368);
    assert!(s.contains("4,194,368 bytes"));
    assert_eq!(chunks::DB_CHUNK, 262_144);
    assert!(s.contains("256 KiB (262,144 bytes)"));
    assert!(s.contains("4 MiB (4,194,304 bytes)"));
    assert!(s.contains("FastCDC 2020"));
    assert!(s.contains("at least 512 KiB, about 1 MiB, at most 2 MiB"));
    assert_eq!((chunks::CDC_MIN, chunks::CDC_AVG, chunks::CDC_MAX), (512 * 1024, 1024 * 1024, 2 * 1024 * 1024));
    assert!(s.contains("HMAC-SHA256(id key, the chunk's bytes)"));
    assert!(s.contains("zstd at level 3"));
    assert_eq!(chunks::ZSTD_LEVEL, 3);
}

#[test]
fn the_docs_example_recovery_code_is_one() {
    let s = section();
    // the first `HC-…` with characters after it (not the prefix `HC-` alone)
    let at = s.match_indices("`HC-").map(|(i, _)| i).find(|i| s[i + 4..].starts_with(|c: char| c.is_ascii_alphanumeric()))
        .expect("an example code");
    let example = &s[at + 1..at + 1 + s[at + 1..].find('`').unwrap()];
    assert_eq!(example.len(), "HC-".len() + 27 + 6, "{example}");
    let code = RecoveryCode::parse(example).unwrap_or_else(|e| panic!("the doc's example {example} doesn't read: {e}"));
    assert_eq!(&code.shown()[..], example);
}

#[test]
fn the_docs_example_manifest_has_the_fields_a_backup_writes() {
    let s = section();
    let start = s.find("### The manifest").expect("The manifest");
    let block = &s[start..];
    let open = block.find("```json\n").expect("a JSON example") + "```json\n".len();
    let close = open + block[open..].find("```").unwrap();
    let doc: Value = serde_json::from_str(&block[open..close]).expect("the doc's example manifest is JSON");
    let mut written: CloudManifest = CloudManifest::default();
    written.snapshot.last_change = Some(Default::default());
    written.snapshot.files.push(Default::default());
    written.chunks.db.push("x".into());
    written.chunks.files.insert("y".into(), vec![]);
    let written = serde_json::to_value(&written).unwrap();
    assert_eq!(keys_of(&doc), keys_of(&written));
    for part in ["device", "db", "last_change", "chunks"] {
        assert_eq!(keys_of(&doc[part]), keys_of(&written[part]), "{part}");
    }
    assert_eq!(keys_of(&doc["files"][0]), keys_of(&written["files"][0]));
    assert_eq!(doc["format"], "gizai-backup");
    assert_eq!(doc["format_version"], 1);
    assert_eq!(doc["db"]["bytes"], 0, "in the cloud db.bytes is 0");
    assert!(doc["db"]["db_bytes"].as_i64().unwrap() > 0);
}
