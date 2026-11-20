//! Docs → Reference: the declarations in a project's code, ported from yodocs. Each file is read with tree-sitter and its
//! language's query (`queries/`, yodocs' own), then each declaration gets its kind (a React component or hook, a Laravel
//! model, controller or migration…), the comment right above it and its text. TypeScript, TSX, JavaScript, PHP, Dart,
//! Kotlin, and the script blocks of Vue and Svelte files (read as TSX). Dart's and Kotlin's queries are new: the grammars
//! yodocs read named their nodes differently.
//! The caller lists the files (git's tracked files) and this reads them from disk: it starts no program.
pub mod api;
pub mod guides;

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Node, Parser, Query, QueryCursor};

/// One declaration, as Docs → Reference lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    /// component, hook, function, class, interface, type, enum, constant, model, controller, migration…
    pub kind: String,
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

/// What Docs → Reference shows: the declarations by file and line, how many files were read, and the files with
/// syntax their grammar can't read, whose declarations may be incomplete.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub entries: Vec<Entry>,
    pub files: usize,
    pub partial: Vec<String>,
}

/// A file bigger than this is left out: generated or bundled code, not something written by hand.
pub const MAX_BYTES: u64 = 512 * 1024;
/// At most this many files are read.
pub const MAX_FILES: usize = 5000;
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

fn lang_of(path: &str) -> Option<Lang> {
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

/// The declarations in `files` (paths relative to `repo`, as git lists them), in file order and by line within a file.
/// Files in installed, built or generated folders, minified or declaration files, files over `MAX_BYTES` and files that
/// aren't UTF-8 are left out; at most `MAX_FILES` files are read.
pub fn reference(repo: &Path, files: &[String]) -> Reference {
    let mut grammars: HashMap<Lang, Grammar> = HashMap::new();
    let mut out = Reference::default();
    let wanted = files.iter().filter_map(|p| lang_of(p).map(|l| (p, l))).filter(|(p, _)| !skipped(p)).take(MAX_FILES);
    for (path, lang) in wanted {
        let full = repo.join(path);
        if std::fs::metadata(&full).map_or(true, |m| !m.is_file() || m.len() > MAX_BYTES) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&full) else { continue };
        let g = grammars.entry(lang).or_insert_with(|| grammar(lang));
        out.files += 1;
        let complete = if lang == Lang::Component {
            out.entries.push(component_entry(path, &text));
            script_blocks(&text).into_iter().all(|(offset, block)| read(g, lang, path, block, &text, offset, &mut out.entries))
        } else {
            read(g, lang, path, &text, &text, 0, &mut out.entries)
        };
        if !complete {
            out.partial.push(path.clone());
        }
    }
    out
}

/// Reads the declarations in `text` (all of a file, or a script block `offset` lines into it) into `out`. False when the
/// grammar met syntax it can't read. `file`: the whole file, for the kinds that depend on what it imports.
fn read(g: &Grammar, lang: Lang, path: &str, text: &str, file: &str, offset: usize, out: &mut Vec<Entry>) -> bool {
    let mut parser = Parser::new();
    if parser.set_language(&g.language).is_err() {
        return false;
    }
    let Some(tree) = parser.parse(text, None) else { return false };
    let lines: Vec<&str> = text.split('\n').collect();
    let names = g.query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&g.query, tree.root_node(), text.as_bytes());
    let mut found = vec![];
    while let Some(m) = matches.next() {
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
            name: name.to_string(),
            path: path.to_string(),
            line: start + 1 + offset,
            end_line: node.end_position().row + 1 + offset,
            doc: doc_above(&lines, start),
            text: first_lines(body, TEXT_LINES),
        });
    }
    found.sort_by_key(|e| e.line);
    found.dedup_by(|a, b| a.line == b.line && a.name == b.name);
    out.append(&mut found);
    !tree.root_node().has_error()
}

/// A .vue or .svelte file is a component named after the file, whatever its script declares.
fn component_entry(path: &str, text: &str) -> Entry {
    let file = path.rsplit('/').next().unwrap_or(path);
    let name = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    Entry {
        name: name.to_string(), kind: "component".into(), path: path.to_string(), line: 1, end_line: text.split('\n').count(),
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
