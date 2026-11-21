//! A project's docs index: yodocs' `index.json`, schema 1 (yodocs src/schema.ts), which the docs page (`viewer.rs`)
//! reads and nothing else. Built from what Hersir reads in the project's files (Reference, API routes, guides) and what
//! it keeps itself (the changelog, decisions, commits). Every section is there, empty when the project has nothing for
//! it, so the page can say which file to add.
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::api::Route;
use crate::markdown::{self, Heading};
use crate::{guides, Reference};

/// The index's shape, as yodocs' viewer reads it.
pub const SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub schema: u32,
    pub project: ProjectInfo,
    pub stats: Stats,
    pub kinds: Vec<KindCount>,
    pub groups: Vec<Group>,
    pub entries: Vec<IndexEntry>,
    pub problems: Vec<Problem>,
    pub guides: Vec<IndexGuide>,
    pub changelog: Vec<Release>,
    pub decisions: Vec<Decision>,
    pub mcp: Vec<McpServer>,
    pub mcp_tools: Vec<McpTool>,
    pub api: Option<Api>,
    pub doc_changes: Option<DocChanges>,
    pub history: Vec<Commit>,
    /// The sections the project switched off, and where.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub switched_off: Vec<SwitchedOff>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SwitchedOff {
    pub section: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub title: String,
    pub built_at: String,
    pub commit: String,
    pub branch: String,
    /// Shown as "Hersir <this>" at the foot of the sidebar.
    pub yodocs_version: String,
    pub probes: Vec<String>,
    /// Hersir's: where the changelog comes from, for the Changelog page's first line.
    pub changelog_from: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub files: usize,
    pub entries: usize,
    pub described: usize,
    pub problems: usize,
    pub duration_ms: u64,
    pub skipped: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KindCount {
    pub name: String,
    pub count: usize,
}

