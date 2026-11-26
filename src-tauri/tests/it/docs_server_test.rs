//! The docs pages (`docs_server`): for this computer only and only to GET; the list of projects with Docs on; a
//! project's page (src/docs built into one file, its one script allowed by its hash and nothing framing it), its index (yodocs' index.json) from its linked folder, a file's source only for a
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
    send_raw(port, format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\n\r\n").as_bytes()).await
}

/// The status, head and body of a request written as `raw`.
async fn send_raw(port: u16, raw: &[u8]) -> (u16, String, String) {
    let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    s.write_all(raw).await.unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    let code = out.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    let (head, body) = out.split_once("\r\n\r\n").unwrap_or_default();
    (code, head.to_string(), body.to_string())
}

/// The docs server for `st` on a free port, and its port.
async fn serving(st: gizai_lib::AppState) -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(docs_server::serve(listener, st));
    port
}

/// `path` behind this start's key: `/t/<key><path>`.
fn keyed(path: &str) -> String {
    format!("/t/{}{path}", docs_server::token().unwrap())
}

// GA-362: every address carries the key, so the paths here do too (`keyed`)
#[tokio::test]
async fn for_this_computer_only_and_only_to_get() {
    let tmp = tempfile::tempdir().unwrap();
    let port = serving(gizai_lib::test_state(tmp.path())).await;
    let host = format!("127.0.0.1:{port}");

    let (code, head, body) = send(port, "GET", &keyed("/"), &host).await;
    assert_eq!(code, 200);
    assert!(body.contains("<title>Docs · Hersir</title>"), "{body}");
    assert!(head.contains("Content-Security-Policy: default-src 'self'; script-src 'self'"), "{head}");
    assert!(head.contains("frame-ancestors 'none'"), "nothing frames it: {head}");
    assert_eq!(send(port, "GET", &keyed("/"), &format!("localhost:{port}")).await.0, 200);
    assert_eq!(send(port, "GET", &keyed("/fonts.css"), &host).await.0, 404, "the page brings its own styles and fonts");
    assert_eq!(send(port, "GET", &keyed("/"), &format!("evil.example:{port}")).await.0, 403, "another host name");
    assert_eq!(send(port, "GET", &keyed("/"), "127.0.0.1").await.0, 403, "without this port");
    assert_eq!(send(port, "POST", &keyed("/"), &host).await.0, 405);
    assert_eq!(send(port, "GET", &keyed("/index.html"), &host).await.0, 404, "the page is a project's only");
    assert_eq!(send(port, "GET", &keyed("/p/%zz/"), &host).await.0, 400);
    let (code, head, _) = send(port, "GET", &keyed("/p/abc"), &host).await;
    assert_eq!(code, 301);
    assert!(head.contains(&format!("Location: {}", keyed("/p/abc/"))), "{head}");
}

/// GA-362 QA: a repository with a file and a commit, linked to a project with Docs on; its id.
fn pearl(st: &gizai_lib::AppState, root: &Path) -> String {
    let repo = root.join("pearl");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    write(&repo, "src/invoice.ts", "/** An invoice. */\nexport class Invoice {\n}\n");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "Add invoices"]);
    project(st, "Pearl", "PEARL", true, Some(&repo))
}

#[test]
fn the_key_is_64_hex_characters_from_the_systems_random_source_the_same_until_hersir_quits() {
    let key = docs_server::token().unwrap();
    assert_eq!(key.len(), 64, "32 bytes: {key}");
    assert!(key.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)), "lower-case hex: {key}");
    assert_ne!(key, "0".repeat(64));
    assert_eq!(docs_server::token().unwrap(), key, "one key for this start");
}

#[test]
fn only_the_whole_key_opens_the_way_compared_whole() {
    let key = "0123456789abcdef";
    assert_eq!(docs_server::behind_key("/t/0123456789abcdef/p/abc/", key), Some("/p/abc/"));
    assert_eq!(docs_server::behind_key("/t/0123456789abcdef/", key), Some("/"));
    assert_eq!(docs_server::behind_key("/t/0123456789abcdef", key), Some(""));
    for wrong in ["/", "/p/abc/", "/t/", "/t", "/t//p/abc/", "/t/0123456789abcde/p/abc/", "/t/0123456789abcdef0/p/abc/",
                  "/t/1123456789abcdef/p/abc/", "/t/0123456789abcdee/p/abc/", "/t/0123456789ABCDEF/p/abc/", "/T/0123456789abcdef/p/abc/",
                  "/x/t/0123456789abcdef/p/abc/", "t/0123456789abcdef/p/abc/", "/t/0123456789abcdef?/p/abc/", ""] {
        assert_eq!(docs_server::behind_key(wrong, key), None, "{wrong:?}");
    }
}

