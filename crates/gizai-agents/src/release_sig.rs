//! A release's signature (GA-260, GA-253). Every release vX.Y.Z has the asset hersir-X.Y.Z.sig, which Jeffrey makes on
//! their own computer with scripts/sign-release.sh (`ssh-keygen -Y sign -n hersir-release`, with one of Hersir's release
//! keys) over this message, byte for byte: the line `hersir vX.Y.Z <commit>` and a newline, where `<commit>` is the
//! 40-character id of the commit the tag points to, followed by the release's SHA256SUMS exactly as uploaded.
//! - Update checks it here, in Hersir itself, never with a program from PATH, against the keys built into Hersir from
//!   `release-signers` at the repository's root (OpenSSH's allowed-signers format: Jeffrey's release key and a spare).
//!   Keys never come from a release.
//! - HERSIR_RELEASE_SIGNERS, a file in the same format, replaces the built-in keys: for tests, which sign fake releases
//!   with a throwaway key, never with Hersir's.
//! - Only ed25519 keys sign releases. Of an allowed-signers line Hersir reads its principals, which must include
//!   hersir-release, and its `namespaces` option, which (when the line has it) must include hersir-release too, as
//!   `ssh-keygen -Y verify -I hersir-release -n hersir-release` checks them. A line with another option
//!   (`cert-authority`, `valid-after`, `valid-before`) holds no key Hersir checks with.
use std::path::Path;

use ssh_key::{Algorithm, HashAlg, PublicKey, SshSig};

/// The namespace of a release signature (`ssh-keygen -Y sign -n hersir-release`), and the principal its key has in
/// release-signers: `hersir-release namespaces="hersir-release" ssh-ed25519 <key> <comment>`.
pub const NAMESPACE: &str = "hersir-release";

/// Hersir's release keys: `release-signers` at the repository's root, built into Hersir.
pub const RELEASE_SIGNERS: &str = include_str!("../../../release-signers");

/// The variable that names an allowed-signers file to check with instead of the built-in keys (tests).
pub const SIGNERS_VAR: &str = "HERSIR_RELEASE_SIGNERS";

/// A key that may sign Hersir's releases: one line of an allowed-signers file.
#[derive(Debug, Clone, PartialEq)]
pub struct Signer {
    pub key: PublicKey,
    /// The line's comment: "Hersir release key".
    pub comment: String,
}

impl Signer {
    /// The key's fingerprint, as `ssh-keygen -l` shows it: "SHA256:…".
    pub fn fingerprint(&self) -> String {
        self.key.fingerprint(HashAlg::Sha256).to_string()
    }
}

/// The message a release's signature is made over: the line `hersir <tag> <commit>` and a newline, then `sums` (the
/// release's SHA256SUMS) exactly as they are.
pub fn message(tag: &str, commit: &str, sums: &[u8]) -> Vec<u8> {
    let mut m = format!("hersir {tag} {commit}\n").into_bytes();
    m.extend_from_slice(sums);
    m
}

/// The keys in `text`, an allowed-signers file, that may sign Hersir's releases (see the module's doc). Empty lines and
/// `#` comments are skipped, and so is a line for another principal or namespace. Err: a line that isn't an allowed
/// signer at all (its number and why), or no key that may sign releases.
pub fn parse_signers(text: &str) -> Result<Vec<Signer>, String> {
    let mut signers = vec![];
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(signer) = parse_line(line).map_err(|why| format!("line {} isn't an allowed signer: {why}", i + 1))? {
            signers.push(signer);
        }
    }
    if signers.is_empty() {
        return Err(format!("it has no ed25519 key for {NAMESPACE}"));
    }
    Ok(signers)
}

/// The keys an update checks a release's signature with: those in the file `file` names (HERSIR_RELEASE_SIGNERS), else
/// the built-in ones. Err: why that file gives none, in a few words.
pub fn signers(file: Option<&Path>) -> Result<Vec<Signer>, String> {
    match file {
        None => parse_signers(RELEASE_SIGNERS).map_err(|why| format!("Hersir's own release keys can't be read: {why}")),
        Some(f) => {
            let text = std::fs::read_to_string(f).map_err(|e| format!("Can't read {SIGNERS_VAR} ({}): {e}", f.display()))?;
            parse_signers(&text).map_err(|why| format!("{SIGNERS_VAR} ({}) holds no release key: {why}", f.display()))
        }
    }
}

