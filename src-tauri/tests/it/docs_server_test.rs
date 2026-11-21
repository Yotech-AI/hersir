//! The docs pages (`docs_server`): for this computer only and only to GET; the list of projects with Docs on; a
//! project's page (yodocs' viewer), its index (yodocs' index.json) from its linked folder, a file's source only for a
//! file the Reference reads, a commit's changed lines; Docs off refused, an unknown project not found; a project
//! without a repository still indexed, with a problem that says how to link one.
use std::path::Path;
use std::process::Command;

use gizai_core::model::ProjectInput;
use gizai_lib::docs_server::{self, route, Reply};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// git in `dir` as a fixed person, whatever this computer's git config says about line ends and signing.
fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").args(["-c", "user.email=t@t", "-c", "user.name=Tess", "-c", "core.autocrlf=false", "-c", "commit.gpgsign=false"])
        .args(args).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim_end().to_string()
}

fn write(dir: &Path, path: &str, text: &str) {
    let p = dir.join(path);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

/// A project with Docs `on` or off, linked to `repo` (no link to GitHub), its main branch main.
fn project(st: &gizai_lib::AppState, name: &str, key: &str, on: bool, repo: Option<&Path>) -> String {
    gizai_core::projects::create(&st.db, &st.you_id, ProjectInput { name: name.into(), key: key.into(),
        repo_path: repo.map(|r| r.to_string_lossy().into_owned()), default_branch: Some("main".into()), dev_docs: Some(on),
        ..Default::default() }).unwrap()
}

fn ok(reply: Reply) -> (&'static str, String) {
    match reply {
        Reply::Ok(mime, body) => (mime, String::from_utf8(body).unwrap()),
        other => panic!("not served: {other:?}"),
    }
}

fn json(reply: Reply) -> serde_json::Value {
    let (mime, body) = ok(reply);
    assert_eq!(mime, "application/json");
    serde_json::from_str(&body).unwrap()
}

/// The status, head and body of one request.
async fn send(port: u16, method: &str, path: &str, host: &str) -> (u16, String, String) {
    let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    s.write_all(format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\n\r\n").as_bytes()).await.unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    let code = out.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    let (head, body) = out.split_once("\r\n\r\n").unwrap_or_default();
    (code, head.to_string(), body.to_string())
}

#[tokio::test]
async fn for_this_computer_only_and_only_to_get() {
    let tmp = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(docs_server::serve(listener, gizai_lib::test_state(tmp.path())));
    let host = format!("127.0.0.1:{port}");

    let (code, head, body) = send(port, "GET", "/", &host).await;
    assert_eq!(code, 200);
    assert!(body.contains("<title>Docs · Hersir</title>"), "{body}");
    assert!(head.contains("Content-Security-Policy: default-src 'self'; script-src 'self';"), "{head}");
    assert!(head.contains("frame-ancestors tauri://localhost http://tauri.localhost https://tauri.localhost"), "only Hersir's window may frame it: {head}");
    assert_eq!(send(port, "GET", "/fonts.css", &format!("localhost:{port}")).await.0, 200);
    assert_eq!(send(port, "GET", "/", &format!("evil.example:{port}")).await.0, 403, "another host name");
    assert_eq!(send(port, "GET", "/", "127.0.0.1").await.0, 403, "without this port");
    assert_eq!(send(port, "POST", "/", &host).await.0, 405);
    assert_eq!(send(port, "GET", "/index.html", &host).await.0, 404, "the page is a project's only");
    assert_eq!(send(port, "GET", "/p/%zz/", &host).await.0, 400);
    let (code, head, _) = send(port, "GET", "/p/abc", &host).await;
    assert_eq!(code, 301);
    assert!(head.contains("Location: /p/abc/"), "{head}");
}

#[test]
fn a_projects_page_index_sources_and_commits() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    let repo = tmp.path().join("pearl");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    write(&repo, "src/invoice.ts", "/** An invoice. */\nexport class Invoice extends Model {\n}\n");
    write(&repo, "README.md", "# Pearl\n\nThe site.\n");
    write(&repo, ".env", "KEY=1\n");
    write(&repo, "CHANGELOG.md", "# Changelog\n\n## [Unreleased]\n\n### Added\n- Invoices\n");
    write(&repo, "docs/decisions/0001-use-postgres.md", "# Use Postgres\n\nStatus: accepted\n");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "Add invoices"]);
    let hash = git(&repo, &["rev-parse", "--short", "HEAD"]);
    let id = project(&st, "Pearl", "PEARL", true, Some(&repo));

    assert_eq!(route(&st, &format!("/p/{id}")), Reply::Moved(format!("/p/{id}/")));
    let (mime, page) = ok(route(&st, &format!("/p/{id}/")));
    assert_eq!(mime, "text/html; charset=utf-8");
    assert!(page.contains(r#"<div id="shell">"#), "yodocs' viewer: {page}");
    assert_eq!(ok(route(&st, &format!("/p/{id}/js/main.js"))).0, "text/javascript; charset=utf-8");

    let index = json(route(&st, &format!("/p/{id}/index.json")));
    assert_eq!(index["schema"], 1);
    assert_eq!((index["project"]["title"].as_str(), index["project"]["branch"].as_str(), index["project"]["commit"].as_str()),
               (Some("Pearl dev docs"), Some("main"), Some(hash.as_str())));
    assert_eq!((index["entries"][0]["id"].as_str(), index["entries"][0]["hasDescription"].as_bool()), (Some("src/invoice.ts#Invoice"), Some(true)));
    assert_eq!(index["guides"][0]["id"], "README");
    let commit = &index["history"][0];
    assert_eq!((commit["hash"].as_str(), commit["subject"].as_str(), commit["author"].as_str()), (Some(hash.as_str()), Some("Add invoices"), Some("Tess")));
    let files: Vec<(&str, bool)> = commit["files"].as_array().unwrap().iter().map(|f| (f["path"].as_str().unwrap(), f["diff"].as_bool().unwrap())).collect();
    assert!(files.contains(&("src/invoice.ts", true)) && files.contains(&(".env", false)), "{files:?}");
    // the project's own changelog and decision records, and the records aren't guides too
    assert_eq!(index["project"]["changelogFrom"], "CHANGELOG.md in Keep a Changelog format");
    assert_eq!((index["changelog"][0]["version"].as_str(), index["changelog"][0]["changes"][0]["type"].as_str()), (Some("Unreleased"), Some("Added")));
    assert_eq!((index["decisions"][0]["number"].as_str(), index["decisions"][0]["status"].as_str()), (Some("0001"), Some("accepted")));
    assert_eq!(index["guides"].as_array().unwrap().iter().map(|g| g["file"].as_str().unwrap()).collect::<Vec<_>>(), ["README.md"]);

    let source = json(route(&st, &format!("/p/{id}/source/src/invoice.ts.json")));
    assert_eq!(source["lines"][1], "export class Invoice extends Model {");
    for refused in [".env", "README.md", "src/../.env", "nope.ts"] {
        assert_eq!(route(&st, &format!("/p/{id}/source/{refused}.json")), Reply::Refused(404, "Not found.".into()), "{refused}");
    }
    // the same docs for agents, as plain text
    let (mime, llms) = ok(route(&st, &format!("/p/{id}/llms.txt")));
    assert_eq!(mime, "text/plain; charset=utf-8");
    assert!(llms.starts_with("# Pearl dev docs\n") && llms.contains("- [0001 Use Postgres](md/decisions/0001.md) — accepted"), "{llms}");
    let (_, md) = ok(route(&st, &format!("/p/{id}/md/reference/src/invoice.ts/Invoice.md")));
    assert!(md.contains("```ts\nexport class Invoice extends Model {\n}\n```"), "{md}");
    assert_eq!(route(&st, &format!("/p/{id}/md/nope.md")), Reply::Refused(404, "Not found.".into()));
    let patches = json(route(&st, &format!("/p/{id}/commits/{hash}.json")));
    assert_eq!(patches["files"]["src/invoice.ts"]["lines"][0], "@@ -0,0 +1,3 @@");
    assert!(patches["files"].get(".env").is_none(), ".env's lines never show");
    assert!(matches!(route(&st, &format!("/p/{id}/commits/--stat.json")), Reply::Refused(404, _)));
}

#[test]
fn only_projects_with_docs_on() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    let on = project(&st, "Pearl", "PEARL", true, None);
    let off = project(&st, "Kiosk", "KIOSK", false, None);

    let (_, list) = ok(route(&st, "/"));
    assert!(list.contains(&format!(r#"<a class="row-item" href="p/{on}/"><span class="name">Pearl</span></a>"#)), "{list}");
    assert!(!list.contains("Kiosk"));
    assert_eq!(route(&st, &format!("/p/{off}/")), Reply::Refused(403, "Docs is off for Kiosk.".into()));
    assert_eq!(route(&st, &format!("/p/{off}/index.json")), Reply::Refused(403, "Docs is off for Kiosk.".into()));
    assert_eq!(route(&st, "/p/no-such-project/"), Reply::Refused(404, "There's no such project.".into()));

    // without a repository: indexed all the same, with a problem that says how to link one
    let index = json(route(&st, &format!("/p/{on}/index.json")));
    assert_eq!(index["problems"][0]["message"], "Pearl has no git repository: link one on its project page → Edit");
    assert_eq!((index["stats"]["problems"].as_u64(), index["entries"].as_array().map(Vec::len)), (Some(1), Some(0)));
}

#[test]
fn with_no_project_on_the_list_says_how_to_turn_docs_on() {
    let tmp = tempfile::tempdir().unwrap();
    let (_, list) = ok(route(&gizai_lib::test_state(tmp.path()), "/"));
    assert!(list.contains("No project has Docs on. Turn it on in Hersir: a project's Edit, under Docs."), "{list}");
}

#[test]
fn what_moved_since_the_last_commit_the_chiefs_records_and_switched_off_sections() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    let repo = tmp.path().join("pearl");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    write(&repo, "src/invoice.ts", "export class Invoice {\n}\n");
    write(&repo, "README.md", "# Pearl\n");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "First"]);
    let hash = git(&repo, &["rev-parse", "--short", "HEAD"]);
    let id = project(&st, "Pearl", "PEARL", true, Some(&repo));
    let index = || json(route(&st, &format!("/p/{id}/index.json")));

    // the first look takes the baseline at the commit: nothing moved yet
    let first = index();
    assert_eq!((first["docChanges"]["total"].as_u64(), first["docChanges"]["since"]["commit"].as_str()), (Some(0), Some(hash.as_str())));
    // a comment written above a declaration, not committed yet: it moved
    write(&repo, "src/invoice.ts", "/** An invoice. */\nexport class Invoice {\n}\n");
    let second = index();
    assert_eq!((second["docChanges"]["total"].as_u64(), second["docChanges"]["documented"][0]["name"].as_str()), (Some(1), Some("Invoice")));

    // the Chief's record, as the page shows it
    gizai_core::devdocs::record_decision(&st.db, &st.you_id, "Chief", &id, 0, gizai_core::devdocs::DecisionInput {
        title: "Use Postgres".into(), decision: "Postgres, not MySQL.".into(), context: "JSON queries.".into(), ..Default::default() }).unwrap();
    let third = index();
    let d = &third["decisions"][0];
    assert_eq!((d["id"].as_str(), d["number"].as_str(), d["status"].as_str(), d["file"].as_str()),
               (Some("decisions/0001"), Some("0001"), Some("accepted"), Some("by Chief in Hersir")));
    assert_eq!(d["sections"].as_array().unwrap().iter().map(|s| s["heading"].as_str().unwrap()).collect::<Vec<_>>(), ["Context", "Decision"]);

    // switched off in the project form: left out whole, and said so
    gizai_core::projects::update(&st.db, &st.you_id, &id, ProjectInput { name: "Pearl".into(), key: "PEARL".into(),
        repo_path: Some(repo.to_string_lossy().into_owned()), default_branch: Some("main".into()),
        docs_off: Some(vec!["guides".into(), "decisions".into()]), ..Default::default() }).unwrap();
    let off = index();
    assert_eq!((off["guides"].as_array().map(Vec::len), off["decisions"].as_array().map(Vec::len)), (Some(0), Some(0)));
    assert_eq!(off["switchedOff"].as_array().unwrap().iter().map(|o| o["section"].as_str().unwrap()).collect::<Vec<_>>(), ["guides", "decisions"]);
    assert!(off["entries"].as_array().is_some_and(|e| !e.is_empty()), "the rest stays");
}
