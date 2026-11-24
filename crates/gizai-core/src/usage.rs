//! AI usage (the Usage page, GA-33): the tokens and API cost of the agents' runs and chat turns, in total, per day, per
//! agent and per project, summed in SQL over `runs`. Nothing here changes a budget: a run's cost still rolls up into its
//! agent's budget only (`runs::agent_spend_since`); the project totals are only shown.
//!
//! - The cost is the CLI's own estimate at API prices (Claude Code's `total_cost_usd`), not a bill: on a subscription it is
//!   not what gets paid.
//! - Input tokens include cache reads and writes: the stream parsers add them up, so `cache_read_tokens` and
//!   `cache_write_tokens` stay empty.
//! - A run with tokens but no cost has an unknown cost, not $0: Codex, Gemini and Other CLIs report no cost (their runs
//!   save 0), while Claude Code always reports one, and tokens always cost something.
//! - Days and months are UTC, like the agents' monthly budgets (`runs::month_start_ms`).
use std::collections::{HashMap, HashSet};
use std::ops::AddAssign;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::model::Run;
use crate::{Error, Result};

const DAY_MS: i64 = 86_400_000;

/// The most days `Usage::days` holds (a period of 30 days needs 30, a month 31).
pub const MAX_DAYS: i64 = 400;

/// The periods of the Usage page's switch: today, the last 7 or 30 days (today included), this month.
pub const PERIODS: [&str; 4] = ["today", "7d", "30d", "month"];

/// SQL that is true for a run of `runs` (aliased `alias`) whose cost is unknown: tokens but no cost.
pub(crate) fn unknown_cost(alias: &str) -> String {
    format!("(COALESCE({alias}.cost_usd_micros, 0) = 0 AND COALESCE({alias}.input_tokens, 0) + COALESCE({alias}.output_tokens, 0) > 0)")
}

/// What a set of runs used. A chat turn of the Chief is a run too (`chat_turns` counts them).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageTotals {
    /// Runs, chat turns included.
    pub runs: i64,
    /// Of `runs`, the chat turns.
    pub chat_turns: i64,
    /// Input tokens, cache reads and writes included.
    pub input_tokens: i64,
    pub output_tokens: i64,
    /// The API cost of the runs that reported one: what their tokens would cost at API prices, not a bill.
    pub cost_usd_micros: i64,
    /// Runs with tokens but no cost (a CLI that reports none): their cost is unknown, and `cost_usd_micros` leaves it out.
    pub unknown_cost_runs: i64,
}

impl AddAssign for UsageTotals {
    fn add_assign(&mut self, o: UsageTotals) {
        self.runs += o.runs;
        self.chat_turns += o.chat_turns;
        self.input_tokens += o.input_tokens;
        self.output_tokens += o.output_tokens;
        self.cost_usd_micros += o.cost_usd_micros;
        self.unknown_cost_runs += o.unknown_cost_runs;
    }
}

impl std::iter::Sum for UsageTotals {
    fn sum<I: Iterator<Item = UsageTotals>>(iter: I) -> UsageTotals {
        iter.fold(UsageTotals::default(), |mut a, b| { a += b; a })
    }
}

/// One UTC day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageDay {
    /// Midnight UTC (Unix ms).
    pub day_start: i64,
    pub totals: UsageTotals,
}

/// One agent, its chat turns and board checks included.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentUsage {
    pub agent_id: String,
    pub name: String,
    /// Its role on the team (lead, frontend, …); for an agent on no team, the role its runs acted in.
    pub role_key: Option<String>,
    pub totals: UsageTotals,
}

/// One project: the runs on its cards (archived cards included).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUsage {
    pub project_id: String,
    pub number: String,
    pub key: String,
    pub name: String,
    pub color: Option<String>,
    pub totals: UsageTotals,
}

