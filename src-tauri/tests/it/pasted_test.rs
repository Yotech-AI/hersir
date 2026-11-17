//! GA-304 QA: an image pasted into the Chief Chat's text box. `paste_clipboard_image` reads the clipboard through the
//! clipboard plugin, which needs a running app and a desktop: that part is checked in the cage (scripts/ui-test.sh). Here
//! are its parts that need neither: the file's name on the local clock (`file_name`), the PNG saved in a folder of its
//! own under `<data dir>/pasted/` (`save_png`), the one-sentence errors (`failed`), and the folders a day old removed
//! when Hersir starts (`clear_old`, run by `open_data`).
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use gizai_lib::pasted::{FAILED, FOLDER, KEEP_MS, clear_old, failed, file_name, save_png};

const HOUR_MS: i64 = 60 * 60 * 1000;

/// A `w` by `h` RGBA image whose every pixel says where it is, so a mix-up of rows, columns or channels shows.
fn pixels(w: u32, h: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            v.extend_from_slice(&[x as u8, y as u8, (x + y) as u8, 255 - (x as u8)]);
        }
    }
    v
}

/// The folders in `data/pasted/`, by name.
fn folders(data: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(data.join(FOLDER)).map(|r| r.flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// One sentence: starts with FAILED, ends with one full stop, no line breaks.
fn one_sentence(e: &str) {
    assert!(e.starts_with("The clipboard's image couldn't be pasted: "), "{e}");
    assert!(e.ends_with('.') && !e.ends_with(".."), "{e}");
    assert!(!e.contains('\n'), "{e}");
}

#[test]
fn the_name_is_pasted_image_with_the_date_and_time_on_the_local_clock() {
    let ms = gizai_core::ids::now_ms();
    let name = file_name(ms);
    // pasted-image-YYYY-MM-DD_HH-MM-SS.png
    let b = name.as_bytes();
    assert_eq!(name.len(), "pasted-image-2026-11-17_14-35-02.png".len(), "{name}");
    assert!(name.starts_with("pasted-image-") && name.ends_with(".png"), "{name}");
    for (i, c) in name["pasted-image-".len()..name.len() - 4].char_indices() {
        let want_digit = ![4, 7, 10, 13, 16].contains(&i);
        assert_eq!(c.is_ascii_digit(), want_digit, "{name} at {i}");
    }
    assert_eq!((b[17], b[20], b[23], b[26], b[29]), (b'-', b'-', b'_', b'-', b'-'), "{name}");
    // the same date and time as the rest of Hersir shows (gizai_core::db::stamp: local time)
    let s = gizai_core::db::stamp(ms);
    assert_eq!(name, format!("pasted-image-{}-{}-{}_{}-{}-{}.png", &s[0..4], &s[4..6], &s[6..8], &s[9..11], &s[11..13], &s[13..15]));
    // seconds count: a second later is another name
    assert_ne!(file_name(ms), file_name(ms + 1000));
}

#[cfg(unix)]
#[test]
fn the_time_in_the_name_is_the_local_time_that_date_prints() {
    // `date` runs in this process's time zone, as Hersir does
    let ms = gizai_core::ids::now_ms() / 1000 * 1000;
    let out = std::process::Command::new("date").arg(format!("-d@{}", ms / 1000)).arg("+%Y-%m-%d_%H-%M-%S").output();
    let Ok(out) = out else { return };
    if !out.status.success() {
        return; // BSD date (macOS) has no -d
    }
    let want = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert_eq!(file_name(ms), format!("pasted-image-{want}.png"));
}

#[test]
fn an_image_is_saved_as_a_png_of_the_same_size_and_pixels_in_a_folder_of_its_own_under_pasted() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let ms = gizai_core::ids::now_ms();
    let (w, h) = (37, 21);
    let rgba = pixels(w, h);
    let path = save_png(&data, &rgba, w, h, ms).unwrap();

    // <data dir>/pasted/<id>/pasted-image-<time>.png
    let folder = path.parent().unwrap();
    assert_eq!(folder.parent().unwrap(), data.join(FOLDER));
    assert_eq!(path.file_name().unwrap().to_string_lossy(), file_name(ms));
    assert_eq!(folders(&data).len(), 1);

    // a valid PNG, read back with the same width, height and pixels
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).unwrap().into_rgba8();
    assert_eq!((img.width(), img.height()), (w, h));
    assert_eq!(img.into_raw(), rgba);
}

