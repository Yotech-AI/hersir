// GA-354: a project's Time tracking tab for a period (usage::project_time_period / project_time_at). A run counts in a
// period when it started in it (`started_at` in [since, until)), on the UTC day it started, with the rules of GA-331 (an
// interrupted run's cap, a queued run never counts). Without a period: every card, and the days from the first run's day
// to today. With one: only the cards with a run that started in it, and every day of the period. Each card and day has
// its API cost and its runs without one (a Codex run). card_times and project_time (the Chief's tools) stay all-time.
use gizai_core::model::*;
use gizai_core::usage::{self, CardTime, ProjectTime, TimeDay};
use gizai_core::{Error, db::Db, ids, projects, runs, seed, tasks, team};

const MIN: i64 = 60_000;
const HOUR: i64 = 60 * MIN;
const DAY: i64 = 24 * HOUR;
/// The run time limit in these tests: the default, 90 minutes.
const LIMIT: i64 = 90 * MIN;
/// 2026-10-09 00:00 UTC, a Friday: the first day of the periods here.
const D0: i64 = 1_791_504_000_000;

struct W {
    db: Db,
    you: String,
    kade: String,
    mobi: String,
    backend: String,
}

fn world() -> W {
    let db = Db::open_in_memory().unwrap();
    let s = seed::ensure_seed(&db, "Jeffrey").unwrap();
    let kade = projects::create(&db, &s.you_id, ProjectInput { name: "Kade".into(), key: "KADE".into(), ..Default::default() }).unwrap();
    let mobi = projects::create(&db, &s.you_id, ProjectInput { name: "Mobi".into(), key: "MOBI".into(), ..Default::default() }).unwrap();
    let agent = |name: &str, role: &str| {
        team::add_agent(&db, &s.you_id, &s.team_id, AgentInput { name: name.into(), role_key: role.into(), ..Default::default() }).unwrap()
    };
    let backend = agent("Backend Agent", "backend");
    W { you: s.you_id.clone(), db, kade, mobi, backend }
}

impl W {
    fn state(&self, name: &str) -> String {
        self.db.read(|c| Ok(c.query_row("SELECT id FROM workflow_states WHERE name=?1", [name], |r| r.get(0))?)).unwrap()
    }

    fn card_in(&self, project: &str, title: &str) -> String {
        tasks::create(&self.db, &self.you, TaskInput { project_id: project.into(), title: title.into(), state_id: Some(self.state("In progress")),
            ..Default::default() }).unwrap()
    }

    fn card(&self, title: &str) -> String { self.card_in(&self.kade, title) }

    fn sql(&self, sql: &str, p: impl rusqlite::Params) {
        self.db.write(None, |w| { w.conn().execute(sql, p)?; Ok(()) }).unwrap();
    }

    fn queued(&self, task: &str) -> String {
        let r = runs::create(&self.db, &self.backend, task, "backend", &ids::new_id(), "/tmp", "/tmp", "b", "/tmp/l").unwrap();
        self.sql("UPDATE tasks SET claimed_by_run_id=NULL, lease_expires_at=NULL WHERE id=?1", [task]);
        r
    }

    fn running(&self, task: &str, start: i64) -> String {
        let r = self.queued(task);
        runs::set_running(&self.db, &r, 4242).unwrap();
        self.sql("UPDATE runs SET started_at=?2 WHERE id=?1", rusqlite::params![r, start]);
        r
    }

    /// A run on `task` from `start` to `end`, with these tokens and API cost (0: the CLI reported none), ended as `status`.
    #[allow(clippy::too_many_arguments)]
    fn run_as(&self, task: &str, start: i64, end: i64, input: i64, output: i64, cost: i64, status: &str, error: Option<&str>) -> String {
        let r = self.queued(task);
        runs::set_running(&self.db, &r, 4242).unwrap();
        runs::finish(&self.db, &r, status, None, cost, input, output, error).unwrap();
        self.sql("UPDATE runs SET started_at=?2, ended_at=?3 WHERE id=?1", rusqlite::params![r, start, end]);
        r
    }

    /// A run that succeeded from `start` to `end`, with these tokens and API cost.
    fn run(&self, task: &str, start: i64, end: i64, input: i64, output: i64, cost: i64) -> String {
        self.run_as(task, start, end, input, output, cost, "succeeded", None)
    }

