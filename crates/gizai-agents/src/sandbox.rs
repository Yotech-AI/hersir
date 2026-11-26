//! GA-265: Hersir's sandbox for a task run. Hersir starts the run's CLI (Claude Code, Codex, Gemini or any other) inside
//! it, and so everything the CLI starts: its tools, a project's scripts and tests, a `node -e`. Whatever sandbox the CLI
//! has of its own runs inside Hersir's (Codex's `workspace-write` still works there).
//!
//! What a run may touch (`policy`):
//! - **read and change:** the worktree, the agent's change folders, the run's temp folder, the parts of the
//!   repository's git folder a commit writes (`worktree::commit_dirs`), its own CLI's folders (`cli_places`: its sign-in
//!   and sessions) but for its settings (`cli_settings`), and the package managers' download caches (`CACHES`);
//! - **read only:** everything else, but for what is hidden; the run's own CLI's settings, instructions, hooks, plugins
//!   and MCP servers among it, so the run can't leave a program there for your own sessions of that CLI;
//! - **hidden** (an empty folder or file in its place): Hersir's data folder (its database, the runs' logs and MCP
//!   configs, the other cards' worktrees), the other runs' temp folders, your keys and logins (`SECRET_PLACES`: `~/.ssh`,
//!   `~/.gnupg`, `~/.aws`, `~/.config/gh`, …), the other CLIs' folders, and on Linux your session's sockets
//!   (`$XDG_RUNTIME_DIR`: your systemd, D-Bus and keyring, which would run or hand out anything outside the sandbox), the
//!   system's D-Bus and Docker's and Podman's sockets. Of these, the run's own MCP config, Gemini settings file and
//!   Hersir's socket come back read only (`RunPlaces::read`).
//! - The network stays as it is.
//!
//! Each system's engine (`Engine`):
//! - **Linux:** bubblewrap (`bwrap`, `bwrap_args`): the run sees the computer read only, in a mount namespace and a PID
//!   namespace of its own, so it sees no other process (and no other process's files through `/proc`). `/tmp` is the
//!   run's temp folder. It needs the `bubblewrap` package and user namespaces (`check`).
//! - **macOS:** Seatbelt (`/usr/bin/sandbox-exec`, `seatbelt_profile`), part of macOS.
//! - **Windows:** none (`check` gives None): the run says so.
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::cli::Kind;

/// What a sandboxed run may touch, as absolute paths (`policy`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Policy {
    /// Where the CLI starts: the worktree.
    pub cwd: PathBuf,
    /// Read and change.
    pub write: Vec<PathBuf>,
    /// Read only, also inside a hidden place.
    pub read: Vec<PathBuf>,
    /// Neither read nor changed: an empty folder, or an empty file, in its place.
    pub hide: Vec<PathBuf>,
    /// Linux: `/tmp` is this folder (the run's temp folder); None: a private empty one.
    pub tmp: Option<PathBuf>,
}

/// What a run's policy is made of (`policy`).
#[derive(Debug, Clone)]
pub struct RunPlaces {
    pub kind: Kind,
    /// The CLI's program: for an Other CLI its name says which CLI it is (`cli_places`).
    pub bin: PathBuf,
    /// The CLI's lines in Settings → Coding CLIs (another account's CLAUDE_CONFIG_DIR, say).
    pub cli_env: Vec<(String, String)>,
    pub worktree: PathBuf,
    /// Read and change besides the worktree: the agent's change folders, the run's temp folder and what a commit writes
    /// in the repository's git folder.
    pub write: Vec<PathBuf>,
    /// Read only: the agent's read folders, and from Hersir's data folder the run's own MCP config, Gemini settings file
    /// and Hersir's socket.
    pub read: Vec<PathBuf>,
    pub temp_dir: Option<PathBuf>,
    /// Hersir's data folder, hidden.
    pub data_dir: PathBuf,
    /// Where every run's temp folder is (`worktree::temp_base`), hidden but for this run's own.
    pub temp_base: PathBuf,
    /// Your home folder; None: none of the places in it are known.
    pub home: Option<PathBuf>,
}

