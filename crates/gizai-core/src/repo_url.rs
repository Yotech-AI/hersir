//! A project's repository link (GitHub, Bitbucket Cloud, or any git URL): normalised for storage, and compared with the
//! remotes of a local repository.
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct RepoUrl {
    pub url: String,
    /// "github", "bitbucket" or "git"
    pub provider: String,
    /// The GitHub owner, or the Bitbucket workspace.
    pub owner: Option<String>,
    pub name: Option<String>,
}

impl RepoUrl {
    /// "owner/name" on GitHub, "workspace/repository" on Bitbucket; None for a plain git URL.
    pub fn full_name(&self) -> Option<String> {
        Some(format!("{}/{}", self.owner.as_deref()?, self.name.as_deref()?))
    }
}

/// A provider in plain words ("GitHub", "Bitbucket"); None for a plain git URL.
pub fn provider_name(provider: &str) -> Option<&'static str> {
    match provider {
        "github" => Some("GitHub"),
        "bitbucket" => Some("Bitbucket"),
        _ => None,
    }
}

/// None for empty input. A GitHub link in any usual form (https, ssh, git@, "owner/name", a page inside the
/// repository) becomes https://github.com/<owner>/<name>; a Bitbucket Cloud link in any usual form (https with or
/// without a user name, git@, ssh://, a page inside the repository like /src/master/) becomes
/// https://bitbucket.org/<workspace>/<repository>; other git URLs and paths are kept as given. A link git could read as
/// an option is refused (`refuse_option`).
pub fn normalize(input: &str) -> Result<Option<RepoUrl>> {
    let s = input.trim();
    if s.is_empty() {
        return Ok(None);
    }
    refuse_option(s)?;
    if let Some((owner, name)) = github(s) {
        return Ok(Some(RepoUrl { url: format!("https://github.com/{owner}/{name}"), provider: "github".into(), owner: Some(owner), name: Some(name) }));
    }
    if let Some((workspace, name)) = bitbucket(s) {
        return Ok(Some(RepoUrl { url: format!("https://bitbucket.org/{workspace}/{name}"), provider: "bitbucket".into(), owner: Some(workspace), name: Some(name) }));
    }
    if on_bitbucket(s).is_some() {
        return Err(Error::Invalid(format!("{s} isn't a Bitbucket repository link (https://bitbucket.org/workspace/repository)")));
    }
    let git_url = ["https://", "http://", "ssh://", "git://", "file://", "/"].iter().any(|p| s.starts_with(p)) || is_scp(s);
    if s.contains("github.com") || s.chars().any(char::is_whitespace) || !git_url {
        return Err(Error::Invalid(format!("{s} isn't a GitHub link (https://github.com/owner/name) or a git URL")));
    }
    Ok(Some(RepoUrl { url: s.to_string(), provider: "git".into(), owner: None, name: None }))
}

/// GA-318 (CWE-88): whether git, or the ssh it starts, could read `link` or a part of it as an option instead of an
/// address: the link starts with "-", or the user name, host or port in it does (in a URL, scheme://user@host:port/…;
/// in git's scp form, user@host:path). git runs some of its options as a command, and so does ssh. Such a link is never
/// saved (`normalize`), and one saved before this check is refused where a fetch or a push starts, before git runs.
pub fn reads_as_option(link: &str) -> bool {
    let s = link.trim();
    if s.starts_with('-') {
        return true;
    }
    let authority = match s.split_once("://") {
        Some((_, rest)) => rest.split(['/', '?', '#']).next().unwrap_or_default(),
        None if is_scp(s) => s.split_once(':').map_or("", |(host, _)| host),
        None => return false,
    };
    // user[:password]@host[:port], the host maybe an IPv6 address in brackets
    let (user, host_port) = match authority.rsplit_once('@') {
        Some((user, rest)) => (Some(user), rest),
        None => (None, authority),
    };
    let (host, port) = match host_port.strip_prefix('[').and_then(|r| r.split_once(']')) {
        Some((host, rest)) => (host, rest.strip_prefix(':')),
        None => match host_port.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (host_port, None),
        },
    };
    [user, Some(host), port].into_iter().flatten().any(|part| part.starts_with('-'))
}

