// GA-248: the app id is dev.hersir.app (ai.gizai.app before 0.17.0) wherever it is used, and the program is hersir.
// On the first start with the new id, the window's storage (the theme, fonts and views you chose) comes along from the
// old id's folder, without ever replacing a folder of the new id.
use std::path::Path;

use gizai_lib::webview_storage::{self, OLD_APP_ID};

fn conf() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn the_app_id_and_the_program_are_hersirs() {
    assert_eq!(gizai_lib::APP_ID, "dev.hersir.app");
    assert_eq!(OLD_APP_ID, "ai.gizai.app");
    let conf = conf();
    assert_eq!(conf["identifier"], gizai_lib::APP_ID, "tauri.conf.json: macOS's bundle id");
    assert_eq!(conf["mainBinaryName"], "hersir");
    assert_eq!(conf["productName"], "Hersir");
    // Linux: one Hersir per data folder, by a D-Bus name that starts with the app id
    let dir = tempfile::tempdir().unwrap();
    let id = gizai_lib::instance_id(dir.path());
    assert!(id.starts_with("dev.hersir.app.d"), "{id}");
}

#[test]
fn the_windows_storage_comes_along_from_the_old_app_ids_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let (from, to) = (tmp.path().join(OLD_APP_ID), tmp.path().join(gizai_lib::APP_ID));
    assert!(!webview_storage::carry(&from, &to).unwrap(), "nothing to carry");
    assert!(!to.exists());

    std::fs::create_dir_all(from.join("localstorage")).unwrap();
    std::fs::write(from.join("localstorage/theme"), "light").unwrap();
    assert!(webview_storage::carry(&from, &to).unwrap());
    assert!(!from.exists(), "the old folder became the new one");
    assert_eq!(std::fs::read_to_string(to.join("localstorage/theme")).unwrap(), "light");

    // a folder with the new id that is there already is never replaced
    std::fs::create_dir_all(from.join("localstorage")).unwrap();
    std::fs::write(from.join("localstorage/theme"), "dark").unwrap();
    assert!(!webview_storage::carry(&from, &to).unwrap());
    assert_eq!(std::fs::read_to_string(to.join("localstorage/theme")).unwrap(), "light");
    assert_eq!(std::fs::read_to_string(from.join("localstorage/theme")).unwrap(), "dark", "the old one stays as it was");
}

#[test]
fn the_folder_is_named_after_the_app_id() {
    let (Some(old), Some(new)) = (webview_storage::folder(OLD_APP_ID), webview_storage::folder(gizai_lib::APP_ID)) else {
        return; // the system doesn't say where (no HOME)
    };
    assert_eq!(old.file_name().unwrap(), OLD_APP_ID);
    assert_eq!(new.file_name().unwrap(), "dev.hersir.app");
    assert_eq!(old.parent(), new.parent(), "side by side");
}
