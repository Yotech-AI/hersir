//! PDF copies through LibreOffice (GA-311): Apple's Pages, Keynote and Numbers files and the old binary Word (.doc,
//! .dot) and PowerPoint (.ppt, .pps) files. Their formats are closed or binary, so Hersir doesn't read them itself
//! (`gizai_core::readable` reads Word, Excel, PowerPoint, OpenDocument and RTF files as text, GA-310). When LibreOffice
//! is installed, Hersir has it make a PDF of such a file beside the Chief's copy, `<name>.pdf`, which the Chief reads
//! with Read: all pages, with layout and pictures. Made once, and made again only when it is missing
//! (`LibreOffice::ensure_pdf`).
//!
//! LibreOffice runs with a profile folder of Hersir's own (`-env:UserInstallation`): a LibreOffice window that is open
//! (on your own profile) doesn't take the job and isn't disturbed. One conversion runs at a time: a second one on the
//! same profile would be handed to the first LibreOffice. It starts as Hersir starts other programs (`os::command`: no
//! console window on Windows), as a tree of its own (`os::spawn_tree`); after its limit (120 s) it is ended with
//! everything it started, and anything it started and left running is ended too.
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, ExitStatus, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::os::{self, End, Tree};

/// The extensions (lower case, without their dot) of the files that get a PDF copy through LibreOffice: Pages, Keynote,
/// Numbers, the old Word document and template, and the old PowerPoint presentation and show.
pub const EXTENSIONS: [&str; 7] = ["pages", "key", "numbers", "doc", "dot", "ppt", "pps"];

/// How long a conversion may take; then Hersir ends LibreOffice, with everything it started.
pub const LIMIT: Duration = Duration::from_secs(120);

/// How long Hersir waits for what it ended to be gone.
const GONE_WAIT: Duration = Duration::from_secs(5);

/// How often Hersir looks whether LibreOffice is done.
const POLL: Duration = Duration::from_millis(50);

/// The file in the conversion's folder that gets what LibreOffice writes (stdout and stderr).
const SAID: &str = "libreoffice-output.txt";

/// One conversion at a time, in all of Hersir: a second one on the same profile would be handed to the first
/// LibreOffice, which may not do it.
static CONVERTING: Mutex<()> = Mutex::new(());

/// The extension of a file called `name` when it gets a PDF copy through LibreOffice, in lower case with its dot
/// (`.pages`); None for any other file.
pub fn pdf_extension(name: &str) -> Option<String> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    EXTENSIONS.contains(&ext.as_str()).then(|| format!(".{ext}"))
}

/// Whether a file called `name` gets a PDF copy through LibreOffice: a Pages, Keynote, Numbers, .doc, .dot, .ppt or .pps
/// file, by its extension (case ignored).
pub fn has_pdf(name: &str) -> bool {
    pdf_extension(name).is_some()
}

/// Where the PDF copy of the file copied at `copy` goes: beside it, with `.pdf` added to its name.
pub fn pdf_path(copy: &Path) -> PathBuf {
    let mut p = copy.as_os_str().to_owned();
    p.push(".pdf");
    PathBuf::from(p)
}

/// Why a file with the extension `ext` (`.pages`) has no PDF copy when LibreOffice isn't installed.
pub fn not_installed(ext: &str) -> String {
    format!("Hersir needs LibreOffice to read {ext} files: install it from https://www.libreoffice.org, or save the file as PDF or .docx")
}

/// Where LibreOffice's program is: `soffice`, else `libreoffice`, on `path` (the PATH Hersir gives the coding CLIs); on
/// macOS also `/Applications/LibreOffice.app/Contents/MacOS/soffice`, and on Windows also
/// `LibreOffice\program\soffice.exe` under Program Files. None when it isn't installed. It looks at the disk: call it off
/// the async threads.
pub fn find(path: &OsStr) -> Option<PathBuf> {
    ["soffice", "libreoffice"].into_iter().find_map(|name| os::find_in(name, path)).or_else(installed)
}

