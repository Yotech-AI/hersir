//! The docs for agents, as yodocs wrote them (its src/output/agents.ts): `llms.txt`, an agent's way in, which names
//! every section and lists the MCP servers and endpoints whole, and one Markdown page per page of the docs, at the
//! address the page's own "Markdown" links use (`md/reference/<file>/<name>.md`, `md/<guide>.md`, `md/changelog.md`…).
use crate::index::{Index, IndexEntry};

/// `md/<path>`'s Markdown, or None when no page has that address. `source` gives a file's lines, for an entry's code.
pub fn page(index: &Index, path: &str, source: &dyn Fn(&str) -> Option<Vec<String>>) -> Option<String> {
    if let Some(rest) = path.strip_prefix("reference/") {
        return index.entries.iter().find(|e| format!("{}.md", e.id.replacen('#', "/", 1)) == rest).map(|e| entry_page(index, e, source));
    }
    if path == "changelog.md" && !index.changelog.is_empty() {
        return Some(changelog_page(index));
    }
    if path == "mcp.md" && !(index.mcp.is_empty() && index.mcp_tools.is_empty()) {
        return Some(mcp_page(index));
    }
    let id = path.strip_suffix(".md")?;
    if let Some(g) = index.guides.iter().find(|g| g.id == id) {
        return Some(format!("# {}\n\n_Source: {}_\n\n{}\n", g.title, g.file, plain(&g.html)));
    }
    index.decisions.iter().find(|d| d.id == id).map(|d| {
        let number = if d.number.is_empty() { String::new() } else { format!("{} · ", d.number) };
        format!("# {number}{}\n\n**Status:** {}  \n**Date:** {}\n\n{}\n", d.title, d.status, d.date, plain(&d.html))
    })
}

fn fence(language: &str, body: &str) -> String {
    format!("```{language}\n{body}\n```")
}

fn entry_page(index: &Index, e: &IndexEntry, source: &dyn Fn(&str) -> Option<Vec<String>>) -> String {
    let code = source(&e.file).map(|lines| lines.iter().skip(e.line_start.saturating_sub(1)).take((e.line_end + 1).saturating_sub(e.line_start.max(1)))
        .cloned().collect::<Vec<_>>().join("\n")).unwrap_or_default();
    let mut facts = vec![("Kind".to_string(), e.kind.clone()), ("File".to_string(), format!("{}:{}-{}", e.file, e.line_start, e.line_end))];
    facts.extend(e.facts.iter().map(|f| (f.label.clone(), f.value.clone())));
    facts.extend(e.tags.iter().map(|t| (format!("@{}", t.name), t.text.clone())));
    let mut out = vec![format!("# {}", e.name), String::new(),
        if e.has_description { e.description.clone() } else { "_No description in the source._".into() }, String::new(),
        "| | |".into(), "| --- | --- |".into()];
    out.extend(facts.iter().map(|(k, v)| format!("| {k} | {} |", v.replace('|', "\\|"))));
    out.push(String::new());
    if !e.signature.is_empty() {
        out.push(format!("## Signature\n\n{}\n", fence("", &e.signature)));
    }
    let inside: Vec<String> = e.children.iter().filter_map(|id| index.entries.iter().find(|x| &x.id == id)).map(|c| {
        let about = if c.has_description { format!(" — {}", c.description.lines().next().unwrap_or_default()) } else { String::new() };
        format!("- `{}` ({}){about}", c.name, c.kind)
    }).collect();
    if !inside.is_empty() {
        out.push(format!("## Inside\n\n{}\n", inside.join("\n")));
    }
    out.push(format!("## Source\n\n{}", fence(e.file.rsplit('.').next().unwrap_or_default(), &code)));
    out.push(String::new());
    out.join("\n")
}

fn changelog_page(index: &Index) -> String {
    let mut out = vec!["# Changelog".to_string(), String::new()];
    for r in &index.changelog {
        out.push(format!("## {}{}", r.version, if r.date.is_empty() { String::new() } else { format!(" · {}", r.date) }));
        out.push(String::new());
        out.extend(r.changes.iter().map(|c| format!("- **{}** {}", c.kind, c.text)));
        out.push(String::new());
    }
    out.join("\n")
}