/// The Usage page for one period. The agents add up to `total`, and so do the projects with `chat` and `no_project`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// The runs created from `since` up to (not including) `until`, Unix ms.
    pub since: i64,
    pub until: i64,
    /// Everything: the runs and chat turns of all agents.
    pub total: UsageTotals,
    /// Per day from `since` (24 hours each, so UTC days when `since` is a UTC midnight), oldest first; days without runs
    /// are zero. At most `MAX_DAYS`.
    pub days: Vec<UsageDay>,
    /// Per agent with runs, the highest cost first.
    pub agents: Vec<AgentUsage>,
    /// Per project with runs, the highest cost first.
    pub projects: Vec<ProjectUsage>,
    /// Runs without a card, so without a project: the Chief's chat turns and board checks.
    pub chat: UsageTotals,
    /// Runs on cards without a project.
    pub no_project: UsageTotals,
}

/// The start and end (exclusive) of a period of the switch at `now`, in whole UTC days up to the end of today: `today`;
/// `7d` and `30d`, today and the 6 or 29 days before it; `month`, from the 1st of this month.
pub fn period_range(period: &str, now: i64) -> Result<(i64, i64)> {
    let today = now - now.rem_euclid(DAY_MS);
    let since = match period {
        "today" => today,
        "7d" => today - 6 * DAY_MS,
        "30d" => today - 29 * DAY_MS,
        "month" => crate::runs::month_start_ms(now),
        _ => return Err(Error::Invalid(format!("unknown period {period}: use today, 7d, 30d or month"))),
    };
    Ok((since, today + DAY_MS))
}

/// The Usage page for a period of the switch (`PERIODS`) at `now`.
pub fn for_period(db: &Db, period: &str, now: i64) -> Result<Usage> {
    let (since, until) = period_range(period, now)?;
    summary(db, since, until)
}

