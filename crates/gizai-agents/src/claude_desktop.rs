//! Claude Desktop's settings file (GA-338): where it is, whether Hersir's MCP server is in it, and putting that server in
//! it. Claude Desktop starts the stdio servers under `mcpServers` in `claude_desktop_config.json` when it starts (it
//! ignores `url` entries), so Hersir's is the `hersir-mcp` shim. Hersir changes only `mcpServers.hersir`: every other
//! server and key is written back as the very text it was, in its place, after the file is copied. A file that isn't JSON
//! with an `mcpServers` object is left as it is. Hersir passes the folder in, so a test never reads the real one.
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::value::RawValue;

/// The settings file's name, in Claude Desktop's folder (`folder`).
pub const FILE: &str = "claude_desktop_config.json";
/// Hersir's server's name under `mcpServers`.
pub const NAME: &str = "hersir";
/// The key Claude Desktop reads its servers from.
const SERVERS: &str = "mcpServers";

/// Where Claude Desktop keeps its settings on this computer; the folder is there once it is installed and ran:
/// macOS `~/Library/Application Support/Claude`, Windows `%APPDATA%\Claude`, Linux (and the rest) `$XDG_CONFIG_HOME/Claude`,
/// else `~/.config/Claude`. None without a home folder (on Windows, without %APPDATA%).
pub fn folder() -> Option<PathBuf> {
    folder_from(|k| std::env::var_os(k).or_else(|| (k == "HOME").then(std::env::home_dir).flatten().map(OsString::from)))
}

/// `folder`, with the environment variables read by `var`: HOME on macOS, APPDATA on Windows, XDG_CONFIG_HOME and HOME
/// elsewhere. One that is empty or not an absolute path counts as not set.
pub fn folder_from(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let dir = |k: &str| var(k).map(PathBuf::from).filter(|p| p.is_absolute());
    if cfg!(target_os = "macos") {
        dir("HOME").map(|h| h.join("Library").join("Application Support").join("Claude"))
    } else if cfg!(windows) {
        dir("APPDATA").map(|a| a.join("Claude"))
    } else {
        dir("XDG_CONFIG_HOME").or_else(|| dir("HOME").map(|h| h.join(".config"))).map(|c| c.join("Claude"))
    }
}

/// The settings file in `folder`, only when that folder is there (Claude Desktop is installed and ran once).
pub fn config_file(folder: Option<&Path>) -> Option<PathBuf> {
    folder.filter(|f| f.is_dir()).map(|f| f.join(FILE))
}

/// The file at `path` is JSON with Hersir's server in it: an object at `mcpServers.hersir`.
pub fn has_hersir(path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else { return false };
    serde_json::from_slice::<serde_json::Value>(&bytes).ok()
        .is_some_and(|v| v.get(SERVERS).and_then(|m| m.get(NAME)).is_some_and(serde_json::Value::is_object))
}

/// Hersir's server in Claude Desktop's settings: the `hersir-mcp` shim at `command` (an absolute path: Claude Desktop
/// doesn't get your shell's PATH) in its `--desktop` mode, with Hersir's socket and the token in its environment, as
/// Claude Desktop may pass a server only a few of your variables. Its fields in this order, as the entry shows them.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Server {
    pub command: String,
    pub args: Vec<String>,
    pub env: ServerEnv,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerEnv {
    #[serde(rename = "HERSIR_SOCKET")]
    pub socket: String,
    #[serde(rename = "HERSIR_TOKEN")]
    pub token: String,
}

impl Server {
    pub fn new(command: &Path, socket: &Path, token: &str) -> Server {
        Server {
            command: command.to_string_lossy().into_owned(),
            args: vec!["--desktop".into()],
            env: ServerEnv { socket: socket.to_string_lossy().into_owned(), token: token.into() },
        }
    }
}

impl std::fmt::Debug for Server {
    /// Like a derived one, without the token.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server").field("command", &self.command).field("args", &self.args).field("socket", &self.env.socket).finish()
    }
}

/// The entry for Claude Desktop's settings, to paste in by hand or as `add_server` puts it in a new file:
/// `{"mcpServers": {"hersir": …}}`, pretty-printed with two spaces.
pub fn entry(server: &Server) -> String {
    let doc = Out::Object(vec![(SERVERS, Out::Object(vec![(NAME, Out::Server(server))]))]);
    serde_json::to_string_pretty(&doc).expect("an entry is always JSON")
}

/// Why `add_server` left the settings file as it was.
#[derive(Debug)]
pub enum NotAdded {
    /// The file isn't JSON, or its `mcpServers` isn't an object: no copy was made either.
    NotJson,
    /// The file couldn't be read.
    Read(io::Error),
    /// The copy or the new file couldn't be written: a copy made is gone again.
    Write(io::Error),
}

/// Puts `server` in the settings file at `path` as `mcpServers.hersir`, in place of an earlier `hersir` server; every
/// other server and key stays as it was. The file is copied first, to `<file>.hersir-backup-<stamp>` in the same folder,
/// and then written whole: a new file in the same folder, renamed over it, 0600 on Linux and macOS. Without a file yet,
/// it is made with only Hersir's server. Returns the copy (None when there was no file).
pub fn add_server(path: &Path, server: &Server, stamp: &str) -> Result<Option<PathBuf>, NotAdded> {
    let old = match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(NotAdded::Read(e)),
    };
    let text = match &old {
        Some(bytes) => with_server(bytes, server).ok_or(NotAdded::NotJson)?,
        None => format!("{}\n", entry(server)),
    };
    let backup = match &old {
        Some(bytes) => Some(back_up(path, bytes, stamp).map_err(NotAdded::Write)?),
        None => None,
    };
    if let Err(e) = replace(path, text.as_bytes()) {
        if let Some(copy) = &backup {
            let _ = std::fs::remove_file(copy);
        }
        return Err(NotAdded::Write(e));
    }
    Ok(backup)
}

