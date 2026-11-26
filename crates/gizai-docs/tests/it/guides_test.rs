//! Docs → Guides: the root README first, then the Markdown under docs/ and doc/ by path, each titled by its first
//! heading or its file name; changelogs, licences, other Markdown and Markdown in installed folders left out.
use std::path::Path;

use gizai_docs::guides::guides;

fn write(dir: &Path, path: &str, text: &str) -> String {
    let p = dir.join(path);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
    path.to_string()
}

#[test]
fn the_readme_first_then_the_docs_folders_markdown_by_path() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let files = vec![
        write(d, "docs/setup.md", "Some intro.\n\n# Setting up\n\nRun it.\n"),
        write(d, "docs/api/auth.md", "# Signing in\n"),
        write(d, "README.md", "# Pearl\n\nThe corporate site.\n"),
        write(d, "doc/faq.mdx", "No heading here.\n"),
        write(d, "CHANGELOG.md", "# Changelog\n"),
        write(d, "docs/LICENSE.md", "# MIT\n"),
        write(d, "CONTRIBUTING.md", "# Contributing\n"),
        write(d, "node_modules/x/README.md", "# x\n"),
        write(d, "src/notes.md", "# Notes\n"),
    ];
    let got = guides(d, &files);
    assert_eq!(got.iter().map(|g| (g.path.as_str(), g.title.as_str())).collect::<Vec<_>>(),
               [("README.md", "Pearl"), ("doc/faq.mdx", "faq"), ("docs/api/auth.md", "Signing in"), ("docs/setup.md", "Setting up")]);
    assert_eq!(got[0].body_md, "# Pearl\n\nThe corporate site.\n");
}