/// Your keys and logins in your home folder, which no run reads: SSH, GnuPG, the clouds' CLIs, Docker's and
/// Kubernetes' logins, the GitHub CLI's, git's stored passwords, password stores and keyrings, and the browsers' profiles
/// (their cookies are logins too). Not macOS's `~/Library/Keychains`: a program reads its keychain file itself, and
/// Claude Code signs in from the login keychain there (its items keep their own access rules).
pub const SECRET_PLACES: [&str; 26] = [
    ".ssh", ".gnupg", ".aws", ".azure", ".kube", ".docker", ".password-store", ".pki", ".netrc", ".git-credentials",
    ".config/gh", ".config/hub", ".config/gcloud", ".config/git/credentials", ".local/share/keyrings", ".cargo/credentials",
    ".cargo/credentials.toml", ".pypirc", ".mozilla", ".config/google-chrome", ".config/chromium", ".config/BraveSoftware",
    ".config/microsoft-edge", ".config/vivaldi", "Library/Cookies", "Library/Application Support/Google/Chrome",
];

/// The package managers' download caches in your home folder, which builds write: npm's, Cargo's registry and git
/// checkouts, the XDG cache (Composer, pip, Playwright's browsers, …), pnpm's store, Yarn's, Bun's, Go's modules, Gradle's
/// and Maven's.
pub const CACHES: [&str; 11] = [
    ".npm", ".cargo/registry", ".cargo/git", ".cache", ".local/share/pnpm", ".yarn", ".bun/install/cache", "go/pkg/mod", ".gradle",
    ".m2/repository", "Library/Caches",
];

/// The coding CLIs Hersir knows the folders of: by `cli::Kind` key or program name, their sign-in, settings and sessions
/// in your home folder. A run gets its own CLI's (`cli_places`) and none of the others'.
const CLI_PLACES: [(&str, &[&str]); 4] = [
    ("claude_code", &[".claude", ".claude.json", ".claude.json.backup"]),
    ("codex", &[".codex"]),
    ("gemini", &[".gemini"]),
    ("opencode", &[".config/opencode", ".local/share/opencode", ".local/state/opencode", ".cache/opencode"]),
];

/// Which of `CLI_PLACES` a run of `kind` is: an Other CLI by its program's name (`opencode`).
fn cli_key(kind: Kind, bin: &Path) -> Option<&'static str> {
    match kind {
        Kind::Other => {
            let name = bin.file_stem()?.to_string_lossy().to_ascii_lowercase();
            CLI_PLACES.iter().map(|(k, _)| *k).find(|k| *k == name)
        }
        k => CLI_PLACES.iter().map(|(key, _)| *key).find(|key| *key == k.key()),
    }
}