#[tokio::test]
async fn without_the_key_every_docs_address_is_not_found_and_with_it_every_page_works() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    let id = pearl(&st, tmp.path());
    let port = serving(st.clone()).await;
    let host = format!("127.0.0.1:{port}");
    let key = docs_server::token().unwrap();
    let flipped = format!("{}{}", &key[..63], if key.ends_with('0') { '1' } else { '0' });

    let pages = ["/".to_string(), format!("/p/{id}"), format!("/p/{id}/"), format!("/p/{id}/index.json"), format!("/p/{id}/llms.txt"),
        format!("/p/{id}/md/reference/src/invoice.ts/Invoice.md"), format!("/p/{id}/source/src/invoice.ts.json")];
    for page in &pages {
        for without in [page.clone(), format!("/t{page}"), format!("/t/{flipped}{page}"), format!("/t/{}{page}", &key[..63]),
                        format!("/t/{key}0{page}"), format!("/t/{}{page}", key.to_uppercase()), format!("/t//{key}{page}"),
                        format!("/x/t/{key}{page}"), format!("/t/{page}")] {
            let (code, head, body) = send(port, "GET", &without, &host).await;
            assert_eq!((code, body.as_str()), (404, "Not found."), "{without}");
            assert!(!head.contains("Location"), "no way on from {without}: {head}");
        }
    }
    // with it: the list of projects, a project's page, its index, a file's source and the docs for agents
    let (code, _, list) = send(port, "GET", &keyed("/"), &host).await;
    assert_eq!(code, 200);
    assert!(list.contains(&format!(r#"<a href="p/{id}/">Pearl</a>"#)), "relative, so it keeps the key: {list}");
    let (code, head, _) = send(port, "GET", &keyed(""), &host).await;
    assert_eq!(code, 301, "the key without its last /");
    assert!(head.contains(&format!("Location: /t/{key}/\r\n")) || head.ends_with(&format!("Location: /t/{key}/")), "{head}");
    let (code, head, _) = send(port, "GET", &keyed(&format!("/p/{id}")), &host).await;
    assert_eq!(code, 301);
    assert!(head.contains(&format!("Location: /t/{key}/p/{id}/")), "{head}");
    let (code, _, page) = send(port, "GET", &keyed(&format!("/p/{id}/")), &host).await;
    assert_eq!((code, page.as_str()), (200, docs_server::PAGE));
    let (code, _, index) = send(port, "GET", &keyed(&format!("/p/{id}/index.json")), &host).await;
    assert_eq!(code, 200);
    assert!(index.contains("src/invoice.ts#Invoice"), "{index}");
    let (code, _, llms) = send(port, "GET", &keyed(&format!("/p/{id}/llms.txt")), &host).await;
    assert_eq!(code, 200);
    assert!(llms.starts_with("# Pearl dev docs\n"), "{llms}");
    assert!(!llms.contains("http://") && !llms.contains("/p/"), "its links are relative to it, so they keep the key: {llms}");
    let (code, _, md) = send(port, "GET", &keyed(&format!("/p/{id}/md/reference/src/invoice.ts/Invoice.md")), &host).await;
    assert_eq!(code, 200);
    assert!(md.contains("export class Invoice"), "{md}");
    let (code, _, source) = send(port, "GET", &keyed(&format!("/p/{id}/source/src/invoice.ts.json")), &host).await;
    assert_eq!(code, 200);
    assert!(source.contains("export class Invoice {"), "{source}");
}

#[tokio::test]
async fn a_request_a_page_on_another_site_made_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    let id = pearl(&st, tmp.path());
    let port = serving(st).await;
    let index = keyed(&format!("/p/{id}/index.json"));
    let with = |site: &str| format!("GET {index} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nSec-Fetch-Site: {site}\r\nSec-Fetch-Mode: cors\r\n\r\n");
    for site in ["cross-site", "Cross-Site", " cross-site "] {
        let (code, _, body) = send_raw(port, with(site).as_bytes()).await;
        assert_eq!(code, 403, "{site:?}: {body}");
        assert!(!body.contains("Invoice"), "{body}");
    }
    let lower = format!("GET {index} HTTP/1.1\r\nhost: 127.0.0.1:{port}\r\nsec-fetch-site: cross-site\r\n\r\n");
    assert_eq!(send_raw(port, lower.as_bytes()).await.0, 403, "a field's name in any case");
    for site in ["same-origin", "none"] {
        assert_eq!(send_raw(port, with(site).as_bytes()).await.0, 200, "{site}: the page's own reads, an address typed or opened");
    }
}

#[tokio::test]
async fn a_line_break_or_other_control_character_in_an_address_gets_400_and_adds_no_header() {
    let tmp = tempfile::tempdir().unwrap();
    let port = serving(gizai_lib::test_state(tmp.path())).await;
    let host = format!("127.0.0.1:{port}");
    let key = docs_server::token().unwrap();
    let injected = [
        keyed("/p/abc%0d%0aSet-Cookie:%20a=1"), keyed("/p/abc%0d%0aSet-Cookie:%20a=1/"), keyed("/p/abc%0D%0ASet-Cookie:%20a=1"),
        keyed("/p/abc%0aSet-Cookie:%20a=1"), keyed("/p/abc%0dSet-Cookie:%20a=1"), keyed("%0d%0aSet-Cookie:%20a=1"),
        format!("/t/{key}%0d%0aSet-Cookie:%20a=1/p/abc"), "/p/abc%0d%0aSet-Cookie:%20a=1".to_string(),
        keyed("/p/abc%00"), keyed("/p/a%09b"), keyed("/p/a%7fb"), keyed("/p/a%c2%85b"),
    ];
    for target in &injected {
        let (code, head, _) = send(port, "GET", target, &host).await;
        assert_eq!(code, 400, "{target}: {head}");
        assert!(!head.to_lowercase().contains("set-cookie") && !head.contains("Location"), "{target}: {head}");
    }
    // not %-encoded: a bare line feed, a tab or a byte past ASCII in the address itself
    for raw in [format!("GET {} HTTP/1.1\r\nHost: {host}\r\n\r\n", keyed("/p/abc\nSet-Cookie: a=1")),
                format!("GET {} HTTP/1.1\r\nHost: {host}\r\n\r\n", keyed("/p/a\tb")),
                format!("GET {} HTTP/1.1\r\nHost: {host}\r\n\r\n", keyed("/p/abé"))] {
        let (code, head, _) = send_raw(port, raw.as_bytes()).await;
        assert_eq!(code, 400, "{raw:?}: {head}");
        assert!(!head.to_lowercase().contains("set-cookie") && !head.contains("Location"), "{raw:?}: {head}");
    }
    // a redirect's Location is the address as sent, never a decoded one, and leaves the query behind
    let (code, head, _) = send(port, "GET", &keyed("/p/a%20b"), &host).await;
    assert_eq!(code, 301);
    assert!(head.contains(&format!("Location: /t/{key}/p/a%20b/")), "{head}");
    let (code, head, _) = send(port, "GET", &keyed("/p/abc?x=%0d%0aSet-Cookie:%20a=1"), &host).await;
    assert_eq!(code, 301, "{head}");
    assert!(head.contains(&format!("Location: /t/{key}/p/abc/")) && !head.to_lowercase().contains("set-cookie"), "{head}");
    // every header line of every answer is one line
    for line in head.split("\r\n") {
        assert!(!line.contains('\n') && !line.contains('\r'), "{line:?}");
    }
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
    assert_eq!(page, docs_server::PAGE, "the one built page");
    assert_eq!(route(&st, &format!("/p/{id}/js/main.js")), Reply::Refused(404, "Not found.".into()), "no files of its own");

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
    assert!(list.contains(&format!(r#"<li><a href="p/{on}/">Pearl</a></li>"#)), "{list}");
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

#[test]
fn the_pages_own_script_runs_by_its_hash_and_no_other() {
    use sha2::{Digest, Sha256};
    let page = r#"<html><head><script type="module" crossorigin>console.log(1)</script><script src="x.js"></script></head></html>"#;
    assert_eq!(docs_server::inline_scripts(page), vec!["console.log(1)"], "the inline one, not the one with a src");
    for script in docs_server::inline_scripts(docs_server::PAGE) {
        let hash = gizai_agents::bitbucket::base64(&Sha256::digest(script.as_bytes()));
        assert!(docs_server::CSP.contains(&format!("'sha256-{hash}'")), "{}", *docs_server::CSP);
    }
    assert!(docs_server::CSP.contains("frame-ancestors 'none'"));
}

/// GA-362 QA: 156 KB of nested `[` doesn't stall the index: the file is cut off at its limit and Home names it, and the
/// next view of the same repository takes it from what the last one kept (`devdocs`'s KEPT) instead of parsing it again.
#[test]
fn a_slow_file_is_cut_off_and_named_and_the_next_index_takes_it_from_what_was_kept() {
    use std::time::{Duration, Instant};
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    let repo = tmp.path().join("slow");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    write(&repo, "src/slow.ts", &format!("export const nested = {};\n", "[".repeat(156 * 1024)));
    write(&repo, "src/fine.ts", "export function fine() {}\n");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "Slow"]);
    let id = project(&st, "Slow", "SLOW", true, Some(&repo));
    let index = || {
        let started = Instant::now();
        let index = json(route(&st, &format!("/p/{id}/index.json")));
        (index, started.elapsed())
    };
    let cut_off = |index: &serde_json::Value| index["problems"].as_array().unwrap().iter()
        .any(|p| p["file"] == "src/slow.ts" && p["message"] == "took too long to read, so its declarations may be incomplete");

    let (first, took) = index();
    assert!(cut_off(&first), "named on Home: {}", first["problems"]);
    assert!(took < gizai_docs::FILE_TIME * 10, "not stalled: {took:?}");
    assert!(first["entries"].as_array().unwrap().iter().any(|e| e["id"] == "src/fine.ts#fine"), "the rest is read as usual");
    let (second, again) = index();
    assert!(cut_off(&second), "{}", second["problems"]);
    assert_eq!(second["entries"], first["entries"]);
    assert!(again + Duration::from_millis(500) < took, "the next view from what was kept: {again:?}, the first {took:?}");
}
