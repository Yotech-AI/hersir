//! Updates from GitHub Releases. The app decides when (src-tauri/src/update.rs); this is how.
//! - The release check asks GitHub for the latest release with curl: no login, and nothing is sent about you or your
//!   work. GitHub answers `/releases/latest` with the newest release that is neither a draft nor a pre-release, and the
//!   files attached to it. Only a signed release (one with hersir-X.Y.Z.sig) is offered.
//! - An update gets the release's source: a shallow fetch of its tag into a folder of its own, which is kept so the next
//!   build is quicker. Before anything of the release runs, it checks the release's signature (`check_signature`,
//!   `release_sig`). Then it downloads the release's ready-made build for this system (`get_build`), or builds the
//!   release with its own installer (`install.sh --build-only`) when there is none, the download fails or its checksum
//!   is wrong. The installer installs it (`install.sh --skip-build`), so every release installs the way its own version
//!   needs. On Windows the installer is `install.ps1` (`-BuildOnly`, then `-SkipBuild`), run with Windows PowerShell.
//! - Every command writes to the update's log and never prompts. Stop ends it with everything it started (its process
//!   group; a Job Object on Windows, see `os::Tree`).
use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufReader, ErrorKind, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::connection::{self, Problem};
use crate::os::{self, End, Tree};
use crate::release_sig::{self, SigProblem, Signer};

/// Hersir's own repository: its releases and their source (HERSIR_REPO, or GIZAI_REPO from before 0.17.0, changes it, as
/// it does for install.sh).
pub const REPO: &str = "https://github.com/Yotech-AI/hersir.git";

/// How long getting a release's source may take.
pub const SOURCE_LIMIT: Duration = Duration::from_secs(15 * 60);
/// How long downloading one of a release's files may take: its signature and SHA256SUMS take a second, a ready-made
/// build a minute or a few. A download that gets less than 1 KB a second for two minutes gives up sooner.
pub const DOWNLOAD_LIMIT: Duration = Duration::from_secs(30 * 60);
/// How long a build may take (the first one builds every dependency).
pub const BUILD_LIMIT: Duration = Duration::from_secs(2 * 60 * 60);
/// How long the install may take.
pub const INSTALL_LIMIT: Duration = Duration::from_secs(10 * 60);

/// A release's version, MAJOR.MINOR.PATCH, as in its tag ("v0.1.6") and in Cargo.toml ("0.1.6").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(pub u64, pub u64, pub u64);

impl Version {
    /// "0.1.6" or "v0.1.6". None for anything else ("0.2.0-beta.1", "0.2", "latest").
    pub fn parse(s: &str) -> Option<Version> {
        let s = s.trim();
        let s = s.strip_prefix(['v', 'V']).unwrap_or(s);
        let parts: Vec<&str> = s.split('.').collect();
        let num = |p: &str| (!p.is_empty() && p.len() <= 9 && p.bytes().all(|b| b.is_ascii_digit())).then(|| p.parse::<u64>().ok()).flatten();
        match parts.as_slice() {
            [a, b, c] => Some(Version(num(a)?, num(b)?, num(c)?)),
            _ => None,
        }
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// Whether `version` is newer than `than`. Both are MAJOR.MINOR.PATCH; anything else is never newer.
pub fn is_newer(version: &str, than: &str) -> bool {
    matches!((Version::parse(version), Version::parse(than)), (Some(a), Some(b)) if a > b)
}

/// A release on GitHub.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    /// "0.1.6"
    pub version: String,
    /// The tag it was made from: "v0.1.6".
    pub tag: String,
    /// Its title on GitHub.
    #[serde(default)]
    pub name: Option<String>,
    /// Its page on GitHub, with the notes.
    #[serde(default)]
    pub url: Option<String>,
    /// When it was published, as GitHub says it (ISO 8601).
    #[serde(default)]
    pub published_at: Option<String>,
    /// Its notes (Markdown), cut at 20,000 characters.
    #[serde(default)]
    pub notes: Option<String>,
    /// The files attached to it (GA-253): its ready-made builds, SHA256SUMS and its signature, hersir-X.Y.Z.sig. None: a
    /// check from before GA-253 found it, so whether it is signed isn't known yet.
    #[serde(default)]
    pub assets: Option<Vec<Asset>>,
}

impl Release {
    /// The file named `name` attached to it.
    pub fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.as_deref()?.iter().find(|a| a.name == name)
    }

    /// Whether it is signed: it has hersir-X.Y.Z.sig. An update is offered only for a signed release.
    pub fn signed(&self) -> bool {
        self.asset(&signature_name(&self.version)).is_some()
    }
}

/// A file attached to a release: its name, and where curl downloads it from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub url: String,
}

/// The name of release `version`'s signature: hersir-X.Y.Z.sig.
pub fn signature_name(version: &str) -> String {
    format!("hersir-{version}.sig")
}

/// The release's checksums of its ready-made builds: one line per archive, `<sha256>  <file name>`.
pub const SUMS_NAME: &str = "SHA256SUMS";

/// This system as the release's ready-made builds name it, with its archive's extension: linux-x86_64 and macos-arm64
/// (.tar.gz), windows-x86_64 (.zip). None for a system without them (an Intel Mac, Linux on arm64): an update builds the
/// release there.
pub fn build_system() -> Option<(&'static str, &'static str)> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some(("linux-x86_64", "tar.gz")),
        ("macos", "aarch64") => Some(("macos-arm64", "tar.gz")),
        ("windows", "x86_64") => Some(("windows-x86_64", "zip")),
        _ => None,
    }
}

/// The name of release `version`'s ready-made build for this system, like hersir-X.Y.Z-linux-x86_64.tar.gz. None: there
/// are none for this system (`build_system`).
pub fn build_name(version: &str) -> Option<String> {
    build_system().map(|(system, ext)| format!("hersir-{version}-{system}.{ext}"))
}

/// This system in a word, for a message: "linux-aarch64".
pub fn system_name() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

const NOTES_CHARS: usize = 20_000;

/// GitHub's answer for a release (GET /repos/{owner}/{repo}/releases/latest).
#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

/// A file attached to a release, in GitHub's answer.
#[derive(Deserialize)]
struct GithubAsset {
    #[serde(default)]
    name: String,
    #[serde(default)]
    browser_download_url: String,
    /// "uploaded" once its upload has finished.
    #[serde(default)]
    state: Option<String>,
}

