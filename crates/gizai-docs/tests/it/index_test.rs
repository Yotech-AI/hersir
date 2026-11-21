//! A project's docs index, yodocs' index.json (schema 1): entries with ids, signatures and what they extend; the
//! Folders tree and the Routes list; guides as HTML; every section present, empty when there is nothing; and a file's
//! source only for a file the Reference reads.
use std::path::Path;

use gizai_docs::api::Route;
use gizai_docs::guides::Guide;
use gizai_docs::index::{build, entries, history, patches, signature, source, CommitFile, Input};
use gizai_docs::{Entry, Reference};

fn entry(name: &str, kind: &str, path: &str, line: usize, doc: &str, text: &str) -> Entry {
    Entry { name: name.into(), kind: kind.into(), path: path.into(), line, end_line: line + 3, doc: doc.into(), text: text.into() }
}

fn reference() -> Reference {
    Reference {
        entries: vec![
            entry("Invoice", "model", "app/Models/Invoice.php", 8, "An invoice.",
                  "#[Fillable([\n  'total',\n])]\nfinal class Invoice extends Model implements HasMedia, Auditable\n{\n}"),
            entry("InvoiceController", "controller", "app/Http/Controllers/InvoiceController.php", 5, "", "class InvoiceController extends Controller {"),
            entry("GET", "function", "src/app/api/leads/route.ts", 1, "", "export async function GET(req: Request) {\n  return x;\n}"),
            entry("GET", "function", "src/app/api/leads/route.ts", 9, "", "export const GET = handler;"),
        ],
        files: 4,
        read: vec!["app/Models/Invoice.php".into(), "app/Http/Controllers/InvoiceController.php".into(),
                   "src/app/api/leads/route.ts".into(), "app/helpers.php".into()],
        partial: vec!["app/helpers.php".into()],
    }
}

#[test]
fn entries_get_ids_signatures_and_what_they_extend() {
    let got = entries(&reference());
    assert_eq!(got.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["app/Models/Invoice.php#Invoice",
        "app/Http/Controllers/InvoiceController.php#InvoiceController", "src/app/api/leads/route.ts#GET", "src/app/api/leads/route.ts#GET~2"]);
    let invoice = &got[0];
    assert_eq!(invoice.signature, "final class Invoice extends Model implements HasMedia, Auditable");
    assert_eq!(invoice.facts.iter().map(|f| (f.label.as_str(), f.value.as_str())).collect::<Vec<_>>(),
               [("Extends", "Model"), ("Implements", "HasMedia, Auditable")]);
    assert!(invoice.has_description && !got[1].has_description);
    assert_eq!((invoice.line_start, invoice.line_end, invoice.name_line), (8, 11, 8));
    assert_eq!(got[2].signature, "export async function GET(req: Request)");
    assert_eq!(got[3].signature, "export const GET = handler");
    assert_eq!(signature("no declaration here\nstill none"), "no declaration here still none");
}