/// One line: `principals [options] keytype base64-key [comment]`. Ok(None): a line that doesn't let its key sign
/// Hersir's releases.
fn parse_line(line: &str) -> Result<Option<Signer>, String> {
    let (principals, rest) = word(line);
    let principals = unquote(principals);
    // the options come before the key, when the line has any: a key starts with its type
    let rest = rest.trim_start();
    let (options, key) = if is_key_type(rest.split_whitespace().next().unwrap_or("")) { ("", rest) } else { word(rest) };
    let key = key.trim();
    if key.is_empty() {
        return Err("it has no key".into());
    }
    let key = PublicKey::from_openssh(key).map_err(|e| format!("its key can't be read ({e})"))?;
    let mut namespaces = None;
    for option in split_outside_quotes(options, ',').into_iter().filter(|o| !o.is_empty()) {
        match option.split_once('=') {
            Some((name, value)) if name.eq_ignore_ascii_case("namespaces") => namespaces = Some(unquote(value).to_string()),
            // cert-authority, valid-after, valid-before, or one Hersir doesn't know
            _ => return Ok(None),
        }
    }
    let allowed = matches_list(principals, NAMESPACE) && namespaces.as_deref().is_none_or(|n| matches_list(n, NAMESPACE));
    if !allowed || key.algorithm() != Algorithm::Ed25519 {
        return Ok(None);
    }
    let comment = key.comment().to_string();
    Ok(Some(Signer { key, comment }))
}

fn is_key_type(word: &str) -> bool {
    ["ssh-", "ecdsa-", "sk-"].iter().any(|p| word.starts_with(p))
}

/// The first word of `s` (a quoted part may hold spaces), and the rest after it.
fn word(s: &str) -> (&str, &str) {
    let mut quoted = false;
    for (i, c) in s.char_indices() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => return (&s[..i], &s[i..]),
            _ => {}
        }
    }
    (s, "")
}

/// `s` split at each `sep` that isn't between double quotes.
fn split_outside_quotes(s: &str, sep: char) -> Vec<&str> {
    let (mut parts, mut start, mut quoted) = (vec![], 0, false);
    for (i, c) in s.char_indices() {
        if c == '"' {
            quoted = !quoted;
        } else if c == sep && !quoted {
            parts.push(&s[start..i]);
            start = i + c.len_utf8();
        }
    }
    parts.push(&s[start..]);
    parts
}

fn unquote(s: &str) -> &str {
    s.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(s)
}

/// Whether `name` matches OpenSSH's pattern-list `list` (ssh_config(5), PATTERNS): comma-separated patterns with `*` and
/// `?`, where a `!` in front of one rules out what it matches. One pattern must match, and no ruled-out one.
fn matches_list(list: &str, name: &str) -> bool {
    let mut matched = false;
    for pattern in list.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (negated, pattern) = pattern.strip_prefix('!').map_or((false, pattern), |p| (true, p));
        if glob(pattern.as_bytes(), name.as_bytes()) {
            if negated {
                return false;
            }
            matched = true;
        }
    }
    matched
}

/// `*` matches any run of characters, `?` any one; everything else matches itself.
fn glob(pattern: &[u8], name: &[u8]) -> bool {
    match (pattern.first(), name.first()) {
        (None, None) => true,
        (Some(b'*'), _) => glob(&pattern[1..], name) || (!name.is_empty() && glob(pattern, &name[1..])),
        (Some(b'?'), Some(_)) => glob(&pattern[1..], &name[1..]),
        (Some(p), Some(n)) if p == n => glob(&pattern[1..], &name[1..]),
        _ => false,
    }
}

/// Why a release's signature doesn't let it install.
#[derive(Debug, Clone, PartialEq)]
pub enum SigProblem {
    /// hersir-X.Y.Z.sig isn't an SSH signature Hersir can read (what ssh-key said).
    Unreadable(String),
    /// It is made for another use than Hersir's releases: its namespace.
    Namespace(String),
    /// It is made with a key that isn't one of the release keys: that key's fingerprint.
    OtherKey(String),
    /// It is made with one of the release keys, but not over this message: the release's SHA256SUMS or the commit of
    /// its tag isn't what was signed.
    NoMatch,
}

/// Checks `sig` (hersir-X.Y.Z.sig, as ssh-keygen writes it) over `message` against `signers`. Ok: the key that made it.
pub fn verify<'a>(sig: &[u8], message: &[u8], signers: &'a [Signer]) -> Result<&'a Signer, SigProblem> {
    let sig = SshSig::from_pem(sig).map_err(|e| SigProblem::Unreadable(e.to_string()))?;
    if sig.namespace() != NAMESPACE {
        return Err(SigProblem::Namespace(sig.namespace().to_string()));
    }
    let signer = signers.iter().find(|s| s.key.key_data() == sig.public_key())
        .ok_or_else(|| SigProblem::OtherKey(sig.public_key().fingerprint(HashAlg::Sha256).to_string()))?;
    signer.key.verify(NAMESPACE, message, &sig).map_err(|_| SigProblem::NoMatch)?;
    Ok(signer)
}
