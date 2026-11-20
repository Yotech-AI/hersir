//! Docs in a browser: Hersir serves its projects' Docs pages at http://127.0.0.1:<port>/, for this computer only. The
//! page is the app's own Docs page, built on its own into the app's docs-web/ (docs.html, vite.docs.config.ts). Its
//! calls come back here as `POST /api/<command>` with JSON, and only the read-only Docs commands of projects with Docs
//! on are answered. A request whose Host isn't 127.0.0.1 or localhost at this port (a page elsewhere pointing a name of
//! its own at it) gets a 403. One request per connection: it is one person's browser on this computer.
use std::net::TcpListener as StdListener;
use std::sync::{Arc, Mutex};

use gizai_core::model::Project;
use gizai_core::projects;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::AppState;

/// The page's files, by path inside the app's build (`docs-web/docs.html`, `docs-web/assets/…`): their bytes and media type.
pub type Assets = Arc<dyn Fn(&str) -> Option<(Vec<u8>, String)> + Send + Sync>;

/// The ports tried, in order: the first free one serves.
pub const PORTS: std::ops::RangeInclusive<u16> = 4380..=4399;
const MAX_HEAD: usize = 16 * 1024;
const MAX_BODY: usize = 64 * 1024;
/// The page's code runs only from here; its calls go only here.
const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; \
font-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";

/// The port Hersir's docs server listens on, once started.
static PORT: Mutex<Option<u16>> = Mutex::new(None);

/// Opens the docs in a browser: starts the server the first time (127.0.0.1, the first free port of `PORTS`), and gives
/// its address. It runs until Hersir quits.
#[tauri::command]
pub async fn docs_serve(app: tauri::AppHandle, st: tauri::State<'_, AppState>) -> Result<String, String> {
    // shortcut: in a release build Tauri answers a file it doesn't have with the app's index.html (manager::get_asset),
    // so a missing file is that page, not a 404; check the name against asset_resolver().iter() if that ever matters.
    let assets: Assets = Arc::new(move |path: &str| {
        app.asset_resolver().get(path.to_string()).map(|a| (a.bytes().to_vec(), a.mime_type().to_string()))
    });
    let port = start(st.inner().clone(), assets)?;
    Ok(format!("http://127.0.0.1:{port}"))
}

/// Starts the server once; later calls give the same port.
pub fn start(st: AppState, assets: Assets) -> Result<u16, String> {
    let mut port = PORT.lock().unwrap();
    if let Some(p) = *port {
        return Ok(p);
    }
    let (listener, p) = PORTS.clone().find_map(|p| StdListener::bind(("127.0.0.1", p)).ok().map(|l| (l, p)))
        .ok_or_else(|| format!("Hersir couldn't find a free port for the docs from {} to {}", PORTS.start(), PORTS.end()))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let listener = TcpListener::from_std(listener).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn(serve(listener, st, assets));
    *port = Some(p);
    Ok(p)
}

/// Answers every connection to `listener` until the process ends.
pub async fn serve(listener: TcpListener, st: AppState, assets: Assets) {
    let Ok(port) = listener.local_addr().map(|a| a.port()) else { return };
    loop {
        let Ok((stream, _)) = listener.accept().await else { continue };
        let (st, assets) = (st.clone(), assets.clone());
        tokio::spawn(async move {
            let _ = handle(stream, &st, &assets, port).await;
        });
    }
}

async fn handle(mut stream: TcpStream, st: &AppState, assets: &Assets, port: u16) -> std::io::Result<()> {
    let mut buf = Vec::new();
    let head_end = loop {
        let mut chunk = [0u8; 4096];
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if buf.len() > MAX_HEAD {
            return respond(&mut stream, 431, "text/plain", b"The request's header is too big.").await;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut request = lines.next().unwrap_or("").split(' ');
    let (method, target) = (request.next().unwrap_or(""), request.next().unwrap_or(""));
    let headers: Vec<(String, String)> = lines.filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string())).collect();
    let header = |name: &str| headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str());
    let host = header("host").unwrap_or("");
    if host != format!("127.0.0.1:{port}") && host != format!("localhost:{port}") {
        return respond(&mut stream, 403, "text/plain", b"Forbidden. Hersir's docs answer to 127.0.0.1 and localhost only.").await;
    }
    let path = target.split(['?', '#']).next().unwrap_or("/");
    match (method, path) {
        ("GET", "/") | ("GET", "/index.html") => serve_asset(&mut stream, assets, "docs-web/docs.html").await,
        ("GET", p) if p.starts_with("/assets/") && plain_name(&p["/assets/".len()..]) => {
            serve_asset(&mut stream, assets, &format!("docs-web/{}", &p[1..])).await
        }
        ("POST", p) if p.starts_with("/api/") => {
            let length: usize = header("content-length").and_then(|l| l.parse().ok()).unwrap_or(0);
            if length > MAX_BODY {
                return respond(&mut stream, 413, "text/plain", b"The request is too big.").await;
            }
            let mut body = buf[head_end + 4..].to_vec();
            while body.len() < length {
                let mut chunk = vec![0u8; length - body.len()];
                let n = stream.read(&mut chunk).await?;
                if n == 0 {
                    break;
                }
                body.extend_from_slice(&chunk[..n]);
            }
            let args: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            let (cmd, st2) = (p["/api/".len()..].to_string(), st.clone());
            let answer = tauri::async_runtime::spawn_blocking(move || api(&st2, &cmd, &args)).await
                .unwrap_or_else(|e| Err((500, e.to_string())));
            match answer {
                Ok(v) => respond(&mut stream, 200, "application/json", v.to_string().as_bytes()).await,
                Err((code, text)) => respond(&mut stream, code, "text/plain; charset=utf-8", text.as_bytes()).await,
            }
        }
        _ => respond(&mut stream, 404, "text/plain", b"Not found.").await,
    }
}

