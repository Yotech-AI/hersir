//! What differs between Linux, macOS and Windows when Gizai starts a program: finding it, starting it without a
//! console window, and ending it with everything it started (`Tree`) (docs/PLATFORMS.md). On Linux and macOS these are
//! the plain std calls and process groups. And for terminals (`terminal`): your shell, the processes in a terminal's
//! session, and on Windows the terminal's Job Object.
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(windows)]
use std::sync::Arc;

/// Windows: a console program started by Gizai (a window app) gets no console window of its own.
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The environment variable `HERSIR_<name>`, else `GIZAI_<name>`, its name before Hersir 0.17.0 (GA-248): the HERSIR_
/// name wins when both are set, and the GIZAI_ one still works alone. An empty one counts as unset.
pub fn hersir_var(name: &str) -> Option<OsString> {
    [format!("HERSIR_{name}"), format!("GIZAI_{name}")].into_iter().filter_map(std::env::var_os).find(|v| !v.is_empty())
}

/// A Command for `program` (a name or a path) that works on every system. On Windows no console window opens, and an
/// npm `.cmd` shim runs what the shim would: `node <its script>` (Gemini, Codex), or the program it points at (an
/// npm-installed Claude Code's `claude.exe`). So arguments with spaces, quotes and line breaks reach the CLI unchanged.
/// Another `.cmd` or `.bat` goes through Rust's own batch-file quoting, which refuses an argument it can't pass safely.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let program = program.as_ref();
        let found = if Path::new(program).components().count() > 1 {
            Some(PathBuf::from(program)).filter(|p| p.is_file()).or_else(|| with_extension(Path::new(program)))
        } else {
            find_in(&program.to_string_lossy(), &std::env::var_os("PATH").unwrap_or_default())
        };
        let mut cmd = match found.as_deref().and_then(npm_shim) {
            Some((runs, script)) => {
                let mut c = Command::new(runs);
                c.args(script);
                c
            }
            None => Command::new(found.as_deref().map(Path::as_os_str).unwrap_or(program)),
        };
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    }
    #[cfg(not(windows))]
    Command::new(program)
}

/// `command` for Tokio.
pub fn tokio_command(program: impl AsRef<OsStr>) -> tokio::process::Command {
    tokio::process::Command::from(command(program))
}

/// Whether `p` is a program this system runs: a file with an execute bit on Linux and macOS; on Windows a file whose
/// extension is in PATHEXT (`.exe`, `.cmd`, `.bat`, `.com`).
pub fn executable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p).map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
    }
    #[cfg(windows)]
    {
        let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy()).to_ascii_lowercase());
        p.is_file() && ext.is_some_and(|e| path_exts().contains(&e))
    }
}

/// Where `name` (a program's name, or a path) is found: a path as it is when it is a program, else the first folder in
/// `path` that has it. On Windows a name without an extension is tried with each PATHEXT extension (`claude` →
/// `claude.exe`, `gemini` → `gemini.cmd`).
pub fn find_in(name: &str, path: &OsStr) -> Option<PathBuf> {
    if name.is_empty() {
        return None;
    }
    let p = Path::new(name);
    if p.components().count() > 1 || p.is_absolute() {
        if executable(p) {
            return Some(p.to_path_buf());
        }
        #[cfg(windows)]
        return with_extension(p);
        #[cfg(not(windows))]
        return None;
    }
    std::env::split_paths(path).filter(|d| !d.as_os_str().is_empty()).find_map(|d| {
        let candidate = d.join(name);
        if executable(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        return with_extension(&candidate);
        #[cfg(not(windows))]
        None
    })
}

/// Windows: `p` with the first PATHEXT extension that makes it a file.
#[cfg(windows)]
fn with_extension(p: &Path) -> Option<PathBuf> {
    path_exts().iter().map(|e| PathBuf::from(format!("{}{e}", p.display()))).find(|c| c.is_file())
}

/// Windows: the extensions that make a program, lower case with their dot (PATHEXT, else the usual four).
#[cfg(windows)]
fn path_exts() -> Vec<String> {
    let own = std::env::var("PATHEXT").unwrap_or_default();
    let exts: Vec<String> = own.split(';').map(|e| e.trim().to_ascii_lowercase()).filter(|e| e.starts_with('.') && e.len() > 1).collect();
    if exts.is_empty() { [".com", ".exe", ".bat", ".cmd"].map(String::from).to_vec() } else { exts }
}

/// What an npm `.cmd` shim runs, read from the shim's text: its path relative to the shim's folder. That is a script
/// npm runs with node, like `node_modules\@google\gemini-cli\bundle\gemini.js`, or a program of its own, like npm's
/// Claude Code: `node_modules\@anthropic-ai\claude-code\bin\claude.exe`. None for any other text. Plain text work, the
/// same on every system (so it can be tested anywhere); `npm_shim` uses it on Windows.
pub fn npm_shim_script(cmd_text: &str) -> Option<&str> {
    // npm's cmd-shim ends with: "%_prog%"  "%dp0%\node_modules\<package>\<script>.js" %*
    // or, for a program that isn't a script: "%dp0%\node_modules\<package>\<program>.exe"   %*
    let start = cmd_text.rfind("\"%dp0%\\")? + "\"%dp0%\\".len();
    let rel = &cmd_text[start..start + cmd_text[start..].find('"')?];
    let lower = rel.to_ascii_lowercase();
    [".js", ".cjs", ".mjs", ".exe"].iter().any(|e| lower.ends_with(e)).then_some(rel)
}

/// Windows: for an npm `.cmd` shim, what to start instead of it, and its first argument: the node it runs a script
/// with (the `node.exe` next to the shim, else node on PATH) and that script; or the program the shim points at
/// (npm's Claude Code, `claude.exe`) with no first argument. None for any other file.
#[cfg(windows)]
pub fn npm_shim(cmd_file: &Path) -> Option<(PathBuf, Option<PathBuf>)> {
    let ext = cmd_file.extension()?.to_string_lossy().to_ascii_lowercase();
    if ext != "cmd" {
        return None;
    }
    let dir = cmd_file.parent()?;
    let text = std::fs::read_to_string(cmd_file).ok().filter(|t| t.len() < 16 * 1024)?;
    let target = dir.join(npm_shim_script(&text)?);
    if !target.is_file() {
        return None;
    }
    if target.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) {
        return Some((target, None));
    }
    let local = dir.join("node.exe");
    let node = if local.is_file() { local } else { find_in("node", &std::env::var_os("PATH").unwrap_or_default())? };
    Some((node, Some(target)))
}

