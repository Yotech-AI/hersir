// GA-331: a project's Time tracking tab (usage::project_time). Per card of the project: the runs that started, the
// ended runs' wall-clock time (an interrupted run at most the run time limit), the run working on it now, and the
// tokens, added up over all of its runs. Archived cards count; another project's cards, the Chief's chat turns and
// board checks don't.
use gizai_core::model::*;
use gizai_core::usage::{self, CardTime};
use gizai_core::{Error, board, chat, db::Db, ids, projects, runs, seed, tasks, team};

const MIN: i64 = 60_000;
const HOUR: i64 = 60 * MIN;
/// The run time limit in these tests: the default, 90 minutes.
const LIMIT: i64 = 90 * MIN;
/// 2026-10-09 10:00 UTC: when the runs here start.
const T0: i64 = 1_791_540_000_000;

struct W {
    db: Db,
    you: String,
    team: String,
    kade: String,
    mobi: String,
    backend: String,
    qa: String,
    lead: String,
}

fn world() -> W {
    let db = Db::open_in_memory().unwrap();
    let s = seed::ensure_seed(&db, "Jeffrey").unwrap();
    let kade = projects::create(&db, &s.you_id, ProjectInput { name: "Kade".into(), key: "KADE".into(), ..Default::default() }).unwrap();
    let mobi = projects::create(&db, &s.you_id, ProjectInput { name: "Mobi".into(), key: "MOBI".into(), ..Default::default() }).unwrap();
    let agent = |name: &str, role: &str, chat: Option<bool>| {
        team::add_agent(&db, &s.you_id, &s.team_id, AgentInput { name: name.into(), role_key: role.into(), chat_enabled: chat, ..Default::default() })
            .unwrap()
    };
    let backend = agent("Backend Agent", "backend", None);
    let qa = agent("QA Agent", "qa", None);
    let lead = agent("Team Lead", "lead", Some(true));
    W { you: s.you_id.clone(), team: s.team_id.clone(), db, kade, mobi, backend, qa, lead }
}

impl W {
    fn state(&self, name: &str) -> String {
        self.db.read(|c| Ok(c.query_row("SELECT id FROM workflow_states WHERE name=?1", [name], |r| r.get(0))?)).unwrap()
    }

    fn card_in(&self, project: &str, column: &str, title: &str) -> String {
        tasks::create(&self.db, &self.you, TaskInput { project_id: project.into(), title: title.into(), state_id: Some(self.state(column)),
            ..Default::default() }).unwrap()
    }

    fn card(&self, title: &str) -> String { self.card_in(&self.kade, "In progress", title) }

    fn sql(&self, sql: &str, p: impl rusqlite::Params) {
        self.db.write(None, |w| { w.conn().execute(sql, p)?; Ok(()) }).unwrap();
    }

    /// A queued run of `agent` on `task` (it never started), as the dispatcher records one.
    fn queued(&self, agent: &str, task: &str) -> String {
        let r = runs::create(&self.db, agent, task, "backend", &ids::new_id(), "/tmp", "/tmp", "b", "/tmp/l").unwrap();
        // let the next run on the card in, as a run's end does
        self.sql("UPDATE tasks SET claimed_by_run_id=NULL, lease_expires_at=NULL WHERE id=?1", [task]);
        r
    }

    /// A run of `agent` on `task` that is working now, started at `start`.
    fn running(&self, agent: &str, task: &str, start: i64) -> String {
        let r = self.queued(agent, task);
        runs::set_running(&self.db, &r, 4242).unwrap();
        self.sql("UPDATE runs SET started_at=?2 WHERE id=?1", rusqlite::params![r, start]);
        r
    }

    /// A run of `agent` on `task` that ran from `start` to `end`, ended as `status` with `error`, with these tokens.
    #[allow(clippy::too_many_arguments)]
    fn ended_as(&self, agent: &str, task: &str, start: i64, end: i64, input: i64, output: i64, status: &str, error: Option<&str>) -> String {
        let r = self.queued(agent, task);
        runs::set_running(&self.db, &r, 4242).unwrap();
        runs::finish(&self.db, &r, status, None, 1_000, input, output, error).unwrap();
        self.sql("UPDATE runs SET started_at=?2, ended_at=?3 WHERE id=?1", rusqlite::params![r, start, end]);
        r
    }

    /// A run that succeeded, from `start` to `end`.
    fn ended(&self, agent: &str, task: &str, start: i64, end: i64, input: i64, output: i64) -> String {
        self.ended_as(agent, task, start, end, input, output, "succeeded", None)
    }

    fn times(&self, project: &str) -> Vec<CardTime> { usage::project_time(&self.db, project, LIMIT).unwrap() }

