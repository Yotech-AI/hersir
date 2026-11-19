//! Settings → Bitbucket: whether Gizai can use Bitbucket Cloud.
//! - The status: the login saved in your keychain (your Atlassian account's email and an API token) and the account it
//!   belongs to, as Bitbucket says.
//! - Save login: asks Bitbucket first and keeps only a login it accepts; Remove login.
//! - Check connection: the token's account, ssh to each SSH host the projects' pushes go to (git@ssh.bitbucket.org, or
//!   an SSH host alias of yours), and for each project with a Bitbucket link whether you can push to it (a dry run),
//!   with a note when its remote still names Bitbucket's old SSH host. Each check says what to do when it fails.
//!
//! Pushes go over SSH with your keys, to ssh.bitbucket.org (GA-315). The login lives only in the keychain: never in
//! Gizai's database, a log, an error or a command line.
use std::time::Duration;

use gizai_agents::bitbucket::{self, Login, SCOPES};
use gizai_agents::connection::{self, BITBUCKET_SSH_HOST, Problem, PushOver};
use gizai_agents::secrets::NOT_RUNNING;
use gizai_agents::worktree;
use gizai_core::model::Project;
use serde::Serialize;

use crate::AppState;
use crate::github::{Check, ConnectionCheck};

/// How long the SSH check may take.
const CHECK_LIMIT: Duration = Duration::from_secs(30);

/// What Settings → Bitbucket shows.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BitbucketStatus {
    /// The email of the login in the keychain.
    pub email: Option<String>,
    /// Whether an API token is saved with it.
    pub has_token: bool,
    /// Who the token belongs to, as Bitbucket names the account.
    pub account: Option<String>,
    /// Why there is no account (no login, Bitbucket refused it or couldn't be reached, the keychain is locked), with
    /// what to do.
    pub account_problem: Option<Problem>,
}

/// Where to make an API token, and the scopes it needs.
fn make_a_token() -> String {
    format!("Make an API token with scopes for Bitbucket (Atlassian account → Security → API tokens → Create API token with scopes) \
             with the scopes {}, and save it here with your Atlassian account's email.", SCOPES.join(", "))
}

fn no_login() -> Problem {
    Problem::new("No Bitbucket login yet", make_a_token())
}

/// The keychain couldn't be read, with what to do when its words don't say.
fn keychain_problem(e: String) -> Problem {
    if e == NOT_RUNNING || e.contains("then try again") {
        Problem::plain(e)
    } else {
        Problem::new(e, "Unlock your keychain, then try again.")
    }
}

/// Your Bitbucket login from the keychain, or why there is none, in plain words. Blocking (the keychain may ask its
/// service).
pub(crate) fn login(st: &AppState) -> Result<Login, String> {
    match bitbucket::saved_login(st.keychain.as_ref()) {
        Ok(Some(login)) => Ok(login),
        Ok(None) => Err("Log in to Bitbucket first (Settings → Bitbucket): Hersir opens and follows its pull requests with your email and an API token".into()),
        Err(e) => Err(e),
    }
}

/// Whether Gizai can use Bitbucket now: the login in the keychain and the account it belongs to (Bitbucket is asked).
pub async fn status(st: &AppState) -> BitbucketStatus {
    let st2 = st.clone();
    tokio::task::spawn_blocking(move || status_blocking(&st2)).await
        .unwrap_or_else(|e| BitbucketStatus { account_problem: Some(Problem::plain(e.to_string())), ..Default::default() })
}

fn status_blocking(st: &AppState) -> BitbucketStatus {
    let login = match bitbucket::saved_login(st.keychain.as_ref()) {
        Ok(Some(login)) => login,
        Ok(None) => return BitbucketStatus { account_problem: Some(no_login()), ..Default::default() },
        Err(e) => return BitbucketStatus { account_problem: Some(keychain_problem(e)), ..Default::default() },
    };
    let mut out = BitbucketStatus { email: Some(login.email.clone()), has_token: !login.token.is_empty(), ..Default::default() };
    match bitbucket::account(&login) {
        Ok(who) => out.account = Some(who),
        Err(p) => out.account_problem = Some(p),
    }
    out
}