/// The bash that runs shell commands (a worktree's prepare commands, your GIT_SSH_COMMAND): `bash` on PATH on Linux and
/// macOS. On Windows Git Bash, found the way Claude Code 2.1 finds it, so both use the same one: CLAUDE_CODE_GIT_BASH_PATH
/// when that file exists, then `%ProgramFiles%\Git\bin\bash.exe`, `%ProgramFiles(x86)%\Git\bin\bash.exe`, the one next
/// to git on `path` (`<git>\..\..\bin\bash.exe`), and last, where Claude Code doesn't look, Git for Windows' install for
/// one user (`%LOCALAPPDATA%\Programs\Git\bin\bash.exe`). Never `C:\Windows\System32\bash.exe`: that one starts WSL,
/// which Gizai doesn't use.
pub fn git_bash(path: &OsStr) -> Option<PathBuf> {
    #[cfg(not(windows))]
    {
        let _ = path;
        Some(PathBuf::from("bash"))
    }
    #[cfg(windows)]
    {
        if let Some(p) = std::env::var_os("CLAUDE_CODE_GIT_BASH_PATH").map(PathBuf::from).filter(|p| p.is_file()) {
            return Some(p);
        }
        let in_git = |git_root: PathBuf| Some(git_root.join("bin").join("bash.exe")).filter(|b| b.is_file());
        let program_files = |var: &str, usual: &str| std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(usual)).join("Git");
        in_git(program_files("ProgramFiles", r"C:\Program Files"))
            .or_else(|| in_git(program_files("ProgramFiles(x86)", r"C:\Program Files (x86)")))
            // <Git>\cmd\git.exe or <Git>\bin\git.exe: bash is <Git>\bin\bash.exe
            .or_else(|| find_in("git", path).and_then(|git| git.parent()?.parent().map(Path::to_path_buf)).and_then(in_git))
            .or_else(|| std::env::var_os("LOCALAPPDATA").map(|l| PathBuf::from(l).join("Programs").join("Git")).and_then(in_git))
    }
}

/// How hard `Tree::end` asks. On Linux and macOS a signal to the tree's process group. On Windows each one ends the
/// whole job at once: Gizai is a window app without a console, and Windows has no signal such an app can send a console
/// program (Ctrl+C only reaches the programs of the console it is pressed in), so there is no gentle step there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// SIGINT, what Ctrl+C sends: a CLI stops its turn and exits.
    Interrupt,
    /// SIGTERM.
    Terminate,
    /// SIGKILL, which no program can ignore.
    Kill,
}

/// A program Gizai started with everything it starts in turn, checked on and ended as one (`spawn_tree`).
/// - Linux and macOS: the process group the program leads. Signals only ever go to that group, never to pid 0 or 1.
///   An agent's CLI (`spawn_tree_tokio`) leads a session of its own too (GA-266), so no process outside it can join
///   its group (see `contains`).
/// - Windows: a Job Object. Every process the program starts is in it too and can't leave it, and the job ends what
///   still runs in it when its last handle closes (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`): when the last clone of the
///   `Tree` drops, or when Gizai exits or crashes, so nothing outlives Gizai. Keep a clone for as long as the program is
///   tracked; dropping the last one after it ended is what ends its leftovers.
#[derive(Clone, Debug)]
pub struct Tree {
    #[cfg(unix)]
    group: u32,
    #[cfg(windows)]
    job: Arc<Job>,
}

impl Tree {
    /// Linux and macOS: the process group `pid` leads (a run a previous Gizai started).
    #[cfg(unix)]
    pub fn led_by(pid: u32) -> Tree {
        Tree { group: pid }
    }

    /// Asks everything in the tree to end (see `End`), and returns at once. Hersir's log names it (`end_for`).
    pub fn end(&self, how: End) {
        self.end_for(how, "Hersir ended a program it started, with everything that program started");
    }