    fn row(&self, project: &str, task: &str) -> CardTime {
        self.times(project).into_iter().find(|c| c.task_id == task).unwrap_or_else(|| panic!("card {task} isn't on the tab"))
    }
}

#[test]
fn a_card_adds_up_the_time_runs_and_tokens_of_all_its_ended_runs() {
    let w = world();
    let t = w.card("Export invoices");
    // 10 and 20 minutes: a start by the Backend Agent, then QA
    w.ended(&w.backend, &t, T0, T0 + 10 * MIN, 1_000, 100);
    w.ended(&w.qa, &t, T0 + HOUR, T0 + HOUR + 20 * MIN, 2_500, 250);
    let c = w.row(&w.kade, &t);
    let task = tasks::get(&w.db, &t).unwrap();
    assert_eq!(c, CardTime { task_id: t.clone(), identifier: task.identifier, title: "Export invoices".into(), column: "In progress".into(),
        archived: false, runs: 2, time_ms: 30 * MIN, running_since: None, input_tokens: 3_500, output_tokens: 350,
        // GA-354: each run here reports 1_000 micros of API cost
        cost_usd_micros: 2_000, unknown_cost_runs: 0 });
}

#[test]
fn every_agents_run_counts_failed_and_cancelled_ones_too_and_the_chiefs_on_the_card() {
    let w = world();
    let t = w.card("Import orders");
    w.ended_as(&w.backend, &t, T0, T0 + 5 * MIN, 10, 1, "failed", Some("exit 1"));
    w.ended_as(&w.qa, &t, T0 + HOUR, T0 + HOUR + 7 * MIN, 20, 2, "cancelled", None);
    // a Continue (a nudge) is a run of its own
    let n = runs::create_nudge(&w.db, &w.backend, &t, "backend", &ids::new_id(), "/tmp", "/tmp", "b", "/tmp/l").unwrap();
    runs::set_running(&w.db, &n, 1).unwrap();
    runs::finish(&w.db, &n, "succeeded", None, 0, 30, 3, None).unwrap();
    w.sql("UPDATE runs SET started_at=?2, ended_at=?3 WHERE id=?1", rusqlite::params![n, T0 + 2 * HOUR, T0 + 2 * HOUR + 11 * MIN]);
    // the Chief's run on the card, and one that ran out of time: its full span, the limit doesn't cut it
    w.ended(&w.lead, &t, T0 + 3 * HOUR, T0 + 3 * HOUR + 2 * MIN, 40, 4);
    w.ended_as(&w.backend, &t, T0 + 4 * HOUR, T0 + 4 * HOUR + 2 * HOUR, 50, 5, "timed_out", None);
    let c = w.row(&w.kade, &t);
    assert_eq!((c.runs, c.time_ms, c.input_tokens, c.output_tokens), (5, (5 + 7 + 11 + 2 + 120) * MIN, 150, 15));
}

#[test]
fn a_card_without_runs_has_none() {
    let w = world();
    let t = w.card("Nothing yet");
    let c = w.row(&w.kade, &t);
    assert_eq!((c.runs, c.time_ms, c.running_since, c.input_tokens, c.output_tokens), (0, 0, None, 0, 0));
}

#[test]
fn a_running_run_counts_as_a_run_and_gives_its_start_but_no_time_or_tokens_until_it_ends() {
    let w = world();
    let t = w.card("Working");
    w.ended(&w.backend, &t, T0, T0 + 12 * MIN, 100, 10);
    let r = w.running(&w.qa, &t, T0 + HOUR);
    let c = w.row(&w.kade, &t);
    assert_eq!((c.runs, c.time_ms, c.running_since, c.input_tokens, c.output_tokens), (2, 12 * MIN, Some(T0 + HOUR), 100, 10));

    // when it ends, its time and tokens are in, and nothing works on the card any more
    runs::finish(&w.db, &r, "succeeded", None, 0, 300, 30, None).unwrap();
    w.sql("UPDATE runs SET ended_at=?2 WHERE id=?1", rusqlite::params![r, T0 + HOUR + 33 * MIN]);
    let c = w.row(&w.kade, &t);
    assert_eq!((c.runs, c.time_ms, c.running_since, c.input_tokens, c.output_tokens), (2, 45 * MIN, None, 400, 40));
}

#[test]
fn with_two_runs_working_the_earliest_start_is_the_one() {
    let w = world();
    let t = w.card("Two at once");
    w.running(&w.backend, &t, T0 + 30 * MIN);
    w.running(&w.qa, &t, T0);
    let c = w.row(&w.kade, &t);
    assert_eq!((c.runs, c.time_ms, c.running_since), (2, 0, Some(T0)));
}

