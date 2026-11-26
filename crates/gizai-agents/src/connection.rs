//! How Gizai reaches GitHub and Bitbucket, and the checks behind Settings → GitHub and Settings → Bitbucket.
//! - A push to GitHub goes over SSH with your keys (the default) or over HTTPS with the GitHub CLI's login; a push to
//!   Bitbucket over SSH with your keys, to Bitbucket's SSH host (`BITBUCKET_SSH_HOST`, GA-315). Either is set for that
//!   one git command only (`PushOver`). A failure says what to check, in plain words and in the terms of the place it
//!   went to (`push_problem`).
//! - Bitbucket's old SSH host (git@bitbucket.org, which stops taking SSH) goes to its new one in every push, fetch and
//!   ls-remote of Gizai's (`PushOver::git_config_in`, `fetch_config`), unless a rewrite of your own decides.
//! - An agent's own push in a task run goes the same way, to the project's own repository only: git settings in the
//!   run's environment (`PushOver::run_config`, `git_env`).
//! - The checks: gh's version and the account it is logged in as, ssh to git@github.com and to Bitbucket's SSH host (or
//!   your own host alias for it) in batch mode, and gh's own login in the browser (`GhLogin`).
//!
//! For GitHub, Gizai never stores a token or password and never asks for one: it uses gh's login and your SSH keys.
//! Bitbucket's login (your email and an API token, for its REST API) is in `bitbucket`.
use std::io::{BufRead, BufReader, ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::github;

/// How a card's branch goes to GitHub (Settings → GitHub → Push over), or to Bitbucket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushOver {
    /// SSH with your own keys, the default: an https://github.com/ address goes to git@github.com: instead.
    Ssh,
    /// HTTPS with the GitHub CLI's login (`gh auth git-credential`) as git's only credential helper: a git@github.com:
    /// address goes to https://github.com/ instead.
    Https { gh: PathBuf },
    /// To Bitbucket Cloud, over SSH with your own keys, to Bitbucket's SSH host: an https://bitbucket.org/ address, also
    /// one with a user name in it (https://jefsev@bitbucket.org/…, see `git_config_for`), and one on its old SSH host
    /// (`OLD_BITBUCKET_SSH`) go to git@ssh.bitbucket.org: instead.
    Bitbucket,
}

/// Bitbucket Cloud's host for git over SSH (GA-315): Atlassian moved it from bitbucket.org, which keeps only HTTPS (the
/// website and the REST API) and stops taking SSH after 10 December 2026. Every push, fetch, check and text of Gizai's
/// that speaks SSH to Bitbucket names this host; your SSH keys stay the same.
pub const BITBUCKET_SSH_HOST: &str = "ssh.bitbucket.org";

/// How an address on Bitbucket's old SSH host starts (bitbucket.org, which stops taking SSH): Gizai sends it to
/// `BITBUCKET_SSH_HOST` instead (`bitbucket_ssh_address`).
pub const OLD_BITBUCKET_SSH: [&str; 2] = ["git@bitbucket.org:", "ssh://git@bitbucket.org/"];

/// How a GitHub address starts, over HTTPS and over SSH.
const HTTPS: [&str; 4] = ["https://github.com/", "http://github.com/", "https://www.github.com/", "http://www.github.com/"];
const SSH: [&str; 2] = ["git@github.com:", "ssh://git@github.com/"];
/// How a Bitbucket address starts over HTTPS (without a user name in it).
const BITBUCKET_HTTPS: [&str; 4] = ["https://bitbucket.org/", "http://bitbucket.org/", "https://www.bitbucket.org/", "http://www.bitbucket.org/"];

/// Where `ssh -T` reaches Bitbucket: git@ssh.bitbucket.org.
pub fn bitbucket_ssh_destination() -> String {
    format!("git@{BITBUCKET_SSH_HOST}")
}

/// How an address on Bitbucket's SSH host starts: git@ssh.bitbucket.org:.
fn bitbucket_ssh_start() -> String {
    format!("{}:", bitbucket_ssh_destination())
}

/// The old SSH start (`OLD_BITBUCKET_SSH`) of `address` and the path after it ("acme/shop.git"), when it is on
/// Bitbucket's old SSH host.
fn on_old_bitbucket_ssh(address: &str) -> Option<(&'static str, &str)> {
    let address = address.trim();
    OLD_BITBUCKET_SSH.iter().find_map(|start| address.strip_prefix(start).map(|path| (*start, path))).filter(|(_, path)| !path.is_empty())
}

/// `address` on Bitbucket's SSH host, when it is on the old one: git@ssh.bitbucket.org:acme/shop.git for
/// git@bitbucket.org:acme/shop.git and for ssh://git@bitbucket.org/acme/shop.git. What the rewrite of the old host gives,
/// and what `git remote set-url` switches a checkout to.
pub fn bitbucket_ssh_address(address: &str) -> Option<String> {
    on_old_bitbucket_ssh(address).map(|(_, path)| format!("{}{path}", bitbucket_ssh_start()))
}

/// Whether your own rewrite starting with `own` decides where every address starting with `start` goes: git takes the
/// longest start that matches an address, so a shorter (or the same) start of yours always matches first.
fn decided_by_own(start: &str, own: &[String]) -> bool {
    own.iter().any(|o| !o.is_empty() && start.starts_with(o.as_str()))
}

