//! Hersir's own log (GA-281): one file per start in `<data>/logs/` (`folder`), named `hersir-<UTC start time>.log`
//! (`file_name`), so that after a crash we, or a user, can read what Hersir printed up to its end. Why: Hersir ended at
//! once on 13 Nov 2026 and on earlier days, started from the app launcher, and whatever it or GTK printed just before
//! (GTK3's `Error 71 (Protocol error) dispatching to Wayland display`, after which GTK ends the app) went nowhere.
//!
//! - **The start** (`start`, in `run`'s setup once the data folder's lock is held, before the database opens): the
//!   first line names the version, the start time, the pid and the system (`first_line`). In a release build that isn't
//!   a headless test or screenshot run, the process's own stdout and stderr then point to the file (`point_std_to`:
//!   `dup2` on Linux and macOS, `SetStdHandle` on Windows), straight to it with no pipe or thread, so Hersir's own lines,
//!   what GTK, WebKit and other libraries print in Hersir's process, and child processes that inherit stdio are in it up
//!   to the last line before an abrupt end. Before that it prints `Hersir's log: <file>` where it was started from.
//!   Test runs and debug builds keep printing to stderr as before; their file gets this module's own lines (`line`),
//!   which they print on stderr too.
//! - **Alive**: every minute a line says Hersir is alive, with its runs at work and memory (`watch`, `alive_line`).
//! - **Panics**: the panic hook writes the message, its place and a backtrace first (`panic_hook`).
//! - **An unexpected end**: `running.json` says this start is under way; quitting writes a last `quit` line and removes
//!   it (`quit`). One found at the next start means that Hersir ended without quitting (killed, crashed, ended by GTK):
//!   the new log says so, and `unexpected-end.json` keeps it until it is dismissed or a newer one replaces it
//!   (`previous_end`, `info`, `dismiss`).
//! - **Old logs**: at start only the 20 newest stay (`KEEP`, `remove_old`).
//!
//! Text from outside Hersir that it logs (git's words, a command's error) goes through `gizai_agents::redact::hidden`
//! first: users send us these files.
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::AppState;

/// The logs folder in the data folder.
pub const FOLDER: &str = "logs";
/// How many logs stay at a start: the newest, by their start time.
pub const KEEP: usize = 20;
/// The file that says a Hersir runs on this data folder now (`Running`): its pid, start time and log.
pub const RUNNING: &str = "running.json";
/// The last unexpected end (`UnexpectedEnd`), until it is dismissed or a newer one replaces it.
pub const UNEXPECTED_END: &str = "unexpected-end.json";
/// How often the alive line comes.
pub const ALIVE_EVERY: Duration = Duration::from_secs(60);

/// `<data>/logs`: what `open_logs_folder` opens.
pub fn folder(data_dir: &Path) -> PathBuf {
    data_dir.join(FOLDER)
}

/// Unix ms as ISO 8601 UTC, to the second: "2026-11-13T14:50:26Z".
pub fn utc(ms: i64) -> String {
    let s = ms.div_euclid(1000).rem_euclid(86_400);
    format!("{}T{:02}:{:02}:{:02}Z", crate::tools::ymd(ms), s / 3600, s / 60 % 60, s % 60)
}

/// The log of a start at `ms`: "hersir-2026-11-13T14-50-26Z.log" (dashes for colons, which Windows refuses in a name).
pub fn file_name(ms: i64) -> String {
    format!("hersir-{}.log", utc(ms).replace(':', "-"))
}

/// The log's first line: "Hersir 0.17.0 started 2026-11-13T14:50:26Z, pid 4242, Linux 6.17.8 x86_64 (…)".
pub fn first_line(version: &str, started_ms: i64, pid: u32, system: &str) -> String {
    format!("Hersir {version} started {}, pid {pid}, {system}", utc(started_ms))
}

