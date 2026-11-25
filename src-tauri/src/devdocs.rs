//! A project's docs page (the Docs section, served by `docs_server`): its index, yodocs' `index.json`
//! (`gizai_docs::index`), from the project's linked folder and Hersir's database; a file's source; a commit's changed
//! lines.
//! - Reference, API routes and guides: the files git tracks in the linked folder, as they are on disk.
//! - Commits: main's newest, as a card's changes measure from it.
//! - Changelog: the project's merged cards (`gizai_core::devdocs::changelog`), by month.
//! - Decisions: the shared Memory notes about the project's decisions (`gizai_core::devdocs::decisions`).
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use gizai_agents::worktree;
use gizai_core::devdocs::{self, Baseline, ChangelogEntry, DecisionNote, DecisionRecord};
use gizai_core::model::Project;
use gizai_docs::index::{self, Change, CommitPatches, Decision, DocChanges, Index, Input, Problem, Release, Section, Since, SourceFile};
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
    let mut input = Input { title: format!("{} dev docs", project.name), version: env!("CARGO_PKG_VERSION").into(),
        off: project.docs_off.clone(), ..Input::default() };
    let mut changelog_file = None;
    let linked = crate::changes::linked_repo(project);
    match &linked {
        Ok(repo) => {
            let repo = repo.as_path();
            let unreadable = |e| format!("Hersir can't read {}'s repository: {}", project.name, crate::pulls::plain(e));
            let files = worktree::tracked_files(repo).map_err(unreadable)?;
            input.reference = gizai_docs::reference(repo, &files);
            input.routes = gizai_docs::api::api(repo, &files);
            let content = gizai_docs::content::read(repo, &files);
            (input.guides, input.changelog, input.decisions, input.mcp, input.api, input.problems) =
                (content.guides, content.changelog, content.decisions, content.mcp, content.api, content.problems);
            changelog_file = content.changelog_file;
            let (label, main) = crate::changes::main_of(project, repo);
            if let Some(main) = main {
                input.history = index::history(&worktree::log_name_status(repo, &main, HISTORY).map_err(unreadable)?);
            }
            input.commit = input.history.first().map(|c| c.hash.clone()).unwrap_or_default();
            input.branch = label;
        }
        Err(why) => input.problems.push(Problem { file: String::new(), message: why.clone(), line: None }),
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
    // the project's own records and the Chief's as one log, the highest number first; then the Memory notes
    input.decisions.extend(devdocs::decision_records(&st.db, &project.id).map_err(|e| e.to_string())?.iter().map(record));
    input.decisions.sort_by_key(|d| std::cmp::Reverse(d.number.parse::<u64>().unwrap_or(0)));
    input.decisions.extend(devdocs::decisions(&st.db, project).map_err(|e| e.to_string())?.iter().map(decision));
    input.built_at = iso(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64));
    input.duration_ms = started.elapsed().as_millis() as u64;
    let mut index = index::build(input);
    if let Ok(repo) = &linked {
        index.doc_changes = doc_changes(st, project, repo, &index);
    }
    Ok(index)
}

/// The project with id `id`, while its Docs is on: the Docs page reads nothing of a project with Docs off.
fn docs_project(st: &AppState, id: &str) -> Result<Project, String> {
    let project = gizai_core::projects::get(&st.db, id).map_err(|e| e.to_string())?;
    if project.dev_docs { Ok(project) } else { Err(format!("Docs is off for {}.", project.name)) }
}

/// The Docs page in Hersir's window (src/docs): the project's index, as the docs server gives a browser index.json.
#[tauri::command]
pub async fn docs_index(st: tauri::State<'_, AppState>, project_id: String) -> Result<Index, String> {
    let st = st.inner().clone();
    tauri::async_runtime::spawn_blocking(move || index_of(&st, &docs_project(&st, &project_id)?)).await.map_err(|e| e.to_string())?
}

/// A file's lines for the Docs page in Hersir's window; none for a file the Reference doesn't read.
#[tauri::command]
pub async fn docs_source(st: tauri::State<'_, AppState>, project_id: String, path: String) -> Result<SourceFile, String> {
    let st = st.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let project = docs_project(&st, &project_id)?;
        Ok(source_of(&project, &path).unwrap_or(SourceFile { path, lines: vec![] }))
    }).await.map_err(|e| e.to_string())?
}

