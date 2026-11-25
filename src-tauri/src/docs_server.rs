//! The docs pages in a browser: Hersir serves each project's docs page at http://127.0.0.1:<port>/p/<project id>/, for
//! this computer only (Open in browser on a project's Docs page). The page is src/docs built into one HTML file (`PAGE`,
//! `vite build --mode docs-web`), the same page Hersir's window shows; it reads the project's `index.json`
//! (`devdocs::index_of`), a file's source (`source/<path>.json`) and a commit's changed lines (`commits/<hash>.json`) from
//! here, and `llms.txt` and `md/…` are the same docs for agents. Only projects with Docs on are served, and only to GET. A
//! request whose Host isn't 127.0.0.1 or localhost at this port (a page elsewhere pointing a name of its own at it) gets a
//! 403. One request per connection: it is one person's browser on this computer.
use std::net::TcpListener as StdListener;
use std::sync::{LazyLock, Mutex};

use gizai_core::model::Project;
use gizai_core::projects;
use gizai_docs::markdown::escape;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::AppState;

/// The ports tried, in order: the first free one serves.
pub const PORTS: std::ops::RangeInclusive<u16> = 4380..=4399;
const MAX_HEAD: usize = 16 * 1024;
/// The Docs page, one file with its script, styles and fonts inside (build.rs; a page that says so when it wasn't built).
pub const PAGE: &str = include_str!(concat!(env!("OUT_DIR"), "/docs.html"));

/// The scripts written inside `html` (a `<script>` without a src), as the browser hashes them.
pub fn inline_scripts(html: &str) -> Vec<&str> {
    let mut out = vec![];
    let mut rest = html;
    while let Some(start) = rest.find("<script") {
        let tag = &rest[start..];
        let (Some(open_end), Some(close)) = (tag.find('>'), tag.find("</script>")) else { break };
        if open_end < close && !tag[..open_end].contains(" src=") {
            out.push(&tag[open_end + 1..close]);
        }
        rest = &tag[close + "</script>".len()..];
    }
    out
}

/// What may run and load: the page's own inline script, by its hash, and calls back to here; nothing may frame it.
pub static CSP: LazyLock<String> = LazyLock::new(|| {
    let scripts: String = inline_scripts(PAGE).iter()
        .map(|s| format!(" 'sha256-{}'", gizai_agents::bitbucket::base64(&Sha256::digest(s.as_bytes()))))
        .collect();
    format!("default-src 'self'; script-src 'self'{scripts}; style-src 'self' 'unsafe-inline'; img-src 'self' data:; \
             font-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'")
});

/// The port Hersir's docs server listens on, once started.
static PORT: Mutex<Option<u16>> = Mutex::new(None);