/// The line every minute: "2026-11-13T14:51:26Z alive: 2 runs at work, Hersir 1.4 GB, free 21.7 GB". Hersir's own
/// memory (`own_memory`) and the free memory are left out when the system doesn't say.
pub fn alive_line(now_ms: i64, runs: usize, own: Option<u64>, free: Option<u64>) -> String {
    let mut said = vec![format!("{runs} {} at work", if runs == 1 { "run" } else { "runs" })];
    if let Some(bytes) = own {
        said.push(format!("Hersir {}", gb(bytes)));
    }
    if let Some(bytes) = free {
        said.push(format!("free {}", gb(bytes)));
    }
    format!("{} alive: {}", utc(now_ms), said.join(", "))
}

/// Bytes in GB with one decimal, as Settings → Runs counts a GB (2³⁰ bytes).
fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / gizai_agents::run_memory::GB as f64)
}

/// A panic as the log says it: "<time> panic: <message> at <file>:<line>", then the backtrace on the lines after it.
pub fn panic_text(now_ms: i64, message: &str, place: &str, backtrace: &str) -> String {
    format!("{} panic: {message} at {place}\n{}", utc(now_ms), backtrace.trim_end())
}

/// `running.json`: the Hersir that runs on this data folder now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Running {
    pub pid: u32,
    /// ISO 8601 UTC.
    pub started_at: String,
    /// Its log, the full path.
    pub file: String,
}

/// A Hersir that ended without quitting: `unexpected-end.json`, and `get_logs_info`'s `unexpectedEnd`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnexpectedEnd {
    /// Its log, the full path.
    pub file: String,
    /// When it started, ISO 8601 UTC.
    pub started_at: String,
    /// When its log was last written (the file's modification time; its start time when the file is gone), ISO 8601
    /// UTC. With the alive line every minute, at most a minute before it ended.
    pub last_written_at: String,
}

/// What `get_logs_info` returns (`LogsInfo` in src/types.ts).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsInfo {
    /// The logs folder, the full path.
    pub folder: String,
    /// This start's log, the full path; "" when this Hersir keeps none (its folder couldn't be made).
    pub current: String,
    /// The previous start that ended unexpectedly, until it is dismissed.
    pub unexpected_end: Option<UnexpectedEnd>,
}

/// The logs of the data folder `data_dir`: its folder, `current` (this start's log) and the unexpected end it keeps.
pub fn info(data_dir: &Path, current: Option<&Path>) -> LogsInfo {
    let dir = folder(data_dir);
    LogsInfo {
        folder: dir.display().to_string(),
        current: current.map(|p| p.display().to_string()).unwrap_or_default(),
        unexpected_end: fs::read_to_string(dir.join(UNEXPECTED_END)).ok().and_then(|t| serde_json::from_str(&t).ok()),
    }
}

/// Dismisses the unexpected end the data folder keeps: `unexpected-end.json` goes. Nothing to dismiss is fine too.
pub fn dismiss(data_dir: &Path) -> Result<(), String> {
    let file = folder(data_dir).join(UNEXPECTED_END);
    match fs::remove_file(&file) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("Couldn't remove {}: {e}", file.display())),
    }
}

/// At start, with the data folder's lock held, so no other Hersir runs on it: a `running.json` left there means the
/// previous Hersir ended without quitting. Its unexpected end is kept in `unexpected-end.json` (replacing an older one)
/// and returned with the line for this start's log. Its log is looked up by name in this logs folder, also when the data
/// folder moved since. `running.json` itself stays, for `mark_running` to replace. None: the previous Hersir quit, or
/// none ran here before.
pub fn previous_end(data_dir: &Path) -> Option<(UnexpectedEnd, String)> {
    let dir = folder(data_dir);
    let text = fs::read_to_string(dir.join(RUNNING)).ok()?;
    let previous: Running = match serde_json::from_str(&text) {
        Ok(r) => r,
        Err(e) => {
            line(&format!("gizai: a previous Hersir left {} behind, which can't be read: {e}", dir.join(RUNNING).display()));
            return None;
        }
    };
    let file = Path::new(&previous.file).file_name().map(|name| dir.join(name)).unwrap_or_else(|| PathBuf::from(&previous.file));
    let last_written_at = fs::metadata(&file).and_then(|m| m.modified()).ok().map(|t| utc(ms_of(t))).unwrap_or_else(|| previous.started_at.clone());
    let end = UnexpectedEnd { file: file.display().to_string(), started_at: previous.started_at, last_written_at };
    if let Err(e) = write_json(&dir.join(UNEXPECTED_END), &end) {
        line(&format!("gizai: can't keep the previous Hersir's unexpected end in {}: {e}", dir.join(UNEXPECTED_END).display()));
    }
    let said = format!("The previous Hersir (pid {}, started {}) ended unexpectedly; its log is {}, last written {}",
                       previous.pid, end.started_at, end.file, end.last_written_at);
    Some((end, said))
}