    fn period(&self, project: &str, since: i64, until: i64) -> ProjectTime {
        usage::project_time_at(&self.db, project, LIMIT, Some(since), Some(until), until + 30 * DAY).unwrap()
    }

    fn all(&self, project: &str, now: i64) -> ProjectTime {
        usage::project_time_at(&self.db, project, LIMIT, None, None, now).unwrap()
    }
}

fn card<'a>(t: &'a ProjectTime, id: &str) -> &'a CardTime {
    t.cards.iter().find(|c| c.task_id == id).unwrap_or_else(|| panic!("card {id} isn't in the period"))
}

fn ids_of(t: &ProjectTime) -> Vec<&str> { t.cards.iter().map(|c| c.task_id.as_str()).collect() }

/// A day's runs, time, input and output tokens, cost and runs without a cost.
fn sums(d: &TimeDay) -> (i64, i64, i64, i64, i64, i64) {
    (d.runs, d.time_ms, d.input_tokens, d.output_tokens, d.cost_usd_micros, d.unknown_cost_runs)
}

#[test]
fn a_run_started_1_ms_before_the_period_isnt_in_it_and_one_started_on_its_last_day_is() {
    let w = world();
    let t = w.card("Edges");
    let (since, until) = (D0, D0 + 3 * DAY);
    // 1 ms before: not in it, though it ends well inside it
    w.run(&t, since - 1, since + HOUR, 1, 1, 1);
    // its first ms: in it, on its first day
    w.run(&t, since, since + 10 * MIN, 10, 1, 100);
    // its last ms, on its last day (it ends after the period): in it, on its last day, all its time
    w.run(&t, until - 1, until + 20 * MIN, 20, 2, 200);
    // the period's end: not in it
    w.run(&t, until, until + HOUR, 1, 1, 1);
    let p = w.period(&w.kade, since, until);
    assert_eq!((p.since, p.until), (Some(since), Some(until)));
    let c = card(&p, &t);
    assert_eq!((c.runs, c.time_ms, c.input_tokens, c.output_tokens, c.cost_usd_micros, c.unknown_cost_runs),
        (2, 30 * MIN + 1, 30, 3, 300, 0));
    assert_eq!(p.days.iter().map(|d| d.day_start).collect::<Vec<_>>(), [D0, D0 + DAY, D0 + 2 * DAY]);
    assert_eq!(sums(&p.days[0]), (1, 10 * MIN, 10, 1, 100, 0));
    assert_eq!(sums(&p.days[1]), (0, 0, 0, 0, 0, 0));
    assert_eq!(sums(&p.days[2]), (1, 20 * MIN + 1, 20, 2, 200, 0));
}

#[test]
fn a_run_counts_on_the_utc_day_it_started_with_its_whole_time() {
    let w = world();
    let t = w.card("Past midnight");
    // 23:30 to 01:30 UTC: two hours, all on the day it started
    w.run(&t, D0 + DAY - 30 * MIN, D0 + DAY + 90 * MIN, 500, 50, 5_000);
    let p = w.period(&w.kade, D0, D0 + 2 * DAY);
    assert_eq!(sums(&p.days[0]), (1, 2 * HOUR, 500, 50, 5_000, 0));
    assert_eq!(sums(&p.days[1]), (0, 0, 0, 0, 0, 0));
    // a period of only the next day hasn't got it
    let next = w.period(&w.kade, D0 + DAY, D0 + 2 * DAY);
    assert!(next.cards.is_empty(), "{:?}", next.cards);
    assert_eq!(next.days.len(), 1);
    assert_eq!(sums(&next.days[0]), (0, 0, 0, 0, 0, 0));
}

