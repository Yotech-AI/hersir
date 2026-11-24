// A project's GitHub link: the forms people paste, stored one way, matched against a repository's remotes.
use gizai_core::{db::Db, model::ProjectInput, projects, repo_url, seed::ensure_seed};

#[test]
fn every_usual_github_form_becomes_one_link() {
    for form in ["https://github.com/acme-labs/shop-app", "https://github.com/acme-labs/shop-app.git", "http://www.github.com/acme-labs/shop-app/",
                 "git@github.com:acme-labs/shop-app.git", "ssh://git@github.com/acme-labs/shop-app.git", "github.com/acme-labs/shop-app",
                 "acme-labs/shop-app", "  https://github.com/acme-labs/shop-app/tree/main  "] {
        let r = repo_url::normalize(form).unwrap().unwrap_or_else(|| panic!("{form}"));
        assert_eq!((r.url.as_str(), r.provider.as_str(), r.owner.as_deref(), r.name.as_deref()),
                   ("https://github.com/acme-labs/shop-app", "github", Some("acme-labs"), Some("shop-app")), "{form}");
    }
    assert!(repo_url::normalize("").unwrap().is_none());
    assert!(repo_url::normalize("   ").unwrap().is_none());
}

#[test]
fn other_git_hosts_and_paths_are_kept_as_given() {
    let r = repo_url::normalize("git@gitlab.com:team/app.git").unwrap().unwrap();
    assert_eq!((r.url.as_str(), r.provider.as_str()), ("git@gitlab.com:team/app.git", "git"));
    assert_eq!(repo_url::normalize("/srv/git/app.git").unwrap().unwrap().url, "/srv/git/app.git");
    for bad in ["not a url", "https://github.com/only-owner", "ftp://x"] {
        assert!(repo_url::normalize(bad).is_err(), "{bad} accepted");
    }
}

#[test]
fn a_remote_matches_the_link_whatever_its_form() {
    assert!(repo_url::same_repo("git@github.com:Acme-Labs/shop-app.git", "https://github.com/acme-labs/shop-app"));
    assert!(!repo_url::same_repo("git@github.com:acme-labs/shop-app.git", "https://github.com/acme-labs/shop"));
    assert!(repo_url::same_repo("/srv/git/app.git/", "/srv/git/app.git"));
    assert!(repo_url::same_repo("/srv/git/app", "file:///srv/git/app.git"));
}

#[test]
fn a_project_keeps_its_github_link_next_to_its_local_repository() {
    let db = Db::open_in_memory().unwrap();
    let s = ensure_seed(&db, "Jeffrey").unwrap();
    let input = ProjectInput { name: "Shop".into(), key: "SH".into(), repo_path: Some("/home/x/shop".into()),
        repo_url: Some("git@github.com:acme-labs/shop-app.git".into()), ..Default::default() };
    let id = projects::create(&db, &s.you_id, input.clone()).unwrap();
    assert_eq!(projects::get(&db, &id).unwrap().repo_url.as_deref(), Some("https://github.com/acme-labs/shop-app"));
    projects::update(&db, &s.you_id, &id, ProjectInput { repo_url: None, ..input.clone() }).unwrap();
    assert_eq!(projects::get(&db, &id).unwrap().repo_url, None, "the link can be removed");
    let e = projects::update(&db, &s.you_id, &id, ProjectInput { repo_path: None, ..input }).unwrap_err();
    assert!(e.to_string().contains("local"), "{e}");
}

// ---- GA-318 (GA-270 #1, CWE-88): a link git, or the ssh it starts, would read as an option is never saved ----

/// Links that start with "-", or whose scp host, URL host, user name or port does. The first is the audit's: git took it
/// as `--upload-pack=…` and ran it through sh.
const OPTION_LINKS: [&str; 11] = [
    "--upload-pack=touch${IFS}PWNED;#@x:y",
    "-oProxyCommand=x",
    "  --upload-pack=touch${IFS}PWNED;#@x:y  ",
    "git@-oProxyCommand=x:acme/app.git",
    "-oProxyCommand=x@host:acme/app.git",
    "ssh://-oProxyCommand=x/acme/app.git",
    "ssh://git@-oProxyCommand=x/acme/app.git",
    "ssh://-oProxyCommand=x@git.example.com/acme/app.git",
    "ssh://git@git.example.com:-oProxyCommand=x/acme/app.git",
    "git://-oProxyCommand=x/acme/app.git",
    "-acme/app",
];