    /// `end`, with why in Hersir's log (GA-369, `signal_log`): one line for the signal to the tree's process group (on
    /// Windows one per process its job ends), when it reached something.
    pub fn end_for(&self, how: End, why: &str) {
        #[cfg(unix)]
        {
            let sig = match how {
                End::Interrupt => libc::SIGINT,
                End::Terminate => libc::SIGTERM,
                End::Kill => libc::SIGKILL,
            };
            // only ever the process group started for this program, never pid 0 or 1, nor Hersir's own group
            send_signal(self.group, true, sig, why);
        }
        #[cfg(windows)]
        {
            let _ = how;
            self.job.terminate_for(why);
        }
    }

    /// Whether anything in the tree still runs. On Linux and macOS the program Gizai started counts until it has been
    /// waited for (reaped).
    pub fn alive(&self) -> bool {
        #[cfg(unix)]
        return match i32::try_from(self.group) {
            // SAFETY: signal 0 only checks that the group exists; nothing is sent.
            Ok(g) if g > 1 => unsafe { libc::kill(-g, 0) == 0 },
            _ => false,
        };
        #[cfg(windows)]
        return self.job.active() > 0;
    }

    /// GA-266: whether the process `pid` is in the tree now: the program or something it started (Hersir's MCP shim
    /// that a chat turn's Claude Code starts, say). What each system lets Hersir check, and why no process outside the
    /// tree can pass:
    /// - Linux and macOS: the process group `pid` is in (getpgid), which must be the tree's. A process can move itself
    ///   only into a group of its own session (setpgid), and an agent's CLI leads a session of its own
    ///   (`spawn_tree_tokio`), so only what it starts is ever in its group. What it starts in a session or group of its
    ///   own (`setsid`, Claude Code's Bash tool) isn't. The check is on the pid as it is now: a process that ended and
    ///   whose pid went to another process counts as that other one.
    /// - Windows: whether `pid`'s process is in the tree's Job Object (IsProcessInJob). Everything the program starts
    ///   is in the job and can't break away from it, and no other process can be put in it: only Hersir holds a handle
    ///   to the job, which isn't inherited.
    ///
    /// Never true for pid 0 or 1, or for a process that is gone.
    pub fn contains(&self, pid: u32) -> bool {
        if pid <= 1 {
            return false;
        }
        #[cfg(unix)]
        {
            let (Ok(p), Ok(g)) = (libc::pid_t::try_from(pid), libc::pid_t::try_from(self.group)) else { return false };
            // SAFETY: getpgid only reads; it fails (-1) for a process that is gone.
            g > 1 && unsafe { libc::getpgid(p) } == g
        }
        #[cfg(windows)]
        return self.job.holds(pid);
    }
}

/// Starts `cmd` as a `Tree` of its own: it and everything it starts. `low`: at low CPU priority, which what it starts
/// inherits (nice 10 on Linux and macOS, below normal on Windows).
///
/// Windows: the program is in its job before any of its code runs. It starts suspended, goes into the job, and then its
/// first thread is resumed (std keeps no handle to that thread, so a Toolhelp32 snapshot of the system's threads finds
/// it). Putting it in the job after a normal start would leave a moment in which something it starts escapes the job.
/// This sets the command's creation flags, which replaces the ones it had: no console window (see `command`) is set
/// again.
pub fn spawn_tree(cmd: &mut Command, low: bool) -> io::Result<(std::process::Child, Tree)> {
    spawn_std(cmd, if low { Priority::Low } else { Priority::Normal })
}

/// Agents' work (GA-369): a worktree's copies, installs and setup command, a run's preview. `spawn_tree` at low CPU
/// priority (nice `LOW_NICE` on Linux and macOS), and on Windows below normal for the whole job, which is nested in the
/// job all agents' work is in, whose CPU rate cap is CPU for agents (`set_agents_cpu`).
pub fn spawn_agents_tree(cmd: &mut Command) -> io::Result<(std::process::Child, Tree)> {
    spawn_std(cmd, Priority::Agents)
}

/// The nice value of agents' work and of Hersir's own update build (GA-369): 10, where your desktop's programs have 0.
pub const LOW_NICE: libc::c_int = 10;

/// How a tree's programs share the CPU.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Priority {
    Normal,
    /// Low priority (Hersir's own update build).
    Low,
    /// Agents' work (GA-369): low priority, and on Windows in the agents' job too.
    Agents,
}

fn spawn_std(cmd: &mut Command, priority: Priority) -> io::Result<(std::process::Child, Tree)> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
        if priority != Priority::Normal {
            // SAFETY: nice(2) is a plain syscall, safe to make between fork and exec. It adds to Hersir's own nice value,
            // which never fails: a Hersir started at nice 5 starts its agents at 15.
            unsafe {
                cmd.pre_exec(|| {
                    libc::nice(LOW_NICE);
                    Ok(())
                });
            }
        }
        let child = cmd.spawn()?;
        let group = child.id();
        Ok((child, Tree { group }))
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use std::os::windows::process::CommandExt;
        let job = Job::new()?;
        cmd.creation_flags(start_flags(priority != Priority::Normal));
        let mut child = cmd.spawn()?;
        if priority == Priority::Agents {
            job.below_normal();
            in_agents_job(child.as_raw_handle());
        }
        match job.adopt(child.as_raw_handle(), child.id()) {
            Ok(()) => Ok((child, Tree { job: Arc::new(job) })),
            Err(e) => {
                // still suspended, or running outside the job: it goes either way
                if kill_child(&mut child, "it couldn't be put in its Job Object") {
                    let _ = child.wait();
                }
                Err(e)
            }
        }
    }
}