/// The value of `name` for the CLI: its own lines in Settings → Coding CLIs, else Hersir's environment.
fn cli_var(cli_env: &[(String, String)], name: &str) -> Option<PathBuf> {
    cli_env.iter().rev().find(|(k, _)| k == name).map(|(_, v)| OsString::from(v))
        .or_else(|| std::env::var_os(name)).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// The run's own CLI's places in `home` (`CLI_PLACES`), and the folder its CLAUDE_CONFIG_DIR or CODEX_HOME names (another
/// account's, say), read and change.
pub fn cli_places(home: &Path, kind: Kind, bin: &Path, cli_env: &[(String, String)]) -> Vec<PathBuf> {
    let Some(key) = cli_key(kind, bin) else { return vec![] };
    let mut own: Vec<PathBuf> = CLI_PLACES.iter().filter(|(k, _)| *k == key).flat_map(|(_, p)| p.iter().map(|p| home.join(p))).collect();
    let var = match kind {
        Kind::ClaudeCode => Some("CLAUDE_CONFIG_DIR"),
        Kind::Codex => Some("CODEX_HOME"),
        _ => None,
    };
    own.extend(var.and_then(|v| cli_var(cli_env, v)));
    own
}

/// What stays read only in a CLI's own folders (`cli_settings`), by `CLI_PLACES` key: the settings, instructions, hooks,
/// plugins, skills and MCP servers your own sessions of that CLI load, so a run can't leave a program there that would
/// start outside the sandbox the next time you use the CLI. Relative to your home folder; `OWN_DIR_SETTINGS` has the same
/// inside a CLAUDE_CONFIG_DIR or CODEX_HOME. Claude Code saves `~/.claude.json` through a new file next to it, which the
/// read-only home folder refuses anyway: a run doesn't change it (checked with Claude Code 2.1.325).
const CLI_SETTINGS: [(&str, &[&str]); 4] = [
    ("claude_code", &[".claude.json", ".claude.json.backup", ".claude/settings.json", ".claude/settings.local.json", ".claude/CLAUDE.md",
                      ".claude/commands", ".claude/agents", ".claude/skills", ".claude/hooks", ".claude/plugins", ".claude/output-styles"]),
    ("codex", &[".codex/config.toml", ".codex/AGENTS.md", ".codex/rules", ".codex/prompts", ".codex/skills"]),
    ("gemini", &[".gemini/settings.json", ".gemini/GEMINI.md", ".gemini/extensions", ".gemini/commands", ".gemini/policies"]),
    ("opencode", &[".config/opencode"]),
];

/// `CLI_SETTINGS` inside the folder a CLAUDE_CONFIG_DIR or CODEX_HOME names.
const OWN_DIR_SETTINGS: [(&str, &[&str]); 2] = [
    ("claude_code", &[".claude.json", ".claude.json.backup", "settings.json", "settings.local.json", "CLAUDE.md", "commands", "agents", "skills",
                      "hooks", "plugins", "output-styles"]),
    ("codex", &["config.toml", "AGENTS.md", "rules", "prompts", "skills"]),
];

/// The read-only part of the run's own CLI's folders (`CLI_SETTINGS`), in `home` and in its CLAUDE_CONFIG_DIR or
/// CODEX_HOME. A file that isn't there yet can't be kept read only: the run could make it.
pub fn cli_settings(home: &Path, kind: Kind, bin: &Path, cli_env: &[(String, String)]) -> Vec<PathBuf> {
    let Some(key) = cli_key(kind, bin) else { return vec![] };
    let mut ro: Vec<PathBuf> = CLI_SETTINGS.iter().filter(|(k, _)| *k == key).flat_map(|(_, p)| p.iter().map(|p| home.join(p))).collect();
    let var = match kind {
        Kind::ClaudeCode => Some("CLAUDE_CONFIG_DIR"),
        Kind::Codex => Some("CODEX_HOME"),
        _ => None,
    };
    if let Some(dir) = var.and_then(|v| cli_var(cli_env, v)) {
        ro.extend(OWN_DIR_SETTINGS.iter().filter(|(k, _)| *k == key).flat_map(|(_, p)| p.iter().map(|p| dir.join(p))));
    }
    ro
}

/// GA-265: what a sandboxed run gets in its environment, for programs that would otherwise write where the sandbox
/// refuses, each in the run's temp folder `temp_dir`:
/// - `BREAKPAD_DUMP_LOCATION`: Chrome's and Chromium's crash reports. Their own place is in your config folder, which is
///   read only, and without it they end at their start (Chromium 143, Chrome DevTools MCP's browser);
/// - `UV_TOOL_DIR` and `UV_TOOL_BIN_DIR`: uv's tools (`uvx`, `uv tool install`), so a tool a run installs is never one
///   you start outside it. Their downloads stay in uv's cache (`~/.cache/uv`).
pub fn run_env(temp_dir: &Path) -> Vec<(String, String)> {
    [("BREAKPAD_DUMP_LOCATION", "crash-reports"), ("UV_TOOL_DIR", "uv-tools"), ("UV_TOOL_BIN_DIR", "uv-bin")]
        .map(|(k, d)| (k.to_string(), temp_dir.join(d).display().to_string())).to_vec()
}

/// The other CLIs' places in `home`: every one of `CLI_PLACES` but the run's own.
pub fn other_cli_places(home: &Path, kind: Kind, bin: &Path) -> Vec<PathBuf> {
    let own = cli_key(kind, bin);
    CLI_PLACES.iter().filter(|(k, _)| Some(*k) != own).flat_map(|(_, p)| p.iter().map(|p| home.join(p))).collect()
}

/// The places on this system no run reaches besides those in your home folder: on Linux your session's sockets
/// (`$XDG_RUNTIME_DIR`, else /run/user/<uid>), the system's D-Bus, and Docker's and Podman's sockets, any of which would
/// run a program outside the sandbox or hand out a secret.
fn system_places() -> Vec<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: getuid has no preconditions.
        let uid = unsafe { libc::getuid() };
        let runtime = std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty()).map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(format!("/run/user/{uid}")));
        [runtime.as_path(), Path::new("/run/dbus"), Path::new("/run/docker.sock"), Path::new("/var/run/docker.sock"), Path::new("/run/podman"),
         Path::new("/run/containerd")].into_iter().map(Path::to_path_buf).collect()
    }
    #[cfg(not(target_os = "linux"))]
    {
        vec![]
    }
}

/// `p` as it is on disk (links followed), when it is there.
fn real(p: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(p).ok()
}

