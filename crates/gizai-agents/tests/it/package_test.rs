//! GA-199: a Hersir from a package (the AUR, Homebrew or Scoop) knows it by `hersir-package.txt` beside its program (its
//! links resolved), or on macOS by running from Homebrew's Cellar or Caskroom, and knows the command that updates it.
//! Only files in a temp folder; on all three systems.
use std::path::{Path, PathBuf};

use gizai_agents::update::{self as up, PACKAGE_MARK};

/// A program `hersir` (`hersir.exe` on Windows) in <tmp>/lib/hersir, with `mark` beside it as hersir-package.txt when
/// given.
fn program(tmp: &Path, mark: Option<&[u8]>) -> PathBuf {
    let lib = tmp.join("lib").join("hersir");
    std::fs::create_dir_all(&lib).unwrap();
    let exe = lib.join(format!("hersir{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&exe, "a program").unwrap();
    if let Some(mark) = mark {
        std::fs::write(lib.join(PACKAGE_MARK), mark).unwrap();
    }
    exe
}

fn package_of_mark(mark: &[u8]) -> Option<String> {
    let tmp = tempfile::tempdir().unwrap();
    up::package_of(&program(tmp.path(), Some(mark)))
}

#[test]
fn the_mark_is_hersir_package_txt() {
    assert_eq!(PACKAGE_MARK, "hersir-package.txt");
}

#[test]
fn each_package_is_named_by_the_mark_beside_the_program() {
    for package in ["aur", "scoop", "homebrew"] {
        assert_eq!(package_of_mark(package.as_bytes()).as_deref(), Some(package), "{package}");
        assert_eq!(package_of_mark(format!("{package}\n").as_bytes()).as_deref(), Some(package), "{package} with a newline");
    }
}

#[test]
fn another_word_is_a_package_too() {
    assert_eq!(package_of_mark(b"nix").as_deref(), Some("nix"));
    assert_eq!(package_of_mark(b"Chocolatey\n").as_deref(), Some("chocolatey"));
}

#[test]
fn upper_case_and_spaces_read_as_the_word() {
    assert_eq!(package_of_mark(b"  AUR  \r\n").as_deref(), Some("aur"), "CRLF, as a Windows editor writes it");
    assert_eq!(package_of_mark(b"\tHomebrew \n").as_deref(), Some("homebrew"));
    assert_eq!(package_of_mark(b"SCOOP").as_deref(), Some("scoop"));
}

#[test]
fn only_the_first_line_counts() {
    assert_eq!(package_of_mark(b"aur\nscoop\n").as_deref(), Some("aur"));
    assert_eq!(package_of_mark(b"\naur\n"), None, "an empty first line is no package, whatever comes after it");
    assert_eq!(package_of_mark(b"   \r\nscoop\r\n"), None, "spaces only on the first line");
}

#[test]
fn an_empty_mark_or_none_is_no_package() {
    assert_eq!(package_of_mark(b""), None, "an empty file");
    assert_eq!(package_of_mark(b"\n"), None, "a newline only");
    assert_eq!(package_of_mark(b"  \t "), None, "spaces only");
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(up::package_of(&program(tmp.path(), None)), None, "no file");
    assert_eq!(up::package_of(&tmp.path().join("nowhere").join("hersir")), None, "a program that isn't there");
}

#[test]
fn a_mark_with_a_byte_order_mark_or_in_utf_16_reads_the_same() {
    assert_eq!(package_of_mark("\u{FEFF}scoop\r\n".as_bytes()).as_deref(), Some("scoop"), "UTF-8 with a BOM");
    let utf16 = |text: &str, unit: fn(u16) -> [u8; 2], bom: [u8; 2]| -> Vec<u8> {
        bom.into_iter().chain(text.encode_utf16().flat_map(unit)).collect()
    };
    // Windows PowerShell 5.1's Out-File and > write UTF-16 LE with a BOM
    assert_eq!(package_of_mark(&utf16("Scoop\r\n", u16::to_le_bytes, [0xFF, 0xFE])).as_deref(), Some("scoop"), "UTF-16 LE");
    assert_eq!(package_of_mark(&utf16("aur\n", u16::to_be_bytes, [0xFE, 0xFF])).as_deref(), Some("aur"), "UTF-16 BE");
}

#[test]
fn a_big_mark_is_read_by_its_start() {
    let mut big = b"aur\n".to_vec();
    big.extend(std::iter::repeat_n(b'x', 1 << 20));
    assert_eq!(package_of_mark(&big).as_deref(), Some("aur"));
}

// Windows: a Scoop Hersir is a hersir.exe; with install.ps1's marker beside it too, both see their own, and the app asks
// package_of first (src-tauri's update flow tests check that).
#[test]
fn a_mark_beside_install_ps1s_marker_is_still_the_package() {
    let tmp = tempfile::tempdir().unwrap();
    let exe = program(tmp.path(), Some(b"scoop"));
    std::fs::write(exe.with_file_name("hersir-installed.txt"), "").unwrap();
    assert_eq!(up::package_of(&exe).as_deref(), Some("scoop"));
}

#[cfg(unix)]
#[test]
fn a_link_to_the_program_reads_the_mark_beside_the_program_itself() {
    let tmp = tempfile::tempdir().unwrap();
    let exe = program(tmp.path(), Some(b"aur"));
    let bin = tmp.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::os::unix::fs::symlink("../lib/hersir/hersir", bin.join("hersir")).unwrap();
    assert_eq!(up::package_of(&bin.join("hersir")).as_deref(), Some("aur"), "bin/hersir links to lib/hersir/hersir");

    // a mark beside the link only is not the program's
    let other = tempfile::tempdir().unwrap();
    let bin = other.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join(PACKAGE_MARK), "aur").unwrap();
    std::os::unix::fs::symlink(&exe, bin.join("hersir")).unwrap();
    std::fs::remove_file(exe.with_file_name(PACKAGE_MARK)).unwrap();
    assert_eq!(up::package_of(&bin.join("hersir")), None, "the folder of the resolved program counts, not the link's");
}

#[test]
fn each_package_has_its_update_command() {
    assert_eq!(up::package_update_command("aur"), Some("yay -Syu"));
    assert_eq!(up::package_update_command("homebrew"), Some("brew update && brew upgrade hersir"));
    assert_eq!(up::package_update_command("scoop"), Some("scoop update; scoop update hersir"));
    for other in ["nix", "chocolatey", "", "brew", "yay"] {
        assert_eq!(up::package_update_command(other), None, "{other:?}");
    }
}

const CELLAR: &str = "/opt/homebrew/Cellar/hersir/1.0.0/bin/hersir";
const CASKROOM: &str = "/opt/homebrew/Caskroom/hersir/1.0.0/Hersir.app/Contents/MacOS/hersir";

/// <tmp>/Cellar/hersir/1.0.0/bin/hersir (or Caskroom), without a mark, and <tmp>/bin/hersir linking to it.
#[cfg(unix)]
fn homebrew_link(tmp: &Path, room: &str) -> PathBuf {
    let real = tmp.join(room).join("hersir/1.0.0/bin");
    std::fs::create_dir_all(&real).unwrap();
    std::fs::write(real.join("hersir"), "a program").unwrap();
    std::fs::create_dir_all(tmp.join("bin")).unwrap();
    std::os::unix::fs::symlink(real.join("hersir"), tmp.join("bin/hersir")).unwrap();
    tmp.join("bin/hersir")
}

#[cfg(target_os = "macos")]
#[test]
fn on_macos_a_hersir_from_homebrews_cellar_or_caskroom_is_homebrew_without_a_mark() {
    assert_eq!(up::package_of(Path::new(CELLAR)).as_deref(), Some("homebrew"), "a formula");
    assert_eq!(up::package_of(Path::new(CASKROOM)).as_deref(), Some("homebrew"), "a cask");
    assert_eq!(up::package_of(Path::new("/usr/local/Cellar/hersir/1.0.0/libexec/hersir")).as_deref(), Some("homebrew"), "an Intel Mac's Homebrew");
    for room in ["Cellar", "Caskroom"] {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(up::package_of(&homebrew_link(tmp.path(), room)).as_deref(), Some("homebrew"), "through a link into {room}");
    }
    for not in ["/opt/homebrew/Cellar/other/1.0.0/bin/hersir", "/opt/homebrew/Cellar/hersir-beta/1.0.0/bin/hersir",
                "/Users/x/Cellar.hersir/bin/hersir", "/Users/x/.local/lib/hersir/hersir", "/Applications/Hersir.app/Contents/MacOS/hersir"] {
        assert_eq!(up::package_of(Path::new(not)), None, "{not}");
    }
}

#[cfg(not(target_os = "macos"))]
#[test]
fn elsewhere_homebrews_folders_mean_nothing() {
    assert_eq!(up::package_of(Path::new(CELLAR)), None);
    assert_eq!(up::package_of(Path::new(CASKROOM)), None);
    #[cfg(unix)]
    for room in ["Cellar", "Caskroom"] {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(up::package_of(&homebrew_link(tmp.path(), room)), None, "through a link into {room}");
    }
}

#[test]
fn the_install_scripts_never_write_the_mark() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for script in ["install.sh", "install.ps1", "scripts/get.sh", "scripts/get.ps1"] {
        let text = std::fs::read_to_string(root.join(script)).unwrap();
        assert!(!text.contains("hersir-package"), "{script} mentions the package mark");
    }
}
