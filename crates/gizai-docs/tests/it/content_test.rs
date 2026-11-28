//! The docs a project writes itself, as yodocs read them: CHANGELOG.md in Keep a Changelog format (Unreleased first,
//! then newest, a section outside the format named as a problem), the decision records in docs/decisions (number,
//! title, status, date, sections, related files; the highest number first), and the guides without the decision
//! records; only files git tracks, never through a link.
use std::path::Path;

use gizai_docs::content::{changelog, decisions, decisions_folder, front_matter, log_entries, read};

fn write(dir: &Path, path: &str, text: &str) -> String {
    let p = dir.join(path);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
    path.to_string()
}

const CHANGELOG: &str = "# Changelog

- a stray line before any version

## [0.3.0] - 2026-10-30

### Removed
- The old **settings** page

## [Unreleased]

### Added
- Search ranks exact names first (SDH-112)

### Chores
- Bumped a dependency

## [0.4.0](https://github.com/acme/site/compare/v0.3.0...v0.4.0) - 2026-11-05

### Fixed
- Routes without a name show their URI
* Deep links scroll below the bar
";

#[test]
fn a_changelog_in_keep_a_changelog_format() {
    let (releases, problems) = changelog("CHANGELOG.md", CHANGELOG);
    assert_eq!(releases.iter().map(|r| (r.version.as_str(), r.date.as_str(), r.unreleased)).collect::<Vec<_>>(),
               [("Unreleased", "", true), ("0.4.0", "2026-11-05", false), ("0.3.0", "2026-10-30", false)]);
    assert_eq!(releases[1].compare_url, "https://github.com/acme/site/compare/v0.3.0...v0.4.0");
    let kinds = |n: usize| releases[n].changes.iter().map(|c| (c.kind.as_str(), c.text.as_str())).collect::<Vec<_>>();
    assert_eq!(kinds(0), [("Added", "Search ranks exact names first (SDH-112)"), ("Other", "Bumped a dependency")]);
    assert_eq!(kinds(1), [("Fixed", "Routes without a name show their URI"), ("Fixed", "Deep links scroll below the bar")]);
    assert_eq!(releases[2].changes[0].html, "The old <strong>settings</strong> page", "a line of Markdown, without a paragraph around it");
    assert_eq!(problems.iter().map(|p| (p.line, p.message.as_str())).collect::<Vec<_>>(), [
        (Some(3), "an entry before any version heading, skipped"),
        (Some(15), "\"Chores\" is not a Keep a Changelog section, so its entries are grouped under Other"),
    ]);
}

#[test]
fn decision_records_with_their_number_status_sections_and_related_files() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let files = vec![
        write(d, "docs/decisions/0119-allowlist.md", "---\ntitle: Docs open only to allowlisted IPs\ndate: 2026-11-05\n---\n\
# 0119 · Ignored, the front matter names it\n\n**Status:** Accepted\n\n## Context\n\nJeffrey asked for it.\n\n## Decision\n\nOne middleware.\n\n\
## Related\n\n- app/Http/Middleware/RestrictToIpAddresses.php\n- routes/devdoc.php and app/Http/Middleware/RestrictToIpAddresses.php\n"),
        write(d, "docs/decisions/0002_search.md", "# 0002 · Search in the browser\n\nStatus: proposed\nDate: 2026-11-04\n"),
        write(d, "docs/decisions/0010-old.md", "# Old way\n\n* **Status**: superseded by 0119\n"),
        write(d, "docs/decisions/README.md", "# How we decide\n"),
        write(d, "docs/decisions/drafts/0003-later.md", "# Later\n"),
    ];
    assert_eq!(decisions_folder(&files), Some("docs/decisions"));
    let got = decisions(d, &files, "docs/decisions");
    assert_eq!(got.iter().map(|x| (x.id.as_str(), x.number.as_str(), x.title.as_str(), x.status.as_str(), x.date.as_str())).collect::<Vec<_>>(), [
        ("decisions/0119", "0119", "Docs open only to allowlisted IPs", "accepted", "2026-11-05"),
        ("decisions/0010", "0010", "Old way", "superseded", ""),
        ("decisions/0002", "0002", "Search in the browser", "proposed", "2026-11-04"),
    ]);
    let adr = &got[0];
    assert_eq!(adr.sections.iter().map(|s| s.heading.as_str()).collect::<Vec<_>>(), ["Context", "Decision", "Related"]);
    assert_eq!(adr.sections[0].html, "<p>Jeffrey asked for it.</p>\n");
    assert_eq!(adr.related_files, ["app/Http/Middleware/RestrictToIpAddresses.php", "routes/devdoc.php"]);
    assert_eq!(adr.file, "docs/decisions/0119-allowlist.md");
    assert_eq!(adr.headings.iter().map(|h| h.text.as_str()).collect::<Vec<_>>(), ["0119 · Ignored, the front matter names it", "Context", "Decision", "Related"]);
}