/// Paths without doubles, in their order.
fn unique(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = vec![];
    for p in paths {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// A run's policy: see the module's doc. Only places that are there go in, as they are on disk (links followed), so a
/// link to a hidden folder can't lead around it.
pub fn policy(p: &RunPlaces) -> Policy {
    let mut write: Vec<PathBuf> = vec![p.worktree.clone()];
    write.extend(p.write.iter().cloned());
    write.extend(p.temp_dir.iter().cloned());
    let mut hide: Vec<PathBuf> = vec![p.data_dir.clone(), p.temp_base.clone()];
    let mut settings: Vec<PathBuf> = vec![];
    if let Some(home) = &p.home {
        write.extend(cli_places(home, p.kind, &p.bin, &p.cli_env));
        write.extend(CACHES.iter().map(|c| home.join(c)));
        settings.extend(cli_settings(home, p.kind, &p.bin, &p.cli_env));
        hide.extend(SECRET_PLACES.iter().map(|s| home.join(s)));
        hide.extend(other_cli_places(home, p.kind, &p.bin));
    }
    write.extend(user_temp_dirs());
    hide.extend(system_places());
    // the own CLI's settings stay read only, also where they are a place of their own (`~/.claude.json`)
    let settings = unique(settings.iter().filter_map(|s| real(s)));
    let write = unique(write.iter().filter_map(|w| real(w)).filter(|w| !settings.contains(w)));
    let read = unique(p.read.iter().filter_map(|r| real(r)).filter(|r| !write.contains(r)).chain(settings));
    // what the run reads or changes is never hidden as well
    let hide = unique(hide.iter().filter_map(|h| real(h)).filter(|h| !write.contains(h) && !read.contains(h)));
    Policy { cwd: real(&p.worktree).unwrap_or_else(|| p.worktree.clone()), write, read, hide, tmp: p.temp_dir.as_deref().and_then(real) }
}

/// macOS: your own temp and cache folders under /var/folders, which Apple's compilers and tools write (clang's module
/// cache, xcrun's): read and change. None elsewhere.
fn user_temp_dirs() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        [libc::_CS_DARWIN_USER_TEMP_DIR, libc::_CS_DARWIN_USER_CACHE_DIR].into_iter().filter_map(|name| {
            let mut buf = vec![0u8; 1024];
            // SAFETY: confstr writes at most the buffer's length, a C string.
            let n = unsafe { libc::confstr(name, buf.as_mut_ptr().cast(), buf.len()) };
            (n > 1 && n <= buf.len()).then(|| PathBuf::from(String::from_utf8_lossy(&buf[..n - 1]).to_string()))
        }).collect()
    }
    #[cfg(not(target_os = "macos"))]
    {
        vec![]
    }
}

/// What a policy does to one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Hide,
    Write,
    Read,
}

/// The policy's places, the shallow ones first: a deeper place decides for what is in it (the worktree in Hersir's
/// hidden data folder, a hidden `~/.cache/opencode` in the writable `~/.cache`). For the same place, hiding goes first.
fn steps(policy: &Policy) -> Vec<(Step, &Path)> {
    let mut all: Vec<(Step, &Path)> = policy.hide.iter().map(|p| (Step::Hide, p.as_path()))
        .chain(policy.write.iter().map(|p| (Step::Write, p.as_path())))
        .chain(policy.read.iter().map(|p| (Step::Read, p.as_path())))
        .collect();
    all.sort_by_key(|(step, p)| (p.components().count(), *step != Step::Hide));
    all
}

/// Linux: bubblewrap's arguments for `policy`, up to and with the `--` before the program:
/// - its own PID and IPC namespaces (`--unshare-pid`, `--unshare-ipc`), a `/proc` of its own and a `/dev` of its own;
/// - the whole computer read only (`--ro-bind / /`), `/tmp` the run's temp folder (else an empty one);
/// - then each place, the shallow ones first (`steps`): a hidden folder an empty one (made read only at the end), a
///   hidden file the empty `/dev/null`, read and change `--bind`, read only `--ro-bind`;
/// - and it starts in the worktree.
///
/// No `--new-session` and no `--die-with-parent`: the run stays in the process group Hersir started it in, so Stop's
/// SIGINT reaches the CLI as before. When the CLI ends, the PID namespace ends with whatever it left running.
pub fn bwrap_args(policy: &Policy) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["--unshare-pid", "--unshare-ipc", "--ro-bind", "/", "/", "--dev", "/dev", "--proc", "/proc"]
        .iter().map(OsString::from).collect();
    match &policy.tmp {
        Some(t) => a.extend([OsString::from("--bind"), t.into(), "/tmp".into()]),
        None => a.extend(["--tmpfs".into(), "/tmp".into()]),
    }
    let mut empty_dirs: Vec<&Path> = vec![];
    for (step, p) in steps(policy) {
        match step {
            Step::Hide if p.is_dir() => {
                a.extend(["--tmpfs".into(), p.into()]);
                empty_dirs.push(p);
            }
            Step::Hide => a.extend(["--ro-bind".into(), "/dev/null".into(), p.into()]),
            Step::Write => a.extend(["--bind".into(), p.into(), p.into()]),
            Step::Read => a.extend(["--ro-bind".into(), p.into(), p.into()]),
        }
    }
    for p in empty_dirs {
        a.extend(["--remount-ro".into(), p.into()]);
    }
    a.extend(["--chdir".into(), policy.cwd.clone().into(), "--".into()]);
    a
}

