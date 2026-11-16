//! GA-253: a release's signature (`release_sig`). The keys built into Hersir are release-signers' two lines; an
//! allowed-signers file (HERSIR_RELEASE_SIGNERS) replaces them for tests; a signature checks only when one of those keys
//! made it, for Hersir's releases, over the exact message. The signatures in tests/fixtures/release-sig were made once
//! with `ssh-keygen -Y sign` by throwaway keys whose private halves were never kept: hersir-9.9.9.sig by the key in its
//! `signers`, hersir-9.9.9.other-key.sig by another key, hersir-9.9.9.git-namespace.sig by the same key with `-n git`,
//! each over `hersir v9.9.9 0123…4567`, a newline and that folder's SHA256SUMS.
use std::path::{Path, PathBuf};

use gizai_agents::release_sig::{self as sig, SigProblem};

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
/// The key in tests/fixtures/release-sig/signers, as ssh-keygen -l shows it.
const THROWAWAY: &str = "SHA256:Fts0n5OxtQzACjpfRD3teRF5gOrJyH3wB0moaUXsZqg";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/release-sig").join(name)
}

fn read(name: &str) -> Vec<u8> {
    std::fs::read(fixture(name)).unwrap()
}

/// The message hersir-9.9.9.sig is over.
fn signed_message() -> Vec<u8> {
    sig::message("v9.9.9", COMMIT, &read("SHA256SUMS"))
}

fn throwaway() -> Vec<sig::Signer> {
    sig::signers(Some(&fixture("signers"))).unwrap()
}

// ---------- the keys built into Hersir ----------

