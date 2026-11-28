//! The docs a project writes itself, found where yodocs looked for them (its src/content/locate.ts): the changelog in
//! Keep a Changelog format (`CHANGELOG.md`), the decision records (`docs/decisions/NNNN-title.md`, ADRs) and the guides.
//! Only files git tracks are read, never through a link.
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::index::{Api, Change, Decision, McpServer, Problem, Release, Section};
use crate::{guides, markdown};

/// Where a project keeps its changelog and its decision records, best first (yodocs' CHANGELOGS and DECISION_FOLDERS).
const CHANGELOGS: [&str; 4] = ["CHANGELOG.md", "CHANGELOG.markdown", "changelog.md", "docs/CHANGELOG.md"];
const DECISION_FOLDERS: [&str; 5] = ["docs/decisions", "docs/adr", "docs/decision-records", "adr", "decisions"];
/// A file bigger than this is read only this far.
pub const MAX_BYTES: usize = 512 * 1024;

/// What a project writes itself: its guides, its changelog and its decision records, and what didn't fit their format.
#[derive(Debug, Clone, Default)]
pub struct Content {
    pub guides: Vec<guides::Guide>,
    pub changelog: Vec<Release>,
    /// The changelog's file, when there is one.
    pub changelog_file: Option<String>,
    pub decisions: Vec<Decision>,
    /// The MCP servers its agents connect to (`.mcp.json`).
    pub mcp: Vec<McpServer>,
    /// Its OpenAPI document, when it has one.
    pub api: Option<Api>,
    pub problems: Vec<Problem>,
}

pub fn read(repo: &Path, files: &[String]) -> Content {
    let folder = decisions_folder(files);
    // the decision records aren't guides too
    let in_folder = |f: &String| folder.is_some_and(|d| f.strip_prefix(d).is_some_and(|rest| rest.starts_with('/')));
    let prose: Vec<String> = files.iter().filter(|f| !in_folder(f)).cloned().collect();
    let mut content = Content { guides: guides::guides(repo, &prose), ..Content::default() };
    if let Some(file) = CHANGELOGS.iter().find(|c| files.iter().any(|f| f == *c))
        && let Some(text) = crate::read_text(repo, file, MAX_BYTES)
    {
        let (releases, problems) = changelog(file, &text);
        content.changelog = releases;
        content.problems = problems;
        content.changelog_file = Some(file.to_string());
    }
    if let Some(folder) = folder {
        content.decisions = decisions(repo, files, folder);
    }
    let (servers, problems) = crate::mcp::servers(repo, files);
    content.mcp = servers;
    content.problems.extend(problems);
    let (api, problems) = crate::openapi::read(repo, files);
    content.api = api;
    content.problems.extend(problems);
    content
}

/// The six kinds of change Keep a Changelog names.
pub const CHANGE_TYPES: [&str; 6] = ["Added", "Changed", "Deprecated", "Removed", "Fixed", "Security"];

static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^##\s+(.+?)\s*$").unwrap());
static SUBHEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^###\s+(.+?)\s*$").unwrap());
static ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*[-*]\s+(.+)$").unwrap());
static LINKED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap());
static BARE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]+)\]").unwrap());
static DATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d{4}-\d{2}-\d{2})").unwrap());
static DASH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+[-–]\s+").unwrap());

