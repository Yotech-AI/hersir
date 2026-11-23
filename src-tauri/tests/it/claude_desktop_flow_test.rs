//! GA-338 QA: Claude Desktop's chat reads your Hersir board. Connect (`claude_desktop::connect`) makes a lasting token as
//! you with the scope `{"client":"claude-desktop"}` and gives the entry for Claude Desktop's settings; with
//! add_to_config it puts that entry in the settings file. Over the socket that token offers and runs only the tools that
//! read; any other is refused before it runs and changes nothing. A token whose scope has no chat, check or question
//! can't run a tool that writes either. Disconnect revokes the token. Claude's folder is always the test's own
//! (`test_state` points it at `<tmp>/claude-desktop`), never the real one.
use std::path::{Path, PathBuf};

use gizai_core::memory::{self, Who};
use gizai_core::model::{AgentInput, ProjectInput, TaskInput};
use gizai_core::{projects, tasks, team, tokens};
use gizai_lib::claude_desktop::{self as desktop, ClaudeDesktopConnection};
use gizai_lib::{AppState, tools};
use serde_json::{Value, json};

const NOT_JSON: &str = "Claude Desktop's settings file can't be read as JSON with an mcpServers list, so Hersir left it as it is. Paste the \
    entry yourself: Claude Desktop → Settings → Developer → Edit Config.";
const NO_FOLDER: &str = "Claude Desktop's settings folder wasn't found on this computer. Paste the entry yourself: Claude Desktop → Settings → \
    Developer → Edit Config.";
const NO_SHIM: &str = "Hersir can't find its hersir-mcp program next to Hersir: reinstall Hersir.";

/// The card's refusal for Claude Desktop, word for word.
fn not_in_desktop(tool: &str) -> String {
    format!("{tool} isn't available in Claude Desktop yet: Claude Desktop can only read your Hersir board for now.")
}

struct T {
    st: AppState,
    tmp: tempfile::TempDir,
}