/// Reads GitHub's answer for a release. A draft, a pre-release, or a tag that isn't vMAJOR.MINOR.PATCH is no release to
/// update to. Its files are those whose upload has finished.
pub fn parse_release(json: &str) -> Result<Release, Problem> {
    let r: GithubRelease = serde_json::from_str(json).map_err(|e| Problem::plain(format!("GitHub gave an answer Hersir can't read ({e})")))?;
    if r.draft || r.prerelease {
        let kind = if r.draft { "a draft" } else { "a pre-release" };
        return Err(Problem::plain(format!("The latest release, {}, is {kind}", r.tag_name)));
    }
    let version = Version::parse(&r.tag_name)
        .ok_or_else(|| Problem::plain(format!("The latest release's tag, {}, isn't a version like v1.2.3", r.tag_name)))?;
    let notes = r.body.map(|b| b.trim().to_string()).filter(|b| !b.is_empty()).map(|b| {
        if b.chars().count() > NOTES_CHARS { b.chars().take(NOTES_CHARS).collect::<String>() + "…" } else { b }
    });
    let assets = r.assets.into_iter()
        .filter(|a| !a.name.is_empty() && !a.browser_download_url.is_empty() && a.state.as_deref().is_none_or(|s| s == "uploaded"))
        .map(|a| Asset { name: a.name, url: a.browser_download_url })
        .collect();
    Ok(Release {
        version: version.to_string(),
        tag: r.tag_name.trim().to_string(),
        name: r.name.filter(|n| !n.trim().is_empty()),
        url: r.html_url,
        published_at: r.published_at,
        notes,
        assets: Some(assets),
    })
}

/// Where the release check asks for the latest release of the GitHub repository owner/name.
pub fn latest_release_url(owner: &str, name: &str) -> String {
    format!("https://api.github.com/repos/{owner}/{name}/releases/latest")
}

/// Asks `url` for the latest release with curl, which follows redirects (to https only) and gives up after `limit`.
/// The answer is GitHub's (see `latest_release_url`); an http:// or file:// URL works too (a test's fake release).
/// Ok(None): there is no release yet.
pub fn latest_release(url: &str, user_agent: &str, limit: Duration) -> Result<Option<Release>, Problem> {
    let mut cmd = os::command("curl");
    // -q first: your ~/.curlrc plays no part
    cmd.args(["-q", "--silent", "--show-error", "--location", "--proto", "=https,http,file", "--proto-redir", "=https"])
        .args(["--max-time", &limit.as_secs().max(1).to_string()])
        .args(["--header", "Accept: application/vnd.github+json", "--header", "X-GitHub-Api-Version: 2022-11-28"])
        .args(["--user-agent", user_agent, "--write-out", "\n%{http_code}", "--url", url]);
    let (ok, out, err) = crate::github::run(cmd, None, limit + Duration::from_secs(5)).map_err(|e| match e.kind() {
        ErrorKind::NotFound => Problem::new("curl isn't installed", "Install curl with your package manager: Hersir asks GitHub for new releases with it."),
        ErrorKind::TimedOut => connection::no_answer(limit),
        _ => Problem::plain(format!("can't run curl: {e}")),
    })?;
    if !ok {
        return Err(curl_problem(&err, url, limit));
    }
    // the answer, then the HTTP status curl adds (000 for a file)
    let (body, status) = out.rsplit_once('\n').unwrap_or(("", out.as_str()));
    match status.trim() {
        "200" | "000" => parse_release(body).map(Some),
        "404" => Ok(None),
        "403" | "429" if body.contains("rate limit") => {
            Err(Problem::new("GitHub turned the check away: too many requests from your network this hour", "Try again later."))
        }
        status => Err(Problem::plain(format!("GitHub answered the release check with HTTP {status}"))),
    }
}

/// What curl's error means ("curl: (6) Could not resolve host: api.github.com"), in plain words.
fn curl_problem(said: &str, url: &str, limit: Duration) -> Problem {
    let line = said.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("curl failed");
    let code = line.strip_prefix("curl: (").and_then(|r| r.split(')').next()).and_then(|c| c.parse::<u32>().ok());
    match code {
        Some(28) => connection::no_answer(limit),
        Some(5 | 6 | 7 | 35 | 52 | 55 | 56) => Problem::new("Can't reach GitHub", "Check your internet connection, then try again."),
        Some(37) => Problem::plain(format!("Can't read {url}")),
        _ => Problem::plain(line.trim_start_matches("curl: ").to_string()),
    }
}

/// The prefix the installer installed the Hersir at `exe` into. None for a Hersir that runs from anywhere else (a build
/// in a checkout's target/, a test build): an update never installs there. The layouts:
/// - Linux (install.sh): `<prefix>/lib/hersir/hersir`. An install from before 0.17.0 (GA-248) is updated where it is:
///   `<prefix>/lib/gizai/hersir`, with `<prefix>/lib/gizai/gizai` (a link to it now, the program itself before).
/// - macOS (install.sh): the same, links into `<lib>/Hersir.app`; started from the Dock or Finder Hersir is
///   `<lib>/Hersir.app/Contents/MacOS/hersir` (`gizai` before 0.17.0). `exe` is resolved first (std's `current_exe`
///   doesn't follow links on macOS), and both give `<prefix>`.
/// - Windows (install.ps1): `<prefix>\hersir.exe`, in a folder that holds install.ps1's marker `hersir-installed.txt` (a
///   build in target\release has none). An install from before 0.17.0 has `gizai.exe` and `gizai-installed.txt`
///   there too.
///
/// A package's Hersir can be in such a place too (the AUR's `/usr/lib/hersir/hersir`), so the app asks `package_of`
/// first: only its package manager updates it.
pub fn install_prefix(exe: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let dir = exe.parent()?;
        let name = exe.file_name()?.to_string_lossy().to_ascii_lowercase();
        let marked = ["hersir-installed.txt", "gizai-installed.txt"].iter().any(|m| dir.join(m).is_file());
        (matches!(name.as_str(), "hersir.exe" | "gizai.exe") && marked).then(|| dir.to_path_buf())
    }
    #[cfg(not(windows))]
    {
        #[cfg(target_os = "macos")]
        let exe = &mac_exe(exe);
        let dir = exe.parent()?;
        let lib = dir.parent()?;
        let names = (dir.file_name()?.to_str()?, exe.file_name()?.to_str()?);
        let installed = matches!(names, ("hersir", "hersir") | ("gizai", "hersir") | ("gizai", "gizai")) && lib.file_name()? == "lib";
        installed.then(|| lib.parent().map(Path::to_path_buf)).flatten()
    }
}