#[test]
fn two_pastes_in_the_same_second_are_two_files_in_two_folders() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let ms = gizai_core::ids::now_ms();
    let a = save_png(&data, &pixels(2, 2), 2, 2, ms).unwrap();
    let b = save_png(&data, &[0, 0, 0, 255], 1, 1, ms).unwrap();
    assert_ne!(a, b);
    assert_eq!(a.file_name(), b.file_name(), "the same name, as the same second");
    assert_ne!(a.parent(), b.parent());
    assert!(a.is_file() && b.is_file());
    assert_eq!(folders(&data).len(), 2);
}

#[test]
fn a_big_screenshot_is_saved_whole() {
    let dir = tempfile::tempdir().unwrap();
    let (w, h) = (1920, 1080);
    let path = save_png(dir.path(), &pixels(w, h), w, h, gizai_core::ids::now_ms()).unwrap();
    let img = image::open(&path).unwrap();
    assert_eq!((img.width(), img.height()), (w, h));
}

#[test]
fn an_image_with_no_pixels_or_the_wrong_number_of_bytes_is_one_sentence_and_leaves_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let ms = gizai_core::ids::now_ms();
    for (rgba, w, h) in [(vec![], 0, 0), (vec![], 4, 0), (vec![1, 2, 3], 1, 1), (pixels(2, 2), 3, 2)] {
        let e = save_png(&data, &rgba, w, h, ms).unwrap_err();
        one_sentence(&e);
    }
    assert!(folders(&data).is_empty(), "{:?}", folders(&data));
}

#[test]
fn a_folder_that_cant_be_made_is_one_sentence_naming_it() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    std::fs::create_dir_all(&data).unwrap();
    // a file where the pasted folder should be
    std::fs::write(data.join(FOLDER), "not a folder").unwrap();
    let e = save_png(&data, &pixels(1, 1), 1, 1, gizai_core::ids::now_ms()).unwrap_err();
    one_sentence(&e);
    assert!(e.contains(&data.join(FOLDER).display().to_string()), "{e}");
    assert_eq!(std::fs::read_to_string(data.join(FOLDER)).unwrap(), "not a folder");
}

#[test]
fn an_error_reads_as_one_sentence_starting_the_same_way() {
    assert_eq!(FAILED, "The clipboard's image couldn't be pasted: ");
    // arboard's own sentences, lower-cased at the start, one full stop
    assert_eq!(failed("The native clipboard is not accessible due to being held by another party."),
        "The clipboard's image couldn't be pasted: the native clipboard is not accessible due to being held by another party.");
    assert_eq!(failed("Unknown error while interacting with the clipboard: X11 server connection timed out because it was unreachable"),
        "The clipboard's image couldn't be pasted: unknown error while interacting with the clipboard: X11 server connection timed out because it was unreachable.");
    // a name or an abbreviation keeps its capitals
    assert_eq!(failed("X11 isn't there"), "The clipboard's image couldn't be pasted: X11 isn't there.");
    assert_eq!(failed("I/O error"), "The clipboard's image couldn't be pasted: I/O error.");
    // spaces and full stops at the end are one full stop; nothing at all still says why
    assert_eq!(failed("  it timed out.  "), "The clipboard's image couldn't be pasted: it timed out.");
    assert_eq!(failed(""), "The clipboard's image couldn't be pasted: no reason was given.");
    for e in ["The native clipboard is not accessible.", "", "x", "É is not ASCII"] {
        one_sentence(&failed(e));
    }
    assert_eq!(failed("Él"), "The clipboard's image couldn't be pasted: él.");
}