#[test]
fn in_a_period_only_the_cards_with_a_run_in_it_with_that_periods_sums_and_all_time_every_card() {
    let w = world();
    let both = w.card("Runs in and out");
    let before = w.card("Runs before only");
    let none = w.card("No runs");
    w.run(&both, D0 - 5 * DAY, D0 - 5 * DAY + HOUR, 1_000, 100, 10_000);
    w.run(&both, D0 + HOUR, D0 + HOUR + 15 * MIN, 200, 20, 2_000);
    w.run(&before, D0 - 2 * DAY, D0 - 2 * DAY + 40 * MIN, 300, 30, 3_000);

    let p = w.period(&w.kade, D0, D0 + 7 * DAY);
    assert_eq!(ids_of(&p), [both.as_str()]);
    let c = card(&p, &both);
    assert_eq!((c.runs, c.time_ms, c.input_tokens, c.output_tokens, c.cost_usd_micros), (1, 15 * MIN, 200, 20, 2_000));
    assert_eq!(p.days.len(), 7);
    assert_eq!(p.first_run, Some(D0 - 5 * DAY), "the first run over all time, whatever the period");

    // All: every card of the project, newest first, with all their runs, as GA-331's list
    let a = w.all(&w.kade, D0 + 2 * DAY);
    assert_eq!((a.since, a.until), (None, None));
    assert_eq!(ids_of(&a), [none.as_str(), before.as_str(), both.as_str()]);
    let c = card(&a, &both);
    assert_eq!((c.runs, c.time_ms, c.input_tokens, c.output_tokens, c.cost_usd_micros), (2, 75 * MIN, 1_200, 120, 12_000));
    let n = card(&a, &none);
    assert_eq!((n.runs, n.time_ms, n.cost_usd_micros, n.unknown_cost_runs), (0, 0, 0, 0));
    assert_eq!(a.cards, usage::project_time(&w.db, &w.kade, LIMIT).unwrap(), "All is GA-331's list");
}

#[test]
fn a_period_without_runs_has_no_cards_and_every_day_at_zero() {
    let w = world();
    let t = w.card("Old work");
    w.run(&t, D0 - 40 * DAY, D0 - 40 * DAY + HOUR, 10, 1, 100);
    let p = w.period(&w.kade, D0, D0 + 31 * DAY);
    assert!(p.cards.is_empty());
    assert_eq!(p.days.len(), 31);
    assert!(p.days.iter().all(|d| sums(d) == (0, 0, 0, 0, 0, 0)));
    assert!(p.days.windows(2).all(|d| d[1].day_start - d[0].day_start == DAY));
}

#[test]
fn a_run_with_tokens_but_no_cost_is_unknown_and_the_known_cost_leaves_it_out() {
    let w = world();
    let t = w.card("Claude and Codex");
    // a Claude run with its cost, a Codex run with tokens and no cost, and a run that failed before it used any
    w.run(&t, D0 + HOUR, D0 + 2 * HOUR, 4_000, 400, 120_000);
    w.run(&t, D0 + 3 * HOUR, D0 + 4 * HOUR, 9_000, 900, 0);
    w.run_as(&t, D0 + DAY, D0 + DAY + MIN, 0, 0, 0, "failed", Some("exit 1"));
    let p = w.period(&w.kade, D0, D0 + 2 * DAY);
    let c = card(&p, &t);
    assert_eq!((c.runs, c.cost_usd_micros, c.unknown_cost_runs), (3, 120_000, 1));
    assert_eq!(sums(&p.days[0]), (2, 2 * HOUR, 13_000, 1_300, 120_000, 1));
    assert_eq!(sums(&p.days[1]), (1, MIN, 0, 0, 0, 0), "no tokens, no cost: nothing unknown");
    let a = w.all(&w.kade, D0 + DAY);
    assert_eq!((card(&a, &t).cost_usd_micros, card(&a, &t).unknown_cost_runs), (120_000, 1));
}

#[test]
fn all_times_days_run_from_the_first_runs_utc_day_to_today() {
    let w = world();
    let t = w.card("Over a week");
    let first = D0 + 22 * HOUR;
    w.run(&t, first, first + HOUR, 10, 1, 100);
    w.run(&t, D0 + 3 * DAY + HOUR, D0 + 3 * DAY + 2 * HOUR, 20, 2, 200);
    let now = D0 + 6 * DAY + 13 * HOUR;
    let a = w.all(&w.kade, now);
    assert_eq!(a.first_run, Some(first));
    assert_eq!(a.days.len(), 7);
    assert_eq!(a.days.first().unwrap().day_start, D0);
    assert_eq!(a.days.last().unwrap().day_start, D0 + 6 * DAY, "today, UTC");
    assert_eq!(sums(&a.days[0]), (1, HOUR, 10, 1, 100, 0));
    assert_eq!(sums(&a.days[3]), (1, HOUR, 20, 2, 200, 0));
    assert_eq!(a.days.iter().map(|d| d.runs).sum::<i64>(), 2);
}