/// macOS: `exe` with its links resolved, and Hersir.app's own program (`<dir>/Hersir.app/Contents/MacOS/hersir`, `gizai`
/// before 0.17.0) taken as the `<dir>/hersir` (`<dir>/gizai`) that install.sh links to it.
#[cfg(target_os = "macos")]
fn mac_exe(exe: &Path) -> PathBuf {
    let exe = exe.canonicalize().unwrap_or_else(|_| exe.to_path_buf());
    let name = exe.file_name().filter(|n| *n == "hersir" || *n == "gizai").map(|n| n.to_os_string());
    let in_app = exe.parent().is_some_and(|d| d.ends_with("Hersir.app/Contents/MacOS"));
    let link = exe.ancestors().nth(3).filter(|_| in_app).zip(name).map(|(app, n)| app.with_file_name(n));
    link.unwrap_or(exe)
}

/// The mark a package leaves beside Hersir's program: a file whose first line names the package manager (GA-199). The
/// install scripts never write it.
pub const PACKAGE_MARK: &str = "hersir-package.txt";

/// The package manager this Hersir came from, or None. Reads `exe` with its links resolved (std::fs::canonicalize):
/// the first line of `hersir-package.txt` in its folder, trimmed and lowercased ("aur", "scoop", "homebrew" or another
/// word; an empty line is none), or on macOS "homebrew" when the path runs through `Cellar/hersir/` or
/// `Caskroom/hersir/` (a file can't go inside Hersir.app without breaking its signature).
pub fn package_of(exe: &Path) -> Option<String> {
    let exe = exe.canonicalize().unwrap_or_else(|_| exe.to_path_buf());
    let marked = exe.parent().and_then(|dir| mark_word(&dir.join(PACKAGE_MARK)));
    marked.or_else(|| from_homebrew(&exe).then(|| "homebrew".to_string()))
}

/// The first line of the mark at `path`, trimmed and lowercased. None: there is no mark there, or its first line is
/// empty. Windows PowerShell 5.1 may write it as UTF-16 (`Out-File`, `>`) or with a byte order mark: both read the same.
fn mark_word(path: &Path) -> Option<String> {
    let mut bytes = Vec::new();
    // a mark is a word: of a bigger file only the start is read
    File::open(path).ok()?.take(4096).read_to_end(&mut bytes).ok()?;
    let utf16 = |rest: &[u8], unit: fn([u8; 2]) -> u16| -> String {
        char::decode_utf16(rest.as_chunks().0.iter().map(|&pair| unit(pair))).map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER)).collect()
    };
    let text = match bytes.as_slice() {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, u16::from_le_bytes),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, u16::from_be_bytes),
        [0xEF, 0xBB, 0xBF, rest @ ..] | rest => String::from_utf8_lossy(rest).into_owned(),
    };
    let word = text.lines().next()?.trim().to_lowercase();
    (!word.is_empty()).then_some(word)
}

/// macOS: whether `exe` (resolved) runs from Homebrew's own folders, `…/Cellar/hersir/…` (a formula) or
/// `…/Caskroom/hersir/…` (a cask). On Linux and Windows such a path means nothing.
fn from_homebrew(exe: &Path) -> bool {
    let dirs: Vec<&OsStr> = exe.parent().map(|d| d.iter().collect()).unwrap_or_default();
    cfg!(target_os = "macos") && dirs.windows(2).any(|w| matches!(w[0].to_str(), Some("Cellar" | "Caskroom")) && w[1] == "hersir")
}

/// The command that updates a package's Hersir: "yay -Syu" for "aur", "brew update && brew upgrade hersir" for
/// "homebrew", "scoop update; scoop update hersir" for "scoop", None for any other package.
pub fn package_update_command(package: &str) -> Option<&'static str> {
    match package {
        "aur" => Some("yay -Syu"),
        // brew upgrade refreshes the tap by itself only once a day, so a release of today needs brew update first
        "homebrew" => Some("brew update && brew upgrade hersir"),
        // scoop update <app> refreshes the buckets by itself only when they are three hours old
        "scoop" => Some("scoop update; scoop update hersir"),
        _ => None,
    }
}

/// The program of an install from before Hersir 0.17.0 in `prefix`, when there is one: `<prefix>/lib/gizai/gizai`, on
/// Windows `<prefix>\gizai.exe` beside `gizai-installed.txt`. The installers update such an install where it is, and keep
/// that program there as a link to the new Hersir (on Windows a copy), as 0.16.0's own update checks it (GA-248).
pub fn gizai_install(prefix: &Path) -> Option<PathBuf> {
    if cfg!(windows) {
        let exe = prefix.join("gizai.exe");
        (exe.is_file() && prefix.join("gizai-installed.txt").is_file()).then_some(exe)
    } else {
        let exe = prefix.join("lib/gizai/gizai");
        exe.symlink_metadata().is_ok().then_some(exe)
    }
}

/// The Hersir installed into `prefix`, by the rule the installers follow: in an install from before 0.17.0 its gizai
/// program (`gizai_install`), else `<prefix>/lib/hersir/hersir` (on macOS a link into Hersir.app), on Windows
/// `<prefix>\hersir.exe`.
pub fn installed_exe(prefix: &Path) -> PathBuf {
    gizai_install(prefix).unwrap_or_else(|| if cfg!(windows) { prefix.join("hersir.exe") } else { prefix.join("lib/hersir/hersir") })
}

