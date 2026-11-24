//! The docs page's MCP section, as yodocs read it (its src/content/mcp.ts and mcp-tools.ts): the MCP servers the
//! project's agents connect to (`.mcp.json`, Claude Code's project file), with what `docs/mcp.md` says about each, and
//! the tools, resources and prompts the project's own Laravel MCP server declares, read from their classes. A server's
//! values never show: a URL's credentials and an argument's secret become placeholders, env and headers only by name.
use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::index::{IndexEntry, McpAnnotation, McpInput, McpServer, McpTool, Problem};
use crate::{markdown, Reference};

/// Where a project keeps its MCP servers, best first (yodocs' MCP_FILES), and its notes on them.
const MCP_FILES: [&str; 3] = [".mcp.json", ".cursor/mcp.json", "mcp.json"];
const NOTES: &str = "docs/mcp.md";
const MAX_BYTES: usize = 256 * 1024;

/// The servers in the project's MCP file, by name, and what was wrong with it.
pub fn servers(repo: &Path, files: &[String]) -> (Vec<McpServer>, Vec<Problem>) {
    let tracked = |p: &str| files.iter().any(|f| f == p);
    let Some(file) = MCP_FILES.iter().find(|f| tracked(f)) else { return (vec![], vec![]) };
    let Some(text) = crate::read_text(repo, file, MAX_BYTES) else { return (vec![], vec![]) };
    let notes = if tracked(NOTES) { crate::read_text(repo, NOTES, MAX_BYTES).map(|t| notes(&t)).unwrap_or_default() } else { HashMap::new() };
    parse(file, &text, &notes)
}

/// `.mcp.json`'s servers (`mcpServers`, or `servers`), each with its notes from `notes` by lower-case name.
pub fn parse(file: &str, text: &str, notes: &HashMap<String, String>) -> (Vec<McpServer>, Vec<Problem>) {
    let mut problems = vec![];
    let json: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => return (vec![], vec![Problem { file: file.into(), message: format!("is not valid JSON: {e}"), line: None }]),
    };
    let block = json.get("mcpServers").or_else(|| json.get("servers")).and_then(Value::as_object);
    let mut servers: Vec<McpServer> = block.into_iter().flatten().filter_map(|(name, raw)| {
        let s = raw.as_object()?;
        let text_of = |k: &str| s.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        let (url, command) = (text_of("url"), text_of("command"));
        let transport = if !url.is_empty() { "http" } else if !command.is_empty() { "stdio" } else { "unknown" };
        let target = if url.is_empty() { command } else { strip_url(&url, file, name, &mut problems) };
        let args = s.get("args").and_then(Value::as_array).map(|a| a.iter().map(|arg| strip_arg(arg, file, name, &mut problems)).collect())
            .unwrap_or_default();
        let names = |k: &str| s.get(k).and_then(Value::as_object).map(|o| o.keys().cloned().collect()).unwrap_or_default();
        Some(McpServer {
            name: name.clone(),
            transport: transport.into(),
            target,
            args,
            env_names: names("env"),
            header_names: names("headers"),
            purpose: notes.get(&name.to_lowercase()).cloned().unwrap_or_default(),
        })
    }).collect();
    servers.sort_by_key(|s| s.name.to_lowercase());
    (servers, problems)
}

/// A URL without its user and password, and with each query value as `${KEY}`.
fn strip_url(url: &str, file: &str, server: &str, problems: &mut Vec<Problem>) -> String {
    let Some((scheme, rest)) = url.split_once("://") else { return url.to_string() };
    let (before_query, query) = rest.split_once('?').map_or((rest, None), |(a, q)| (a, Some(q)));
    let (authority, path) = before_query.find('/').map_or((before_query, ""), |i| before_query.split_at(i));
    let mut stripped = false;
    let host = match authority.rsplit_once('@') {
        Some((_, host)) => {
            stripped = true;
            host
        }
        None => authority,
    };
    let query = query.map(|q| {
        let (q, fragment) = q.split_once('#').map_or((q, None), |(q, f)| (q, Some(f)));
        let pairs: Vec<String> = q.split('&').filter(|p| !p.is_empty()).map(|pair| {
            stripped = true;
            let key = pair.split_once('=').map_or(pair, |(k, _)| k);
            format!("{key}=${{{}}}", key.to_uppercase())
        }).collect();
        format!("?{}{}", pairs.join("&"), fragment.map(|f| format!("#{f}")).unwrap_or_default())
    }).unwrap_or_default();
    if stripped {
        problems.push(Problem { file: file.into(), line: None,
            message: format!("the URL for \"{server}\" carried credentials in it, which have been replaced with placeholders") });
    }
    format!("{scheme}://{host}{path}{query}")
}

/// An argument that carries a value rather than naming one: `--api-key=abc`.
static INLINE_VALUE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(--?[\w-]*(?:token|key|secret|password|pass|auth|credential)[\w-]*)=(.+)$").unwrap());

fn strip_arg(arg: &Value, file: &str, server: &str, problems: &mut Vec<Problem>) -> String {
    let arg = arg.as_str().map_or_else(|| arg.to_string(), str::to_string);
    match INLINE_VALUE.captures(&arg) {
        Some(c) => {
            problems.push(Problem { file: file.into(), line: None,
                message: format!("an argument to \"{server}\" carried a value for {}, which has been replaced with a placeholder", &c[1]) });
            format!("{}=${{VALUE}}", &c[1])
        }
        None => arg,
    }
}

