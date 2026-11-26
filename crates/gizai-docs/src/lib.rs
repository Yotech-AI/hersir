//! Docs → Reference: the declarations in a project's code, ported from yodocs. Each file is read with tree-sitter and its
//! language's query (`queries/`, yodocs' own), then each declaration gets its kind (a React component or hook, a Laravel
//! model, controller or migration…), the comment right above it and its text. TypeScript, TSX, JavaScript, PHP, Dart,
//! Kotlin, and the script blocks of Vue and Svelte files (read as TSX). Dart's and Kotlin's queries are new: the grammars
//! yodocs read named their nodes differently.
//! The caller lists the files (git's tracked files) and this reads them from disk: it starts no program. Every file is
//! read the one safe way (`open_plain`): a plain file only, never through a link, never waiting on a FIFO, and never
//! more than a limit.
pub mod agents;
pub mod api;
pub mod content;
pub mod guides;
pub mod index;
pub mod markdown;
pub mod mcp;
pub mod openapi;

use std::cell::Cell;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::ops::ControlFlow;
use std::path::{Component, Path};
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Node, ParseOptions, ParseState, Parser, Query, QueryCursor, QueryCursorOptions, QueryCursorState};

/// One declaration, as Docs → Reference lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    /// component, hook, function, class, interface, type, enum, constant, model, controller, migration…
    pub kind: String,
    /// What the grammar calls it (class, function, interface…), before its kind is decided.
    pub syntactic: String,
    /// The file, relative to the repository, with `/`.
    pub path: String,
    /// The first and last line it spans, counted from 1.
    pub line: usize,
    pub end_line: usize,
    /// The comment right above it, without its markers.
    pub doc: String,
    /// Its text, at most `TEXT_LINES` lines.
    pub text: String,
}

/// What Docs → Reference shows: the declarations by file and line, how many files were read and which, the files with
/// syntax their grammar can't read, and the files Hersir stopped reading at `FILE_TIME` or `MATCH_LIMIT`: the
/// declarations of both may be incomplete.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub entries: Vec<Entry>,
    pub files: usize,
    pub read: Vec<String>,
    pub partial: Vec<String>,
    pub cut_off: Vec<String>,
}

/// A file bigger than this is left out: generated or bundled code, not something written by hand.
pub const MAX_BYTES: u64 = 512 * 1024;
/// At most this many files are read.
pub const MAX_FILES: usize = 5000;
/// How long one file's parse and query may take together (GA-362): then Hersir stops and keeps the declarations found so
/// far. A file can be written to make tree-sitter slow (156 KB of nested `[` took 15 s), and the Docs page waits for
/// every file. A file of 512 KB written by hand takes well under a tenth of this.
pub const FILE_TIME: Duration = Duration::from_secs(1);
/// At most this many matches in progress at once in a file's query (tree-sitter's match limit, GA-362). Past it the
/// oldest is dropped, so the file's declarations may be incomplete and it counts as cut off.
pub const MATCH_LIMIT: u32 = 256;
/// An entry's text stops after this many lines.
pub const TEXT_LINES: usize = 200;
/// Members, imports and PHP's namespace and use lines are left out: they are inside the classes listed, or not something
/// a reader looks for.
const LEFT_OUT: [&str; 5] = ["method", "property", "import", "namespace", "use"];
/// Folders of installed, built or generated code, wherever they are in the tree.
const SKIPPED_DIRS: [&str; 10] = ["vendor", "node_modules", "dist", "build", ".next", "out", "coverage", "storage", "public", ".nuxt"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Lang {
    TypeScript,
    Tsx,
    JavaScript,
    Php,
    Dart,
    Kotlin,
    /// A .vue or .svelte file: its script blocks, read as TSX.
    Component,
}

/// A file git tracks in `repo` (`path` relative to it, as git lists it), opened for reading when it is a plain file,
/// with its size; None for anything else (GA-362). Never through a link: a tracked link could point at `/dev/zero`, which
/// never ends, or at a file outside the project (`O_NOFOLLOW`; on Windows the link itself is opened, a reparse point,
/// and isn't a plain file). Without waiting: opening a FIFO waits for a writer, for good (`O_NONBLOCK`, cleared again
/// once the file is known to be plain). And checked on the open file, so nothing can be swapped in after the check.
fn open_plain(repo: &Path, path: &str) -> Option<(File, u64)> {
    // relative and inside the folder: no `..`, no root or drive
    if path.is_empty() || !Path::new(path).components().all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_FLAG_OPEN_REPARSE_POINT: a link or junction is opened itself, not what it points at
        const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(OPEN_REPARSE_POINT);
    }
    let file = options.open(repo.join(path)).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        // a plain file reads as usual from here: some file systems would honour O_NONBLOCK on a read
        let fd = file.as_raw_fd();
        // SAFETY: fcntl on a descriptor this function owns (`file`), with flags only
        let cleared = unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            flags >= 0 && libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK) >= 0
        };
        if !cleared {
            return None;
        }
    }
    Some((file, meta.len()))
}

