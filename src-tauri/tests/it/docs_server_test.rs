//! Docs in a browser (`docs_server`): the page and its files for this computer only (another host name refused, a file
//! name with `..` refused); the read-only Docs calls of a project with Docs on answered; a project with Docs off, any
//! other command and a call without what it needs refused.
use std::collections::HashMap;
use std::sync::Arc;

use gizai_core::model::ProjectInput;
use gizai_lib::docs_server::{self, Assets};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// The server on a free port of 127.0.0.1, with a page and one script as its files.
async fn server(st: gizai_lib::AppState) -> u16 {
    let files: HashMap<String, (Vec<u8>, String)> = [
        ("docs-web/docs.html", "<!doctype html><title>Hersir docs</title>", "text/html"),
        ("docs-web/assets/docs-1a2b.js", "console.log(1)", "text/javascript"),
    ].into_iter().map(|(p, b, m)| (p.to_string(), (b.as_bytes().to_vec(), m.to_string()))).collect();
    let assets: Assets = Arc::new(move |p: &str| files.get(p).cloned());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(docs_server::serve(listener, st, assets));
    port
}

/// The status and body of one request.
async fn send(port: u16, method: &str, path: &str, host: &str, body: &str) -> (u16, String) {
    let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let request = format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len());
    s.write_all(request.as_bytes()).await.unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    let code = out.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    (code, out.split_once("\r\n\r\n").map(|(_, b)| b.to_string()).unwrap_or_default())
}

fn project(st: &gizai_lib::AppState, name: &str, key: &str, docs: bool) -> String {
    gizai_core::projects::create(&st.db, &st.you_id, ProjectInput { name: name.into(), key: key.into(), dev_docs: Some(docs),
        ..Default::default() }).unwrap()
}

#[tokio::test]
async fn the_page_and_its_files_for_this_computer_only() {
    let tmp = tempfile::tempdir().unwrap();
    let port = server(gizai_lib::test_state(tmp.path())).await;
    let host = format!("127.0.0.1:{port}");
    assert_eq!(send(port, "GET", "/", &host, "").await, (200, "<!doctype html><title>Hersir docs</title>".into()));
    assert_eq!(send(port, "GET", "/assets/docs-1a2b.js", &format!("localhost:{port}"), "").await, (200, "console.log(1)".into()));
    assert_eq!(send(port, "GET", "/", &format!("evil.example:{port}"), "").await.0, 403, "another host name");
    assert_eq!(send(port, "GET", "/", "127.0.0.1", "").await.0, 403, "without this port");
    assert_eq!(send(port, "GET", "/assets/..%2Fsecret", &host, "").await.0, 404);
    assert_eq!(send(port, "GET", "/assets/missing.js", &host, "").await.0, 404);
    assert_eq!(send(port, "DELETE", "/", &host, "").await.0, 404);
}

#[tokio::test]
async fn only_the_docs_calls_of_projects_with_docs_on() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    let on = project(&st, "Pearl", "PEARL", true);
    let off = project(&st, "Kiosk", "KIOSK", false);
    let port = server(st).await;
    let host = format!("127.0.0.1:{port}");
    let call = |cmd: &str, args: String| { let (h, c) = (host.clone(), format!("/api/{cmd}")); async move { send(port, "POST", &c, &h, &args).await } };

    let (code, body) = call("list_projects", "{}".into()).await;
    assert_eq!(code, 200);
    let list: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(list.as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect::<Vec<_>>(), ["Pearl"], "only projects with Docs on");
    assert_eq!(call("get_project", format!(r#"{{"id":"{on}"}}"#)).await.0, 200);
    assert_eq!(call("docs_changelog", format!(r#"{{"projectId":"{on}"}}"#)).await, (200, "[]".into()));
    assert_eq!(call("docs_changelog", format!(r#"{{"projectId":"{off}"}}"#)).await, (403, "Docs is off for Kiosk.".into()));
    assert_eq!(call("get_project", format!(r#"{{"id":"{off}"}}"#)).await.0, 403);
    assert_eq!(call("docs_changelog", r#"{"projectId":"no-such-project"}"#.into()).await.0, 404);
    assert_eq!(call("save_project", format!(r#"{{"projectId":"{on}"}}"#)).await, (404, "Hersir's docs don't answer save_project.".into()));
    assert_eq!(call("docs_changelog", "{}".into()).await, (400, "docs_changelog needs projectId".into()));
    assert_eq!(call("docs_commits", format!(r#"{{"projectId":"{on}"}}"#)).await,
               (500, "Pearl has no git repository: link one on its project page → Edit".into()));
}