/// `spawn_tree` for Tokio, at normal priority: Claude Code asked for its tools (`tool_catalog::ask_claude`). Must be
/// called inside a Tokio runtime.
///
/// Linux and macOS: the program leads a session of its own (setsid), and so its own process group, as with
/// `spawn_tree`. In Gizai's session any process (another run's, a preview's) could move itself into that group
/// (setpgid), and then pass for one of the run's own (`Tree::contains`, GA-266); from another session it can't. A run
/// has no controlling terminal either way: Gizai itself has none when your desktop starts it.
pub fn spawn_tree_tokio(cmd: &mut tokio::process::Command) -> io::Result<(tokio::process::Child, Tree)> {
    spawn_tokio(cmd, false)
}

/// `spawn_tree_tokio` for agents' work (GA-369): an agent's CLI in a task run, a chat answer or a board check
/// (`process::spawn_exec`), at low CPU priority as `spawn_agents_tree` says. On Linux its session's autogroup gets nice
/// `LOW_NICE` too: where the kernel groups each session's processes for the CPU (autogroup, outside the agent runs'
/// slice), a nice value only counts inside its own session.
pub fn spawn_agents_tree_tokio(cmd: &mut tokio::process::Command) -> io::Result<(tokio::process::Child, Tree)> {
    spawn_tokio(cmd, true)
}

fn spawn_tokio(cmd: &mut tokio::process::Command, agents: bool) -> io::Result<(tokio::process::Child, Tree)> {
    #[cfg(unix)]
    {
        // SAFETY: setsid(2) and nice(2) are plain syscalls, safe to make between fork and exec. setsid fails only for a
        // process that already leads a group, which the child Gizai just forked doesn't.
        unsafe {
            cmd.pre_exec(move || {
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                if agents {
                    libc::nice(LOW_NICE);
                    #[cfg(target_os = "linux")]
                    autogroup_nice();
                }
                Ok(())
            });
        }
        let child = cmd.spawn()?;
        // None only for a child that has been waited for
        let group = child.id().unwrap_or(0);
        Ok((child, Tree { group }))
    }
    #[cfg(windows)]
    {
        let job = Job::new()?;
        cmd.creation_flags(start_flags(agents));
        let mut child = cmd.spawn()?;
        if agents {
            job.below_normal();
            if let Some(process) = child.raw_handle() {
                in_agents_job(process);
            }
        }
        let adopted = match (child.raw_handle(), child.id()) {
            (Some(process), Some(pid)) => job.adopt(process, pid),
            _ => Err(io::Error::other("it ended at once")),
        };
        match adopted {
            Ok(()) => Ok((child, Tree { job: Arc::new(job) })),
            Err(e) => {
                start_kill_child(&mut child, "it couldn't be put in its Job Object");
                Err(e)
            }
        }
    }
}

/// Linux, between fork and exec of a program that leads a new session: that session's autogroup gets nice
/// `LOW_NICE` (/proc/self/autogroup). Nothing when the kernel has no autogroups, or turns the write down (it takes one
/// such write in a tenth of a second).
#[cfg(target_os = "linux")]
fn autogroup_nice() {
    // SAFETY: open, write and close are async-signal-safe syscalls; the path is a NUL-terminated constant.
    unsafe {
        let fd = libc::open(c"/proc/self/autogroup".as_ptr(), libc::O_WRONLY | libc::O_CLOEXEC);
        if fd >= 0 {
            let _ = libc::write(fd, b"10".as_ptr().cast(), 2);
            libc::close(fd);
        }
    }
}