/// Writes `running.json` for this start: from now on an end without quitting shows at the next start.
pub fn mark_running(data_dir: &Path, running: &Running) -> std::io::Result<()> {
    write_json(&folder(data_dir).join(RUNNING), running)
}

/// Removes all but the `KEEP` newest logs (`hersir-*.log`, by the start time in their name) in `dir`, never one in
/// `keep` (by name: this start's log, the previous one's, the kept unexpected end's). Returns how many it removed.
pub fn remove_old(dir: &Path, keep: &[&Path]) -> usize {
    let Ok(entries) = fs::read_dir(dir) else { return 0 };
    let mut logs: Vec<((String, u32), PathBuf)> = entries.flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| Some((order(e.file_name().to_str()?)?, e.path())))
        .collect();
    // newest first
    logs.sort_by(|a, b| b.0.cmp(&a.0));
    let kept = |path: &Path| keep.iter().any(|k| k.file_name().is_some() && k.file_name() == path.file_name());
    logs.into_iter().skip(KEEP).filter(|(_, path)| !kept(path)).filter(|(_, path)| fs::remove_file(path).is_ok()).count()
}

/// A log's place among the others, by its name: its start time, then its number among the starts in that second
/// ("hersir-…26Z.log" is 1, "hersir-…26Z-2.log" 2). None for a file that isn't a log.
fn order(name: &str) -> Option<(String, u32)> {
    let stem = name.strip_prefix("hersir-")?.strip_suffix(".log")?;
    match stem.rsplit_once("Z-") {
        Some((time, n)) => Some((format!("{time}Z"), n.parse().ok()?)),
        None => Some((stem.to_string(), 1)),
    }
}

/// Makes this start's log in `dir`, empty: `file_name`, else, for a second start in the same second, with `-2`, `-3` …
/// Opened to append, so every write lands at its end, also through stdout and stderr.
fn create(dir: &Path, ms: i64) -> std::io::Result<(PathBuf, File)> {
    let name = file_name(ms);
    let stem = name.trim_end_matches(".log");
    for n in 1..=100 {
        let path = dir.join(if n == 1 { name.clone() } else { format!("{stem}-{n}.log") });
        match OpenOptions::new().append(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, format!("{name} and 99 more like it are there already")))
}

/// Writes `value` to `path` as JSON, whole or not at all: through a file next to it, renamed into place.
fn write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?)?;
    fs::rename(&tmp, path)
}

/// A file time as Unix ms.
fn ms_of(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

// ---- this start's log: Hersir's own process only (`run`), never a test's ----

/// This start's log.
struct Log {
    data_dir: PathBuf,
    file: PathBuf,
    /// The file, open to append. Never closed: on Windows stdout and stderr are this handle.
    out: File,
    /// Whether stdout and stderr point to it (a release build outside a test run).
    redirected: bool,
}

static LOG: OnceLock<Log> = OnceLock::new();
/// Whether the quit line is written: no alive line comes after it. Held while either is written.
static QUIT: Mutex<bool> = Mutex::new(false);

/// This start's log, once `start` made it.
pub fn current() -> Option<&'static Path> {
    LOG.get().map(|log| log.file.as_path())
}