/// The `-c` rules that send Bitbucket's old SSH host to its new one (`OLD_BITBUCKET_SSH` → git@ssh.bitbucket.org:), one
/// per old start, but none for a start your own git config already rewrites (`own`, the starts
/// `worktree::own_rewrites` gives): yours decides, like a rule that sends git@bitbucket.org: to an SSH host alias of
/// yours. A longer rule of yours (git@bitbucket.org:acme/) still wins for the addresses it matches, as git takes the
/// longest.
fn old_bitbucket_ssh_rules(own: &[String]) -> Vec<String> {
    OLD_BITBUCKET_SSH.iter().filter(|start| !decided_by_own(start, own))
        .map(|start| format!("url.{}.insteadOf={start}", bitbucket_ssh_start())).collect()
}

/// The `-c` settings for one fetch or `ls-remote` of Gizai's from `addresses` (as written: the URLs of the remote it goes
/// through, or the address itself) in a repository whose own git config rewrites the starts `own`
/// (`worktree::own_rewrites`): an address on Bitbucket's old SSH host goes to its new one, the way Gizai's push does,
/// unless a rewrite of yours decides (`old_bitbucket_ssh_rules`). Nothing for any other address: a fetch goes as the
/// remote is written.
pub fn fetch_config(addresses: &[String], own: &[String]) -> Vec<String> {
    if addresses.iter().any(|a| on_old_bitbucket_ssh(a).is_some()) {
        old_bitbucket_ssh_rules(own)
    } else {
        vec![]
    }
}

impl PushOver {
    /// The `-c` settings for one git command; they change no git config and no remote. git takes the longest rewrite
    /// that matches an address, so a more specific one in your own config (a mirror, a test's local repository) wins.
    /// To Bitbucket: its https addresses and those on its old SSH host go to git@ssh.bitbucket.org: (`git_config_in`
    /// leaves out a rule for the old host that a rewrite of your own decides).
    pub fn git_config(&self) -> Vec<String> {
        match self {
            PushOver::Ssh => HTTPS.iter().map(|from| format!("url.git@github.com:.insteadOf={from}")).collect(),
            PushOver::Https { gh } => {
                let mut c: Vec<String> = SSH.iter().chain(&HTTPS[1..]).map(|from| format!("url.https://github.com/.insteadOf={from}")).collect();
                // the empty helper first: none of your own helpers runs (or asks), only gh's login is used
                c.push("credential.helper=".into());
                c.push(format!("credential.helper=!{} auth git-credential", sh_quote(&gh.to_string_lossy())));
                c
            }
            PushOver::Bitbucket => self.git_config_in(&[], &[]),
        }
    }

    /// `git_config` for a push to `addresses` (where it goes, as the remote has them written): `git_config_in` a
    /// repository without rewrites of its own.
    pub fn git_config_for(&self, addresses: &[String]) -> Vec<String> {
        self.git_config_in(addresses, &[])
    }

    /// `git_config` for a push (or its `ls-remote`) to `addresses` (where it goes, as the remote has them written), in a
    /// repository whose own git config rewrites the address starts `own` (`worktree::own_rewrites`). To Bitbucket:
    /// - an https address with a user name in it (what Bitbucket's Clone button gives) goes to git@ssh.bitbucket.org:
    ///   too: git rewrites only the start of an address, so each user name needs a rule of its own. One with a password
    ///   in it is left as it is (its password never goes in a command line);
    /// - a rule for Bitbucket's old SSH host is left out when a rewrite of yours already decides its start
    ///   (`old_bitbucket_ssh_rules`): git@bitbucket.org: then goes where your rule sends it, an SSH host alias of yours
    ///   for instance.
    pub fn git_config_in(&self, addresses: &[String], own: &[String]) -> Vec<String> {
        if *self != PushOver::Bitbucket {
            return self.git_config();
        }
        let mut c: Vec<String> = BITBUCKET_HTTPS.iter().map(|from| format!("url.{}.insteadOf={from}", bitbucket_ssh_start())).collect();
        c.extend(old_bitbucket_ssh_rules(own));
        for rule in addresses.iter().filter_map(|a| bitbucket_user_start(a)).map(|from| format!("url.{}.insteadOf={from}", bitbucket_ssh_start())) {
            if !c.contains(&rule) {
                c.push(rule);
            }
        }
        c
    }