/// What a restart after an update starts in `prefix`: the first of these that is there. On Linux and macOS the launcher,
/// else the program, in the install's lib folder: `<prefix>/lib/hersir`, or `<prefix>/lib/gizai` for an install from
/// before 0.17.0, where 0.16.0's own `gizai-launch` and `gizai` come last. On Windows `<prefix>\hersir.exe`, else
/// `<prefix>\gizai.exe`.
pub fn restart_programs(prefix: &Path) -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![prefix.join("hersir.exe"), prefix.join("gizai.exe")]
    } else if gizai_install(prefix).is_some() {
        let lib = prefix.join("lib/gizai");
        ["hersir-launch", "hersir", "gizai-launch", "gizai"].iter().map(|n| lib.join(n)).collect()
    } else {
        let lib = prefix.join("lib/hersir");
        ["hersir-launch", "hersir"].iter().map(|n| lib.join(n)).collect()
    }
}

/// The version the Hersir installed into `prefix` says it is: "hersir 0.17.0" from `hersir --version` gives "0.17.0"
/// ("gizai 0.16.0" from a Hersir before 0.17.0 gives "0.16.0").
pub fn installed_version(prefix: &Path) -> Option<String> {
    version_of(&installed_exe(prefix), Duration::from_secs(10)).ok()
}

/// What the program `exe` says its version is: the second word of `exe --version`, a version like 0.17.0. Err: why it
/// says none, in a few words (it doesn't start here, it fails, it says something else).
fn version_of(exe: &Path, limit: Duration) -> Result<String, String> {
    let mut cmd = os::command(exe);
    cmd.arg("--version");
    let (ok, out, err) = crate::github::run(cmd, None, limit).map_err(|e| e.to_string())?;
    match out.split_whitespace().nth(1).filter(|w| Version::parse(w).is_some()) {
        Some(word) if ok => Ok(word.to_string()),
        _ => {
            let said = [err.trim(), out.trim()].into_iter().find(|s| !s.is_empty()).and_then(|s| s.lines().next());
            Err(match said {
                Some(line) if ok => format!("it says {line}"),
                Some(line) => format!("it failed: {line}"),
                None => "it failed and said nothing".into(),
            })
        }
    }
}

/// The version a source says it is: its Cargo.toml's [workspace.package] version.
pub fn source_version(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("Cargo.toml")).ok()?;
    let mut in_package = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_package = line == "[workspace.package]";
            continue;
        }
        if in_package && let Some(rest) = line.strip_prefix("version") && let Some(value) = rest.trim_start().strip_prefix('=') {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// Stops an update's commands: the one that runs now ends with everything it started, and no other one starts.
#[derive(Clone, Default)]
pub struct Stop {
    asked: Arc<AtomicBool>,
    /// The process group (on Windows the job) of the command that runs now (None: none).
    tree: Arc<Mutex<Option<Tree>>>,
}

impl Stop {
    /// SIGTERM to the process group of the command that runs now (SIGKILL follows after 5 seconds). On Windows its job
    /// ends at once (see `os::End`).
    pub fn stop(&self) {
        self.asked.store(true, Ordering::SeqCst);
        if let Some(tree) = self.tree.lock().unwrap_or_else(|p| p.into_inner()).as_ref() {
            tree.end(End::Terminate);
        }
    }

    pub fn asked(&self) -> bool {
        self.asked.load(Ordering::SeqCst)
    }

    /// The command that runs now (None: none).
    fn now(&self, tree: Option<Tree>) {
        *self.tree.lock().unwrap_or_else(|p| p.into_inner()) = tree;
    }
}

/// An update's log: what every command said, and Gizai's own lines between them.
#[derive(Clone)]
pub struct Log(Arc<Mutex<std::fs::File>>);

impl Log {
    /// A new log at `path`; an older one there is replaced.
    pub fn create(path: &Path) -> std::io::Result<Log> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Ok(Log(Arc::new(Mutex::new(std::fs::File::create(path)?))))
    }

    /// One line of Gizai's own.
    pub fn say(&self, line: &str) {
        let _ = writeln!(self.0.lock().unwrap_or_else(|p| p.into_inner()), "{line}");
    }

    fn write(&self, bytes: &[u8]) {
        let _ = self.0.lock().unwrap_or_else(|p| p.into_inner()).write_all(bytes);
    }
}

/// Why a step of an update didn't work, in plain words, with the end of what its command said (all of it is in the
/// log).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UpdateFailed {
    pub what: String,
    pub output: String,
}

impl UpdateFailed {
    pub fn plain(what: impl Into<String>) -> UpdateFailed {
        UpdateFailed { what: what.into(), output: String::new() }
    }
}

impl std::fmt::Display for UpdateFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.what)?;
        if !self.output.is_empty() {
            write!(f, ":\n{}", self.output)?;
        }
        Ok(())
    }
}

/// Gets the source of release `tag` from `repo` into `dir`, a folder of its own (made when missing): a shallow fetch of
/// that tag alone, checked out as it is. Everything else in the folder goes, except the build's target/, so the next
/// build is quicker. git only ever works in `dir`'s own repository, never in one around it.
pub fn get_source(repo: &str, tag: &str, dir: &Path, log: &Log, stop: &Stop) -> Result<(), UpdateFailed> {
    std::fs::create_dir_all(dir).map_err(|e| UpdateFailed::plain(format!("Can't make {}: {e}", dir.display())))?;
    let dir = &dir.canonicalize().map_err(|e| UpdateFailed::plain(format!("Can't find {}: {e}", dir.display())))?;
    if !own_repo(dir) {
        // a .git cut short (Stop, a full disk) is made again
        let dot_git = dir.join(".git");
        let _ = if dot_git.is_dir() { std::fs::remove_dir_all(&dot_git) } else { std::fs::remove_file(&dot_git) };
        run("git init", git(dir, &["init", "--quiet"]), log, SOURCE_LIMIT, stop, false)?;
    }
    let refspec = format!("+refs/tags/{tag}:refs/tags/{tag}");
    run(&format!("git fetch --depth 1 {repo} {refspec}"), git(dir, &["fetch", "--depth", "1", "--no-tags", "--force", "--", repo, &refspec]),
        log, SOURCE_LIMIT, stop, false)
        .map_err(|f| fetch_problem(f, repo, tag))?;
    run("git checkout", git(dir, &["checkout", "--quiet", "--force", "--detach", &format!("refs/tags/{tag}")]), log, SOURCE_LIMIT, stop, false)?;
    run("git clean", git(dir, &["clean", "-ffdxq", "--exclude=/target/"]), log, SOURCE_LIMIT, stop, false)?;
    Ok(())
}

