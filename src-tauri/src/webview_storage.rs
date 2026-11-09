//! The page's own storage (GA-248): what the window keeps in localStorage, such as the theme, fonts, sizes and views you
//! chose. The webview keeps it in a folder named after the app id: `<local data>/<id>` on Linux and Windows (Tauri's
//! app_local_data_dir: `$XDG_DATA_HOME` or `~/.local/share`, and `%LOCALAPPDATA%`), and `~/Library/WebKit/<id>` on
//! macOS. The id was ai.gizai.app before Hersir 0.17.0 and is dev.hersir.app now, so on the first start with the new id
//! the old folder becomes the new one, and nothing you chose there resets. Only the Hersir on your usual data does this
//! (a dev build or a test on other data never moves it), only while no other Hersir holds that data (one from before
//! the update that still runs), and never over a folder with the new id that is there already.
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The app id before Hersir 0.17.0.
pub const OLD_APP_ID: &str = "ai.gizai.app";

/// The folder the webview keeps the page's storage in for the app id `id`. None when the system doesn't say where.
pub fn folder(id: &str) -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    #[cfg(target_os = "macos")]
    return var("HOME").map(|home| home.join("Library/WebKit").join(id));
    #[cfg(windows)]
    return var("LOCALAPPDATA").map(|local| local.join(id));
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let base = var("XDG_DATA_HOME").filter(|d| d.is_absolute()).or_else(|| var("HOME").map(|home| home.join(".local/share")))?;
        Some(base.join(id))
    }
}

/// Renames the folder `from` to `to` when there is a folder `from` and nothing at `to` yet. Ok(true): renamed. On
/// Windows the webview of a Hersir that has just quit may hold the folder a moment longer, so it is tried again for
/// three seconds.
pub fn carry(from: &Path, to: &Path) -> std::io::Result<bool> {
    if !from.is_dir() || to.symlink_metadata().is_ok() {
        return Ok(false);
    }
    let mut tries = 0;
    loop {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(true),
            Err(_) if cfg!(windows) && tries < 10 => {
                tries += 1;
                std::thread::sleep(Duration::from_millis(300));
            }
            Err(e) => return Err(e),
        }
    }
}

/// When Hersir starts on the data in `data`, before its window opens: the page's storage moves from the old app id's
/// folder to the new one's, when `data` is your usual data and no other Hersir holds it.
pub fn carry_at_start(data: &Path) {
    if crate::data_label(data).is_some() {
        return;
    }
    let (Some(from), Some(to)) = (folder(OLD_APP_ID), folder(crate::APP_ID)) else { return };
    if !from.is_dir() || to.symlink_metadata().is_ok() {
        return;
    }
    // A Hersir from before the update that still runs on this data keeps its storage where it is.
    let Ok(lock) = crate::lock_data_dir(data) else { return };
    match carry(&from, &to) {
        Ok(true) => eprintln!("gizai: moved the window's storage (your settings in it) from {} to {}", from.display(), to.display()),
        Ok(false) => {}
        Err(e) => eprintln!("gizai: can't move the window's storage from {} to {}: {e}", from.display(), to.display()),
    }
    drop(lock);
}