/// CPU for agents (GA-369): from now on agents' work may use at most `cores` of this computer's cores together, what
/// already runs too; None, or all of them, for no cap. On Windows the CPU rate cap of the job all agents' jobs are in
/// (`spawn_agents_tree`), a hard cap. Elsewhere nothing: Linux caps the agent runs' slice instead
/// (`run_memory::Slice::set_cpu_cap`), and macOS has no cap. Err says why Windows refused it.
pub fn set_agents_cpu(cores: Option<u32>) -> Result<(), String> {
    #[cfg(windows)]
    {
        let all = crate::run_cpu::cores().max(1);
        // in hundredths of a percent of all the computer's CPU, as the job's CpuRate counts
        let rate = match cores {
            Some(c) if c < all => ((u64::from(c.max(1)) * 10_000 / u64::from(all)) as u32).max(1),
            _ => 0,
        };
        AGENTS_RATE.store(rate, std::sync::atomic::Ordering::SeqCst);
        match AGENTS_JOB.get() {
            Some(Some(job)) => job.cap_cpu(rate).map_err(|e| format!("Windows refused the agents' CPU cap: {e}")),
            _ => Ok(()),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = cores;
        Ok(())
    }
}

/// Windows: the job all agents' jobs are in (GA-369), made at the first agents' tree with CPU for agents as its CPU rate
/// cap; None when Windows wouldn't make it, and then agents' work starts in a job of its own only.
#[cfg(windows)]
static AGENTS_JOB: std::sync::OnceLock<Option<Job>> = std::sync::OnceLock::new();

/// Windows: CPU for agents as the agents' job's CpuRate (hundredths of a percent of all the computer's CPU); 0 for no
/// cap.
#[cfg(windows)]
static AGENTS_RATE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Windows: puts the suspended process `process` in the agents' job, before it goes in its own job, which so is nested
/// in the agents' one and held to its CPU cap. When Windows refuses, the process starts in its own job only, and
/// Hersir's log says why, once.
#[cfg(windows)]
fn in_agents_job(process: windows_sys::Win32::Foundation::HANDLE) {
    static SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let agents = AGENTS_JOB.get_or_init(|| {
        let job = Job::new().ok()?;
        let rate = AGENTS_RATE.load(std::sync::atomic::Ordering::SeqCst);
        if rate > 0 && let Err(e) = job.cap_cpu(rate) {
            eprintln!("gizai: agents' work starts without a CPU cap: Windows refused it: {e}");
        }
        Some(job)
    });
    let added = match agents {
        Some(job) => job.add(process).map_err(|e| e.to_string()),
        None => Err("Windows wouldn't make the agents' job".to_string()),
    };
    if let Err(e) = added
        && !SAID.swap(true, std::sync::atomic::Ordering::SeqCst)
    {
        eprintln!("gizai: agents' work starts outside the agents' job, without its CPU cap: {e}");
    }
}

/// Windows: the exit code of the programs in a job Gizai ends (there is no signal to report instead).
#[cfg(windows)]
const ENDED: u32 = 1;

/// Windows: the creation flags of a program started as a `Tree`.
#[cfg(windows)]
fn start_flags(low: bool) -> u32 {
    use windows_sys::Win32::System::Threading::{BELOW_NORMAL_PRIORITY_CLASS, CREATE_SUSPENDED};
    CREATE_NO_WINDOW | CREATE_SUSPENDED | if low { BELOW_NORMAL_PRIORITY_CLASS } else { 0 }
}

/// Windows: a Job Object's handle, closed on drop, which ends what still runs in the job. An agent's `Tree` and a
/// terminal's shell (`terminal`) each have one.
#[cfg(windows)]
#[derive(Debug)]
pub(crate) struct Job(windows_sys::Win32::Foundation::HANDLE);

// SAFETY: a kernel handle may be used, and closed, on any thread.
#[cfg(windows)]
unsafe impl Send for Job {}
#[cfg(windows)]
unsafe impl Sync for Job {}

#[cfg(windows)]
impl Job {
    /// A new job that ends everything in it when its last handle closes. Its handle isn't inherited, so only Gizai holds
    /// it, and it allows no breakaway: a program in it can't start anything outside it.
    pub(crate) fn new() -> io::Result<Job> {
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        };
        // SAFETY: plain calls with valid pointers; from here on the handle belongs to the Job, which closes it.
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = Job(handle);
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let size = std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
            if SetInformationJobObject(job.0, JobObjectExtendedLimitInformation, (&raw const limits).cast(), size) == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(job)
        }
    }

    /// Agents' work (GA-369): every process in the job at below-normal priority (JOB_OBJECT_LIMIT_PRIORITY_CLASS), as
    /// the program also starts (`start_flags`). Best effort: when Windows refuses, the job keeps the limits `new` gave it,
    /// and the program's own below-normal priority, which what it starts inherits, stays.
    fn below_normal(&self) {
        use windows_sys::Win32::System::JobObjects::{
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_PRIORITY_CLASS, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JobObjectExtendedLimitInformation, SetInformationJobObject,
        };
        use windows_sys::Win32::System::Threading::BELOW_NORMAL_PRIORITY_CLASS;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_PRIORITY_CLASS;
        limits.BasicLimitInformation.PriorityClass = BELOW_NORMAL_PRIORITY_CLASS;
        let size = std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
        // SAFETY: a plain call with a valid handle and a struct of the size given.
        unsafe {
            SetInformationJobObject(self.0, JobObjectExtendedLimitInformation, (&raw const limits).cast(), size);
        }
    }

    /// The agents' job (GA-369): a hard cap of `rate` hundredths of a percent of all the computer's CPU for everything in
    /// it and in the jobs nested in it, at once for what already runs too; 0 for none.
    fn cap_cpu(&self, rate: u32) -> io::Result<()> {
        use windows_sys::Win32::System::JobObjects::{
            JOB_OBJECT_CPU_RATE_CONTROL_ENABLE, JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP, JOBOBJECT_CPU_RATE_CONTROL_INFORMATION,
            JobObjectCpuRateControlInformation, SetInformationJobObject,
        };
        let mut info = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION::default();
        if rate > 0 {
            info.ControlFlags = JOB_OBJECT_CPU_RATE_CONTROL_ENABLE | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP;
            info.Anonymous.CpuRate = rate.min(10_000);
        }
        let size = std::mem::size_of::<JOBOBJECT_CPU_RATE_CONTROL_INFORMATION>() as u32;
        // SAFETY: a plain call with a valid handle and a struct of the size given.
        if unsafe { SetInformationJobObject(self.0, JobObjectCpuRateControlInformation, (&raw const info).cast(), size) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Puts the suspended process `process` (id `pid`) in the job, then resumes its threads.
    fn adopt(&self, process: windows_sys::Win32::Foundation::HANDLE, pid: u32) -> io::Result<()> {
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next};
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
        use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};
        // SAFETY: plain calls with valid handles and pointers; every handle opened here is closed here.
        unsafe {
            if AssignProcessToJobObject(self.0, process) == 0 {
                return Err(io::Error::last_os_error());
            }
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return Err(io::Error::last_os_error());
            }
            let mut entry = THREADENTRY32 { dwSize: std::mem::size_of::<THREADENTRY32>() as u32, ..Default::default() };
            let mut resumed = 0;
            let mut more = Thread32First(snapshot, &mut entry) != 0;
            while more {
                if entry.th32OwnerProcessID == pid {
                    let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                    if !thread.is_null() {
                        if ResumeThread(thread) != u32::MAX {
                            resumed += 1;
                        }
                        CloseHandle(thread);
                    }
                }
                more = Thread32Next(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
            if resumed == 0 {
                return Err(io::Error::other("Windows didn't let Hersir resume the program it started"));
            }
            Ok(())
        }
    }

    /// How many processes run in the job now (0 when Windows doesn't say).
    fn active(&self) -> u32 {
        use windows_sys::Win32::System::JobObjects::{JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JobObjectBasicAccountingInformation, QueryInformationJobObject};
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        let size = std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32;
        // SAFETY: a plain call with a valid handle and a buffer of the size given.
        let ok = unsafe { QueryInformationJobObject(self.0, JobObjectBasicAccountingInformation, (&raw mut info).cast(), size, std::ptr::null_mut()) };
        if ok == 0 { 0 } else { info.ActiveProcesses }
    }

    /// GA-266: whether the process `pid` runs in the job (or in a job inside it). False when Windows doesn't say: a
    /// process that is gone, or that Hersir may not look at.
    fn holds(&self, pid: u32) -> bool {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::JobObjects::IsProcessInJob;
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
        // SAFETY: plain calls with valid handles and a valid pointer; the process handle opened here is closed here.
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return false;
            }
            let mut inside = 0;
            let ok = IsProcessInJob(process, self.0, &mut inside) != 0;
            CloseHandle(process);
            ok && inside != 0
        }
    }

    /// Ends every process in the job, and names each in Hersir's log with `why` (GA-369, `signal_log`).
    pub(crate) fn terminate_for(&self, why: &str) {
        let about: Vec<crate::signal_log::About> = self.processes().into_iter().filter(|&p| p != 0).map(crate::signal_log::about).collect();
        // SAFETY: a plain call with a valid handle.
        let ended = unsafe { windows_sys::Win32::System::JobObjects::TerminateJobObject(self.0, ENDED) } != 0;
        if ended {
            for a in &about {
                crate::signal_log::record(a, false, "TerminateJobObject", why);
            }
        }
    }

    /// Puts the running process `process` in the job (a terminal's shell, which portable-pty starts itself, not
    /// suspended): what it starts from then on is in the job too.
    pub(crate) fn add(&self, process: windows_sys::Win32::Foundation::HANDLE) -> io::Result<()> {
        // SAFETY: a plain call with valid handles.
        if unsafe { windows_sys::Win32::System::JobObjects::AssignProcessToJobObject(self.0, process) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// The ids of the processes in the job now (none when Windows doesn't say).
    fn processes(&self) -> Vec<u32> {
        use windows_sys::Win32::System::JobObjects::{JOBOBJECT_BASIC_PROCESS_ID_LIST, JobObjectBasicProcessIdList, QueryInformationJobObject};
        // the list's two counts, then up to 1,023 ids, in words the size of an id (ULONG_PTR)
        let mut words = vec![0usize; 1024];
        let size = (words.len() * std::mem::size_of::<usize>()) as u32;
        // SAFETY: the buffer is `size` bytes, aligned for the list; Windows fills in at most that many.
        unsafe {
            let list = words.as_mut_ptr().cast::<JOBOBJECT_BASIC_PROCESS_ID_LIST>();
            if QueryInformationJobObject(self.0, JobObjectBasicProcessIdList, list.cast(), size, std::ptr::null_mut()) == 0 {
                return vec![];
            }
            let n = (*list).NumberOfProcessIdsInList as usize;
            let first = std::ptr::addr_of!((*list).ProcessIdList).cast::<usize>();
            let room = (size as usize - (first as usize - list as usize)) / std::mem::size_of::<usize>();
            std::slice::from_raw_parts(first, n.min(room)).iter().map(|&id| id as u32).collect()
        }
    }

    /// The program started last in the job, other than `except` (a terminal's shell) and the console hosts (conhost,
    /// OpenConsole): its name without `.exe` ("node", "cargo"). None when only those run.
    pub(crate) fn newest(&self, except: u32) -> Option<String> {
        use std::os::windows::ffi::OsStringExt;
        use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
        use windows_sys::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
        let mut newest: Option<(u64, String)> = None;
        for pid in self.processes().into_iter().filter(|&p| p != except && p != 0) {
            let mut path = [0u16; 1024];
            let mut len = path.len() as u32;
            let (mut created, mut exited, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
            // SAFETY: plain calls with buffers of the sizes given; the handle opened here is closed here.
            let found = unsafe {
                let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if process.is_null() {
                    continue;
                }
                let named = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, path.as_mut_ptr(), &mut len) != 0;
                let timed = GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) != 0;
                CloseHandle(process);
                named && timed
            };
            if !found {
                continue;
            }
            let file = PathBuf::from(OsString::from_wide(&path[..len as usize]));
            let Some(name) = file.file_stem().map(|s| s.to_string_lossy().to_string()) else { continue };
            if ["conhost", "openconsole"].contains(&name.to_ascii_lowercase().as_str()) {
                continue;
            }
            let at = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
            if newest.as_ref().is_none_or(|(t, _)| at > *t) {
                newest = Some((at, name));
            }
        }
        newest.map(|(_, name)| name)
    }
}