/// A changelog in Keep a Changelog format (yodocs' readChangelog): `## [1.2.0](compare) - 2026-11-05` or
/// `## [Unreleased]`, `### Added`, `- a change`. A section the format doesn't name is grouped under Other and named as a
/// problem; a change before any version is a problem and left out. Unreleased first, then newest.
pub fn changelog(file: &str, text: &str) -> (Vec<Release>, Vec<Problem>) {
    let mut releases: Vec<Release> = vec![];
    let mut problems = vec![];
    let mut kind = "Other".to_string();
    for (i, line) in text.lines().enumerate() {
        if let Some(c) = HEADING.captures(line) {
            let heading = &c[1];
            let linked = LINKED.captures(heading);
            let version = linked.as_ref().map(|l| l[1].to_string())
                .or_else(|| BARE.captures(heading).map(|b| b[1].to_string()))
                .unwrap_or_else(|| DASH.split(heading).next().unwrap_or(heading).to_string()).trim().to_string();
            releases.push(Release {
                unreleased: version.to_lowercase().contains("unreleased"),
                date: DATE.captures(heading).map(|d| d[1].to_string()).unwrap_or_default(),
                compare_url: linked.map(|l| l[2].to_string()).unwrap_or_default(),
                version,
                changes: vec![],
            });
            kind = "Other".into();
        } else if let Some(c) = SUBHEADING.captures(line) {
            let name = c[1].trim();
            match CHANGE_TYPES.iter().find(|t| t.eq_ignore_ascii_case(name)) {
                Some(t) => kind = t.to_string(),
                None => {
                    kind = "Other".into();
                    if !releases.is_empty() {
                        problems.push(Problem { file: file.into(), line: Some(i + 1),
                            message: format!("\"{name}\" is not a Keep a Changelog section, so its entries are grouped under Other") });
                    }
                }
            }
        } else if let Some(c) = ITEM.captures(line) {
            match releases.last_mut() {
                Some(release) => release.changes.push(Change { kind: kind.clone(), text: c[1].to_string(), html: inline(&c[1]) }),
                None => problems.push(Problem { file: file.into(), line: Some(i + 1), message: "an entry before any version heading, skipped".into() }),
            }
        }
    }
    releases.sort_by(|a, b| b.unreleased.cmp(&a.unreleased).then_with(|| b.date.cmp(&a.date)));
    (releases, problems)
}

/// One line of Markdown as HTML, without the paragraph around it.
pub fn inline(text: &str) -> String {
    let html = markdown::render(text).html;
    let trimmed = html.trim_end();
    match trimmed.strip_prefix("<p>").and_then(|t| t.strip_suffix("</p>")) {
        Some(inner) if !inner.contains("<p>") => inner.to_string(),
        _ => html,
    }
}

/// An ADR file name: `0001-title.md`.
static ADR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(\d+)[-_](.+)\.md$").unwrap());
/// Each status and the words that name it, in English and Dutch.
const STATUSES: [(&str, &[&str]); 5] = [("proposed", &["proposed", "voorgesteld", "voorstel"]),
    ("accepted", &["accepted", "aanvaard", "geaccepteerd", "goedgekeurd"]), ("rejected", &["rejected", "verworpen", "afgewezen"]),
    ("superseded", &["superseded", "vervangen"]), ("deprecated", &["deprecated", "vervallen", "verouderd"])];
/// A date in a status line: `aanvaard — gebouwd op …; kaart #395, 2026-11-23.`
static ISO_DATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\d{4}-\d{2}-\d{2}\b").unwrap());

/// The status a status line names first ("superseded by 0119 (accepted)" is superseded), or unknown.
fn status_of(line: &str) -> String {
    let line = line.to_lowercase();
    STATUSES.iter().flat_map(|(status, words)| words.iter().filter_map(|w| line.find(w).map(|at| (at, *status))))
        .min().map_or("unknown", |(_, status)| status).to_string()
}
/// A repository path in a Related section: a folder, a name and an extension.
static REPO_PATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b((?:[\w.-]+/)+[\w.-]+\.\w{1,6})\b").unwrap());
static TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^#\s+(.+)$").unwrap());
static NUMBERED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+\s*[·.:\-–—]\s*").unwrap());

/// The folder with the decision records: the first of yodocs' with an ADR file in it, else the first there is.
pub fn decisions_folder(files: &[String]) -> Option<&'static str> {
    let in_it = |folder: &str, adr_only: bool| files.iter().any(|f| f.strip_prefix(folder).and_then(|r| r.strip_prefix('/'))
        .is_some_and(|name| !name.contains('/') && (!adr_only || ADR.is_match(name))));
    DECISION_FOLDERS.iter().find(|d| in_it(d, true)).or_else(|| DECISION_FOLDERS.iter().find(|d| in_it(d, false))).copied()
}