    /// The git settings of a task run of a project on GitHub or Bitbucket, so that the agent's own `git push` to the
    /// project's repository goes the way Gizai's own push does (`git_config_for`). Nothing else changes: other
    /// addresses, fetches (but those from Bitbucket's old SSH host), and `git remote get-url` without --push. As (key,
    /// value) pairs for `git_env`.
    /// - `full_name`: the repository, "acme/shop".
    /// - `addresses`: the addresses it goes by, as written: the project's link, and the URL of each remote of its
    ///   repository that is the same repository.
    /// - `own`: the starts of the addresses your own git config rewrites (`worktree::own_rewrites`).
    ///
    /// Each address Gizai's push would send elsewhere (an https one over SSH and to Bitbucket, an SSH one over HTTPS) gets
    /// a `pushInsteadOf` rule to the repository's address (`push_address`), as written, without and with .git. git takes
    /// a rule for every address that starts with it, and a `pushInsteadOf` beats every `insteadOf`; so an address that a
    /// rewrite of your own touches (one that starts it, or one for an address that starts with it: a mirror, a test's
    /// local repository) gets none, and yours decides, as it does for Gizai's own push. Over HTTPS, gh's login is the
    /// only credential helper for github.com (an empty one first, as `git_config` does).
    ///
    /// To Bitbucket, an address of the repository on Bitbucket's old SSH host (git@bitbucket.org:acme/shop.git,
    /// ssh://git@bitbucket.org/acme/shop.git) gets an `insteadOf` rule to the same path on its SSH host instead
    /// (git@ssh.bitbucket.org:acme/shop), for the agent's fetches and pushes alike, with the same exception for your own
    /// rewrites. It names the repository without .git, so every form of its address goes to the same repository there.
    pub fn run_config(&self, full_name: &str, addresses: &[String], own: &[String]) -> Vec<(String, String)> {
        let key = format!("url.{}.pushInsteadOf", self.push_address(full_name));
        let mut c: Vec<(String, String)> = vec![];
        let mut add = |rule: (String, String)| {
            if !c.contains(&rule) {
                c.push(rule);
            }
        };
        for address in addresses.iter().map(|a| a.trim()).filter(|a| self.rewrites(a)) {
            let stem = address.trim_end_matches('/');
            let stem = stem.strip_suffix(".git").unwrap_or(stem);
            if own.iter().any(|start| stem.starts_with(start.as_str()) || start.starts_with(stem)) {
                continue;
            }
            if *self == PushOver::Bitbucket
                && let Some((old, path)) = on_old_bitbucket_ssh(stem)
            {
                add((format!("url.{}{path}.insteadOf", bitbucket_ssh_start()), format!("{old}{path}")));
                continue;
            }
            for from in [address.to_string(), stem.to_string(), format!("{stem}.git")] {
                add((key.clone(), from));
            }
        }
        if let PushOver::Https { gh } = self {
            let helper = "credential.https://github.com.helper".to_string();
            c.push((helper.clone(), String::new()));
            c.push((helper, format!("!{} auth git-credential", sh_quote(&gh.to_string_lossy()))));
        }
        c
    }

    /// Where a push of the repository `full_name` ("acme/shop") goes: git@github.com:acme/shop.git over SSH,
    /// https://github.com/acme/shop.git over HTTPS, git@ssh.bitbucket.org:acme/shop.git to Bitbucket.
    fn push_address(&self, full_name: &str) -> String {
        match self {
            PushOver::Ssh => format!("git@github.com:{full_name}.git"),
            PushOver::Https { .. } => format!("https://github.com/{full_name}.git"),
            PushOver::Bitbucket => format!("{}{full_name}.git", bitbucket_ssh_start()),
        }
    }

    /// Whether Gizai's own push (`git_config_for`) sends `address` somewhere else.
    fn rewrites(&self, address: &str) -> bool {
        self.git_config_for(&[address.to_string()]).iter()
            .filter_map(|rule| rule.split_once(".insteadOf=").map(|(_, from)| from))
            .any(|from| address.starts_with(from))
    }

    /// The place a push goes to, for the words a problem is said in.
    fn host(&self) -> Host {
        match self {
            PushOver::Bitbucket => Host::Bitbucket,
            _ => Host::GitHub,
        }
    }
}

/// git settings ((key, value) pairs) as a program's environment, for every git it starts: GIT_CONFIG_COUNT,
/// GIT_CONFIG_KEY_<n> and GIT_CONFIG_VALUE_<n> (git 2.31+). They change no git config file, and take the place of any
/// such settings the environment had. Nothing for no settings.
pub fn git_env(settings: &[(String, String)]) -> Vec<(String, String)> {
    if settings.is_empty() {
        return vec![];
    }
    let mut env = vec![("GIT_CONFIG_COUNT".to_string(), settings.len().to_string())];
    for (n, (key, value)) in settings.iter().enumerate() {
        env.push((format!("GIT_CONFIG_KEY_{n}"), key.clone()));
        env.push((format!("GIT_CONFIG_VALUE_{n}"), value.clone()));
    }
    env
}

/// "https://jefsev@bitbucket.org/" for https://jefsev@bitbucket.org/acme/shop.git; None for any other address.
fn bitbucket_user_start(address: &str) -> Option<String> {
    let (scheme, rest) = address.trim().split_once("://")?;
    if !matches!(scheme, "https" | "http") {
        return None;
    }
    let (user, path) = rest.split_once('@')?;
    if user.is_empty() || user.contains(['/', ':']) {
        return None;
    }
    let host = ["bitbucket.org/", "www.bitbucket.org/"].into_iter().find(|h| path.starts_with(h))?;
    Some(format!("{scheme}://{user}@{host}"))
}

/// Where a push goes: GitHub or Bitbucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Host {
    GitHub,
    Bitbucket,
}

/// What went wrong talking to GitHub, and what to do about it, in plain words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Problem {
    pub what: String,
    /// What to do, when Gizai knows.
    pub fix: Option<String>,
}

impl Problem {
    pub fn new(what: impl Into<String>, fix: impl Into<String>) -> Problem {
        Problem { what: what.into(), fix: Some(fix.into()) }
    }

    pub fn plain(what: impl Into<String>) -> Problem {
        Problem { what: what.into(), fix: None }
    }
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.fix {
            Some(fix) => write!(f, "{}. {fix}", self.what),
            None => write!(f, "{}", self.what),
        }
    }
}

const OR_HTTPS: &str = "Or push over HTTPS with gh's login (Settings → GitHub).";

