//! Secrets Hersir keeps for you, like the sign-ins of MCP servers: in your keychain (the Secret Service on Linux, like GNOME
//! Keyring or KeePassXC), each under a key like `mcp/<server id>/oauth`, never in Hersir's database. Since 0.17.0 under
//! the service `hersir`, with the ones saved under `gizai` before still found (`WithOld`, GA-249).
//! Headless UI tests and QA runs use a file in its place (HERSIR_FAKE_KEYCHAIN), unit tests memory.
//! Errors are plain sentences and never hold a value.
use std::collections::{BTreeMap, HashMap};
use std::io::{ErrorKind, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

/// The keychain service every Hersir secret is saved under; the key is the account name.
pub const SERVICE: &str = "hersir";

/// The service Hersir saved its secrets under before 0.17.0, still read (`WithOld`).
pub const OLD_SERVICE: &str = "gizai";

/// What Gizai says when there is no Secret Service to keep its secrets.
pub const NOT_RUNNING: &str = "No keychain is running (Secret Service): start one, like GNOME Keyring or KeePassXC";

/// Where Gizai keeps secrets.
pub trait Keychain: Send + Sync {
    /// The secret under `key`; None when there is none.
    fn get(&self, key: &str) -> Result<Option<String>, String>;
    /// Saves `value` under `key`, in place of what was there.
    fn set(&self, key: &str, value: &str) -> Result<(), String>;
    /// Ok when it wasn't there.
    fn delete(&self, key: &str) -> Result<(), String>;
}

/// The OS keychain (Secret Service on Linux) through the keyring crate: one service (`SERVICE`, `OLD_SERVICE`), the key
/// as the user/account name.
#[derive(Debug, Clone, Copy)]
pub struct OsKeychain {
    service: &'static str,
}

impl OsKeychain {
    pub fn new(service: &'static str) -> Self {
        OsKeychain { service }
    }

    fn entry(&self, key: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(self.service, key).map_err(os_error)
    }
}

impl Keychain for OsKeychain {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        match self.entry(key)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(os_error(e)),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.entry(key)?.set_password(value).map_err(os_error)
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        match self.entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(os_error(e)),
        }
    }
}

/// What the OS keychain said, in plain words and never with a value.
fn os_error(e: keyring::Error) -> String {
    use keyring::Error as E;
    let reason = match &e {
        E::PlatformFailure(inner) | E::NoStorageAccess(inner) => {
            let said = inner.to_string();
            if not_running(&said) {
                return NOT_RUNNING.to_string();
            }
            if said.contains("prompt dismissed") {
                "its unlock prompt was dismissed".to_string()
            } else if said.contains("locked") {
                "it is locked: unlock it, then try again".to_string()
            } else {
                said
            }
        }
        E::NoEntry => "nothing is saved under this key".to_string(),
        E::BadEncoding(_) => "what it holds under this key isn't text".to_string(),
        E::TooLong(name, max) => format!("the {name} is longer than its {max} characters"),
        E::Invalid(name, why) => format!("the {name} isn't valid ({why})"),
        E::Ambiguous(items) => format!("it holds {} entries for this key", items.len()),
        other => other.to_string(),
    };
    format!("The keychain refused: {reason}")
}

/// Whether the Secret Service error says no keychain is running: no D-Bus session, or nothing that provides
/// org.freedesktop.secrets.
fn not_running(said: &str) -> bool {
    ["no secret service provider", "ServiceUnknown", "was not provided by any", "DBus.Error.Spawn"].iter().any(|s| said.contains(s))
}

/// Hersir's keychain since 0.17.0 (GA-249): `new` (the service `hersir`) with `old` (`gizai`, before 0.17.0) behind it.
/// A secret found only in `old` is used, and saved in `new` too; it stays in `old`, so an older Hersir still finds it
/// after a downgrade. Saving saves in `new`; deleting deletes in both.
#[derive(Debug, Default)]
pub struct WithOld<N, O> {
    pub new: N,
    pub old: O,
}

impl<N: Keychain, O: Keychain> Keychain for WithOld<N, O> {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        if let Some(value) = self.new.get(key)? {
            return Ok(Some(value));
        }
        let Some(value) = self.old.get(key)? else { return Ok(None) };
        // When this fails the secret still works, from `old`, and the next get tries again.
        if let Err(e) = self.new.set(key, &value) {
            eprintln!("gizai: saving a secret under the keychain service {SERVICE} too failed: {e}");
        }
        Ok(Some(value))
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.new.set(key, value)
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        let new = self.new.delete(key);
        let old = self.old.delete(key);
        new.and(old)
    }
}