/// Starts this start's log in the data folder `data_dir` (`run`'s setup: once its lock is held, before the database
/// opens, so the start's own lines are in it). Makes the logs folder and this start's file with its first line, and
/// prints `Hersir's log: <file>` where Hersir was started from; with `redirect` (a release build outside a test run) it
/// then points stdout and stderr to the file. Then the previous Hersir's unexpected end, if it had one (`previous_end`),
/// this start's `running.json`, and only the `KEEP` newest logs stay, besides the previous one's and that of the
/// unexpected end kept until it is dismissed. Returns the log; Err: Hersir keeps none this time and prints where it did
/// before.
pub fn start(data_dir: &Path, redirect: bool) -> Result<PathBuf, String> {
    let dir = folder(data_dir);
    fs::create_dir_all(&dir).map_err(|e| format!("can't make {}: {e}", dir.display()))?;
    let now = gizai_core::ids::now_ms();
    let (file, out) = create(&dir, now).map_err(|e| format!("can't make a log in {}: {e}", dir.display()))?;
    let pid = std::process::id();
    let first = first_line(env!("CARGO_PKG_VERSION"), now, pid, &system());
    let _ = (&out).write_all(format!("{first}\n").as_bytes());
    eprintln!("Hersir's log: {}", file.display());
    let redirected = redirect && match point_std_to(&out) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("gizai: Hersir prints here, not in its log: {e}");
            false
        }
    };
    if !redirected {
        eprintln!("{first}");
    }
    let _ = LOG.set(Log { data_dir: data_dir.to_path_buf(), file: file.clone(), out, redirected });
    let previous = previous_end(data_dir);
    if let Some((_, said)) = &previous {
        line(said);
    }
    if let Err(e) = mark_running(data_dir, &Running { pid, started_at: utc(now), file: file.display().to_string() }) {
        line(&format!("gizai: can't write {}, so an unexpected end of this Hersir won't show at the next start: {e}", dir.join(RUNNING).display()));
    }
    // this start's log stays, and so do the previous one's and that of the unexpected end kept until it is dismissed,
    // which the page points to
    let previous_file = previous.map(|(end, _)| PathBuf::from(end.file));
    let kept_end = info(data_dir, None).unexpected_end.map(|end| PathBuf::from(end.file));
    let keep: Vec<&Path> = [Some(file.as_path()), previous_file.as_deref(), kept_end.as_deref()].into_iter().flatten().collect();
    let removed = remove_old(&dir, &keep);
    if removed > 0 {
        line(&format!("gizai: removed {removed} old logs (the {KEEP} newest stay)"));
    }
    Ok(file)
}

/// Writes `text` as a line in this start's log, in one write, and on stderr too when stderr doesn't point to the log (a
/// test run, a debug build). Without a log (before `start`, or when it failed), on stderr only.
pub fn line(text: &str) {
    let Some(log) = LOG.get() else {
        eprintln!("{text}");
        return;
    };
    let _ = (&log.out).write_all(format!("{text}\n").as_bytes());
    if !log.redirected {
        eprintln!("{text}");
    }
}

/// Quitting's last step (`RunEvent::Exit`, after the agents and the slice have ended), and a start that fails once the
/// log runs: the last line, `<time> quit`, and `running.json` goes, so the next start knows this Hersir quit.
pub fn quit() {
    let Some(log) = LOG.get() else { return };
    let mut quit = QUIT.lock().unwrap_or_else(|e| e.into_inner());
    if !*quit {
        line(&format!("{} quit", utc(gizai_core::ids::now_ms())));
        *quit = true;
    }
    let _ = fs::remove_file(folder(&log.data_dir).join(RUNNING));
}