/// Hersir for a test with a stand-in for the hersir-mcp program next to it (Connect only needs it to be a file).
fn setup() -> T {
    let tmp = tempfile::tempdir().unwrap();
    let mut st = gizai_lib::test_state(tmp.path());
    let bin = tmp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let shim = bin.join(format!("hersir-mcp{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&shim, "stand-in").unwrap();
    st.mcp_shim = Some(shim);
    T { st, tmp }
}

impl T {
    fn claude_dir(&self) -> PathBuf {
        self.tmp.path().join("claude-desktop")
    }
    /// Claude Desktop is installed and ran once: its folder is there.
    fn install_claude(&self) -> PathBuf {
        std::fs::create_dir_all(self.claude_dir()).unwrap();
        self.claude_dir().join("claude_desktop_config.json")
    }
    fn connect(&self, add: bool) -> ClaudeDesktopConnection {
        desktop::connect(&self.st, add).unwrap()
    }
    fn token_rows(&self) -> i64 {
        self.st.db.read(|c| Ok(c.query_row("SELECT count(*) FROM api_tokens", [], |r| r.get(0))?)).unwrap()
    }
}

fn server_of(c: &ClaudeDesktopConnection) -> Value {
    let v: Value = serde_json::from_str(&c.entry).unwrap();
    assert_eq!(v.as_object().unwrap().len(), 1, "{}", c.entry);
    assert_eq!(v["mcpServers"].as_object().unwrap().len(), 1, "{}", c.entry);
    v["mcpServers"]["hersir"].clone()
}

fn token_of(c: &ClaudeDesktopConnection) -> String {
    server_of(c)["env"]["HERSIR_TOKEN"].as_str().unwrap().to_string()
}

/// The names in `dir`, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

// ---- Connect: the entry and the token ----

#[test]
fn connect_without_adding_gives_the_entry_with_the_shims_absolute_path_desktop_mode_the_socket_and_a_64_hex_token() {
    let t = setup();
    let config = t.install_claude();
    let c = t.connect(false);
    let s = server_of(&c);
    let shim = t.st.mcp_shim.clone().unwrap();
    assert_eq!(s["command"], shim.to_string_lossy().as_ref());
    assert!(Path::new(s["command"].as_str().unwrap()).is_absolute());
    assert_eq!(s["args"], json!(["--desktop"]));
    assert_eq!(s["env"]["HERSIR_SOCKET"], t.st.mcp_socket.to_string_lossy().as_ref(), "Hersir's socket (its pipe on Windows)");
    let token = token_of(&c);
    assert_eq!(token.len(), 64);
    assert!(token.chars().all(|ch| ch.is_ascii_hexdigit()), "{token}");
    // nothing else in the server, and its fields in the card's order (in the text: a parsed Value sorts its keys)
    assert_eq!(s.as_object().unwrap().len(), 3);
    assert_eq!(s["env"].as_object().unwrap().len(), 2);
    let at: Vec<usize> = ["\"command\"", "\"args\"", "\"env\"", "\"HERSIR_SOCKET\"", "\"HERSIR_TOKEN\""].iter()
        .map(|k| c.entry.find(k).unwrap_or_else(|| panic!("{k} in {}", c.entry))).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{at:?}\n{}", c.entry);
    // pretty-printed with two spaces
    assert!(c.entry.starts_with("{\n  \"mcpServers\": {\n    \"hersir\": {\n      \"command\": "), "{}", c.entry);
    // without add_to_config the settings file isn't read or written
    assert!(!c.wrote_config);
    assert_eq!((c.backup_path.as_deref(), c.config_error.as_deref()), (None, None));
    assert!(!config.exists(), "no settings file made");
    assert!(names(&t.claude_dir()).is_empty());
    assert!(c.status.connected);
    assert!(!c.status.config_has_hersir);
    assert_eq!(c.status.shim_path.as_deref(), Some(shim.to_string_lossy().as_ref()));
    assert_eq!(c.config_path.as_deref(), Some(config.to_string_lossy().as_ref()));
}

#[test]
fn the_token_is_stored_as_its_sha256_with_no_expiry_the_scope_client_claude_desktop_and_you_as_its_actor_and_nowhere_itself() {
    let t = setup();
    let before = gizai_core::ids::now_ms();
    let c = t.connect(false);
    let token = token_of(&c);
    let (actor, scope, expires, revoked): (String, String, Option<i64>, Option<i64>) = t.st.db.read(|db| Ok(db.query_row(
        "SELECT actor_id, scopes_json, expires_at, revoked_at FROM api_tokens WHERE token_sha256=?1", [tokens::sha256_hex(&token)],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?)).unwrap();
    assert_eq!(actor, t.st.you_id, "you, the person running Hersir");
    assert_eq!(Who::of(&t.st.db, &actor).unwrap(), Who::Person(t.st.you_id.clone()), "memory reads it as a person");
    assert_eq!(scope, r#"{"client":"claude-desktop"}"#);
    assert_eq!((expires, revoked), (None, None));
    let at = c.status.connected_at.expect("connectedAt");
    assert!(at >= before && at <= gizai_core::ids::now_ms(), "{at}");
    // the token itself is in neither Hersir's database nor its data folder
    let rows: Vec<String> = t.st.db.read(|db| {
        let mut hits = vec![];
        let tables: Vec<String> = db.prepare("SELECT name FROM sqlite_master WHERE type='table'")?
            .query_map([], |r| r.get::<_, String>(0))?.collect::<Result<_, _>>()?;
        for table in tables {
            let cols: Vec<String> = db.prepare("SELECT name FROM pragma_table_info(?1)")?
                .query_map([&table], |r| r.get::<_, String>(0))?.collect::<Result<_, _>>()?;
            for col in cols {
                let n: i64 = db.query_row(&format!("SELECT count(*) FROM \"{table}\" WHERE instr(CAST(\"{col}\" AS TEXT), ?1) > 0"), [&token],
                                          |r| r.get(0))?;
                if n > 0 {
                    hits.push(format!("{table}.{col}"));
                }
            }
        }
        Ok(hits)
    }).unwrap();
    assert!(rows.is_empty(), "the token is in the database: {rows:?}");
    fn walk(dir: &Path, needle: &[u8], out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            let m = std::fs::symlink_metadata(&p).unwrap();
            if m.is_dir() {
                walk(&p, needle, out);
            } else if m.is_file() {
                // On Windows the open database is locked against reading; its rows are checked above. On Linux and macOS
                // every file is read, the database and its WAL too.
                let bytes = match std::fs::read(&p) {
                    Ok(b) => b,
                    Err(e) if cfg!(windows) => {
                        eprintln!("not read: {} ({e})", p.display());
                        continue;
                    }
                    Err(e) => panic!("{}: {e}", p.display()),
                };
                if bytes.windows(needle.len()).any(|w| w == needle) {
                    out.push(p);
                }
            }
        }
    }
    let mut found = vec![];
    walk(&t.st.data_dir, token.as_bytes(), &mut found);
    assert!(found.is_empty(), "the token is in Hersir's data folder: {found:?}");
    // and it isn't in the Debug text of what Connect gives
    assert!(!format!("{c:?}").contains(&token));
}

#[test]
fn connecting_again_revokes_the_earlier_token_and_leaves_one_valid() {
    let t = setup();
    let first = token_of(&t.connect(false));
    let second = token_of(&t.connect(false));
    assert_ne!(first, second);
    assert!(tokens::verify(&t.st.db, &first).unwrap().is_none(), "the earlier token is revoked");
    assert!(tokens::verify(&t.st.db, &second).unwrap().is_some());
    let valid: i64 = t.st.db.read(|c| Ok(c.query_row(
        "SELECT count(*) FROM api_tokens WHERE scopes_json='{\"client\":\"claude-desktop\"}' AND revoked_at IS NULL", [], |r| r.get(0))?)).unwrap();
    assert_eq!(valid, 1);
}

#[test]
fn without_hersir_mcp_next_to_hersir_connect_fails_with_the_cards_sentence_and_makes_no_token() {
    let mut t = setup();
    t.install_claude();
    for shim in [None, Some(t.tmp.path().join("bin").join("not-there"))] {
        t.st.mcp_shim = shim;
        let rows = t.token_rows();
        assert_eq!(desktop::connect(&t.st, true).unwrap_err(), NO_SHIM);
        assert_eq!(desktop::connect(&t.st, false).unwrap_err(), NO_SHIM);
        assert_eq!(t.token_rows(), rows, "no token is made");
        let s = desktop::status(&t.st).unwrap();
        assert_eq!((s.connected, s.connected_at, s.shim_path), (false, None, None));
        assert!(names(&t.claude_dir()).is_empty(), "and nothing is written");
    }
}

// ---- the status and its JSON ----

#[test]
fn the_status_and_the_connection_use_the_cards_camel_case_fields() {
    let t = setup();
    let s = desktop::status(&t.st).unwrap();
    let v = serde_json::to_value(&s).unwrap();
    let mut keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["configHasHersir", "configPath", "connected", "connectedAt", "shimPath"]);
    assert_eq!(v["connected"], false);
    assert!(v["connectedAt"].is_null() && v["configPath"].is_null(), "{v}");
    let c = serde_json::to_value(t.connect(true)).unwrap();
    let mut keys: Vec<&String> = c.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["backupPath", "configError", "configPath", "entry", "status", "wroteConfig"]);
    assert!(c["status"]["connectedAt"].is_i64(), "{c}");
    assert_eq!(c["status"]["connected"], true);
}

#[test]
fn tests_point_claudes_folder_at_their_own_temporary_folder() {
    let t = setup();
    let dir = t.st.claude_desktop_dir.clone().unwrap();
    assert!(dir.starts_with(t.tmp.path()), "{dir:?}");
    assert_ne!(Some(dir), gizai_agents::claude_desktop::folder(), "never the real Claude Desktop's folder");
}

// ---- Add to Claude Desktop ----

const OLD: &str = "{\n  \"globalShortcut\": \"Ctrl+Space\",\n  \"mcpServers\": {\n    \"filesystem\": {\"command\": \"npx\", \"args\": [\"-y\", \
    \"@modelcontextprotocol/server-filesystem\", \"/home/j\"]}\n  },\n  \"preferences\": {\"menuBarEnabled\": false}\n}\n";

#[test]
fn adding_to_an_existing_settings_file_copies_it_first_keeps_the_other_server_and_keys_and_sets_hersir_as_in_the_entry() {
    let t = setup();
    let config = t.install_claude();
    std::fs::write(&config, OLD).unwrap();
    assert!(!desktop::status(&t.st).unwrap().config_has_hersir);
    let c = t.connect(true);
    assert!(c.wrote_config, "{c:?}");
    assert_eq!(c.config_error, None);
    assert_eq!(c.config_path.as_deref(), Some(config.to_string_lossy().as_ref()));
    // the copy: claude_desktop_config.json.hersir-backup-YYYYMMDD-HHMMSS in the same folder, with exactly the old bytes
    let backup = PathBuf::from(c.backup_path.clone().expect("a copy"));
    assert_eq!(backup.parent(), Some(t.claude_dir().as_path()));
    let name = backup.file_name().unwrap().to_string_lossy().into_owned();
    let stamp = name.strip_prefix("claude_desktop_config.json.hersir-backup-").unwrap_or_else(|| panic!("{name}"));
    let (day, time) = stamp.split_once('-').unwrap_or_else(|| panic!("{stamp}"));
    assert!(day.len() == 8 && time.len() == 6 && format!("{day}{time}").chars().all(|ch| ch.is_ascii_digit()), "{stamp}");
    assert!(day.starts_with("20"), "{stamp}");
    assert_eq!(std::fs::read(&backup).unwrap(), OLD.as_bytes());
    assert_eq!(names(&t.claude_dir()), ["claude_desktop_config.json".to_string(), name.clone()], "no temp file left");
    // the new file: both servers, the other keys unchanged, hersir as in the entry
    let v: Value = serde_json::from_slice(&std::fs::read(&config).unwrap()).unwrap();
    let old: Value = serde_json::from_str(OLD).unwrap();
    assert_eq!(v["globalShortcut"], old["globalShortcut"]);
    assert_eq!(v["preferences"], old["preferences"]);
    assert_eq!(v["mcpServers"]["filesystem"], old["mcpServers"]["filesystem"]);
    assert_eq!(v["mcpServers"]["hersir"], server_of(&c));
    assert_eq!(v["mcpServers"].as_object().unwrap().len(), 2);
    assert_eq!(v.as_object().unwrap().len(), 3);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&config).unwrap().permissions().mode() & 0o777, 0o600);
    }
    assert!(c.status.config_has_hersir);
    let s = desktop::status(&t.st).unwrap();
    assert!(s.config_has_hersir && s.connected);
    assert_eq!(s.config_path.as_deref(), Some(config.to_string_lossy().as_ref()));
    // the token in the file is the valid one
    assert!(tokens::verify(&t.st.db, v["mcpServers"]["hersir"]["env"]["HERSIR_TOKEN"].as_str().unwrap()).unwrap().is_some());
}