/// What the runs created from `since` up to (not including) `until` used: in total, per day, per agent and per project.
pub fn summary(db: &Db, since: i64, until: i64) -> Result<Usage> {
    if until <= since {
        return Err(Error::Invalid("the period ends before it starts".into()));
    }
    let unknown = unknown_cost("r");
    let sums = format!("count(*), COALESCE(SUM(r.trigger = 'chat'), 0), COALESCE(SUM(r.input_tokens), 0), COALESCE(SUM(r.output_tokens), 0),
                        COALESCE(SUM(r.cost_usd_micros), 0), COALESCE(SUM({unknown}), 0)");
    let period = "r.created_at >= ?1 AND r.created_at < ?2";
    let totals = |r: &rusqlite::Row, at: usize| -> rusqlite::Result<UsageTotals> {
        Ok(UsageTotals { runs: r.get(at)?, chat_turns: r.get(at + 1)?, input_tokens: r.get(at + 2)?, output_tokens: r.get(at + 3)?,
                         cost_usd_micros: r.get(at + 4)?, unknown_cost_runs: r.get(at + 5)? })
    };
    db.read(|c| {
        let total = c.query_row(&format!("SELECT {sums} FROM runs r WHERE {period}"), params![since, until], |r| totals(r, 0))?;

        let n_days = ((until as i128 - since as i128 + DAY_MS as i128 - 1) / DAY_MS as i128).min(MAX_DAYS as i128) as i64;
        let mut days: Vec<UsageDay> = (0..n_days).map(|i| UsageDay { day_start: since + i * DAY_MS, totals: UsageTotals::default() }).collect();
        let mut st = c.prepare(&format!("SELECT (r.created_at - ?1) / {DAY_MS}, {sums} FROM runs r WHERE {period} GROUP BY 1"))?;
        for row in st.query_map(params![since, until], |r| Ok((r.get::<_, i64>(0)?, totals(r, 1)?)))? {
            let (day, t) = row?;
            if let Some(d) = usize::try_from(day).ok().and_then(|i| days.get_mut(i)) {
                d.totals = t;
            }
        }

        let mut st = c.prepare(&format!(
            "SELECT r.agent_actor_id, COALESCE(a.name, 'A removed agent'),
                    COALESCE((SELECT m.role_key FROM team_members m WHERE m.actor_id = r.agent_actor_id ORDER BY m.is_lead DESC LIMIT 1), MAX(r.role_key)),
                    {sums}
             FROM runs r LEFT JOIN actors a ON a.id = r.agent_actor_id
             WHERE {period} GROUP BY r.agent_actor_id"))?;
        let mut agents = st.query_map(params![since, until], |r| Ok(AgentUsage {
            agent_id: r.get(0)?, name: r.get(1)?, role_key: r.get(2)?, totals: totals(r, 3)?,
        }))?.collect::<rusqlite::Result<Vec<_>>>()?;
        agents.sort_by(|a, b| most_first(&a.totals, &b.totals).then_with(|| a.name.cmp(&b.name)));

        // Grouped by card or none, then by project: runs without a card are the chat line, runs on a card without a
        // project the no-project line.
        let mut st = c.prepare(&format!(
            "SELECT r.task_id IS NULL, t.project_id, COALESCE(p.number, ''), COALESCE(p.key, ''), COALESCE(p.name, 'A removed project'), p.color,
                    {sums}
             FROM runs r LEFT JOIN tasks t ON t.id = r.task_id LEFT JOIN projects p ON p.id = t.project_id
             WHERE {period} GROUP BY r.task_id IS NULL, t.project_id"))?;
        let (mut projects, mut chat, mut no_project) = (vec![], UsageTotals::default(), UsageTotals::default());
        let rows = st.query_map(params![since, until], |r| Ok((r.get::<_, bool>(0)?, r.get::<_, Option<String>>(1)?, ProjectUsage {
            project_id: String::new(), number: r.get(2)?, key: r.get(3)?, name: r.get(4)?, color: r.get(5)?, totals: totals(r, 6)?,
        })))?;
        for row in rows {
            match row? {
                (true, _, p) => chat += p.totals,
                (false, None, p) => no_project += p.totals,
                (false, Some(id), p) => projects.push(ProjectUsage { project_id: id, ..p }),
            }
        }
        projects.sort_by(|a, b| most_first(&a.totals, &b.totals).then_with(|| a.name.cmp(&b.name)));

        Ok(Usage { since, until, total, days, agents, projects, chat, no_project })
    })
}

/// One card on a project's Time tracking tab (GA-331): how long the agents worked on it and the tokens that took, added
/// up over all of its runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardTime {
    pub task_id: String,
    pub identifier: String,
    pub title: String,
    /// The name of the card's board column.
    pub column: String,
    pub archived: bool,
    /// Runs that started.
    pub runs: i64,
    /// The ended runs' time; an interrupted run counts at most the run time limit.
    pub time_ms: i64,
    /// `started_at` of the run working on the card now.
    pub running_since: Option<i64>,
    /// Cache reads and writes included.
    pub input_tokens: i64,
    pub output_tokens: i64,
    /// The runs' API cost that is known (GA-354): an estimate at API prices, not a bill.
    pub cost_usd_micros: i64,
    /// Runs with tokens but no cost (`unknown_cost`): their cost is unknown, and `cost_usd_micros` leaves it out.
    pub unknown_cost_runs: i64,
}

/// One UTC day of a project's Time tracking tab (GA-354): what the runs that started on it added up to, by the rules of
/// `card_times` (a run counts on the day it started, its whole time included).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeDay {
    /// Midnight UTC (Unix ms).
    pub day_start: i64,
    /// Runs that started on it.
    pub runs: i64,
    /// Those runs' time once they ended; an interrupted run counts at most the run time limit.
    pub time_ms: i64,
    /// Cache reads and writes included.
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd_micros: i64,
    pub unknown_cost_runs: i64,
}

/// A project's Time tracking tab for a period (GA-354): its cards and its UTC days.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTime {
    /// The period, Unix ms: the runs that started from `since` up to (not including) `until`. Both None: all of them.
    pub since: Option<i64>,
    pub until: Option<i64>,
    /// When the project's first run started (over all time, whatever the period), None before its first run.
    pub first_run: Option<i64>,
    /// All time: every card of the project. In a period: only the cards with a run that started in it, with their sums for
    /// that period. Newest card first.
    pub cards: Vec<CardTime>,
    /// All time: from the UTC day of `first_run` to today. In a period: every day of it. Oldest first; days without runs
    /// are 0. At most `MAX_TIME_DAYS`.
    pub days: Vec<TimeDay>,
}

/// The most days `ProjectTime::days` holds: ten years. A longer period is `Error::Invalid`; all time keeps its newest days.
pub const MAX_TIME_DAYS: i64 = 3_660;

/// The error of a run Hersir found working when it started again (`runs::recover_interrupted`): it ended then, at the
/// next start-up, not when it stopped.
const INTERRUPTED: &str = "interrupted";

const MINUTE_MS: i64 = 60_000;

/// The project's Time tracking tab (GA-331): one row per card of the project (open, done, cancelled and archived), with
/// what every agent's runs on it added up to (builders, QA, DevOps and the Chief's), newest card first, by the rules of
/// `card_times` (GA-332: the Chief's tools count with them too, so the tab and the tools agree).
///
/// An unknown project is `Error::NotFound`.
pub fn project_time(db: &Db, project_id: &str, max_run_ms: i64) -> Result<Vec<CardTime>> {
    db.read(|c| {
        let ids = project_cards(c, project_id)?;
        let mut times = card_times_in(c, &ids, max_run_ms, None)?;
        Ok(ids.iter().filter_map(|id| times.remove(id)).collect())
    })
}

/// The project's Time tracking tab for a period (GA-354): its cards and UTC days, by the rules of `card_times`, for the runs
/// that started from `since` up to (not including) `until`, Unix ms, or for all of them with neither. A run counts in a
/// period when it started in it, and on the UTC day it started. `project_time` is this tab for all time, as a list.
///
/// Give both `since` and `until` or neither; a period that ends before it starts, or is longer than `MAX_TIME_DAYS`, is
/// `Error::Invalid`. An unknown project is `Error::NotFound`.
pub fn project_time_period(db: &Db, project_id: &str, max_run_ms: i64, since: Option<i64>, until: Option<i64>) -> Result<ProjectTime> {
    project_time_at(db, project_id, max_run_ms, since, until, crate::ids::now_ms())
}

/// `project_time_period` at `now`: all time's days end with the UTC day of `now`.
pub fn project_time_at(db: &Db, project_id: &str, max_run_ms: i64, since: Option<i64>, until: Option<i64>, now: i64) -> Result<ProjectTime> {
    let period = match (since, until) {
        (None, None) => None,
        (Some(s), Some(u)) if u <= s => return Err(Error::Invalid("the period ends before it starts".into())),
        (Some(s), Some(u)) if (u as i128 - s as i128) > (MAX_TIME_DAYS as i128) * DAY_MS as i128 =>
            return Err(Error::Invalid(format!("a period of at most {MAX_TIME_DAYS} days"))),
        (Some(s), Some(u)) => Some((s, u)),
        _ => return Err(Error::Invalid("give both since and until, or neither".into())),
    };
    db.read(|c| {
        let ids = project_cards(c, project_id)?;
        let mut times = card_times_in(c, &ids, max_run_ms, period)?;
        let cards: Vec<CardTime> = ids.iter().filter_map(|id| times.remove(id)).filter(|t| period.is_none() || t.runs > 0).collect();
        let first_run: Option<i64> = c.query_row(
            "SELECT MIN(r.started_at) FROM runs r JOIN tasks t ON t.id = r.task_id
             WHERE t.project_id = ?1 AND r.deleted_at IS NULL AND r.started_at IS NOT NULL",
            [project_id], |r| r.get(0))?;
        // the days: the period's, or all time's from the day of the first run to today (or the newest run's day, if later)
        let (start, end) = match (period, first_run) {
            (Some(p), _) => p,
            (None, None) => (0, 0),
            (None, Some(first)) => {
                let last: i64 = c.query_row(
                    "SELECT COALESCE(MAX(r.started_at), 0) FROM runs r JOIN tasks t ON t.id = r.task_id
                     WHERE t.project_id = ?1 AND r.deleted_at IS NULL AND r.started_at IS NOT NULL",
                    [project_id], |r| r.get(0))?;
                let end = utc_day(now.max(last)) + DAY_MS;
                (utc_day(first).max(end - MAX_TIME_DAYS * DAY_MS), end)
            }
        };
        let n_days = ((end as i128 - start as i128 + DAY_MS as i128 - 1) / DAY_MS as i128).clamp(0, MAX_TIME_DAYS as i128) as i64;
        let mut days: Vec<TimeDay> = (0..n_days).map(|i| TimeDay { day_start: start + i * DAY_MS, ..TimeDay::default() }).collect();
        if n_days > 0 {
            let span = "MAX(r.ended_at - r.started_at, 0)";
            let unknown = unknown_cost("r");
            let mut st = c.prepare_cached(&format!(
                "SELECT (r.started_at - ?2) / {DAY_MS}, count(*),
                        COALESCE(SUM(CASE WHEN r.ended_at IS NOT NULL THEN
                            CASE WHEN r.error = ?5 THEN MIN({span}, ?4) ELSE {span} END END), 0),
                        COALESCE(SUM(r.input_tokens), 0), COALESCE(SUM(r.output_tokens), 0),
                        COALESCE(SUM(r.cost_usd_micros), 0), COALESCE(SUM({unknown}), 0)
                 FROM runs r JOIN tasks t ON t.id = r.task_id
                 WHERE t.project_id = ?1 AND r.deleted_at IS NULL AND r.started_at >= ?2 AND r.started_at < ?3
                 GROUP BY 1"))?;
            let rows = st.query_map(params![project_id, start, end, max_run_ms.max(0), INTERRUPTED], |r| Ok((r.get::<_, i64>(0)?, TimeDay {
                day_start: 0, runs: r.get(1)?, time_ms: r.get(2)?, input_tokens: r.get(3)?, output_tokens: r.get(4)?,
                cost_usd_micros: r.get(5)?, unknown_cost_runs: r.get(6)?,
            })))?;
            for row in rows {
                let (i, d) = row?;
                if let Some(day) = usize::try_from(i).ok().and_then(|i| days.get_mut(i)) {
                    *day = TimeDay { day_start: day.day_start, ..d };
                }
            }
        }
        Ok(ProjectTime { since: period.map(|p| p.0), until: period.map(|p| p.1), first_run, cards, days })
    })
}

/// Midnight UTC of the day `ms` falls on.
fn utc_day(ms: i64) -> i64 {
    ms - ms.rem_euclid(DAY_MS)
}

/// Each card's agent time and tokens, added up over all of its runs, keyed by task id (GA-332): the one place for the
/// rules of the project's Time tracking tab (`project_time`) and of the Chief's get_task, list_tasks and get_project. One
/// query over `task_ids`, archived cards included; an id given twice counts once, and an unknown one isn't in the map.
/// A run removed (`deleted_at`) is left out, as in `runs::list_for_task`; the Chief's chat turns and board checks have
/// no card, so they never count.
///
/// - `runs`: the runs that started; a queued run that never did isn't one.
/// - `time_ms`: each ended run's wall-clock span, from its start to its end (waiting for an approval included), never
///   below 0. An interrupted run (`runs::recover_interrupted`) ended at the next start-up, not when it stopped, so it
///   counts at most `max_run_ms`, the run time limit as it is set now.
/// - `running_since`: the earliest start of a run that started and hasn't ended.
/// - Tokens: the sums; a running run's are saved when it ends.
pub fn card_times(db: &Db, task_ids: &[String], max_run_ms: i64) -> Result<HashMap<String, CardTime>> {
    db.read(|c| card_times_in(c, task_ids, max_run_ms, None))
}

/// `card_times` over the runs that started in `period` (from its start up to, not including, its end), or over all of
/// them with None (GA-354): a run that never started is in no period.
fn card_times_in(c: &Connection, task_ids: &[String], max_run_ms: i64, period: Option<(i64, i64)>) -> Result<HashMap<String, CardTime>> {
    let mut out = HashMap::with_capacity(task_ids.len());
    if task_ids.is_empty() {
        return Ok(out);
    }
    let span = "MAX(r.ended_at - r.started_at, 0)";
    let ended = "r.started_at IS NOT NULL AND r.ended_at IS NOT NULL";
    let unknown = unknown_cost("r");
    // DISTINCT: a card given twice would join its runs twice and count them double
    let mut st = c.prepare_cached(&format!(
        "SELECT t.id, t.identifier, t.title, COALESCE(s.name, ''), t.deleted_at IS NOT NULL,
                count(r.started_at),
                COALESCE(SUM(CASE WHEN {ended} THEN
                    CASE WHEN r.error = ?3 THEN MIN({span}, ?2) ELSE {span} END END), 0),
                MIN(CASE WHEN r.started_at IS NOT NULL AND r.ended_at IS NULL THEN r.started_at END),
                COALESCE(SUM(r.input_tokens), 0), COALESCE(SUM(r.output_tokens), 0),
                COALESCE(SUM(r.cost_usd_micros), 0), COALESCE(SUM({unknown}), 0)
         FROM (SELECT DISTINCT value AS id FROM json_each(?1)) j
         JOIN tasks t ON t.id = j.id
         LEFT JOIN workflow_states s ON s.id = t.state_id
         LEFT JOIN runs r ON r.task_id = t.id AND r.deleted_at IS NULL
                         AND (?4 IS NULL OR r.started_at >= ?4) AND (?5 IS NULL OR r.started_at < ?5)
         GROUP BY t.id"))?;
    let (since, until) = (period.map(|p| p.0), period.map(|p| p.1));
    let rows = st.query_map(params![serde_json::to_string(task_ids)?, max_run_ms.max(0), INTERRUPTED, since, until], |r| Ok(CardTime {
        task_id: r.get(0)?, identifier: r.get(1)?, title: r.get(2)?, column: r.get(3)?, archived: r.get(4)?,
        runs: r.get(5)?, time_ms: r.get(6)?, running_since: r.get(7)?, input_tokens: r.get(8)?, output_tokens: r.get(9)?,
        cost_usd_micros: r.get(10)?, unknown_cost_runs: r.get(11)?,
    }))?;
    for row in rows {
        let card = row?;
        out.insert(card.task_id.clone(), card);
    }
    Ok(out)
}

/// The ids of a project's cards (open, done, cancelled and archived), newest first: the rows of its Time tracking tab.
/// An unknown project is `Error::NotFound`.
fn project_cards(c: &Connection, project_id: &str) -> Result<Vec<String>> {
    c.query_row("SELECT 1 FROM projects WHERE id = ?1", [project_id], |_| Ok(()))
        .optional()?
        .ok_or_else(|| Error::NotFound(format!("project {project_id}")))?;
    let mut st = c.prepare("SELECT id FROM tasks WHERE project_id = ?1 ORDER BY created_at DESC, identifier DESC")?;
    Ok(st.query_map([project_id], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?)
}

/// A card's agent time at `now`, in ms: its ended runs' time, plus the time of the run working on it now (the Time
/// tracking tab's `agentTimeMs`).
pub fn agent_ms(c: &CardTime, now: i64) -> i64 {
    c.time_ms.saturating_add(c.running_since.map_or(0, |since| now.saturating_sub(since).max(0)))
}

/// A card's agent minutes at `now`, as the Chief's tools give them (GA-332): `agent_ms` in whole minutes, rounded.
pub fn agent_minutes(c: &CardTime, now: i64) -> i64 {
    whole_minutes(agent_ms(c, now))
}

/// One run's agent time at `now`, in ms, by `card_times`' rules for that run alone (GA-332): None for a run that never
/// started; from its start up to `now` while it works (it started and hasn't ended); once it ended, its span, never below
/// 0, and at most `max_run_ms` for an interrupted run.
pub fn run_ms(r: &Run, max_run_ms: i64, now: i64) -> Option<i64> {
    let start = r.started_at?;
    Some(match r.ended_at {
        None => now.saturating_sub(start).max(0),
        Some(end) if r.error.as_deref() == Some(INTERRUPTED) => end.saturating_sub(start).max(0).min(max_run_ms.max(0)),
        Some(end) => end.saturating_sub(start).max(0),
    })
}

/// `ms` in whole minutes, rounded: 29 seconds are 0 minutes, 30 seconds 1, 89 seconds 1 and 90 seconds 2.
pub fn whole_minutes(ms: i64) -> i64 {
    ms.max(0).saturating_add(MINUTE_MS / 2) / MINUTE_MS
}

/// The nearest rank of `sorted` (low to high) at `percent`: the value at rank ceil(percent / 100 × its length), counted
/// from 1 (50 is the median); None when it is empty. Of 10, 20, 30 and 40, 50 gives 20 and 90 gives 40.
pub fn nearest_rank(sorted: &[i64], percent: u32) -> Option<i64> {
    let rank = (sorted.len() * percent.min(100) as usize).div_ceil(100).max(1);
    sorted.get(rank - 1).copied()
}

/// get_project's time_tracking (GA-332): a project's Time tracking tab added up at `now`, over all its cards (archived
/// ones included), and the agent minutes its cards in Done took, to estimate new cards from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TimeTracking {
    /// Its cards with a run that started.
    pub cards_with_runs: i64,
    /// The runs that started, on all its cards.
    pub runs: i64,
    /// All its cards' agent time at `now` (`agent_ms`) added up, then in whole minutes, rounded.
    pub agent_minutes: i64,
    /// Cache reads and writes included.
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub done_cards: DoneCards,
}

/// A project's cards in a done column (category `done`; archived ones included, cancelled ones left out) with a run that
/// started, and the nearest rank (`nearest_rank`) of their agent minutes (`agent_minutes`) at 50 and 90 percent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DoneCards {
    pub count: i64,
    /// None when `count` is 0.
    pub median_minutes: Option<i64>,
    /// None when `count` is 0.
    pub p90_minutes: Option<i64>,
}

/// A project's `TimeTracking` at `now`, from its Time tracking tab's rows (`project_time`). An unknown project is
/// `Error::NotFound`.
pub fn time_tracking(db: &Db, project_id: &str, max_run_ms: i64, now: i64) -> Result<TimeTracking> {
    db.read(|c| {
        let ids = project_cards(c, project_id)?;
        let times = card_times_in(c, &ids, max_run_ms, None)?;
        let done: HashSet<String> = c.prepare(
            "SELECT t.id FROM tasks t JOIN workflow_states s ON s.id = t.state_id WHERE t.project_id = ?1 AND s.category = 'done'")?
            .query_map([project_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let (mut tt, mut ms, mut done_minutes) = (TimeTracking::default(), 0i64, Vec::new());
        for card in times.values() {
            tt.runs += card.runs;
            tt.input_tokens += card.input_tokens;
            tt.output_tokens += card.output_tokens;
            ms = ms.saturating_add(agent_ms(card, now));
            if card.runs > 0 {
                tt.cards_with_runs += 1;
                if done.contains(&card.task_id) {
                    done_minutes.push(agent_minutes(card, now));
                }
            }
        }
        tt.agent_minutes = whole_minutes(ms);
        done_minutes.sort_unstable();
        tt.done_cards = DoneCards {
            count: done_minutes.len() as i64,
            median_minutes: nearest_rank(&done_minutes, 50),
            p90_minutes: nearest_rank(&done_minutes, 90),
        };
        Ok(tt)
    })
}

/// The highest cost first, then the most tokens.
fn most_first(a: &UsageTotals, b: &UsageTotals) -> std::cmp::Ordering {
    b.cost_usd_micros.cmp(&a.cost_usd_micros).then_with(|| (b.input_tokens + b.output_tokens).cmp(&(a.input_tokens + a.output_tokens)))
}