/// What a failed push (or its dry run) means, from everything git said: what to check, in plain words, in the terms of
/// the place it went to (GitHub, or Bitbucket for `PushOver::Bitbucket`). Else git's own words.
pub fn push_problem(said: &str, over: &PushOver) -> Problem {
    if over.host() == Host::Bitbucket {
        return bitbucket_push_problem(said);
    }
    let has = |s: &str| said.contains(s);
    if let Some(p) = ssh_problem(said, Host::GitHub) {
        return p;
    }
    if has("Repository not found") {
        return Problem::new("GitHub has no repository at this address that your account can see",
                            "Check the project's GitHub link, and that your GitHub account has access to the repository.");
    }
    if let Some(who) = denied_to(said) {
        return Problem::new(format!("Your GitHub account {who} can't push to this repository"),
                            "Ask for write access to it, or use an account that has it.");
    }
    let no_login = ["could not read Username", "could not read Password", "terminal prompts disabled"].iter().any(|s| has(s));
    let refused = has("Authentication failed") || has("Invalid username or token");
    if no_login || refused {
        return match over {
            PushOver::Https { .. } if refused => Problem::new("GitHub refused gh's login for HTTPS",
                                                              "Use Log in with GitHub again in Settings → GitHub, or run gh auth login in a terminal."),
            PushOver::Https { .. } => not_logged_in_for_https(),
            // (Bitbucket's pushes are told apart above)
            PushOver::Ssh | PushOver::Bitbucket => Problem::new("git tried HTTPS, and has no login for it",
                                          "Check where this repository pushes to (git remote -v), or push over HTTPS with gh's login (Settings → GitHub)."),
        };
    }
    if has("refusing to allow an OAuth App to create or update workflow") {
        return Problem::new("gh's login may not change GitHub Actions workflows",
                            "Run gh auth refresh -s workflow in a terminal, or push over SSH (Settings → GitHub).");
    }
    if has("[rejected]") || has("non-fast-forward") || has("(fetch first)") {
        return Problem::new("The branch on GitHub has commits this one doesn't",
                            "Hersir never forces a push: merge GitHub's copy into the branch first.");
    }
    if ["Could not resolve host", "Failed to connect to", "Couldn't connect to server", "Connection timed out", "Network is unreachable"].iter().any(|s| has(s)) {
        return Problem::new("Can't reach github.com", "Check your internet connection, then try again.");
    }
    if has("src refspec") && has("does not match any") {
        return Problem::new("The repository has no commits yet", "Hersir tries a push with its latest commit: commit something first.");
    }
    Problem::plain(last_words(said))
}

/// What a failed push to Bitbucket means, in Bitbucket's terms. Else git's (or Bitbucket's) own words.
fn bitbucket_push_problem(said: &str) -> Problem {
    let has = |s: &str| said.contains(s);
    if let Some(p) = ssh_problem(said, Host::Bitbucket) {
        return p;
    }
    if has("repository does not exist") || has("Repository not found") || has("repository not found") {
        return Problem::new("Bitbucket has no repository at this address that your account can see",
                            "Check the project's Bitbucket link, and that your Bitbucket account has access to the repository.");
    }
    if ["access denied", "Access denied", "Forbidden", "read-only", "read only"].iter().any(|s| has(s)) {
        return Problem::new("Your Bitbucket account can't push to this repository",
                            "Ask a workspace admin for write access to it. An access key (a repository's deployment key) can only read.");
    }
    if ["could not read Username", "could not read Password", "terminal prompts disabled", "Authentication failed"].iter().any(|s| has(s)) {
        return Problem::new("git tried HTTPS, and has no login for it",
                            format!("Check where this repository pushes to (git remote -v): Hersir pushes to Bitbucket over SSH \
                                     ({BITBUCKET_SSH_HOST}), with your SSH keys."));
    }
    if has("[rejected]") || has("non-fast-forward") || has("(fetch first)") {
        return Problem::new("The branch on Bitbucket has commits this one doesn't",
                            "Hersir never forces a push: merge Bitbucket's copy into the branch first.");
    }
    // (ssh's own words are told apart above: these are git's over HTTPS, which stays on bitbucket.org)
    if ["Could not resolve host", "Failed to connect to", "Couldn't connect to server", "Connection timed out", "Network is unreachable"].iter().any(|s| has(s)) {
        return Problem::new("Can't reach bitbucket.org over HTTPS",
                            format!("Check your internet connection, then try again. Hersir pushes to Bitbucket over SSH \
                                     ({BITBUCKET_SSH_HOST}): git remote -v shows where this repository goes."));
    }
    if has("src refspec") && has("does not match any") {
        return Problem::new("The repository has no commits yet", "Hersir tries a push with its latest commit: commit something first.");
    }
    Problem::plain(last_words(said))
}

/// Where you add an SSH key on Bitbucket.
const BITBUCKET_KEYS: &str = "Bitbucket → Personal settings → SSH keys (bitbucket.org/account/settings/ssh-keys/)";

