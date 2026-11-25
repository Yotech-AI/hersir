//! Docs: the optional `changelog` on an agent's result line, the card's line in its project's changelog, read apart from
//! the verdict so a result line parses as it always did; and the Docs section that asks a builder for it.
use gizai_agents::outcome::{changelog, parse};
use gizai_agents::prompt::{with_changelog, CHANGELOG_SECTION};

#[test]
fn the_changelog_line_on_the_result_line() {
    let line = |extra: &str| format!("Done.\nGIZAI_RESULT: {{\"outcome\":\"ready_for_testing\",\"summary\":\"Built.\",\"issues\":[]{extra}}}");
    assert_eq!(changelog(&line(r#","changelog":{"type":"Added","text":"  Invoices export as CSV.  "}"#)),
               Some(("Added".to_string(), "Invoices export as CSV.".to_string())));
    for none in [line(""), line(r#","changelog":"Added: CSV""#), line(r#","changelog":{"type":"Added"}"#), line(r#","changelog":{"type":" ","text":"x"}"#)] {
        assert_eq!(changelog(&none), None, "{none}");
        assert!(parse(&none).is_some(), "the verdict reads as it always did: {none}");
    }
    assert_eq!(changelog(r#"GIZAI_RESULT: {"outcome":"nonsense","changelog":{"type":"Added","text":"x"}}"#), None, "only on a result line that counts");
}

#[test]
fn a_builder_in_a_project_with_a_changelog_is_asked_for_the_line() {
    let asked = with_changelog("Do the task.\n", true);
    assert!(asked.starts_with("Do the task.\n\n## Docs\n\n") && asked.ends_with(CHANGELOG_SECTION), "{asked}");
    assert!(CHANGELOG_SECTION.contains("Added, Changed, Deprecated, Removed, Fixed or Security"));
    assert_eq!(with_changelog("Do the task.\n", false), "Do the task.\n");
}
