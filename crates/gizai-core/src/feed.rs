//! GA-344: the change feed. Every write goes through `Db::write`, whoever makes it (a tool, a Tauri command, a run or
//! gizai-core itself), and logs its changes in `changes`; the change counter is the `seq` of the newest one
//! (`Db::change_seq`). A view of the board that can't be told about a change asks again and again instead (Claude
//! Desktop's board widget, through the board_changes tool): with the counter of its last answer it gets the cards that
//! changed since, and the counter to ask with next.
//!
//! A card changed when its own row did (its column, fields, labels, hold, builds_on, pull request, archive), or a row
//! that belongs to it: a comment, a run (its start, end, outcome and what it recorded) or a file on it or on one of its
//! comments; and a card that builds on a card whose own row changed (its `waits_for` may have). Writes that change no
//! card log nothing for it: settings, tokens, an agent's heartbeat, chat sessions and queues, a board check's own notes.
use crate::db::Db;
use crate::Result;
use rusqlite::Connection;
use serde::Serialize;

/// At most this many cards in one answer: when more changed since, the answer says `reload` instead.
pub const MAX_CARDS: usize = 200;

/// The tables whose changes change what every card on the board shows, or the board itself: its columns (and the agents
/// on them), labels, agents and people (names, paused), projects and teams.
pub const BOARD_TABLES: [&str; 6] = ["workflow_states", "labels", "actors", "projects", "teams", "team_members"];

/// What changed after a counter (`since`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Changes {
    /// The change counter now: the `since` to ask with next.
    pub counter: i64,
    /// The cards that changed, by identifier (GA-12), the one changed last first. Empty with `reload`.
    pub tasks: Vec<String>,
    /// The board's columns, labels, agents, people, projects or teams changed too.
    pub board: bool,
    /// Load the whole board again: `since` is past the counter (a restored backup starts it lower) or more than
    /// `MAX_CARDS` cards changed.
    pub reload: bool,
}

/// What changed after `since`, a counter an earlier answer gave; None: only the counter now, to start from. With the
/// latest counter, no cards and no work: the counter in memory says nothing changed.
pub fn since(db: &Db, since: Option<i64>) -> Result<Changes> {
    let now = db.change_seq();
    let Some(since) = since else { return Ok(Changes { counter: now, tasks: vec![], board: false, reload: false }) };
    if since == now {
        return Ok(Changes { counter: now, tasks: vec![], board: false, reload: false });
    }
    db.read(|c| {
        // the counter as this read sees the table: the rows read are exactly those up to it
        let counter: i64 = c.query_row("SELECT COALESCE(max(seq), 0) FROM changes", [], |r| r.get(0))?;
        if since > counter {
            return Ok(Changes { counter, tasks: vec![], board: false, reload: true });
        }
        let since = since.max(0);
        let mut tasks = cards(c, since, counter)?;
        let reload = tasks.len() > MAX_CARDS;
        if reload {
            tasks.clear();
        }
        Ok(Changes { counter, tasks, board: board(c, since, counter)?, reload })
    })
}

/// The identifiers of the cards with a change after `since` up to `until`, the one changed last first: at most
/// `MAX_CARDS` + 1, so the caller can tell there were more.
fn cards(c: &Connection, since: i64, until: i64) -> Result<Vec<String>> {
    let mut st = c.prepare_cached(
        "WITH ch AS (SELECT seq, table_name, row_id FROM changes WHERE seq > ?1 AND seq <= ?2),
         changed(task_id, seq) AS (
             SELECT row_id, seq FROM ch WHERE table_name = 'tasks'
             UNION ALL SELECT m.task_id, ch.seq FROM ch JOIN comments m ON m.id = ch.row_id WHERE ch.table_name = 'comments'
             UNION ALL SELECT r.task_id, ch.seq FROM ch JOIN runs r ON r.id = ch.row_id
                       WHERE ch.table_name = 'runs' AND r.task_id IS NOT NULL
             UNION ALL SELECT f.owner_id, ch.seq FROM ch JOIN files f ON f.id = ch.row_id
                       WHERE ch.table_name = 'files' AND f.owner_type = 'task'
             UNION ALL SELECT m.task_id, ch.seq FROM ch JOIN files f ON f.id = ch.row_id JOIN comments m ON m.id = f.owner_id
                       WHERE ch.table_name = 'files' AND f.owner_type = 'comment'
             UNION ALL SELECT b.task_id, ch.seq FROM ch JOIN task_builds_on b ON b.builds_on_id = ch.row_id
                       WHERE ch.table_name = 'tasks'
         )
         SELECT t.identifier FROM changed x JOIN tasks t ON t.id = x.task_id
         GROUP BY t.id ORDER BY max(x.seq) DESC LIMIT ?3",
    )?;
    let rows = st.query_map(rusqlite::params![since, until, MAX_CARDS as i64 + 1], |r| r.get(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<String>>>()?)
}

/// Whether a table of `BOARD_TABLES` has a change after `since` up to `until`.
fn board(c: &Connection, since: i64, until: i64) -> Result<bool> {
    let tables = BOARD_TABLES.iter().map(|t| format!("'{t}'")).collect::<Vec<_>>().join(", ");
    let sql = format!("SELECT EXISTS (SELECT 1 FROM changes WHERE seq > ?1 AND seq <= ?2 AND table_name IN ({tables}))");
    Ok(c.prepare_cached(&sql)?.query_row(rusqlite::params![since, until], |r| r.get(0))?)
}
