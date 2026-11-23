//! GA-322 (GA-270 #2, #9), end to end with fake CLIs, never the real ones:
//! - a Codex run that points its worktree's `commondir` at a folder of its own, or adds a `config.worktree`, each with a
//!   planted filter (crates/gizai-agents/tests/fake-cli-git-folder.sh), ends with the card on hold, blocked, with a
//!   sentence; Hersir pushes nothing, its next git there (the Changes view) runs no filter, the next start is held before
//!   it starts, and Run by hand refuses. Plain git in that worktree does run the filter, so each probe is a real one;
//! - a Gemini run in a worktree whose `.env` sets `BASH_ENV`, `ENV` and `GOOGLE_GEMINI_BASE_URL` gets none of them
//!   (crates/gizai-agents/tests/fake-gemini-env.sh reads the `.env` the way Gemini 0.65 does), and its own settings file
//!   goes at the run's end;
//! - Hersir's start makes each project's card folder its own (`worktree::card_folder`), and removes a leftover
//!   `.gemini.json`.
// Linux and macOS only: these tests run shell scripts as fake programs, which Windows can't start.
#![cfg(unix)]
use std::path::{Path, PathBuf};
use std::process::Command;

use gizai_agents::stream::RunEvent;
use gizai_core::clis::Cli;
use gizai_core::model::{AgentInput, TaskPatch};

#[path = "../support/data_lock.rs"]
mod data_lock;

const FAKE_GIT_FOLDER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-cli-git-folder.sh");
const FAKE_GEMINI_ENV: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-gemini-env.sh");
const LINK: &str = "https://github.com/acme/shop";

/// git in `dir` as a fixed person, without the environment of a git Hersir starts.
fn git_out(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git").args(["-c", "user.email=t@t", "-c", "user.name=t", "-c", "commit.gpgsign=false"]).args(args).current_dir(dir)
        .env_remove("GIT_DIR").env_remove("GIT_COMMON_DIR").env_remove("GIT_WORK_TREE").output().unwrap()
}