/// git in `dir` that never looks for a repository above it, so a broken one here can't send it into a checkout around
/// the data folder.
fn git(dir: &Path, args: &[&str]) -> Command {
    let mut c = os::command("git");
    c.arg("-C").arg(dir).args(args);
    if let Some(parent) = dir.parent() {
        c.env("GIT_CEILING_DIRECTORIES", parent);
    }
    c
}

/// Whether `dir` (canonical) holds a git repository of its own that works.
fn own_repo(dir: &Path) -> bool {
    let Ok(out) = git(dir, &["rev-parse", "--show-toplevel"]).stdin(Stdio::null()).stderr(Stdio::null()).output() else { return false };
    out.status.success() && Path::new(String::from_utf8_lossy(&out.stdout).trim()).canonicalize().ok().as_deref() == Some(dir)
}

/// What a failed fetch of a release's tag means, in plain words.
fn fetch_problem(f: UpdateFailed, repo: &str, tag: &str) -> UpdateFailed {
    let has = |s: &str| f.output.contains(s);
    let what = if has("couldn't find remote ref") {
        format!("{repo} has no tag {tag}. Check for updates again: the release may have been removed")
    } else if ["Could not resolve host", "Failed to connect", "Couldn't connect to server", "Connection timed out", "Network is unreachable"].iter().any(|s| has(s)) {
        "Can't reach GitHub. Check your internet connection, then try again".to_string()
    } else if has("Repository not found") || has("does not appear to be a git repository") {
        format!("Hersir can't find its repository at {repo}")
    } else {
        return f;
    };
    UpdateFailed { what, output: f.output }
}

/// Downloads `url` into `to` with curl, as one of the update's commands (`run`): in the log, ended by Stop, after
/// DOWNLOAD_LIMIT, or when it stalls. curl follows redirects to https only (GitHub sends a release's files on to its
/// own storage), and takes an http:// or file:// address too (a test's fake release). What was at `to` goes first, and
/// so does what a download that fails leaves, so nothing there passes for the file.
pub fn download(url: &str, to: &Path, log: &Log, stop: &Stop) -> Result<(), UpdateFailed> {
    let name = to.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    remove_path(to).map_err(|e| UpdateFailed::plain(format!("Can't replace {}: {e}", to.display())))?;
    let mut cmd = os::command("curl");
    // -q first: your ~/.curlrc plays no part
    cmd.args(["-q", "--silent", "--show-error", "--fail", "--location", "--proto", "=https,http,file", "--proto-redir", "=https"])
        .args(["--max-time", &DOWNLOAD_LIMIT.as_secs().to_string(), "--speed-limit", "1024", "--speed-time", "120"])
        .arg("--output").arg(to).args(["--url", url]);
    let shown = format!("curl --output {} {url}", to.display());
    run(&shown, cmd, log, DOWNLOAD_LIMIT + Duration::from_secs(30), stop, false).map(|_| ()).map_err(|f| {
        let _ = remove_path(to);
        if stop.asked() {
            return f;
        }
        let why = if f.output.trim().is_empty() { f.what.clone() } else { download_problem(&f.output) };
        UpdateFailed { what: format!("Couldn't download {name}: {why}"), output: f.output }
    })
}

/// What curl said went wrong with a download ("curl: (22) The requested URL returned error: 404"), in plain words.
fn download_problem(said: &str) -> String {
    let line = said.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("curl failed");
    let line = line.strip_prefix("curl: ").unwrap_or(line);
    let (code, text) = match line.strip_prefix('(').and_then(|r| r.split_once(") ")) {
        Some((code, text)) => (code.parse::<u32>().ok(), text),
        None => (None, line),
    };
    match code {
        Some(5 | 6 | 7 | 35 | 52 | 55 | 56) => "Can't reach GitHub".into(),
        Some(28) => "it got no answer, or too little, in time".into(),
        _ => text.to_string(),
    }
}