/// A file name in assets/: letters, digits, `-`, `_` and `.`, never `..`.
fn plain_name(name: &str) -> bool {
    !name.is_empty() && !name.contains("..") && name.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
}

async fn serve_asset(stream: &mut TcpStream, assets: &Assets, path: &str) -> std::io::Result<()> {
    match assets(path) {
        Some((bytes, mime)) => respond(stream, 200, &mime, &bytes).await,
        None => respond(stream, 404, "text/plain", b"Not found.").await,
    }
}

async fn respond(stream: &mut TcpStream, code: u16, mime: &str, body: &[u8]) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nContent-Security-Policy: {CSP}\r\n\
X-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await
}

/// The page's calls Hersir answers: only the read-only Docs commands, only of projects with Docs on. An error is
/// (status, text): 400 a call without what it needs, 403 Docs off, 404 no such project or command, 500 a failure.
pub fn api(st: &AppState, cmd: &str, args: &Value) -> Result<Value, (u16, String)> {
    let text = |key: &str| args.get(key).and_then(Value::as_str).map(str::to_string);
    let need = |key: &str| text(key).ok_or_else(|| (400, format!("{cmd} needs {key}")));
    let docs_on = |id: &str| -> Result<Project, (u16, String)> {
        let p = projects::get(&st.db, id).map_err(|_| (404, "There's no such project.".to_string()))?;
        if p.dev_docs { Ok(p) } else { Err((403, format!("Docs is off for {}.", p.name))) }
    };
    let json = |v: Result<Value, String>| v.map_err(|e| (500, e));
    let to = |v: serde_json::Result<Value>| v.map_err(|e| e.to_string());
    match cmd {
        "list_projects" => {
            let all = projects::list(&st.db).map_err(|e| (500, e.to_string()))?;
            json(to(serde_json::to_value(all.into_iter().filter(|p| p.dev_docs).collect::<Vec<_>>())))
        }
        "get_project" => json(to(serde_json::to_value(docs_on(&need("id")?)?))),
        _ => {
            let id = need("projectId")?;
            let project = docs_on(&id)?;
            let v = match cmd {
                "docs_reference" => crate::devdocs::reference_of(st, &id).and_then(|r| to(serde_json::to_value(r))),
                "docs_api" => crate::devdocs::api_of(st, &id).and_then(|r| to(serde_json::to_value(r))),
                "docs_guides" => crate::devdocs::guides_of(st, &id).and_then(|r| to(serde_json::to_value(r))),
                "docs_changelog" => gizai_core::devdocs::changelog(&st.db, &id).map_err(|e| e.to_string()).and_then(|r| to(serde_json::to_value(r))),
                "docs_decisions" => gizai_core::devdocs::decisions(&st.db, &project).map_err(|e| e.to_string()).and_then(|r| to(serde_json::to_value(r))),
                "docs_commits" => crate::changes::commits_of_project(st, &id).and_then(|r| to(serde_json::to_value(r))),
                "docs_commit_files" => crate::changes::files_of_commit(st, &id, &need("sha")?).and_then(|r| to(serde_json::to_value(r))),
                "docs_file_diff" => crate::changes::file_of_commit(st, &id, &need("path")?, text("oldPath").as_deref(), &need("from")?, &need("to")?)
                    .and_then(|r| to(serde_json::to_value(r))),
                _ => return Err((404, format!("Hersir's docs don't answer {cmd}."))),
            };
            json(v)
        }
    }
}