/// macOS: LibreOffice in Applications.
#[cfg(target_os = "macos")]
fn installed() -> Option<PathBuf> {
    Some(PathBuf::from("/Applications/LibreOffice.app/Contents/MacOS/soffice")).filter(|p| os::executable(p))
}

/// Windows: LibreOffice under Program Files (the 64-bit folder, then the 32-bit one).
#[cfg(windows)]
fn installed() -> Option<PathBuf> {
    let folders = ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"].into_iter().filter_map(std::env::var_os).map(PathBuf::from)
        .chain([PathBuf::from(r"C:\Program Files"), PathBuf::from(r"C:\Program Files (x86)")]);
    folders.map(|f| f.join("LibreOffice").join("program").join("soffice.exe")).find(|p| p.is_file())
}

/// Linux: only on the PATH.
#[cfg(not(any(target_os = "macos", windows)))]
fn installed() -> Option<PathBuf> {
    None
}

/// `dir` as the file URL LibreOffice takes for its profile folder (`-env:UserInstallation=`): `file:///home/you/…` on
/// Linux and macOS, `file:///C:/Users/you/…` on Windows, `file://server/share/…` for a Windows network folder; a
/// relative `dir` from the current folder. Every byte but ASCII letters, digits and `-._~/:` is percent-encoded (a name
/// outside ASCII as UTF-8), so a space, `#` or `%` in the path gets through.
pub fn file_url(dir: &Path) -> String {
    let dir = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
    #[cfg(unix)]
    let path: Vec<u8> = {
        use std::os::unix::ffi::OsStrExt;
        dir.as_os_str().as_bytes().to_vec()
    };
    #[cfg(windows)]
    let path: Vec<u8> = {
        let p = dir.to_string_lossy().replace('\\', "/");
        // a path Windows was given whole (\\?\C:\…, \\?\UNC\server\share\…) as the usual kind
        let usual = match p.strip_prefix("//?/UNC/") {
            Some(unc) => format!("//{unc}"),
            None => p.strip_prefix("//?/").unwrap_or(&p).to_string(),
        };
        usual.into_bytes()
    };
    url_of(&path)
}

/// The file URL of a path written with `/` (`file_url`).
fn url_of(path: &[u8]) -> String {
    let (head, rest): (&str, &[u8]) = match path {
        [b'/', b'/', unc @ ..] => ("file://", unc),
        [b'/', ..] => ("file://", path),
        _ => ("file:///", path),
    };
    let mut url = String::from(head);
    for &b in rest {
        if b.is_ascii_alphanumeric() || b"-._~/:".contains(&b) {
            url.push(char::from(b));
        } else {
            url.push_str(&format!("%{b:02X}"));
        }
    }
    url
}

/// How Hersir runs LibreOffice: in a profile folder of its own, for at most a limit, and which program. Cheap to clone.
#[derive(Debug, Clone)]
pub struct LibreOffice {
    /// Hersir's own profile folder for LibreOffice (`-env:UserInstallation`); LibreOffice makes it when it isn't there.
    pub profile: PathBuf,
    /// How long a conversion may take (`LIMIT`).
    pub limit: Duration,
    /// Which program runs.
    pub program: Program,
}

/// Which LibreOffice program `LibreOffice` runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Program {
    /// The one `find` finds, looked for each time a PDF has to be made, so one installed a moment ago is found.
    Find,
    /// This one (tests: a fake soffice).
    At(PathBuf),
    /// None, as when LibreOffice isn't installed (tests).
    Missing,
}

impl LibreOffice {
    /// LibreOffice with its profile in `profile`, found when it is needed, with the usual limit (`LIMIT`).
    pub fn new(profile: PathBuf) -> LibreOffice {
        LibreOffice { profile, limit: LIMIT, program: Program::Find }
    }