/// A sidebar group: a tree (Folders) or a flat list (Routes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Group {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub count: usize,
    pub rows: Vec<GroupRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GroupRow {
    pub label: String,
    /// An entry id, or a folder or file path in the Folders tree. Empty: nothing to open.
    pub target: String,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub badge: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<GroupRow>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Fact {
    pub label: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocTag {
    pub name: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexEntry {
    /// `<file>#<name>`, `~2` and on for a second one of the same name: deep links depend on it.
    pub id: String,
    pub name: String,
    pub kind: String,
    pub syntactic_kind: String,
    pub file: String,
    pub line_start: usize,
    pub line_end: usize,
    pub name_line: usize,
    pub signature: String,
    pub description: String,
    pub has_description: bool,
    pub tags: Vec<DocTag>,
    pub facts: Vec<Fact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub children: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Problem {
    pub file: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IndexGuide {
    pub id: String,
    pub title: String,
    pub group: String,
    pub order: usize,
    pub file: String,
    pub updated: String,
    pub html: String,
    pub headings: Vec<Heading>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Change {
    /// Added, Changed, Deprecated, Removed, Fixed, Security or Other.
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
    pub html: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub version: String,
    /// `YYYY-MM-DD`, empty for Unreleased.
    pub date: String,
    pub unreleased: bool,
    pub compare_url: String,
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section {
    pub heading: String,
    pub html: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Decision {
    pub id: String,
    pub number: String,
    pub title: String,
    pub date: String,
    /// proposed, accepted, rejected, superseded, deprecated or unknown.
    pub status: String,
    pub sections: Vec<Section>,
    pub related_files: Vec<String>,
    pub file: String,
    pub html: String,
    pub headings: Vec<Heading>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub name: String,
    pub transport: String,
    pub target: String,
    pub args: Vec<String>,
    pub env_names: Vec<String>,
    pub header_names: Vec<String>,
    pub purpose: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct McpInput {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub description: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct McpAnnotation {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct McpTool {
    pub id: String,
    pub name: String,
    pub title: String,
    pub description: String,
    pub annotations: Vec<McpAnnotation>,
    pub inputs: Vec<McpInput>,
    /// tool, resource or prompt.
    pub sort: String,
    pub file: String,
    pub entry: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    pub id: String,
    pub method: String,
    pub path: String,
    pub summary: String,
    pub description: String,
    pub tags: Vec<String>,
    pub auth: Vec<String>,
    pub parameters: Vec<Parameter>,
    pub request_body: serde_json::Value,
    pub responses: Vec<ApiResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "in")]
    pub location: String,
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ApiResponse {
    pub status: String,
    pub description: String,
    pub schema: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Api {
    pub title: String,
    pub version: String,
    pub file: String,
    pub endpoints: Vec<Endpoint>,
    pub schemas: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocChange {
    pub id: String,
    pub name: String,
    pub file: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Since {
    pub commit: String,
    pub taken_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocChanges {
    pub since: Since,
    pub added: Vec<DocChange>,
    pub removed: Vec<DocChange>,
    pub documented: Vec<DocChange>,
    pub undocumented: Vec<DocChange>,
    pub changed: Vec<DocChange>,
    pub total: usize,
}

/// One file a commit touched. `status`: git's letter (A, M, D, R…). `from`: a renamed file's old path. `diff`: its
/// changed lines can be shown (`commits/<hash>.json`): a file the Reference reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommitFile {
    pub status: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    pub diff: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Commit {
    pub hash: String,
    /// ISO 8601.
    pub date: String,
    pub author: String,
    pub subject: String,
    pub files: Vec<CommitFile>,
}

/// The changed lines of one file of a commit, cut short after `PATCH_LINES`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FilePatch {
    pub lines: Vec<String>,
    pub truncated: bool,
}

/// `commits/<hash>.json`: a commit's changed lines, by file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CommitPatches {
    pub files: BTreeMap<String, FilePatch>,
}

/// One file's change is cut short past this: one reformat shouldn't fill the page.
pub const PATCH_LINES: usize = 400;

/// One of the project's files, for an entry's Source (`source/<path>.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceFile {
    pub path: String,
    pub lines: Vec<String>,
}

/// What an index is built from.
#[derive(Debug, Clone, Default)]
pub struct Input {
    /// `<project> dev docs`.
    pub title: String,
    pub commit: String,
    pub branch: String,
    /// Hersir's version.
    pub version: String,
    pub built_at: String,
    pub duration_ms: u64,
    pub reference: Reference,
    pub routes: Vec<Route>,
    pub guides: Vec<guides::Guide>,
    pub changelog: Vec<Release>,
    pub changelog_from: String,
    pub decisions: Vec<Decision>,
    pub mcp: Vec<McpServer>,
    /// The project's OpenAPI document; without one, its routes fill the API section.
    pub api: Option<Api>,
    pub history: Vec<Commit>,
    /// What else didn't read right: a changelog section outside the format, say.
    pub problems: Vec<Problem>,
    /// The sections the project switched off (reference, guides, api, changelog, decisions, mcp): left out whole.
    pub off: Vec<String>,
}

pub fn build(input: Input) -> Index {
    let mut entries = entries(&input.reference);
    let mut groups = vec![folder_tree(&input.reference.read, &entries)];
    let mut probes = vec![];
    let mut handlers = None;
    if !input.routes.is_empty() {
        let routes = routes_group(&input.routes, &entries);
        // each route on the entry that answers it, for that page's "Used by routes"
        let at: HashMap<String, usize> = entries.iter().enumerate().map(|(i, e)| (e.id.clone(), i)).collect();
        for (route, row) in input.routes.iter().zip(&routes.rows) {
            if let Some(&i) = at.get(&row.target) {
                entries[i].facts.push(Fact { label: "Route".into(), value: format!("{} {}", route.method, route.path), target: None });
            }
        }
        handlers = routes_api(&input.routes, &routes.rows, &entries);
        groups.push(routes);
        probes.push("routes".to_string());
    }
    let mut problems: Vec<Problem> = input.reference.partial.iter().map(|file| Problem {
        file: file.clone(),
        message: "has syntax the parser can't read, so its declarations may be incomplete".into(),
        line: None,
    }).collect();
    problems.extend(input.problems);
    let mcp_tools = crate::mcp::tools(&input.reference, &entries);
    let mut index = Index {
        schema: SCHEMA,
        project: ProjectInfo {
            title: input.title,
            built_at: input.built_at,
            commit: input.commit,
            branch: input.branch,
            yodocs_version: input.version,
            probes,
            changelog_from: input.changelog_from,
        },
        stats: Stats {
            files: input.reference.files,
            entries: entries.len(),
            described: entries.iter().filter(|e| e.has_description).count(),
            problems: problems.len(),
            duration_ms: input.duration_ms,
            skipped: BTreeMap::new(),
        },
        kinds: kinds(&entries),
        groups,
        guides: input.guides.iter().enumerate().map(|(order, g)| guide(g, order)).collect(),
        entries,
        problems,
        changelog: input.changelog,
        decisions: input.decisions,
        mcp_tools,
        mcp: input.mcp,
        api: input.api.or(handlers),
        doc_changes: None,
        history: input.history,
        switched_off: vec![],
    };
    switch_off(&mut index, &input.off);
    index
}

/// Where a project switches a section off, as the page names it.
pub const SWITCH: &str = "Edit project → Docs";

/// Leaves the switched-off sections out of `index` whole, not just hidden: whatever the page can reach is in the index.
/// With the Reference off, the MCP tools and the endpoints its routes make stay, without their links to the code.
fn switch_off(index: &mut Index, off: &[String]) {
    for section in off {
        match section.as_str() {
            "reference" => {
                index.entries.clear();
                index.groups.clear();
                index.kinds.clear();
                let partial = "has syntax the parser can't read, so its declarations may be incomplete";
                index.problems.retain(|p| p.message != partial);
                index.stats = Stats { files: 0, entries: 0, described: 0, problems: index.problems.len(), ..index.stats.clone() };
                index.mcp_tools.iter_mut().for_each(|t| t.entry.clear());
            }
            "guides" => index.guides.clear(),
            "api" => index.api = None,
            "changelog" => index.changelog.clear(),
            "decisions" => index.decisions.clear(),
            "mcp" => {
                index.mcp.clear();
                index.mcp_tools.clear();
            }
            _ => continue,
        }
        index.switched_off.push(SwitchedOff { section: section.clone(), key: SWITCH.into() });
    }
}

/// What "Changed in the docs" compares for an entry: its name, file and kind, whether something is written above it,
/// and its signature (as an FNV-1a hash, so a project's baseline stays small).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    pub name: String,
    pub file: String,
    pub kind: String,
    pub described: bool,
    pub signature: u64,
}

/// The entries' fingerprints by id: what a baseline keeps.
pub fn fingerprints(entries: &[IndexEntry]) -> BTreeMap<String, Fingerprint> {
    let hash = |s: &str| s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3));
    entries.iter().map(|e| (e.id.clone(), Fingerprint { name: e.name.clone(), file: e.file.clone(), kind: e.kind.clone(),
        described: e.has_description, signature: hash(&e.signature) })).collect()
}

/// What moved between `before` (the baseline) and `after` (yodocs' compareDocs): an entry that got something written
/// above it is documented, one that lost it undocumented, one whose signature changed changed, and the rest added or
/// removed.
pub fn compare(before: &BTreeMap<String, Fingerprint>, after: &BTreeMap<String, Fingerprint>, since: Since) -> DocChanges {
    let row = |id: &String, f: &Fingerprint| DocChange { id: id.clone(), name: f.name.clone(), file: f.file.clone(), kind: f.kind.clone() };
    let mut d = DocChanges { since, added: vec![], removed: vec![], documented: vec![], undocumented: vec![], changed: vec![], total: 0 };
    for (id, now) in after {
        match before.get(id) {
            None => d.added.push(row(id, now)),
            Some(was) if !was.described && now.described => d.documented.push(row(id, now)),
            Some(was) if was.described && !now.described => d.undocumented.push(row(id, now)),
            Some(was) if was.signature != now.signature => d.changed.push(row(id, now)),
            Some(_) => {}
        }
    }
    d.removed = before.iter().filter(|(id, _)| !after.contains_key(*id)).map(|(id, f)| row(id, f)).collect();
    d.total = d.added.len() + d.removed.len() + d.documented.len() + d.undocumented.len() + d.changed.len();
    d
}

/// The Reference's declarations as index entries, each with an id, its signature and what it extends or implements. A
/// class no rule named takes the kind of the project class it extends (`inherit_kinds`).
pub fn entries(reference: &Reference) -> Vec<IndexEntry> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<IndexEntry> = reference.entries.iter().map(|e| {
        let base = format!("{}#{}", e.path, e.name);
        let n = seen.entry(base.clone()).or_insert(0);
        *n += 1;
        let signature = signature(&e.text);
        IndexEntry {
            id: if *n == 1 { base } else { format!("{base}~{n}") },
            name: e.name.clone(),
            kind: e.kind.clone(),
            syntactic_kind: e.syntactic.clone(),
            file: e.path.clone(),
            line_start: e.line,
            line_end: e.end_line,
            name_line: e.line,
            facts: facts(&signature),
            signature,
            description: e.doc.clone(),
            has_description: !e.doc.trim().is_empty(),
            tags: vec![],
            parent: None,
            children: vec![],
        }
    }).collect();
    inherit_kinds(&mut out);
    out
}

/// The kinds a grammar gives a type, whose kind another class can take.
const TYPE_KINDS: [&str; 6] = ["class", "interface", "trait", "enum", "object", "mixin"];

/// A class no rule named takes the kind of the project class it extends, when a rule named that one (yodocs'
/// inheritKinds): a tool extending a project's abstract FactoryTool, which extends Laravel's Tool, is an MCP tool too.
/// By name within the project, to a fixed point, at most six deep; a kind a rule decided stays.
fn inherit_kinds(entries: &mut [IndexEntry]) {
    let base_of = |e: &IndexEntry| e.facts.iter().find(|f| f.label == "Extends")
        .map(|f| f.value.split(',').next().unwrap_or_default().trim().rsplit('\\').next().unwrap_or_default().to_string());
    for _ in 0..6 {
        let named: HashMap<String, String> = entries.iter()
            .filter(|e| e.parent.is_none() && TYPE_KINDS.contains(&e.syntactic_kind.as_str()) && e.kind != e.syntactic_kind)
            .map(|e| (e.name.clone(), e.kind.clone())).collect();
        let mut changed = false;
        for e in entries.iter_mut().filter(|e| e.kind == e.syntactic_kind) {
            if let Some(kind) = base_of(e).filter(|b| *b != e.name).and_then(|b| named.get(&b)) {
                e.kind = kind.clone();
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

/// Words that start a declaration in the languages the Reference reads (yodocs' DECLARES).
static DECLARES: LazyLock<Regex> = LazyLock::new(|| Regex::new(
    r"^(?:export\s+|default\s+|public\s+|protected\s+|private\s+|static\s+|final\s+|abstract\s+|readonly\s+|async\s+|override\s+|open\s+|data\s+|suspend\s+)*(?:class|interface|trait|enum|function|fun|const|let|var|type|object|val|def|mixin|extension|record|struct)\b",
).unwrap());
static EXTENDS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bextends\s+((?:[\\\w]+\s*,\s*)*[\\\w]+)").unwrap());
static IMPLEMENTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bimplements\s+((?:[\\\w]+\s*,\s*)*[\\\w]+)").unwrap());

/// Everything before the body, from the line that declares something: an attribute over many lines above a class
/// isn't its signature (yodocs' signatureOf).
pub fn signature(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').map(str::trim).collect();
    let from = lines.iter().position(|l| DECLARES.is_match(l)).unwrap_or(0);
    let mut head = vec![];
    for line in &lines[from..] {
        match line.find(['{', ';']) {
            Some(stop) => {
                head.push(&line[..stop]);
                break;
            }
            None => head.push(line),
        }
    }
    head.join(" ").split_whitespace().collect::<Vec<_>>().join(" ")
}

fn facts(signature: &str) -> Vec<Fact> {
    let mut facts = vec![];
    if let Some(c) = EXTENDS.captures(signature) {
        facts.push(Fact { label: "Extends".into(), value: c[1].trim().into(), target: None });
    }
    if let Some(c) = IMPLEMENTS.captures(signature) {
        let names: Vec<&str> = c[1].split(',').map(str::trim).filter(|n| !n.is_empty()).collect();
        facts.push(Fact { label: "Implements".into(), value: names.join(", "), target: None });
    }
    facts
}

/// The kinds with how many entries each has, most first.
fn kinds(entries: &[IndexEntry]) -> Vec<KindCount> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for e in entries {
        *counts.entry(e.kind.as_str()).or_default() += 1;
    }
    let mut out: Vec<KindCount> = counts.into_iter().map(|(name, count)| KindCount { name: name.into(), count }).collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    out
}

/// The Folders group: every file the Reference read, folders first, each counting the entries under it.
fn folder_tree(read: &[String], entries: &[IndexEntry]) -> Group {
    #[derive(Default)]
    struct Node {
        path: String,
        count: usize,
        children: BTreeMap<String, Node>,
    }
    let mut per_file: HashMap<&str, usize> = HashMap::new();
    for e in entries {
        *per_file.entry(e.file.as_str()).or_default() += 1;
    }
    let mut root = Node::default();
    for file in read {
        let n = per_file.get(file.as_str()).copied().unwrap_or(0);
        let parts: Vec<&str> = file.split('/').collect();
        let mut node = &mut root;
        for i in 0..parts.len() {
            let child = node.children.entry(parts[i].to_string()).or_insert_with(|| Node { path: parts[..=i].join("/"), ..Node::default() });
            child.count += n;
            node = child;
        }
    }
    fn rows(node: &Node) -> Vec<GroupRow> {
        let mut kids: Vec<(&String, &Node)> = node.children.iter().collect();
        kids.sort_by(|(an, a), (bn, b)| a.children.is_empty().cmp(&b.children.is_empty())
            .then_with(|| an.to_lowercase().cmp(&bn.to_lowercase())).then_with(|| an.cmp(bn)));
        kids.into_iter().map(|(name, child)| GroupRow {
            label: name.clone(),
            target: child.path.clone(),
            count: child.count,
            badge: None,
            children: (!child.children.is_empty()).then(|| rows(child)),
        }).collect()
    }
    Group { name: "Folders".into(), kind: "tree".into(), count: entries.len(), rows: rows(&root) }
}

/// The Routes group: each route with its method, opening the class that answers it (Laravel's `Controller@method`),
/// else the first declaration in its file (Next.js), else nothing.
fn routes_group(routes: &[Route], entries: &[IndexEntry]) -> Group {
    let mut by_name: HashMap<&str, &str> = HashMap::new();
    let mut by_file: HashMap<&str, &str> = HashMap::new();
    for e in entries {
        by_name.entry(e.name.as_str()).or_insert(e.id.as_str());
        by_file.entry(e.file.as_str()).or_insert(e.id.as_str());
    }
    let rows: Vec<GroupRow> = routes.iter().map(|r| {
        let class = r.handler.split('@').next().unwrap_or_default().rsplit('\\').next().unwrap_or_default();
        let target = by_name.get(class).filter(|_| !class.is_empty()).or_else(|| by_file.get(r.file.as_str()));
        GroupRow { label: r.path.clone(), target: target.map(|t| t.to_string()).unwrap_or_default(), count: 0, badge: Some(r.method.clone()), children: None }
    }).collect();
    Group { name: "Routes".into(), kind: "flat".into(), count: rows.len(), rows }
}

/// Without an OpenAPI document, the routes as the API section (yodocs' route handlers): each one that isn't a page, with
/// its path's parameters, summed up by the first sentence of what answers it, tagged by the first part of its path.
fn routes_api(routes: &[Route], rows: &[GroupRow], entries: &[IndexEntry]) -> Option<Api> {
    let by_id: HashMap<&str, &IndexEntry> = entries.iter().map(|e| (e.id.as_str(), e)).collect();
    let mut endpoints: Vec<Endpoint> = routes.iter().zip(rows).filter(|(r, _)| r.method != "PAGE").map(|(r, row)| {
        let described = by_id.get(row.target.as_str()).map_or("", |e| e.description.as_str());
        let first = described.split_inclusive(". ").next().unwrap_or_default().lines().next().unwrap_or_default().trim();
        let answered = if r.handler.is_empty() { format!("In {}:{}.", r.file, r.line) } else { format!("{} answers it, in {}:{}.", r.handler, r.file, r.line) };
        Endpoint {
            id: format!("{}{}", r.method.to_lowercase(), r.path),
            method: r.method.clone(),
            path: r.path.clone(),
            summary: if first.is_empty() { format!("{} {}", r.method, r.path) } else { first.to_string() },
            description: [described.trim(), &answered].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join("\n\n"),
            tags: vec![r.path.split('/').find(|s| !s.is_empty() && *s != "api").unwrap_or("Endpoints").to_string()],
            auth: vec![],
            parameters: path_params(&r.path),
            request_body: serde_json::Value::Null,
            responses: vec![],
        }
    }).collect();
    if endpoints.is_empty() {
        return None;
    }
    endpoints.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.method.cmp(&b.method)));
    let mut files: Vec<&str> = routes.iter().filter(|r| r.method != "PAGE").map(|r| r.file.as_str()).collect();
    files.sort_unstable();
    files.dedup();
    Some(Api { title: "Route handlers".into(), version: String::new(), file: files.join(" and "), endpoints, schemas: BTreeMap::new() })
}

/// A path's parameters: Laravel's `{invoice}` (`{page?}` optional), Next.js's `[id]` and `[...all]` (`[[...all]]`
/// optional).
fn path_params(path: &str) -> Vec<Parameter> {
    path.split('/').filter_map(|s| {
        let (name, required) = if let Some(inner) = s.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
            inner.strip_suffix('?').map_or((inner, true), |n| (n, false))
        } else if let Some(inner) = s.strip_prefix("[[").and_then(|s| s.strip_suffix("]]")) {
            (inner, false)
        } else if let Some(inner) = s.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            (inner, true)
        } else {
            return None;
        };
        Some(Parameter { name: name.trim_start_matches("...").to_string(), location: "path".into(), required, description: String::new() })
    }).collect()
}

/// A guide with its Markdown as HTML. Its group is its folder under docs/ (or doc/): none at the root, right in docs/, or
/// in docs/guides/, the guides folder yodocs asked for.
fn guide(g: &guides::Guide, order: usize) -> IndexGuide {
    let rendered = markdown::render(&g.body_md);
    let (folder, name) = g.path.rsplit_once('/').unwrap_or(("", g.path.as_str()));
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    let group = folder.strip_prefix("docs").or_else(|| folder.strip_prefix("doc")).map_or(folder, |rest| rest.trim_start_matches('/'));
    let group = group.strip_prefix("guides").filter(|rest| rest.is_empty() || rest.starts_with('/')).map_or(group, |rest| rest.trim_start_matches('/'));
    IndexGuide {
        id: if folder.is_empty() { stem.to_string() } else { format!("{folder}/{stem}") },
        title: g.title.clone(),
        group: group.to_string(),
        order,
        file: g.path.clone(),
        updated: String::new(),
        html: rendered.html,
        headings: rendered.headings,
    }
}

/// One of the project's files for an entry's Source: only a file git tracks that the Reference reads, not a link, at
/// most `MAX_BYTES`.
pub fn source(repo: &Path, files: &[String], path: &str) -> Option<SourceFile> {
    if !files.iter().any(|f| f == path) || crate::lang_of(path).is_none() || crate::skipped(path) {
        return None;
    }
    let full = repo.join(path);
    let meta = std::fs::symlink_metadata(&full).ok()?;
    if !meta.is_file() || meta.len() > crate::MAX_BYTES {
        return None;
    }
    let text = std::fs::read_to_string(&full).ok()?;
    Some(SourceFile { path: path.to_string(), lines: text.split('\n').map(str::to_string).collect() })
}

/// A file whose text the page may show: one the Reference reads (its languages, not in installed or built folders).
pub fn readable(path: &str) -> bool {
    crate::lang_of(path).is_some() && !crate::skipped(path)
}

/// Main's commits with the files each changed, from `gizai_agents::worktree::log_name_status` (yodocs' parseHistory).
/// A merge lists the files of its pull request.
pub fn history(log: &str) -> Vec<Commit> {
    log.split('\x1e').filter(|record| !record.trim().is_empty()).map(|record| {
        let mut lines = record.split('\n');
        let mut head = lines.next().unwrap_or_default().split('\x1f');
        let mut field = || head.next().unwrap_or_default().to_string();
        let (hash, date, author, subject) = (field(), field(), field(), field());
        let files = lines.filter(|l| l.contains('\t')).map(|line| {
            let columns: Vec<&str> = line.split('\t').collect();
            let path = columns[columns.len() - 1].to_string();
            CommitFile {
                status: columns[0].chars().next().map(String::from).unwrap_or_default(),
                diff: readable(&path),
                from: (columns.len() == 3).then(|| columns[1].to_string()),
                path,
            }
        }).collect();
        Commit { hash, date, author, subject, files }
    }).collect()
}

/// One commit's `git show -p` (`gizai_agents::worktree::show_patch`) as the changed lines of each file the Reference
/// reads (yodocs' parsePatches): a commit that changed `.env` still lists it, but never shows its lines.
pub fn patches(show: &str) -> CommitPatches {
    let mut out = CommitPatches::default();
    for part in split_diffs(show) {
        let lines: Vec<&str> = part.split('\n').collect();
        // git ends a path with a space in it with a tab
        let named = |prefix: &str| lines.iter().find_map(|l| l.strip_prefix(prefix)).map(|p| p.trim_end_matches('\t').to_string());
        // a removed file has no new path, so it goes under its old one
        let Some(path) = named("+++ b/").or_else(|| named("--- a/")) else { continue };
        let Some(start) = lines.iter().position(|l| l.starts_with("@@")) else { continue };
        if !readable(&path) {
            continue;
        }
        let body: Vec<String> = lines[start..].iter().filter(|l| l.starts_with(['@', '+', '-', ' '])).map(|l| l.to_string()).collect();
        let truncated = body.len() > PATCH_LINES;
        out.files.insert(path, FilePatch { lines: body.into_iter().take(PATCH_LINES).collect(), truncated });
    }
    out
}

/// `git show -p`'s output split at each file's `diff --git` line.
fn split_diffs(show: &str) -> Vec<&str> {
    let mut starts: Vec<usize> = show.match_indices("diff --git ").map(|(i, _)| i).filter(|&i| i == 0 || show.as_bytes()[i - 1] == b'\n').collect();
    starts.push(show.len());
    starts.windows(2).map(|w| &show[w[0]..w[1]]).collect()
}
