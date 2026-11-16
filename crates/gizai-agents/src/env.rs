//! Hersir's own environment variables (GA-249). Each has a `HERSIR_` name and the `GIZAI_` one it had before 0.17.0:
//! Hersir reads `HERSIR_<NAME>`, else `GIZAI_<NAME>`, so the old name still works alone and the new one wins when both
//! are set (an empty `HERSIR_<NAME>` gives way to a `GIZAI_<NAME>` that is set). What Hersir sets for another program
//! (a preview, the MCP shim) it sets under both names. The installers' and the updater's variables aren't read here,
//! and `GIZAI_RESULT` is a line in an agent's answer, not a variable.
use std::ffi::{OsStr, OsString};

/// The prefix of Hersir's variables.
pub const PREFIX: &str = "HERSIR_";
/// The prefix they had before 0.17.0, which still works.
pub const OLD_PREFIX: &str = "GIZAI_";

/// The `GIZAI_` name of the `HERSIR_` variable `name`: `GIZAI_DATA_DIR` for `HERSIR_DATA_DIR`. None for another name.
pub fn old_name(name: &str) -> Option<String> {
    name.strip_prefix(PREFIX).map(|rest| format!("{OLD_PREFIX}{rest}"))
}

/// The value of `name` (a `HERSIR_` variable), else of its `GIZAI_` name: `HERSIR_X` when it is set and not empty, else
/// `GIZAI_X` when that is set, else what `HERSIR_X` holds (empty, or nothing).
pub fn var_os(name: &str) -> Option<OsString> {
    let new = std::env::var_os(name);
    match new {
        Some(v) if !v.is_empty() => Some(v),
        _ => old_name(name).and_then(std::env::var_os).or(new),
    }
}

/// `var_os` as text: None when neither name is set, or the value that counts isn't valid Unicode.
pub fn var(name: &str) -> Option<String> {
    var_os(name).and_then(|v| v.into_string().ok())
}

/// Whether `key` is one of Hersir's variables, by either prefix, in any case (Windows' names are).
pub fn is_own(key: &OsStr) -> bool {
    let k = key.to_string_lossy().to_ascii_uppercase();
    k.starts_with(PREFIX) || k.starts_with(OLD_PREFIX)
}