#[test]
fn a_project_without_runs_has_every_card_no_first_run_and_no_days() {
    let w = world();
    let t = w.card("Not started");
    w.queued(&t);
    let a = w.all(&w.kade, D0);
    assert_eq!(ids_of(&a), [t.as_str()]);
    assert_eq!((a.first_run, a.days.len()), (None, 0));
    // a project without cards
    let m = w.all(&w.mobi, D0);
    assert_eq!((m.cards.len(), m.first_run, m.days.len()), (0, None, 0));
}

#[test]
fn the_interrupted_cap_a_running_run_and_a_queued_run_follow_ga_331s_rules_in_a_period() {
    let w = world();
    let t = w.card("Rules");
    // interrupted: closed for a night, at most the limit
    w.run_as(&t, D0 + HOUR, D0 + 15 * HOUR, 0, 0, 0, "failed", Some("interrupted"));
    // timed out after two hours: its full span
    w.run_as(&t, D0 + DAY, D0 + DAY + 2 * HOUR, 0, 0, 0, "timed_out", None);
    // a queued run never counts, also when Hersir ends it at start-up
    w.queued(&t);
    assert_eq!(runs::recover_interrupted(&w.db).unwrap(), 1);
    // working now: a run, its start, no time yet
    let u = w.card("Working");
    w.running(&u, D0 + 2 * DAY + 5 * HOUR);
    let p = w.period(&w.kade, D0, D0 + 3 * DAY);
    let c = card(&p, &t);
    assert_eq!((c.runs, c.time_ms, c.running_since), (2, LIMIT + 2 * HOUR, None));
    let r = card(&p, &u);
    assert_eq!((r.runs, r.time_ms, r.running_since), (1, 0, Some(D0 + 2 * DAY + 5 * HOUR)));
    assert_eq!(p.days.iter().map(|d| (d.runs, d.time_ms)).collect::<Vec<_>>(), [(1, LIMIT), (1, 2 * HOUR), (1, 0)]);
    // a run working now that started before the period isn't in it
    let q = w.period(&w.kade, D0 + 3 * DAY, D0 + 4 * DAY);
    assert!(q.cards.is_empty(), "{:?}", q.cards);
}

#[test]
fn removed_runs_and_another_projects_runs_are_left_out_of_the_cards_and_the_days() {
    let w = world();
    let k = w.card("Kade card");
    let m = w.card_in(&w.mobi, "Mobi card");
    w.run(&k, D0 + HOUR, D0 + 2 * HOUR, 10, 1, 100);
    let gone = w.run(&k, D0 + 3 * HOUR, D0 + 9 * HOUR, 900, 90, 9_000);
    w.sql("UPDATE runs SET deleted_at=?2 WHERE id=?1", rusqlite::params![gone, D0 + 10 * HOUR]);
    w.run(&m, D0 + HOUR, D0 + 5 * HOUR, 500, 50, 5_000);
    let p = w.period(&w.kade, D0, D0 + DAY);
    assert_eq!(ids_of(&p), [k.as_str()]);
    assert_eq!(sums(&p.days[0]), (1, HOUR, 10, 1, 100, 0));
    let mobi = w.period(&w.mobi, D0, D0 + DAY);
    assert_eq!(ids_of(&mobi), [m.as_str()]);
    assert_eq!(sums(&mobi.days[0]), (1, 4 * HOUR, 500, 50, 5_000, 0));
}

#[test]
fn an_archived_card_with_a_run_in_the_period_is_in_it() {
    let w = world();
    let t = w.card("Archived one");
    w.run(&t, D0 + HOUR, D0 + 2 * HOUR, 10, 1, 100);
    tasks::move_to(&w.db, &w.you, &t, &w.state("Done"), "zz").unwrap();
    tasks::archive(&w.db, &w.you, &t).unwrap();
    let p = w.period(&w.kade, D0, D0 + DAY);
    assert_eq!(ids_of(&p), [t.as_str()]);
    assert!(card(&p, &t).archived);
}