/// What ssh said went wrong, in plain words.
fn ssh_problem(said: &str, host: Host) -> Option<Problem> {
    if host == Host::Bitbucket {
        return bitbucket_ssh_problem(said);
    }
    let has = |s: &str| said.contains(s);
    if has("Permission denied (publickey") {
        Some(Problem::new("GitHub didn't accept your SSH key",
                          format!("Add your public key to your GitHub account (github.com/settings/keys), or load it into ssh-agent with ssh-add. {OR_HTTPS}")))
    } else if has("Host key verification failed") {
        Some(Problem::new("This computer doesn't trust GitHub's SSH host key yet",
                          "Run ssh -T git@github.com once in a terminal and answer yes."))
    } else if has("ssh: not found") || has("ssh: command not found") || has("cannot run ssh") {
        Some(ssh_missing(Host::GitHub))
    } else if ["ssh: connect to host", "Could not resolve hostname", "Connection closed by", "kex_exchange_identification", "Connection reset by"].iter().any(|s| has(s)) {
        Some(Problem::new("Can't reach github.com over SSH",
                          format!("Check your internet connection, and that no firewall blocks SSH (port 22). {OR_HTTPS}")))
    } else {
        None
    }
}

/// What ssh said went wrong talking to Bitbucket, in Bitbucket's terms: its SSH host is `BITBUCKET_SSH_HOST`.
fn bitbucket_ssh_problem(said: &str) -> Option<Problem> {
    let has = |s: &str| said.contains(s);
    if has("Permission denied (publickey") {
        Some(Problem::new("Bitbucket didn't accept your SSH key",
                          format!("Add your public key in {BITBUCKET_KEYS}, or load it into ssh-agent with ssh-add.")))
    } else if has("Host key verification failed") {
        Some(Problem::new("This computer doesn't trust Bitbucket's SSH host key yet",
                          format!("Run ssh -T {} once in a terminal and answer yes.", bitbucket_ssh_destination())))
    } else if has("ssh: not found") || has("ssh: command not found") || has("cannot run ssh") {
        Some(ssh_missing(Host::Bitbucket))
    } else if ["ssh: connect to host", "Could not resolve hostname", "Connection closed by", "kex_exchange_identification", "Connection reset by"].iter().any(|s| has(s)) {
        Some(Problem::new(format!("Can't reach {BITBUCKET_SSH_HOST} over SSH"),
                          "Check your internet connection, and that no firewall blocks SSH (port 22)."))
    } else {
        None
    }
}

fn ssh_missing(host: Host) -> Problem {
    match host {
        Host::GitHub => Problem::new("ssh isn't installed", format!("Install OpenSSH. {OR_HTTPS}")),
        Host::Bitbucket => Problem::new("ssh isn't installed", format!("Install OpenSSH: Hersir pushes to Bitbucket over SSH ({BITBUCKET_SSH_HOST}).")),
    }
}

fn not_logged_in_for_https() -> Problem {
    Problem::new("The GitHub CLI isn't logged in, so git has no login for HTTPS",
                 "Use Log in with GitHub in Settings → GitHub, or run gh auth login in a terminal.")
}

/// GitHub gave no answer within `limit`.
pub fn no_answer(limit: Duration) -> Problem {
    no_answer_from(Host::GitHub, limit)
}

/// The place a push went to (`over`) gave no answer within `limit`.
pub fn push_no_answer(over: &PushOver, limit: Duration) -> Problem {
    no_answer_from(over.host(), limit)
}

fn no_answer_from(host: Host, limit: Duration) -> Problem {
    let name = match host {
        Host::GitHub => "GitHub",
        Host::Bitbucket => "Bitbucket",
    };
    Problem::new(format!("{name} gave no answer within {}", span(limit)), "Check your internet connection, then try again.")
}

/// "2 minutes", "30 seconds".
fn span(d: Duration) -> String {
    let s = d.as_secs();
    match s {
        60 => "a minute".into(),
        s if s > 60 && s % 60 == 0 => format!("{} minutes", s / 60),
        s => format!("{s} seconds"),
    }
}

/// "octocat" in "Permission to acme/shop.git denied to octocat."
fn denied_to(said: &str) -> Option<String> {
    let line = said.lines().find(|l| l.contains("Permission to ") && l.contains(" denied to "))?;
    let who = line.split(" denied to ").nth(1)?.trim().trim_end_matches('.');
    (!who.is_empty()).then(|| who.to_string())
}

/// git's (or ssh's) most telling line: its first error that isn't a generic one, else its last line; without the
/// "fatal:" and "error:" prefixes.
fn last_words(said: &str) -> String {
    const GENERIC: [&str; 4] = ["failed to push some refs", "Could not read from remote repository", "the remote end hung up", "unable to access"];
    let tidy = |l: &str| {
        let l = l.trim();
        let l = l.strip_prefix("remote: ").unwrap_or(l);
        ["fatal: ", "error: ", "ERROR: "].iter().fold(l, |l, p| l.strip_prefix(p).unwrap_or(l)).trim().to_string()
    };
    let lines: Vec<&str> = said.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("hint:")).collect();
    let error = lines.iter().find(|l| {
        ["fatal:", "error:", "ERROR:", "remote: error:"].iter().any(|p| l.starts_with(p)) && !GENERIC.iter().any(|g| l.contains(g))
    });
    match error.or(lines.last()) {
        Some(l) => tidy(l),
        None => "git failed".into(),
    }
}

