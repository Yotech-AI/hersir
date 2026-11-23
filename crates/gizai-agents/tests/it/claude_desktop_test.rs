// GA-338 QA: Claude Desktop's settings file (`gizai_agents::claude_desktop`): where its folder is on each system, the
// entry Hersir gives, and Add to Claude Desktop: a copy of the old bytes first, then the whole file written anew with
// only `mcpServers.hersir` changed, 0600 on Linux and macOS; a file that isn't JSON with an `mcpServers` object is left
// as it is, with no copy. Every test works in a temporary folder, never in the real Claude Desktop's.
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use gizai_agents::claude_desktop::{self as desktop, NotAdded, Server};
use serde_json::{Value, json};

const STAMP: &str = "20261123-142501";

fn server() -> Server {
    Server::new(Path::new("/opt/hersir/hersir-mcp"), Path::new("/run/user/1000/gizai/abc.sock"), &"ab".repeat(32))
}

fn file_in(dir: &Path) -> PathBuf {
    dir.join(desktop::FILE)
}

/// The names in `dir`, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

fn read_json(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}

#[cfg(unix)]
fn mode(p: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

fn vars(list: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
    let list: Vec<(String, String)> = list.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    move |k| list.iter().find(|(n, _)| n == k).map(|(_, v)| OsString::from(v))
}

// ---- where the folder is ----

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn on_linux_the_folder_is_xdg_config_home_claude_else_dot_config_claude() {
    assert_eq!(desktop::folder_from(vars(&[("XDG_CONFIG_HOME", "/x/conf"), ("HOME", "/home/j")])), Some(PathBuf::from("/x/conf/Claude")));
    assert_eq!(desktop::folder_from(vars(&[("HOME", "/home/j")])), Some(PathBuf::from("/home/j/.config/Claude")));
    // an empty or relative XDG_CONFIG_HOME counts as not set
    assert_eq!(desktop::folder_from(vars(&[("XDG_CONFIG_HOME", ""), ("HOME", "/home/j")])), Some(PathBuf::from("/home/j/.config/Claude")));
    assert_eq!(desktop::folder_from(vars(&[("XDG_CONFIG_HOME", "conf"), ("HOME", "/home/j")])), Some(PathBuf::from("/home/j/.config/Claude")));
    assert_eq!(desktop::folder_from(vars(&[])), None);
}

#[cfg(target_os = "macos")]
#[test]
fn on_macos_the_folder_is_in_application_support() {
    assert_eq!(desktop::folder_from(vars(&[("HOME", "/Users/j"), ("XDG_CONFIG_HOME", "/x/conf")])),
               Some(PathBuf::from("/Users/j/Library/Application Support/Claude")));
    assert_eq!(desktop::folder_from(vars(&[])), None);
}

#[cfg(windows)]
#[test]
fn on_windows_the_folder_is_appdata_claude() {
    assert_eq!(desktop::folder_from(vars(&[("APPDATA", r"C:\Users\j\AppData\Roaming")])), Some(PathBuf::from(r"C:\Users\j\AppData\Roaming\Claude")));
    assert_eq!(desktop::folder_from(vars(&[("HOME", r"C:\Users\j")])), None);
}

#[test]
fn the_settings_file_is_only_given_when_claude_desktops_folder_is_there() {
    let tmp = tempfile::tempdir().unwrap();
    let claude = tmp.path().join("Claude");
    assert_eq!(desktop::config_file(None), None);
    assert_eq!(desktop::config_file(Some(&claude)), None, "not installed: no folder");
    std::fs::create_dir(&claude).unwrap();
    assert_eq!(desktop::config_file(Some(&claude)), Some(claude.join("claude_desktop_config.json")));
    assert!(!claude.join("claude_desktop_config.json").exists(), "asking makes nothing");
}

// ---- the entry ----

#[test]
fn the_entry_is_the_shim_in_desktop_mode_with_the_socket_and_the_token_pretty_printed_with_two_spaces() {
    let token = "ab".repeat(32);
    let entry = desktop::entry(&server());
    assert_eq!(entry, format!(r#"{{
  "mcpServers": {{
    "hersir": {{
      "command": "/opt/hersir/hersir-mcp",
      "args": [
        "--desktop"
      ],
      "env": {{
        "HERSIR_SOCKET": "/run/user/1000/gizai/abc.sock",
        "HERSIR_TOKEN": "{token}"
      }}
    }}
  }}
}}"#));
    let v: Value = serde_json::from_str(&entry).unwrap();
    assert_eq!(v, json!({"mcpServers": {"hersir": {"command": "/opt/hersir/hersir-mcp", "args": ["--desktop"],
                                                   "env": {"HERSIR_SOCKET": "/run/user/1000/gizai/abc.sock", "HERSIR_TOKEN": token}}}}));
    assert!(!format!("{:?}", server()).contains(&token), "Debug leaves the token out");
}

// ---- Add to Claude Desktop ----

const OLD: &str = r#"{
    "globalShortcut": "Ctrl+Space",
    "mcpServers": {
        "filesystem": {"command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem", "/home/j/Documents"]},
        "hersir": {"command": "/old/hersir-mcp", "args": [], "env": {"HERSIR_TOKEN": "old"}},
        "weather": {
            "command": "uvx",
            "env": {"KEY": "1e400", "Ü": "\u00fc"}
        }
    },
    "preferences": {"menuBarEnabled": false, "n": 1.50}
}"#;