#[test]
fn the_days_add_up_to_the_cards() {
    let w = world();
    let a = w.card("A");
    let b = w.card("B");
    for i in 0..10 {
        w.run(&a, D0 + i * 7 * HOUR, D0 + i * 7 * HOUR + (i + 1) * MIN, 100 * i, 10 * i, if i % 3 == 0 { 0 } else { 1_000 * i });
        w.run(&b, D0 + i * 11 * HOUR, D0 + i * 11 * HOUR + 2 * (i + 1) * MIN, 50 * i, 5 * i, 500 * i);
    }
    let p = w.period(&w.kade, D0 + DAY, D0 + 4 * DAY);
    let cards = p.cards.iter().fold((0, 0, 0, 0, 0, 0), |s, c| {
        (s.0 + c.runs, s.1 + c.time_ms, s.2 + c.input_tokens, s.3 + c.output_tokens, s.4 + c.cost_usd_micros, s.5 + c.unknown_cost_runs)
    });
    let days = p.days.iter().fold((0, 0, 0, 0, 0, 0), |s, d| {
        let d = sums(d);
        (s.0 + d.0, s.1 + d.1, s.2 + d.2, s.3 + d.3, s.4 + d.4, s.5 + d.5)
    });
    assert_eq!(days, cards);
    assert!(cards.0 > 0 && cards.0 < 20, "some runs are in the period, not all: {cards:?}");
}

#[test]
fn a_period_needs_both_ends_an_end_after_its_start_and_at_most_ten_years() {
    let w = world();
    let bad = |since: Option<i64>, until: Option<i64>| {
        let r = usage::project_time_at(&w.db, &w.kade, LIMIT, since, until, D0);
        assert!(matches!(r, Err(Error::Invalid(_))), "{since:?}..{until:?}: expected Invalid, got {r:?}");
    };
    bad(Some(D0), None);
    bad(None, Some(D0));
    bad(Some(D0), Some(D0));
    bad(Some(D0 + DAY), Some(D0));
    bad(Some(D0), Some(D0 + (usage::MAX_TIME_DAYS + 1) * DAY));
    // ten years is the most
    let ten = usage::project_time_at(&w.db, &w.kade, LIMIT, Some(D0), Some(D0 + usage::MAX_TIME_DAYS * DAY), D0).unwrap();
    assert_eq!(ten.days.len() as i64, usage::MAX_TIME_DAYS);
    let r = usage::project_time_period(&w.db, "no-such-project", LIMIT, None, None);
    assert!(matches!(r, Err(Error::NotFound(_))), "expected NotFound, got {r:?}");
}

#[test]
fn project_time_period_without_a_period_ends_its_days_today() {
    let w = world();
    let t = w.card("Yesterday");
    let today = ids::now_ms() / DAY * DAY;
    w.run(&t, today - DAY + HOUR, today - DAY + 2 * HOUR, 10, 1, 100);
    let a = usage::project_time_period(&w.db, &w.kade, LIMIT, None, None).unwrap();
    let last = a.days.last().unwrap().day_start;
    // the test may cross midnight UTC
    assert!(last == today || last == today + DAY, "last day {last}, today {today}");
    assert_eq!(a.days.first().unwrap().day_start, today - DAY);
}

#[test]
fn the_chiefs_numbers_stay_all_time() {
    let w = world();
    let t = w.card("Old and new");
    w.run(&t, D0 - 10 * DAY, D0 - 10 * DAY + HOUR, 1_000, 100, 10_000);
    w.run(&t, D0 + HOUR, D0 + HOUR + 30 * MIN, 200, 20, 2_000);
    let p = w.period(&w.kade, D0, D0 + DAY);
    assert_eq!(card(&p, &t).runs, 1);
    let all = usage::card_times(&w.db, std::slice::from_ref(&t), LIMIT).unwrap();
    let c = &all[&t];
    assert_eq!((c.runs, c.time_ms, c.input_tokens, c.cost_usd_micros), (2, 90 * MIN, 1_200, 12_000));
    let list = usage::project_time(&w.db, &w.kade, LIMIT).unwrap();
    assert_eq!((list[0].runs, list[0].time_ms), (2, 90 * MIN));
}

#[test]
fn it_says_project_time_and_its_days_in_camel_case_as_the_ui_reads_them() {
    let d = TimeDay { day_start: 1, runs: 2, time_ms: 3, input_tokens: 4, output_tokens: 5, cost_usd_micros: 6, unknown_cost_runs: 7 };
    let p = ProjectTime { since: None, until: Some(9), first_run: Some(8), cards: vec![], days: vec![d] };
    assert_eq!(serde_json::to_value(&p).unwrap(), serde_json::json!({"since": null, "until": 9, "firstRun": 8, "cards": [],
        "days": [{"dayStart": 1, "runs": 2, "timeMs": 3, "inputTokens": 4, "outputTokens": 5, "costUsdMicros": 6, "unknownCostRuns": 7}]}));
}