/// The ADRs right in `folder` (yodocs' readDecisions): their number as written, title, date and status (from front
/// matter or a `Status:` line, however bold), their `##` sections, and the repository paths their Related section
/// names. The highest number first.
pub fn decisions(repo: &Path, files: &[String], folder: &str) -> Vec<Decision> {
    let mut out: Vec<Decision> = files.iter().filter_map(|path| {
        let name = path.strip_prefix(folder)?.strip_prefix('/')?;
        let named = ADR.captures(name).filter(|_| !name.contains('/'))?;
        let source = crate::read_text(repo, path, MAX_BYTES)?;
        let (data, body) = front_matter(&source);
        let rendered = markdown::render(body);
        let title = data.get("title").cloned()
            .or_else(|| TITLE.captures(body).map(|t| NUMBERED.replace(t[1].trim(), "").to_string()))
            .unwrap_or_else(|| named[2].replace(['-', '_'], " "));
        let parts: Vec<&str> = split_h2(body);
        let sections = parts.iter().map(|part| {
            let (head, rest) = part.split_once('\n').unwrap_or((part, ""));
            Section { heading: head.trim().to_string(), html: markdown::render(rest).html }
        }).collect();
        let related = parts.iter().find(|p| p.trim_start().to_lowercase().starts_with("related"))
            .map(|p| REPO_PATH.captures_iter(p).map(|c| c[1].to_string()).collect::<Vec<_>>()).unwrap_or_default();
        let mut related_files: Vec<String> = vec![];
        for r in related {
            if !related_files.contains(&r) {
                related_files.push(r);
            }
        }
        let status = data.get("status").cloned().unwrap_or_else(|| field(body, "status"));
        let date = [data.get("date").cloned().unwrap_or_default(), field(body, "date"), field(body, "datum"),
            ISO_DATE.find(&status).map(|m| m.as_str().to_string()).unwrap_or_default()].into_iter().find(|d| !d.is_empty()).unwrap_or_default();
        Some(Decision {
            id: format!("decisions/{}", &named[1]),
            number: named[1].to_string(),
            title,
            date,
            status: status_of(&status),
            sections,
            related_files,
            file: path.clone(),
            html: rendered.html,
            headings: rendered.headings,
        })
    }).collect();
    out.sort_by(|a, b| number(&b.number).cmp(&number(&a.number)).then_with(|| b.number.cmp(&a.number)));
    out
}

fn number(n: &str) -> u64 {
    n.parse().unwrap_or(0)
}

/// The parts of `body` after each `## ` heading, each starting with its heading's text.
fn split_h2(body: &str) -> Vec<&str> {
    let mut starts: Vec<usize> = vec![];
    let mut at = 0;
    for line in body.split_inclusive('\n') {
        if line.starts_with("## ") || line.starts_with("##\t") {
            starts.push(at + 3);
        }
        at += line.len();
    }
    let ends: Vec<usize> = starts.iter().skip(1).map(|s| s - 3).chain(std::iter::once(body.len())).collect();
    starts.iter().zip(ends).map(|(s, e)| &body[*s..e]).collect()
}

/// `Status: accepted`, `**Status:** accepted` or `**Status**: accepted`, also as a list item (MADR's `* Status: …`):
/// the value, without the bold.
fn field(body: &str, name: &str) -> String {
    let re = Regex::new(&format!(r"(?im)^\s*(?:[-*]\s+)?\*{{0,2}}{name}\*{{0,2}}\s*:\s*(.+)$")).unwrap();
    re.captures(body).map(|c| c[1].trim().trim_matches('*').trim().to_string()).unwrap_or_default()
}

/// A `---` front matter block's `key: value` lines, and the text after it (yodocs' frontMatter: scalars only).
pub fn front_matter(source: &str) -> (BTreeMap<String, String>, &str) {
    static BLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^---\r?\n([\s\S]*?)\r?\n---\r?\n?").unwrap());
    static PAIR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*([\w-]+)\s*:\s*(.*)$").unwrap());
    let Some(block) = BLOCK.captures(source) else { return (BTreeMap::new(), source) };
    let data = block[1].lines().filter_map(|line| PAIR.captures(line))
        .map(|p| (p[1].to_string(), p[2].trim().trim_matches(['"', '\'']).to_string())).collect();
    (data, &source[block[0].len()..])
}

