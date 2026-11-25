//! Docs → API: the routes a project answers on, read from its files without running it. Ported from yodocs' probes,
//! except that a Laravel app's routes are read from its `routes/` files instead of asked of artisan:
//! - Next.js: the app router's `route.*` files (their exported methods) and `page.*` files, and the pages router's files;
//! - Laravel: the `Route::` calls in `routes/*.php` (not console or channels), with their prefix and controller groups,
//!   `match`, `view`, `resource` and `apiResource`; `routes/api.php` under `/api`.
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

/// One route, as Docs → API lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    /// GET, POST… for an endpoint; ANY when every method reaches it (a pages router API file); PAGE for a page.
    pub method: String,
    /// The URL, its parameters as the project writes them: `/invoices/[id]` in Next.js, `/invoices/{invoice}` in Laravel.
    pub path: String,
    /// What answers it in Laravel (`InvoiceController@show`, `closure`, `view welcome`); empty in Next.js.
    pub handler: String,
    pub file: String,
    pub line: usize,
}

const HTTP: [&str; 7] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
const SCRIPTS: [&str; 5] = ["ts", "tsx", "js", "jsx", "mjs"];

/// The routes in `files` (relative to `repo`, as git lists them): Next.js's, then Laravel's, each in file order.
pub fn api(repo: &Path, files: &[String]) -> Vec<Route> {
    let read = |f: &str| std::fs::read_to_string(repo.join(f)).ok();
    let mut out = next_routes(files, &read);
    for f in files.iter().filter(|f| laravel_file(f)) {
        if let Some(text) = read(f) {
            laravel_routes(&text, f, &mut out);
        }
    }
    out
}

// ---- Next.js ----

/// Three ways a route file exports a method: `export async function GET`, `export const GET = …`, and
/// `export const { GET, POST } = …` (how better-auth mounts itself).
static EXPORTED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"export\s+(?:async\s+)?(?:function\s+(\w+)|const\s+(?:\{([^}]+)\}|(\w+))\s*[=:])").unwrap()
});

fn line_at(text: &str, at: usize) -> usize {
    text[..at].matches('\n').count() + 1
}

/// The HTTP methods a route file exports, each with its line, in HTTP's order.
fn methods_in(text: &str) -> Vec<(&'static str, usize)> {
    let mut found: Vec<(&'static str, usize)> = vec![];
    for m in EXPORTED.captures_iter(text) {
        let line = line_at(text, m.get(0).map_or(0, |g| g.start()));
        let list = m.get(2).map_or("", |g| g.as_str());
        let names = [m.get(1).map(|g| g.as_str()), m.get(3).map(|g| g.as_str())].into_iter().flatten().chain(list.split(','));
        for name in names {
            // a renamed destructure is `GET: handler`, and the export is still GET
            let name = name.split(':').next().unwrap_or("").trim();
            if let Some(method) = HTTP.iter().find(|h| **h == name) && !found.iter().any(|(m, _)| m == method) {
                found.push((method, line));
            }
        }
    }
    found.sort_by_key(|(m, _)| HTTP.iter().position(|h| h == m));
    found
}

/// The URL a folder answers on: a route group `(name)` and a parallel slot `@name` don't show in it, and a private
/// folder `_name` isn't routed at all (None). `[id]` and `[...all]` stay as the project writes them.
fn url_for(segments: &[&str]) -> Option<String> {
    let mut kept = vec![];
    for s in segments {
        if s.starts_with('_') {
            return None;
        }
        if s.starts_with('@') || (s.starts_with('(') && s.ends_with(')')) {
            continue;
        }
        kept.push(*s);
    }
    Some(format!("/{}", kept.join("/")))
}

fn under<'a>(path: &'a str, roots: &[&str]) -> Option<&'a str> {
    roots.iter().find_map(|r| path.strip_prefix(r))
}

fn next_routes(files: &[String], read: &dyn Fn(&str) -> Option<String>) -> Vec<Route> {
    let mut out = vec![];
    for f in files {
        let name = f.rsplit('/').next().unwrap_or(f);
        let Some((stem, ext)) = name.rsplit_once('.') else { continue };
        if !SCRIPTS.contains(&ext) {
            continue;
        }
        if let Some(rest) = under(f, &["src/app/", "app/"]) && (stem == "route" || stem == "page") {
            let segments: Vec<&str> = rest.split('/').collect();
            let Some(url) = url_for(&segments[..segments.len() - 1]) else { continue };
            if stem == "page" {
                out.push(Route { method: "PAGE".into(), path: url, handler: String::new(), file: f.clone(), line: 1 });
            } else if let Some(text) = read(f) {
                for (method, line) in methods_in(&text) {
                    out.push(Route { method: method.into(), path: url.clone(), handler: String::new(), file: f.clone(), line });
                }
            }
        } else if let Some(rest) = under(f, &["src/pages/", "pages/"]) {
            if rest.split('/').any(|s| s.starts_with('_')) {
                continue;
            }
            let route = &rest[..rest.len() - ext.len() - 1];
            let route = route.strip_suffix("/index").or_else(|| (route == "index").then_some("")).unwrap_or(route);
            let method = if rest.starts_with("api/") { "ANY" } else { "PAGE" };
            out.push(Route { method: method.into(), path: format!("/{route}"), handler: String::new(), file: f.clone(), line: 1 });
        }
    }
    out
}