#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        // SAFETY: the handle is the Job's own and is closed once. With KILL_ON_JOB_CLOSE this ends what still runs in it.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

// ---- terminals (GA-133, see `terminal`) ----

/// The shell a terminal starts, with its arguments (docs/research/terminal.md §3.2 Q1):
/// - Linux: `$SHELL`, else your entry in the password database, else `/bin/sh`, with no arguments. On a
///   pseudo-terminal that is an interactive shell, as GNOME Terminal and Konsole start it, so ~/.bashrc or ~/.zshrc
///   runs (Gizai itself started from your login shell).
/// - macOS: the same, as a login shell (`-l`), as Terminal.app starts it, so /etc/zprofile and ~/.zprofile set up PATH.
/// - Windows: PowerShell 7 (`pwsh.exe`) when it is on `path`, else Windows PowerShell (`powershell.exe`), both without
///   their banner (`-NoLogo`); else `%ComSpec%` (cmd.exe).
pub fn user_shell(path: &OsStr) -> (PathBuf, Vec<OsString>) {
    #[cfg(unix)]
    {
        let shell = std::env::var_os("SHELL")
            .filter(|s| !s.is_empty())
            .and_then(|s| find_in(&s.to_string_lossy(), path))
            .or_else(password_shell)
            .unwrap_or_else(|| PathBuf::from("/bin/sh"));
        let args = if cfg!(target_os = "macos") { vec![OsString::from("-l")] } else { vec![] };
        (shell, args)
    }
    #[cfg(windows)]
    {
        for name in ["pwsh.exe", "powershell.exe"] {
            if let Some(p) = find_in(name, path) {
                return (p, vec![OsString::from("-NoLogo")]);
            }
        }
        let cmd = std::env::var_os("ComSpec").map(PathBuf::from).filter(|p| p.is_file()).unwrap_or_else(|| PathBuf::from("cmd.exe"));
        (cmd, vec![])
    }
}