#[test]
fn connecting_again_with_add_replaces_hersirs_server_with_the_new_token_and_keeps_the_rest() {
    let t = setup();
    let config = t.install_claude();
    std::fs::write(&config, OLD).unwrap();
    let first = t.connect(true);
    let second = t.connect(true);
    assert!(second.wrote_config);
    let v: Value = serde_json::from_slice(&std::fs::read(&config).unwrap()).unwrap();
    assert_eq!(v["mcpServers"]["hersir"]["env"]["HERSIR_TOKEN"], token_of(&second).as_str());
    assert_ne!(token_of(&first), token_of(&second));
    assert_eq!(v["mcpServers"].as_object().unwrap().keys().collect::<Vec<_>>(), ["filesystem", "hersir"]);
    // the second copy holds the file as the first Connect left it
    let copy = std::fs::read_to_string(second.backup_path.unwrap()).unwrap();
    assert!(copy.contains(&token_of(&first)), "{copy}");
}

#[test]
fn with_claudes_folder_but_no_settings_file_it_is_made_with_only_hersir_and_there_is_no_copy() {
    let t = setup();
    let config = t.install_claude();
    let c = t.connect(true);
    assert!(c.wrote_config);
    assert_eq!((c.backup_path.as_deref(), c.config_error.as_deref()), (None, None));
    let v: Value = serde_json::from_slice(&std::fs::read(&config).unwrap()).unwrap();
    assert_eq!(v, serde_json::from_str::<Value>(&c.entry).unwrap(), "only the hersir server");
    assert_eq!(names(&t.claude_dir()), ["claude_desktop_config.json"]);
    assert!(desktop::status(&t.st).unwrap().config_has_hersir);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&config).unwrap().permissions().mode() & 0o777, 0o600);
    }
}