fn option_sentence(link: &str) -> String {
    format!("{} isn't a repository link: it, or its user name, host or port, starts with \"-\", which git reads as an option", link.trim())
}

#[test]
fn a_link_git_would_read_as_an_option_is_refused_with_a_sentence() {
    for bad in OPTION_LINKS {
        assert!(repo_url::reads_as_option(bad), "{bad}");
        assert_eq!(repo_url::normalize(bad).expect_err(bad).to_string(), option_sentence(bad), "{bad}");
        assert_eq!(repo_url::refuse_option(bad).expect_err(bad).to_string(), option_sentence(bad), "{bad}");
    }
}

#[test]
fn usual_links_still_work_https_git_at_bitbucket_ssh_and_a_local_path() {
    let kept = |form: &str, url: &str, provider: &str| {
        assert!(!repo_url::reads_as_option(form), "{form}");
        repo_url::refuse_option(form).unwrap_or_else(|e| panic!("{form}: {e}"));
        let r = repo_url::normalize(form).unwrap_or_else(|e| panic!("{form}: {e}")).unwrap();
        assert_eq!((r.url.as_str(), r.provider.as_str()), (url, provider), "{form}");
    };
    kept("https://github.com/acme-labs/shop-app", "https://github.com/acme-labs/shop-app", "github");
    kept("git@github.com:owner/repo.git", "https://github.com/owner/repo", "github");
    kept("ssh://git@github.com/owner/repo.git", "https://github.com/owner/repo", "github");
    kept("git@ssh.bitbucket.org:acme/shop-app.git", "https://bitbucket.org/acme/shop-app", "bitbucket");
    kept("ssh://git@ssh.bitbucket.org/acme/shop-app.git", "https://bitbucket.org/acme/shop-app", "bitbucket");
    kept("https://jefsev@bitbucket.org/acme/shop-app.git", "https://bitbucket.org/acme/shop-app", "bitbucket");
    kept("/srv/git/app.git", "/srv/git/app.git", "git");
    kept("file:///srv/git/app.git", "file:///srv/git/app.git", "git");
    kept("git@gitlab.com:team/app.git", "git@gitlab.com:team/app.git", "git");
    // a "-" inside a host, a user name or a path is no option
    kept("git@my-git.example.com:team/app.git", "git@my-git.example.com:team/app.git", "git");
    kept("my-user@git.example.com:team/app.git", "my-user@git.example.com:team/app.git", "git");
    kept("ssh://git@git-host.example.com:2222/team/app.git", "ssh://git@git-host.example.com:2222/team/app.git", "git");
    kept("/srv/git/-odd/app.git", "/srv/git/-odd/app.git", "git");
    kept("https://git.example.com/team/-app.git", "https://git.example.com/team/-app.git", "git");
}

#[test]
fn a_project_with_a_link_git_would_read_as_an_option_is_never_saved_on_create_or_update() {
    let db = Db::open_in_memory().unwrap();
    let s = ensure_seed(&db, "Jeffrey").unwrap();
    let input = |key: &str, link: &str| ProjectInput { name: format!("Shop {key}"), key: key.into(), repo_path: Some("/home/x/shop".into()),
        repo_url: Some(link.into()), ..Default::default() };
    let before = projects::list(&db).unwrap().len();
    for (i, bad) in OPTION_LINKS.iter().enumerate() {
        let e = projects::create(&db, &s.you_id, input(&format!("B{i}"), bad)).expect_err(bad).to_string();
        assert_eq!(e, option_sentence(bad), "{bad}");
    }
    assert_eq!(projects::list(&db).unwrap().len(), before, "a refused project isn't made");
    let id = projects::create(&db, &s.you_id, input("SH", "git@github.com:acme-labs/shop-app.git")).unwrap();
    for bad in OPTION_LINKS {
        let e = projects::update(&db, &s.you_id, &id, input("SH", bad)).expect_err(bad).to_string();
        assert_eq!(e, option_sentence(bad), "{bad}");
        assert_eq!(projects::get(&db, &id).unwrap().repo_url.as_deref(), Some("https://github.com/acme-labs/shop-app"), "{bad} changed nothing");
    }
}