/// A string in a Seatbelt profile: in double quotes, with `\` and `"` escaped.
fn sb_str(p: &Path) -> String {
    format!("\"{}\"", p.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\""))
}

/// macOS: the Seatbelt profile for `policy` (for `sandbox-exec -p`). Everything is allowed but writing, and then each
/// place, the shallow ones first (`steps`; in a profile the last rule that matches wins): a hidden place can't be read or
/// changed, a read and change one can be both, a read one read. Writing the terminal and `/dev/null` stays allowed. A
/// run can't send Apple Events (`osascript` would make another app run anything) or have Launch Services open an app.
pub fn seatbelt_profile(policy: &Policy) -> String {
    let mut p = String::from("(version 1)\n(allow default)\n(deny file-write*)\n");
    p.push_str("(allow file-write* (literal \"/dev/null\") (literal \"/dev/zero\") (literal \"/dev/dtracehelper\") (regex #\"^/dev/tty\") \
                (regex #\"^/dev/fd/\"))\n");
    for (step, path) in steps(policy) {
        let place = if path.is_dir() { format!("(subpath {})", sb_str(path)) } else { format!("(literal {})", sb_str(path)) };
        let rule = match step {
            Step::Hide => "deny file-read* file-write*",
            Step::Write => "allow file-read* file-write*",
            Step::Read => "allow file-read*",
        };
        p.push_str(&format!("({rule} {place})\n"));
    }
    p.push_str("(deny mach-lookup (global-name \"com.apple.coreservices.appleevents\") (global-name \"com.apple.coreservices.launchservicesd\"))\n");
    p
}

/// The sandbox on this system, ready to start a run in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Engine {
    /// Linux: bubblewrap, at this path.
    Bubblewrap(PathBuf),
    /// macOS: Seatbelt, through `/usr/bin/sandbox-exec`.
    Seatbelt,
}

/// What a run starts instead of its CLI: the sandbox's program, and its arguments up to the CLI's program (which comes
/// next, then the CLI's own arguments).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wrap {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

impl Engine {
    /// The program and arguments that start a CLI inside this sandbox with `policy`.
    pub fn wrap(&self, policy: &Policy) -> Wrap {
        match self {
            Engine::Bubblewrap(bwrap) => Wrap { program: bwrap.clone(), args: bwrap_args(policy) },
            Engine::Seatbelt => Wrap {
                program: PathBuf::from(SANDBOX_EXEC),
                args: vec!["-p".into(), seatbelt_profile(policy).into(), "--".into()],
            },
        }
    }

    /// Its name as the docs and the run's notes give it.
    pub fn name(&self) -> &'static str {
        match self {
            Engine::Bubblewrap(_) => "bubblewrap",
            Engine::Seatbelt => "Seatbelt",
        }
    }
}

/// macOS's Seatbelt command.
const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

/// What a run says where Hersir has no sandbox: on Windows (WSL2 is out of scope), and on any system but Linux and macOS.
#[cfg(windows)]
pub const NO_SANDBOX: &str = "No sandbox on Windows: this run can read and change whatever your account can.";
#[cfg(not(windows))]
pub const NO_SANDBOX: &str = "No sandbox on this system: this run can read and change whatever your account can.";

/// What a run says when Settings → Coding CLIs → Run agents without a sandbox on this computer is on.
pub const RAN_WITHOUT: &str = "No sandbox: Settings → Coding CLIs → Run agents without a sandbox on this computer is on, so this run \
                               can read and change whatever your account can.";