/// Every minute while Hersir runs, the alive line (`alive_line`): the runs at work (`runs::live`), Hersir's own memory
/// (`own_memory`) and the free memory (`run_memory::available`). On a thread of its own, so it comes also while the
/// async runtime is busy.
pub fn watch(st: AppState) {
    let watching = std::thread::Builder::new().name("hersir-alive".into()).spawn(move || loop {
        std::thread::sleep(ALIVE_EVERY);
        let said = alive_line(gizai_core::ids::now_ms(), crate::runs::live(&st).len(), own_memory(),
                              gizai_agents::run_memory::available());
        let quit = QUIT.lock().unwrap_or_else(|e| e.into_inner());
        if *quit {
            return;
        }
        line(&said);
    });
    if let Err(e) = watching {
        line(&format!("gizai: the log's line every minute can't start: {e}"));
    }
}

/// Hersir's panic hook (`run`, first): a panic writes `<time> panic: <message> at <file>:<line>` and a backtrace in the
/// log first (`panic_text`; on stderr too when that isn't the log), then the usual hook runs. Also for a panic before the
/// log started: then on stderr only.
pub fn panic_hook() {
    let usual = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info.payload();
        let message = payload.downcast_ref::<&str>().map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(no message)".into());
        let place = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_else(|| "an unknown place".into());
        let backtrace = std::backtrace::Backtrace::force_capture();
        line(&panic_text(gizai_core::ids::now_ms(), &message, &place, &backtrace.to_string()));
        usual(info);
    }));
}