#[test]
fn a_settings_file_that_isnt_valid_json_is_left_byte_for_byte_with_no_copy_and_the_entry_still_comes_with_a_valid_token() {
    for bad in ["{\"mcpServers\": {\"filesystem\": {\"command\": \"npx\"},}}", "{\"mcpServers\": [\"a\"]}"] {
        let t = setup();
        let config = t.install_claude();
        std::fs::write(&config, bad).unwrap();
        let c = t.connect(true);
        assert!(!c.wrote_config, "{bad}");
        assert_eq!(c.config_error.as_deref(), Some(NOT_JSON), "{bad}");
        assert_eq!(c.backup_path, None);
        assert_eq!(std::fs::read(&config).unwrap(), bad.as_bytes(), "byte for byte unchanged");
        assert_eq!(names(&t.claude_dir()), ["claude_desktop_config.json"], "no copy");
        let token = token_of(&c);
        assert_eq!(token.len(), 64);
        assert!(tokens::verify(&t.st.db, &token).unwrap().is_some(), "a valid token");
        assert!(c.status.connected && !c.status.config_has_hersir);
    }
}

#[test]
fn without_claudes_folder_config_path_is_null_the_error_says_so_and_nothing_is_written() {
    let t = setup();
    let c = t.connect(true);
    assert_eq!(c.config_path, None);
    assert!(!c.wrote_config);
    assert_eq!(c.config_error.as_deref(), Some(NO_FOLDER));
    assert_eq!(c.backup_path, None);
    assert!(!t.claude_dir().exists(), "the folder isn't made");
    assert!(tokens::verify(&t.st.db, &token_of(&c)).unwrap().is_some(), "the token and the entry are still there");
    let s = desktop::status(&t.st).unwrap();
    assert_eq!((s.config_path, s.config_has_hersir), (None, false));
    // without add_to_config there is no error
    assert_eq!(t.connect(false).config_error, None);
}

// ---- Disconnect ----