/// Save login: asks Bitbucket who `email` and `token` belong to (`/user`), and only when it accepts them keeps them in
/// the keychain, in place of the login there. Otherwise nothing is saved, and the error says why in plain words.
pub async fn save_login(st: &AppState, email: String, token: String) -> Result<BitbucketStatus, String> {
    let (email, token) = (email.trim().to_string(), token.trim().to_string());
    if email.is_empty() || !email.contains('@') || email.chars().any(char::is_whitespace) {
        return Err("Fill in your Atlassian account's email: the one you log in to Bitbucket with".into());
    }
    if token.is_empty() {
        return Err(format!("Fill in an API token. {}", make_a_token()));
    }
    if token.chars().any(char::is_whitespace) {
        return Err("The API token has a space or line break in it: copy it again".into());
    }
    let st2 = st.clone();
    tokio::task::spawn_blocking(move || {
        let login = Login { email, token };
        let account = bitbucket::account(&login).map_err(|p| format!("{p} Nothing was saved."))?;
        bitbucket::save_login(st2.keychain.as_ref(), &login).map_err(|e| format!("Bitbucket accepted the login, but it couldn't be saved: {e}"))?;
        Ok(BitbucketStatus { email: Some(login.email), has_token: true, account: Some(account), account_problem: None })
    }).await.map_err(|e| e.to_string())?
}

/// Remove login: takes the Bitbucket login out of the keychain. Pull requests on Bitbucket then wait for a new one.
pub async fn remove_login(st: &AppState) -> Result<BitbucketStatus, String> {
    let st2 = st.clone();
    tokio::task::spawn_blocking(move || bitbucket::remove_login(st2.keychain.as_ref())).await.map_err(|e| e.to_string())?
        .map_err(|e| format!("The Bitbucket login couldn't be removed: {e}"))?;
    Ok(status(st).await)
}

/// Check connection: the account the API token belongs to; `ssh -T` in batch mode to each SSH host the Bitbucket
/// projects' pushes really go to, once per host and named in its line (git@ssh.bitbucket.org, or an SSH host alias of
/// yours that your own git rule sends a project's address to), so it uses the key a push uses; and for each project
/// with a Bitbucket link whether you can push to it, the way Open pull request would (a dry run over SSH: nothing is
/// sent), with a note (which fails nothing) when its remote still names Bitbucket's old SSH host. Never asks for
/// anything, and changes nothing.
pub async fn check(st: &AppState) -> ConnectionCheck {
    let st2 = st.clone();
    tokio::task::spawn_blocking(move || check_blocking(&st2)).await.unwrap_or_else(|e| ConnectionCheck {
        ok: false, push_over: "ssh".into(), checks: vec![Check::failed("Check connection", Problem::plain(e.to_string()))],
    })
}

fn check_blocking(st: &AppState) -> ConnectionCheck {
    let mut checks = vec![match bitbucket::saved_login(st.keychain.as_ref()) {
        Ok(Some(login)) => match bitbucket::account(&login) {
            Ok(who) => Check::ok("Account", format!("Logged in to Bitbucket as {who}")),
            Err(p) => Check::failed("Account", p),
        },
        Ok(None) => Check::failed("Account", no_login()),
        Err(e) => Check::failed("Account", keychain_problem(e)),
    }];
    let routes: Vec<Route> = crate::github::linked_projects(st, "bitbucket").iter().map(|(p, link, repo)| route(p, link, repo)).collect();
    // each host at the same time: one that gives no answer holds up the others no longer than itself
    let hosts: Vec<_> = ssh_hosts(&routes).into_iter().map(|host| std::thread::spawn(move || ssh_line(&host))).collect();
    checks.extend(hosts.into_iter().filter_map(|t| t.join().ok()));
    // each project's note right after its own line
    for line in crate::github::project_checks(st, "bitbucket", Some(&PushOver::Bitbucket)) {
        let notes: Vec<Check> = routes.iter().filter(|r| line.project_id.as_deref() == Some(r.project_id.as_str()))
            .flat_map(|r| r.notes.clone()).collect();
        checks.push(line);
        checks.extend(notes);
    }
    ConnectionCheck { ok: !checks.iter().any(|c| c.result == "failed"), push_over: "ssh".into(), checks }
}

