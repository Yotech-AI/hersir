//! The docs for agents, as yodocs wrote them: llms.txt, the way in, with every section named and the MCP servers and
//! endpoints listed whole; and each page as Markdown at the address the page's own Markdown links use.
use gizai_docs::agents::{llms_txt, page, plain};
use gizai_docs::api::Route;
use gizai_docs::guides::Guide;
use gizai_docs::index::{build, Change, Decision, Input, McpServer, Release};
use gizai_docs::{Entry, Reference};

fn index() -> gizai_docs::index::Index {
    let reference = Reference {
        entries: vec![Entry { name: "Invoice".into(), kind: "model".into(), syntactic: "class".into(), path: "app/Models/Invoice.php".into(),
            line: 2, end_line: 4, doc: "An invoice.".into(), text: "class Invoice extends Model\n{\n}".into() }],
        files: 1, read: vec!["app/Models/Invoice.php".into()], partial: vec![], cut_off: vec![],
    };
    build(Input {
        title: "Pearl dev docs".into(), commit: "abc1234".into(), branch: "origin/main".into(), version: "0.19.0".into(),
        built_at: "2026-11-21T10:00:00Z".into(), reference,
        routes: vec![Route { method: "GET".into(), path: "/invoices".into(), handler: "App\\Models\\Invoice@index".into(), file: "routes/web.php".into(), line: 3 }],
        guides: vec![Guide { path: "README.md".into(), title: "Pearl".into(), body_md: "# Pearl\n\nThe **site**.\n\n- one\n- two\n".into() }],
        changelog: vec![Release { version: "0.4.0".into(), date: "2026-11-05".into(), unreleased: false, compare_url: String::new(),
            changes: vec![Change { kind: "Fixed".into(), text: "Routes show their URI".into(), html: String::new() }] }],
        decisions: vec![Decision { id: "decisions/0119".into(), number: "0119".into(), title: "Allowlisted IPs only".into(), date: "2026-11-05".into(),
            status: "accepted".into(), sections: vec![], related_files: vec![], file: "docs/decisions/0119-ips.md".into(),
            html: "<h2 id=\"context\">Context</h2>\n<p>Jeffrey asked.</p>\n".into(), headings: vec![] }],
        mcp: vec![McpServer { name: "sts".into(), transport: "stdio".into(), target: "php".into(), args: vec!["artisan".into(), "mcp:start".into()],
            env_names: vec!["STS_TOKEN".into()], header_names: vec![], purpose: String::new() }],
        ..Input::default()
    })
}

#[test]
fn llms_txt_names_every_section_with_the_servers_and_endpoints_whole() {
    let text = llms_txt(&index());
    for line in ["# Pearl dev docs", "> Dev docs built from the repository by Hersir 0.19.0.", "> 1 entry across 1 file, 1 with a description.",
        "> Built 2026-11-21T10:00:00Z from abc1234 on origin/main.", "- **sts** (stdio): `php artisan mcp:start`, env STS_TOKEN",
        "From `routes/web.php`, Route handlers.", "- `GET /invoices` — An invoice.", "- [Pearl](md/README.md) — source `README.md`",
        "- [0119 Allowlisted IPs only](md/decisions/0119.md) — accepted", "Latest: 0.4.0 (2026-11-05). See `md/changelog.md`.",
        "Kinds present: model (1)"] {
        assert!(text.contains(line), "no {line:?} in:\n{text}");
    }
}

#[test]
fn each_page_as_markdown_at_its_address() {
    let index = index();
    let source = |path: &str| (path == "app/Models/Invoice.php").then(|| vec!["<?php".to_string(), "class Invoice extends Model".into(), "{".into(), "}".into()]);
    let entry = page(&index, "reference/app/Models/Invoice.php/Invoice.md", &source).unwrap();
    assert!(entry.starts_with("# Invoice\n\nAn invoice.\n"), "{entry}");
    assert!(entry.contains("| Kind | model |") && entry.contains("| File | app/Models/Invoice.php:2-4 |") && entry.contains("| Extends | Model |")
        && entry.contains("| Route | GET /invoices |"), "{entry}");
    assert!(entry.contains("## Source\n\n```php\nclass Invoice extends Model\n{\n}\n```"), "{entry}");
    assert_eq!(page(&index, "README.md", &source).unwrap(), "# Pearl\n\n_Source: README.md_\n\n# Pearl\n\nThe site.\n\n- one\n\n- two\n");
    assert_eq!(page(&index, "decisions/0119.md", &source).unwrap(),
               "# 0119 · Allowlisted IPs only\n\n**Status:** accepted  \n**Date:** 2026-11-05\n\n## Context\n\nJeffrey asked.\n");
    assert_eq!(page(&index, "changelog.md", &source).unwrap(), "# Changelog\n\n## 0.4.0 · 2026-11-05\n\n- **Fixed** Routes show their URI\n");
    assert!(page(&index, "mcp.md", &source).unwrap().contains("- Environment: `${STS_TOKEN}`"));
    for missing in ["reference/nope.md", "nope.md", "README", "../index.json"] {
        assert!(page(&index, missing, &source).is_none(), "{missing}");
    }
}

#[test]
fn html_back_to_markdown_like_text() {
    assert_eq!(plain("<h3 id=\"x\">Run it</h3>\n<p>Use <code>npx x</code> &amp; wait &lt;1s.</p>\n<ul>\n<li>a</li>\n</ul>\n"),
               "### Run it\n\nUse `npx x` & wait <1s.\n\n- a");
}

#[test]
fn llms_txt_says_which_sections_are_switched_off() {
    let index = build(Input { title: "Pearl dev docs".into(), off: vec!["mcp".into(), "reference".into()], ..Input::default() });
    assert!(llms_txt(&index).contains("> Switched off in this project, so not in these docs: mcp, reference (Edit project → Docs)."), "{}", llms_txt(&index));
    assert!(!llms_txt(&build(Input::default())).contains("Switched off"));
}