#[test]
fn a_queued_run_that_never_started_adds_no_run_and_no_time_even_when_it_ends() {
    let w = world();
    let t = w.card("Waiting");
    w.ended(&w.backend, &t, T0, T0 + 3 * MIN, 5, 1);
    w.queued(&w.qa, &t);
    let c = w.row(&w.kade, &t);
    assert_eq!((c.runs, c.time_ms, c.running_since), (1, 3 * MIN, None));
    // a queued run Hersir found at start-up ends as interrupted, with an end but no start: still nothing
    assert_eq!(runs::recover_interrupted(&w.db).unwrap(), 1);
    let c = w.row(&w.kade, &t);
    assert_eq!((c.runs, c.time_ms, c.running_since), (1, 3 * MIN, None));
}

#[test]
fn an_interrupted_run_counts_at_most_the_run_time_limit_and_an_ended_one_its_full_span() {
    let w = world();
    let t = w.card("Interrupted");
    // closed for a night: its recorded end is the next start-up, 14 hours later
    w.ended_as(&w.backend, &t, T0, T0 + 14 * HOUR, 0, 0, "failed", Some("interrupted"));
    assert_eq!(w.row(&w.kade, &t).time_ms, LIMIT);
    // one shorter than the limit counts its span
    let u = w.card("Interrupted soon");
    w.ended_as(&w.backend, &u, T0, T0 + 25 * MIN, 0, 0, "failed", Some("interrupted"));
    assert_eq!(w.row(&w.kade, &u).time_ms, 25 * MIN);
    // a run that failed otherwise, longer than the limit, counts all of it
    let v = w.card("Failed long");
    w.ended_as(&w.backend, &v, T0, T0 + 14 * HOUR, 0, 0, "failed", Some("exit 1"));
    assert_eq!(w.row(&w.kade, &v).time_ms, 14 * HOUR);
    // the limit is the one passed in, as it is set now
    let c = usage::project_time(&w.db, &w.kade, 5 * MIN).unwrap();
    let ms = |id: &str| c.iter().find(|r| r.task_id == id).unwrap().time_ms;
    assert_eq!((ms(&t), ms(&u), ms(&v)), (5 * MIN, 5 * MIN, 14 * HOUR));
}

#[test]
fn a_run_hersir_recovers_at_start_up_counts_at_most_the_limit() {
    let w = world();
    let t = w.card("Left running");
    // it started long before the start-up that ends it (recover_interrupted sets its end to now)
    let r = w.running(&w.backend, &t, ids::now_ms() - 10 * HOUR);
    assert_eq!(runs::recover_interrupted(&w.db).unwrap(), 1);
    let run = runs::get(&w.db, &r).unwrap();
    assert_eq!(run.error.as_deref(), Some("interrupted"));
    let c = w.row(&w.kade, &t);
    assert_eq!((c.runs, c.time_ms, c.running_since), (1, LIMIT, None));
}

#[test]
fn a_span_below_zero_counts_as_zero() {
    let w = world();
    let t = w.card("Clock went back");
    w.ended(&w.backend, &t, T0, T0 - 5 * MIN, 1, 1);
    w.ended_as(&w.backend, &t, T0, T0 - 5 * MIN, 1, 1, "failed", Some("interrupted"));
    w.ended(&w.backend, &t, T0, T0 + 4 * MIN, 1, 1);
    let c = w.row(&w.kade, &t);
    assert_eq!((c.runs, c.time_ms), (3, 4 * MIN));
}

#[test]
fn a_removed_run_is_left_out() {
    let w = world();
    let t = w.card("Removed run");
    w.ended(&w.backend, &t, T0, T0 + 10 * MIN, 100, 10);
    let gone = w.ended(&w.backend, &t, T0 + HOUR, T0 + 2 * HOUR, 900, 90);
    let working = w.running(&w.qa, &t, T0 + 3 * HOUR);
    w.sql("UPDATE runs SET deleted_at=?2 WHERE id IN (?1, ?3)", rusqlite::params![gone, T0 + 5 * HOUR, working]);
    let c = w.row(&w.kade, &t);
    assert_eq!((c.runs, c.time_ms, c.running_since, c.input_tokens, c.output_tokens), (1, 10 * MIN, None, 100, 10));
}