/// Removes what is at `path`: a file, a link or a whole folder. Nothing there is fine.
fn remove_path(path: &Path) -> std::io::Result<()> {
    match path.symlink_metadata() {
        Ok(m) if m.is_dir() => std::fs::remove_dir_all(path),
        Ok(_) => std::fs::remove_file(path),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// The folder in `dir` (<data folder>/update) a ready-made build is unpacked into before it moves into the source.
const UNPACKED: &str = "unpacked";

/// Removes what an earlier update downloaded into `dir` (<data folder>/update): signatures, SHA256SUMS, ready-made
/// builds, and a build half unpacked. The source and the log stay.
pub fn clear_downloads(dir: &Path) {
    let Ok(items) = std::fs::read_dir(dir) else { return };
    for item in items.flatten() {
        let name = item.file_name().to_string_lossy().to_string();
        let downloaded = name.starts_with("hersir-") && [".sig", ".tar.gz", ".zip"].iter().any(|e| name.ends_with(e));
        if downloaded || name == SUMS_NAME || name == UNPACKED {
            let _ = remove_path(&item.path());
        }
    }
}

/// Checks `release`'s signature before anything of it runs (GA-253): downloads hersir-X.Y.Z.sig and SHA256SUMS into
/// `dir` (<data folder>/update), and checks in Hersir itself (`release_sig`) that one of `signers` signed the message
/// of the release's tag, with the commit it has in `source` as fetched, and that SHA256SUMS (`release_sig::message`).
/// What an earlier update downloaded goes first. Ok: SHA256SUMS, which the signature vouches for. Err: why Hersir
/// doesn't install the release, in one line.
pub fn check_signature(release: &Release, source: &Path, dir: &Path, signers: &[Signer], log: &Log, stop: &Stop) -> Result<Vec<u8>, UpdateFailed> {
    let tag = &release.tag;
    let sig_name = signature_name(&release.version);
    let refuse = |why: String| UpdateFailed::plain(format!("{why}, so Hersir doesn't install it"));
    // the check offers only a signed release, but its files may have changed since
    let sig_asset = release.asset(&sig_name).ok_or_else(|| refuse(format!("{tag} isn't signed: it has no {sig_name}")))?;
    let sums_asset = release.asset(SUMS_NAME).ok_or_else(|| refuse(format!("{tag} has no {SUMS_NAME}, which its signature is over")))?;
    clear_downloads(dir);
    let (sig_file, sums_file) = (dir.join(&sig_name), dir.join(SUMS_NAME));
    download(&sig_asset.url, &sig_file, log, stop)?;
    download(&sums_asset.url, &sums_file, log, stop)?;
    // each is a few hundred bytes: a file far bigger is neither
    let read = |p: &Path| match std::fs::metadata(p) {
        Ok(m) if m.len() > 1 << 20 => Err(refuse(format!("{} is {} bytes, far too big for what it should be", p.display(), m.len()))),
        _ => std::fs::read(p).map_err(|e| UpdateFailed::plain(format!("Can't read {}: {e}", p.display()))),
    };
    let (sig, sums) = (read(&sig_file)?, read(&sums_file)?);
    let commit = tag_commit(source, tag)?;
    log.say(&format!("The tag {tag} is commit {commit}, and the source is at it."));
    match release_sig::verify(&sig, &release_sig::message(tag, &commit, &sums), signers) {
        Ok(signer) => {
            log.say(&format!("The signature checks: {sig_name} is by {} ({}), over {tag} at {commit} and the release's {SUMS_NAME}.",
                             signer.comment, signer.fingerprint()));
            Ok(sums)
        }
        Err(problem) => Err(refuse(match problem {
            SigProblem::Unreadable(e) => format!("{sig_name} isn't a signature Hersir can read ({e})"),
            SigProblem::Namespace(n) => format!("{sig_name} is a signature for {n}, not for Hersir's releases"),
            SigProblem::OtherKey(f) => format!("{sig_name} is signed with a key that isn't one of Hersir's release keys ({f})"),
            SigProblem::NoMatch => format!("The signature of {tag} doesn't check: the release's {SUMS_NAME} or the commit of its tag ({}) isn't what was signed",
                                           &commit[..12]),
        })),
    }
}

/// The commit the tag `tag` points to in `dir`'s repository, as fetched (`refs/tags/<tag>^{commit}`): its 40-character
/// id. The source checked out there must be at that commit.
pub fn tag_commit(dir: &Path, tag: &str) -> Result<String, UpdateFailed> {
    let id = |rev: &str| -> Option<String> {
        let out = git(dir, &["rev-parse", "--verify", "--quiet", rev]).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
        let id = String::from_utf8_lossy(&out.stdout).trim().to_ascii_lowercase();
        (out.status.success() && id.len() == 40 && id.bytes().all(|b| b.is_ascii_hexdigit())).then_some(id)
    };
    let commit = id(&format!("refs/tags/{tag}^{{commit}}"))
        .ok_or_else(|| UpdateFailed::plain(format!("Can't tell which commit the tag {tag} points to, so Hersir doesn't install it")))?;
    if id("HEAD^{commit}").as_deref() != Some(commit.as_str()) {
        return Err(UpdateFailed::plain(format!("The source isn't at the commit of the tag {tag}, so Hersir doesn't install it")));
    }
    Ok(commit)
}

/// Gets release `version`'s ready-made build `asset` (this system's archive) into `source` instead of building it there
/// (GA-253): downloads it into `dir` (<data folder>/update), checks its sha256 against `sums` (the release's SHA256SUMS,
/// which its signature vouches for), unpacks it, and moves its files into the source's target/release, each replacing
/// the one of its name there. Last, the build's own hersir must run on this system and say `version`. Err: why not, in
/// a few words (the update builds the release instead), or "Stopped".
pub fn get_build(asset: &Asset, version: &str, sums: &[u8], source: &Path, dir: &Path, log: &Log, stop: &Stop) -> Result<(), UpdateFailed> {
    let name = &asset.name;
    let expected = checksum_for(&String::from_utf8_lossy(sums), name)
        .ok_or_else(|| UpdateFailed::plain(format!("The release's {SUMS_NAME} has no line for {name}")))?;
    let archive = dir.join(name);
    download(&asset.url, &archive, log, stop)?;
    let got = sha256_file(&archive).map_err(|e| UpdateFailed::plain(format!("Can't read {}: {e}", archive.display())))?;
    if got != expected {
        log.say(&format!("{name}: its sha256 is {got}, but the release's {SUMS_NAME} says {expected}."));
        return Err(UpdateFailed::plain(format!("{name} doesn't match the release's {SUMS_NAME}")));
    }
    log.say(&format!("{name}: its sha256 is {got}, as the release's {SUMS_NAME} says."));
    let staging = dir.join(UNPACKED);
    let unpacked = unpack(&archive, &staging).and_then(|()| move_build(&staging, source));
    let _ = remove_path(&staging);
    unpacked.map_err(|why| UpdateFailed::plain(format!("Couldn't unpack {name}: {why}")))?;
    // its files are in the source now
    let _ = std::fs::remove_file(&archive);
    log.say(&format!("Unpacked {name} into {}.", source.display()));
    let program = if cfg!(windows) { "hersir.exe" } else { "hersir" };
    match version_of(&source.join("target").join("release").join(program), Duration::from_secs(30)) {
        Ok(v) if v == version => {
            log.say(&format!("Its target/release/{program} --version says {v}."));
            Ok(())
        }
        Ok(v) => Err(UpdateFailed::plain(format!("The ready-made build says it is version {v}, not {version}"))),
        Err(why) => Err(UpdateFailed::plain(format!("The ready-made build doesn't run on this system: {why}"))),
    }
}

/// The sha256 of the file at `path`, in lowercase hex, as sha256sum writes it.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    std::io::copy(&mut BufReader::new(File::open(path)?), &mut hash)?;
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// The sha256 `sums` (SHA256SUMS as sha256sum writes it: `<sha256>  <file name>`, or `<sha256> *<file name>`) gives for
/// the file `name`, in lowercase. None: it has no line for it.
pub fn checksum_for(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (sum, file) = line.split_once(' ')?;
        let file = file.strip_prefix(' ').or_else(|| file.strip_prefix('*'))?;
        (file == name && sum.len() == 64 && sum.bytes().all(|b| b.is_ascii_hexdigit())).then(|| sum.to_ascii_lowercase())
    })
}

