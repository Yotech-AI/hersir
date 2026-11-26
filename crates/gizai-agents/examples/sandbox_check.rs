//! GA-265: what Hersir's sandbox lets a task run do on this computer, checked by starting programs in it:
//! `cargo run -p gizai-agents --example sandbox_check`. It makes a stand-in data folder, worktrees, temp folders and home
//! folder of its own in the system's temp folder, or in HERSIR_CHECK_DIR (never yours), runs each check in the sandbox a Claude Code run gets,
//! prints one line per check, and ends with 1 when one isn't as it should be. CI runs it on Linux and macOS.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use gizai_agents::cli::Kind;
use gizai_agents::sandbox;

fn main() {
    let engine = match sandbox::check() {
        Ok(Some(e)) => e,
        Ok(None) => {
            println!("No sandbox on this system.");
            return;
        }
        Err(why) => {
            println!("The sandbox can't start: {why}");
            std::process::exit(1);
        }
    };
    // short, as a run's temp folder is: Chromium's socket path in it must stay under about 100 characters
    let root = std::env::var_os("HERSIR_CHECK_DIR").filter(|d| !d.is_empty()).map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join(format!("hsc-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&root);
    let ok = check(&engine, &root);
    let _ = std::fs::remove_dir_all(&root);
    if !ok {
        std::process::exit(1);
    }
}

/// `git` in `dir` with an identity of its own; whether it worked.
fn git(dir: &Path, args: &[&str]) -> bool {
    Command::new("git").args(["-c", "user.email=check@hersir.invalid", "-c", "user.name=check"]).args(args).current_dir(dir)
        .output().is_ok_and(|o| o.status.success())
}

fn check(engine: &sandbox::Engine, root: &Path) -> bool {
    let (data, temp_base, home, repo) = (root.join("d"), root.join("t"), root.join("h"), root.join("g"));
    let (wt, other, run, other_run) = (data.join("worktrees/P/C1"), data.join("worktrees/P/C2"), temp_base.join("r"), temp_base.join("r2"));
    for d in [&other, &run, &other_run, &repo, &home.join(".ssh"), &home.join(".config/gh"), &home.join(".claude"), &home.join(".codex")] {
        std::fs::create_dir_all(d).expect("a folder in the temp folder");
    }
    // the worktree of a git repository when git is there, else a plain folder
    let with_git = git(&repo, &["init", "-q"]) && git(&repo, &["commit", "-q", "--allow-empty", "-m", "first"])
        && git(&repo, &["worktree", "add", "-q", "-b", "c1", &wt.to_string_lossy()]);
    std::fs::create_dir_all(&wt).expect("the worktree");
    for (f, text) in [(data.join("hersir.db"), "the database"), (other.join("code.txt"), "another card's code"), (other_run.join("r2.mcp.json"), "{}"),
                      (home.join(".ssh/id_ed25519"), "a key"), (home.join(".config/gh/hosts.yml"), "a token"), (home.join(".codex/auth.json"), "{}"),
                      (home.join(".claude/.credentials.json"), "{}"), (home.join(".claude/settings.json"), "{}"), (wt.join("README.md"), "hi")] {
        std::fs::write(&f, text).expect("a file in the temp folder");
    }
    let commit_dirs = if with_git { gizai_agents::worktree::commit_dirs(&wt).unwrap_or_default() } else { vec![] };
    let policy = sandbox::policy(&sandbox::RunPlaces {
        kind: Kind::ClaudeCode, bin: PathBuf::from("claude"), cli_env: vec![], worktree: wt.clone(), write: commit_dirs, read: vec![],
        temp_dir: Some(run.clone()), data_dir: data.clone(), temp_base: temp_base.clone(), home: Some(home.clone()),
    });
    let wrap = engine.wrap(&policy);
    // whether the script worked, and the end of what it wrote to stderr
    let sh = |script: &str| -> (bool, String) {
        match Command::new(&wrap.program).args(&wrap.args).arg("/bin/sh").arg("-c").arg(script).current_dir(&wt)
            .env("HOME", &home).env("TMPDIR", &run).envs(sandbox::run_env(&run)).stdin(Stdio::null()).output() {
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr);
                let tail: Vec<&str> = err.lines().filter(|l| !l.trim().is_empty()).collect();
                (o.status.success(), tail[tail.len().saturating_sub(4)..].join("\n      "))
            }
            Err(e) => (false, e.to_string()),
        }
    };
    let q = |p: &Path| format!("'{}'", p.display());
    let mut checks: Vec<(String, bool, String)> = vec![
        ("reads its worktree".into(), true, "cat README.md".into()),
        ("reads Hersir's database".into(), false, format!("cat {}", q(&data.join("hersir.db")))),
        ("reads another card's worktree".into(), false, format!("cat {}", q(&other.join("code.txt")))),
        ("reads another run's temp folder".into(), false, format!("cat {}", q(&other_run.join("r2.mcp.json")))),
        ("reads ~/.ssh/id_ed25519".into(), false, "cat ~/.ssh/id_ed25519".into()),
        ("reads gh's login (~/.config/gh)".into(), false, "cat ~/.config/gh/hosts.yml".into()),
        ("reads another CLI's sign-in (~/.codex)".into(), false, "cat ~/.codex/auth.json".into()),
        ("reads its own CLI's sign-in (~/.claude)".into(), true, "cat ~/.claude/.credentials.json".into()),
        ("keeps its own CLI's sessions (~/.claude/projects)".into(), true, "mkdir -p ~/.claude/projects/check && rmdir ~/.claude/projects/check".into()),
        ("changes its own CLI's settings".into(), false, "echo x >> ~/.claude/settings.json".into()),
        ("writes its worktree".into(), true, "echo x > written.txt".into()),
        ("writes its temp folder".into(), true, "echo x > \"$TMPDIR/written.txt\"".into()),
        ("writes your home folder".into(), false, "echo x > ~/written.txt".into()),
        ("writes Hersir's data folder".into(), false, format!("echo x > {}", q(&data.join("written.txt")))),
    ];
    if with_git {
        checks.push(("commits in its worktree".into(), true, "echo x > c.txt && git add c.txt && git -c user.email=check@hersir.invalid -c user.name=check commit -q -m check".into()));
    }
    if Command::new("curl").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success()) {
        checks.push(("looks up names and reaches https://github.com".into(), true, "curl -sS -o /dev/null --max-time 20 https://github.com".into()));
    }
    if cfg!(target_os = "macos") {
        checks.push(("reaches the keychain".into(), true, "/usr/bin/security list-keychains -d system".into()));
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    if let Some(found) = gizai_agents::browser::find_browser(&path) {
        checks.push((format!("takes a headless screenshot with {}", found.name), true,
                     format!("{} --headless --no-first-run --use-mock-keychain --password-store=basic --user-data-dir=\"$TMPDIR/profile\" \
                              --screenshot=\"$TMPDIR/shot.png\" --window-size=400,300 about:blank; test -s \"$TMPDIR/shot.png\"", q(&found.path))));
    }
    println!("Hersir's sandbox ({}) on this computer:", engine.name());
    let mut all = true;
    for (what, should, script) in &checks {
        let (did, err) = sh(script);
        let fine = did == *should;
        all &= fine;
        println!("  [{}] {what}: {}{}", if fine { "ok" } else { "FAIL" }, if did { "yes" } else { "no" },
                 if fine { String::new() } else { format!(" (should be {})", if *should { "yes" } else { "no" }) });
        if !fine && !err.is_empty() {
            println!("      {err}");
        }
    }
    all
}