/// `s` as one shell word.
fn sh_quote(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "/._-+:@%,".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

/// ssh as git runs it for a push: your own GIT_SSH_COMMAND when you set one, else ssh in batch mode, so it never asks.
fn ssh_command() -> Result<std::process::Command, Problem> {
    Ok(match std::env::var("GIT_SSH_COMMAND") {
        Ok(own) if !own.trim().is_empty() => {
            let mut c = crate::os::command(own_ssh_shell()?);
            c.arg("-c").arg(format!("{own} \"$@\"")).arg(&own);
            c
        }
        _ => {
            let mut c = crate::os::command("ssh");
            c.args(["-o", "BatchMode=yes"]);
            c
        }
    })
}

/// What `ssh -T <to>` says, in batch mode so it never asks. It runs your own GIT_SSH_COMMAND when you set one, the way
/// git does for a push.
fn ssh_t(to: &str, host: Host, limit: Duration) -> Result<String, Problem> {
    let mut cmd = ssh_command()?;
    cmd.args(["-T", to]).env("SSH_ASKPASS_REQUIRE", "never");
    let (_, out, err) = github::run(cmd, None, limit).map_err(|e| match e.kind() {
        ErrorKind::NotFound => ssh_missing(host),
        ErrorKind::TimedOut => no_answer_from(host, limit),
        _ => Problem::plain(format!("can't run ssh: {e}")),
    })?;
    Ok(format!("{err}\n{out}"))
}

/// The shell that runs your GIT_SSH_COMMAND, as git's own does: `sh` on Linux and macOS, Git Bash on Windows (Git for
/// Windows runs it with its own shell too).
fn own_ssh_shell() -> Result<PathBuf, Problem> {
    #[cfg(not(windows))]
    return Ok(PathBuf::from("sh"));
    #[cfg(windows)]
    crate::os::git_bash(&std::env::var_os("PATH").unwrap_or_default()).ok_or_else(|| {
        Problem::new("Git for Windows isn't installed", "Install Git for Windows: Hersir runs your GIT_SSH_COMMAND with its Git Bash, as git does.")
    })
}

/// Whether ssh reaches GitHub with your keys, in batch mode so it never asks: `ssh -T git@github.com`, which GitHub
/// answers with "Hi <login>! You've successfully authenticated, …". Ok: the account your key belongs to. It runs your
/// own GIT_SSH_COMMAND when you set one, the way git does for a push.
pub fn ssh_check(limit: Duration) -> Result<String, Problem> {
    let said = ssh_t("git@github.com", Host::GitHub, limit)?;
    // GitHub ends the session with exit code 1 even when the key is accepted, so its words decide
    if let Some(who) = said.lines().filter(|l| l.contains("successfully authenticated")).find_map(|l| l.trim().strip_prefix("Hi ")?.split('!').next()) {
        return Ok(who.trim().to_string());
    }
    Err(ssh_problem(&said, Host::GitHub).unwrap_or_else(|| Problem::plain(last_words(&said))))
}

/// Whether ssh reaches Bitbucket's SSH host with your keys: `bitbucket_ssh_check_at` git@ssh.bitbucket.org.
pub fn bitbucket_ssh_check(limit: Duration) -> Result<Option<String>, Problem> {
    bitbucket_ssh_check_at(&bitbucket_ssh_destination(), limit)
}

/// Whether ssh reaches Bitbucket at `to` with your keys, in batch mode so it never asks: `ssh -T <to>`, where `to` is
/// git@ssh.bitbucket.org, or the SSH host alias of yours a project's pushes go to (git@bitbucket-snx), so the check
/// uses the key such a push uses (`ssh_destination`). Bitbucket answers a key it accepts with "authenticated via ssh
/// key." (or "logged in as <username>." for an older one) and "You can use git to connect to Bitbucket. Shell access is
/// disabled", not the way GitHub does; its words decide, not its exit code. Ok: the account your key belongs to, when
/// Bitbucket names it.
pub fn bitbucket_ssh_check_at(to: &str, limit: Duration) -> Result<Option<String>, Problem> {
    let said = ssh_t(to, Host::Bitbucket, limit)?;
    if let Some(who) = said.lines().find_map(|l| word_after(l, "logged in as ")) {
        return Ok(Some(who));
    }
    if ["authenticated via", "You can use git to connect to Bitbucket", "You can use git or hg to connect to Bitbucket"].iter().any(|w| said.contains(w)) {
        return Ok(None);
    }
    Err(bitbucket_ssh_problem(&said).unwrap_or_else(|| Problem::plain(last_words(&said))))
}

/// The host name ssh really connects to for `to` ("git@bitbucket-snx"): the HostName of an SSH host alias of yours, as
/// `ssh -G` works it out from your ~/.ssh/config, in lower case. ssh -G connects nowhere and asks nothing (git runs it
/// too, to tell which ssh it has). It runs your own GIT_SSH_COMMAND when you set one, the way git does. None when ssh
/// can't say.
pub fn ssh_hostname(to: &str, limit: Duration) -> Option<String> {
    let mut cmd = ssh_command().ok()?;
    cmd.args(["-G", to]).env("SSH_ASKPASS_REQUIRE", "never");
    let (ok, out, _) = github::run(cmd, None, limit).ok()?;
    if !ok {
        return None;
    }
    out.lines().find_map(|l| l.trim().strip_prefix("hostname ")).map(|h| h.trim().to_ascii_lowercase()).filter(|h| !h.is_empty())
}

/// Where `ssh -T` reaches the host of the git address `address`, when it goes over SSH: "git@ssh.bitbucket.org" for
/// git@ssh.bitbucket.org:acme/shop.git and ssh://git@ssh.bitbucket.org/acme/shop.git, "git@bitbucket-snx" for
/// git@bitbucket-snx:acme/shop.git (an SSH host alias of yours), "ssh://git@host:2222" with a port. None for an address
/// that doesn't go over SSH: https, a path, a file:// URL.
pub fn ssh_destination(address: &str) -> Option<String> {
    let a = address.trim();
    if let Some(rest) = ["ssh://", "git+ssh://", "ssh+git://"].iter().find_map(|p| a.strip_prefix(p)) {
        let host = rest.split('/').next().filter(|h| !h.is_empty())?;
        // ssh takes a port only in its own URL form
        return Some(if host.rsplit_once(':').is_some_and(|(_, port)| !port.is_empty() && port.chars().all(|c| c.is_ascii_digit())) {
            format!("ssh://{host}")
        } else {
            host.trim_end_matches(':').to_string()
        });
    }
    if a.contains("://") {
        return None;
    }
    // git's scp-like form, [user@]host:path, with no slash before the colon (a path like ./a:b is a file)
    let (host, path) = a.split_once(':')?;
    let drive = host.len() == 1 && host.chars().all(|c| c.is_ascii_alphabetic());
    (!host.is_empty() && !path.is_empty() && !host.contains(['/', '\\']) && !drive).then(|| host.to_string())
}

/// Why gh couldn't be run, with what to do.
fn gh_failed(gh: &Path, e: std::io::Error, limit: Duration) -> Problem {
    match e.kind() {
        ErrorKind::NotFound => Problem::new(format!("The GitHub CLI isn't at {}", gh.display()),
                                            "Install it from cli.github.com, or set its path in Settings → GitHub."),
        ErrorKind::TimedOut => Problem::new(format!("gh gave no answer within {}", span(limit)), "Check your internet connection, then try again."),
        _ => Problem::plain(format!("can't run gh: {e}")),
    }
}

/// The GitHub CLI's version ("2.62.0"), or why it doesn't run.
pub fn gh_version(gh: &Path, limit: Duration) -> Result<String, Problem> {
    let mut cmd = github::gh_command(gh);
    cmd.arg("--version");
    let (ok, out, err) = github::run(cmd, None, limit).map_err(|e| gh_failed(gh, e, limit))?;
    if !ok {
        return Err(Problem::plain(last_words(&format!("{out}\n{err}"))));
    }
    let first = out.lines().next().unwrap_or("").trim();
    let version = first.strip_prefix("gh version ").and_then(|v| v.split_whitespace().next()).unwrap_or(first);
    Ok(version.to_string())
}

fn not_logged_in() -> Problem {
    Problem::new("The GitHub CLI isn't logged in", "Use Log in with GitHub, or run gh auth login in a terminal.")
}

/// The account the GitHub CLI is logged in as on github.com (its active one), or why there is none, with what to do.
/// `gh auth status` asks GitHub whether the login still works; Gizai never reads its token.
pub fn gh_account(gh: &Path, limit: Duration) -> Result<String, Problem> {
    let mut cmd = github::gh_command(gh);
    cmd.args(["auth", "status", "--hostname", "github.com"]);
    let (_, out, err) = github::run(cmd, None, limit).map_err(|e| gh_failed(gh, e, limit))?;
    account_from_status(&format!("{out}\n{err}"))
}

/// The active account in an answer of `gh auth status` (gh lists it first), or why there is none.
pub fn account_from_status(said: &str) -> Result<String, Problem> {
    for line in said.lines().map(str::trim) {
        if let Some(who) = word_after(line, "Logged in to github.com account ").or_else(|| word_after(line, "Logged in to github.com as ")) {
            return Ok(who);
        }
        // gh says this when GitHub refuses its token, and also when it can't reach GitHub
        if line.contains("Failed to log in to github.com") {
            let who = word_after(line, "Failed to log in to github.com account ").map(|w| format!(" as {w}")).unwrap_or_default();
            return Err(Problem::new(format!("gh's login{who} doesn't work: GitHub refused it, or couldn't be reached"),
                                    "Check your internet connection. If that's fine, use Log in with GitHub again, or run gh auth login in a terminal."));
        }
        if line.contains("Timeout trying to log in to github.com") {
            return Err(Problem::new("GitHub didn't answer gh in time", "Check your internet connection, then try again."));
        }
    }
    if said.contains("error connecting to") {
        return Err(Problem::new("gh can't reach GitHub", "Check your internet connection, then try again."));
    }
    if said.contains("not logged in") || said.contains("gh auth login") {
        return Err(not_logged_in());
    }
    Err(Problem::plain(last_words(said)))
}

/// The word after `prefix` in `line`, without trailing punctuation.
fn word_after(line: &str, prefix: &str) -> Option<String> {
    let rest = &line[line.find(prefix)? + prefix.len()..];
    let word = rest.split_whitespace().next()?.trim_end_matches(['.', ',', '!', ':']);
    (!word.is_empty()).then(|| word.to_string())
}

/// Where you enter gh's one-time code, when gh doesn't say.
pub const DEVICE_URL: &str = "https://github.com/login/device";

/// The command that logs gh in from a terminal, for when the login in the app can't work.
pub fn login_command(gh: &Path) -> String {
    let on_path = gh.parent().is_some_and(|dir| std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d == dir)));
    let bin = if on_path && gh.file_name().is_some_and(|n| n == "gh") { "gh".to_string() } else { sh_quote(&gh.to_string_lossy()) };
    format!("{bin} auth login --hostname github.com --web")
}