/// Points this process's stdout and stderr (fds 1 and 2) to `log`, straight to the file: no pipe or thread that an
/// abrupt end could cut off. What Rust still holds for stdout goes where it was going first.
#[cfg(unix)]
fn point_std_to(log: &File) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    let _ = std::io::stdout().flush();
    for fd in [libc::STDOUT_FILENO, libc::STDERR_FILENO] {
        // SAFETY: dup2 with the log's open descriptor onto a standard one; the log stays open (`LOG` keeps it).
        if unsafe { libc::dup2(log.as_raw_fd(), fd) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Windows: points the process's standard output and error handles to `log` (`SetStdHandle`), which Rust's own printing
/// and the programs Hersir starts with inherited stdio use from now on. An installed Hersir is a window app without a
/// console, so before this they went nowhere.
#[cfg(windows)]
fn point_std_to(log: &File) -> std::io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Console::{STD_ERROR_HANDLE, STD_OUTPUT_HANDLE, SetStdHandle};
    for which in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        // SAFETY: the log's open handle, which stays open as long as Hersir runs (`LOG` keeps it).
        if unsafe { SetStdHandle(which, log.as_raw_handle()) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn point_std_to(_log: &File) -> std::io::Result<()> {
    Err(std::io::Error::other("not on this system"))
}

/// This system as the first line names it. Linux: uname's name, release and machine, with the distribution, the
/// session (Wayland or X11, the desktop) and WebKitGTK's version. macOS: its version, Darwin's and the machine. Windows:
/// its name and the machine. What the system doesn't say is left out.
pub fn system() -> String {
    #[cfg(target_os = "linux")]
    {
        let mut said = vec![];
        if let Some(name) = fs::read_to_string("/etc/os-release").ok().and_then(|t| os_release_name(&t)) {
            said.push(name);
        }
        let session: Vec<String> = ["XDG_SESSION_TYPE", "XDG_CURRENT_DESKTOP"].iter()
            .filter_map(|v| std::env::var(v).ok()).filter(|v| !v.is_empty()).collect();
        if !session.is_empty() {
            said.push(session.join(", "));
        }
        // SAFETY: plain calls without arguments into WebKitGTK, which Hersir links.
        let (major, minor, micro) = unsafe {
            use webkit2gtk::ffi::{webkit_get_major_version, webkit_get_micro_version, webkit_get_minor_version};
            (webkit_get_major_version(), webkit_get_minor_version(), webkit_get_micro_version())
        };
        said.push(format!("WebKitGTK {major}.{minor}.{micro}"));
        let base = uname().unwrap_or_else(|| format!("Linux {}", std::env::consts::ARCH));
        format!("{base} ({})", said.join("; "))
    }
    #[cfg(target_os = "macos")]
    {
        let version = sysctl_text("kern.osproductversion").map(|v| format!("macOS {v}")).unwrap_or_else(|| "macOS".into());
        match uname() {
            Some(darwin) => format!("{version} ({darwin})"),
            None => format!("{version} {}", std::env::consts::ARCH),
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        format!("{} {}", if cfg!(windows) { "Windows" } else { std::env::consts::OS }, std::env::consts::ARCH)
    }
}

/// /etc/os-release's PRETTY_NAME ("Arch Linux"), else its NAME, without quotes.
pub fn os_release_name(text: &str) -> Option<String> {
    let value = |key: &str| text.lines().find_map(|l| l.strip_prefix(key)?.strip_prefix('=').map(|v| v.trim().trim_matches(['"', '\'']).to_string()));
    value("PRETTY_NAME").or_else(|| value("NAME")).filter(|v| !v.is_empty())
}

/// uname's name, release and machine: "Linux 6.17.8-arch1-1 x86_64", "Darwin 24.1.0 arm64".
#[cfg(unix)]
fn uname() -> Option<String> {
    // SAFETY: uname fills the struct we own; its fields are NUL-terminated strings.
    unsafe {
        let mut u: libc::utsname = std::mem::zeroed();
        if libc::uname(&mut u) != 0 {
            return None;
        }
        let text = |f: &[libc::c_char]| std::ffi::CStr::from_ptr(f.as_ptr()).to_string_lossy().into_owned();
        Some(format!("{} {} {}", text(&u.sysname), text(&u.release), text(&u.machine)))
    }
}

/// macOS: a text sysctl (kern.osproductversion: "15.1").
#[cfg(target_os = "macos")]
fn sysctl_text(name: &str) -> Option<String> {
    let name = std::ffi::CString::new(name).ok()?;
    let mut buf = [0u8; 256];
    let mut size = buf.len();
    // SAFETY: sysctlbyname writes at most `size` bytes into `buf` and sets `size` to how many it wrote.
    let ok = unsafe { libc::sysctlbyname(name.as_ptr(), buf.as_mut_ptr().cast(), &mut size, std::ptr::null_mut(), 0) } == 0;
    if !ok {
        return None;
    }
    let text = std::ffi::CStr::from_bytes_until_nul(&buf[..size.min(buf.len())]).ok()?.to_string_lossy().trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// What Hersir itself uses now, in bytes: on Linux the resident memory of its own process and of the WebKit processes it
/// started (its page, network and GPU processes, children or further down), from /proc; on macOS and Windows its own
/// process's. None when the system doesn't say.
pub fn own_memory() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: a plain call without pointers.
        let page = u64::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) }).ok().filter(|p| *p > 0)?;
        let procs: Vec<Proc> = fs::read_dir("/proc").ok()?.flatten().filter_map(|e| {
            let pid = e.file_name().to_str()?.parse::<u32>().ok()?;
            parse_stat(pid, &fs::read_to_string(e.path().join("stat")).ok()?)
        }).collect();
        Some(own_pages(std::process::id(), &procs)? * page)
    }
    #[cfg(target_os = "macos")]
    {
        // SAFETY: proc_pidinfo writes at most `size` bytes into `info`, which we own, and returns how many it wrote.
        unsafe {
            let mut info: libc::proc_taskinfo = std::mem::zeroed();
            let size = std::mem::size_of::<libc::proc_taskinfo>() as libc::c_int;
            let n = libc::proc_pidinfo(libc::getpid(), libc::PROC_PIDTASKINFO, 0, (&raw mut info).cast(), size);
            (n == size).then_some(info.pti_resident_size)
        }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
        use windows_sys::Win32::System::Threading::GetCurrentProcess;
        let cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let mut counters = PROCESS_MEMORY_COUNTERS { cb, ..Default::default() };
        // SAFETY: a buffer of the size cb gives, for this process's own pseudo handle.
        (unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, cb) } != 0).then_some(counters.WorkingSetSize as u64)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    None
}

