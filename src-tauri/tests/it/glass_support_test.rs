// GA-292 QA: which blur the system gives the see-through window (src/glass.rs), the answer the UI's glass follows
// (src/lib/glass.ts): vibrancy on macOS, Mica on Windows 11, Hyprland's compositor on Linux when HYPRLAND_INSTANCE_SIGNATURE
// is set, and none elsewhere. Reads the environment only, never changes it, so it can run beside the other tests.
use std::ffi::OsStr;
use std::path::Path;

use gizai_lib::glass::{glass_support, linux_support, support};

#[test]
fn linux_is_hyprland_only_with_its_signature() {
    assert_eq!(linux_support(None), "none");
    assert_eq!(linux_support(Some(OsStr::new(""))), "none");
    assert_eq!(linux_support(Some(OsStr::new("4520b30d498daca8079365bdb909a8dea38e8d55_1731772800_1234567890"))), "compositor");
}

/// GA-299: the command is async (it runs on the async runtime, not on the main thread) and answers what `support()` does.
#[tokio::test]
async fn the_command_answers_one_of_the_four_the_ui_knows() {
    assert_eq!(glass_support().await, support());
    assert!(["vibrancy", "mica", "compositor", "none"].contains(&support()), "{}", support());
}

/// GA-299: what the window gets is the answer as a JSON string, the text src/lib/glass.ts reads, as it was when the
/// command wasn't async.
#[tokio::test]
async fn the_window_gets_the_same_text_as_before() {
    let answer = glass_support().await;
    assert_eq!(serde_json::to_value(answer).unwrap(), serde_json::Value::String(support().to_string()));
    assert_eq!(serde_json::to_string(answer).unwrap(), format!("\"{}\"", support()));
}

/// GA-299: glass_support is written as an async Tauri command, and the window can still call it.
#[test]
fn glass_support_is_an_async_command_in_the_handler() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let rs = std::fs::read_to_string(dir.join("glass.rs")).unwrap();
    let at = rs.find("#[tauri::command]").expect("glass.rs has a command");
    let fn_at = at + rs[at..].find("fn ").unwrap();
    assert!(rs[fn_at..].starts_with("fn glass_support()"), "{}", &rs[at..fn_at + 30]);
    assert!(rs[at..fn_at].contains("async"), "glass_support is async: {}", &rs[at..fn_at + 30]);
    let lib = std::fs::read_to_string(dir.join("lib.rs")).unwrap();
    let handler = &lib[lib.find("generate_handler![").expect("lib.rs has generate_handler")..];
    assert!(handler[..handler.find(']').unwrap()].contains("glass::glass_support"));
}

#[cfg(target_os = "linux")]
#[test]
fn on_linux_it_follows_hyprlands_signature_in_the_environment() {
    assert_eq!(support(), linux_support(std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").as_deref()));
}

#[cfg(target_os = "macos")]
#[test]
fn on_macos_it_is_vibrancy() {
    assert_eq!(support(), "vibrancy");
}

#[cfg(windows)]
#[test]
fn on_windows_it_is_mica_from_windows_11() {
    let build = windows_version::OsVersion::current().build;
    assert_eq!(support(), if build >= 22000 { "mica" } else { "none" });
}

/// The window is see-through and asks for the effects when it is made; macOS needs the private API for it, and the UI may
/// change the effects afterwards.
#[test]
fn the_main_window_is_transparent_and_asks_for_the_effects() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let conf: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("tauri.conf.json")).unwrap()).unwrap();
    let main = &conf["app"]["windows"][0];
    assert_eq!(main["label"], "main");
    assert_eq!(main["transparent"], true);
    assert_eq!(main["windowEffects"]["effects"], serde_json::json!(["underWindowBackground", "mica"]));
    assert_eq!(main["windowEffects"]["state"], "followsWindowActiveState");
    assert_eq!(conf["app"]["macOSPrivateApi"], true);
    let cap: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("capabilities").join("default.json")).unwrap()).unwrap();
    assert!(cap["permissions"].as_array().unwrap().iter().any(|p| p == "core:window:allow-set-effects"));
}
