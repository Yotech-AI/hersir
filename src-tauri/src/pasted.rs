//! An image pasted into the Chief Chat's text box (GA-304). WebKitGTK may not hand a pasted image to the page, so Hersir
//! reads the clipboard's image itself, through the clipboard plugin's Rust API (the window gets no new right), and saves
//! it as a PNG in a folder of its own under `<data dir>/pasted/`. The message then adds that file like a picked one, and
//! sending copies it into the file store (`chat::send_with_files`), so the folder is only needed until then: Hersir
//! removes those older than a day when it starts (`clear_old`).
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::AppState;

/// The folder in the data folder that holds the pasted images, each in a folder of its own.
pub const FOLDER: &str = "pasted";
/// How long a pasted image's folder is kept: it is needed until its message is sent.
pub const KEEP_MS: i64 = 24 * 60 * 60 * 1000;
/// How every error starts: the composer shows it in its warning banner.
pub const FAILED: &str = "The clipboard's image couldn't be pasted: ";

/// What the clipboard plugin passes on (as text) when the clipboard holds no image: arboard's `ContentNotAvailable`,
/// for a clipboard with only text on it, or with nothing.
const NO_IMAGE: &str = "The clipboard contents were not available in the requested format or the clipboard is empty.";

/// `why` as the end of one sentence starting with `FAILED`: a first word that isn't a name in lower case, one full stop.
pub fn failed(why: impl std::fmt::Display) -> String {
    let why = why.to_string();
    let why = why.trim().trim_end_matches('.').trim_end();
    let mut chars = why.chars();
    let why = match (chars.next(), chars.next()) {
        (None, _) => "no reason was given".to_string(),
        // "The native clipboard …" reads "the native clipboard …"; "X11 …" and "I/O …" stay as they are
        (Some(first), Some(second)) if first.is_uppercase() && second.is_lowercase() => first.to_lowercase().chain(why[first.len_utf8()..].chars()).collect(),
        _ => why.to_string(),
    };
    format!("{FAILED}{why}.")
}

/// `pasted-image-2026-11-17_14-35-02.png` for a paste at `ms` (Unix milliseconds), on this computer's clock.
pub fn file_name(ms: i64) -> String {
    // "20261117-143502-123"
    let s = gizai_core::db::stamp(ms);
    format!("pasted-image-{}-{}-{}_{}-{}-{}.png", &s[0..4], &s[4..6], &s[6..8], &s[9..11], &s[11..13], &s[13..15])
}

/// Saves an image (RGBA, 4 bytes a pixel, row after row from the top) as a PNG named for `ms` (`file_name`), in a new
/// folder of its own (a random id) under `data_dir/pasted/`, and returns its path. An error is one sentence (`failed`);
/// nothing is left behind then.
pub fn save_png(data_dir: &Path, rgba: &[u8], width: u32, height: u32, ms: i64) -> Result<PathBuf, String> {
    if width == 0 || height == 0 {
        return Err(failed("it has no pixels"));
    }
    if rgba.len() as u64 != u64::from(width) * u64::from(height) * 4 {
        return Err(failed(format!("its {} bytes don't make an image of {width} by {height} pixels", rgba.len())));
    }
    let mut png = Vec::new();
    PngEncoder::new(&mut png).write_image(rgba, width, height, ExtendedColorType::Rgba8)
        .map_err(|e| failed(format!("it couldn't be made a PNG ({e})")))?;
    let dir = data_dir.join(FOLDER).join(gizai_core::ids::new_id());
    std::fs::create_dir_all(&dir).map_err(|e| failed(format!("the folder {} couldn't be made ({e})", dir.display())))?;
    let path = dir.join(file_name(ms));
    if let Err(e) = std::fs::write(&path, &png) {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(failed(format!("{} couldn't be saved ({e})", path.display())));
    }
    Ok(path)
}

/// Removes the folders in `data_dir/pasted/` last changed more than `KEEP_MS` before `now_ms`, with what is in them.
/// Newer folders stay, and so do files, links and a folder whose time can't be read. Returns how many went; none when
/// there is no such folder.
pub fn clear_old(data_dir: &Path, now_ms: i64) -> usize {
    let Ok(entries) = std::fs::read_dir(data_dir.join(FOLDER)) else { return 0 };
    let before = UNIX_EPOCH + Duration::from_millis((now_ms - KEEP_MS).max(0) as u64);
    let mut n = 0;
    for e in entries.flatten() {
        // Neither file_type nor metadata follows a link.
        let old = e.file_type().is_ok_and(|t| t.is_dir()) && e.metadata().and_then(|m| m.modified()).is_ok_and(|t| t < before);
        if old && std::fs::remove_dir_all(e.path()).is_ok() {
            n += 1;
        }
    }
    n
}

/// The clipboard's image, saved as a PNG under `<data dir>/pasted/` (`save_png`): its path, or None when the clipboard
/// holds no image (only text, or nothing). Read off the main thread: on Linux the clipboard libraries can deadlock there.
#[tauri::command]
pub async fn paste_clipboard_image(app: AppHandle, st: State<'_, AppState>) -> Result<Option<String>, String> {
    let dir = st.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let image = match app.clipboard().read_image() {
            Ok(image) => image,
            Err(e) if e.to_string() == NO_IMAGE => return Ok(None),
            Err(e) => return Err(failed(e)),
        };
        save_png(&dir, image.rgba(), image.width(), image.height(), gizai_core::ids::now_ms()).map(|p| Some(p.display().to_string()))
    })
    .await
    .map_err(failed)?
}