/// One decision in a decision log: the note the Chief keeps per project in Memory (`Decisions/<project>`), a list item
/// per decision, its date first (`- 2026-11-03: Title: what was decided (CARD-1)`), some under date headings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub date: String,
    /// The words before the first colon when they are short, else the first sentence, cut at a word.
    pub title: String,
    /// The whole line without its date (Markdown), its indented lines after it, and a card in brackets after the date at
    /// its end.
    pub body: String,
}

/// `- 2026-11-03: …`, `- 2026-11-20 (PEAR-3): …`: a list item with its date first.
static DATED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{4}-\d{2}-\d{2})\s*(?:\(([^)]*)\))?\s*:\s*(.*)$").unwrap());
/// `## 18 november`, `### 25 November 2026`: a heading that is a date, in English or Dutch.
static DATE_HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^#+\s+(\d{1,2})\s+([a-z]+)(?:\s+(\d{4}))?\s*$").unwrap());
/// A month's number by its name's first three letters, in English or Dutch.
fn month(name: &str) -> Option<&'static str> {
    Some(match name.to_lowercase().get(..3)? {
        "jan" => "01", "feb" => "02", "mar" | "maa" => "03", "apr" => "04", "may" | "mei" => "05", "jun" => "06",
        "jul" => "07", "aug" => "08", "sep" => "09", "oct" | "okt" => "10", "nov" => "11", "dec" => "12",
        _ => return None,
    })
}
const TITLE_MAX: usize = 90;

/// The decisions in a log note, in the note's order. A line takes its own date, else its heading's (in the year of
/// `today`, the note's own date), else `today`. None when no list item starts with a date: then the note is one decision.
pub fn log_entries(md: &str, today: &str) -> Vec<LogEntry> {
    let year = today.get(..4).unwrap_or("1970");
    let mut entries: Vec<(LogEntry, bool)> = vec![];
    let mut heading_date: Option<String> = None;
    let mut open = false;
    for line in md.lines() {
        if line.starts_with('#') {
            open = false;
            heading_date = DATE_HEADING.captures(line).and_then(|c| {
                Some(format!("{}-{}-{:0>2}", c.get(3).map_or(year, |y| y.as_str()), month(&c[2])?, &c[1]))
            });
        } else if let Some(item) = ITEM.captures(line).filter(|_| !line.starts_with("  ")) {
            open = true;
            let text = item[1].trim();
            let entry = match DATED.captures(text) {
                Some(c) => {
                    let card = c.get(2).map(|m| format!(" ({})", m.as_str())).unwrap_or_default();
                    (LogEntry { date: c[1].to_string(), title: String::new(), body: format!("{}{card}", &c[3]) }, true)
                }
                None => (LogEntry { date: heading_date.clone().unwrap_or_else(|| today.to_string()), title: String::new(), body: text.into() }, false),
            };
            entries.push(entry);
        } else if open && !line.trim().is_empty() && let Some((last, _)) = entries.last_mut() {
            // a line that goes on: before its card, which ends the line
            let card = last.body.rfind(" (").filter(|_| last.body.ends_with(')')).map(|i| last.body.split_off(i)).unwrap_or_default();
            last.body = format!("{}\n{}{card}", last.body, line.trim());
        }
    }
    if !entries.iter().any(|(_, dated)| *dated) {
        return vec![];
    }
    entries.into_iter().map(|(mut e, _)| {
        e.title = log_title(&e.body);
        e
    }).collect()
}

fn log_title(body: &str) -> String {
    let first = body.lines().next().unwrap_or_default();
    if let Some(i) = first.find(": ").filter(|i| *i <= TITLE_MAX && !first[..*i].contains(". ")) {
        return first[..i].trim().to_string();
    }
    let sentence = first.find(". ").map_or(first, |i| &first[..=i]).trim();
    if sentence.chars().count() <= TITLE_MAX {
        return sentence.to_string();
    }
    let cut: String = sentence.chars().take(TITLE_MAX).collect();
    format!("{}…", cut[..cut.rfind(' ').unwrap_or(cut.len())].trim_end())
}