/// In memory, for tests.
#[derive(Default)]
pub struct MemoryKeychain {
    values: Mutex<HashMap<String, String>>,
}

impl std::fmt::Debug for MemoryKeychain {
    // Only the keys: the values are secrets.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let values = self.values.lock().unwrap_or_else(PoisonError::into_inner);
        let mut keys: Vec<&String> = values.keys().collect();
        keys.sort();
        f.debug_struct("MemoryKeychain").field("keys", &keys).finish()
    }
}

impl Keychain for MemoryKeychain {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        Ok(self.values.lock().unwrap_or_else(PoisonError::into_inner).get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.values.lock().unwrap_or_else(PoisonError::into_inner).insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        self.values.lock().unwrap_or_else(PoisonError::into_inner).remove(key);
        Ok(())
    }
}

/// A JSON file (0600) standing in for the keychain in headless UI tests and QA runs: one object, key → value.
#[derive(Debug)]
pub struct FileKeychain {
    path: PathBuf,
    /// One change at a time within this process.
    lock: Mutex<()>,
}

impl FileKeychain {
    pub fn new(path: PathBuf) -> Self {
        FileKeychain { path, lock: Mutex::new(()) }
    }

    /// Everything in the file; nothing when there is no file yet.
    fn read(&self) -> Result<BTreeMap<String, String>, String> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) if text.trim().is_empty() => Ok(BTreeMap::new()),
            Ok(text) => serde_json::from_str(&text).map_err(|_| self.refused("isn't a JSON object of texts")),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(self.refused(&format!("can't be read ({e})"))),
        }
    }

    /// Writes a new file next to it (0600 from the start) and renames it over the old one, so it is never half written.
    /// Windows has no modes: a file in your own profile inherits its folder's ACL, which lets only you, SYSTEM and
    /// Administrators read it.
    fn write(&self, values: &BTreeMap<String, String>) -> Result<(), String> {
        let text = serde_json::to_string_pretty(values).map_err(|_| self.refused("can't be written"))?;
        if let Some(dir) = self.path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| self.refused(&format!("has no folder ({e})")))?;
        }
        let mut name = self.path.file_name().unwrap_or_default().to_os_string();
        name.push(format!(".{}.new", std::process::id()));
        let new = self.path.with_file_name(name);
        let _ = std::fs::remove_file(&new);
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let written = options.open(&new)
            .and_then(|mut f| f.write_all(text.as_bytes()).and_then(|()| f.sync_all()))
            .and_then(|()| std::fs::rename(&new, &self.path));
        written.map_err(|e| {
            let _ = std::fs::remove_file(&new);
            self.refused(&format!("can't be written ({e})"))
        })
    }

    fn refused(&self, why: &str) -> String {
        format!("The keychain refused: its test file {} {why}", self.path.display())
    }

    /// Reads the file, changes it with `change` and writes it back when `change` says it changed.
    fn change(&self, change: impl FnOnce(&mut BTreeMap<String, String>) -> bool) -> Result<(), String> {
        let _one_at_a_time = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let mut values = self.read()?;
        if change(&mut values) {
            self.write(&values)?;
        }
        Ok(())
    }
}

impl Keychain for FileKeychain {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        let _one_at_a_time = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        Ok(self.read()?.remove(key))
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.change(|values| {
            values.insert(key.to_string(), value.to_string());
            true
        })
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        self.change(|values| values.remove(key).is_some())
    }
}

/// HERSIR_FAKE_KEYCHAIN=<file> (or GIZAI_FAKE_KEYCHAIN) → FileKeychain, else the OS keychain: service `hersir`, with
/// `gizai` behind it (`WithOld`).
pub fn from_env() -> Arc<dyn Keychain> {
    match crate::env::var_os("HERSIR_FAKE_KEYCHAIN") {
        Some(path) if !path.is_empty() => Arc::new(FileKeychain::new(PathBuf::from(path))),
        _ => Arc::new(WithOld { new: OsKeychain::new(SERVICE), old: OsKeychain::new(OLD_SERVICE) }),
    }
}