/// gh's own login in the browser, run without a terminal (`gh auth login --web`). gh shows a one-time code and a
/// link, waits until you enter the code on GitHub, then keeps its login where it always does (your keyring). Gizai
/// only shows the code and the link. It asks for gh's usual access plus `workflow`, so a push over HTTPS may include
/// GitHub Actions workflows.
pub struct GhLogin {
    gh: PathBuf,
    child: Child,
    /// The one-time code to enter on GitHub.
    pub code: String,
    /// Where to enter it.
    pub url: String,
    lines: Receiver<String>,
    said: Vec<String>,
}

impl GhLogin {
    /// Starts gh's login and returns once gh shows its one-time code (at most `limit`).
    pub fn start(gh: &Path, limit: Duration) -> Result<GhLogin, Problem> {
        let mut cmd = github::gh_command(gh);
        cmd.args(["auth", "login", "--web", "--hostname", "github.com", "--scopes", "workflow"])
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| gh_failed(gh, e, limit))?;
        let (tx, lines) = mpsc::channel();
        let pipes: [Option<Box<dyn Read + Send>>; 2] = [child.stdout.take().map(|p| Box::new(p) as _), child.stderr.take().map(|p| Box::new(p) as _)];
        for pipe in pipes.into_iter().flatten() {
            let tx = tx.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
        }
        drop(tx);
        let (mut code, mut url, mut said) = (None::<String>, None::<String>, vec![]);
        let until = Instant::now() + limit;
        let mut code_at = None;
        loop {
            if code.is_some() && (url.is_some() || code_at.is_some_and(|at: Instant| at.elapsed() > Duration::from_secs(2))) {
                break;
            }
            match lines.recv_timeout(Duration::from_millis(100)) {
                Ok(line) => {
                    if code.is_none() {
                        code = word_after(&line, "one-time code: ");
                        code_at = code.as_ref().map(|_| Instant::now());
                    }
                    if url.is_none() {
                        url = line.split_whitespace().find(|w| w.starts_with("https://github.com/")).map(|w| w.trim_end_matches(['.', ',']).to_string());
                    }
                    said.push(line);
                }
                Err(RecvTimeoutError::Timeout) if Instant::now() < until => {}
                Err(RecvTimeoutError::Timeout) => {
                    crate::os::kill_child(&mut child, "the GitHub CLI (gh) showed no one-time code in time");
                    let _ = child.wait();
                    return Err(Problem::new(format!("gh showed no one-time code within {}", span(limit)),
                                            format!("Run {} in a terminal instead.", login_command(gh))));
                }
                // gh ended before it showed a code
                Err(RecvTimeoutError::Disconnected) => {
                    let _ = child.wait();
                    return Err(login_problem(gh, &said.join("\n")));
                }
            }
        }
        let code = code.unwrap_or_default();
        Ok(GhLogin { gh: gh.to_path_buf(), child, code, url: url.unwrap_or_else(|| DEVICE_URL.into()), lines, said })
    }

    /// Waits until gh is done: you entered the code on GitHub (Ok, with the account gh says it logged in as), gh gave
    /// up, `limit` passed or `stop` was set (gh is ended).
    pub fn wait(mut self, limit: Duration, stop: &AtomicBool) -> Result<Option<String>, Problem> {
        let until = Instant::now() + limit;
        let status = loop {
            match self.child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if stop.load(Ordering::Relaxed) => {
                    self.end();
                    return Err(Problem::plain("Login cancelled"));
                }
                Ok(None) if Instant::now() >= until => {
                    self.end();
                    return Err(Problem::new(format!("The code wasn't entered within {}", span(limit)), "Log in again."));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(e) => return Err(Problem::plain(format!("can't follow gh's login: {e}"))),
            }
        };
        // what gh said last (its pipes close as it ends)
        let deadline = Instant::now() + Duration::from_secs(2);
        while let Ok(line) = self.lines.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            self.said.push(line);
        }
        let said = self.said.join("\n");
        if status.success() {
            Ok(word_after(&said, "Logged in as "))
        } else {
            Err(login_problem(&self.gh, &said))
        }
    }

    fn end(&mut self) {
        crate::os::kill_child(&mut self.child, "the GitHub CLI's (gh) login ended or was cancelled");
        let _ = self.child.wait();
    }
}

/// Why gh's login didn't work, with what to do instead.
fn login_problem(gh: &Path, said: &str) -> Problem {
    let command = login_command(gh);
    if let Some(var) = ["GH_TOKEN", "GITHUB_TOKEN", "GH_ENTERPRISE_TOKEN"].iter().find(|v| said.contains(&format!("{v} environment variable"))) {
        return Problem::new(format!("gh uses the token in {var}, so it can't log in another way"),
                            format!("Remove {var} from the environment Hersir starts in, or log in with {command} in a terminal."));
    }
    if said.contains("expired") {
        return Problem::new("The code expired before it was entered", "Log in again.");
    }
    if said.contains("error connecting to") {
        return Problem::new("gh can't reach GitHub", "Check your internet connection, then try again.");
    }
    let words = said.lines().map(str::trim).rfind(|l| !l.is_empty() && !l.contains("one-time code") && !l.starts_with("Open this URL"));
    match words {
        Some(w) => Problem::new(format!("gh's login didn't work: {}", w.trim_start_matches(['!', 'X', '✓', ' '])), format!("Run {command} in a terminal instead.")),
        None => Problem::new("gh's login ended without saying why", format!("Run {command} in a terminal instead.")),
    }
}
