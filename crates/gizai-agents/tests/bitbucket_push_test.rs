//! GA-59: pushes to Bitbucket Cloud without Bitbucket. A fake ssh as your own GIT_SSH_COMMAND (kept, as Gizai keeps one
//! you set) logs its arguments and serves "Bitbucket" from local bare repositories. GA-315: it serves Bitbucket's SSH
//! host git@ssh.bitbucket.org and an SSH host alias of yours (git@bb-alias), and refuses git@bitbucket.org (Bitbucket's
//! old SSH host, which stops taking SSH), so a test shows that nothing goes there any more. Nothing here reaches
//! bitbucket.org or github.com. Every test in this file that runs git or ssh runs that fake ssh.
// Linux and macOS only: these tests run shell or Python scripts as fake programs, which Windows can't start.
#![cfg(unix)]
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use gizai_agents::connection::{self, Problem, PushOver};
use gizai_agents::{behind, worktree};

const LIMIT: Duration = Duration::from_secs(20);

/// Bitbucket's SSH host, where Gizai's pushes, fetches and checks go (GA-315).
const NEW: &str = "git@ssh.bitbucket.org";
/// Bitbucket's old SSH host, which the fake ssh refuses.
const OLD: &str = "git@bitbucket.org";
/// An SSH host alias of yours (its own key), which the fake ssh serves as Bitbucket too.
const ALIAS: &str = "git@bb-alias";

fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").args(args).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap().trim().to_string()
}

fn has_branch(repo: &Path, branch: &str) -> bool {
    Command::new("git").args(["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).current_dir(repo).status().unwrap().success()
}

fn write_script(path: &Path, script: &str) {
    let mut sh = Command::new("sh").args(["-c", r#"cat > "$1" && chmod 755 "$1""#, "sh"]).arg(path).stdin(Stdio::piped()).spawn().unwrap();
    sh.stdin.take().unwrap().write_all(script.as_bytes()).unwrap();
    assert!(sh.wait().unwrap().success(), "can't write {}", path.display());
}

/// The fake ssh's folder: base/ssh-args (one line per call) and "Bitbucket" at base/bitbucket/<workspace>/<repo>(.git)
/// (found with or without .git). It serves git@ssh.bitbucket.org and the alias git@bb-alias, and refuses
/// git@bitbucket.org the way a host that takes no SSH does. A file fake-ssh-mode in a bare repository makes ssh fail
/// for it: publickey or hostkey. `ssh -T` answers the way base/t-mode says: ok, ok-exit1, loggedin, publickey or
/// hostkey. `ssh -G` names the alias's HostName: ssh.bitbucket.org for git@bb-alias, bitbucket.org for git@bb-old.
fn fake_ssh() -> &'static Path {
    static BASE: OnceLock<PathBuf> = OnceLock::new();
    BASE.get_or_init(|| {
        let base = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap().keep();
        let ssh = base.join("bin/ssh");
        std::fs::create_dir_all(ssh.parent().unwrap()).unwrap();
        write_script(&ssh, &format!(r#"#!/bin/sh
base='{base}'
if [ "$1" = "-G" ]; then
  for a in "$@"; do
    case "$a" in
      git@bb-alias|bb-alias) echo "user git"; echo "hostname ssh.bitbucket.org"; echo "port 22"; exit 0 ;;
      git@bb-old|bb-old) echo "user git"; echo "hostname BitBucket.org"; echo "port 22"; exit 0 ;;
    esac
  done
  exit 1
fi
echo "$*" >> "$base/ssh-args"
cmd=""
host=""
for a in "$@"; do
  case "$a" in
    git-receive-pack*|git-upload-pack*) cmd="$a" ;;
    git@ssh.bitbucket.org|git@bb-alias) host="bitbucket" ;;
    git@bitbucket.org) host="old" ;;
  esac
done
if [ "$host" = "old" ]; then
  echo "fake ssh: bitbucket.org takes no SSH any more: $*" >&2; exit 255
fi
if [ "$host" != "bitbucket" ]; then
  echo "fake ssh: only git@ssh.bitbucket.org and git@bb-alias are served here, not: $*" >&2; exit 255
fi
shell="You can use git to connect to Bitbucket. Shell access is disabled"
if [ -z "$cmd" ]; then
  case "$(cat "$base/t-mode" 2>/dev/null)" in
    ok) echo "authenticated via ssh key."; echo; echo "$shell"; exit 0 ;;
    ok-exit1) echo "authenticated via ssh key."; echo; echo "$shell"; exit 1 ;;
    loggedin) echo "logged in as jefsev."; echo; echo "$shell"; exit 0 ;;
    publickey) echo "git@ssh.bitbucket.org: Permission denied (publickey)." >&2; exit 255 ;;
    hostkey) echo "Host key verification failed." >&2; exit 255 ;;
  esac
  echo "fake ssh: no t-mode" >&2; exit 255
fi
path="${{cmd#* }}"
path="$(printf '%s' "$path" | tr -d "'")"
path="${{path#/}}"
repo="$base/bitbucket/$path"
[ -d "$repo" ] || repo="$repo.git"
case "$(cat "$repo/fake-ssh-mode" 2>/dev/null)" in
  publickey) echo "git@ssh.bitbucket.org: Permission denied (publickey)." >&2; exit 255 ;;
  hostkey) echo "Host key verification failed." >&2; exit 255 ;;
esac
verb="${{cmd%% *}}"
exec git "${{verb#git-}}" "$repo"
"#, base = base.display()));
        // SAFETY: set once, before this test binary starts any process (every test that does calls fake_ssh first)
        unsafe { std::env::set_var("GIT_SSH_COMMAND", &ssh) };
        base
    })
}

fn ssh_calls() -> Vec<String> {
    std::fs::read_to_string(fake_ssh().join("ssh-args")).unwrap_or_default().lines().map(String::from).collect()
}

/// The fake ssh's calls about the repository `path` ("acme/shop"), with or without .git.
fn calls_for(path: &str) -> Vec<String> {
    ssh_calls().into_iter().filter(|c| c.contains(&format!("'{path}'")) || c.contains(&format!("'{path}.git'"))
        || c.contains(&format!("'/{path}'")) || c.contains(&format!("'/{path}.git'"))).collect()
}