#[test]
fn no_pasted_folder_means_nothing_to_clear() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(clear_old(dir.path(), gizai_core::ids::now_ms()), 0);
    assert_eq!(clear_old(&dir.path().join("not there"), gizai_core::ids::now_ms()), 0);
}

#[test]
fn folders_go_once_they_are_over_a_day_old_and_files_beside_them_stay() {
    assert_eq!(KEEP_MS, 24 * HOUR_MS);
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let now = gizai_core::ids::now_ms();
    let a = save_png(&data, &pixels(1, 1), 1, 1, now).unwrap();
    let b = save_png(&data, &pixels(1, 1), 1, 1, now).unwrap();
    std::fs::write(data.join(FOLDER).join("a file.txt"), "kept").unwrap();

    // now, and a day less a minute later: all kept
    assert_eq!(clear_old(&data, now), 0);
    assert_eq!(clear_old(&data, now + KEEP_MS - 60_000), 0);
    assert!(a.is_file() && b.is_file());
    // a day and a minute later: both folders and their images are gone, the file is not
    assert_eq!(clear_old(&data, now + KEEP_MS + 60_000), 2);
    assert!(!a.exists() && !b.exists() && !a.parent().unwrap().exists());
    assert!(folders(&data).is_empty());
    assert_eq!(std::fs::read_to_string(data.join(FOLDER).join("a file.txt")).unwrap(), "kept");
}

/// Sets a folder's last change `ago` before now (Unix: a folder opens as a file there; Windows needs a flag std lacks).
#[cfg(unix)]
fn age(folder: &Path, ago: Duration) {
    std::fs::File::open(folder).unwrap().set_modified(SystemTime::now() - ago).unwrap();
}

#[cfg(unix)]
fn pasted_folder(data: &Path, name: &str, ago: Duration) -> PathBuf {
    let d = data.join(FOLDER).join(name);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("pasted-image-2026-11-16_10-00-00.png"), b"\x89PNG").unwrap();
    age(&d, ago);
    d
}

#[cfg(unix)]
#[test]
fn only_folders_over_a_day_old_go_and_a_link_and_where_it_points_stay() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let hour = Duration::from_secs(3600);
    let old = pasted_folder(&data, "old", 25 * hour);
    let week = pasted_folder(&data, "a week", 7 * 24 * hour);
    let young = pasted_folder(&data, "young", 23 * hour);
    let new = pasted_folder(&data, "new", Duration::ZERO);
    // a link to a folder outside, itself old: neither the link nor its folder goes
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("keep.png"), "x").unwrap();
    age(&outside, 48 * hour);
    std::os::unix::fs::symlink(&outside, data.join(FOLDER).join("link")).unwrap();

    assert_eq!(clear_old(&data, gizai_core::ids::now_ms()), 2);
    assert!(!old.exists() && !week.exists());
    assert!(young.join("pasted-image-2026-11-16_10-00-00.png").is_file() && new.is_dir());
    assert!(data.join(FOLDER).join("link").symlink_metadata().is_ok());
    assert!(outside.join("keep.png").is_file());
}

#[cfg(unix)]
#[test]
fn hersir_removes_the_pasted_folders_over_a_day_old_when_it_starts_and_keeps_newer_ones() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let hour = Duration::from_secs(3600);
    let old = pasted_folder(&data, "old", 30 * hour);
    let young = pasted_folder(&data, "young", 2 * hour);
    let st = gizai_lib::test_state(dir.path());
    assert_eq!(st.data_dir, data);
    assert!(!old.exists(), "the folder of 30 hours ago is still there");
    assert!(young.is_dir(), "the folder of 2 hours ago is gone");
}
