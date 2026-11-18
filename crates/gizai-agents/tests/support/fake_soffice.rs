//! GA-311 QA: LibreOffice for the tests of the PDF copies, fake and real. The fake is tests/fake-soffice.sh (what it
//! does is told by the first line of the file it converts; it logs each call in the profile folder). The real one is
//! used only where it is installed: it makes an old Word document (.doc) and an old PowerPoint deck (.ppt) from the
//! flat OpenDocument files in tests/fixtures/libreoffice (through .odt and .odp, in a throwaway profile), as the card
//! asks. Only std and gizai_agents, so both the agents crate's and the app's tests can include it with `#[path]`.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// This crate's folder or the app's: where tests/fake-soffice.sh and the fixtures are, from either crate's tests.
fn agents_tests() -> PathBuf {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    [here.join("tests"), here.join("..").join("crates").join("gizai-agents").join("tests")].into_iter()
        .find(|d| d.join("fake-soffice.sh").is_file()).expect("crates/gizai-agents/tests/fake-soffice.sh")
}

/// Puts the fake soffice at `at` (its folder made when missing), executable. Copied by a child, so this process never
/// holds a script open for writing (that would make running it fail with "Text file busy").
pub fn install_fake(at: &Path) -> PathBuf {
    let src = agents_tests().join("fake-soffice.sh");
    let ok = Command::new("sh").args(["-c", r#"mkdir -p "$(dirname "$2")" && cp "$1" "$2" && chmod 755 "$2""#, "sh"])
        .arg(&src).arg(at).status().map(|s| s.success()).unwrap_or(false);
    assert!(ok, "can't put the fake soffice at {}", at.display());
    at.to_path_buf()
}

/// The fake's calls with the profile folder `profile`: each one's lines (its arguments, then PATH=…), oldest first.
pub fn calls(profile: &Path) -> Vec<Vec<String>> {
    let mut found: Vec<(std::time::SystemTime, Vec<String>)> = std::fs::read_dir(profile).into_iter().flatten().flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("call-"))
        .map(|e| (e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH),
                  std::fs::read_to_string(e.path()).unwrap_or_default().lines().map(str::to_string).collect()))
        .collect();
    found.sort_by_key(|(t, _)| *t);
    found.into_iter().map(|(_, l)| l).collect()
}

/// The real LibreOffice on this computer's PATH (or in its usual place), as Hersir finds it; None when it isn't installed.
pub fn real() -> Option<PathBuf> {
    gizai_agents::libreoffice::find(&std::env::var_os("PATH").unwrap_or_default())
}

/// The note a test prints when it skips its real-LibreOffice part.
pub fn skipped(test: &str) {
    eprintln!("{test}: LibreOffice isn't installed here, so the real conversion wasn't tried (the fake tests still ran)");
}

/// A .doc and a .ppt made with the real LibreOffice `soffice` from the fixtures (brief.fodt → .odt → .doc, two pages;
/// deck.fodp → .odp → .ppt, two slides), once per test program. Their bytes.
pub fn old_doc_and_ppt(soffice: &Path) -> &'static (Vec<u8>, Vec<u8>) {
    static MADE: OnceLock<(Vec<u8>, Vec<u8>)> = OnceLock::new();
    MADE.get_or_init(|| {
        let d = tempfile::tempdir().unwrap().keep();
        let profile = format!("-env:UserInstallation={}", gizai_agents::libreoffice::file_url(&d.join("profile")));
        let fixtures = agents_tests().join("fixtures").join("libreoffice");
        let convert = |to: &str, src: &Path, out: &Path| {
            let status = Command::new(soffice).args(["--headless", "--norestore", &profile, "--convert-to", to, "--outdir"])
                .arg(out).arg(src).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().unwrap();
            assert!(status.success(), "soffice --convert-to {to} {}: {status}", src.display());
        };
        convert("odt", &fixtures.join("brief.fodt"), &d.join("odf"));
        convert("odp", &fixtures.join("deck.fodp"), &d.join("odf"));
        convert("doc", &d.join("odf").join("brief.odt"), &d.join("old"));
        convert("ppt", &d.join("odf").join("deck.odp"), &d.join("old"));
        let (doc, ppt) = (std::fs::read(d.join("old").join("brief.doc")).unwrap(), std::fs::read(d.join("old").join("deck.ppt")).unwrap());
        // the old binary formats: OLE compound files
        for b in [&doc, &ppt] {
            assert_eq!(&b[..8], [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1], "an OLE compound file");
        }
        (doc, ppt)
    })
}

/// How many pages the PDF `pdf` has: its page objects (`/Type/Page`, not `/Type/Pages`).
pub fn pages(pdf: &[u8]) -> usize {
    let mut n = 0;
    for pat in [&b"/Type/Page"[..], &b"/Type /Page"[..]] {
        n += pdf.windows(pat.len() + 1).filter(|w| w.starts_with(pat) && !w[pat.len()].is_ascii_alphabetic()).count();
    }
    n
}

/// Whether `pdf` is a whole PDF: it starts with `%PDF-` and ends with `%%EOF`.
pub fn whole_pdf(pdf: &[u8]) -> bool {
    pdf.starts_with(b"%PDF-") && pdf[pdf.len().saturating_sub(1024)..].windows(5).any(|w| w == b"%%EOF")
}

/// Linux: the processes whose command line has `needle` (a profile folder's path), but not this test program.
#[cfg(target_os = "linux")]
pub fn running_with(needle: &str) -> Vec<u32> {
    let me = std::process::id();
    std::fs::read_dir("/proc").into_iter().flatten().flatten()
        .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
        .filter(|&pid| pid != me && !zombie(pid))
        .filter(|pid| std::fs::read(format!("/proc/{pid}/cmdline")).is_ok_and(|c| String::from_utf8_lossy(&c).contains(needle)))
        .collect()
}

/// Linux: whether `pid` has ended but isn't reaped yet.
#[cfg(target_os = "linux")]
fn zombie(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| s.rsplit_once(')').is_some_and(|(_, rest)| rest.trim_start().starts_with('Z')))
}

/// Whether the process `pid` still runs (a zombie, ended but not reaped, doesn't count on Linux).
#[cfg(unix)]
pub fn runs(pid: u32) -> bool {
    // SAFETY: signal 0 only checks that the process exists; nothing is sent.
    let there = unsafe { libc::kill(pid as i32, 0) == 0 };
    #[cfg(target_os = "linux")]
    return there && !zombie(pid);
    #[cfg(not(target_os = "linux"))]
    return there;
}

/// Waits up to `within` for `done`; whether it came.
pub fn wait_for(within: Duration, mut done: impl FnMut() -> bool) -> bool {
    let until = Instant::now() + within;
    loop {
        if done() {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A pid the fake wrote in `file`.
pub fn pid_in(file: &Path) -> u32 {
    std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{}: {e}", file.display())).trim().parse().unwrap()
}