/// Whether the fake ssh was asked to receive a push into `path` (as git sends it) on git@ssh.bitbucket.org.
fn pushed_over_ssh(path: &str) -> bool {
    ssh_calls().iter().any(|c| has_host(c, NEW) && c.contains(&format!("git-receive-pack '{path}'")))
}

/// Whether the ssh call `call` went to `host` (one of its words).
fn has_host(call: &str, host: &str) -> bool {
    call.split_whitespace().any(|w| w == host)
}

/// Nothing about `path` went to Bitbucket's old SSH host, and something went to `host`.
fn only_to(path: &str, host: &str) {
    let calls = calls_for(path);
    assert!(!calls.is_empty(), "no ssh call for {path}: {:?}", ssh_calls());
    assert!(calls.iter().all(|c| has_host(c, host)), "every call for {path} to {host}: {calls:?}");
    assert!(!calls.iter().any(|c| has_host(c, OLD)), "nothing to {OLD}: {calls:?}");
}

/// "Bitbucket" with a bare repository at base/bitbucket/<bare> behind the fake ssh, and a local clone of it whose remote
/// origin is `origin`, the way a project links to Bitbucket. The clone allows no https, so a push that didn't go over
/// ssh fails. Its branch card has a commit Bitbucket doesn't.
fn bitbucket(tmp: &Path, bare: &str, origin: &str) -> (PathBuf, PathBuf) {
    let base = fake_ssh();
    let src = tmp.join("src");
    std::fs::create_dir(&src).unwrap();
    git(&src, &["init", "-q", "-b", "master"]);
    git(&src, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "init"]);
    let bare = base.join("bitbucket").join(bare);
    git(tmp, &["clone", "-q", "--bare", src.to_str().unwrap(), bare.to_str().unwrap()]);
    let local = tmp.join("local");
    git(tmp, &["clone", "-q", bare.to_str().unwrap(), local.to_str().unwrap()]);
    git(&local, &["remote", "set-url", "origin", origin]);
    git(&local, &["config", "protocol.https.allow", "never"]);
    git(&local, &["checkout", "-q", "-b", "card"]);
    commit(&local, "the card's work");
    git(&local, &["checkout", "-q", "master"]);
    (bare, local)
}

/// A new commit on Bitbucket's master (from the source repository `bitbucket` made), which the clone hasn't fetched.
fn new_on_bitbucket(tmp: &Path, bare: &Path, message: &str) -> String {
    let src = tmp.join("src");
    commit(&src, message);
    git(&src, &["push", "-q", bare.to_str().unwrap(), "HEAD:master"]);
    git(bare, &["rev-parse", "master"])
}