/// Unpacks a release's ready-made build `archive` (a .zip, else a .tar.gz) into `staging`, made anew. Everything in it
/// must be under target/release, by a plain relative path: files and folders, and in a .tar.gz links that stay inside
/// their folder (as an app bundle may have). Err: why not.
pub fn unpack(archive: &Path, staging: &Path) -> Result<(), String> {
    remove_path(staging).map_err(|e| format!("can't clear {}: {e}", staging.display()))?;
    std::fs::create_dir_all(staging).map_err(|e| format!("can't make {}: {e}", staging.display()))?;
    let file = File::open(archive).map_err(|e| format!("can't read it: {e}"))?;
    if archive.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
        unpack_zip(file, staging)
    } else {
        unpack_tar_gz(file, staging)
    }
}

/// Whether `path`, in a build's archive, is a plain relative path under target/release (or one of those two folders).
fn in_build(path: &Path) -> bool {
    let mut parts = vec![];
    for c in path.components() {
        match c {
            Component::Normal(p) => parts.push(p),
            Component::CurDir => {}
            _ => return false,
        }
    }
    !parts.is_empty() && parts.iter().zip([OsStr::new("target"), OsStr::new("release")]).all(|(p, want)| *p == want)
}

fn unpack_tar_gz(file: File, staging: &Path) -> Result<(), String> {
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(BufReader::new(file)));
    // the files' modes without setuid and the like, and never their owners
    tar.set_preserve_permissions(false);
    tar.set_overwrite(true);
    for entry in tar.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        let kind = entry.header().entry_type();
        if kind.is_pax_global_extensions() {
            continue;
        }
        if !(kind.is_file() || kind.is_dir() || kind.is_symlink()) {
            return Err(format!("{} is neither a file, a folder nor a link", path.display()));
        }
        if !in_build(&path) {
            return Err(format!("{} isn't under target/release", path.display()));
        }
        if kind.is_symlink() {
            let to = entry.link_name().map_err(|e| e.to_string())?.ok_or_else(|| format!("the link {} leads nowhere", path.display()))?;
            if !to.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir)) {
                return Err(format!("the link {} leads out of its folder, to {}", path.display(), to.display()));
            }
        }
        match entry.unpack_in(staging) {
            Ok(true) => {}
            Ok(false) => return Err(format!("{} would land outside its folder", path.display())),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        }
    }
    Ok(())
}

fn unpack_zip(file: File, staging: &Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(BufReader::new(file)).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().map(|n| n.to_string()).unwrap_or_else(|_| format!("its file {}", i + 1));
        let path = f.enclosed_name().filter(|p| in_build(p)).ok_or_else(|| format!("{name} isn't under target/release"))?;
        let to = staging.join(path);
        if f.is_dir() {
            std::fs::create_dir_all(&to).map_err(|e| format!("{name}: {e}"))?;
            continue;
        }
        if f.is_symlink() {
            return Err(format!("{name} is a link"));
        }
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{name}: {e}"))?;
        }
        let mut out = File::create(&to).map_err(|e| format!("{name}: {e}"))?;
        std::io::copy(&mut f, &mut out).map_err(|e| format!("{name}: {e}"))?;
        #[cfg(unix)]
        if let Some(mode) = f.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&to, std::fs::Permissions::from_mode(mode & 0o777)).map_err(|e| format!("{name}: {e}"))?;
        }
    }
    Ok(())
}

/// Moves what `staging` holds in target/release into `source`'s target/release: each file or folder there replaces the
/// one of its name in the source whole, so an app bundle keeps no file of an older build.
fn move_build(staging: &Path, source: &Path) -> Result<(), String> {
    let (from, to) = (staging.join("target").join("release"), source.join("target").join("release"));
    let items: Vec<_> = std::fs::read_dir(&from).map(|d| d.flatten().collect()).unwrap_or_default();
    if items.is_empty() {
        return Err("it holds nothing in target/release".into());
    }
    std::fs::create_dir_all(&to).map_err(|e| format!("can't make {}: {e}", to.display()))?;
    for item in items {
        let dst = to.join(item.file_name());
        remove_path(&dst).map_err(|e| format!("can't replace {}: {e}", dst.display()))?;
        std::fs::rename(item.path(), &dst).map_err(|e| format!("can't move it into {}: {e}", to.display()))?;
    }
    Ok(())
}

/// The release's installer, and its two steps (build, install) as given to it and as the log shows them.
#[cfg(not(windows))]
const INSTALLER: &str = "install.sh";
#[cfg(not(windows))]
const STEPS: [(&str, &str); 2] = [("--build-only", "./install.sh --build-only"), ("--skip-build", "./install.sh --skip-build")];
#[cfg(windows)]
const INSTALLER: &str = "install.ps1";
#[cfg(windows)]
const STEPS: [(&str, &str); 2] = [("-BuildOnly", r".\install.ps1 -BuildOnly"), ("-SkipBuild", r".\install.ps1 -SkipBuild")];

/// Builds the release in `dir` with its own installer, `install.sh --build-only` (`install.ps1 -BuildOnly` on Windows),
/// at low CPU priority so Gizai stays quick meanwhile. `path` is the PATH the build gets (None: Gizai's own).
pub fn build(dir: &Path, path: Option<&OsStr>, log: &Log, stop: &Stop) -> Result<(), UpdateFailed> {
    let mut cmd = installer(dir, path)?;
    let (arg, shown) = STEPS[0];
    cmd.arg(arg);
    run(shown, cmd, log, BUILD_LIMIT, stop, true).map(|_| ())
}

/// Installs what `build` made into `prefix` with the release's installer, `install.sh --skip-build`. Its desktop entry
/// and icons go under the prefix too (`<prefix>/share`: ~/.local/share for the usual install), so an install anywhere
/// else, a test's scratch folder, never changes the launcher entry you use. The installer backs up the data folder
/// there (`<prefix>/share/hersir`, or `<prefix>/share/gizai` from before 0.17.0 when it holds gizai.db and there is no
/// hersir one: the rule Hersir's own data folder follows) with the new build first, and installs nothing when it can't.
/// On Windows `install.ps1 -SkipBuild` gets the prefix only: XDG_DATA_HOME means nothing there, and the data folder
/// install.ps1 backs up is `%APPDATA%\Hersir`, or `%APPDATA%\Gizai` by the same rule.
/// The prefix goes under both its names, HERSIR_PREFIX and GIZAI_PREFIX (its name before 0.17.0), as Hersir sets each
/// of its variables for other programs (GA-248).
pub fn install(dir: &Path, prefix: &Path, path: Option<&OsStr>, log: &Log, stop: &Stop) -> Result<(), UpdateFailed> {
    let mut cmd = installer(dir, path)?;
    let (arg, shown) = STEPS[1];
    cmd.arg(arg).env("HERSIR_PREFIX", prefix).env("GIZAI_PREFIX", prefix);
    #[cfg(not(windows))]
    cmd.env("XDG_DATA_HOME", prefix.join("share"));
    run(shown, cmd, log, INSTALL_LIMIT, stop, false).map(|_| ())
}