    /// The PDF copy of the file copied at `copy`, called `name` (`has_pdf`), made when it isn't there yet (`pdf_path`):
    /// its path, or why it couldn't be made, in plain words on one line. A PDF copy that is there is kept as it is, also
    /// when LibreOffice has gone since. `path` gives the PATH that `find` looks on (and LibreOffice then runs with),
    /// asked for only when a PDF has to be made. Never half a PDF: LibreOffice writes it in a new folder beside the copy,
    /// and only a PDF it finished is moved in place. Blocks while LibreOffice works (at most `limit`) and while another
    /// conversion runs: call it off the async threads.
    pub fn ensure_pdf(&self, name: &str, copy: &Path, path: impl FnOnce() -> OsString) -> Result<PathBuf, String> {
        let out = pdf_path(copy);
        if out.is_file() {
            return Ok(out);
        }
        let Some(ext) = pdf_extension(name) else {
            return Err("Hersir makes PDF copies only of Pages, Keynote, Numbers, .doc and .ppt files".into());
        };
        let size = fs::metadata(copy).map_err(|e| format!("it can't be read: {e}"))?.len();
        if size == 0 {
            return Err("the file is empty".into());
        }
        let (program, path) = match &self.program {
            Program::Find => {
                let path = path();
                (find(&path), Some(path))
            }
            Program::At(p) => (Some(p.clone()), None),
            Program::Missing => (None, None),
        };
        let Some(program) = program else {
            return Err(not_installed(&ext));
        };
        let _one = CONVERTING.lock().unwrap_or_else(|e| e.into_inner());
        // made by another call while this one waited
        if out.is_file() {
            return Ok(out);
        }
        let dir = new_dir(copy).map_err(|e| format!("its PDF copy couldn't be made: {e}"))?;
        let made = self.convert(&program, path.as_deref(), copy, &dir).and_then(|pdf| place(&pdf, &out));
        let _ = fs::remove_dir_all(&dir);
        made
    }

    /// Has LibreOffice (`program`, run with `path` as its PATH when given) make a PDF of `copy` in the empty folder `dir`:
    /// the PDF it made, or why there is none. After `limit` LibreOffice is ended with everything it started; when it ends
    /// by itself, anything it started and left running is ended too.
    fn convert(&self, program: &Path, path: Option<&OsStr>, copy: &Path, dir: &Path) -> Result<PathBuf, String> {
        let said = File::create(dir.join(SAID)).and_then(|f| Ok((f.try_clone()?, f)))
            .map_err(|e| format!("its PDF copy couldn't be made: {e}"))?;
        let mut cmd = os::command(program);
        cmd.arg("--headless").arg("--norestore").arg(format!("-env:UserInstallation={}", file_url(&self.profile)))
            .arg("--convert-to").arg("pdf").arg("--outdir").arg(dir).arg(copy)
            .stdin(Stdio::null()).stdout(said.0).stderr(said.1);
        if let Some(path) = path {
            cmd.env("PATH", path);
        }
        let (mut child, tree) = os::spawn_tree(&mut cmd, false)
            .map_err(|e| format!("LibreOffice couldn't be started ({}): {e}", program.display()))?;
        let until = Instant::now() + self.limit;
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break Ok(s),
                Ok(None) if Instant::now() < until => std::thread::sleep(POLL),
                Ok(None) => break Err(format!("LibreOffice took more than {} to make its PDF, so Hersir stopped it", secs(self.limit))),
                Err(e) => break Err(format!("Hersir lost track of LibreOffice while it made its PDF ({e})")),
            }
        };
        end(&mut child, &tree);
        let status = status?;
        match made_pdf(dir) {
            // a whole PDF, also when LibreOffice failed as it quit
            Some((pdf, whole)) if whole || status.success() => Ok(pdf),
            _ => Err(failed(status, &last_said(&dir.join(SAID)))),
        }
    }
}

/// Ends what is left of a LibreOffice: everything in its tree when anything there still runs (after its limit, or what
/// it started and left running), and waits a moment for it to be gone. Reaps the program Hersir started.
fn end(child: &mut Child, tree: &Tree) {
    let running = !matches!(child.try_wait(), Ok(Some(_)));
    if running || tree.alive() {
        tree.end(End::Kill);
    }
    let _ = child.wait();
    let until = Instant::now() + GONE_WAIT;
    while tree.alive() && Instant::now() < until {
        std::thread::sleep(POLL);
    }
}