/// A commit's changed lines for the Docs page in Hersir's window.
#[tauri::command]
pub async fn docs_commit(st: tauri::State<'_, AppState>, project_id: String, hash: String) -> Result<CommitPatches, String> {
    let st = st.inner().clone();
    tauri::async_runtime::spawn_blocking(move || patches_of(&docs_project(&st, &project_id)?, &hash)).await.map_err(|e| e.to_string())?
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

/// The merged cards, newest first, as one release a month ("November 2026"), dated by its newest card: each with the line
/// its builder wrote, else its title as Other.
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
            let line = card.text.as_deref().unwrap_or(&card.title);
            release.changes.push(Change {
                kind: card.kind.clone().unwrap_or_else(|| "Other".into()),
                text: format!("{} {line}", card.identifier),
                html: format!("<span class=\"mono\">{}</span> {}", markdown::escape(&card.identifier), markdown::escape(line)),
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

/// A decision record the Chief keeps, as the page shows one: its sections, then the card it was made for and the record
/// it replaces under Related.
fn record(d: &DecisionRecord) -> Decision {
    let mut parts: Vec<(&str, String)> = [("Context", &d.context), ("Decision", &d.decision), ("Consequences", &d.consequences),
        ("Alternatives considered", &d.alternatives)].into_iter().filter(|(_, text)| !text.trim().is_empty())
        .map(|(heading, text)| (heading, text.trim().to_string())).collect();
    let related: Vec<String> = d.task.iter().map(|t| format!("- Made for {t}")).chain(d.supersedes.map(|n| format!("- Replaces {n:04}"))).collect();
    if !related.is_empty() {
        parts.push(("Related", related.join("\n")));
    }
    let md: String = parts.iter().map(|(heading, text)| format!("## {heading}\n\n{text}\n\n")).collect();
    let rendered = markdown::render(&md);
    Decision {
        id: format!("decisions/{:04}", d.number),
        number: format!("{:04}", d.number),
        title: d.title.clone(),
        date: crate::tools::ymd(d.created_at),
        status: d.status.clone(),
        sections: parts.iter().map(|(heading, text)| Section { heading: heading.to_string(), html: markdown::render(text).html }).collect(),
        related_files: d.related_files.clone(),
        file: format!("by {} in Hersir", d.decided_by),
        html: rendered.html,
        headings: rendered.headings,
    }
}

/// "Changed in the docs" (yodocs' manifest): what moved in the declarations since the linked folder's last commit,
/// against the baseline taken then. A new commit takes a new baseline, so the list starts again at every commit. None
/// without a commit, or with the Reference off.
fn doc_changes(st: &AppState, project: &Project, repo: &std::path::Path, index: &Index) -> Option<DocChanges> {
    if project.docs_off.iter().any(|s| s == "reference") {
        return None;
    }
    let head = gizai_agents::changes::commit_of(repo, "HEAD")?;
    let now = index::fingerprints(&index.entries);
    let since = |commit: &str, taken_at: i64| Since { commit: commit.chars().take(7).collect(), taken_at: iso(taken_at) };
    match devdocs::baseline(&st.db, &project.id).ok().flatten() {
        Some(b) if b.commit == head => Some(index::compare(&serde_json::from_str(&b.entries).ok()?, &now, since(&b.commit, b.taken_at))),
        _ => {
            let taken_at = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
            let entries = serde_json::to_string(&now).ok()?;
            devdocs::set_baseline(&st.db, &project.id, &Baseline { commit: head.clone(), taken_at, entries }).ok()?;
            Some(index::compare(&now, &now, since(&head, taken_at)))
        }
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
        let card = |id: &str, title: &str, at| ChangelogEntry { task_id: id.into(), identifier: id.into(), title: title.into(), at, kind: None, text: None };
        let got = super::releases(&[card("PEAR-3", "Dark <mode>", NOV_21), card("PEAR-2", "Search", NOV_21 - 2 * DAY),
                                    card("PEAR-1", "Blog", NOV_21 - 30 * DAY)]);
        assert_eq!(got.iter().map(|r| (r.version.as_str(), r.date.as_str(), r.changes.len())).collect::<Vec<_>>(),
                   [("November 2026", "2026-11-21", 2), ("October 2026", "2026-10-22", 1)]);
        let change = &got[0].changes[0];
        assert_eq!((change.kind.as_str(), change.text.as_str()), ("Other", "PEAR-3 Dark <mode>"));
        assert_eq!(change.html, "<span class=\"mono\">PEAR-3</span> Dark &lt;mode&gt;");
        assert!(super::releases(&[]).is_empty());
        let typed = ChangelogEntry { kind: Some("Fixed".into()), text: Some("Search finds <names>".into()), ..card("PEAR-4", "Search", NOV_21) };
        let change = &super::releases(&[typed])[0].changes[0];
        assert_eq!((change.kind.as_str(), change.text.as_str()), ("Fixed", "PEAR-4 Search finds <names>"), "the builder's line, not the title");
        assert_eq!(change.html, "<span class=\"mono\">PEAR-4</span> Search finds &lt;names&gt;");
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