#[test]
fn disconnect_revokes_the_token_and_leaves_the_settings_file_as_it_is() {
    let t = setup();
    let config = t.install_claude();
    let c = t.connect(true);
    let before = std::fs::read(&config).unwrap();
    let s = desktop::disconnect(&t.st).unwrap();
    assert!(!s.connected);
    assert_eq!(s.connected_at, None);
    assert!(s.config_has_hersir, "the shim stays in Claude Desktop's settings");
    assert_eq!(std::fs::read(&config).unwrap(), before, "the settings file is unchanged");
    assert_eq!(names(&t.claude_dir()), ["claude_desktop_config.json"]);
    assert!(tokens::verify(&t.st.db, &token_of(&c)).unwrap().is_none(), "the token is revoked");
    // twice is fine
    assert!(!desktop::disconnect(&t.st).unwrap().connected);
}

// ---- over the socket (Linux and macOS: the Unix socket) ----

#[cfg(unix)]
mod socket {
    use super::*;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
    use tokio::net::UnixStream;
    use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

    const WAIT: std::time::Duration = std::time::Duration::from_secs(10);

    /// One open connection, as the shim makes it: the hello line with the token, then one JSON-RPC message per line.
    struct Conn {
        lines: Lines<BufReader<OwnedReadHalf>>,
        w: OwnedWriteHalf,
    }

