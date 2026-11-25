//! GA-266: what an agent's CLI gets of Hersir's own environment. A run starts with Hersir's environment, which can hold
//! other programs' keys: another CLI's API key, a GitHub token, AWS credentials. A variable whose name says it holds a
//! secret (`secret`) stays out of the run, unless the run's own CLI reads it to sign in (`own`). What the run's `Exec`
//! sets on top (the CLI's lines in Settings → Coding CLIs, and what Hersir sets for the run) it always gets.
use std::ffi::{OsStr, OsString};

use crate::cli::Kind;

/// Whether `name` is a secret's: it ends in `_API_KEY` or `_TOKEN`, has `SECRET` or `PASSWORD` in it, or starts with
/// `AWS_`, in any case.
pub fn secret(name: &OsStr) -> bool {
    let n = name.to_string_lossy().to_ascii_uppercase();
    n.ends_with("_API_KEY") || n.ends_with("_TOKEN") || n.contains("SECRET") || n.contains("PASSWORD") || n.starts_with("AWS_")
}

/// The variables with a secret's name that a CLI reads to sign in, as its own code has them (Claude Code 2.1.325,
/// Codex 0.163.0, Gemini CLI 0.65.0). Hersir knows of none for an Other CLI: its key goes in its lines in Settings →
/// Coding CLIs.
pub fn own(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::ClaudeCode => &[
            "ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "CLAUDE_CODE_OAUTH_TOKEN", "CLAUDE_CODE_OAUTH_REFRESH_TOKEN",
            "ANTHROPIC_FOUNDRY_API_KEY", "ANTHROPIC_AWS_API_KEY", "AWS_BEARER_TOKEN_BEDROCK",
        ],
        Kind::Codex => &["OPENAI_API_KEY", "CODEX_API_KEY", "CODEX_ACCESS_TOKEN"],
        Kind::Gemini => &["GEMINI_API_KEY", "GOOGLE_API_KEY", "GOOGLE_CLOUD_ACCESS_TOKEN"],
        Kind::Other => &[],
    }
}

/// Claude Code on Amazon Bedrock: with this on, it signs in with AWS's own variables (AWS_PROFILE,
/// AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, …), so a Claude Code run then keeps every `AWS_` one.
pub const BEDROCK: &str = "CLAUDE_CODE_USE_BEDROCK";

/// The variables of `from` (Hersir's environment) that a run of a `kind` CLI doesn't get: each with a secret's name
/// (`secret`), except its CLI's own (`own`; a Claude Code run on Amazon Bedrock keeps the `AWS_` ones too, see
/// `BEDROCK`) and those `set` sets (`Exec::env`), which it gets with the value set there. Without a `kind`, every one
/// with a secret's name that `set` doesn't set.
pub fn left_out(from: impl IntoIterator<Item = (OsString, OsString)>, kind: Option<Kind>, set: &[(String, String)]) -> Vec<OsString> {
    let from: Vec<(OsString, OsString)> = from.into_iter().collect();
    let own = kind.map(own).unwrap_or_default();
    // what the CLI's settings say wins over Hersir's own environment, as it does when the run starts
    let bedrock = kind == Some(Kind::ClaudeCode)
        && match set.iter().rev().find(|(k, _)| same(OsStr::new(k), BEDROCK)) {
            Some((_, v)) => on(OsStr::new(v)),
            None => from.iter().rev().find(|(k, _)| same(k, BEDROCK)).is_some_and(|(_, v)| on(v)),
        };
    from.into_iter().map(|(k, _)| k)
        .filter(|k| secret(k))
        .filter(|k| !own.iter().any(|o| same(k, o)))
        .filter(|k| !(bedrock && k.to_string_lossy().to_ascii_uppercase().starts_with("AWS_")))
        .filter(|k| !set.iter().any(|(s, _)| same(k, s)))
        .collect()
}

/// `left_out` of Hersir's own environment now.
pub fn left_out_now(kind: Option<Kind>, set: &[(String, String)]) -> Vec<OsString> {
    left_out(std::env::vars_os(), kind, set)
}

/// Whether two variable names are the same variable: as they are on Linux and macOS, in any case on Windows.
fn same(a: &OsStr, b: &str) -> bool {
    if cfg!(windows) { a.to_string_lossy().eq_ignore_ascii_case(b) } else { a == b }
}

/// A switch on as Claude Code reads one: 1, true, yes or on, in any case.
fn on(v: &OsStr) -> bool {
    ["1", "true", "yes", "on"].contains(&v.to_string_lossy().trim().to_ascii_lowercase().as_str())
}
