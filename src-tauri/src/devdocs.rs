//! A project's docs page (the Docs section, served by `docs_server`): its index, yodocs' `index.json`
//! (`gizai_docs::index`), from the project's linked folder and Hersir's database; a file's source; a commit's changed
//! lines.
//! - Reference, API routes and guides: the files git tracks in the linked folder, as they are on disk.
//! - Commits: main's newest, as a card's changes measure from it.
//! - Changelog: the project's merged cards (`gizai_core::devdocs::changelog`), by month.
//! - Decisions: the shared Memory notes about the project's decisions (`gizai_core::devdocs::decisions`).
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use gizai_agents::worktree;
use gizai_core::devdocs::{self, ChangelogEntry, DecisionNote};
use gizai_core::model::Project;
use gizai_docs::index::{self, Change, CommitPatches, Decision, Index, Input, Problem, Release, SourceFile};
use gizai_docs::markdown;

use crate::AppState;

/// How many of main's commits the page lists.
const HISTORY: usize = 50;

const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October",
    "November", "December"];

/// The project's docs index. A project without a linked repository still gets the changelog and decisions Hersir keeps,
/// and a problem that says how to link one.
pub fn index_of(st: &AppState, project: &Project) -> Result<Index, String> {
    let started = Instant::now();
    let mut input = Input { title: format!("{} dev docs", project.name), version: env!("CARGO_PKG_VERSION").into(), ..Input::default() };
    let mut changelog_file = None;
    match crate::changes::linked_repo(project) {
        Ok(repo) => {
            let unreadable = |e| format!("Hersir can't read {}'s repository: {}", project.name, crate::pulls::plain(e));
            let files = worktree::tracked_files(&repo).map_err(unreadable)?;
            input.reference = gizai_docs::reference(&repo, &files);
            input.routes = gizai_docs::api::api(&repo, &files);
            let content = gizai_docs::content::read(&repo, &files);
            (input.guides, input.changelog, input.decisions, input.mcp, input.api, input.problems) =
                (content.guides, content.changelog, content.decisions, content.mcp, content.api, content.problems);
            changelog_file = content.changelog_file;
            let (label, main) = crate::changes::main_of(project, &repo);
            if let Some(main) = main {
                input.history = index::history(&worktree::log_name_status(&repo, &main, HISTORY).map_err(unreadable)?);
            }
            input.commit = input.history.first().map(|c| c.hash.clone()).unwrap_or_default();
            input.branch = label;
        }
        Err(why) => input.problems.push(Problem { file: String::new(), message: why, line: None }),
    }
    // the project's own CHANGELOG.md and decision records, then what Hersir keeps
    let merged = releases(&devdocs::changelog(&st.db, &project.id).map_err(|e| e.to_string())?);
    input.changelog_from = match (&changelog_file, merged.is_empty()) {
        (Some(file), true) => format!("{file} in Keep a Changelog format"),
        (Some(file), false) => format!("{file} and the cards merged in Hersir"),
        (None, _) => "the cards merged in Hersir (in a Deploy or Done column)".into(),
    };
    input.changelog.extend(merged);
    input.changelog.sort_by(|a, b| b.unreleased.cmp(&a.unreleased).then_with(|| b.date.cmp(&a.date)));
    input.decisions.extend(devdocs::decisions(&st.db, project).map_err(|e| e.to_string())?.iter().map(decision));
    input.built_at = iso(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64));
    input.duration_ms = started.elapsed().as_millis() as u64;
    let index = index::build(input);
    Ok(index)
}

/// One file of the project for an entry's Source: a file the Reference reads, else None.
pub fn source_of(project: &Project, path: &str) -> Option<SourceFile> {
    let repo = crate::changes::linked_repo(project).ok()?;
    let files = worktree::tracked_files(&repo).ok()?;
    index::source(&repo, &files, path)
}

/// A commit's changed lines, of the files the Reference reads.
pub fn patches_of(project: &Project, commit: &str) -> Result<CommitPatches, String> {
    let repo = crate::changes::linked_repo(project)?;
    let show = worktree::show_patch(&repo, commit)
        .map_err(|e| format!("Hersir can't read {}'s commit {commit}: {}", project.name, crate::pulls::plain(e)))?;
    Ok(index::patches(&show))
}