#[test]
fn a_dutch_record_with_its_number_and_a_dash_in_the_title_and_its_date_in_the_status_line() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let files = vec![
        write(d, "docs/decisions/0111-ci-op-develop.md", "# 0111 — CI op develop in plaats van op featurebranches\n\n\
**Status:** aanvaard — gebouwd op `chore/otus-395`; kaart #395, 2026-11-23. **Bouwt op:** installs (0031).\n\n## Context\n\nCI draaide op elke pull request.\n"),
        write(d, "docs/decisions/0110-mcp.md", "# 0110 – De teams in de eigen AI\n\nStatus: voorgesteld\nDatum: 2026-11-20\n"),
        write(d, "docs/decisions/0109-old.md", "# 0109: Oude aanpak\n\nStatus: vervangen door 0111 (aanvaard)\n"),
    ];
    let got = decisions(d, &files, "docs/decisions");
    assert_eq!(got.iter().map(|x| (x.number.as_str(), x.title.as_str(), x.status.as_str(), x.date.as_str())).collect::<Vec<_>>(), [
        ("0111", "CI op develop in plaats van op featurebranches", "accepted", "2026-11-23"),
        ("0110", "De teams in de eigen AI", "proposed", "2026-11-20"),
        ("0109", "Oude aanpak", "superseded", ""),
    ], "a dash or colon after the number, Dutch statuses (the first one named), a Datum line, or the date in the status line");
}

#[test]
fn a_status_in_a_language_hersir_doesnt_know_shows_as_written() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let files = vec![
        write(d, "docs/decisions/0007-postgres.md", "# 0007 – Usare Postgres\n\nStatus: accettato il 2026-11-20\n"),
        write(d, "docs/decisions/0008-cache.md", "# 0008 · Niente cache\n\n**Status:** **Proposta**, da discutere\n"),
        write(d, "docs/decisions/0009-senza.md", "# 0009 Senza stato\n\nTesto.\n"),
    ];
    let got = decisions(d, &files, "docs/decisions");
    assert_eq!(got.iter().map(|x| (x.number.as_str(), x.status.as_str(), x.date.as_str())).collect::<Vec<_>>(), [
        ("0009", "unknown", ""), ("0008", "proposta", ""), ("0007", "accettato", "2026-11-20"),
    ], "the status line's first word, never a wrong one; unknown only without a status line");
}

#[test]
fn front_matter_is_its_scalars_and_the_text_after_it() {
    let (data, body) = front_matter("---\ntitle: \"Install\"\norder: 2\n---\n# Install\n");
    assert_eq!((data["title"].as_str(), data["order"].as_str(), body), ("Install", "2", "# Install\n"));
    let (data, body) = front_matter("# No front matter\n");
    assert!(data.is_empty() && body == "# No front matter\n");
}

#[test]
fn a_projects_own_docs_without_the_decisions_among_the_guides_and_never_through_a_link() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.md"), "# Secret\n").unwrap();
    let mut files = vec![write(d, "README.md", "# Pearl\n"), write(d, "docs/setup.md", "# Setting up\n"),
        write(d, "docs/adr/0001-use-postgres.md", "# Use Postgres\n\nStatus: accepted\n"), write(d, "CHANGELOG.md", CHANGELOG)];
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(outside.path().join("secret.md"), d.join("docs/secret.md")).unwrap();
        files.push("docs/secret.md".into());
    }
    let c = read(d, &files);
    assert_eq!(c.guides.iter().map(|g| g.path.as_str()).collect::<Vec<_>>(), ["README.md", "docs/setup.md"]);
    assert_eq!(c.decisions.iter().map(|x| x.title.as_str()).collect::<Vec<_>>(), ["Use Postgres"]);
    assert_eq!((c.changelog_file.as_deref(), c.changelog.len(), c.problems.len()), (Some("CHANGELOG.md"), 3, 2));
    let none = read(d, &["README.md".to_string()]);
    assert!(none.changelog_file.is_none() && none.changelog.is_empty() && none.decisions.is_empty(), "only files git tracks");
}

/// The Chief's log of decisions in Memory, as Jeffrey's SDH note keeps it: a list item per decision, its date first, some
/// under date headings added later; and a card in brackets after the date.
const LOG: &str = "- 2026-11-03: SDH design (3 Nov): SDH-80 owns the Figma base library.\n\
- 2026-11-03: Jefsev OK'd the Reports menu labels ('yes, for now good'): STS's email reports first,\n  then a Finance group. (SDH-80, SDH-81)\n\
- 2026-11-04 (SDH-99): Cache on the servers stays file for now.\n\n\
## 9 november\n\n\
- 2026-11-10: Link rule confirmed: exactly one candidate customer is linked automatically.\n\
- Domains keep their status on migration.\n\n\
## 18 november\n\n\
- 2026-11-25: A very long decision without a short lead of its own that goes on and on about the migration of every domain. And more.\n";

#[test]
fn a_decision_log_is_one_decision_a_line_each_with_its_own_date_and_a_short_title() {
    let got = log_entries(LOG, "2026-11-25");
    let rows: Vec<(&str, &str)> = got.iter().map(|e| (e.date.as_str(), e.title.as_str())).collect();
    assert_eq!(rows, [
        ("2026-11-03", "SDH design (3 Nov)"),
        ("2026-11-03", "Jefsev OK'd the Reports menu labels ('yes, for now good')"),
        ("2026-11-04", "Cache on the servers stays file for now."),
        ("2026-11-10", "Link rule confirmed"),
        ("2026-11-09", "Domains keep their status on migration."),
        ("2026-11-25", "A very long decision without a short lead of its own that goes on and on about the…"),
    ], "a line's own date wins over the heading it sits under; a line without one takes its heading's");
    assert_eq!(got[1].body, "Jefsev OK'd the Reports menu labels ('yes, for now good'): STS's email reports first,\nthen a Finance group. (SDH-80, SDH-81)",
               "the line goes on over its indented lines");
    assert_eq!(got[2].body, "Cache on the servers stays file for now. (SDH-99)", "the card after the date stays, at the end");
    assert!(log_entries("# Use Postgres\n\nOne database.\n\n- Fast\n- Known\n", "2026-11-25").is_empty(), "no dated line: not a log");
}
