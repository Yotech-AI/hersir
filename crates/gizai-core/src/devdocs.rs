//! Docs, the sidebar's section of a project's developer docs (switched on per project): its Changelog, the project's
//! merged cards newest first; and its Decisions, the shared memory notes about the project's decisions. Docs → Commits
//! reads git, so it is in the app (`changes`).
use serde::Serialize;

use crate::db::Db;
use crate::memory;
use crate::model::Project;
use crate::Result;

/// How many merged cards Docs → Changelog lists.
pub const CHANGELOG_LIMIT: i64 = 200;

/// A merged card, as Docs → Changelog lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogEntry {
    pub task_id: String,
    pub identifier: String,
    pub title: String,
    /// When it reached Done, else when it last changed (a card in a Deploy column), in Unix milliseconds.
    pub at: i64,
}

/// The project's merged cards (in a Deploy or Done column, archived ones too), newest first: by when they reached Done,
/// else when they last changed. At most `CHANGELOG_LIMIT`. Archiving is the only way a card gets `deleted_at`, and only a
/// card in Done is archived, so a card with it was merged too.
pub fn changelog(db: &Db, project_id: &str) -> Result<Vec<ChangelogEntry>> {
    db.read(|c| {
        let mut st = c.prepare(
            "SELECT t.id, t.identifier, t.title, COALESCE(t.completed_at, t.updated_at) AS at FROM tasks t
               JOIN workflow_states s ON s.id = t.state_id
              WHERE t.project_id = ?1 AND s.category IN ('deploy', 'done')
              ORDER BY at DESC, t.rowid DESC LIMIT ?2")?;
        let rows = st.query_map(rusqlite::params![project_id, CHANGELOG_LIMIT], |r| {
            Ok(ChangelogEntry { task_id: r.get(0)?, identifier: r.get(1)?, title: r.get(2)?, at: r.get(3)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })
}

/// A decision note, as Docs → Decisions shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionNote {
    pub id: String,
    /// Folder and title, like `Decisions/Kade portal`.
    pub path: String,
    pub title: String,
    /// Its text without its properties (Markdown).
    pub body_md: String,
    pub updated_at: i64,
}

/// The shared memory notes about `project`'s decisions, newest first: in `Decisions/`, or of `type: decision`, with the
/// project's key or name (any case) as `project:`. An agent's own notes and the Chief's aren't shared, so never these.
pub fn decisions(db: &Db, project: &Project) -> Result<Vec<DecisionNote>> {
    let notes: Vec<(String, String, String, i64)> = db.read(|c| {
        let mut st = c.prepare("SELECT id, path, body_md, updated_at FROM docs WHERE kind = 'memory' AND deleted_at IS NULL
                                ORDER BY updated_at DESC, rowid DESC")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })?;
    let about = |v: &String| v.eq_ignore_ascii_case(&project.key) || v.eq_ignore_ascii_case(&project.name);
    Ok(notes.into_iter().filter_map(|(id, path, body, updated_at)| {
        let folder = path.split('/').next().and_then(memory::folder_type)?;
        let props = memory::properties(&body);
        let decision = folder == "decision" || props.get("type").is_some_and(|t| t.iter().any(|v| v.eq_ignore_ascii_case("decision")));
        let ours = props.get("project").is_some_and(|p| p.iter().any(about));
        (decision && ours).then(|| DecisionNote {
            title: memory::title_of(&path).to_string(), body_md: without_properties(&body).to_string(), id, path, updated_at,
        })
    }).collect())
}

/// `body` without the YAML properties at its start (between `---` lines) and the blank lines after them.
fn without_properties(body: &str) -> &str {
    let Some(rest) = body.strip_prefix("---\n") else { return body };
    let after = match rest.strip_prefix("---\n") {
        Some(after) => after,
        None => match rest.find("\n---\n") {
            Some(end) => &rest[end + 5..],
            None => return body,
        },
    };
    after.trim_start_matches('\n')
}