/// At most `limit` bytes of an open file (`take`): a file that grows while it is read stops there all the same.
fn read_upto(file: File, limit: u64, size: u64) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(usize::try_from(size.min(limit)).unwrap_or(0));
    file.take(limit).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

/// A file git tracks in `repo`, as text (`open_plain`): at most `max` bytes of it, None when it isn't a plain file.
pub(crate) fn read_text(repo: &Path, path: &str, max: usize) -> Option<String> {
    let (file, size) = open_plain(repo, path)?;
    let bytes = read_upto(file, max as u64, size)?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// A file git tracks in `repo`, whole and as UTF-8 (`open_plain`): None when it isn't a plain file, is bigger than `max`
/// bytes (left out, not cut short) or isn't UTF-8.
pub(crate) fn read_whole(repo: &Path, path: &str, max: u64) -> Option<String> {
    let (file, size) = open_plain(repo, path)?;
    if size > max {
        return None;
    }
    // one byte more than allowed tells a file that grew past `max` since it was opened
    let bytes = read_upto(file, max + 1, size)?;
    if bytes.len() as u64 > max {
        return None;
    }
    String::from_utf8(bytes).ok()
}

pub(crate) fn lang_of(path: &str) -> Option<Lang> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".d.ts") || lower.ends_with(".min.js") {
        return None;
    }
    let ext = lower.rsplit_once('.')?.1;
    Some(match ext {
        "ts" | "mts" | "cts" => Lang::TypeScript,
        "tsx" => Lang::Tsx,
        "js" | "mjs" | "cjs" | "jsx" => Lang::JavaScript,
        "php" => Lang::Php,
        "dart" => Lang::Dart,
        "kt" | "kts" => Lang::Kotlin,
        "vue" | "svelte" => Lang::Component,
        _ => return None,
    })
}

/// Whether `path` is in a folder of installed, built or generated code.
pub(crate) fn skipped(path: &str) -> bool {
    path.split('/').rev().skip(1).any(|dir| SKIPPED_DIRS.contains(&dir))
}

struct Grammar {
    language: Language,
    query: Query,
}

fn grammar(lang: Lang) -> Grammar {
    let (language, query): (Language, &str) = match lang {
        Lang::TypeScript => (tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(), include_str!("../queries/typescript.scm")),
        Lang::Tsx | Lang::Component => (tree_sitter_typescript::LANGUAGE_TSX.into(), include_str!("../queries/tsx.scm")),
        Lang::JavaScript => (tree_sitter_javascript::LANGUAGE.into(), include_str!("../queries/javascript.scm")),
        Lang::Php => (tree_sitter_php::LANGUAGE_PHP.into(), include_str!("../queries/php.scm")),
        Lang::Dart => (tree_sitter_dart::LANGUAGE.into(), include_str!("../queries/dart.scm")),
        Lang::Kotlin => (tree_sitter_kotlin_ng::LANGUAGE.into(), include_str!("../queries/kotlin.scm")),
    };
    // The queries ship with Hersir and a test compiles each: a failure here is a bug, not the project's.
    let query = Query::new(&language, query).expect("a query in queries/ doesn't compile");
    Grammar { language, query }
}

/// How a file's read ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Whole,
    /// The grammar met syntax it can't read.
    Partial,
    /// Stopped at `FILE_TIME`, or past `MATCH_LIMIT`.
    CutOff,
}

/// What a view of the Reference keeps for the next view of the same repository (`reference_kept`, GA-362): each file's
/// declarations and how its read ended, by its path, with a hash of the text they came from. The next view reads every
/// file again but parses only the ones whose text changed: a new commit, or an edit not committed yet. So a file that
/// runs into `FILE_TIME` costs that time once, not at every view.
#[derive(Debug, Default)]
pub struct Kept {
    files: HashMap<String, KeptFile>,
}