#[test]
fn every_section_is_in_the_index_in_the_shape_the_viewer_reads() {
    let routes = vec![
        Route { method: "GET".into(), path: "/invoices".into(), handler: "App\\Http\\Controllers\\InvoiceController@index".into(), file: "routes/web.php".into(), line: 3 },
        Route { method: "POST".into(), path: "/api/leads".into(), handler: String::new(), file: "src/app/api/leads/route.ts".into(), line: 1 },
        Route { method: "GET".into(), path: "/up".into(), handler: "Closure".into(), file: "routes/web.php".into(), line: 9 },
    ];
    let guides = vec![
        Guide { path: "README.md".into(), title: "Pearl".into(), body_md: "# Pearl\n\nThe site.\n".into() },
        Guide { path: "docs/api/auth.md".into(), title: "Signing in".into(), body_md: "## Tokens\n".into() },
        Guide { path: "docs/guides/01-install.md".into(), title: "Install".into(), body_md: "Run it.\n".into() },
    ];
    let index = build(Input { title: "Pearl dev docs".into(), commit: "abc1234".into(), branch: "origin/main".into(),
        version: "0.21.0".into(), built_at: "2026-11-21T10:00:00Z".into(), duration_ms: 73, reference: reference(), routes, guides,
        changelog_from: "the cards merged in Hersir".into(), ..Input::default() });
    let json = serde_json::to_value(&index).unwrap();
    for key in ["schema", "project", "stats", "kinds", "groups", "entries", "problems", "guides", "changelog", "decisions", "mcp",
                "mcpTools", "api", "docChanges", "history"] {
        assert!(json.get(key).is_some(), "no {key}");
    }
    assert_eq!(json["schema"], 1);
    assert_eq!(json["api"], serde_json::Value::Null);
    assert_eq!(json["project"]["yodocsVersion"], "0.21.0");
    assert_eq!(json["project"]["builtAt"], "2026-11-21T10:00:00Z");
    assert_eq!(json["stats"], serde_json::json!({ "files": 4, "entries": 4, "described": 1, "problems": 1, "durationMs": 73, "skipped": {} }));
    assert_eq!(json["kinds"][0], serde_json::json!({ "name": "function", "count": 2 }));
    assert_eq!(json["entries"][0]["hasDescription"], true);
    assert_eq!(json["entries"][0]["syntacticKind"], "model");
    assert!(json["entries"][0].get("parent").is_none());
    assert_eq!(json["problems"][0]["file"], "app/helpers.php");

    // Folders: folders first, then files, each counting its entries; a file without any still listed
    let folders = &json["groups"][0];
    assert_eq!((folders["name"].as_str(), folders["type"].as_str(), folders["count"].as_u64()), (Some("Folders"), Some("tree"), Some(4)));
    let app = &folders["rows"][0];
    assert_eq!((app["label"].as_str(), app["target"].as_str(), app["count"].as_u64()), (Some("app"), Some("app"), Some(2)));
    assert_eq!(app["children"].as_array().unwrap().iter().map(|r| r["label"].as_str().unwrap()).collect::<Vec<_>>(), ["Http", "Models", "helpers.php"]);
    assert!(app["children"][2].get("children").is_none(), "a file is a leaf");

    // Routes: each opens the class that answers it, else the first declaration in its file, else nothing
    let routes = &json["groups"][1];
    assert_eq!((routes["name"].as_str(), routes["type"].as_str(), routes["count"].as_u64()), (Some("Routes"), Some("flat"), Some(3)));
    let rows: Vec<(&str, &str, &str)> = routes["rows"].as_array().unwrap().iter()
        .map(|r| (r["label"].as_str().unwrap(), r["badge"].as_str().unwrap(), r["target"].as_str().unwrap())).collect();
    assert_eq!(rows, [("/invoices", "GET", "app/Http/Controllers/InvoiceController.php#InvoiceController"),
                      ("/api/leads", "POST", "src/app/api/leads/route.ts#GET"), ("/up", "GET", "")]);
    // and each entry a route reaches says so, for its page's "Used by routes"
    let routes_of = |n: usize| json["entries"][n]["facts"].as_array().unwrap().iter().filter(|f| f["label"] == "Route")
        .map(|f| f["value"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!((routes_of(1), routes_of(2), routes_of(0)), (vec!["GET /invoices".to_string()], vec!["POST /api/leads".to_string()], vec![]));
    assert_eq!(json["project"]["probes"], serde_json::json!(["routes"]));

    // Guides: an id from the path, a group from the folder under docs/, the Markdown as HTML with its headings
    let g = &json["guides"];
    assert_eq!((g[0]["id"].as_str(), g[0]["group"].as_str(), g[0]["order"].as_u64()), (Some("README"), Some(""), Some(0)));
    assert_eq!((g[1]["id"].as_str(), g[1]["group"].as_str(), g[1]["file"].as_str()), (Some("docs/api/auth"), Some("api"), Some("docs/api/auth.md")));
    assert_eq!((g[2]["id"].as_str(), g[2]["group"].as_str()), (Some("docs/guides/01-install"), Some("")), "docs/guides is plain guides");
    assert_eq!(g[1]["html"], "<h2 id=\"tokens\">Tokens</h2>\n");
    assert_eq!(g[1]["headings"], serde_json::json!([{ "level": 2, "text": "Tokens", "slug": "tokens" }]));
}

fn write(dir: &Path, path: &str, text: &str) -> String {
    let p = dir.join(path);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
    path.to_string()
}

#[test]
fn a_files_source_only_for_a_file_the_reference_reads() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.ts"), "const key = 1;\n").unwrap();
    let mut files = vec![write(d, "src/a.ts", "export const a = 1;\nexport const b = 2;\n"), write(d, ".env", "KEY=1\n"),
                         write(d, "node_modules/x/index.ts", "x\n")];
    write(d, "src/untracked.ts", "const u = 1;\n");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(outside.path().join("secret.ts"), d.join("src/link.ts")).unwrap();
        files.push("src/link.ts".into());
    }
    let a = source(d, &files, "src/a.ts").unwrap();
    assert_eq!((a.path.as_str(), a.lines.as_slice()), ("src/a.ts", ["export const a = 1;", "export const b = 2;", ""].map(String::from).as_slice()));
    for refused in [".env", "node_modules/x/index.ts", "src/untracked.ts", "src/link.ts", "src/../src/a.ts", "../secret.ts"] {
        assert!(source(d, &files, refused).is_none(), "{refused}");
    }
}