#[test]
fn the_keys_built_into_hersir_are_release_signers_two_lines() {
    let file = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../release-signers")).unwrap();
    assert_eq!(sig::RELEASE_SIGNERS, file, "built in from release-signers as it is");
    let lines: Vec<&str> = file.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 2, "Jeffrey's release key and the spare: {file}");
    let built_in = sig::signers(None).expect("both lines parse");
    assert_eq!(built_in.len(), 2, "{built_in:?}");
    for (signer, line) in built_in.iter().zip(&lines) {
        // hersir-release namespaces="hersir-release" ssh-ed25519 <key> <comment>
        let words: Vec<&str> = line.splitn(5, ' ').collect();
        assert_eq!(&words[..3], ["hersir-release", r#"namespaces="hersir-release""#, "ssh-ed25519"], "{line}");
        let key = ssh_key::PublicKey::from_openssh(&format!("{} {}", words[2], words[3])).unwrap();
        assert_eq!(signer.key.key_data(), key.key_data(), "{line}");
        assert_eq!(signer.comment, words[4]);
        assert!(signer.fingerprint().starts_with("SHA256:"), "{}", signer.fingerprint());
    }
    assert_ne!(built_in[0].key, built_in[1].key, "two different keys");
    assert_eq!(sig::parse_signers(sig::RELEASE_SIGNERS).unwrap(), built_in);
}

#[test]
fn a_signature_by_another_key_fails_against_the_built_in_keys() {
    let built_in = sig::signers(None).unwrap();
    match sig::verify(&read("hersir-9.9.9.sig"), &signed_message(), &built_in) {
        Err(SigProblem::OtherKey(fingerprint)) => assert_eq!(fingerprint, THROWAWAY),
        other => panic!("a throwaway key's signature passed for a release: {other:?}"),
    }
    assert!(matches!(sig::verify(&read("hersir-9.9.9.other-key.sig"), &signed_message(), &built_in), Err(SigProblem::OtherKey(_))));
}

// ---------- HERSIR_RELEASE_SIGNERS and the check ----------

#[test]
fn an_allowed_signers_file_replaces_the_built_in_keys_and_a_good_signature_checks() {
    let signers = throwaway();
    assert_eq!(signers.len(), 1);
    assert_eq!(signers[0].comment, "QA throwaway release key");
    assert_eq!(signers[0].fingerprint(), THROWAWAY);
    let by = sig::verify(&read("hersir-9.9.9.sig"), &signed_message(), &signers).expect("it checks");
    assert_eq!(by.fingerprint(), THROWAWAY);
    // CRLF line ends in the .sig (a file saved on Windows) still read
    let crlf = String::from_utf8(read("hersir-9.9.9.sig")).unwrap().replace('\n', "\r\n");
    assert!(sig::verify(crlf.as_bytes(), &signed_message(), &signers).is_ok());
}

#[test]
fn the_message_is_the_tag_line_then_sha256sums_byte_for_byte() {
    assert_eq!(sig::message("v9.9.9", COMMIT, b"abc  hersir.tar.gz\n"),
               format!("hersir v9.9.9 {COMMIT}\nabc  hersir.tar.gz\n").into_bytes());
    assert_eq!(sig::message("v1.2.3", COMMIT, b""), format!("hersir v1.2.3 {COMMIT}\n").into_bytes());
}

#[test]
fn a_changed_sha256sums_another_commit_or_another_tag_doesnt_check() {
    let (signers, signature) = (throwaway(), read("hersir-9.9.9.sig"));
    let sums = read("SHA256SUMS");
    let mut changed = sums.clone();
    changed[0] = b'4';
    let mut longer = sums.clone();
    longer.extend_from_slice(b"\n");
    let other_commit = "1123456789abcdef0123456789abcdef01234567";
    for (what, message) in [("one changed sum", sig::message("v9.9.9", COMMIT, &changed)),
                            ("a blank line more", sig::message("v9.9.9", COMMIT, &longer)),
                            ("no SHA256SUMS", sig::message("v9.9.9", COMMIT, b"")),
                            ("another commit", sig::message("v9.9.9", other_commit, &sums)),
                            ("another tag", sig::message("v9.9.8", COMMIT, &sums))] {
        assert_eq!(sig::verify(&signature, &message, &signers).unwrap_err(), SigProblem::NoMatch, "{what}");
    }
}

#[test]
fn a_signature_by_another_key_for_another_use_or_no_signature_at_all_fails() {
    let signers = throwaway();
    let message = signed_message();
    assert!(matches!(sig::verify(&read("hersir-9.9.9.other-key.sig"), &message, &signers), Err(SigProblem::OtherKey(f)) if f != THROWAWAY));
    assert_eq!(sig::verify(&read("hersir-9.9.9.git-namespace.sig"), &message, &signers).unwrap_err(), SigProblem::Namespace("git".into()),
               "a git commit signature by the right key isn't a release signature");
    for junk in [&b""[..], b"not a signature", b"-----BEGIN SSH SIGNATURE-----\nAAAA\n-----END SSH SIGNATURE-----\n", &read("SHA256SUMS")] {
        assert!(matches!(sig::verify(junk, &message, &signers), Err(SigProblem::Unreadable(_))), "{:?}", String::from_utf8_lossy(junk));
    }
}

// ---------- allowed-signers lines ----------

#[test]
fn an_allowed_signers_line_counts_only_for_hersir_release_and_its_namespace() {
    let key = std::fs::read_to_string(fixture("signers")).unwrap().trim().splitn(3, ' ').nth(2).unwrap().to_string();
    let count = |text: String| sig::parse_signers(&text).map(|s| s.len());
    assert_eq!(count(format!("# Hersir's release keys\n\nhersir-release namespaces=\"hersir-release\" {key}\n")), Ok(1), "comments and blank lines");
    assert_eq!(count(format!("hersir-release {key}")), Ok(1), "a line without namespaces= allows every namespace");
    assert_eq!(count(format!("\"hersir-release\" namespaces=\"git,hersir-release\" {key}")), Ok(1));
    assert_eq!(count(format!("*,!someone namespaces=\"hersir-*\" {key}")), Ok(1), "OpenSSH's pattern lists");
    for (why, line) in [("another principal", format!("someone namespaces=\"hersir-release\" {key}")),
                        ("another namespace", format!("hersir-release namespaces=\"git\" {key}")),
                        ("the principal ruled out", format!("*,!hersir-release {key}")),
                        ("a certificate authority", format!("hersir-release cert-authority {key}")),
                        ("a time limit", format!("hersir-release valid-before=\"20300101\" {key}"))] {
        let e = sig::parse_signers(&line).unwrap_err();
        assert!(e.contains("no ed25519 key for hersir-release"), "{why}: {e}");
    }
    let e = sig::parse_signers("# nothing\nhersir-release namespaces=\"hersir-release\" ssh-ed25519 !!!notbase64 x\n").unwrap_err();
    assert!(e.contains("line 2 isn't an allowed signer"), "{e}");
    assert!(sig::parse_signers("hersir-release").unwrap_err().contains("line 1"), "a line without a key");
    assert!(sig::parse_signers("").is_err(), "no keys at all");
}

#[test]
fn hersir_release_signers_that_gives_no_key_says_why() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("missing");
    let e = sig::signers(Some(&missing)).unwrap_err();
    assert!(e.contains("HERSIR_RELEASE_SIGNERS") && e.contains("Can't read"), "{e}");
    let empty = tmp.path().join("empty");
    std::fs::write(&empty, "# nothing here\n").unwrap();
    let e = sig::signers(Some(&empty)).unwrap_err();
    assert!(e.contains("HERSIR_RELEASE_SIGNERS") && e.contains("holds no release key"), "{e}");
}
