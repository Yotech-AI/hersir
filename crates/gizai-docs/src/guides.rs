//! Docs → Guides: a project's own Markdown docs, read from the files git tracks: the README at its root first, then
//! the Markdown under `docs/` or `doc/`, by path. A changelog, a licence and Markdown in installed or built folders are
//! left out.
use std::path::Path;

use serde::Serialize;

/// One guide, as Docs → Guides shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Guide {
    pub path: String,
    /// Its first `# ` heading, else its file name without `.md`.
    pub title: String,
    /// Its Markdown, at most `MAX_BYTES` of it.
    pub body_md: String,
}

/// A guide is read up to this many bytes.
pub const MAX_BYTES: usize = 256 * 1024;
/// At most this many guides.
pub const MAX_GUIDES: usize = 200;

fn is_guide(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    if !(lower.ends_with(".md") || lower.ends_with(".mdx")) || crate::skipped(path) {
        return false;
    }
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    if name.starts_with("changelog") || name.starts_with("license") || name.starts_with("licence") {
        return false;
    }
    lower == "readme.md" || lower.starts_with("docs/") || lower.starts_with("doc/")
}

/// The guides among `files` (relative to `repo`, as git lists them): the root README first, then by path.
pub fn guides(repo: &Path, files: &[String]) -> Vec<Guide> {
    let mut paths: Vec<&String> = files.iter().filter(|f| is_guide(f)).collect();
    paths.sort_by_key(|p| (!p.eq_ignore_ascii_case("readme.md"), p.to_ascii_lowercase()));
    paths.into_iter().take(MAX_GUIDES).filter_map(|path| {
        let text = crate::read_text(repo, path, MAX_BYTES)?;
        let file = path.rsplit('/').next().unwrap_or(path);
        let stem = file.rsplit_once('.').map_or(file, |(s, _)| s);
        let title = text.lines().find_map(|l| l.strip_prefix("# ")).map(str::trim).filter(|t| !t.is_empty()).unwrap_or(stem);
        Some(Guide { path: path.clone(), title: title.to_string(), body_md: text })
    }).collect()
}