#[test]
fn archived_done_and_cancelled_cards_are_on_it_with_their_column() {
    let w = world();
    team::add_state(&w.db, &w.you, &w.team, "Cancelled", &w.state("Done"), "cancelled").unwrap();
    let open = w.card_in(&w.kade, "To do", "Open");
    let done = w.card_in(&w.kade, "Done", "Done one");
    let cancelled = w.card_in(&w.kade, "Cancelled", "Cancelled one");
    let archived = w.card_in(&w.kade, "In progress", "Archived one");
    w.ended(&w.backend, &archived, T0, T0 + 42 * MIN, 700, 70);
    tasks::move_to(&w.db, &w.you, &archived, &w.state("Done"), "zz").unwrap();
    tasks::archive(&w.db, &w.you, &archived).unwrap();
    assert!(tasks::archived(&w.db, Some(&w.kade)).unwrap().iter().any(|a| a.id == archived));

    let rows = w.times(&w.kade);
    let col = |id: &str| rows.iter().find(|c| c.task_id == id).map(|c| (c.column.clone(), c.archived)).unwrap();
    assert_eq!(rows.len(), 4);
    assert_eq!(col(&open), ("To do".into(), false));
    assert_eq!(col(&done), ("Done".into(), false));
    assert_eq!(col(&cancelled), ("Cancelled".into(), false));
    assert_eq!(col(&archived), ("Done".into(), true));
    let a = w.row(&w.kade, &archived);
    assert_eq!((a.runs, a.time_ms, a.input_tokens, a.output_tokens), (1, 42 * MIN, 700, 70));
}

#[test]
fn another_projects_cards_and_runs_are_left_out() {
    let w = world();
    let k = w.card_in(&w.kade, "In progress", "Kade card");
    let m = w.card_in(&w.mobi, "In progress", "Mobi card");
    w.ended(&w.backend, &k, T0, T0 + 10 * MIN, 10, 1);
    w.ended(&w.backend, &m, T0, T0 + 50 * MIN, 500, 50);
    let kade = w.times(&w.kade);
    assert_eq!(kade.iter().map(|c| c.task_id.as_str()).collect::<Vec<_>>(), [k.as_str()]);
    assert_eq!((kade[0].runs, kade[0].time_ms, kade[0].input_tokens), (1, 10 * MIN, 10));
    let mobi = w.times(&w.mobi);
    assert_eq!(mobi.iter().map(|c| c.task_id.as_str()).collect::<Vec<_>>(), [m.as_str()]);
    assert_eq!((mobi[0].runs, mobi[0].time_ms, mobi[0].input_tokens), (1, 50 * MIN, 500));
}

#[test]
fn the_chiefs_chat_turns_and_board_checks_are_left_out() {
    let w = world();
    let t = w.card("With a chat around it");
    w.ended(&w.backend, &t, T0, T0 + 10 * MIN, 100, 10);
    // a chat turn and a board check, both long and expensive
    let th = chat::create_thread(&w.db, &w.you, &w.lead, "How is KADE doing?").unwrap();
    let turn = runs::create_chat(&w.db, &w.lead, &th, &ids::new_id(), "/tmp/lead", "/tmp/r.jsonl").unwrap();
    runs::set_running(&w.db, &turn, 7).unwrap();
    runs::finish_chat(&w.db, &turn, "succeeded", 9_000, 90_000, 9_000, None).unwrap();
    let check = board::create_run(&w.db, &w.lead, &ids::new_id(), "/tmp/lead", "/tmp/c.jsonl", &[]).unwrap();
    board::finish_run(&w.db, &check, "succeeded", 5_000, 40_000, 4_000, None, None).unwrap();
    w.sql("UPDATE runs SET started_at=?2, ended_at=?3 WHERE id IN (?1, ?4)", rusqlite::params![turn, T0, T0 + 5 * HOUR, check]);
    let rows = w.times(&w.kade);
    assert_eq!(rows.len(), 1, "a row per card, none for the chat or the board check");
    assert_eq!((rows[0].runs, rows[0].time_ms, rows[0].input_tokens, rows[0].output_tokens), (1, 10 * MIN, 100, 10));
}

#[test]
fn a_project_without_cards_is_an_empty_list_and_an_unknown_one_an_error() {
    let w = world();
    assert_eq!(w.times(&w.mobi), vec![]);
    let r = usage::project_time(&w.db, "no-such-project", LIMIT);
    assert!(matches!(r, Err(Error::NotFound(_))), "expected NotFound, got {r:?}");
}

#[test]
fn it_says_card_time_in_camel_case_as_the_ui_reads_it() {
    let c = CardTime { task_id: "t".into(), identifier: "KADE-1".into(), title: "A".into(), column: "Done".into(), archived: true, runs: 2,
        time_ms: 3, running_since: Some(4), input_tokens: 5, output_tokens: 6, cost_usd_micros: 7, unknown_cost_runs: 8 };
    assert_eq!(serde_json::to_value(&c).unwrap(), serde_json::json!({"taskId": "t", "identifier": "KADE-1", "title": "A", "column": "Done",
        "archived": true, "runs": 2, "timeMs": 3, "runningSince": 4, "inputTokens": 5, "outputTokens": 6, "costUsdMicros": 7,
        "unknownCostRuns": 8}));
}