fn commit(local: &Path, message: &str) {
    git(local, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", message]);
}

/// The clone's own git config, as a file and as git lists it.
fn config(local: &Path) -> (String, String) {
    (std::fs::read_to_string(local.join(".git/config")).unwrap(), git(local, &["config", "--list", "--local"]))
}

fn no_github(text: &str) {
    assert!(!text.to_lowercase().contains("github"), "GitHub in a Bitbucket problem: {text}");
}

fn strings(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

// ---- pushes go over SSH to git@ssh.bitbucket.org ----

#[test]
fn a_push_to_a_bitbucket_https_link_goes_over_ssh_to_ssh_bitbucket_org_and_changes_no_config() {
    let tmp = tempfile::tempdir().unwrap();
    let (bare, local) = bitbucket(tmp.path(), "acme/shop.git", "https://bitbucket.org/acme/shop");
    let before = config(&local);
    // through the remote
    worktree::push_branch_over(&local, "origin", "card", &PushOver::Bitbucket).unwrap();
    assert_eq!(git(&bare, &["rev-parse", "card"]), git(&local, &["rev-parse", "card"]), "the branch is on Bitbucket");
    assert!(pushed_over_ssh("acme/shop"), "{:?}", ssh_calls());
    // to the link itself
    git(&local, &["checkout", "-q", "card"]);
    commit(&local, "more");
    worktree::push_branch_over(&local, "https://bitbucket.org/acme/shop", "card", &PushOver::Bitbucket).unwrap();
    assert_eq!(git(&bare, &["rev-parse", "card"]), git(&local, &["rev-parse", "card"]), "the newer commit is on Bitbucket");
    only_to("acme/shop", NEW);
    // nothing changed in the repository's config or remotes
    assert_eq!(config(&local), before);
    assert_eq!(git(&local, &["config", "--get", "remote.origin.url"]), "https://bitbucket.org/acme/shop");
    assert!(!config(&local).1.to_lowercase().contains("insteadof"), "{}", config(&local).1);
    // without PushOver::Bitbucket the link stays https, which this clone doesn't allow: the rewrite is Bitbucket's
    commit(&local, "and more");
    let e = worktree::push_branch_over(&local, "origin", "card", &PushOver::Ssh).unwrap_err().to_string();
    assert!(e.contains("transport 'https' not allowed"), "{e}");
    assert_ne!(git(&bare, &["rev-parse", "card"]), git(&local, &["rev-parse", "card"]));
}

#[test]
fn a_link_with_a_user_name_in_it_goes_over_ssh_too() {
    let tmp = tempfile::tempdir().unwrap();
    // what Bitbucket's Clone button gives
    let (bare, local) = bitbucket(tmp.path(), "acme/web.git", "https://jefsev@bitbucket.org/acme/web.git");
    let before = config(&local);
    worktree::push_branch_over(&local, "origin", "card", &PushOver::Bitbucket).unwrap();
    assert_eq!(git(&bare, &["rev-parse", "card"]), git(&local, &["rev-parse", "card"]), "the branch is on Bitbucket");
    assert!(pushed_over_ssh("acme/web.git"), "{:?}", ssh_calls());
    // and to that link itself
    git(&local, &["checkout", "-q", "card"]);
    commit(&local, "more");
    worktree::push_branch_over(&local, "https://jefsev@bitbucket.org/acme/web.git", "card", &PushOver::Bitbucket).unwrap();
    assert_eq!(git(&bare, &["rev-parse", "card"]), git(&local, &["rev-parse", "card"]));
    only_to("acme/web", NEW);
    assert_eq!(config(&local), before, "no git config or remote changed");
    assert_eq!(git(&local, &["config", "--get", "remote.origin.url"]), "https://jefsev@bitbucket.org/acme/web.git");
}

#[test]
fn a_push_url_with_a_user_name_goes_over_ssh_too() {
    let tmp = tempfile::tempdir().unwrap();
    // fetches over ssh from a repository that isn't there, pushes to the user@ link: only the push URL can work
    let (bare, local) = bitbucket(tmp.path(), "acme/pushurl.git", "git@bitbucket.org:acme/elsewhere.git");
    git(&local, &["remote", "set-url", "--push", "origin", "https://tjitske@bitbucket.org/acme/pushurl.git"]);
    let before = config(&local);
    worktree::push_branch_over(&local, "origin", "card", &PushOver::Bitbucket).unwrap();
    assert_eq!(git(&bare, &["rev-parse", "card"]), git(&local, &["rev-parse", "card"]), "the branch is on Bitbucket");
    assert!(pushed_over_ssh("acme/pushurl.git"), "{:?}", ssh_calls());
    only_to("acme/pushurl", NEW);
    assert_eq!(config(&local), before, "no git config or remote changed");
    assert_eq!(git(&local, &["config", "--get", "remote.origin.pushurl"]), "https://tjitske@bitbucket.org/acme/pushurl.git");
    assert_eq!(git(&local, &["config", "--get", "remote.origin.url"]), "git@bitbucket.org:acme/elsewhere.git");
}

/// GA-315: a remote on Bitbucket's old SSH host (git@bitbucket.org:…, ssh://git@bitbucket.org/…), or one already on its
/// new one, pushes to git@ssh.bitbucket.org: the checkout keeps working without any change, and nothing goes to the old
/// host.
#[test]
fn a_remote_on_the_old_or_the_new_ssh_host_pushes_to_ssh_bitbucket_org_and_its_config_stays() {
    for (i, origin) in ["git@bitbucket.org:acme/old-scp.git", "ssh://git@bitbucket.org/acme/old-url.git",
                        "git@ssh.bitbucket.org:acme/new-scp.git", "ssh://git@ssh.bitbucket.org/acme/new-url.git"].into_iter().enumerate() {
        let tmp = tempfile::tempdir().unwrap();
        let path = origin.rsplit_once(['/', ':']).map(|(_, r)| r).unwrap().trim_end_matches(".git");
        let path = format!("acme/{path}");
        let (bare, local) = bitbucket(tmp.path(), &format!("{path}.git"), origin);
        let before = config(&local);
        // through the remote: Open pull request and the push after a run
        worktree::push_branch_over(&local, "origin", "card", &PushOver::Bitbucket).unwrap_or_else(|e| panic!("{origin}: {e}"));
        assert_eq!(git(&bare, &["rev-parse", "card"]), git(&local, &["rev-parse", "card"]), "{origin}: the branch is on Bitbucket");
        // to the address itself
        git(&local, &["checkout", "-q", "card"]);
        commit(&local, "more");
        worktree::push_branch_over(&local, origin, "card", &PushOver::Bitbucket).unwrap_or_else(|e| panic!("{origin}: {e}"));
        assert_eq!(git(&bare, &["rev-parse", "card"]), git(&local, &["rev-parse", "card"]), "{origin}: the newer commit");
        // Check connection's dry run
        worktree::can_push(&local, "origin", &PushOver::Bitbucket, LIMIT).unwrap_or_else(|p| panic!("{origin}: {p}"));
        assert!(pushed_over_ssh(&path) || pushed_over_ssh(&format!("{path}.git")) || pushed_over_ssh(&format!("/{path}.git")), "{i} {origin}: {:?}", ssh_calls());
        only_to(&path, NEW);
        assert_eq!(config(&local), before, "{origin}: no git config or remote changed");
        assert_eq!(git(&local, &["config", "--get", "remote.origin.url"]), origin);
        // where git itself says the push goes, with Gizai's settings (an ssh:// URL already on the new host stays one)
        let want = if origin.starts_with("ssh://git@ssh.") { origin.to_string() } else { format!("{NEW}:{path}.git") };
        assert_eq!(worktree::push_url(&local, "origin", &PushOver::Bitbucket), Some(want), "{origin}");
    }
    // without Gizai's settings, git@bitbucket.org is refused here: the old host takes no SSH
    let tmp = tempfile::tempdir().unwrap();
    let (bare, local) = bitbucket(tmp.path(), "acme/old-plain.git", "git@bitbucket.org:acme/old-plain.git");
    let o = Command::new("git").args(["push", "-q", "origin", "card"]).current_dir(&local).env("GIT_TERMINAL_PROMPT", "0").output().unwrap();
    assert!(!o.status.success(), "the fake refuses the old host");
    assert!(String::from_utf8_lossy(&o.stderr).contains("bitbucket.org takes no SSH any more"), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(!has_branch(&bare, "card"));
}

#[test]
fn a_link_with_a_password_in_it_is_left_alone_and_its_password_never_shows() {
    let link = "https://jefsev:s3cretPASS@bitbucket.org/acme/pw.git";
    // no rule for it: its password never goes in a command line
    let rules = PushOver::Bitbucket.git_config_for(&[link.to_string()]);
    assert!(rules.iter().all(|r| !r.contains("s3cretPASS") && !r.contains("jefsev")), "{rules:?}");
    assert!(PushOver::Bitbucket.git_config_for(&["https://jefsev@bitbucket.org/acme/web.git".to_string()])
        .contains(&"url.git@ssh.bitbucket.org:.insteadOf=https://jefsev@bitbucket.org/".to_string()), "a user name alone gets one");

    let tmp = tempfile::tempdir().unwrap();
    let (bare, local) = bitbucket(tmp.path(), "acme/pw.git", link);
    let e = worktree::push_branch_over(&local, "origin", "card", &PushOver::Bitbucket).unwrap_err().to_string();
    assert!(e.contains("Couldn't push card to origin: transport 'https' not allowed"), "it stayed https, a plain message: {e}");
    assert!(!e.contains("s3cretPASS"), "{e}");
    let p = worktree::can_push(&local, "origin", &PushOver::Bitbucket, LIMIT).unwrap_err();
    assert!(!p.to_string().contains("s3cretPASS") && !format!("{p:?}").contains("s3cretPASS"), "{p:?}");
    assert!(!ssh_calls().iter().any(|c| c.contains("acme/pw")), "not over ssh: {:?}", ssh_calls());
    assert!(!has_branch(&bare, "card"), "nothing pushed");
}

/// GA-315: Gizai's push rules send every https form and both old SSH forms to git@ssh.bitbucket.org:, and none goes to
/// git@bitbucket.org: any more. GitHub's are as they were.
#[test]
fn the_bitbucket_push_rules_all_go_to_ssh_bitbucket_org_and_githubs_are_unchanged() {
    let rules = PushOver::Bitbucket.git_config();
    let want: Vec<String> = ["https://bitbucket.org/", "http://bitbucket.org/", "https://www.bitbucket.org/", "http://www.bitbucket.org/",
                             "git@bitbucket.org:", "ssh://git@bitbucket.org/"].iter()
        .map(|from| format!("url.git@ssh.bitbucket.org:.insteadOf={from}")).collect();
    assert_eq!(rules, want);
    assert_eq!(connection::BITBUCKET_SSH_HOST, "ssh.bitbucket.org");
    assert_eq!(connection::bitbucket_ssh_destination(), NEW);
    assert!(!rules.iter().any(|r| r.starts_with("url.git@bitbucket.org:")), "{rules:?}");
    assert_eq!(PushOver::Ssh.git_config(), ["https://github.com/", "http://github.com/", "https://www.github.com/", "http://www.github.com/"]
        .iter().map(|from| format!("url.git@github.com:.insteadOf={from}")).collect::<Vec<_>>(), "GitHub's are as they were");
    assert_eq!(PushOver::Ssh.git_config_in(&strings(&["git@bitbucket.org:acme/x.git"]), &[]), PushOver::Ssh.git_config());
    // a rule of yours for the old host's start decides: Gizai adds none for it
    let own = strings(&["git@bitbucket.org:"]);
    let rules = PushOver::Bitbucket.git_config_in(&strings(&["git@bitbucket.org:acme/x.git"]), &own);
    assert!(!rules.iter().any(|r| r.ends_with("=git@bitbucket.org:")), "{rules:?}");
    assert!(rules.contains(&"url.git@ssh.bitbucket.org:.insteadOf=ssh://git@bitbucket.org/".to_string()), "the other start still goes: {rules:?}");
    // a longer rule of yours (one workspace, as in Jefsev's global config) still leaves Gizai's: git takes the longest
    let rules = PushOver::Bitbucket.git_config_in(&[], &strings(&["git@bitbucket.org:snxo2021/"]));
    assert!(rules.contains(&"url.git@ssh.bitbucket.org:.insteadOf=git@bitbucket.org:".to_string()), "{rules:?}");
}

#[test]
fn the_old_ssh_address_and_ssh_destination_helpers_read_each_form() {
    use connection::{bitbucket_ssh_address, fetch_config, ssh_destination};
    assert_eq!(bitbucket_ssh_address("git@bitbucket.org:acme/shop.git").as_deref(), Some("git@ssh.bitbucket.org:acme/shop.git"));
    assert_eq!(bitbucket_ssh_address(" ssh://git@bitbucket.org/acme/shop.git ").as_deref(), Some("git@ssh.bitbucket.org:acme/shop.git"));
    for not_old in ["git@ssh.bitbucket.org:acme/shop.git", "https://bitbucket.org/acme/shop", "git@bitbucket.org:", "git@github.com:acme/shop.git",
                    "git@bitbucket-snx:acme/shop.git"] {
        assert_eq!(bitbucket_ssh_address(not_old), None, "{not_old}");
    }
    for (address, to) in [("git@ssh.bitbucket.org:acme/shop.git", Some("git@ssh.bitbucket.org")),
                          ("ssh://git@ssh.bitbucket.org/acme/shop.git", Some("git@ssh.bitbucket.org")),
                          ("git@bitbucket-snx:snxo2021/sdh.git", Some("git@bitbucket-snx")),
                          ("bitbucket-snx:snxo2021/sdh.git", Some("bitbucket-snx")),
                          ("ssh://git@host.example:2222/acme/x.git", Some("ssh://git@host.example:2222")),
                          ("git+ssh://git@ssh.bitbucket.org/acme/x.git", Some("git@ssh.bitbucket.org")),
                          ("https://bitbucket.org/acme/shop", None), ("https://jefsev@bitbucket.org/acme/shop.git", None),
                          ("/home/me/shop", None), ("file:///home/me/shop", None), ("./a:b", None), ("C:/repos/shop", None)] {
        assert_eq!(ssh_destination(address).as_deref(), to, "{address}");
    }
    // a fetch's settings: the old host's rules only for an address on it, and none for a start your own rule decides
    let both = strings(&["url.git@ssh.bitbucket.org:.insteadOf=git@bitbucket.org:", "url.git@ssh.bitbucket.org:.insteadOf=ssh://git@bitbucket.org/"]);
    assert_eq!(fetch_config(&strings(&["origin", "git@bitbucket.org:acme/x.git"]), &[]), both);
    assert_eq!(fetch_config(&strings(&["ssh://git@bitbucket.org/acme/x.git"]), &strings(&["git@bitbucket.org:snxo2021/"])), both);
    assert_eq!(fetch_config(&strings(&["git@bitbucket.org:acme/x.git"]), &strings(&["git@bitbucket.org:"])), both[1..].to_vec());
    assert_eq!(fetch_config(&strings(&["git@bitbucket.org:acme/x.git"]), &strings(&["git@bitbucket.org:", "ssh://git@"])), Vec::<String>::new());
    for untouched in ["git@ssh.bitbucket.org:acme/x.git", "https://bitbucket.org/acme/x", "git@github.com:acme/x.git", "https://github.com/acme/x"] {
        assert_eq!(fetch_config(&strings(&["origin", untouched]), &[]), Vec::<String>::new(), "{untouched}");
    }
}

// ---- GA-315: fetches go to ssh.bitbucket.org too ----

/// A card start, the Chief's copies (`fetch_start`), the 'linked folder is outdated' check (`behind::fetch_origin_main`,
/// with `remote_head`) and the PR check (`ls_remote`) through a remote that still names git@bitbucket.org: all reach
/// Bitbucket over git@ssh.bitbucket.org, and the remote stays as it is.
#[test]
fn fetches_and_the_pr_checks_ls_remote_through_an_old_host_remote_go_to_ssh_bitbucket_org() {
    for origin in ["git@bitbucket.org:acme/fetch-scp.git", "ssh://git@bitbucket.org/acme/fetch-url.git"] {
        let tmp = tempfile::tempdir().unwrap();
        let path = format!("acme/{}", origin.rsplit_once('/').or_else(|| origin.rsplit_once(':')).unwrap().1.trim_end_matches(".git"));
        let (bare, local) = bitbucket(tmp.path(), &format!("{path}.git"), origin);
        let before = config(&local);
        let new = new_on_bitbucket(tmp.path(), &bare, "on bitbucket");
        // card start through the remote, and from the address itself
        assert_eq!(worktree::fetch_start(&local, Some("origin"), "https://bitbucket.org/x/y", "master", LIMIT).unwrap(), "refs/remotes/origin/master");
        assert_eq!(git(&local, &["rev-parse", "refs/remotes/origin/master"]), new, "{origin}");
        let newer = new_on_bitbucket(tmp.path(), &bare, "newer");
        assert_eq!(worktree::fetch_start(&local, None, origin, "master", LIMIT).unwrap(), "refs/gizai/base/master");
        assert_eq!(git(&local, &["rev-parse", "refs/gizai/base/master"]), newer, "{origin}");
        // the linked folder's check: asks origin which branch is main, then fetches it
        assert_eq!(worktree::remote_head(&local, "origin", LIMIT).unwrap().as_deref(), Some("master"));
        let _ = Command::new("git").args(["remote", "set-head", "origin", "-d"]).current_dir(&local).output();
        let newest = new_on_bitbucket(tmp.path(), &bare, "newest");
        let main = behind::fetch_origin_main(&local, LIMIT).unwrap_or_else(|e| panic!("{origin}: {e}"));
        assert_eq!((main.rev.as_str(), main.branch.as_str()), ("refs/remotes/origin/master", "master"));
        assert_eq!(git(&local, &["rev-parse", "refs/remotes/origin/master"]), newest);
        // the PR check's ls-remote
        let refs = worktree::ls_remote(&local, "origin", &strings(&["refs/heads/master"]), &PushOver::Bitbucket, LIMIT).unwrap();
        assert_eq!(refs, vec![("refs/heads/master".to_string(), newest.clone())], "{origin}");
        only_to(&path, NEW);
        assert!(calls_for(&path).iter().any(|c| c.contains("git-upload-pack")), "{:?}", calls_for(&path));
        let after = config(&local);
        assert_eq!(git(&local, &["config", "--get", "remote.origin.url"]), origin, "the remote is left as it was");
        assert!(!after.1.to_lowercase().contains("insteadof"), "no rule was written: {}", after.1);
        assert_eq!(after, before, "{origin}: no git config changed");
    }
}

#[test]
fn a_remote_already_on_ssh_bitbucket_org_or_on_github_fetches_as_it_is_written() {
    let tmp = tempfile::tempdir().unwrap();
    let (bare, local) = bitbucket(tmp.path(), "acme/already.git", "git@ssh.bitbucket.org:acme/already.git");
    assert_eq!(worktree::fetch_config(&local, "origin"), Vec::<String>::new(), "nothing to rewrite");
    let new = new_on_bitbucket(tmp.path(), &bare, "on bitbucket");
    worktree::fetch_start(&local, Some("origin"), "https://bitbucket.org/acme/already", "master", LIMIT).unwrap();
    assert_eq!(git(&local, &["rev-parse", "refs/remotes/origin/master"]), new);
    only_to("acme/already", NEW);
    // GitHub, over ssh or https: no setting at all, as before
    git(&local, &["remote", "add", "gh", "git@github.com:acme/already.git"]);
    git(&local, &["remote", "add", "gh-https", "https://github.com/acme/already"]);
    for from in ["gh", "gh-https", "git@github.com:acme/already.git", "https://github.com/acme/already"] {
        assert_eq!(worktree::fetch_config(&local, from), Vec::<String>::new(), "{from}");
    }
    // and an old-host remote gets the rules, for its name and for its address
    git(&local, &["remote", "add", "old", "git@bitbucket.org:acme/already.git"]);
    assert_eq!(worktree::fetch_config(&local, "old").len(), 2);
    assert_eq!(worktree::fetch_config(&local, "ssh://git@bitbucket.org/acme/already.git").len(), 2);
}

// ---- GA-315: a rule of your own to an SSH host alias decides ----

/// Jefsev's setup: his own rule sends git@bitbucket.org: to his SSH host alias (bitbucket-snx, a key of its own). Gizai
/// adds no rule for that start, so pushes, fetches, the PR check and Check connection's dry run go to the alias, not to
/// git@ssh.bitbucket.org. (In the repository's own config here; a global one is read the same way, `own_rewrites`.)
#[test]
fn your_own_rule_to_an_ssh_alias_sends_pushes_and_fetches_there_not_to_ssh_bitbucket_org() {
    let tmp = tempfile::tempdir().unwrap();
    let (bare, local) = bitbucket(tmp.path(), "snx/sdh.git", "git@bitbucket.org:snx/sdh.git");
    git(&local, &["config", "url.git@bb-alias:.insteadOf", "git@bitbucket.org:"]);
    let before = config(&local);
    assert!(worktree::own_rewrites(&local).contains(&"git@bitbucket.org:".to_string()));
    assert!(!worktree::fetch_config(&local, "origin").iter().any(|r| r.ends_with("=git@bitbucket.org:")), "{:?}", worktree::fetch_config(&local, "origin"));
    // push (Open pull request, the push after a run)
    worktree::push_branch_over(&local, "origin", "card", &PushOver::Bitbucket).unwrap();
    assert_eq!(git(&bare, &["rev-parse", "card"]), git(&local, &["rev-parse", "card"]));
    // Check connection's dry run, and where it says the push goes
    worktree::can_push(&local, "origin", &PushOver::Bitbucket, LIMIT).unwrap();
    assert_eq!(worktree::push_url(&local, "origin", &PushOver::Bitbucket).as_deref(), Some("git@bb-alias:snx/sdh.git"));
    // fetches: a card start, the linked folder's check, the PR check
    let new = new_on_bitbucket(tmp.path(), &bare, "on bitbucket");
    worktree::fetch_start(&local, Some("origin"), "https://bitbucket.org/snx/sdh", "master", LIMIT).unwrap();
    assert_eq!(git(&local, &["rev-parse", "refs/remotes/origin/master"]), new);
    assert_eq!(worktree::remote_head(&local, "origin", LIMIT).unwrap().as_deref(), Some("master"));
    let refs = worktree::ls_remote(&local, "origin", &strings(&["refs/heads/master"]), &PushOver::Bitbucket, LIMIT).unwrap();
    assert_eq!(refs, vec![("refs/heads/master".to_string(), new)]);
    only_to("snx/sdh", ALIAS);
    assert!(!calls_for("snx/sdh").iter().any(|c| has_host(c, NEW)), "not to {NEW}: {:?}", calls_for("snx/sdh"));
    assert_eq!(config(&local), before, "no git config or remote changed");
}

/// A rule of yours for one workspace only (as in Jefsev's global config: git@bitbucket.org:snxo2021/): that workspace
/// goes to your alias, another one still to git@ssh.bitbucket.org.
#[test]
fn a_rule_of_yours_for_one_workspace_leaves_the_others_to_ssh_bitbucket_org() {
    let tmp = tempfile::tempdir().unwrap();
    let (bare, local) = bitbucket(tmp.path(), "snxws/dove.git", "git@bitbucket.org:snxws/dove.git");
    git(&local, &["config", "url.git@bb-alias:snxws/.insteadOf", "git@bitbucket.org:snxws/"]);
    let other_bare = fake_ssh().join("bitbucket/acmews/other.git");
    git(tmp.path(), &["clone", "-q", "--bare", bare.to_str().unwrap(), other_bare.to_str().unwrap()]);
    git(&local, &["remote", "add", "other", "git@bitbucket.org:acmews/other.git"]);
    worktree::push_branch_over(&local, "origin", "card", &PushOver::Bitbucket).unwrap();
    worktree::push_branch_over(&local, "other", "card", &PushOver::Bitbucket).unwrap();
    assert_eq!(git(&other_bare, &["rev-parse", "card"]), git(&local, &["rev-parse", "card"]));
    worktree::fetch_start(&local, Some("other"), "https://bitbucket.org/acmews/other", "master", LIMIT).unwrap();
    only_to("snxws/dove", ALIAS);
    only_to("acmews/other", NEW);
}

// ---- GA-315: a task run's own git ----

/// git as an agent runs it in a task run: the run's settings in its environment (`git_env`).
fn agent_git(dir: &Path, settings: &[(String, String)], args: &[&str]) -> std::process::Output {
    Command::new("git").args(args).current_dir(dir).envs(connection::git_env(settings)).env("GIT_TERMINAL_PROMPT", "0").output().unwrap()
}

/// In a task run of a Bitbucket project, the agent's own `git fetch` and `git push` of the project's repository reach
/// git@ssh.bitbucket.org, also through a remote that still names git@bitbucket.org; another repository's address stays
/// as it is written (and so goes nowhere here).
#[test]
fn in_a_task_run_the_agents_fetch_and_push_of_the_projects_repository_go_to_ssh_bitbucket_org() {
    let tmp = tempfile::tempdir().unwrap();
    let (bare, local) = bitbucket(tmp.path(), "acme/run.git", "git@bitbucket.org:acme/run.git");
    git(&local, &["remote", "add", "upstream", "ssh://git@bitbucket.org/acme/run.git"]);
    git(&local, &["remote", "add", "web", "https://bitbucket.org/acme/run"]);
    git(&local, &["remote", "add", "other", "git@bitbucket.org:acme/other-run.git"]);
    let before = config(&local);
    // as pulls::run_git_config passes them: the link, and each remote that is the same repository
    let c = PushOver::Bitbucket.run_config("acme/run", &strings(&["https://bitbucket.org/acme/run", "git@bitbucket.org:acme/run.git",
        "ssh://git@bitbucket.org/acme/run.git", "https://bitbucket.org/acme/run"]), &worktree::own_rewrites(&local));
    let new = new_on_bitbucket(tmp.path(), &bare, "on bitbucket");
    for remote in ["origin", "upstream"] {
        let o = agent_git(&local, &c, &["fetch", "-q", remote]);
        assert!(o.status.success(), "{remote}: {}", String::from_utf8_lossy(&o.stderr));
        assert_eq!(git(&local, &["rev-parse", &format!("refs/remotes/{remote}/master")]), new);
    }
    git(&local, &["checkout", "-q", "card"]);
    for remote in ["origin", "upstream", "web"] {
        commit(&local, remote);
        let o = agent_git(&local, &c, &["push", "-q", remote, "card"]);
        assert!(o.status.success(), "{remote}: {}", String::from_utf8_lossy(&o.stderr));
        assert_eq!(git(&bare, &["rev-parse", "card"]), git(&local, &["rev-parse", "card"]), "{remote}");
    }
    only_to("acme/run", NEW);
    // the addresses git uses in the run
    let url = |args: &[&str]| String::from_utf8(agent_git(&local, &c, args).stdout).unwrap().trim().to_string();
    assert_eq!(url(&["remote", "get-url", "origin"]), "git@ssh.bitbucket.org:acme/run.git");
    assert_eq!(url(&["remote", "get-url", "--push", "upstream"]), "git@ssh.bitbucket.org:acme/run.git");
    assert_eq!(url(&["remote", "get-url", "--push", "web"]), "git@ssh.bitbucket.org:acme/run.git");
    assert_eq!(url(&["remote", "get-url", "web"]), "https://bitbucket.org/acme/run", "an https remote still fetches as written");
    assert_eq!(url(&["remote", "get-url", "other"]), "git@bitbucket.org:acme/other-run.git", "another repository stays as written");
    assert_eq!(url(&["remote", "get-url", "--push", "other"]), "git@bitbucket.org:acme/other-run.git");
    assert_eq!(config(&local), before, "the run's settings are in its environment only");
}

/// The same run with your own rule to an SSH host alias: no setting for the old host, so the agent's git goes to your
/// alias.
#[test]
fn in_a_task_run_your_own_rule_to_an_alias_decides() {
    let tmp = tempfile::tempdir().unwrap();
    let (bare, local) = bitbucket(tmp.path(), "snx/run-alias.git", "git@bitbucket.org:snx/run-alias.git");
    git(&local, &["config", "url.git@bb-alias:.insteadOf", "git@bitbucket.org:"]);
    let c = PushOver::Bitbucket.run_config("snx/run-alias", &strings(&["https://bitbucket.org/snx/run-alias", "git@bitbucket.org:snx/run-alias.git"]),
                                           &worktree::own_rewrites(&local));
    assert!(!c.iter().any(|(k, v)| k.ends_with(".insteadOf") && v.starts_with("git@bitbucket.org:")), "{c:?}");
    let new = new_on_bitbucket(tmp.path(), &bare, "on bitbucket");
    let o = agent_git(&local, &c, &["fetch", "-q", "origin"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(git(&local, &["rev-parse", "refs/remotes/origin/master"]), new);
    git(&local, &["checkout", "-q", "card"]);
    let o = agent_git(&local, &c, &["push", "-q", "origin", "card"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(git(&bare, &["rev-parse", "card"]), git(&local, &["rev-parse", "card"]));
    only_to("snx/run-alias", ALIAS);
}

// ---- a failed push says what to check, in Bitbucket's terms ----

#[test]
fn a_key_or_host_key_bitbucket_refuses_is_said_in_bitbucket_terms_and_pushes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let (bare, local) = bitbucket(tmp.path(), "acme/locked.git", "https://bitbucket.org/acme/locked");
    std::fs::write(bare.join("fake-ssh-mode"), "publickey").unwrap();
    let e = worktree::push_branch_over(&local, "origin", "card", &PushOver::Bitbucket).unwrap_err().to_string();
    assert!(e.contains("Couldn't push card to origin: Bitbucket didn't accept your SSH key."), "{e}");
    assert!(e.contains("Bitbucket → Personal settings → SSH keys"), "{e}");
    no_github(&e);
    std::fs::write(bare.join("fake-ssh-mode"), "hostkey").unwrap();
    let e = worktree::push_branch_over(&local, "origin", "card", &PushOver::Bitbucket).unwrap_err().to_string();
    assert!(e.contains("This computer doesn't trust Bitbucket's SSH host key yet. Run ssh -T git@ssh.bitbucket.org once in a terminal and answer yes."), "{e}");
    no_github(&e);
    assert!(pushed_over_ssh("acme/locked"), "{:?}", ssh_calls());
    only_to("acme/locked", NEW);
    assert!(!has_branch(&bare, "card"), "nothing pushed");
}

#[test]
fn a_failed_push_to_bitbucket_says_what_to_check_in_bitbucket_terms() {
    let bb = PushOver::Bitbucket;
    let tail = "fatal: Could not read from remote repository.\n\nPlease make sure you have the correct access rights\nand the repository exists.\n";
    let cases = [
        (format!("git@ssh.bitbucket.org: Permission denied (publickey).\n{tail}"), "Bitbucket didn't accept your SSH key"),
        (format!("Host key verification failed.\n{tail}"), "This computer doesn't trust Bitbucket's SSH host key yet"),
        (format!("conq: repository does not exist.\n{tail}"), "Bitbucket has no repository at this address that your account can see"),
        (format!("remote: Repository not found\n{tail}"), "Bitbucket has no repository at this address that your account can see"),
        (format!("conq: repository access denied. access via a deployment key is read-only.\n{tail}"), "Your Bitbucket account can't push to this repository"),
        (" ! [rejected]        card -> card (fetch first)\nerror: failed to push some refs to 'ssh.bitbucket.org:acme/shop.git'\n".to_string(),
         "The branch on Bitbucket has commits this one doesn't"),
        // git's own words over HTTPS (which stays on bitbucket.org)
        ("fatal: unable to access 'https://bitbucket.org/acme/shop.git/': Could not resolve host: bitbucket.org\n".to_string(), "Can't reach bitbucket.org over HTTPS"),
        (format!("ssh: Could not resolve hostname ssh.bitbucket.org: Name or service not known\n{tail}"), "Can't reach ssh.bitbucket.org over SSH"),
        ("fatal: could not read Username for 'https://bitbucket.org': terminal prompts disabled\n".to_string(), "git tried HTTPS, and has no login for it"),
        ("error: cannot run ssh: No such file or directory\nfatal: unable to fork\n".to_string(), "ssh isn't installed"),
        ("error: src refspec card does not match any\nerror: failed to push some refs to 'ssh.bitbucket.org:acme/shop.git'\n".to_string(),
         "The repository has no commits yet"),
        (format!("ssh: connect to host ssh.bitbucket.org port 22: Connection timed out\n{tail}"), "Can't reach ssh.bitbucket.org over SSH"),
        (format!("kex_exchange_identification: Connection closed by remote host\n{tail}"), "Can't reach ssh.bitbucket.org over SSH"),
    ];
    for (said, what) in &cases {
        let p = connection::push_problem(said, &bb);
        assert_eq!(p.what, *what, "{said}");
        assert!(p.fix.is_some(), "a fix for: {said}");
        no_github(&p.to_string());
        // GA-315: no problem text sends you to the old SSH host
        assert!(!p.to_string().contains("git@bitbucket.org"), "{p}");
        assert!(!p.to_string().contains("bitbucket.org over SSH") || p.to_string().contains("ssh.bitbucket.org over SSH"), "{p}");
    }
    let fix = |said: &str| connection::push_problem(said, &bb).fix.unwrap();
    let key = fix(&cases[0].0);
    assert!(key.contains("Bitbucket → Personal settings → SSH keys") && key.contains("ssh-add"), "{key}");
    assert_eq!(fix(&cases[1].0), "Run ssh -T git@ssh.bitbucket.org once in a terminal and answer yes.");
    assert!(fix(&cases[2].0).contains("Bitbucket link"));
    let rejected = fix(&cases[5].0);
    assert!(rejected.contains("never forces") && rejected.contains("Bitbucket's copy"), "{rejected}");
    // the texts that name where Hersir pushes name ssh.bitbucket.org
    for i in [6, 8, 9] {
        let f = fix(&cases[i].0);
        assert!(f.contains("over SSH") && f.contains("ssh.bitbucket.org"), "{}: {f}", cases[i].1);
    }
    assert!(fix(&cases[9].0).contains("Bitbucket over SSH"));
    // anything else: git's own telling line
    let p = connection::push_problem("fatal: 'nowhere' does not appear to be a git repository\nfatal: Could not read from remote repository.\n", &bb);
    assert_eq!(p, Problem::plain("'nowhere' does not appear to be a git repository"));
    // no answer in time
    assert_eq!(connection::push_no_answer(&bb, Duration::from_secs(2)).to_string(),
               "Bitbucket gave no answer within 2 seconds. Check your internet connection, then try again.");
}

#[test]
fn the_same_words_from_a_push_to_github_still_speak_github() {
    let ssh = PushOver::Ssh;
    let p = connection::push_problem("git@github.com: Permission denied (publickey).\nfatal: Could not read from remote repository.\n", &ssh);
    assert_eq!(p.what, "GitHub didn't accept your SSH key");
    assert!(p.fix.unwrap().contains("github.com/settings/keys"));
    // even with Bitbucket's own words in it, a push over PushOver::Ssh is GitHub's
    let p = connection::push_problem("git@ssh.bitbucket.org: Permission denied (publickey).\n", &ssh);
    assert_eq!(p.what, "GitHub didn't accept your SSH key");
    let p = connection::push_problem("Host key verification failed.\n", &ssh);
    assert!(p.fix.unwrap().contains("ssh -T git@github.com"));
    let p = connection::push_problem("ERROR: Repository not found.\n", &ssh);
    assert_eq!(p.what, "GitHub has no repository at this address that your account can see");
    assert_eq!(connection::push_no_answer(&ssh, Duration::from_secs(2)).what, "GitHub gave no answer within 2 seconds");
}

// ---- Check connection: a dry run, and ssh -T git@ssh.bitbucket.org ----

#[test]
fn can_push_to_bitbucket_is_a_dry_run_that_sends_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let (bare, local) = bitbucket(tmp.path(), "acme/check.git", "https://bitbucket.org/acme/check");
    let before = config(&local);
    worktree::can_push(&local, "origin", &PushOver::Bitbucket, LIMIT).unwrap();
    assert!(pushed_over_ssh("acme/check"), "{:?}", ssh_calls());
    assert!(!has_branch(&bare, "gizai-connection-check") && !has_branch(&bare, "card"), "nothing was pushed");
    assert_eq!(config(&local), before);
    // a key Bitbucket refuses: in its terms
    std::fs::write(bare.join("fake-ssh-mode"), "publickey").unwrap();
    let p = worktree::can_push(&local, "origin", &PushOver::Bitbucket, LIMIT).unwrap_err();
    assert_eq!(p.what, "Bitbucket didn't accept your SSH key");
    assert!(p.fix.as_deref().is_some_and(|f| f.contains("Bitbucket → Personal settings → SSH keys")), "{p:?}");
    no_github(&p.to_string());
    assert!(!has_branch(&bare, "gizai-connection-check"));
    only_to("acme/check", NEW);
}

/// One test at a time may use base/t-mode.
static T_MODE: Mutex<()> = Mutex::new(());

fn t_mode(mode: &str) {
    std::fs::write(fake_ssh().join("t-mode"), mode).unwrap();
}

#[test]
fn the_bitbucket_ssh_check_goes_by_bitbucket_words_not_the_exit_code() {
    let _one = T_MODE.lock().unwrap_or_else(|e| e.into_inner());
    // (only the tests that hold T_MODE run ssh -T: the calls from here on are this test's)
    let before = ssh_calls().len();
    t_mode("ok");
    assert_eq!(connection::bitbucket_ssh_check(LIMIT), Ok(None));
    t_mode("ok-exit1");
    assert_eq!(connection::bitbucket_ssh_check(LIMIT), Ok(None), "accepted although ssh ended with 1");
    t_mode("loggedin");
    assert_eq!(connection::bitbucket_ssh_check(LIMIT), Ok(Some("jefsev".into())));
    let mine: Vec<String> = ssh_calls()[before..].iter().filter(|c| c.starts_with("-T ")).cloned().collect();
    assert_eq!(mine, ["-T git@ssh.bitbucket.org"; 3], "each check asks Bitbucket's SSH host, never git@bitbucket.org");
}

#[test]
fn the_bitbucket_ssh_check_says_where_the_key_goes_and_how_to_trust_the_host() {
    let _one = T_MODE.lock().unwrap_or_else(|e| e.into_inner());
    t_mode("publickey");
    let p = connection::bitbucket_ssh_check(LIMIT).unwrap_err();
    assert_eq!(p.what, "Bitbucket didn't accept your SSH key");
    assert!(p.fix.as_deref().is_some_and(|f| f.contains("Personal settings → SSH keys")), "{p:?}");
    no_github(&p.to_string());
    t_mode("hostkey");
    let p = connection::bitbucket_ssh_check(LIMIT).unwrap_err();
    assert_eq!(p.what, "This computer doesn't trust Bitbucket's SSH host key yet");
    assert_eq!(p.fix.as_deref(), Some("Run ssh -T git@ssh.bitbucket.org once in a terminal and answer yes."), "{p:?}");
    no_github(&p.to_string());
    assert!(ssh_calls().iter().any(|c| c == "-T git@ssh.bitbucket.org"), "{:?}", ssh_calls());
}

/// Check connection checks the SSH host a project's pushes really go to: an alias of yours with `ssh -T git@bb-alias`,
/// so it tests the key such a push uses. `ssh -G` tells where an alias goes.
#[test]
fn the_bitbucket_ssh_check_at_an_alias_asks_that_alias_and_ssh_g_says_where_it_goes() {
    let _one = T_MODE.lock().unwrap_or_else(|e| e.into_inner());
    t_mode("loggedin");
    assert_eq!(connection::bitbucket_ssh_check_at(ALIAS, LIMIT), Ok(Some("jefsev".into())));
    assert!(ssh_calls().iter().any(|c| c == "-T git@bb-alias"), "{:?}", ssh_calls());
    t_mode("hostkey");
    let p = connection::bitbucket_ssh_check_at(ALIAS, LIMIT).unwrap_err();
    assert_eq!(p.what, "This computer doesn't trust Bitbucket's SSH host key yet");
    // the old host itself: refused, said in plain words
    t_mode("ok");
    assert!(connection::bitbucket_ssh_check_at(OLD, LIMIT).is_err());
    // ssh -G: the alias's HostName, in lower case; None when ssh can't say
    assert_eq!(connection::ssh_hostname(ALIAS, LIMIT).as_deref(), Some("ssh.bitbucket.org"));
    assert_eq!(connection::ssh_hostname("git@bb-old", LIMIT).as_deref(), Some("bitbucket.org"));
    assert_eq!(connection::ssh_hostname("git@unknown-alias", LIMIT), None);
}