/// Linux and macOS: your login shell from the password database, when it is a program.
#[cfg(unix)]
fn password_shell() -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let mut buf = vec![0 as libc::c_char; 16 * 1024];
    // SAFETY: getpwuid_r fills `entry` with pointers into `buf`, which outlives their use here.
    let shell = unsafe {
        let mut entry: libc::passwd = std::mem::zeroed();
        let mut found: *mut libc::passwd = std::ptr::null_mut();
        if libc::getpwuid_r(libc::getuid(), &mut entry, buf.as_mut_ptr(), buf.len(), &mut found) != 0 || found.is_null() || entry.pw_shell.is_null() {
            return None;
        }
        std::ffi::CStr::from_ptr(entry.pw_shell).to_bytes().to_vec()
    };
    let p = PathBuf::from(OsStr::from_bytes(&shell));
    (p.is_absolute() && executable(&p)).then_some(p)
}

/// Linux and macOS: the processes in the session `session` that still run (a zombie has ended), each with its process
/// group. A terminal's shell leads a session of its own, so these are the processes on that terminal: what runs in the
/// foreground, background jobs and `nohup` programs. When the shell ends, the kernel takes the terminal away from what
/// it left (their tty in /proc/<pid>/stat becomes 0), but not their session, and the session's id can't go to another
/// process while the shell isn't reaped (`terminal` reaps it last). A process that left on purpose (`setsid`, a
/// daemon) isn't in it. Never pid 0 or 1, nor Gizai itself.
#[cfg(unix)]
pub fn in_session(session: u32) -> Vec<(u32, u32)> {
    if session <= 1 {
        return vec![];
    }
    session_processes(session)
}

/// Linux: from /proc/<pid>/stat.
#[cfg(target_os = "linux")]
fn session_processes(session: u32) -> Vec<(u32, u32)> {
    let me = std::process::id();
    let Ok(dir) = std::fs::read_dir("/proc") else { return vec![] };
    dir.flatten().filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok()).filter(|&pid| pid > 1 && pid != me).filter_map(|pid| {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        // after "pid (comm) " come: state, ppid, pgrp, session, tty_nr, …
        let fields: Vec<&str> = stat[stat.rfind(')')? + 1..].split_whitespace().collect();
        let (state, group, sid) = (*fields.first()?, fields.get(2)?.parse::<u32>().ok()?, fields.get(3)?.parse::<u32>().ok()?);
        (sid == session && !matches!(state, "Z" | "X")).then_some((pid, group))
    }).collect()
}