/// The merged cards, newest first, as one release a month ("November 2026"), dated by its newest card.
fn releases(merged: &[ChangelogEntry]) -> Vec<Release> {
    let mut out: Vec<Release> = vec![];
    for card in merged {
        let day = crate::tools::ymd(card.at);
        let month = day[5..7].parse::<usize>().unwrap_or(1).clamp(1, 12);
        let version = format!("{} {}", MONTHS[month - 1], &day[..4]);
        if out.last().is_none_or(|r| r.version != version) {
            out.push(Release { version, date: day, unreleased: false, compare_url: String::new(), changes: vec![] });
        }
        if let Some(release) = out.last_mut() {
            release.changes.push(Change {
                kind: "Other".into(),
                text: format!("{} {}", card.identifier, card.title),
                html: format!("<span class=\"mono\">{}</span> {}", markdown::escape(&card.identifier), markdown::escape(&card.title)),
            });
        }
    }
    out
}

/// A decision note as a decision: the Chief made it, so it counts as accepted. Its heading, if it repeats the title,
/// goes: the page shows the title already.
fn decision(note: &DecisionNote) -> Decision {
    let heading = format!("# {}", note.title);
    let body = note.body_md.trim_start().strip_prefix(heading.as_str()).filter(|rest| rest.is_empty() || rest.starts_with('\n'))
        .unwrap_or(&note.body_md);
    let rendered = markdown::render(body);
    Decision {
        id: note.id.clone(),
        number: String::new(),
        title: note.title.clone(),
        date: crate::tools::ymd(note.updated_at),
        status: "accepted".into(),
        sections: vec![],
        related_files: vec![],
        file: note.path.clone(),
        html: rendered.html,
        headings: rendered.headings,
    }
}

/// Unix ms → "2026-11-21T10:00:00Z".
fn iso(ms: i64) -> String {
    let s = ms.div_euclid(1000);
    format!("{}T{:02}:{:02}:{:02}Z", crate::tools::ymd(ms), s.rem_euclid(86_400) / 3600, s.rem_euclid(3600) / 60, s.rem_euclid(60))
}

#[cfg(test)]
mod tests {
    use gizai_core::devdocs::{ChangelogEntry, DecisionNote};

    const DAY: i64 = 86_400_000;
    /// 2026-11-21, 10:00 UTC.
    const NOV_21: i64 = 1_795_255_200_000;

    #[test]
    fn the_merged_cards_are_one_release_a_month_newest_first() {
        let card = |id: &str, title: &str, at| ChangelogEntry { task_id: id.into(), identifier: id.into(), title: title.into(), at };
        let got = super::releases(&[card("PEAR-3", "Dark <mode>", NOV_21), card("PEAR-2", "Search", NOV_21 - 2 * DAY),
                                    card("PEAR-1", "Blog", NOV_21 - 30 * DAY)]);
        assert_eq!(got.iter().map(|r| (r.version.as_str(), r.date.as_str(), r.changes.len())).collect::<Vec<_>>(),
                   [("November 2026", "2026-11-21", 2), ("October 2026", "2026-10-22", 1)]);
        let change = &got[0].changes[0];
        assert_eq!((change.kind.as_str(), change.text.as_str()), ("Other", "PEAR-3 Dark <mode>"));
        assert_eq!(change.html, "<span class=\"mono\">PEAR-3</span> Dark &lt;mode&gt;");
        assert!(super::releases(&[]).is_empty());
    }

    #[test]
    fn a_decision_note_without_its_title_twice() {
        let note = |body: &str| DecisionNote { id: "n1".into(), path: "Decisions/Pearl".into(), title: "Pearl".into(), body_md: body.into(), updated_at: NOV_21 };
        let d = super::decision(&note("# Pearl\n\n- 2026-11-20 (PEAR-3): Postgres, not MySQL.\n"));
        assert_eq!((d.id.as_str(), d.title.as_str(), d.date.as_str(), d.status.as_str(), d.file.as_str()),
                   ("n1", "Pearl", "2026-11-21", "accepted", "Decisions/Pearl"));
        assert_eq!(d.html, "<ul>\n<li>2026-11-20 (PEAR-3): Postgres, not MySQL.</li>\n</ul>\n");
        assert!(super::decision(&note("# Pearl harbour\n")).html.contains("<h1"), "another heading stays");
    }

    #[test]
    fn iso_is_utc_to_the_second() {
        assert_eq!(super::iso(1_795_255_200_000), "2026-11-21T10:00:00Z");
        assert_eq!(super::iso(0), "1970-01-01T00:00:00Z");
    }
}