/// The sandbox of this system, checked by starting `true` in it: Some engine on Linux and macOS when it works, None on
/// Windows (there is none), and Err with a sentence that says what to install or fix on this system when it can't
/// start (Hersir then doesn't start the run).
pub fn check() -> Result<Option<Engine>, String> {
    #[cfg(target_os = "linux")]
    {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let Some(bwrap) = crate::os::find_in("bwrap", &path).or_else(|| Some(PathBuf::from("/usr/bin/bwrap")).filter(|p| crate::os::executable(p)))
        else {
            return Err(missing_bubblewrap(&os_release()));
        };
        let out = crate::os::command(&bwrap)
            .args(["--unshare-pid", "--unshare-ipc", "--ro-bind", "/", "/", "--dev", "/dev", "--proc", "/proc", "--", "true"])
            .stdin(std::process::Stdio::null()).output();
        match out {
            Ok(o) if o.status.success() => Ok(Some(Engine::Bubblewrap(bwrap))),
            Ok(o) => Err(bubblewrap_fails(String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("").trim())),
            Err(e) => Err(bubblewrap_fails(&e.to_string())),
        }
    }
    #[cfg(target_os = "macos")]
    {
        let out = crate::os::command(SANDBOX_EXEC).args(["-p", "(version 1)(allow default)", "--", "/usr/bin/true"])
            .stdin(std::process::Stdio::null()).output();
        match out {
            Ok(o) if o.status.success() => Ok(Some(Engine::Seatbelt)),
            Ok(o) => Err(format!("Hersir's sandbox (macOS's sandbox-exec) can't start: {}. {SWITCH}",
                                 String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("").trim())),
            Err(e) => Err(format!("Hersir's sandbox (macOS's {SANDBOX_EXEC}) can't start: {e}. {SWITCH}")),
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Ok(None)
    }
}

/// The way out of a sandbox that can't start, after what to fix.
pub const SWITCH: &str = "Or switch on Settings → Coding CLIs → Run agents without a sandbox on this computer.";

/// Linux: /etc/os-release's text, empty when it can't be read.
#[cfg(target_os = "linux")]
fn os_release() -> String {
    std::fs::read_to_string("/etc/os-release").or_else(|_| std::fs::read_to_string("/usr/lib/os-release")).unwrap_or_default()
}

/// The command that installs bubblewrap on the Linux system `os_release` (/etc/os-release's text) describes, by its ID
/// and ID_LIKE: apt, dnf, pacman, zypper or apk. None for another system.
pub fn install_bubblewrap(os_release: &str) -> Option<&'static str> {
    let ids: Vec<String> = os_release.lines()
        .filter_map(|l| l.strip_prefix("ID=").or_else(|| l.strip_prefix("ID_LIKE=")))
        .flat_map(|v| v.trim_matches('"').split_whitespace().map(str::to_ascii_lowercase).collect::<Vec<_>>())
        .collect();
    let has = |names: &[&str]| ids.iter().any(|i| names.contains(&i.as_str()));
    if has(&["debian", "ubuntu"]) {
        Some("sudo apt install bubblewrap")
    } else if has(&["fedora", "rhel", "centos"]) {
        Some("sudo dnf install bubblewrap")
    } else if has(&["arch"]) {
        Some("sudo pacman -S bubblewrap")
    } else if has(&["suse", "opensuse"]) {
        Some("sudo zypper install bubblewrap")
    } else if has(&["alpine"]) {
        Some("sudo apk add bubblewrap")
    } else {
        None
    }
}

/// Why a run doesn't start when bubblewrap isn't installed, with what to install on this system (`install_bubblewrap`).
pub fn missing_bubblewrap(os_release: &str) -> String {
    let how = match install_bubblewrap(os_release) {
        Some(cmd) => format!("install it with `{cmd}`"),
        None => "install your system's bubblewrap package".to_string(),
    };
    format!("Hersir runs agents in a sandbox, which needs bubblewrap (bwrap) on this computer: {how}, then start the card again. {SWITCH}")
}

/// Why a run doesn't start when bubblewrap is there but can't make its sandbox: most often user namespaces are off.
pub fn bubblewrap_fails(why: &str) -> String {
    let why = if why.is_empty() { "it ended with an error".to_string() } else { why.to_string() };
    format!("Hersir's sandbox (bubblewrap) can't start on this computer: {why}. It needs user namespaces: check that \
             kernel.unprivileged_userns_clone (Debian) or user.max_user_namespaces isn't 0, and on Ubuntu 24.04 and newer that \
             the bubblewrap package's AppArmor profile is loaded; inside a container, run Hersir outside it. {SWITCH}")
}