#[test]
fn mains_commits_with_their_files_and_a_commits_changed_lines() {
    let log = "\x1eabc1234\x1f2026-11-20T10:00:00+01:00\x1fRemi\x1fAdd the form\n\nA\tsrc/form.tsx\nM\t.env\nR100\tsrc/old.ts\tsrc/new.ts\n\
               \x1edef5678\x1f2026-11-19T09:00:00+01:00\x1fJeffrey\x1fMerge pull request #2";
    let h = history(log);
    assert_eq!(h.len(), 2);
    assert_eq!((h[0].hash.as_str(), h[0].date.as_str(), h[0].author.as_str(), h[0].subject.as_str()),
               ("abc1234", "2026-11-20T10:00:00+01:00", "Remi", "Add the form"));
    let file = |status: &str, path: &str, from: Option<&str>, diff| CommitFile { status: status.into(), path: path.into(), from: from.map(String::from), diff };
    assert_eq!(h[0].files, vec![file("A", "src/form.tsx", None, true), file("M", ".env", None, false), file("R", "src/new.ts", Some("src/old.ts"), true)]);
    assert!(h[1].files.is_empty());

    let show = "diff --git a/src/form.tsx b/src/form.tsx\nnew file mode 100644\n--- /dev/null\n+++ b/src/form.tsx\n@@ -0,0 +1,2 @@\n\
                +export function Form() {}\n+// done\ndiff --git a/.env b/.env\n--- a/.env\n+++ b/.env\n@@ -1 +1 @@\n-KEY=1\n+KEY=2\n\
                diff --git a/src/gone.ts b/src/gone.ts\ndeleted file mode 100644\n--- a/src/gone.ts\n+++ /dev/null\n@@ -1 +0,0 @@\n-x\n\
                diff --git a/logo.png b/logo.png\nBinary files a/logo.png and b/logo.png differ";
    let p = patches(show);
    assert_eq!(p.files.keys().collect::<Vec<_>>(), ["src/form.tsx", "src/gone.ts"], ".env never shows its lines, a binary file has none");
    assert_eq!(p.files["src/form.tsx"].lines, ["@@ -0,0 +1,2 @@", "+export function Form() {}", "+// done"]);
    assert_eq!(p.files["src/gone.ts"].lines, ["@@ -1 +0,0 @@", "-x"]);
    let long = format!("diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1,500 +1,500 @@\n{}", "+x\n".repeat(500));
    let p = patches(&long);
    assert_eq!((p.files["a.ts"].lines.len(), p.files["a.ts"].truncated), (400, true));
}
