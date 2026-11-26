//! Small helpers shared by the modules: base64, hex, random bytes, the time, and addresses.
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::{Error, Result};

/// Standard base64 with padding, as the protocol has it.
pub(crate) fn b64(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

/// Standard base64 with padding, read strictly; None when it isn't.
pub(crate) fn unb64(text: &str) -> Option<Vec<u8>> {
    STANDARD.decode(text.trim()).ok()
}

/// Lowercase hex.
pub(crate) fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(HEX[usize::from(b >> 4)]));
        out.push(char::from(HEX[usize::from(b & 15)]));
    }
    out
}

/// `s` is `len` hex digits, lowercase.
pub(crate) fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `n` random bytes from the system.
pub(crate) fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(|e| Error::Local(format!("This computer gave no random bytes: {e}")))?;
    Ok(bytes)
}

/// Now, in ms since 1970.
pub(crate) fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// A server's text for a message: one line of at most 300 characters.
pub(crate) fn clean(s: &str) -> String {
    let line: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let line = line.trim();
    if line.chars().count() > 300 { format!("{}…", line.chars().take(300).collect::<String>()) } else { line.to_string() }
}

/// Percent-encodes all but RFC 3986's unreserved characters: a path segment.
pub(crate) fn pct(s: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            out.push('%');
            out.push(char::from(HEX[usize::from(b >> 4)]));
            out.push(char::from(HEX[usize::from(b & 15)]));
        }
    }
    out
}

/// Undoes percent-encoding, and `+` for a space (a query or form part).
pub(crate) fn unpct(s: &str) -> String {
    let b = s.as_bytes();
    let hex = |i: usize| b.get(i).and_then(|c| char::from(*c).to_digit(16));
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if hex(i + 1).is_some() && hex(i + 2).is_some() => {
                out.push((hex(i + 1).unwrap_or(0) * 16 + hex(i + 2).unwrap_or(0)) as u8);
                i += 3;
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The host of an http(s) address, in lowercase, IPv6 in brackets; None when it isn't one (or has a user in it).
pub(crate) fn host_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.trim().split_once("://")?;
    if !(scheme.eq_ignore_ascii_case("https") || scheme.eq_ignore_ascii_case("http")) {
        return None;
    }
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    if authority.is_empty() || authority.contains(['@', '\\']) || authority.contains(char::is_whitespace) {
        return None;
    }
    let host = if authority.starts_with('[') {
        authority.find(']').map_or(authority, |end| &authority[..=end])
    } else {
        authority.split(':').next().unwrap_or(authority)
    };
    Some(host.to_ascii_lowercase())
}

/// Addresses on this computer, the only ones Hersir talks to over plain http.
pub(crate) fn is_loopback(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "[::1]")
}

/// Whether `url` is https, or http on this computer: the only addresses Hersir sends to.
pub(crate) fn safe_url(url: &str) -> bool {
    let Some(host) = host_of(url) else { return false };
    let https = url.trim().get(..8).is_some_and(|s| s.eq_ignore_ascii_case("https://"));
    https || is_loopback(&host)
}

/// An address for a message: without its query or fragment (a presigned URL's query is its key), at most 200 characters.
pub(crate) fn shown(url: &str) -> String {
    let base = url.trim().split(['?', '#']).next().unwrap_or_default();
    if base.chars().count() > 200 { format!("{}…", base.chars().take(200).collect::<String>()) } else { base.to_string() }
}
