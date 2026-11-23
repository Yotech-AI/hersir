//! GA-338: Claude Desktop's chat uses your Hersir board. Connect makes a lasting token for it, as you (`tools::CLAUDE_DESKTOP`;
//! GA-341: with the Chief's tools and rules, `tools::GizaiTools::desktop`), and gives the entry for Claude Desktop's
//! settings: the `hersir-mcp` shim in its `--desktop`
//! mode, with Hersir's socket and the token. Hersir can put that entry in the settings file itself
//! (`gizai_agents::claude_desktop::add_server`). Disconnect revokes the token, and the settings file stays as it is.
//! The token is in the entry only: Hersir keeps its sha256, never the token itself, in its database or its data folder.
use std::path::PathBuf;

use serde::Serialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::tools::CLAUDE_DESKTOP;
use gizai_agents::claude_desktop::{self as desktop, NotAdded};
use gizai_core::tokens;

/// Why Connect made no token: Hersir didn't find its shim.
pub const NO_SHIM: &str = "Hersir can't find its hersir-mcp program next to Hersir: reinstall Hersir.";

/// Why the entry wasn't put in the settings file, which Hersir left as it is: it isn't JSON with an `mcpServers` object.
pub const NOT_JSON: &str = "Claude Desktop's settings file can't be read as JSON with an mcpServers list, so Hersir left it as it is. \
    Paste the entry yourself: Claude Desktop → Settings → Developer → Edit Config.";

/// Why the entry wasn't put in the settings file: Claude Desktop's folder isn't on this computer.
pub const NO_FOLDER: &str = "Claude Desktop's settings folder wasn't found on this computer. Paste the entry yourself: Claude Desktop → \
    Settings → Developer → Edit Config.";

/// What Settings shows about Claude Desktop (`claude_desktop_status`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktopStatus {
    /// A Claude Desktop token is valid.
    pub connected: bool,
    /// When that token was made (ms); None without one.
    pub connected_at: Option<i64>,
    /// Claude Desktop's settings file, only when its folder is there (Claude Desktop is installed and ran once).
    pub config_path: Option<String>,
    /// That file is JSON with Hersir's server in it (`mcpServers.hersir`).
    pub config_has_hersir: bool,
    /// The `hersir-mcp` shim's absolute path; None when Hersir doesn't find it.
    pub shim_path: Option<String>,
}

/// What Connect gives (`claude_desktop_connect`): the entry, with the new token, and what became of the settings file.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktopConnection {
    /// `{"mcpServers": {"hersir": …}}`, pretty-printed with two spaces: the only place the token is given.
    pub entry: String,
    /// The entry is in Claude Desktop's settings file now.
    pub wrote_config: bool,
    pub config_path: Option<String>,
    /// The copy of the settings file from before Hersir changed it; None when it made the file, or didn't write it.
    pub backup_path: Option<String>,
    /// Why the entry isn't in the settings file, when it was asked to put it there; None otherwise.
    pub config_error: Option<String>,
    /// The status after connecting.
    pub status: ClaudeDesktopStatus,
}

impl std::fmt::Debug for ClaudeDesktopConnection {
    /// Like a derived one, without the entry: it holds the token.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClaudeDesktopConnection")
            .field("wrote_config", &self.wrote_config)
            .field("config_path", &self.config_path)
            .field("backup_path", &self.backup_path)
            .field("config_error", &self.config_error)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

/// The scope of Claude Desktop's token, exactly: `{"client": "claude-desktop"}`.
pub fn scope() -> Value {
    json!({"client": CLAUDE_DESKTOP})
}

/// The shim (`AppState::mcp_shim`, which Hersir finds with `mcp::shim_bin` when it starts) while it is there, as an
/// absolute path: Claude Desktop doesn't get your shell's PATH.
fn shim(st: &AppState) -> Option<PathBuf> {
    st.mcp_shim.as_deref().filter(|p| p.is_file()).and_then(|p| std::path::absolute(p).ok())
}

fn config_file(st: &AppState) -> Option<PathBuf> {
    desktop::config_file(st.claude_desktop_dir.as_deref())
}

fn text(p: PathBuf) -> String {
    p.to_string_lossy().into_owned()
}

pub fn status(st: &AppState) -> Result<ClaudeDesktopStatus, String> {
    let connected_at = tokens::made_at(&st.db, &scope()).map_err(|e| e.to_string())?;
    let config = config_file(st);
    Ok(ClaudeDesktopStatus {
        connected: connected_at.is_some(),
        connected_at,
        config_has_hersir: config.as_deref().is_some_and(desktop::has_hersir),
        config_path: config.map(text),
        shim_path: shim(st).map(text),
    })
}

/// Connect: a new lasting token for Claude Desktop, as you (`AppState::you_id`), which revokes the earlier one, and the
/// entry with it. With `add_to_config`, the entry goes in Claude Desktop's settings file too (a copy of the file first);
/// when it can't, the token and the entry are still there, and `config_error` says why. Without the shim nothing is made.
pub fn connect(st: &AppState, add_to_config: bool) -> Result<ClaudeDesktopConnection, String> {
    let shim = shim(st).ok_or(NO_SHIM)?;
    let token = tokens::mint_lasting(&st.db, &st.you_id, scope()).map_err(|e| e.to_string())?;
    let server = desktop::Server::new(&shim, &st.mcp_socket, &token);
    let config = config_file(st);
    let (wrote_config, backup, config_error) = match (add_to_config, &config) {
        (false, _) => (false, None, None),
        (true, None) => (false, None, Some(NO_FOLDER.to_string())),
        (true, Some(path)) => match desktop::add_server(path, &server, &backup_stamp()) {
            Ok(backup) => (true, backup, None),
            Err(NotAdded::NotJson) => (false, None, Some(NOT_JSON.to_string())),
            Err(NotAdded::Read(e)) => (false, None, Some(format!("Hersir couldn't read Claude Desktop's settings file ({e}), so it left it as \
                it is. Paste the entry yourself: Claude Desktop → Settings → Developer → Edit Config."))),
            Err(NotAdded::Write(e)) => (false, None, Some(format!("Hersir couldn't write Claude Desktop's settings file ({e}), so it left it \
                as it is. Paste the entry yourself: Claude Desktop → Settings → Developer → Edit Config."))),
        },
    };
    Ok(ClaudeDesktopConnection {
        entry: desktop::entry(&server),
        wrote_config,
        config_path: config.map(text),
        backup_path: backup.map(text),
        config_error,
        status: status(st)?,
    })
}

/// Disconnect: revokes Claude Desktop's token, so Hersir refuses the shim from then on (an open connection too, at its next
/// message: `mcp::serve_turn`). Claude Desktop's settings file stays as it is.
pub fn disconnect(st: &AppState) -> Result<ClaudeDesktopStatus, String> {
    tokens::revoke_scope(&st.db, &scope()).map_err(|e| e.to_string())?;
    status(st)
}

/// `YYYYMMDD-HHMMSS` now, in local time, for the copy of the settings file.
fn backup_stamp() -> String {
    let stamp = gizai_core::db::stamp(gizai_core::ids::now_ms());
    match stamp.rsplit_once('-') {
        Some((to_the_second, _ms)) => to_the_second.to_string(),
        None => stamp,
    }
}