fn mcp_page(index: &Index) -> String {
    let mut out = vec!["# MCP connections".to_string(), String::new(),
        "Values are never included. Environment variables and headers are listed by name only.".into(), String::new()];
    for m in &index.mcp {
        out.push(format!("## {}", m.name));
        out.push(String::new());
        out.push(format!("- Transport: {}", m.transport));
        out.push(format!("- {}: `{}`", if m.transport == "http" { "URL" } else { "Command" }, m.target));
        if !m.args.is_empty() {
            out.push(format!("- Arguments: `{}`", m.args.join(" ")));
        }
        if !m.env_names.is_empty() {
            out.push(format!("- Environment: {}", m.env_names.iter().map(|n| format!("`${{{n}}}`")).collect::<Vec<_>>().join(", ")));
        }
        if !m.header_names.is_empty() {
            out.push(format!("- Headers: {}", m.header_names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ")));
        }
        if !m.purpose.is_empty() {
            out.push(String::new());
            out.push(plain(&m.purpose));
        }
        out.push(String::new());
    }
    if !index.mcp_tools.is_empty() {
        out.push("## The project's own server".into());
        out.push(String::new());
        for t in &index.mcp_tools {
            out.push(format!("- `{}` ({}): {}", t.name, t.sort, t.description.lines().next().unwrap_or_default()));
        }
        out.push(String::new());
    }
    out.join("\n")
}

/// `n entries`, `1 entry`.
fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The way in for an agent: the project's sections, with the MCP servers and the endpoints listed whole, as they are
/// what agents ask for most and short enough to include.
pub fn llms_txt(index: &Index) -> String {
    let p = &index.project;
    let s = &index.stats;
    let mut out = vec![format!("# {}", p.title), String::new(), format!("> Dev docs built from the repository by Hersir {}.", p.yodocs_version),
        format!("> {} across {}, {} with a description.", plural(s.entries, "entry", "entries"), plural(s.files, "file", "files"), s.described),
        format!("> Built {}{}.", p.built_at, if p.commit.is_empty() { String::new() } else { format!(" from {} on {}", p.commit, p.branch) }),
        String::new(), "Every page below is also Markdown under `md/`. The full index is `index.json`.".into(), String::new()];
    // said up front, so an agent doesn't go looking for a section that isn't there and conclude the project has none
    if !index.switched_off.is_empty() {
        let named: Vec<&str> = index.switched_off.iter().map(|o| o.section.as_str()).collect();
        out.insert(5, format!("> Switched off in this project, so not in these docs: {} ({}).", named.join(", "), crate::index::SWITCH));
    }
    if !index.mcp.is_empty() {
        out.extend(["## MCP servers".into(), String::new(), "Used by agents working on this project. Values are never recorded.".into(), String::new()]);
        for m in &index.mcp {
            let args = if m.args.is_empty() { String::new() } else { format!(" {}", m.args.join(" ")) };
            let env = if m.env_names.is_empty() { String::new() } else { format!(", env {}", m.env_names.join(", ")) };
            out.push(format!("- **{}** ({}): `{}{args}`{env}", m.name, m.transport, m.target));
        }
        out.extend([String::new(), "Full detail: `md/mcp.md`".into(), String::new()]);
    }
    if let Some(api) = index.api.as_ref().filter(|a| !a.endpoints.is_empty()) {
        out.extend(["## API endpoints".into(), String::new(), format!("From `{}`, {} {}.", api.file, api.title, api.version).replace(" .", "."),
            String::new()]);
        out.extend(api.endpoints.iter().map(|e| {
            format!("- `{} {}`{}", e.method, e.path, if e.summary.is_empty() { String::new() } else { format!(" — {}", e.summary) })
        }));
        out.push(String::new());
    }
    if !index.groups.is_empty() {
        out.extend(["## Code reference".into(), String::new()]);
        for g in &index.groups {
            out.push(format!("- **{}** ({}): {}", g.name, plural(g.count, "entry", "entries"),
                g.rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>().join(", ")));
        }
        out.push(String::new());
        out.push(format!("Kinds present: {}", index.kinds.iter().map(|k| format!("{} ({})", k.name, k.count)).collect::<Vec<_>>().join(", ")));
        out.extend([String::new(), "One Markdown file per entry under `md/reference/`.".into(), String::new()]);
    }
    if !index.guides.is_empty() {
        out.extend(["## Guides".into(), String::new()]);
        out.extend(index.guides.iter().map(|g| format!("- [{}](md/{}.md) — source `{}`", g.title, g.id, g.file)));
        out.push(String::new());
    }
    if !index.decisions.is_empty() {
        out.extend(["## Decisions".into(), String::new()]);
        out.extend(index.decisions.iter().map(|d| format!("- [{}](md/{}.md) — {}", format!("{} {}", d.number, d.title).trim(), d.id, d.status)));
        out.push(String::new());
    }
    if let Some(latest) = index.changelog.first() {
        let date = if latest.date.is_empty() { String::new() } else { format!(" ({})", latest.date) };
        out.extend(["## Changelog".into(), String::new(), format!("Latest: {}{date}. See `md/changelog.md`.", latest.version), String::new()]);
    }
    if !index.history.is_empty() {
        out.extend(["## Recent commits".into(), String::new(), "Newest first, with the files each one touched.".into(), String::new()]);
        for c in index.history.iter().take(20) {
            let files: Vec<String> = c.files.iter().take(8).map(|f| format!("`{}`", f.path)).collect();
            let more = if c.files.len() > 8 { format!(", and {} more", c.files.len() - 8) } else { String::new() };
            let touched = if files.is_empty() { String::new() } else { format!(": {}{more}", files.join(", ")) };
            out.push(format!("- `{}` {} {}{touched}", c.hash, c.date.get(..10).unwrap_or(&c.date), c.subject));
        }
        out.push(String::new());
    }
    if !index.problems.is_empty() {
        out.extend(["## Known gaps".into(), String::new()]);
        for problem in index.problems.iter().take(20) {
            let line = problem.line.map(|l| format!(":{l}")).unwrap_or_default();
            out.push(format!("- `{}`{line} — {}", problem.file, problem.message));
        }
        out.push(String::new());
    }
    out.join("\n")
}

/// Rendered HTML back to Markdown-like text, for the agents' copy (yodocs' plain).
pub fn plain(html: &str) -> String {
    use std::sync::LazyLock;
    use regex::Regex;
    static BLOCK_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)</(p|div|h[1-6]|li|tr)>").unwrap());
    static LI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<li[^>]*>").unwrap());
    static H: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<h([1-6])[^>]*>").unwrap());
    static CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)</?code[^>]*>").unwrap());
    static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
    static GAPS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());
    let text = BLOCK_END.replace_all(html, "\n");
    let text = LI.replace_all(&text, "- ");
    let text = H.replace_all(&text, |c: &regex::Captures| format!("\n{} ", "#".repeat(c[1].parse().unwrap_or(1))));
    let text = CODE.replace_all(&text, "`");
    let text = TAG.replace_all(&text, "");
    let text = text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&");
    GAPS.replace_all(&text, "\n\n").trim().to_string()
}