    impl Conn {
        async fn open(st: &AppState, token: &str) -> Conn {
            let (r, mut w) = UnixStream::connect(&st.mcp_socket).await.unwrap().into_split();
            w.write_all(gizai_mcp::hello_line(token).as_bytes()).await.unwrap();
            Conn { lines: BufReader::new(r).lines(), w }
        }
        async fn send(&mut self, msg: Value) {
            self.w.write_all(format!("{msg}\n").as_bytes()).await.unwrap();
        }
        /// The next line from Hersir; None when it closed the connection.
        async fn next(&mut self) -> Option<Value> {
            match tokio::time::timeout(WAIT, self.lines.next_line()).await {
                Ok(Ok(Some(l))) => Some(serde_json::from_str(&l).unwrap()),
                Ok(_) => None,
                Err(_) => panic!("Hersir didn't answer in time"),
            }
        }
        async fn ask(&mut self, msg: Value) -> Option<Value> {
            self.send(msg).await;
            self.next().await
        }
        async fn initialize(&mut self) -> Value {
            let r = self.ask(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}})).await.unwrap();
            self.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).await;
            r
        }
        async fn call(&mut self, id: i64, name: &str, args: Value) -> Value {
            let r = self.ask(json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": args}})).await
                .unwrap_or_else(|| panic!("{name}: no answer"));
            assert_eq!(r["id"], id, "{r}");
            r["result"].clone()
        }
    }

    fn text(result: &Value) -> String {
        result["content"][0]["text"].as_str().unwrap_or_default().to_string()
    }

    /// Every row of every table, as text: to see that a call changed nothing.
    fn dump(st: &AppState) -> Vec<String> {
        st.db.read(|c| {
            let tables: Vec<String> = c.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?
                .query_map([], |r| r.get::<_, String>(0))?.collect::<Result<_, _>>()?;
            let mut out = vec![];
            for table in tables {
                let mut q = c.prepare(&format!("SELECT * FROM \"{table}\""))?;
                let n = q.column_count();
                let mut rows = q.query([])?;
                let mut lines = vec![];
                while let Some(r) = rows.next()? {
                    lines.push(format!("{table}: {:?}", (0..n).map(|i| format!("{:?}", r.get_ref(i))).collect::<Vec<_>>()));
                }
                lines.sort();
                out.extend(lines);
            }
            Ok(out)
        }).unwrap()
    }

    /// The tables a refused call must not touch, with their row counts: cards, comments, the activity lines (`changes`),
    /// calls waiting for Approve, memory notes and docs, runs and chats.
    fn counts(st: &AppState) -> Vec<(String, i64)> {
        ["tasks", "comments", "changes", "pending_actions", "docs", "doc_versions", "runs", "chat_threads", "chat_messages"].iter().map(|t| {
            (t.to_string(), st.db.read(|c| Ok(c.query_row(&format!("SELECT count(*) FROM \"{t}\""), [], |r| r.get::<_, i64>(0))?)).unwrap())
        }).collect()
    }

    struct S {
        t: T,
        lead: String,
        _server: tokio::task::JoinHandle<()>,
    }

    /// Hersir with its socket served, a Team Lead with Chat on, project KADE with KADE-1 in To do and a memory note.
    fn served() -> S {
        let t = setup();
        let st = &t.st;
        let team_id = team::list(&st.db).unwrap()[0].id.clone();
        let lead = team::add_agent(&st.db, &st.you_id, &team_id, AgentInput { name: "Team Lead".into(), role_key: "lead".into(),
            chat_enabled: Some(true), ..Default::default() }).unwrap();
        let project = projects::create(&st.db, &st.you_id, ProjectInput { name: "Kade portal".into(), key: "KADE".into(), ..Default::default() }).unwrap();
        tasks::create(&st.db, &st.you_id, TaskInput { project_id: project, title: "Export invoices as CSV".into(), ..Default::default() }).unwrap();
        memory::write(&st.db, &Who::Person(st.you_id.clone()), "Decisions/Exports", "Exports are CSV, for the accountant.", None, None).unwrap();
        let server = gizai_lib::mcp::start(st).unwrap();
        S { t, lead, _server: server }
    }

    /// The tools that change something, with arguments that would work if the tool ran.
    fn writes() -> Vec<(&'static str, Value)> {
        vec![
            ("create_task", json!({"project": "KADE", "title": "Made from Claude Desktop"})),
            ("move_task", json!({"task": "KADE-1", "column": "Done"})),
            ("comment_on_task", json!({"task": "KADE-1", "body_md": "A comment from Claude Desktop"})),
            ("start_agent_run", json!({"task": "KADE-1", "agent": "Team Lead"})),
            ("memory_write", json!({"path": "Decisions/From Claude Desktop", "body_md": "Written from Claude Desktop."})),
            ("merge_pull_request", json!({"task": "KADE-1"})),
        ]
    }

    #[tokio::test]
    async fn the_desktop_token_names_the_server_hersir_lists_exactly_the_read_only_tools_and_reads_the_board() {
        let s = served();
        let token = token_of(&s.t.connect(false));
        let mut c = Conn::open(&s.t.st, &token).await;
        let init = c.initialize().await;
        assert_eq!(init["result"]["serverInfo"]["name"], "hersir", "{init}");
        let list = c.ask(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})).await.unwrap();
        let mut listed: Vec<String> = list["result"]["tools"].as_array().unwrap().iter().map(|d| d["name"].as_str().unwrap().to_string()).collect();
        let mut read_only: Vec<String> = tools::catalog().into_iter().filter(|d| d.read_only).map(|d| d.name).collect();
        // GA-344: and board_changes, Claude Desktop's own (not in the catalog the other tokens get)
        read_only.push(tools::BOARD_CHANGES.to_string());
        listed.sort();
        read_only.sort();
        assert_eq!(listed, read_only, "exactly the tools ToolDef::read_only marks, and board_changes");
        assert!(list["result"]["tools"].as_array().unwrap().iter().all(|d| d["annotations"]["readOnlyHint"] == true));
        for name in ["get_overview", "list_tasks", "memory_read", "memory_list", "memory_search", "get_task", "check_board"] {
            assert!(read_only.contains(&name.to_string()), "{name} reads");
        }
        // get_overview, list_tasks and memory_read work
        let overview = c.call(3, "get_overview", json!({})).await;
        assert_eq!(overview["isError"], false, "{overview}");
        let o: Value = serde_json::from_str(&text(&overview)).unwrap();
        assert_eq!(o["agents"][0]["name"], "Team Lead", "{o}");
        let listed = c.call(4, "list_tasks", json!({"project": "KADE"})).await;
        assert_eq!(listed["isError"], false, "{listed}");
        assert!(text(&listed).contains("Export invoices as CSV") && text(&listed).contains("KADE-1"), "{listed}");
        let note = c.call(5, "memory_read", json!({"note": "Decisions/Exports"})).await;
        assert_eq!(note["isError"], false, "{note}");
        let n: Value = serde_json::from_str(&text(&note)).unwrap();
        assert_eq!(n["note"]["body_md"], "Exports are CSV, for the accountant.");
        let task = c.call(6, "get_task", json!({"task": "KADE-1"})).await;
        assert_eq!(task["isError"], false, "{task}");
        // GA-344: board_changes over the socket: without since only the counter; with it, nothing changed since
        let first = c.call(7, "board_changes", json!({})).await;
        assert_eq!(first["isError"], false, "{first}");
        let f: Value = serde_json::from_str(&text(&first)).unwrap();
        assert!(f["counter"].as_i64().unwrap() > 0, "{f}");
        assert_eq!((f["tasks"].clone(), f["board"].clone(), f["reload"].clone()), (json!([]), json!(false), json!(false)), "{f}");
        let again: Value = serde_json::from_str(&text(&c.call(8, "board_changes", json!({"since": f["counter"]})).await)).unwrap();
        assert_eq!(again, f, "with the latest counter: the same counter and no cards");
        let _ = &s.lead;
    }

    #[tokio::test]
    async fn the_desktop_token_is_refused_every_tool_that_writes_with_the_cards_text_and_nothing_changes() {
        let s = served();
        let token = token_of(&s.t.connect(false));
        let mut c = Conn::open(&s.t.st, &token).await;
        c.initialize().await;
        // a read first, so whatever a first call does is done
        assert_eq!(c.call(2, "get_overview", json!({})).await["isError"], false);
        let before = dump(&s.t.st);
        let counted = counts(&s.t.st);
        assert!(counted.iter().any(|(t, n)| t == "changes" && *n > 0), "the setup left activity lines: {counted:?}");
        let mut id = 10;
        for (name, args) in writes() {
            id += 1;
            let r = c.call(id, name, args).await;
            assert_eq!(r["isError"], true, "{name}: {r}");
            assert_eq!(text(&r), not_in_desktop(name), "{name}");
        }
        // every other tool that isn't read-only too, with or without arguments
        for d in tools::catalog().into_iter().filter(|d| !d.read_only) {
            id += 1;
            let r = c.call(id, &d.name, json!({})).await;
            assert_eq!((r["isError"].clone(), text(&r)), (json!(true), not_in_desktop(&d.name)), "{}", d.name);
        }
        assert_eq!(counts(&s.t.st), counted, "no row, no activity line, no pending action");
        assert_eq!(dump(&s.t.st), before, "nothing in the database changed");
        // the connection still answers reads afterwards
        assert_eq!(c.call(id + 1, "list_tasks", json!({})).await["isError"], false);
    }

    #[tokio::test]
    async fn a_token_whose_scope_is_empty_or_names_no_chat_check_or_question_changes_nothing_with_any_tool_that_isnt_read_only() {
        let s = served();
        let you = s.t.st.you_id.clone();
        for (who, scope) in [(you.clone(), json!({})), (s.lead.clone(), json!({})), (you.clone(), json!({"run": "R1"})),
                             (you.clone(), json!({"client": "something-else"}))] {
            let token = tokens::mint(&s.t.st.db, &who, scope.clone(), 60_000).unwrap();
            let mut c = Conn::open(&s.t.st, &token).await;
            c.initialize().await;
            assert_eq!(c.call(2, "get_overview", json!({})).await["isError"], false, "{scope}: reads work");
            let before = dump(&s.t.st);
            let mut id = 10;
            let mut all: Vec<(String, Value)> = writes().into_iter().map(|(n, a)| (n.to_string(), a)).collect();
            all.extend(tools::catalog().into_iter().filter(|d| !d.read_only).map(|d| (d.name, json!({}))));
            for (name, args) in all {
                id += 1;
                let r = c.call(id, &name, args).await;
                assert_eq!(r["isError"], true, "{scope} {name}: {r}");
                assert!(text(&r).starts_with(&format!("{name} isn't available")), "{scope} {name}: {}", text(&r));
                assert_eq!(dump(&s.t.st), before, "{scope} {name} changed something");
            }
        }
        // the control: the same arguments do work in the calls Hersir makes itself (unchanged), and the check sees it
        let before = dump(&s.t.st);
        let made = tools::call(&s.t.st, &s.lead, "create_task", json!({"project": "KADE", "title": "Made from Claude Desktop"})).await;
        assert!(made.is_ok(), "{made:?}");
        assert!(tools::call(&s.t.st, &s.lead, "comment_on_task", json!({"task": "KADE-1", "body_md": "A comment"})).await.is_ok());
        assert_ne!(dump(&s.t.st), before);
    }

    #[tokio::test]
    async fn after_a_new_connect_a_shim_with_the_old_token_is_refused_and_one_with_the_new_token_is_served() {
        let s = served();
        let old = token_of(&s.t.connect(false));
        let new = token_of(&s.t.connect(false));
        let mut refused = Conn::open(&s.t.st, &old).await;
        let r = refused.ask(json!({"jsonrpc": "2.0", "id": 1, "method": "ping"})).await.unwrap();
        assert!(r["error"].as_str().is_some_and(|e| e.contains("not valid")), "{r}");
        assert!(refused.next().await.is_none(), "and the connection ends");
        let mut served = Conn::open(&s.t.st, &new).await;
        assert_eq!(served.initialize().await["result"]["serverInfo"]["name"], "hersir");
        assert_eq!(served.call(2, "get_overview", json!({})).await["isError"], false);
    }

    #[tokio::test]
    async fn disconnect_refuses_a_new_shim_and_ends_an_open_connection_at_its_next_message() {
        let s = served();
        let token = token_of(&s.t.connect(false));
        let mut open = Conn::open(&s.t.st, &token).await;
        open.initialize().await;
        assert_eq!(open.call(2, "get_overview", json!({})).await["isError"], false);
        let st = desktop::disconnect(&s.t.st).unwrap();
        assert!(!st.connected && st.connected_at.is_none());
        let r = open.ask(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "get_overview", "arguments": {}}})).await.unwrap();
        assert!(r.get("result").is_none() && r["error"].as_str().is_some_and(|e| e.contains("not valid")), "{r}");
        assert!(open.next().await.is_none(), "the open connection ends");
        let mut again = Conn::open(&s.t.st, &token).await;
        let r = again.ask(json!({"jsonrpc": "2.0", "id": 1, "method": "ping"})).await.unwrap();
        assert!(r["error"].as_str().is_some_and(|e| e.contains("not valid")), "{r}");
    }

    #[tokio::test]
    async fn a_new_connect_ends_the_connection_of_the_earlier_token() {
        let s = served();
        let old = token_of(&s.t.connect(false));
        let mut open = Conn::open(&s.t.st, &old).await;
        open.initialize().await;
        s.t.connect(false);
        let r = open.ask(json!({"jsonrpc": "2.0", "id": 2, "method": "ping"})).await.unwrap();
        assert!(r["error"].as_str().is_some_and(|e| e.contains("not valid")), "{r}");
        assert!(open.next().await.is_none());
    }

    /// The real hersir-mcp: target/debug/hersir-mcp, which the crate gizai-mcp builds (built here when missing).
    fn real_shim() -> PathBuf {
        static BUILT: std::sync::Once = std::sync::Once::new();
        let path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../target/debug/hersir-mcp"));
        BUILT.call_once(|| {
            if path.is_file() {
                return;
            }
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
            let ok = std::process::Command::new(cargo).args(["build", "-q", "-p", "gizai-mcp"])
                .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/..")).status().map(|s| s.success()).unwrap_or(false);
            assert!(ok && path.is_file(), "could not build hersir-mcp (the crate gizai-mcp)");
        });
        path
    }

    /// Starts the entry's server as Claude Desktop would, with only the entry's environment, sends `requests` and closes
    /// its input: the lines it wrote.
    fn run_entry(c: &ClaudeDesktopConnection, requests: &[Value]) -> Vec<Value> {
        use std::io::{BufRead, Write};
        let s = server_of(c);
        let mut cmd = std::process::Command::new(s["command"].as_str().unwrap());
        cmd.args(s["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap())).env_clear();
        for (k, v) in s["env"].as_object().unwrap() {
            cmd.env(k, v.as_str().unwrap());
        }
        let mut child = cmd.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null())
            .spawn().unwrap();
        let mut stdin = child.stdin.take().unwrap();
        for q in requests {
            stdin.write_all(format!("{q}\n").as_bytes()).unwrap();
        }
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for l in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(l).is_err() {
                    break;
                }
            }
        });
        let mut out = vec![];
        let want = requests.iter().filter(|q| q.get("id").is_some()).count();
        while out.len() < want {
            match rx.recv_timeout(WAIT) {
                Ok(l) => out.push(serde_json::from_str(&l).unwrap_or(Value::String(l))),
                Err(_) => break,
            }
        }
        drop(stdin);
        let _ = child.kill();
        let _ = child.wait();
        out
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_real_hersir_mcp_from_the_entry_with_only_its_environment_reads_the_board_and_is_refused_a_write() {
        let mut s = served();
        s.t.st.mcp_shim = Some(real_shim());
        let c = s.t.connect(false);
        let requests = [
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "list_tasks", "arguments": {}}}),
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "create_task", "arguments": {"project": "KADE", "title": "x"}}}),
        ];
        let tasks_before = counts(&s.t.st);
        let out = tokio::task::spawn_blocking({
            let (c, requests) = (c.clone(), requests.clone());
            move || run_entry(&c, &requests)
        }).await.unwrap();
        assert_eq!(out.len(), 4, "{out:?}");
        assert_eq!(out[0]["result"]["serverInfo"]["name"], "hersir");
        // the tools that read, and board_changes (GA-344)
        let listed = out[1]["result"]["tools"].as_array().unwrap();
        assert_eq!(listed.len(), tools::catalog().iter().filter(|d| d.read_only).count() + 1);
        assert!(listed.iter().any(|d| d["name"] == tools::BOARD_CHANGES), "{listed:?}");
        assert_eq!(out[2]["result"]["isError"], false, "{}", out[2]);
        assert!(text(&out[2]["result"]).contains("KADE-1"));
        assert_eq!(out[3]["result"]["isError"], true);
        assert_eq!(text(&out[3]["result"]), not_in_desktop("create_task"));
        assert_eq!(counts(&s.t.st), tasks_before);
        // after Disconnect the same entry reads nothing from Hersir any more (whatever the shim then tells Claude)
        desktop::disconnect(&s.t.st).unwrap();
        let out = tokio::task::spawn_blocking(move || run_entry(&c, &requests[..4])).await.unwrap();
        let all = format!("{out:?}");
        assert!(!all.contains("KADE-1") && !all.contains("Export invoices as CSV"), "{all}");
        assert!(out.iter().all(|l| l["result"]["isError"] != false || l["id"] != 3), "{all}");
    }
}