/// The file's text (`bytes`) with `server` as `mcpServers.hersir`, pretty-printed with two spaces: every other member as
/// the text it was, in its place, and Hersir's where an earlier one was, else last. None when the text isn't a JSON
/// object, or its `mcpServers` isn't an object. Of two members with one name the last counts, as in JSON.parse.
fn with_server(bytes: &[u8], server: &Server) -> Option<String> {
    let top: Members = serde_json::from_slice(bytes).ok()?;
    let at = top.0.iter().rposition(|(k, _)| k == SERVERS);
    let servers: Members = match at {
        Some(i) => serde_json::from_str(top.0[i].1.get()).ok()?,
        None => Members(Vec::new()),
    };
    let mut list: Vec<(&str, Out)> = Vec::new();
    for (k, v) in &servers.0 {
        match k.as_str() {
            NAME if list.iter().any(|(n, _)| *n == NAME) => {}
            NAME => list.push((NAME, Out::Server(server))),
            _ => list.push((k.as_str(), Out::Raw(v))),
        }
    }
    if !list.iter().any(|(n, _)| *n == NAME) {
        list.push((NAME, Out::Server(server)));
    }
    let mut ours = Some(Out::Object(list));
    let mut members: Vec<(&str, Out)> = Vec::new();
    for (i, (k, v)) in top.0.iter().enumerate() {
        match ours.take_if(|_| Some(i) == at) {
            Some(o) => members.push((k.as_str(), o)),
            None => members.push((k.as_str(), Out::Raw(v))),
        }
    }
    if let Some(o) = ours {
        members.push((SERVERS, o));
    }
    let mut text = serde_json::to_string_pretty(&Out::Object(members)).ok()?;
    if bytes.ends_with(b"\n") {
        text.push('\n');
    }
    Some(text)
}

/// An object's members in the file's order, each value as the very text it was.
struct Members(Vec<(String, Box<RawValue>)>);

impl<'de> Deserialize<'de> for Members {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Members, D::Error> {
        struct InOrder;
        impl<'de> Visitor<'de> for InOrder {
            type Value = Members;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Members, A::Error> {
                let mut members = Vec::new();
                while let Some(member) = map.next_entry::<String, Box<RawValue>>()? {
                    members.push(member);
                }
                Ok(Members(members))
            }
        }
        d.deserialize_map(InOrder)
    }
}

/// What is written: an object's members in order, a value as the text it was, or Hersir's server.
enum Out<'a> {
    Object(Vec<(&'a str, Out<'a>)>),
    Raw(&'a RawValue),
    Server(&'a Server),
}

impl Serialize for Out<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Out::Object(members) => {
                let mut m = s.serialize_map(Some(members.len()))?;
                for (k, v) in members {
                    m.serialize_entry(k, v)?;
                }
                m.end()
            }
            Out::Raw(raw) => raw.serialize(s),
            Out::Server(server) => server.serialize(s),
        }
    }
}

/// A new file at `path`, readable only by you on Linux and macOS (on Windows it takes its folder's access list); an error
/// when something is there already.
fn create_private(path: &Path) -> io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts.open(path)
}

/// Writes `bytes` to the new file `path`, which nothing else has open yet; on an error it is removed again.
fn fill(path: &Path, mut f: File, bytes: &[u8]) -> io::Result<()> {
    let done = f.write_all(bytes).and_then(|_| f.sync_all());
    drop(f);
    if done.is_err() {
        let _ = std::fs::remove_file(path);
    }
    done
}

/// Copies `bytes`, the file at `path` as Hersir read it, to `<file>.hersir-backup-<stamp>` in its folder, readable only by
/// you on Linux and macOS; `…-2`, `…-3` when that name is taken (never over an earlier copy). Returns the copy.
fn back_up(path: &Path, bytes: &[u8], stamp: &str) -> io::Result<PathBuf> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| FILE.into());
    for n in 1..100 {
        let copy = path.with_file_name(match n {
            1 => format!("{name}.hersir-backup-{stamp}"),
            n => format!("{name}.hersir-backup-{stamp}-{n}"),
        });
        match create_private(&copy) {
            Ok(f) => return fill(&copy, f, bytes).map(|_| copy),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "there are too many copies from this second already"))
}

/// Writes `bytes` as the file at `path`, whole: a new file in the same folder, renamed over it, so Claude Desktop never
/// reads half a file; 0600 on Linux and macOS. A link is followed: the file it points to is replaced, and the link stays.
fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let path = match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => std::fs::canonicalize(path)?,
        _ => path.to_path_buf(),
    };
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| FILE.into());
    let mut raw = [0u8; 8];
    getrandom::fill(&mut raw).map_err(|e| io::Error::other(format!("no random name for the new file: {e}")))?;
    let tmp = path.with_file_name(format!(".{name}.hersir-{}.tmp", raw.iter().map(|b| format!("{b:02x}")).collect::<String>()));
    fill(&tmp, create_private(&tmp)?, bytes)?;
    let done = private(&tmp).and_then(|_| std::fs::rename(&tmp, &path));
    if done.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    done
}

/// 0600 on Linux and macOS, whatever the umask made of it; nothing on Windows.
fn private(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