#[test]
fn an_existing_file_is_copied_byte_for_byte_and_only_mcp_servers_hersir_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let path = file_in(tmp.path());
    std::fs::write(&path, OLD).unwrap();
    let backup = desktop::add_server(&path, &server(), STAMP).unwrap().expect("a copy");
    assert_eq!(backup, tmp.path().join(format!("claude_desktop_config.json.hersir-backup-{STAMP}")));
    assert_eq!(std::fs::read(&backup).unwrap(), OLD.as_bytes(), "the copy holds exactly the old bytes");
    assert_eq!(names(tmp.path()), [desktop::FILE.to_string(), format!("claude_desktop_config.json.hersir-backup-{STAMP}")],
               "no temp file is left");

    let new = std::fs::read_to_string(&path).unwrap();
    let v: Value = serde_json::from_str(&new).unwrap();
    let old: Value = serde_json::from_str(OLD).unwrap();
    // the other keys and servers are as they were
    assert_eq!(v["globalShortcut"], old["globalShortcut"]);
    assert_eq!(v["preferences"], old["preferences"]);
    assert_eq!(v["mcpServers"]["filesystem"], old["mcpServers"]["filesystem"]);
    assert_eq!(v["mcpServers"]["weather"], old["mcpServers"]["weather"]);
    // their text too, numbers and escapes included
    for kept in [r#""Ctrl+Space""#, r#"{"command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem", "/home/j/Documents"]}"#,
                 r#"{"KEY": "1e400", "Ü": "\u00fc"}"#, r#"{"menuBarEnabled": false, "n": 1.50}"#] {
        assert!(new.contains(kept), "{kept} is kept as it was in:\n{new}");
    }
    // hersir is replaced, in its place, as in the entry
    let entry: Value = serde_json::from_str(&desktop::entry(&server())).unwrap();
    assert_eq!(v["mcpServers"]["hersir"], entry["mcpServers"]["hersir"]);
    let order = |text: &str, keys: &[&str]| keys.iter().map(|k| text.find(&format!("\"{k}\"")).unwrap()).collect::<Vec<_>>();
    let at = order(&new, &["globalShortcut", "mcpServers", "filesystem", "hersir", "weather", "preferences"]);
    assert!(at.windows(2).all(|w| w[0] < w[1]), "the members keep their order: {at:?}\n{new}");
    assert_eq!(new.matches("\"hersir\"").count(), 1);
    assert!(!new.contains("/old/hersir-mcp"));
    assert_eq!(v.as_object().unwrap().len(), 3);
    assert_eq!(v["mcpServers"].as_object().unwrap().len(), 3);
    assert!(desktop::has_hersir(&path));
    #[cfg(unix)]
    {
        assert_eq!(mode(&path), 0o600, "the new file");
        assert_eq!(mode(&backup), 0o600, "the copy, which holds the old token");
    }
}

#[cfg(unix)]
#[test]
fn a_file_others_could_read_is_0600_after_it_is_written() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let path = file_in(tmp.path());
    std::fs::write(&path, r#"{"mcpServers": {}}"#).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    desktop::add_server(&path, &server(), STAMP).unwrap();
    assert_eq!(mode(&path), 0o600);
}

#[test]
fn a_file_without_mcp_servers_gets_it_last_and_keeps_its_other_keys() {
    let tmp = tempfile::tempdir().unwrap();
    let path = file_in(tmp.path());
    std::fs::write(&path, "{\"globalShortcut\": \"Alt+C\", \"x\": [1, 2]}\n").unwrap();
    assert!(!desktop::has_hersir(&path));
    assert!(desktop::add_server(&path, &server(), STAMP).unwrap().is_some());
    let v = read_json(&path);
    assert_eq!(v["globalShortcut"], "Alt+C");
    assert_eq!(v["x"], json!([1, 2]));
    assert_eq!(v["mcpServers"].as_object().unwrap().keys().collect::<Vec<_>>(), ["hersir"]);
    assert!(std::fs::read_to_string(&path).unwrap().ends_with("}\n"), "a file that ended with a new line still does");
}

