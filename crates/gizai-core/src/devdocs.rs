//! Docs, the sidebar's section of a project's developer docs (switched on per project), what Hersir keeps for it: the
//! Changelog, the project's merged cards newest first, each with the line its builder wrote (`set_change`); the
//! Decisions, the records the Chief keeps (`record_decision`) and the shared memory notes about the project's
//! decisions; and the baseline "Changed in the docs" compares with. What the page reads from the project's files is
//! in `gizai_docs`.
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::db::Db;
use crate::model::Project;
use crate::{ids, memory, Error, Result};

/// The sections of a project's Docs page a person can switch off in the project form, as the page names them.
pub const SECTIONS: [&str; 6] = ["reference", "guides", "api", "changelog", "decisions", "mcp"];

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
    /// The line its builder wrote (`set_change`): its kind (Added, Fixed…) and text. None: it has none.
    pub kind: Option<String>,
    pub text: Option<String>,
}

/// The project's merged cards (in a Deploy or Done column, archived ones too), newest first: by when they reached Done,
/// else when they last changed. At most `CHANGELOG_LIMIT`. Archiving is the only way a card gets `deleted_at`, and only a
/// card in Done is archived, so a card with it was merged too.
pub fn changelog(db: &Db, project_id: &str) -> Result<Vec<ChangelogEntry>> {
    db.read(|c| {
        let mut st = c.prepare(
            "SELECT t.id, t.identifier, t.title, COALESCE(t.completed_at, t.updated_at) AS at, d.kind, d.text FROM tasks t
               JOIN workflow_states s ON s.id = t.state_id
               LEFT JOIN docs_changes d ON d.task_id = t.id
              WHERE t.project_id = ?1 AND s.category IN ('deploy', 'done')
              ORDER BY at DESC, t.rowid DESC LIMIT ?2")?;
        let rows = st.query_map(rusqlite::params![project_id, CHANGELOG_LIMIT], |r| {
            Ok(ChangelogEntry { task_id: r.get(0)?, identifier: r.get(1)?, title: r.get(2)?, at: r.get(3)?, kind: r.get(4)?, text: r.get(5)? })
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

/// The kinds of change Keep a Changelog names: a card's changelog line is one of them.
pub const CHANGE_KINDS: [&str; 6] = ["Added", "Changed", "Deprecated", "Removed", "Fixed", "Security"];
/// A changelog line is cut to this many characters.
pub const CHANGE_MAX: usize = 300;

/// Sets `task_id`'s line in its project's changelog, as the agent that built it wrote it: its kind (`CHANGE_KINDS`, in
/// any case) and one plain line, cut to `CHANGE_MAX`. A newer line for the card replaces the older one. It shows on the
/// Docs page once the card is merged.
pub fn set_change(db: &Db, actor: Option<&str>, task_id: &str, kind: &str, text: &str) -> Result<()> {
    let kind = CHANGE_KINDS.iter().find(|k| k.eq_ignore_ascii_case(kind.trim()))
        .ok_or_else(|| Error::Invalid(format!("{kind:?} isn't a kind of change: {}", CHANGE_KINDS.join(", "))))?;
    let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(CHANGE_MAX).collect();
    if text.is_empty() {
        return Err(Error::Invalid("a changelog line needs its text".into()));
    }
    db.write(actor, |w| {
        let n = w.conn().execute(
            "INSERT INTO docs_changes(task_id, kind, text, updated_at) SELECT id, ?2, ?3, ?4 FROM tasks WHERE id = ?1
             ON CONFLICT(task_id) DO UPDATE SET kind = excluded.kind, text = excluded.text, updated_at = excluded.updated_at",
            rusqlite::params![task_id, kind, text, ids::now_ms()])?;
        if n == 0 {
            return Err(Error::NotFound(format!("card {task_id}")));
        }
        w.update("docs_changes", task_id, json!({ "kind": kind, "text": text }))
    })
}

/// What a decision record can be: proposed, accepted, rejected, superseded or deprecated, as ADRs name them.
pub const STATUSES: [&str; 5] = ["proposed", "accepted", "rejected", "superseded", "deprecated"];

/// A decision record the Chief keeps for a project, numbered per project like an ADR.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionRecord {
    pub id: String,
    pub number: i64,
    pub title: String,
    pub status: String,
    pub context: String,
    pub decision: String,
    pub consequences: String,
    pub alternatives: String,
    pub related_files: Vec<String>,
    /// The card it was made for, by its identifier (KADE-12).
    pub task: Option<String>,
    /// The number of the record it replaces.
    pub supersedes: Option<i64>,
    /// Who made it: the Chief's name, or a person's.
    pub decided_by: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A decision to record: a title and the decision, and what led to it, follows from it and was weighed against it.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct DecisionInput {
    pub title: String,
    pub decision: String,
    pub context: String,
    pub consequences: String,
    pub alternatives: String,
    /// proposed or accepted (the default); a decision is recorded when it is made or put forward.
    pub status: Option<String>,
    pub related_files: Vec<String>,
    pub task_id: Option<String>,
    pub supersedes: Option<i64>,
}

/// Records a decision about `project_id`, numbered after both Hersir's records for it and `after` (the highest number
/// in the project's own decision records), so the two read as one log. A record it supersedes becomes superseded.
pub fn record_decision(db: &Db, actor: &str, decided_by: &str, project_id: &str, after: i64, input: DecisionInput) -> Result<DecisionRecord> {
    let title = input.title.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() || input.decision.trim().is_empty() {
        return Err(Error::Invalid("a decision needs its title and the decision itself".into()));
    }
    let status = match input.status.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => "accepted".to_string(),
        Some(s) if ["proposed", "accepted"].iter().any(|k| k.eq_ignore_ascii_case(s)) => s.to_lowercase(),
        Some(s) => return Err(Error::Invalid(format!("{s:?}: a decision is recorded as proposed or accepted"))),
    };
    let id = ids::new_id();
    let now = ids::now_ms();
    db.write(Some(actor), |w| {
        let c = w.conn();
        if c.query_row("SELECT count(*) FROM projects WHERE id = ?1", [project_id], |r| r.get::<_, i64>(0))? == 0 {
            return Err(Error::NotFound(format!("project {project_id}")));
        }
        let ours: i64 = c.query_row("SELECT COALESCE(MAX(number), 0) FROM docs_decisions WHERE project_id = ?1", [project_id], |r| r.get(0))?;
        let number = ours.max(after) + 1;
        if let Some(old) = input.supersedes {
            c.execute("UPDATE docs_decisions SET status = 'superseded', updated_at = ?3 WHERE project_id = ?1 AND number = ?2",
                rusqlite::params![project_id, old, now])?;
        }
        c.execute(
            "INSERT INTO docs_decisions(id, project_id, number, title, status, context, decision, consequences, alternatives,
               related_files, task_id, supersedes, decided_by, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, (SELECT id FROM tasks WHERE id = ?11), ?12, ?13, ?14, ?14)",
            rusqlite::params![id, project_id, number, title, status, input.context.trim(), input.decision.trim(), input.consequences.trim(),
                input.alternatives.trim(), serde_json::to_string(&input.related_files)?, input.task_id, input.supersedes, decided_by, now])?;
        w.insert("docs_decisions", &id, json!({ "projectId": project_id, "number": number, "title": title, "status": status }))?;
        Ok(())
    })?;
    decision_records(db, project_id)?.into_iter().find(|d| d.id == id).ok_or_else(|| Error::NotFound(format!("decision {id}")))
}

/// The decision records Hersir keeps for `project_id`, the highest number first.
pub fn decision_records(db: &Db, project_id: &str) -> Result<Vec<DecisionRecord>> {
    db.read(|c| {
        let mut st = c.prepare(
            "SELECT d.id, d.number, d.title, d.status, d.context, d.decision, d.consequences, d.alternatives, d.related_files,
                    t.identifier, d.supersedes, d.decided_by, d.created_at, d.updated_at
               FROM docs_decisions d LEFT JOIN tasks t ON t.id = d.task_id
              WHERE d.project_id = ?1 ORDER BY d.number DESC")?;
        let rows = st.query_map([project_id], |r| Ok(DecisionRecord {
            id: r.get(0)?, number: r.get(1)?, title: r.get(2)?, status: r.get(3)?, context: r.get(4)?, decision: r.get(5)?,
            consequences: r.get(6)?, alternatives: r.get(7)?,
            related_files: serde_json::from_str(&r.get::<_, String>(8)?).unwrap_or_default(),
            task: r.get(9)?, supersedes: r.get(10)?, decided_by: r.get(11)?, created_at: r.get(12)?, updated_at: r.get(13)?,
        }))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })
}

/// A project's declarations as they were at a commit, for "Changed in the docs": `entries` is `gizai_docs`'s
/// snapshot (JSON), kept as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Baseline {
    pub commit: String,
    pub taken_at: i64,
    pub entries: String,
}

/// `project_id`'s baseline, if one was taken.
pub fn baseline(db: &Db, project_id: &str) -> Result<Option<Baseline>> {
    db.read(|c| {
        let mut st = c.prepare("SELECT commit_id, taken_at, entries FROM docs_baselines WHERE project_id = ?1")?;
        let mut rows = st.query_map([project_id], |r| Ok(Baseline { commit: r.get(0)?, taken_at: r.get(1)?, entries: r.get(2)? }))?;
        Ok(rows.next().transpose()?)
    })
}

/// Keeps `baseline` as `project_id`'s, in place of the one before. Hersir's own cache: not in the change log.
pub fn set_baseline(db: &Db, project_id: &str, baseline: &Baseline) -> Result<()> {
    db.write(None, |w| {
        w.conn().execute(
            "INSERT INTO docs_baselines(project_id, commit_id, taken_at, entries) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(project_id) DO UPDATE SET commit_id = excluded.commit_id, taken_at = excluded.taken_at, entries = excluded.entries",
            rusqlite::params![project_id, baseline.commit, baseline.taken_at, baseline.entries])?;
        Ok(())
    })
}