/// `docs/mcp.md`'s sections by lower-case heading, as HTML: a heading named after a server says what it is for.
fn notes(text: &str) -> HashMap<String, String> {
    static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^##+[ \t]+").unwrap());
    HEADING.split(text).skip(1).map(|part| {
        let (head, rest) = part.split_once('\n').unwrap_or((part, ""));
        (head.trim().to_lowercase(), markdown::render(rest).html)
    }).collect()
}

/// `#[Title('Search blueprints')]`, `#[IsReadOnly]`: a PHP attribute and what is in its brackets.
static ATTRIBUTE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#\[\s*([A-Za-z_]\w*)\s*(?:\(([^\]]*)\))?\s*\]").unwrap());
/// `protected string $name = 'search_blueprints';`
static NAME_PROPERTY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\$name\s*=\s*['"]([^'"]+)['"]"#).unwrap());
static QUOTED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^\s*['"]([\s\S]*)['"]\s*$"#).unwrap());
/// `'brief' => $schema->string()`: an input's name and type.
static INPUT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"['"]([\w.-]+)['"]\s*=>\s*\$schema\s*->\s*(\w+)\s*\("#).unwrap());
static SCHEMA_BODY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"function\s+schema\s*\([^)]*\)[^{]*\{([\s\S]*?)\n\s{4}\}").unwrap());
static DESCRIPTION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"->\s*description\s*\(\s*['"]([\s\S]*?)['"]\s*\)"#).unwrap());
static REQUIRED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"->\s*required\s*\(").unwrap());
/// PHP attributes that aren't a tool's.
const NOT_METADATA: [&str; 4] = ["Attribute", "Override", "Deprecated", "ReturnTypeWillChange"];

/// The tools, resources and prompts the project's own MCP server declares: each class the Reference found as one (not
/// an abstract one), with its name, title, description, annotations and inputs from its attributes and schema method.
pub fn tools(reference: &Reference, entries: &[IndexEntry]) -> Vec<McpTool> {
    let mut out: Vec<McpTool> = reference.entries.iter().zip(entries).filter_map(|(source, entry)| {
        let sort = match entry.kind.as_str() { "mcp tool" => "tool", "mcp resource" => "resource", "mcp prompt" => "prompt", _ => return None };
        if entry.signature.trim_start().starts_with("abstract") {
            return None;
        }
        let text = &source.text;
        let attributes: Vec<McpAnnotation> = ATTRIBUTE.captures_iter(text).filter(|c| !NOT_METADATA.contains(&&c[1]))
            .map(|c| {
                let raw = c.get(2).map_or("", |m| m.as_str()).trim();
                let value = QUOTED.captures(raw).map_or(raw.to_string(), |q| q[1].to_string());
                McpAnnotation { name: c[1].to_string(), value: unescape(&value) }
            }).collect();
        let name = NAME_PROPERTY.captures(text).map_or_else(|| snake(&entry.name), |c| c[1].to_string());
        let named = |n: &str| attributes.iter().find(|a| a.name == n).map(|a| a.value.clone());
        Some(McpTool {
            id: format!("mcp/{sort}/{name}"),
            title: named("Title").unwrap_or_else(|| entry.name.clone()),
            description: named("Description").unwrap_or_else(|| entry.description.clone()),
            annotations: attributes.iter().filter(|a| a.name != "Title" && a.name != "Description").cloned().collect(),
            inputs: inputs(text),
            sort: sort.into(),
            file: entry.file.clone(),
            entry: entry.id.clone(),
            name,
        })
    }).collect();
    out.sort_by(|a, b| a.sort.cmp(&b.sort).then_with(|| a.name.cmp(&b.name)));
    out
}

/// The schema method's keys, each with its type, description and whether it is required: read from the text between
/// one key and the next, as the calls are chained, maybe over several lines.
fn inputs(text: &str) -> Vec<McpInput> {
    let source = SCHEMA_BODY.captures(text).and_then(|c| c.get(1)).map_or(text, |m| m.as_str());
    let hits: Vec<_> = INPUT.captures_iter(source).collect();
    hits.iter().enumerate().map(|(i, c)| {
        let from = c.get(0).map_or(0, |m| m.start());
        let to = hits.get(i + 1).and_then(|n| n.get(0)).map_or(source.len(), |m| m.start());
        let chain = &source[from..to];
        McpInput {
            name: c[1].to_string(),
            kind: c[2].to_string(),
            description: DESCRIPTION.captures(chain).map(|d| unescape(&d[1])).unwrap_or_default(),
            required: REQUIRED.is_match(chain),
        }
    }).collect()
}

/// PHP's `\'`, `\"` and `\\` in a quoted string, as the characters.
fn unescape(text: &str) -> String {
    static ESCAPED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\\(['"\\])"#).unwrap());
    ESCAPED.replace_all(text, "$1").into_owned()
}

/// `SearchBlueprints` → `search_blueprints`, for a tool that names no `$name`.
fn snake(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (i, c) in chars.iter().enumerate() {
        let after_lower = i > 0 && (chars[i - 1].is_lowercase() || chars[i - 1].is_ascii_digit());
        let acronym_end = i > 0 && chars[i - 1].is_uppercase() && chars.get(i + 1).is_some_and(|n| n.is_lowercase());
        if c.is_uppercase() && (after_lower || acronym_end) {
            out.push('_');
        }
        out.extend(c.to_lowercase());
    }
    out
}