#[derive(Debug)]
struct KeptFile {
    hash: [u8; 32],
    entries: Vec<Entry>,
    outcome: Outcome,
}

impl Kept {
    /// How many files it keeps.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// The declarations in `files` (paths relative to `repo`, as git lists them), in file order and by line within a file.
/// Files in installed, built or generated folders, minified or declaration files, files over `MAX_BYTES`, files that
/// aren't UTF-8 and anything that isn't a plain file (`read_whole`) are left out; at most `MAX_FILES` files are read,
/// each for at most `FILE_TIME`.
pub fn reference(repo: &Path, files: &[String]) -> Reference {
    reference_kept(repo, files, &mut Kept::default())
}

/// `reference`, reusing what the last view of this repository kept (`kept`) for every file whose text is the same, and
/// keeping this view's for the next one.
pub fn reference_kept(repo: &Path, files: &[String], kept: &mut Kept) -> Reference {
    let mut grammars: HashMap<Lang, Grammar> = HashMap::new();
    let mut out = Reference::default();
    let mut now: HashMap<String, KeptFile> = HashMap::new();
    let wanted = files.iter().filter_map(|p| lang_of(p).map(|l| (p, l))).filter(|(p, _)| !skipped(p)).take(MAX_FILES);
    for (path, lang) in wanted {
        let Some(text) = read_whole(repo, path, MAX_BYTES) else { continue };
        out.files += 1;
        out.read.push(path.clone());
        let hash: [u8; 32] = Sha256::digest(text.as_bytes()).into();
        let file = match kept.files.remove(path) {
            Some(file) if file.hash == hash => file,
            _ => {
                let g = grammars.entry(lang).or_insert_with(|| grammar(lang));
                let mut entries = vec![];
                let outcome = read_file(g, lang, path, &text, &mut entries);
                KeptFile { hash, entries, outcome }
            }
        };
        out.entries.extend(file.entries.iter().cloned());
        match file.outcome {
            Outcome::Whole => {}
            Outcome::Partial => out.partial.push(path.clone()),
            Outcome::CutOff => out.cut_off.push(path.clone()),
        }
        now.insert(path.clone(), file);
    }
    // only this view's files: one deleted or no longer read isn't kept
    kept.files = now;
    out
}

/// Reads one file's declarations into `out`, within `FILE_TIME`: a .vue or .svelte file's script blocks one after the
/// other, until one isn't read whole.
fn read_file(g: &Grammar, lang: Lang, path: &str, text: &str, out: &mut Vec<Entry>) -> Outcome {
    let until = Instant::now() + FILE_TIME;
    if lang != Lang::Component {
        return read(g, lang, path, text, text, 0, until, out);
    }
    out.push(component_entry(path, text));
    for (offset, block) in script_blocks(text) {
        let outcome = read(g, lang, path, block, text, offset, until, out);
        if outcome != Outcome::Whole {
            return outcome;
        }
    }
    Outcome::Whole
}

/// Reads the declarations in `text` (all of a file, or a script block `offset` lines into it) into `out`, until `until`:
/// the parse, the query and the work on each match stop there, keeping what was found (CutOff), as they do past
/// `MATCH_LIMIT`. Partial when the grammar met syntax it can't read. `file`: the whole file, for the kinds that depend on
/// what it imports.
#[allow(clippy::too_many_arguments)]
fn read(g: &Grammar, lang: Lang, path: &str, text: &str, file: &str, offset: usize, until: Instant, out: &mut Vec<Entry>) -> Outcome {
    let mut parser = Parser::new();
    if parser.set_language(&g.language).is_err() {
        return Outcome::Partial;
    }
    let stopped = Cell::new(false);
    let late = || {
        if Instant::now() >= until {
            stopped.set(true);
        }
        stopped.get()
    };
    let bytes = text.as_bytes();
    let mut parsing = |_: &ParseState| if late() { ControlFlow::Break(()) } else { ControlFlow::Continue(()) };
    let tree = parser.parse_with_options(&mut |at: usize, _| bytes.get(at..).unwrap_or_default(), None,
        Some(ParseOptions::new().progress_callback(&mut parsing)));
    let Some(tree) = tree else { return if stopped.get() { Outcome::CutOff } else { Outcome::Partial } };
    let lines: Vec<&str> = text.split('\n').collect();
    let names = g.query.capture_names();
    let mut cursor = QueryCursor::new();
    cursor.set_match_limit(MATCH_LIMIT);
    let mut querying = |_: &QueryCursorState| if late() { ControlFlow::Break(()) } else { ControlFlow::Continue(()) };
    let mut matches = cursor.matches_with_options(&g.query, tree.root_node(), bytes,
        QueryCursorOptions::new().progress_callback(&mut querying));
    let mut found = vec![];
    while let Some(m) = matches.next() {
        // the work on one match can be long too (the comment above it, what it extends), so the time counts here as well
        if late() {
            break;
        }
        let Some(sym) = m.captures().iter().find(|c| names[c.index as usize].starts_with("sym.")) else { continue };
        let kind = &names[sym.index as usize]["sym.".len()..];
        if LEFT_OUT.contains(&kind) {
            continue;
        }
        let name = m.captures().iter().find(|c| names[c.index as usize] == "name")
            .and_then(|c| c.node.utf8_text(text.as_bytes()).ok()).unwrap_or("(anonymous)");
        let node = sym.node;
        let body = node.utf8_text(text.as_bytes()).unwrap_or_default();
        let start = node.start_position().row;
        found.push(Entry {
            kind: kind_for(lang, kind, name, body, file, &node),
            syntactic: kind.to_string(),
            name: name.to_string(),
            path: path.to_string(),
            line: start + 1 + offset,
            end_line: node.end_position().row + 1 + offset,
            doc: doc_above(&lines, start),
            text: first_lines(body, TEXT_LINES),
        });
    }
    drop(matches);
    found.sort_by_key(|e| e.line);
    found.dedup_by(|a, b| a.line == b.line && a.name == b.name);
    out.append(&mut found);
    if stopped.get() || cursor.did_exceed_match_limit() {
        Outcome::CutOff
    } else if tree.root_node().has_error() {
        Outcome::Partial
    } else {
        Outcome::Whole
    }
}

/// A .vue or .svelte file is a component named after the file, whatever its script declares.
fn component_entry(path: &str, text: &str) -> Entry {
    let file = path.rsplit('/').next().unwrap_or(path);
    let name = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    Entry {
        name: name.to_string(), kind: "component".into(), syntactic: "component".into(), path: path.to_string(), line: 1,
        end_line: text.split('\n').count(),
        doc: String::new(), text: first_lines(text, TEXT_LINES),
    }
}

/// The `<script>` blocks of a .vue or .svelte file: each one's text, and how many lines into the file it starts.
fn script_blocks(text: &str) -> Vec<(usize, &str)> {
    let mut out = vec![];
    let mut from = 0;
    while let Some(open) = text[from..].find("<script").map(|i| i + from) {
        let Some(body) = text[open..].find('>').map(|i| open + i + 1) else { break };
        let Some(close) = text[body..].find("</script>").map(|i| body + i) else { break };
        out.push((text[..body].matches('\n').count(), &text[body..close]));
        from = close + "</script>".len();
    }
    out
}

/// The comment right above line `start` (counted from 0), without its markers: `/** … */`, `/* … */`, a run of `//` or
/// `///` lines, or PHP's `#` (not an attribute, `#[…]`).
fn doc_above(lines: &[&str], start: usize) -> String {
    let mut i = start;
    let mut got = vec![];
    while i > 0 {
        let l = lines[i - 1].trim();
        let block = l.starts_with("/*") || l.starts_with('*') || l.ends_with("*/");
        let line = l.starts_with("//") || (l.starts_with('#') && !l.starts_with("#["));
        if !block && !line {
            break;
        }
        got.push(l);
        i -= 1;
        if l.starts_with("/*") {
            break;
        }
    }
    got.reverse();
    let text: Vec<&str> = got.iter().map(|l| {
        let l = l.trim_start_matches("/**").trim_start_matches("/*").trim_end_matches("*/");
        let l = l.trim_start_matches("///").trim_start_matches("//").trim_start_matches('#');
        l.trim().trim_start_matches('*').trim()
    }).collect();
    text.join("\n").trim().to_string()
}

fn first_lines(text: &str, n: usize) -> String {
    let mut lines: Vec<&str> = text.split('\n').take(n + 1).collect();
    if lines.len() > n {
        lines.truncate(n);
        lines.push("…");
    }
    lines.join("\n")
}

/// Whether `node` renders markup: a JSX element or fragment anywhere inside it.
fn has_jsx(node: &Node) -> bool {
    let mut cursor = node.walk();
    let mut stack = vec![*node];
    while let Some(n) = stack.pop() {
        if matches!(n.kind(), "jsx_element" | "jsx_self_closing_element" | "jsx_fragment") {
            return true;
        }
        stack.extend(n.children(&mut cursor));
    }
    false
}

/// What yodocs' kind detection makes of a declaration: React's components and hooks by their conventions, Laravel's
/// classes by what they extend or implement (by the last part of the name, `Controller`, not its namespace), middleware
/// by the shape of its handle method, and exceptions by name; else the query's own kind.
fn kind_for(lang: Lang, kind: &str, name: &str, text: &str, file: &str, node: &Node) -> String {
    let script = matches!(lang, Lang::Tsx | Lang::JavaScript | Lang::Component);
    if kind == "function" && script && name.starts_with(|c: char| c.is_ascii_uppercase()) && has_jsx(node) {
        return "component".into();
    }
    if kind == "function" && name.len() > 3 && name.starts_with("use") && name[3..].starts_with(|c: char| c.is_ascii_uppercase()) {
        return "hook".into();
    }
    if lang == Lang::Php && matches!(kind, "class" | "interface" | "trait" | "enum") {
        return php_kind(kind, name, text, file).to_string();
    }
    let header = text.split(['{', '(']).next().unwrap_or(text);
    let words: Vec<&str> = header.split_whitespace().collect();
    // Kotlin: an interface and an enum class are class declarations with their keyword
    if lang == Lang::Kotlin && kind == "class" {
        if words.contains(&"interface") {
            return "interface".into();
        }
        if words.windows(2).any(|w| w == ["enum", "class"]) {
            return "enum".into();
        }
    }
    // Flutter: a class extending a widget, or a widget's State, is a widget
    if lang == Lang::Dart && kind == "class" {
        let base = words.iter().skip_while(|w| **w != "extends").nth(1).map(|b| b.split('<').next().unwrap_or(b));
        if base.is_some_and(|b| b.ends_with("Widget") || b == "State") {
            return "widget".into();
        }
    }
    kind.to_string()
}

fn php_kind<'a>(kind: &'a str, name: &str, text: &str, file: &str) -> &'a str {
    let header = text.split('{').next().unwrap_or(text);
    let bases: Vec<&str> = header.split_whitespace().skip_while(|w| *w != "extends" && *w != "implements")
        .filter(|w| *w != "extends" && *w != "implements")
        .flat_map(|w| w.split(',')).map(|w| w.trim().rsplit('\\').next().unwrap_or("")).filter(|w| !w.is_empty()).collect();
    let is = |base: &str| bases.contains(&base);
    if is("Model") || is("Authenticatable") || is("Pivot") {
        "model"
    } else if is("Migration") {
        "migration"
    } else if is("Command") {
        "command"
    } else if is("Seeder") {
        "seeder"
    } else if is("Factory") {
        "factory"
    } else if is("Notification") {
        "notification"
    } else if is("Mailable") {
        "mailable"
    } else if is("ShouldQueue") {
        "job"
    } else if is("CastsAttributes") {
        "cast"
    } else if is("ValidationRule") {
        "validation rule"
    } else if is("Controller") {
        "controller"
    } else if is("Tool") && file.contains("Laravel\\Mcp\\Server\\Tool") {
        "mcp tool"
    } else if is("Resource") && file.contains("Laravel\\Mcp\\Server\\Resource") {
        "mcp resource"
    } else if is("Prompt") && file.contains("Laravel\\Mcp\\Server\\Prompt") {
        "mcp prompt"
    } else if is("Server") && file.contains("Laravel\\Mcp\\Server;") {
        "mcp server"
    } else if is("JsonResource") || is("ResourceCollection") {
        "api resource"
    } else if is("Component") && file.contains("Livewire\\Component") {
        "livewire component"
    } else if is("Component") {
        "blade component"
    } else if text.contains("function handle(") && text.contains("$next") {
        "middleware"
    } else if bases.iter().any(|b| b.ends_with("Exception") || b.ends_with("Error")) || name.ends_with("Exception") {
        "exception"
    } else {
        kind
    }
}