fn git(dir: &Path, args: &[&str]) -> String {
    let o = git_out(dir, args);
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

fn has_branch(repo: &Path, branch: &str) -> bool {
    git_out(repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).status.success()
}

/// Writes an executable script through a child `sh` (see push_after_run_test: no write handle left in this process).
fn write_script(path: &Path, script: &str) {
    use std::io::Write;
    let mut sh = Command::new("sh").args(["-c", r#"cat > "$1" && chmod 755 "$1""#, "sh"]).arg(path)
        .stdin(std::process::Stdio::piped()).spawn().unwrap();
    sh.stdin.take().unwrap().write_all(script.as_bytes()).unwrap();
    assert!(sh.wait().unwrap().success(), "can't write {}", path.display());
}

/// A program at `tmp/<name>` that runs the fake `script` with bash, with the same arguments and stdin.
fn fake(tmp: &Path, name: &str, script: &str) -> String {
    let path = tmp.join(name);
    write_script(&path, &format!("#!/bin/sh\nexec bash '{script}' \"$@\"\n"));
    path.display().to_string()
}

/// A repository at `dir` with a.txt (and `extra` files) committed on main.
fn repo_at(dir: &Path, extra: &[(&str, &str)]) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    for (name, text) in extra {
        std::fs::write(dir.join(name), text).unwrap();
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "init"]);
    dir.to_path_buf()
}

/// Adds a CLI in Settings → Coding CLIs and puts the Backend Agent on it.
fn backend_on_cli(st: &gizai_lib::AppState, name: &str, kind: &str, command: &str, env: &[String]) -> String {
    let mut list: Vec<Cli> = gizai_core::clis::list(&st.db).unwrap();
    list.push(Cli { name: name.into(), kind: kind.into(), command: command.into(), env: env.to_vec(), ..Default::default() });
    let cli = gizai_lib::clis::save(st, list).unwrap().into_iter().find(|c| c.cli.name == name).unwrap().cli.id;
    let (_, agent) = gizai_core::team::all_agents(&st.db).unwrap().into_iter().find(|(_, m)| m.role_key == "backend").unwrap();
    gizai_core::team::update_agent(&st.db, &st.you_id, &agent.actor_id, AgentInput { name: agent.name.clone(), role_key: "backend".into(),
        adapter: cli, ..Default::default() }).unwrap();
    agent.actor_id
}

/// Makes In progress Manual, so the queue doesn't start a card that stayed there again by itself.
fn in_progress_manual(st: &gizai_lib::AppState) {
    let team_id = gizai_core::team::list(&st.db).unwrap()[0].id.clone();
    let state = gizai_core::team::get(&st.db, &team_id).unwrap().states.into_iter().find(|s| s.category == "in_progress").unwrap().id;
    gizai_core::columns::set_column(&st.db, &st.you_id, &state, gizai_core::columns::ColumnInput { auto: Some(false), ..Default::default() }).unwrap();
}

/// A run's notes from Hersir, as Show output reads them back from its log.
fn notes(st: &gizai_lib::AppState, run_id: &str) -> Vec<String> {
    gizai_lib::runs::events_for(st, run_id).into_iter()
        .filter_map(|e| match e.event { RunEvent::Note { text } => Some(text), _ => None }).collect()
}

fn stderr_of(run: &gizai_core::model::Run) -> String {
    std::fs::read_to_string(Path::new(&run.log_path).with_extension("stderr.log")).unwrap()
}

fn unhold(st: &gizai_lib::AppState, task: &str) {
    gizai_core::tasks::update(&st.db, &st.you_id, task, TaskPatch { hold: Some(String::new()), ..Default::default() }).unwrap();
}

/// "GitHub" (a bare repository under tmp/github, with a.txt on main) and the project's local clone of
/// https://github.com/acme/shop, which allows no transport but local files.
fn github(tmp: &Path) -> (PathBuf, PathBuf) {
    let src = repo_at(&tmp.join("src"), &[]);
    let bare = tmp.join("github/acme/shop");
    git(tmp, &["clone", "-q", "--bare", src.to_str().unwrap(), bare.to_str().unwrap()]);
    git(&bare, &["config", "core.hooksPath", bare.join("hooks").to_str().unwrap()]);
    let local = tmp.join("local");
    let rewrite = format!("url.{}/.insteadOf=https://github.com/acme/", tmp.join("github/acme").display());
    git(tmp, &["clone", "-q", "-c", &rewrite, "-c", "protocol.allow=never", "-c", "protocol.file.allow=always",
               "-c", "user.email=t@t", "-c", "user.name=t", LINK, local.to_str().unwrap()]);
    (bare, local)
}

#[tokio::test]
async fn a_codex_run_that_points_commondir_at_its_own_folder_holds_the_card_and_hersirs_git_runs_no_planted_filter() {
    let tmp = tempfile::tempdir().unwrap();
    let tmp = tmp.path().canonicalize().unwrap();
    let st = gizai_lib::test_state(&tmp);
    let (bare, local) = github(&tmp);
    let task = gizai_lib::test_task(&st, local.to_str().unwrap(), "backend");
    let p = gizai_core::projects::list(&st.db).unwrap().into_iter().find(|p| p.key == "KADE").unwrap();
    gizai_core::projects::update(&st.db, &st.you_id, &p.id, gizai_core::model::ProjectInput { name: p.name.clone(), key: p.key.clone(),
        repo_path: p.repo_path.clone(), repo_url: Some(LINK.into()), default_branch: Some("main".into()), ..Default::default() }).unwrap();
    let (probe, evil) = (tmp.join("probe-ran"), tmp.join("evil-common"));
    let codex = fake(&tmp, "codex", FAKE_GIT_FOLDER);
    let agent = backend_on_cli(&st, "Codex", "codex", &codex, &["FAKE_KIND=codex".into(), "FAKE_GIT_FOLDER=commondir".into(),
        format!("FAKE_PROBE={}", probe.display()), format!("FAKE_EVIL={}", evil.display())]);
    in_progress_manual(&st);

    let s = gizai_lib::runs::run_once(&st, &task, None, None).await.unwrap();
    let run = gizai_core::runs::get(&st.db, &s.run_id).unwrap();
    assert_eq!(run.status, "succeeded", "{run:?}");
    let (wt, branch) = (PathBuf::from(run.worktree_path.clone().unwrap()), run.branch.clone().unwrap());
    let commondir = local.join(".git/worktrees/KADE-1/commondir");
    assert_eq!(std::fs::read_to_string(&commondir).unwrap().trim(), evil.display().to_string(), "the fake changed it");

    // the card is held, blocked, with a sentence; it didn't move on, and nothing was pushed
    let t = gizai_core::tasks::get(&st.db, &task).unwrap();
    let reason = t.hold_reason.clone().unwrap_or_default();
    assert_eq!(t.hold.as_deref(), Some("blocked"), "{t:?}");
    assert!(reason.starts_with(&format!("{} no longer says ../..", commondir.display())) && reason.contains("take the hold off"), "{reason}");
    assert_ne!(t.state_name, "Testing", "the run's ready_for_testing doesn't move a held card");
    assert!(!has_branch(&bare, &branch), "Hersir pushed a branch whose git folder the run changed");
    assert!(notes(&st, &s.run_id).iter().any(|n| *n == reason), "the run's log says it too: {:?}", notes(&st, &s.run_id));
    assert!(!probe.exists(), "Hersir's git ran the planted filter at the run's end");

    // Hersir's next git there: the Changes view reads the worktree from the project's repository, and runs no filter
    let changes = gizai_lib::changes::of_card(&st, &task).unwrap();
    assert!(changes.files.iter().any(|f| f.path == "a.txt"), "{:?}", changes.files);
    assert!(!probe.exists(), "Hersir's Changes view ran the planted filter");

    // taking the hold off without setting it right: the next start is held before it starts, and Run by hand refuses
    unhold(&st, &task);
    let err = gizai_lib::runs::run_once(&st, &task, Some(agent), None).await.map(|s| s.status).unwrap_err();
    assert_eq!(err, reason);
    let t = gizai_core::tasks::get(&st.db, &task).unwrap();
    assert_eq!((t.hold.as_deref(), t.hold_reason.as_deref()), (Some("blocked"), Some(reason.as_str())));
    assert_eq!(gizai_core::runs::list_for_task(&st.db, &task).unwrap().len(), 1, "no second run");
    unhold(&st, &task);
    gizai_core::tasks::update(&st.db, &st.you_id, &task, TaskPatch { assignee_id: Some(st.you_id.clone()), ..Default::default() }).unwrap();
    assert_eq!(gizai_lib::runs::start_by_hand(&st, &task).await.unwrap_err(), reason, "Run by hand (Open in editor) refuses");
    assert!(!probe.exists());

    // the probe is real: plain git in that worktree follows commondir and runs the filter, as you
    git(&wt, &["diff"]);
    assert!(probe.exists(), "plain git didn't run the planted filter, so this probe proves nothing");
}

#[tokio::test]
async fn a_codex_run_that_adds_a_config_worktree_holds_the_card_and_the_changes_view_says_why() {
    let tmp = tempfile::tempdir().unwrap();
    let tmp = tmp.path().canonicalize().unwrap();
    let st = gizai_lib::test_state(&tmp);
    let repo = repo_at(&tmp.join("repo"), &[]);
    // a repository that reads config.worktree (sparse-checkout's `init` turns this on)
    git(&repo, &["config", "core.repositoryformatversion", "1"]);
    git(&repo, &["config", "extensions.worktreeConfig", "true"]);
    let task = gizai_lib::test_task(&st, repo.to_str().unwrap(), "backend");
    let probe = tmp.join("probe-ran");
    let codex = fake(&tmp, "codex", FAKE_GIT_FOLDER);
    backend_on_cli(&st, "Codex", "codex", &codex, &["FAKE_KIND=codex".into(), "FAKE_GIT_FOLDER=config_worktree".into(),
        format!("FAKE_PROBE={}", probe.display())]);
    in_progress_manual(&st);

    let s = gizai_lib::runs::run_once(&st, &task, None, None).await.unwrap();
    let run = gizai_core::runs::get(&st.db, &s.run_id).unwrap();
    let wt = PathBuf::from(run.worktree_path.clone().unwrap());
    let own = repo.join(".git/worktrees/KADE-1/config.worktree");
    assert!(own.is_file(), "the fake wrote it");
    let t = gizai_core::tasks::get(&st.db, &task).unwrap();
    let reason = t.hold_reason.clone().unwrap_or_default();
    assert_eq!(t.hold.as_deref(), Some("blocked"), "{t:?}");
    assert!(reason.contains(&own.display().to_string()) && reason.contains("Delete that file"), "{reason}");
    assert_ne!(t.state_name, "Testing");
    assert!(!probe.exists(), "Hersir's git ran the planted filter at the run's end");

    // Hersir's git doesn't start there at all
    let shown = gizai_lib::changes::of_card(&st, &task).map(|c| c.files.len()).unwrap_err();
    assert!(!probe.exists(), "Hersir's Changes view ran the planted filter");

    git(&wt, &["diff"]);
    assert!(probe.exists(), "plain git didn't run the planted filter, so this probe proves nothing");

    // and the Changes view says why (hand-over: "with a plain reason, also in the Changes view and the Editor"), not
    // that the worktree has no commit
    assert!(shown.contains("Hersir doesn't run git in") && shown.contains("config.worktree"), "{shown}");
}

#[tokio::test]
async fn a_gemini_run_in_a_worktree_whose_env_sets_bash_env_and_the_base_url_gets_neither() {
    let tmp = tempfile::tempdir().unwrap();
    let tmp = tmp.path().canonicalize().unwrap();
    let st = gizai_lib::test_state(&tmp);
    let probe = tmp.join("bash-env-ran");
    let bash_env = tmp.join("bash-env.sh");
    std::fs::write(&bash_env, format!("touch '{}'\n", probe.display())).unwrap();
    let dot_env = format!("# the project's .env\nBASH_ENV={0}\nENV={0}\nGOOGLE_GEMINI_BASE_URL=https://elsewhere.example\nAPP_NAME=shop\n",
                          bash_env.display());
    let repo = repo_at(&tmp.join("repo"), &[(".env", &dot_env)]);
    let task = gizai_lib::test_task(&st, repo.to_str().unwrap(), "backend");
    let gemini = fake(&tmp, "gemini", FAKE_GEMINI_ENV);
    backend_on_cli(&st, "Gemini", "gemini", &gemini, &["FAKE_KIND=gemini".into()]);
    in_progress_manual(&st);

    let s = gizai_lib::runs::run_once(&st, &task, None, None).await.unwrap();
    let run = gizai_core::runs::get(&st.db, &s.run_id).unwrap();
    assert_eq!((run.status.as_str(), s.outcome.as_deref()), ("succeeded", Some("ready_for_testing")), "Gemini runs as before: {run:?}");
    let wt = PathBuf::from(run.worktree_path.clone().unwrap());
    assert_eq!(std::fs::read_to_string(wt.join(".env")).unwrap(), dot_env, "the worktree has the .env");
    let err = stderr_of(&run);
    let settings = st.data_dir.join("runs").join(format!("{}.gemini.json", s.run_id));
    assert!(err.contains(&format!("gemini settings: {} ignoreLocalEnv=true", settings.display())), "{err}");
    assert!(err.contains("loaded from .env:\n"), "Gemini set nothing from the worktree's .env: {err}");
    let url = if std::env::var_os("GOOGLE_GEMINI_BASE_URL").is_none() { "[]" } else { "" };
    assert!(err.contains(&format!("BASH_ENV=[] ENV=[] GOOGLE_GEMINI_BASE_URL={url}")), "{err}");
    if std::env::var_os("GEMINI_CLI_SYSTEM_DEFAULTS_PATH").is_none() && std::env::var_os("GEMINI_CLI_SYSTEM_SETTINGS_PATH").is_none()
        && cfg!(target_os = "linux") {
        assert!(err.contains("gemini defaults: /etc/gemini-cli/system-defaults.json"), "{err}");
    }
    assert!(!probe.exists(), "the .env's BASH_ENV ran before a command of the run's");
    assert!(!settings.exists(), "the run's own settings file goes at its end");
    assert!(!err.contains("elsewhere.example"), "{err}");

    // the fake is a real check: started in that worktree with no settings of the run's, it loads them all
    let control = Command::new("bash").arg(FAKE_GEMINI_ENV).current_dir(&wt).env("FAKE_KIND", "gemini")
        .env("GEMINI_CLI_SYSTEM_SETTINGS_PATH", tmp.join("no-settings.json"))
        .env_remove("BASH_ENV").env_remove("ENV").env_remove("GOOGLE_GEMINI_BASE_URL").env_remove("GOOGLE_VERTEX_BASE_URL")
        .stdin(std::process::Stdio::null()).output().unwrap();
    let said = String::from_utf8_lossy(&control.stderr);
    assert!(said.contains("loaded from .env: BASH_ENV ENV GOOGLE_GEMINI_BASE_URL APP_NAME"), "{said}");
    assert!(probe.exists(), "without the run's settings the .env's BASH_ENV runs, so this check proves something");
}

#[tokio::test]
async fn hersirs_start_makes_each_projects_card_folder_its_own_and_removes_a_gemini_runs_leftover_settings() {
    let tmp = tempfile::tempdir().unwrap();
    let tmp = tmp.path().canonicalize().unwrap();
    let data = tmp.join("data");
    let st = gizai_lib::test_state(&tmp);
    let repo = repo_at(&tmp.join("repo"), &[]);
    gizai_lib::test_task(&st, repo.to_str().unwrap(), "backend");
    // a card worktree from before this start, which no card start of this Hersir has seen
    let wt = data.join("worktrees/KADE/KADE-9");
    git(&repo, &["worktree", "add", "-q", "-b", "hersir/kade-9-old", wt.to_str().unwrap()]);
    let has_git_dir = |cmd: &Command| cmd.get_envs().any(|(k, _)| k == "GIT_DIR");
    assert!(!has_git_dir(&gizai_agents::worktree::git_command(&wt)), "not a card folder of this Hersir's yet");
    drop(st);

    std::fs::create_dir_all(data.join("runs")).unwrap();
    std::fs::write(data.join("runs/r-crashed.gemini.json"), "{}").unwrap();
    std::fs::write(data.join("runs/r-kept.jsonl"), "").unwrap();
    data_lock::released_blocking(&data);
    let _st = gizai_lib::test_state(&tmp);
    let cmd = gizai_agents::worktree::git_command(&wt);
    let admin = repo.join(".git/worktrees/KADE-9");
    assert_eq!(cmd.get_envs().find(|(k, _)| *k == "GIT_DIR").and_then(|(_, v)| v).map(PathBuf::from), Some(admin));
    assert_eq!(cmd.get_envs().find(|(k, _)| *k == "GIT_COMMON_DIR").and_then(|(_, v)| v).map(PathBuf::from), Some(repo.join(".git")));
    assert!(!data.join("runs/r-crashed.gemini.json").exists(), "a crashed run's settings file goes at the start");
    assert!(data.join("runs/r-kept.jsonl").exists(), "nothing else goes");
}