/// What a request gets.
#[derive(Debug, PartialEq, Eq)]
pub enum Reply {
    /// A media type and the body.
    Ok(&'static str, Vec<u8>),
    /// A page's address without its last `/`, sent to the one with it: the page's own links are relative to it.
    Moved(String),
    /// A status and why.
    Refused(u16, String),
}

/// Starts the docs server the first time (127.0.0.1, the first free port of `PORTS`), and gives its address. It runs
/// until Hersir quits.
#[tauri::command]
pub async fn docs_serve(st: tauri::State<'_, AppState>) -> Result<String, String> {
    let port = start(st.inner().clone())?;
    Ok(format!("http://127.0.0.1:{port}"))
}

/// Starts the server once; later calls give the same port.
pub fn start(st: AppState) -> Result<u16, String> {
    let mut port = PORT.lock().unwrap();
    if let Some(p) = *port {
        return Ok(p);
    }
    let (listener, p) = PORTS.clone().find_map(|p| StdListener::bind(("127.0.0.1", p)).ok().map(|l| (l, p)))
        .ok_or_else(|| format!("Hersir couldn't find a free port for the docs from {} to {}", PORTS.start(), PORTS.end()))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let listener = TcpListener::from_std(listener).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn(serve(listener, st));
    *port = Some(p);
    Ok(p)
}

/// Answers every connection to `listener` until the process ends.
pub async fn serve(listener: TcpListener, st: AppState) {
    let Ok(port) = listener.local_addr().map(|a| a.port()) else { return };
    loop {
        let Ok((stream, _)) = listener.accept().await else { continue };
        let st = st.clone();
        tokio::spawn(async move {
            let _ = handle(stream, st, port).await;
        });
    }
}

async fn handle(mut stream: TcpStream, st: AppState, port: u16) -> std::io::Result<()> {
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
            return respond(&mut stream, Reply::Refused(431, "The request's header is too big.".into())).await;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut request = lines.next().unwrap_or("").split(' ');
    let (method, target) = (request.next().unwrap_or(""), request.next().unwrap_or(""));
    let host = lines.filter_map(|l| l.split_once(':')).find(|(k, _)| k.trim().eq_ignore_ascii_case("host")).map(|(_, v)| v.trim());
    if host != Some(&format!("127.0.0.1:{port}")) && host != Some(&format!("localhost:{port}")) {
        return respond(&mut stream, Reply::Refused(403, "Forbidden. Hersir's docs answer to 127.0.0.1 and localhost only.".into())).await;
    }
    if method != "GET" {
        return respond(&mut stream, Reply::Refused(405, "Hersir's docs are read only.".into())).await;
    }
    let Some(path) = decode(target.split(['?', '#']).next().unwrap_or("/")) else {
        return respond(&mut stream, Reply::Refused(400, "The address isn't valid.".into())).await;
    };
    let reply = tauri::async_runtime::spawn_blocking(move || route(&st, &path)).await
        .unwrap_or_else(|e| Reply::Refused(500, e.to_string()));
    respond(&mut stream, reply).await
}

/// What a GET of `path` gets: the list of projects with Docs on at `/`; a project's page, index, sources and commits
/// under `/p/<id>/`; nothing anywhere else.
pub fn route(st: &AppState, path: &str) -> Reply {
    if path == "/" {
        return projects_page(st);
    }
    let Some(rest) = path.strip_prefix("/p/") else { return not_found() };
    let Some((id, file)) = rest.split_once('/') else { return Reply::Moved(format!("/p/{rest}/")) };
    let project = match projects::get(&st.db, id) {
        Ok(p) => p,
        Err(_) => return Reply::Refused(404, "There's no such project.".into()),
    };
    if !project.dev_docs {
        return Reply::Refused(403, format!("Docs is off for {}.", project.name));
    }
    let json = |value: Result<Vec<u8>, serde_json::Error>| match value {
        Ok(body) => Reply::Ok("application/json", body),
        Err(e) => Reply::Refused(500, e.to_string()),
    };
    if file.is_empty() {
        return Reply::Ok("text/html; charset=utf-8", PAGE.as_bytes().to_vec());
    }
    if file == "index.json" {
        return match crate::devdocs::index_of(st, &project) {
            Ok(index) => json(serde_json::to_vec(&index)),
            Err(why) => Reply::Refused(500, why),
        };
    }
    // for agents: llms.txt, and every page as Markdown, as plain text so a browser shows it
    if file == "llms.txt" || file.starts_with("md/") {
        let index = match crate::devdocs::index_of(st, &project) {
            Ok(index) => index,
            Err(why) => return Reply::Refused(500, why),
        };
        let text = match file.strip_prefix("md/") {
            Some(md) => gizai_docs::agents::page(&index, md, &|path| crate::devdocs::source_of(&project, path).map(|s| s.lines)),
            None => Some(gizai_docs::agents::llms_txt(&index)),
        };
        return text.map_or_else(not_found, |t| Reply::Ok("text/plain; charset=utf-8", t.into_bytes()));
    }
    if let Some(source) = file.strip_prefix("source/").and_then(|f| f.strip_suffix(".json")) {
        return match crate::devdocs::source_of(&project, source) {
            Some(s) => json(serde_json::to_vec(&s)),
            None => not_found(),
        };
    }
    if let Some(commit) = file.strip_prefix("commits/").and_then(|f| f.strip_suffix(".json")) {
        return match crate::devdocs::patches_of(&project, commit) {
            Ok(p) => json(serde_json::to_vec(&p)),
            Err(why) => Reply::Refused(404, why),
        };
    }
    not_found()
}

fn not_found() -> Reply {
    Reply::Refused(404, "Not found.".into())
}

/// The projects with Docs on, each linking to its page: a plain list in the system's font and theme.
fn projects_page(st: &AppState) -> Reply {
    let all = match projects::list(&st.db) {
        Ok(all) => all,
        Err(e) => return Reply::Refused(500, e.to_string()),
    };
    let on: Vec<&Project> = all.iter().filter(|p| p.dev_docs).collect();
    let list = if on.is_empty() {
        "<p>No project has Docs on. Turn it on in Hersir: a project's Edit, under Docs.</p>".to_string()
    } else {
        let rows: String = on.iter()
            .map(|p| format!("<li><a href=\"p/{}/\">{}</a></li>", escape(&p.id), escape(&p.name)))
            .collect();
        format!("<ul>{rows}</ul>")
    };
    let page = format!("<!doctype html>
<html lang=\"en\">
  <head>
    <meta charset=\"utf-8\" />
    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />
    <meta name=\"robots\" content=\"noindex, nofollow\" />
    <title>Docs · Hersir</title>
    <style>:root {{ color-scheme: dark light; font-family: system-ui, sans-serif; }} body {{ max-width: 640px; margin: 48px auto; padding: 0 16px; }}
      a {{ color: #7b9bff; }} li {{ margin: 6px 0; }}</style>
  </head>
  <body>
    <h1>Docs</h1><p>The projects with Docs on in Hersir, on this computer.</p>{list}
  </body>
</html>
");
    Reply::Ok("text/html; charset=utf-8", page.into_bytes())
}

/// A request path with its `%XX` escapes decoded, or None when they aren't UTF-8 or hide a NUL.
fn decode(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok().filter(|p| !p.contains('\0'))
}

async fn respond(stream: &mut TcpStream, reply: Reply) -> std::io::Result<()> {
    let (code, reason, mime, location, body) = match reply {
        Reply::Ok(mime, body) => (200, "OK", mime, None, body),
        Reply::Moved(to) => (301, "Moved Permanently", "text/plain", Some(to), b"Moved.".to_vec()),
        Reply::Refused(code, text) => {
            let reason = match code {
                400 => "Bad Request",
                403 => "Forbidden",
                404 => "Not Found",
                405 => "Method Not Allowed",
                431 => "Request Header Fields Too Large",
                _ => "Error",
            };
            (code, reason, "text/plain; charset=utf-8", None, text.into_bytes())
        }
    };
    let location = location.map(|l| format!("Location: {l}\r\n")).unwrap_or_default();
    let head = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\n{location}Content-Security-Policy: {}\r\n\
X-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len(), *CSP
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(&body).await?;
    stream.flush().await
}