/// The release's installer, run in its source, building into the source's own target/: install.sh with bash, on
/// Windows install.ps1 with Windows PowerShell (`-ExecutionPolicy Bypass`, as the release's own script isn't signed;
/// `-NonInteractive`, so a question fails instead of waiting). It inherits none of the installer's own variables, by
/// either name (HERSIR_ and, from before 0.17.0, GIZAI_): `install` gives it the prefix.
fn installer(dir: &Path, path: Option<&OsStr>) -> Result<Command, UpdateFailed> {
    let script = dir.join(INSTALLER);
    if !script.is_file() {
        return Err(UpdateFailed::plain(format!("The release has no {INSTALLER}, so Hersir can't build it")));
    }
    #[cfg(not(windows))]
    let mut cmd = os::command("bash");
    #[cfg(windows)]
    let mut cmd = os::command("powershell.exe");
    #[cfg(windows)]
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"]);
    cmd.arg(script).current_dir(dir)
        .env("CARGO_TARGET_DIR", dir.join("target")).env("TAURI_TELEMETRY_DISABLED", "1")
        .env_remove("HERSIR_REPO").env_remove("HERSIR_BRANCH").env_remove("HERSIR_PREFIX").env_remove("HERSIR_DATA_DIR")
        .env_remove("GIZAI_REPO").env_remove("GIZAI_BRANCH").env_remove("GIZAI_PREFIX").env_remove("GIZAI_DATA_DIR");
    if let Some(p) = path {
        cmd.env("PATH", p);
    }
    Ok(cmd)
}

/// Runs `cmd` (`shown` in the log) in its own process group (a Job Object on Windows) without prompts, its output going
/// to the log. It is ended, with everything it started, after `limit` or when `stop` is asked. `low`: at low CPU
/// priority (nice 10; below normal on Windows). Ok: the end of its output.
fn run(shown: &str, mut cmd: Command, log: &Log, limit: Duration, stop: &Stop, low: bool) -> Result<String, UpdateFailed> {
    if stop.asked() {
        return Err(UpdateFailed::plain("Stopped"));
    }
    log.say(&format!("$ {shown}"));
    let program = cmd.get_program().to_string_lossy().to_string();
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0").env("NO_COLOR", "1").env("CARGO_TERM_COLOR", "never")
        .env("npm_config_color", "false").env("npm_config_update_notifier", "false").env("npm_config_fund", "false").env("npm_config_audit", "false");
    let (mut child, tree) = os::spawn_tree(&mut cmd, low).map_err(|e| match e.kind() {
        ErrorKind::NotFound => UpdateFailed::plain(format!("{program} isn't installed (or not on Hersir's PATH)")),
        _ => UpdateFailed::plain(format!("Couldn't start {shown}: {e}")),
    })?;
    stop.now(Some(tree.clone()));
    let output = Arc::new(Mutex::new(Vec::<u8>::new()));
    let pipes: Vec<Box<dyn Read + Send>> = [
        child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>),
        child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>),
    ].into_iter().flatten().collect();
    let readers: Vec<_> = pipes.into_iter().map(|mut pipe| {
        let (output, log) = (output.clone(), log.clone());
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            while let Ok(n) = pipe.read(&mut buf) {
                if n == 0 {
                    break;
                }
                log.write(&buf[..n]);
                let mut o = output.lock().unwrap_or_else(|p| p.into_inner());
                o.extend_from_slice(&buf[..n]);
                if o.len() > 256 * 1024 {
                    let cut = o.len() - 64 * 1024;
                    o.drain(..cut);
                }
            }
        })
    }).collect();
    let until = Instant::now() + limit;
    let (mut ending, mut timed_out) = (None::<Instant>, false);
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) => {}
            Err(_) => break None,
        }
        if ending.is_none() && (stop.asked() || Instant::now() >= until) {
            timed_out = !stop.asked();
            tree.end(End::Terminate);
            ending = Some(Instant::now());
        }
        if ending.is_some_and(|at| at.elapsed() > Duration::from_secs(5)) {
            tree.end(End::Kill);
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    stop.now(None);
    // what it left running in the background (a compiler cache's server) would hold its output open
    tree.end(End::Terminate);
    let quiet_by = Instant::now() + Duration::from_secs(3);
    while readers.iter().any(|r| !r.is_finished()) && Instant::now() < quiet_by {
        std::thread::sleep(Duration::from_millis(50));
    }
    if readers.iter().any(|r| !r.is_finished()) {
        tree.end(End::Kill);
        let quiet_by = Instant::now() + Duration::from_secs(2);
        while readers.iter().any(|r| !r.is_finished()) && Instant::now() < quiet_by {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    // a reader still busy belongs to something that left the group and keeps the pipe open: it is left behind
    for r in readers.into_iter().filter(|r| r.is_finished()) {
        let _ = r.join();
    }
    let text = crate::prepare::tail(&String::from_utf8_lossy(&output.lock().unwrap_or_else(|p| p.into_inner())));
    let failed = |what: String| UpdateFailed { what, output: text.clone() };
    match status {
        _ if ending.is_some() && !timed_out => Err(failed("Stopped".into())),
        _ if timed_out => Err(failed(format!("{shown} was stopped after {} minutes", limit.as_secs() / 60))),
        Some(s) if s.success() => Ok(text),
        Some(s) => Err(failed(match s.code() {
            Some(code) => format!("{shown} failed (exit code {code})"),
            None => format!("{shown} was ended by a signal"),
        })),
        None => Err(failed(format!("Hersir lost track of {shown}"))),
    }
}
