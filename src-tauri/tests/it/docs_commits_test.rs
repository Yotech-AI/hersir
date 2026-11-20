//! Docs → Commits through the app (`changes::commits_of_project`, `files_of_commit`, `file_of_commit`): main's newest
//! commits in a project's linked folder, newest first; a commit's files against its first parent, a first commit's
//! against the empty tree; one file's texts; and a project without a repository, which says how to link one.
use std::path::Path;
use std::process::Command;

use gizai_core::model::ProjectInput;
use gizai_lib::changes::{self, FileStatus};

/// git in `dir` as a fixed person, whatever this computer's git config says about line ends and signing.
fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").args(["-c", "user.email=t@t", "-c", "user.name=Tess", "-c", "core.autocrlf=false", "-c", "commit.gpgsign=false"])
        .args(args).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim_end().to_string()
}

/// Writes `text` to `path` in `dir`, commits it as `message` and returns the commit's id.
fn commit(dir: &Path, path: &str, text: &str, message: &str) -> String {
    std::fs::write(dir.join(path), text).unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    git(dir, &["rev-parse", "HEAD"])
}

/// A project named `name` with Docs on, linked to `repo` (no link to GitHub), its main branch main.
fn project(st: &gizai_lib::AppState, name: &str, key: &str, repo: Option<&Path>) -> String {
    gizai_core::projects::create(&st.db, &st.you_id, ProjectInput { name: name.into(), key: key.into(),
        repo_path: repo.map(|r| r.to_string_lossy().into_owned()), default_branch: Some("main".into()), dev_docs: Some(true),
        ..Default::default() }).unwrap()
}

#[test]
fn a_projects_commits_newest_first_with_each_ones_files_and_texts() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    let repo = tmp.path().join("pearl");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    let first = commit(&repo, "readme.md", "Pearl\n", "First");
    let second = commit(&repo, "readme.md", "Pearl\nCorporate site\n", "Say what it is");
    let id = project(&st, "Pearl", "PEARL", Some(&repo));

    let list = changes::commits_of_project(&st, &id).unwrap();
    assert_eq!(list.label, "main", "a project without a link: its local main");
    assert_eq!(list.commits.iter().map(|c| (c.sha.as_str(), c.subject.as_str())).collect::<Vec<_>>(),
               [(second.as_str(), "Say what it is"), (first.as_str(), "First")]);

    let files = changes::files_of_commit(&st, &id, &second).unwrap();
    assert_eq!((files.from.as_str(), files.to.as_str()), (first.as_str(), second.as_str()), "measured from its first parent");
    assert_eq!(files.files.iter().map(|f| (f.path.as_str(), f.status, f.additions, f.deletions)).collect::<Vec<_>>(),
               [("readme.md", FileStatus::Modified, Some(1), Some(0))]);
    let diff = changes::file_of_commit(&st, &id, "readme.md", None, &files.from, &files.to).unwrap();
    assert_eq!((diff.old_text.as_deref(), diff.new_text.as_deref()), (Some("Pearl\n"), Some("Pearl\nCorporate site\n")));

    let started = changes::files_of_commit(&st, &id, &first).unwrap();
    assert_ne!(started.from, first, "the first commit is measured from the empty tree");
    assert_eq!(started.files.iter().map(|f| (f.path.as_str(), f.status)).collect::<Vec<_>>(), [("readme.md", FileStatus::Added)]);
}

#[test]
fn a_project_without_a_repository_says_how_to_link_one_and_a_commit_id_is_checked() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    let bare = project(&st, "Kiosk", "KIOSK", None);
    assert_eq!(changes::commits_of_project(&st, &bare).unwrap_err(), "Kiosk has no git repository: link one on its project page → Edit");

    let repo = tmp.path().join("shop");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    commit(&repo, "a.txt", "a\n", "First");
    let shop = project(&st, "Shop", "SHOP", Some(&repo));
    assert_eq!(changes::files_of_commit(&st, &shop, "--all").unwrap_err(), "\"--all\" isn't a commit id");
}

#[test]
fn a_projects_reference_reads_the_files_git_tracks() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    let repo = tmp.path().join("pearl");
    std::fs::create_dir_all(repo.join("src")).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("src/Header.tsx"), "/** The site's header. */\nexport function Header() {\n  return <header />;\n}\n").unwrap();
    commit(&repo, "src/menu.ts", "export function useMenu() { return 1; }\n", "First");
    std::fs::write(repo.join("src/untracked.ts"), "export function notYet() {}\n").unwrap();
    let id = project(&st, "Pearl", "PEARL", Some(&repo));

    let r = gizai_lib::devdocs::reference_of(&st, &id).unwrap();
    assert_eq!(r.entries.iter().map(|e| (e.name.as_str(), e.kind.as_str(), e.path.as_str())).collect::<Vec<_>>(),
               [("Header", "component", "src/Header.tsx"), ("useMenu", "hook", "src/menu.ts")], "tracked files only, an untracked one left out");
    assert_eq!(r.entries[0].doc, "The site's header.");
}

#[test]
fn a_projects_api_reads_its_route_files() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    let repo = tmp.path().join("pearl");
    std::fs::create_dir_all(repo.join("app/api/ping")).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    commit(&repo, "app/api/ping/route.ts", "export async function GET() { return Response.json({}); }\n", "Ping");
    let id = project(&st, "Pearl", "PEARL", Some(&repo));
    let routes = gizai_lib::devdocs::api_of(&st, &id).unwrap();
    assert_eq!(routes.iter().map(|r| (r.method.as_str(), r.path.as_str(), r.line)).collect::<Vec<_>>(), [("GET", "/api/ping", 1)]);
}