/// macOS, which has no /proc: every process (proc_listallpids), its session (getsid) and its group and state from the
/// kernel's proc_pidinfo (PROC_PIDTBSDINFO).
#[cfg(target_os = "macos")]
fn session_processes(session: u32) -> Vec<(u32, u32)> {
    let me = std::process::id();
    let mut pids = vec![0 as libc::c_int; 4096];
    let n = loop {
        let size = (pids.len() * std::mem::size_of::<libc::c_int>()) as libc::c_int;
        // SAFETY: the buffer holds `size` bytes; the kernel writes at most that many and returns how many ids it wrote.
        let n = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), size) };
        if n < 0 {
            return vec![];
        }
        if (n as usize) < pids.len() || pids.len() >= 1 << 20 {
            break (n as usize).min(pids.len());
        }
        pids.resize(pids.len() * 2, 0);
    };
    pids[..n].iter().filter_map(|&pid| {
        let p = u32::try_from(pid).ok().filter(|&p| p > 1 && p != me)?;
        // SAFETY: plain syscalls; proc_pidinfo writes at most `size` bytes into `info`, a plain C struct.
        unsafe {
            if libc::getsid(pid) != session as libc::pid_t {
                return None;
            }
            let mut info: libc::proc_bsdinfo = std::mem::zeroed();
            let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
            if libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, (&raw mut info).cast(), size) != size || info.pbi_status == libc::SZOMB {
                return None;
            }
            Some((p, info.pbi_pgid))
        }
    }).collect()
}

/// Linux and macOS: the name the kernel keeps for the process `pid` ("sleep", "npm"; on Linux at most 15 characters).
#[cfg(target_os = "linux")]
pub fn process_name(pid: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/comm")).ok().map(|n| n.trim_end().to_string()).filter(|n| !n.is_empty())
}

/// Linux and macOS: the name the kernel keeps for the process `pid` ("sleep", "npm").
#[cfg(target_os = "macos")]
pub fn process_name(pid: u32) -> Option<String> {
    let mut name = [0u8; 256];
    // SAFETY: proc_name writes at most the size given into `name` and returns the name's length.
    let n = unsafe { libc::proc_name(libc::pid_t::try_from(pid).ok()?, name.as_mut_ptr().cast(), name.len() as u32) };
    (n > 0).then(|| String::from_utf8_lossy(&name[..(n as usize).min(name.len())]).to_string())
}

/// Windows: the name of the program the process `pid` runs, without `.exe` ("node", "claude"), for Hersir's log
/// (GA-369).
#[cfg(windows)]
pub fn process_name(pid: u32) -> Option<String> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
    let mut path = [0u16; 1024];
    let mut len = path.len() as u32;
    // SAFETY: plain calls with a buffer of the size given; the handle opened here is closed here.
    let named = unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let named = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, path.as_mut_ptr(), &mut len) != 0;
        CloseHandle(process);
        named
    };
    if !named {
        return None;
    }
    let file = PathBuf::from(OsString::from_wide(&path[..(len as usize).min(path.len())]));
    file.file_stem().map(|s| s.to_string_lossy().to_string())
}

/// Other systems: no name.
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub fn process_name(pid: u32) -> Option<String> {
    let _ = pid;
    None
}

/// Linux and macOS: sends `signal` to the process `pid`, or with `group` to the process group it names. Never to 0 or
/// 1, nor to Gizai or its own group. GA-369: what it reached is named in Hersir's log with `why` (`signal_log`): its
/// name and cgroup, read just before. Returns whether it reached a process.
#[cfg(unix)]
pub(crate) fn send_signal(pid: u32, group: bool, signal: libc::c_int, why: &str) -> bool {
    let Ok(id) = libc::pid_t::try_from(pid) else { return false };
    // SAFETY: getpid and getpgrp only read.
    if id <= 1 || id == unsafe { libc::getpid() } || id == unsafe { libc::getpgrp() } {
        return false;
    }
    let about = crate::signal_log::about(pid);
    // SAFETY: a plain syscall to the process, or with a negative pid to the process group, named above.
    let sent = unsafe { libc::kill(if group { -id } else { id }, signal) } == 0;
    if sent {
        crate::signal_log::record(&about, group, &crate::signal_log::signal_name(signal), why);
    }
    sent
}

/// Ends `child`, a program Hersir started itself and gives up on (it didn't answer in time), and names it in Hersir's
/// log with `why` (GA-369, `signal_log`): SIGKILL on Linux and macOS, TerminateProcess on Windows.
/// Returns whether it ended it now (false for one that had ended already).
pub fn kill_child(child: &mut std::process::Child, why: &str) -> bool {
    // one that has ended already gets nothing, and no line
    if !matches!(child.try_wait(), Ok(None)) {
        return false;
    }
    let about = crate::signal_log::about(child.id());
    let ended = child.kill().is_ok();
    if ended {
        crate::signal_log::record(&about, false, if cfg!(windows) { "TerminateProcess" } else { "SIGKILL" }, why);
    }
    ended
}

/// `kill_child` for Tokio's child, without waiting for it.
pub fn start_kill_child(child: &mut tokio::process::Child, why: &str) {
    if !matches!(child.try_wait(), Ok(None)) {
        return;
    }
    let about = child.id().map(crate::signal_log::about);
    if child.start_kill().is_ok()
        && let Some(about) = about
    {
        crate::signal_log::record(&about, false, if cfg!(windows) { "TerminateProcess" } else { "SIGKILL" }, why);
    }
}