// ---- Laravel ----

fn laravel_file(f: &str) -> bool {
    f.strip_prefix("routes/").is_some_and(|name| name.ends_with(".php") && !matches!(name, "console.php" | "channels.php"))
}

static VERB: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)(?:Route::|->)(get|post|put|patch|delete|options|any)\s*\(\s*(['"])(.*?)['"]\s*,\s*(.*)"#).unwrap()
});
static MATCH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)(?:Route::|->)match\s*\(\s*\[([^\]]*)\]\s*,\s*['"](.*?)['"]\s*,\s*(.*)"#).unwrap()
});
static RESOURCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:Route::|->)(resource|apiResource)\s*\(\s*['"](.*?)['"]\s*,\s*([\w\\]+)::class"#).unwrap()
});
static ONLY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"->(only|except)\s*\(\s*\[([^\]]*)\]").unwrap());
static VIEW: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"Route::view\s*\(\s*['"](.*?)['"]\s*,\s*['"](.*?)['"]"#).unwrap());
static PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"prefix\s*\(\s*['"]([^'"]*)['"]\s*\)|['"]prefix['"]\s*=>\s*['"]([^'"]*)['"]"#).unwrap()
});
static CONTROLLER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"controller\s*\(\s*([\w\\]+)::class\s*\)").unwrap());
static ACTION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^\[\s*([\w\\]+)::class\s*,\s*['"](\w+)['"]\s*\]"#).unwrap());
static AT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^['"]([\w\\]+)@(\w+)['"]"#).unwrap());
static CLASS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([\w\\]+)::class").unwrap());
static METHOD_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^['"](\w+)['"]"#).unwrap());

/// A group a route is inside: its prefix, its controller, and the brace depth its closure opened at.
struct Group {
    prefix: String,
    controller: Option<String>,
    depth: usize,
}

fn last(name: &str) -> &str {
    name.rsplit('\\').next().unwrap_or(name)
}

/// What answers a route, from the text after its URI: `Controller@method`, `closure`, or an action of the group's
/// controller.
fn handler(text: &str, controller: Option<&str>) -> String {
    let t = text.trim_start();
    if let Some(c) = ACTION.captures(t) {
        return format!("{}@{}", last(&c[1]), &c[2]);
    }
    if let Some(c) = AT.captures(t) {
        return format!("{}@{}", last(&c[1]), &c[2]);
    }
    if let Some(c) = CLASS.captures(t) {
        return last(&c[1]).to_string();
    }
    if t.starts_with("function") || t.starts_with("fn") || t.starts_with("static") {
        return "closure".into();
    }
    match (controller, METHOD_NAME.captures(t)) {
        (Some(ctrl), Some(c)) => format!("{ctrl}@{}", &c[1]),
        _ => String::new(),
    }
}

fn join(prefixes: &[&str], uri: &str) -> String {
    let parts: Vec<&str> = prefixes.iter().copied().chain([uri]).flat_map(|p| p.split('/')).filter(|s| !s.is_empty()).collect();
    format!("/{}", parts.join("/"))
}

/// `photos` → `photo`, as Laravel names a resource's parameter (the usual English plurals).
fn singular(word: &str) -> String {
    if let Some(stem) = word.strip_suffix("ies") {
        format!("{stem}y")
    } else if word.ends_with("sses") || word.ends_with("shes") || word.ends_with("ches") || word.ends_with("xes") {
        word[..word.len() - 2].to_string()
    } else {
        word.strip_suffix('s').unwrap_or(word).to_string()
    }
}

/// The routes `Route::resource('photos', …)` makes, without create and edit for `apiResource`; `photos.comments` nests
/// under `/photos/{photo}`.
fn resource(name: &str, controller: &str, api: bool) -> Vec<(&'static str, String, String)> {
    let parts: Vec<&str> = name.split('.').filter(|p| !p.is_empty()).collect();
    let Some((leaf, parents)) = parts.split_last() else { return vec![] };
    let base: String = parents.iter().map(|p| format!("{p}/{{{}}}/", singular(p))).collect::<String>() + leaf;
    let one = format!("{base}/{{{}}}", singular(leaf));
    let mut out = vec![("GET", base.clone(), "index")];
    if !api {
        out.push(("GET", format!("{base}/create"), "create"));
    }
    out.push(("POST", base, "store"));
    out.push(("GET", one.clone(), "show"));
    if !api {
        out.push(("GET", format!("{one}/edit"), "edit"));
    }
    out.push(("PUT", one.clone(), "update"));
    out.push(("DELETE", one, "destroy"));
    out.into_iter().map(|(m, p, action)| (m, p, format!("{controller}@{action}"))).collect()
}

/// The routes in one Laravel routes file, appended to `out`.
fn laravel_routes(text: &str, file: &str, out: &mut Vec<Route>) {
    let base = if file == "routes/api.php" { "/api" } else { "" };
    let bytes = text.as_bytes();
    let mut groups: Vec<Group> = vec![];
    let mut depth = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' => i = skip_string(bytes, i),
            b'/' if bytes.get(i + 1) == Some(&b'/') => i = text[i..].find('\n').map_or(bytes.len(), |n| i + n),
            b'#' if bytes.get(i + 1) != Some(&b'[') => i = text[i..].find('\n').map_or(bytes.len(), |n| i + n),
            b'/' if bytes.get(i + 1) == Some(&b'*') => i = text[i..].find("*/").map_or(bytes.len(), |n| i + n + 2),
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth = depth.saturating_sub(1);
                groups.retain(|g| g.depth <= depth);
                i += 1;
            }
            b'R' if text[i..].starts_with("Route::") => {
                let (end, opens_group) = statement_end(bytes, i);
                let statement = &text[i..end];
                let line = line_at(text, i);
                if opens_group {
                    let prefix = PREFIX.captures(statement).and_then(|c| c.get(1).or(c.get(2))).map_or("", |g| g.as_str());
                    let controller = CONTROLLER.captures(statement).map(|c| last(&c[1]).to_string())
                        .or_else(|| groups.last().and_then(|g| g.controller.clone()));
                    depth += 1;
                    groups.push(Group { prefix: prefix.to_string(), controller, depth });
                    i = end + 1;
                    continue;
                }
                let mut prefixes: Vec<&str> = vec![base];
                prefixes.extend(groups.iter().map(|g| g.prefix.as_str()));
                let controller = groups.last().and_then(|g| g.controller.as_deref());
                let mut add = |method: &str, uri: &str, handler: String| {
                    out.push(Route { method: method.into(), path: join(&prefixes, uri), handler, file: file.into(), line });
                };
                if let Some(c) = RESOURCE.captures(statement) {
                    // ->only([...]) keeps those actions, ->except([...]) drops them
                    let limit = ONLY.captures(statement).map(|o| {
                        let actions: Vec<String> = o[2].split(',').map(|a| a.trim().trim_matches(['\'', '"']).to_string()).collect();
                        (&o[1] == "only", actions)
                    });
                    for (m, p, h) in resource(&c[2], last(&c[3]), &c[1] == "apiResource") {
                        let action = h.rsplit('@').next().unwrap_or("").to_string();
                        if limit.as_ref().is_none_or(|(only, list)| list.contains(&action) == *only) {
                            add(m, &p, h);
                        }
                    }
                } else if let Some(c) = MATCH.captures(statement) {
                    let h = handler(&c[3], controller);
                    for m in c[1].split(',').map(|m| m.trim().trim_matches(['\'', '"']).to_uppercase()).filter(|m| !m.is_empty()) {
                        add(&m, &c[2], h.clone());
                    }
                } else if let Some(c) = VIEW.captures(statement) {
                    add("GET", &c[1], format!("view {}", &c[2]));
                } else if let Some(c) = VERB.captures(statement) {
                    let method = if &c[1] == "any" { "ANY".to_string() } else { c[1].to_uppercase() };
                    add(&method, &c[3], handler(&c[4], controller));
                }
                i = end;
            }
            _ => i += 1,
        }
    }
}

/// Past the end of the quoted string that starts at `i`.
fn skip_string(bytes: &[u8], i: usize) -> usize {
    let quote = bytes[i];
    let mut j = i + 1;
    while j < bytes.len() {
        match bytes[j] {
            b'\\' => j += 2,
            b if b == quote => return j + 1,
            _ => j += 1,
        }
    }
    bytes.len()
}

/// Where the `Route::` statement at `i` ends: at its `;` (closures inside it included), or at the `{` that opens a
/// group's closure (true), where the routes inside it start.
fn statement_end(bytes: &[u8], i: usize) -> (usize, bool) {
    let (mut parens, mut braces) = (0i32, 0i32);
    let mut j = i;
    while j < bytes.len() {
        match bytes[j] {
            b'\'' | b'"' => {
                j = skip_string(bytes, j);
                continue;
            }
            b'(' => parens += 1,
            b')' => parens -= 1,
            b'{' if braces == 0 && is_group(&bytes[i..j]) => return (j, true),
            b'{' => braces += 1,
            b'}' => braces -= 1,
            b';' if parens <= 0 && braces <= 0 => return (j + 1, false),
            _ => {}
        }
        j += 1;
    }
    (bytes.len(), false)
}

/// Whether a statement so far is a group whose closure starts here: `->group(function () {` or `Route::group([…], function () {`.
fn is_group(so_far: &[u8]) -> bool {
    let text = String::from_utf8_lossy(so_far);
    text.contains("->group(") || text.starts_with("Route::group(")
}