/// A process as Linux's /proc/<pid>/stat has it: its pid, its parent, its name and its resident memory in pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proc {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub rss_pages: u64,
}

/// /proc/<pid>/stat's text: the name is between the first '(' and the last ')' (it may hold spaces and brackets), the
/// parent is the 4th field and the resident pages the 24th. None for text that isn't such a line.
pub fn parse_stat(pid: u32, text: &str) -> Option<Proc> {
    let (open, close) = (text.find('(')?, text.rfind(')')?);
    let name = text.get(open + 1..close)?.to_string();
    let fields: Vec<&str> = text.get(close + 1..)?.split_whitespace().collect();
    // fields[0] is the state (field 3)
    Some(Proc { pid, ppid: fields.get(1)?.parse().ok()?, name, rss_pages: fields.get(21)?.parse().ok()? })
}

/// The resident pages of the process `me` and of the WebKit processes below it in `procs` (named WebKit…, its children
/// or further down: a sandbox may start them through bwrap). None when `me` isn't in `procs`.
pub fn own_pages(me: u32, procs: &[Proc]) -> Option<u64> {
    let own = procs.iter().find(|p| p.pid == me)?.rss_pages;
    let parents: std::collections::HashMap<u32, u32> = procs.iter().map(|p| (p.pid, p.ppid)).collect();
    let below_me = |pid: u32| {
        let mut at = pid;
        // at most 32 steps up: /proc can change while it is read
        for _ in 0..32 {
            match parents.get(&at) {
                Some(&parent) if parent == me => return true,
                Some(&parent) if parent > 1 => at = parent,
                _ => return false,
            }
        }
        false
    };
    let webkit: u64 = procs.iter().filter(|p| p.pid != me && p.name.starts_with("WebKit") && below_me(p.pid)).map(|p| p.rss_pages).sum();
    Some(own + webkit)
}

// ---- commands for the page (GA-282) ----

/// Settings' log row and the notice after an unexpected end: the logs folder, this start's log and the previous start
/// that ended unexpectedly, until it is dismissed. File work: off the main thread.
#[tauri::command]
pub async fn get_logs_info(st: tauri::State<'_, AppState>) -> Result<LogsInfo, String> {
    let dir = st.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || info(&dir, current())).await.map_err(|e| e.to_string())
}

/// Opens the logs folder (`folder`) in the system's file manager, made first if it went missing.
#[tauri::command]
pub async fn open_logs_folder(app: AppHandle, st: tauri::State<'_, AppState>) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let dir = folder(&st.data_dir);
    let _ = fs::create_dir_all(&dir);
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| format!("Couldn't open {}: {e}", dir.display()))
}

/// Dismisses the unexpected end (`dismiss`): `get_logs_info` says none from now on, also after a restart.
#[tauri::command]
pub async fn dismiss_unexpected_end(st: tauri::State<'_, AppState>) -> Result<(), String> {
    let dir = st.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || dismiss(&dir)).await.map_err(|e| e.to_string())?
}

/// Whether the self-test panic may run: only in a self-test run (`selftest` is HERSIR_SELFTEST being set).
pub fn panic_command(selftest: bool) -> Result<(), String> {
    if selftest { Ok(()) } else { Err("only in a self-test run".into()) }
}

/// The message of the self-test panic.
pub const SELFTEST_PANIC: &str = "the self-test panic (selftest_panic), on the main thread as asked";

/// The panic hook's self-test (QA): panics on the main thread a moment after this returns, so the log gets a `panic:`
/// line with its place and a backtrace, and Hersir most likely ends there, as after a real crash (`running.json` stays,
/// so the next start reports an unexpected end). Refused outside a self-test run (HERSIR_SELFTEST), as `selftest_paste`.
#[tauri::command]
pub async fn selftest_panic(app: AppHandle) -> Result<(), String> {
    panic_command(gizai_agents::env::var("HERSIR_SELFTEST").is_some())?;
    app.run_on_main_thread(|| panic!("{SELFTEST_PANIC}")).map_err(|e| e.to_string())
}