/// Where a Bitbucket project's pushes go, for Check connection.
struct Route {
    project_id: String,
    /// The SSH host they go to (`connection::ssh_destination`), when they go over SSH: git@ssh.bitbucket.org, or an SSH
    /// host alias of yours.
    host: Option<String>,
    /// What Check connection notes about its remote.
    notes: Vec<Check>,
}

/// Where pushes of project `p` (linked to `link`, the repository `repo`) go, the way Open pull request pushes: through
/// its checkout's remote for the link, else to the link (`pulls::push_to`), as git works it out with Gizai's settings and
/// your own rewrites (`worktree::push_url`). Notes, which fail nothing:
/// - a remote that still names Bitbucket's old SSH host (git@bitbucket.org:…) without a rewrite of yours: Hersir already
///   uses ssh.bitbucket.org for it, and `git remote set-url` switches the checkout itself;
/// - pushes that go to the old SSH host all the same, which only a rewrite of yours can do: that rule is yours to change.
fn route(p: &Project, link: &str, repo: &str) -> Route {
    let mut out = Route { project_id: p.id.clone(), host: None, notes: vec![] };
    let Some(folder) = crate::github::project_folder(p) else { return out };
    let remote = crate::git::remote_for(folder, link);
    let to = remote.clone().unwrap_or_else(|| link.to_string());
    let goes_to = worktree::push_url(folder, &to, &PushOver::Bitbucket);
    out.host = goes_to.as_deref().and_then(connection::ssh_destination);
    let note = |text: String, fix: String| Check { project_id: Some(p.id.clone()), repo: Some(repo.to_string()), ..Check::note(&p.name, text, fix) };
    if let Some(name) = &remote {
        let own = worktree::own_rewrites(folder);
        let (urls, push_urls) = worktree::remote_addresses(folder, name);
        // the address it fetches from (its first) and the one it pushes to, as written
        let switch: Vec<String> = [(urls.first(), ""), (push_urls.first(), "--push ")].into_iter()
            .filter_map(|(url, flag)| {
                let url = url?;
                let new = connection::bitbucket_ssh_address(url)?;
                (!own.iter().any(|o| !o.is_empty() && url.starts_with(o.as_str()))).then(|| format!("git remote set-url {flag}{name} {new}"))
            })
            .collect();
        if !switch.is_empty() {
            out.notes.push(note(format!("Its remote {name} still uses git@bitbucket.org, Bitbucket's old SSH host, which stops taking SSH. \
                                         Hersir already uses {BITBUCKET_SSH_HOST} for it."),
                                format!("To switch the checkout itself, run {} in {}.", switch.join(" and "), folder.display())));
        }
    }
    if let Some(old) = goes_to.as_deref().filter(|u| connection::bitbucket_ssh_address(u).is_some()).and_then(connection::ssh_destination) {
        out.notes.push(note(format!("A git rule of your own sends its pushes to {old}, Bitbucket's old SSH host, which stops taking SSH."),
                            format!("Change that rule to send them to git@{BITBUCKET_SSH_HOST}: instead. \
                                     git config --show-origin --get-regexp '^url\\.' shows your rules and where each one is.")));
    }
    out
}

/// The SSH hosts the projects' pushes go to, each once, in the projects' order; Bitbucket's own
/// (git@ssh.bitbucket.org) when none goes over SSH, as with no project yet.
fn ssh_hosts(routes: &[Route]) -> Vec<String> {
    let mut hosts: Vec<String> = vec![];
    for host in routes.iter().filter_map(|r| r.host.clone()) {
        if !hosts.contains(&host) {
            hosts.push(host);
        }
    }
    if hosts.is_empty() {
        hosts.push(connection::bitbucket_ssh_destination());
    }
    hosts
}

/// The SSH line for `host`: whether it accepts your SSH key (`ssh -T`), with the host in its words.
fn ssh_line(host: &str) -> Check {
    match connection::bitbucket_ssh_check_at(host, CHECK_LIMIT) {
        Ok(Some(who)) => Check::ok("SSH", format!("{host} accepts your SSH key, as {who}")),
        Ok(None) => Check::ok("SSH", format!("{host} accepts your SSH key")),
        Err(p) => {
            let text = format!("{host}: {}", p.what);
            Check { text, ..Check::failed("SSH", p) }
        }
    }
}