/// Err with a sentence when git could read `link` as an option (`reads_as_option`).
pub fn refuse_option(link: &str) -> Result<()> {
    if reads_as_option(link) {
        return Err(Error::Invalid(format!("{} isn't a repository link: it, or its user name, host or port, starts with \"-\", which \
                                           git reads as an option", link.trim())));
    }
    Ok(())
}

/// Whether two links name the same repository (GitHub and Bitbucket ignore case and the form of the link; ".git", a
/// trailing "/" and "file://" don't count).
pub fn same_repo(a: &str, b: &str) -> bool {
    key(a) == key(b)
}

fn key(url: &str) -> String {
    let s = url.trim();
    if let Some((owner, name)) = github(s) {
        return format!("github.com/{owner}/{name}").to_lowercase();
    }
    if let Some((workspace, name)) = bitbucket(s) {
        return format!("bitbucket.org/{workspace}/{name}").to_lowercase();
    }
    let s = s.strip_prefix("file://").unwrap_or(s).trim_end_matches('/');
    s.strip_suffix(".git").unwrap_or(s).to_string()
}

/// user@host:path
fn is_scp(s: &str) -> bool {
    matches!(s.split_once(':'), Some((host, path)) if host.contains('@') && !host.contains('/') && !path.is_empty())
}

/// (owner, name) of a GitHub repository link.
fn github(s: &str) -> Option<(String, String)> {
    let rest = if let Some(r) = s.strip_prefix("git@github.com:").or_else(|| s.strip_prefix("ssh://git@github.com/")) {
        r
    } else {
        let r = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://")).unwrap_or(s);
        let r = r.strip_prefix("www.").unwrap_or(r);
        match r.strip_prefix("github.com/") {
            Some(r) => r,
            // the "owner/name" shorthand
            None if !s.contains(':') && !s.starts_with(['/', '.', '~']) && s.matches('/').count() == 1 => s,
            None => return None,
        }
    };
    let mut parts = rest.split('/').filter(|p| !p.is_empty());
    let owner = parts.next()?;
    let name = parts.next()?;
    let name = name.strip_suffix(".git").unwrap_or(name);
    let owner_ok = owner.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    let name_ok = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    (owner_ok && name_ok).then(|| (owner.to_string(), name.to_string()))
}

/// (workspace, repository) of a Bitbucket Cloud link: https://bitbucket.org/<workspace>/<repository> with or without a
/// user name before the host (https://jefsev@bitbucket.org/…, the address Bitbucket's Clone button gives), ".git", or a
/// page inside the repository (/src/master/, /pull-requests/12); over SSH git@ssh.bitbucket.org:<workspace>/<repository>.git
/// and ssh://git@ssh.bitbucket.org/… (Bitbucket's SSH host since GA-315), and the same on its old SSH host bitbucket.org.
fn bitbucket(s: &str) -> Option<(String, String)> {
    let rest = on_bitbucket(s)?;
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let mut parts = rest.split('/').filter(|p| !p.is_empty());
    let workspace = parts.next()?;
    let name = parts.next()?;
    let name = name.strip_suffix(".git").unwrap_or(name);
    let ok = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    (ok(workspace) && ok(name)).then(|| (workspace.to_string(), name.to_string()))
}

/// How an address on Bitbucket Cloud starts over SSH: on its SSH host ssh.bitbucket.org (the host
/// `gizai_agents::connection::BITBUCKET_SSH_HOST` pushes and fetches go to), and on its old one, bitbucket.org, which
/// stops taking SSH but is still written in many remotes.
const BITBUCKET_SSH: [&str; 4] = ["git@ssh.bitbucket.org:", "ssh://git@ssh.bitbucket.org/", "git@bitbucket.org:", "ssh://git@bitbucket.org/"];

/// The path of an address on Bitbucket Cloud (what follows "bitbucket.org/", "git@ssh.bitbucket.org:" or
/// "git@bitbucket.org:"), if it is one.
fn on_bitbucket(s: &str) -> Option<&str> {
    if let Some(r) = BITBUCKET_SSH.iter().find_map(|start| s.strip_prefix(start)) {
        return Some(r);
    }
    let r = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://")).unwrap_or(s);
    // a user name (or user:password) before the host is never kept
    let r = match r.split_once('@') {
        Some((user, host)) if !user.contains('/') => host,
        _ => r,
    };
    let r = r.strip_prefix("www.").unwrap_or(r);
    r.strip_prefix("bitbucket.org/")
}