/// `d` as LibreOffice's limit is said: "120 s", "1.5 s".
fn secs(d: Duration) -> String {
    if d.subsec_millis() == 0 { format!("{} s", d.as_secs()) } else { format!("{} s", d.as_secs_f64()) }
}

/// Why LibreOffice made no PDF: what it said last (an error first), else how it ended.
fn failed(status: ExitStatus, said: &str) -> String {
    if !said.is_empty() {
        return format!("LibreOffice couldn't make a PDF of it ({said})");
    }
    match status.code() {
        Some(0) => "LibreOffice ended without making a PDF of it".into(),
        Some(code) => format!("LibreOffice couldn't make a PDF of it (exit code {code})"),
        None => format!("LibreOffice couldn't make a PDF of it (it stopped: {status})"),
    }
}

/// The line LibreOffice wrote last that says why it made no PDF, from the file `said`: its last line that starts with
/// "Error", else its last line that isn't a warning, on one line and at most 300 characters. Empty when there is none.
fn last_said(said: &Path) -> String {
    let text = tail(said, 64 << 10).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    // GLib's, Fontconfig's and Java's notes on stderr, which aren't why
    let warning = |l: &str| l.to_ascii_lowercase().contains("warning") || l.starts_with("javaldx");
    let line = lines.iter().rev().find(|l| l.to_ascii_lowercase().starts_with("error"))
        .or_else(|| lines.iter().rev().find(|l| !warning(l))).copied().unwrap_or_default();
    line.chars().filter(|c| !c.is_control()).take(300).collect()
}

/// The last `max` bytes of the file at `path`.
fn tail(path: &Path, max: u64) -> std::io::Result<Vec<u8>> {
    let mut f = File::open(path)?;
    let len = f.metadata()?.len();
    f.seek(SeekFrom::Start(len.saturating_sub(max)))?;
    let mut buf = Vec::new();
    f.take(max).read_to_end(&mut buf)?;
    Ok(buf)
}

/// The PDF LibreOffice made in `dir` (a file that ends in `.pdf`, not empty), and whether it is whole: it starts as a
/// PDF does (`%PDF-`) and ends with its end mark (`%%EOF`).
fn made_pdf(dir: &Path) -> Option<(PathBuf, bool)> {
    let pdf = fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.extension().is_some_and(|x| x.eq_ignore_ascii_case("pdf")) && fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() > 0)
    })?;
    let head = File::open(&pdf).and_then(|mut f| {
        let mut b = [0u8; 5];
        f.read_exact(&mut b).map(|()| b)
    });
    let whole = head.is_ok_and(|b| &b == b"%PDF-") && tail(&pdf, 1024).is_ok_and(|t| t.windows(5).any(|w| w == b"%%EOF"));
    Some((pdf, whole))
}

/// Moves the PDF LibreOffice made (`pdf`) to `out`, beside the copy (in the same folder as `pdf`'s, so it is there
/// whole or not at all).
fn place(pdf: &Path, out: &Path) -> Result<PathBuf, String> {
    match fs::rename(pdf, out) {
        Ok(()) => Ok(out.to_path_buf()),
        // made at the same time by another Hersir on this data (on Windows a rename doesn't replace a file)
        Err(_) if out.is_file() => Ok(out.to_path_buf()),
        Err(e) => Err(format!("its PDF copy couldn't be written: {e}")),
    }
}

/// A new, empty folder beside `copy` for LibreOffice to write its PDF in: `.pdf-<random>.part`.
fn new_dir(copy: &Path) -> std::io::Result<PathBuf> {
    let parent = copy.parent().ok_or_else(|| std::io::Error::other("it has no folder"))?;
    let mut last = None;
    for _ in 0..8 {
        let mut tag = [0u8; 6];
        if getrandom::fill(&mut tag).is_err() {
            let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
            tag.copy_from_slice(&(n as u64 ^ u64::from(std::process::id())).to_be_bytes()[2..]);
        }
        let hex: String = tag.iter().map(|b| format!("{b:02x}")).collect();
        let dir = parent.join(format!(".pdf-{hex}.part"));
        match fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| std::io::Error::other("no folder name was free")))
}