#[test]
fn without_a_file_it_is_made_with_only_hersirs_server_and_there_is_no_copy() {
    let tmp = tempfile::tempdir().unwrap();
    let path = file_in(tmp.path());
    assert_eq!(desktop::add_server(&path, &server(), STAMP).unwrap(), None);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), format!("{}\n", desktop::entry(&server())));
    assert_eq!(read_json(&path).as_object().unwrap().keys().collect::<Vec<_>>(), ["mcpServers"]);
    assert_eq!(read_json(&path)["mcpServers"].as_object().unwrap().keys().collect::<Vec<_>>(), ["hersir"]);
    assert_eq!(names(tmp.path()), [desktop::FILE]);
    #[cfg(unix)]
    assert_eq!(mode(&path), 0o600);
}

#[test]
fn a_file_that_isnt_json_with_an_mcp_servers_object_is_left_as_it_is_with_no_copy() {
    for bad in ["{ \"mcpServers\": { \"a\": 1 ", "not json", "", "[]", "\"text\"", "{\"mcpServers\": []}", "{\"mcpServers\": \"x\"}",
                "{\"mcpServers\": null}", "{\"mcpServers\": 3}", "{\"a\": 1} trailing"] {
        let tmp = tempfile::tempdir().unwrap();
        let path = file_in(tmp.path());
        std::fs::write(&path, bad).unwrap();
        let r = desktop::add_server(&path, &server(), STAMP);
        assert!(matches!(r, Err(NotAdded::NotJson)), "{bad:?}: {r:?}");
        assert_eq!(std::fs::read(&path).unwrap(), bad.as_bytes(), "{bad:?}: byte for byte unchanged");
        assert_eq!(names(tmp.path()), [desktop::FILE], "{bad:?}: no copy, no temp file");
        assert!(!desktop::has_hersir(&path));
    }
}

#[test]
fn a_second_copy_in_the_same_second_gets_its_own_name_and_never_overwrites_the_first() {
    let tmp = tempfile::tempdir().unwrap();
    let path = file_in(tmp.path());
    std::fs::write(&path, OLD).unwrap();
    let first = desktop::add_server(&path, &server(), STAMP).unwrap().unwrap();
    let written = std::fs::read(&path).unwrap();
    let second = desktop::add_server(&path, &server(), STAMP).unwrap().unwrap();
    assert_eq!(second, tmp.path().join(format!("claude_desktop_config.json.hersir-backup-{STAMP}-2")));
    assert_eq!(std::fs::read(&first).unwrap(), OLD.as_bytes());
    assert_eq!(std::fs::read(&second).unwrap(), written);
    assert_eq!(std::fs::read(&path).unwrap(), written, "the same entry again changes nothing more");
}

#[test]
fn has_hersir_needs_json_with_an_object_at_mcp_servers_hersir() {
    let tmp = tempfile::tempdir().unwrap();
    let path = file_in(tmp.path());
    assert!(!desktop::has_hersir(&path), "no file");
    for (text, has) in [(r#"{"mcpServers": {"hersir": {"command": "x"}}}"#, true), (r#"{"mcpServers": {"hersir": "x"}}"#, false),
                        (r#"{"mcpServers": {"other": {}}}"#, false), (r#"{"hersir": {}}"#, false), ("{not json", false)] {
        std::fs::write(&path, text).unwrap();
        assert_eq!(desktop::has_hersir(&path), has, "{text}");
    }
}

#[cfg(unix)]
#[test]
fn a_settings_file_that_is_a_link_stays_a_link_and_its_target_gets_the_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("dotfiles");
    std::fs::create_dir(&real).unwrap();
    std::fs::write(real.join(desktop::FILE), r#"{"mcpServers": {"a": {"command": "a"}}}"#).unwrap();
    let claude = tmp.path().join("Claude");
    std::fs::create_dir(&claude).unwrap();
    std::os::unix::fs::symlink(real.join(desktop::FILE), file_in(&claude)).unwrap();
    desktop::add_server(&file_in(&claude), &server(), STAMP).unwrap();
    assert!(std::fs::symlink_metadata(file_in(&claude)).unwrap().file_type().is_symlink());
    let v = read_json(&real.join(desktop::FILE));
    assert_eq!(v["mcpServers"]["a"]["command"], "a");
    assert!(v["mcpServers"]["hersir"].is_object());
}
