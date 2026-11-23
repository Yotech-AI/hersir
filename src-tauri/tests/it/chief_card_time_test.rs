//! GA-332 QA: the Chief's get_task, list_tasks and get_project give each card's agent time and tokens, called the way
//! the MCP server calls them, by the rules of the project's Time tracking tab (GA-331), from `usage::card_times`:
//! - get_task: `agent_time` over all of the card's runs (not only the 5 it shows), and each shown run its `minutes`
//!   (null for one that never started, up to now while it works, at most the run time limit when interrupted) and
//!   `tokens` (input + output).
//! - list_tasks: every line `agent_minutes` and `tokens` (0 without runs), the archived list too, from one card_times.
//! - get_project: `time_tracking` with the totals of the Time tracking tab's footer, and `done_cards` with the nearest
//!   rank median and p90 of the agent minutes of its cards in a done column that had runs (archived ones in, cancelled
//!   ones out).
//! - The Chief's chat turns and board checks count nowhere; the descriptions end with the card's sentences; every field
//!   the three tools gave before is still there.
use std::collections::HashMap;

use gizai_core::model::*;
use gizai_core::usage::{self, CardTime};
use gizai_core::{board, chat, ids, projects, runs, settings, tasks, team};
use gizai_lib::{AppState, tools};
use serde_json::{Value, json};

const FAKE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude.sh");

const MIN: i64 = 60_000;
const HOUR: i64 = 60 * MIN;
const SEC: i64 = 1_000;

/// The card's sentences, word for word.
const GET_TASK_SENTENCE: &str = "agent_time: what the agents' runs on it took, added up over all its runs (each start, Continue and nudge is a run): the minutes from each run's start to its end (up to now while one works; a run cut off when Hersir closed counts at most the run time limit) and the tokens (input with cache, and output), as the project's Time tracking tab counts them. Each run has its own minutes and tokens.";
const LIST_TASKS_SENTENCE: &str = "Each task has agent_minutes and tokens: its agents' time and tokens over all its runs, as in get_task's agent_time.";
const GET_PROJECT_SENTENCE: &str = "time_tracking: its agents' minutes and tokens over all its cards, and for its cards in Done that had runs, the median and 90th percentile of agent minutes per card: use them to estimate new cards.";

struct T {
    st: AppState,
    team: String,
    lead: String,
    backend: String,
    frontend: String,
    qa: String,
    kade: String,
    dir: tempfile::TempDir,
}

fn add_agent(st: &AppState, team_id: &str, name: &str, role: &str, chat: Option<bool>) -> String {
    team::add_agent(&st.db, &st.you_id, team_id, AgentInput { name: name.into(), role_key: role.into(), chat_enabled: chat, ..Default::default() })
        .unwrap()
}

/// A Chief (Team Lead, Chat on), a Backend, Frontend and QA Agent on the one team, and project Kade portal (KADE). The
/// queue is paused, so no card starts by itself.
fn setup() -> T {
    let dir = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(dir.path());
    settings::set(&st.db, "claude_bin", &FAKE.to_string()).unwrap();
    settings::set(&st.db, "agents_paused", &true).unwrap();
    let team_id = team::list(&st.db).unwrap()[0].id.clone();
    let lead = add_agent(&st, &team_id, "Team Lead", "lead", Some(true));
    let backend = add_agent(&st, &team_id, "Backend Agent", "backend", None);
    let frontend = add_agent(&st, &team_id, "Frontend Agent", "frontend", None);
    let qa = add_agent(&st, &team_id, "QA Agent", "qa", None);
    let kade = projects::create(&st.db, &st.you_id, ProjectInput { name: "Kade portal".into(), key: "KADE".into(), ..Default::default() }).unwrap();
    T { st, team: team_id, lead, backend, frontend, qa, kade, dir }
}

impl T {
    async fn ok(&self, name: &str, args: Value) -> Value {
        tools::call(&self.st, &self.lead, name, args).await.unwrap_or_else(|e| panic!("{name} failed: {e}"))
    }

    fn sql(&self, sql: &str, id: &str) {
        self.st.db.write(None, |w| { w.conn().execute(sql, [id])?; Ok(()) }).unwrap();
    }

    fn state(&self, name: &str) -> String {
        self.st.db.read(|c| Ok(c.query_row("SELECT id FROM workflow_states WHERE team_id=?1 AND name=?2", [self.team.as_str(), name],
            |r| r.get(0))?)).unwrap()
    }

    fn project(&self, name: &str, key: &str) -> String {
        projects::create(&self.st.db, &self.st.you_id, ProjectInput { name: name.into(), key: key.into(), ..Default::default() }).unwrap()
    }

    /// A card of `project` in `column`: its id and identifier.
    fn card_of(&self, project: &str, column: &str, title: &str) -> (String, String) {
        let id = tasks::create(&self.st.db, &self.st.you_id, TaskInput { project_id: project.into(), title: title.into(),
            state_id: Some(self.state(column)), ..Default::default() }).unwrap();
        let ident = tasks::get(&self.st.db, &id).unwrap().identifier;
        (id, ident)
    }

    fn card(&self, column: &str, title: &str) -> (String, String) { self.card_of(&self.kade, column, title) }

    fn path(&self) -> String { self.dir.path().display().to_string() }

    /// A queued run of `agent` on `task`, made at `made` (it never started), as the dispatcher records one.
    fn queued_as(&self, agent: &str, task: &str, nudge: bool, made: i64) -> String {
        let p = self.path();
        let r = if nudge {
            runs::create_nudge(&self.st.db, agent, task, "backend", &ids::new_id(), &p, &p, "b", &format!("{p}/l")).unwrap()
        } else {
            runs::create(&self.st.db, agent, task, "backend", &ids::new_id(), &p, &p, "b", &format!("{p}/l")).unwrap()
        };
        // let the next run on the card in, as a run's end does; and a known order for get_task's newest 5
        self.sql("UPDATE tasks SET claimed_by_run_id=NULL, lease_expires_at=NULL WHERE id=?1", task);
        self.sql(&format!("UPDATE runs SET created_at={made} WHERE id=?1"), &r);
        r
    }

    fn queued(&self, agent: &str, task: &str, made: i64) -> String { self.queued_as(agent, task, false, made) }

    /// A run of `agent` on `task` working now, started at `start`.
    fn running(&self, agent: &str, task: &str, start: i64) -> String {
        let r = self.queued(agent, task, start);
        runs::set_running(self.db(), &r, 4242).unwrap();
        self.sql(&format!("UPDATE runs SET started_at={start} WHERE id=?1"), &r);
        r
    }

    /// A run (a nudge when `nudge`) from `start` to `end`, ended as `status` with `error`, with these tokens.
    #[allow(clippy::too_many_arguments)]
    fn ended_full(&self, agent: &str, task: &str, nudge: bool, start: i64, end: i64, input: i64, output: i64, status: &str,
                  error: Option<&str>) -> String {
        let r = self.queued_as(agent, task, nudge, start);
        runs::set_running(self.db(), &r, 4242).unwrap();
        runs::finish(self.db(), &r, status, None, 1_000, input, output, error).unwrap();
        self.sql(&format!("UPDATE runs SET started_at={start}, ended_at={end} WHERE id=?1"), &r);
        r
    }

    #[allow(clippy::too_many_arguments)]
    fn ended_as(&self, agent: &str, task: &str, start: i64, end: i64, input: i64, output: i64, status: &str, error: Option<&str>) -> String {
        self.ended_full(agent, task, false, start, end, input, output, status, error)
    }

    fn ended(&self, agent: &str, task: &str, start: i64, end: i64, input: i64, output: i64) -> String {
        self.ended_as(agent, task, start, end, input, output, "succeeded", None)
    }

    fn db(&self) -> &gizai_core::db::Db { &self.st.db }

    fn max_run_ms(&self) -> i64 { gizai_lib::runs::get_settings(&self.st).max_run_minutes as i64 * MIN }

    /// The project's Time tracking tab, as its command reads it (the run time limit as it is set now).
    fn tab(&self, project: &str) -> Vec<CardTime> { usage::project_time(self.db(), project, self.max_run_ms()).unwrap() }

    fn tab_row(&self, project: &str, task: &str) -> CardTime {
        self.tab(project).into_iter().find(|c| c.task_id == task).unwrap_or_else(|| panic!("{task} isn't on the tab"))
    }

    /// A move into Done, then Archive, as the user does it.
    fn archive(&self, task: &str) {
        tasks::move_to(self.db(), &self.st.you_id, task, &self.state("Done"), "zz").unwrap();
        tasks::archive(self.db(), &self.st.you_id, task).unwrap();
    }
}

/// A line of list_tasks by its identifier.
fn line<'a>(list: &'a Value, ident: &str) -> &'a Value {
    list["tasks"].as_array().unwrap().iter().find(|l| l["task"] == ident).unwrap_or_else(|| panic!("{ident} isn't listed: {list}"))
}

fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().unwrap_or_else(|| panic!("not an object: {v}")).keys().cloned().collect();
    k.sort();
    k
}

fn sorted(list: &[&str]) -> Vec<String> {
    let mut k: Vec<String> = list.iter().map(|s| s.to_string()).collect();
    k.sort();
    k
}

// ---- get_task ------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn get_task_adds_up_all_seven_runs_of_every_agent_and_gives_each_shown_run_its_minutes_and_tokens() {
    let t = setup();
    let (task, ident) = t.card("In progress", "Export invoices");
    let base = ids::now_ms() - 2 * 24 * HOUR;
    // seven runs that started, oldest first: a start by the Backend Agent, QA, the Chief's run on the card, the
    // Frontend Agent's cancelled one, a nudge, one Hersir cut off for a night, and 30 seconds of QA
    t.ended(&t.backend, &task, base, base + 10 * MIN, 1_000, 100);
    t.ended_as(&t.qa, &task, base + HOUR, base + HOUR + 20 * MIN, 2_000, 200, "failed", Some("exit 1"));
    t.ended(&t.lead, &task, base + 2 * HOUR, base + 2 * HOUR + 2 * MIN, 40, 4);
    t.ended_as(&t.frontend, &task, base + 3 * HOUR, base + 3 * HOUR + 15 * MIN, 500, 50, "cancelled", None);
    t.ended_full(&t.backend, &task, true, base + 4 * HOUR, base + 4 * HOUR + 5 * MIN, 300, 30, "succeeded", None);
    t.ended_as(&t.backend, &task, base + 5 * HOUR, base + 19 * HOUR, 700, 70, "failed", Some("interrupted"));
    t.ended(&t.qa, &task, base + 20 * HOUR, base + 20 * HOUR + 30 * SEC, 50, 5);
    // and a queued run that never started: not a run, no time
    t.queued(&t.qa, &task, base + 21 * HOUR);

    let got = t.ok("get_task", json!({"task": ident})).await;
    // 10 + 20 + 2 + 15 + 5 + 90 (the limit) minutes and 30 seconds: 142.5 minutes, rounded to 143
    assert_eq!(got["task"]["agent_time"], json!({"runs": 7, "minutes": 143, "working_now": false,
        "input_tokens": 4_590, "output_tokens": 459}), "{got}");

    // the newest 5, newest first, each with its own minutes and tokens
    let shown: Vec<(Value, Value, Value)> = got["runs"].as_array().unwrap().iter()
        .map(|r| (r["trigger"].clone(), r["minutes"].clone(), r["tokens"].clone())).collect();
    assert_eq!(shown, vec![
        (json!("routed"), Value::Null, json!(0)),
        (json!("routed"), json!(1), json!(55)),
        (json!("routed"), json!(90), json!(770)),
        // a nudge reads as trigger result_nudge (runs::RESULT_NUDGE)
        (json!("result_nudge"), json!(5), json!(330)),
        (json!("routed"), json!(15), json!(550)),
    ], "{got}");

    // the same numbers as the card's row on the project's Time tracking tab
    let row = t.tab_row(&t.kade, &task);
    let a = &got["task"]["agent_time"];
    assert_eq!((a["runs"].as_i64(), a["minutes"].as_i64(), a["input_tokens"].as_i64(), a["output_tokens"].as_i64()),
               (Some(row.runs), Some(usage::whole_minutes(row.time_ms)), Some(row.input_tokens), Some(row.output_tokens)));
    assert_eq!(row.running_since, None);
}

#[tokio::test]
async fn a_card_without_runs_has_zero_agent_time_and_no_runs() {
    let t = setup();
    let (_, ident) = t.card("To do", "Nothing yet");
    let got = t.ok("get_task", json!({"task": ident})).await;
    assert_eq!(got["task"]["agent_time"], json!({"runs": 0, "minutes": 0, "working_now": false, "input_tokens": 0, "output_tokens": 0}));
    assert_eq!(got["runs"], json!([]));
}

#[tokio::test]
async fn a_run_working_now_counts_up_to_now_and_says_working_now() {
    let t = setup();
    let (task, ident) = t.card("In progress", "Working");
    let now = ids::now_ms();
    t.ended(&t.backend, &task, now - 2 * HOUR, now - 2 * HOUR + 10 * MIN, 100, 10);
    let r = t.running(&t.qa, &task, now - 25 * MIN);

    let got = t.ok("get_task", json!({"task": ident})).await;
    assert_eq!(got["task"]["agent_time"], json!({"runs": 2, "minutes": 35, "working_now": true, "input_tokens": 100, "output_tokens": 10}),
               "{got}");
    // the running run is the newest: its minutes up to now, no tokens until it ends
    assert_eq!((got["runs"][0]["status"].clone(), got["runs"][0]["minutes"].clone(), got["runs"][0]["tokens"].clone()),
               (json!("running"), json!(25), json!(0)), "{got}");
    assert_eq!(got["runs"][1]["minutes"], 10);
    // list_tasks and get_project count it up to now too
    let list = t.ok("list_tasks", json!({"project": "KADE"})).await;
    assert_eq!((line(&list, &ident)["agent_minutes"].clone(), line(&list, &ident)["tokens"].clone()), (json!(35), json!(110)), "{list}");
    let p = t.ok("get_project", json!({"project": "KADE"})).await;
    assert_eq!((p["time_tracking"]["agent_minutes"].clone(), p["time_tracking"]["runs"].clone()), (json!(35), json!(2)), "{p}");

    // once it ends it isn't working any more, and its span counts
    runs::finish(t.db(), &r, "succeeded", None, 0, 300, 30, None).unwrap();
    t.sql(&format!("UPDATE runs SET started_at={}, ended_at={} WHERE id=?1", now - 60 * MIN, now - 20 * MIN), &r);
    let got = t.ok("get_task", json!({"task": ident})).await;
    assert_eq!(got["task"]["agent_time"], json!({"runs": 2, "minutes": 50, "working_now": false, "input_tokens": 400, "output_tokens": 40}));
    assert_eq!(got["runs"][0]["minutes"], 40);
    assert_eq!(got["runs"][0]["tokens"], 330);
}

#[tokio::test]
async fn an_interrupted_run_counts_at_most_the_run_time_limit_as_it_is_set_now() {
    let t = setup();
    // Settings → Agents and runs: 30 minutes
    settings::set(t.db(), "max_run_minutes", &30u64).unwrap();
    assert_eq!(gizai_lib::runs::get_settings(&t.st).max_run_minutes, 30);
    let (task, ident) = t.card("In progress", "Interrupted");
    let base = ids::now_ms() - 3 * 24 * HOUR;
    // closed for a night: counts the limit; a short one its span; one that failed otherwise all of its 3 hours
    t.ended_as(&t.backend, &task, base, base + 14 * HOUR, 0, 0, "failed", Some("interrupted"));
    t.ended_as(&t.backend, &task, base + 15 * HOUR, base + 15 * HOUR + 12 * MIN, 0, 0, "failed", Some("interrupted"));
    t.ended_as(&t.backend, &task, base + 16 * HOUR, base + 19 * HOUR, 0, 0, "failed", Some("exit 1"));
    // and one left running when Hersir closed, which the next start-up ends as interrupted
    t.running(&t.qa, &task, ids::now_ms() - 10 * HOUR);
    assert_eq!(runs::recover_interrupted(t.db()).unwrap(), 1);

    let got = t.ok("get_task", json!({"task": ident})).await;
    let minutes: Vec<Value> = got["runs"].as_array().unwrap().iter().map(|r| r["minutes"].clone()).collect();
    assert_eq!(minutes, [json!(30), json!(180), json!(12), json!(30)], "{got}");
    assert_eq!(got["task"]["agent_time"]["minutes"], 30 + 180 + 12 + 30);
    assert_eq!(got["task"]["agent_time"]["working_now"], false);
    assert_eq!(got["task"]["agent_time"]["minutes"].as_i64(), Some(usage::whole_minutes(t.tab_row(&t.kade, &task).time_ms)));

    // with the default limit (90 minutes) the night counts 90
    settings::set(t.db(), "max_run_minutes", &90u64).unwrap();
    let got = t.ok("get_task", json!({"task": ident})).await;
    assert_eq!(got["task"]["agent_time"]["minutes"], 90 + 180 + 12 + 90, "{got}");
}

#[tokio::test]
async fn a_runs_minutes_are_rounded_to_whole_minutes() {
    let t = setup();
    let (task, ident) = t.card("In progress", "Short runs");
    let base = ids::now_ms() - 24 * HOUR;
    t.ended(&t.backend, &task, base, base + 29 * SEC, 1, 0);
    t.ended(&t.backend, &task, base + HOUR, base + HOUR + 89 * SEC, 1, 0);
    t.ended(&t.backend, &task, base + 2 * HOUR, base + 2 * HOUR + 90 * SEC, 1, 0);
    // a clock that went back: never below 0
    t.ended(&t.backend, &task, base + 3 * HOUR, base + 3 * HOUR - 5 * MIN, 1, 0);
    let got = t.ok("get_task", json!({"task": ident})).await;
    let minutes: Vec<Value> = got["runs"].as_array().unwrap().iter().map(|r| r["minutes"].clone()).collect();
    assert_eq!(minutes, [json!(0), json!(2), json!(1), json!(0)], "{got}");
    // the card: 29 + 89 + 90 seconds, 3.47 minutes, rounded once
    assert_eq!(got["task"]["agent_time"]["minutes"], 3);
    assert_eq!(got["task"]["agent_time"]["runs"], 4);
}

#[tokio::test]
async fn a_removed_run_is_left_out_of_agent_time() {
    let t = setup();
    let (task, ident) = t.card("In progress", "Removed run");
    let base = ids::now_ms() - 24 * HOUR;
    t.ended(&t.backend, &task, base, base + 10 * MIN, 100, 10);
    let gone = t.ended(&t.backend, &task, base + HOUR, base + 2 * HOUR, 900, 90);
    t.sql(&format!("UPDATE runs SET deleted_at={} WHERE id=?1", base + 3 * HOUR), &gone);
    let got = t.ok("get_task", json!({"task": ident})).await;
    assert_eq!(got["task"]["agent_time"], json!({"runs": 1, "minutes": 10, "working_now": false, "input_tokens": 100, "output_tokens": 10}));
    assert_eq!(got["runs"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn an_archived_card_has_its_agent_time_in_get_task() {
    let t = setup();
    let (task, ident) = t.card("In progress", "Archived one");
    let base = ids::now_ms() - 24 * HOUR;
    t.ended(&t.backend, &task, base, base + 42 * MIN, 700, 70);
    t.archive(&task);
    let got = t.ok("get_task", json!({"task": ident})).await;
    assert_eq!(got["task"]["archived"], true);
    assert_eq!(got["task"]["agent_time"], json!({"runs": 1, "minutes": 42, "working_now": false, "input_tokens": 700, "output_tokens": 70}));
}

// ---- list_tasks ----------------------------------------------------------------------------------------------------

#[tokio::test]
async fn list_tasks_gives_every_line_agent_minutes_and_tokens_and_zero_without_runs_also_archived() {
    let t = setup();
    let base = ids::now_ms() - 24 * HOUR;
    let (a, a_id) = t.card("In progress", "With runs");
    t.ended(&t.backend, &a, base, base + 10 * MIN, 1_000, 100);
    t.ended(&t.qa, &a, base + HOUR, base + HOUR + 6 * MIN, 1_500, 150);
    let (_, b_id) = t.card("To do", "Without runs");
    let (d, d_id) = t.card("Done", "Done one");
    t.ended(&t.backend, &d, base, base + 7 * MIN, 70, 7);
    let (c, c_id) = t.card("In progress", "Archived one");
    t.ended(&t.backend, &c, base, base + 20 * MIN, 50, 5);
    t.archive(&c);

    let list = t.ok("list_tasks", json!({"project": "KADE"})).await;
    assert_eq!(list["count"], 2, "{list}");
    assert_eq!((line(&list, &a_id)["agent_minutes"].clone(), line(&list, &a_id)["tokens"].clone()), (json!(16), json!(2_750)));
    assert_eq!((line(&list, &b_id)["agent_minutes"].clone(), line(&list, &b_id)["tokens"].clone()), (json!(0), json!(0)));

    let all = t.ok("list_tasks", json!({"include_done": true})).await;
    assert_eq!((line(&all, &d_id)["agent_minutes"].clone(), line(&all, &d_id)["tokens"].clone()), (json!(7), json!(77)));
    assert_eq!(line(&all, &a_id)["agent_minutes"], 16);

    let archived = t.ok("list_tasks", json!({"archived": true})).await;
    assert_eq!(archived["count"], 1, "{archived}");
    let l = line(&archived, &c_id);
    assert_eq!((l["archived"].clone(), l["agent_minutes"].clone(), l["tokens"].clone()), (json!(true), json!(20), json!(55)), "{archived}");

    // the same numbers as each card's row on the Time tracking tab
    for c in t.tab(&t.kade) {
        let list = if c.archived { &archived } else { &all };
        let l = line(list, &c.identifier);
        assert_eq!(l["agent_minutes"].as_i64(), Some(usage::whole_minutes(c.time_ms)), "{}", c.identifier);
        assert_eq!(l["tokens"].as_i64(), Some(c.input_tokens + c.output_tokens), "{}", c.identifier);
    }

    // with a limit, the lines shown still have theirs
    let one = t.ok("list_tasks", json!({"include_done": true, "limit": 1})).await;
    assert_eq!(one["shown"], 1);
    assert!(one["tasks"][0]["agent_minutes"].is_i64() && one["tasks"][0]["tokens"].is_i64(), "{one}");
}

#[tokio::test]
async fn list_tasks_counts_a_card_from_all_its_runs_and_one_working_now() {
    let t = setup();
    let (task, ident) = t.card("In progress", "Many runs");
    let base = ids::now_ms() - 24 * HOUR;
    // seven runs: list_tasks counts all of them, not get_task's 5
    for i in 0..7 {
        t.ended(&t.backend, &task, base + i * HOUR, base + i * HOUR + 3 * MIN, 10, 1);
    }
    let list = t.ok("list_tasks", json!({})).await;
    assert_eq!((line(&list, &ident)["agent_minutes"].clone(), line(&list, &ident)["tokens"].clone()), (json!(21), json!(77)), "{list}");
    let got = t.ok("get_task", json!({"task": ident})).await;
    assert_eq!(got["task"]["agent_time"]["runs"], 7);
    assert_eq!(got["task"]["agent_time"]["minutes"], line(&list, &ident)["agent_minutes"]);
    assert_eq!(got["task"]["agent_time"]["input_tokens"].as_i64().unwrap() + got["task"]["agent_time"]["output_tokens"].as_i64().unwrap(),
               line(&list, &ident)["tokens"].as_i64().unwrap());
}

/// The body of `fn <name>` in the tools' read.rs, up to the next function.
fn read_rs_fn(name: &str) -> String {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/tools/read.rs")).unwrap();
    let start = text.find(&format!("fn {name}(")).unwrap_or_else(|| panic!("no fn {name}"));
    let rest = &text[start..];
    let end = rest[1..].find("\nfn ").into_iter().chain(rest[1..].find("\npub(crate) ")).chain(rest[1..].find("\npub fn ")).min()
        .map_or(rest.len(), |e| e + 1);
    rest[..end].to_string()
}

#[test]
fn list_tasks_reads_the_times_of_the_whole_list_with_one_card_times_call() {
    let body = read_rs_fn("list_tasks");
    assert_eq!(body.matches("card_times(").count(), 1, "{body}");
    assert!(!body.contains("project_time("), "{body}");
    // the one call comes before the lines are made, not inside the closure that makes each line
    let call = body.find("card_times(").unwrap();
    let per_line = body.find("task_line(t)").expect("each line from task_line");
    assert!(call < per_line, "card_times is called once for the list, before the lines: {body}");
    // get_task and get_project count by the same rules: card_times, and time_tracking over the tab's rows
    assert_eq!(read_rs_fn("get_task").matches("usage::card_times(").count(), 1);
    assert!(read_rs_fn("get_project").contains("usage::time_tracking("));
}

#[test]
fn card_times_counts_an_id_given_twice_once_and_leaves_an_unknown_one_out() {
    let t = setup();
    let base = ids::now_ms() - 24 * HOUR;
    let (a, _) = t.card("In progress", "One");
    let (b, _) = t.card("To do", "Two");
    t.ended(&t.backend, &a, base, base + 10 * MIN, 100, 10);
    t.ended(&t.qa, &a, base + HOUR, base + HOUR + 5 * MIN, 200, 20);
    let ids_given = vec![a.clone(), a.clone(), b.clone(), "no-such-card".to_string()];
    let times: HashMap<String, CardTime> = usage::card_times(t.db(), &ids_given, 90 * MIN).unwrap();
    assert_eq!(times.len(), 2);
    let one = &times[&a];
    assert_eq!((one.runs, one.time_ms, one.input_tokens, one.output_tokens), (2, 15 * MIN, 300, 30));
    let two = &times[&b];
    assert_eq!((two.runs, two.time_ms, two.running_since, two.input_tokens, two.output_tokens), (0, 0, None, 0, 0));
    assert!(usage::card_times(t.db(), &[], 90 * MIN).unwrap().is_empty());
    // the tab and card_times agree, row for row
    for row in t.tab(&t.kade) {
        assert_eq!(times.get(&row.task_id), Some(&row));
    }
}

// ---- get_project ---------------------------------------------------------------------------------------------------

#[tokio::test]
async fn get_project_time_tracking_adds_up_all_its_cards_and_gives_the_median_and_p90_of_its_done_cards() {
    let t = setup();
    team::add_state(t.db(), &t.st.you_id, &t.team, "Cancelled", &t.state("Done"), "cancelled").unwrap();
    let mobi = t.project("Mobi app", "MOBI");
    let base = ids::now_ms() - 3 * 24 * HOUR;
    // four cards in Done with runs, given out of order: 40, 10, 30 (archived) and 20 minutes; the 20 in two runs
    let (d40, _) = t.card("Done", "Forty");
    t.ended(&t.backend, &d40, base, base + 40 * MIN, 4_000, 400);
    let (d10, _) = t.card("Done", "Ten");
    t.ended(&t.backend, &d10, base, base + 10 * MIN, 1_000, 100);
    let (d30, _) = t.card("In progress", "Thirty, archived");
    t.ended(&t.qa, &d30, base, base + 30 * MIN, 3_000, 300);
    t.archive(&d30);
    let (d20, _) = t.card("Done", "Twenty");
    t.ended(&t.backend, &d20, base, base + 12 * MIN, 1_200, 120);
    t.ended(&t.qa, &d20, base + HOUR, base + HOUR + 8 * MIN, 800, 80);
    // a removed run on a done card counts nowhere
    let gone = t.ended(&t.qa, &d20, base + 2 * HOUR, base + 9 * HOUR, 99_999, 9_999);
    t.sql(&format!("UPDATE runs SET deleted_at={} WHERE id=?1", base + 10 * HOUR), &gone);
    // left out of done_cards: a cancelled card, a card in Done without runs, one in Deploy and one in progress; they
    // count in the totals
    let (cancelled, _) = t.card("Cancelled", "Cancelled one");
    t.ended(&t.backend, &cancelled, base, base + 500 * MIN, 5_000, 500);
    t.card("Done", "Done without runs");
    let (deploy, _) = t.card("Deploy", "In Deploy");
    t.ended(&t.backend, &deploy, base, base + 60 * MIN, 600, 60);
    let (open, _) = t.card("In progress", "Open");
    t.ended(&t.frontend, &open, base, base + 7 * MIN, 70, 7);
    // another project's card in Done counts nowhere here
    let (other, _) = t.card_of(&mobi, "Done", "Mobi card");
    t.ended(&t.backend, &other, base, base + 1_000 * MIN, 50_000, 5_000);

    let p = t.ok("get_project", json!({"project": "KADE"})).await;
    let input = 4_000 + 1_000 + 3_000 + 1_200 + 800 + 5_000 + 600 + 70;
    let output = 400 + 100 + 300 + 120 + 80 + 500 + 60 + 7;
    assert_eq!(p["time_tracking"], json!({
        "cards_with_runs": 7, "runs": 8, "agent_minutes": 40 + 10 + 30 + 20 + 500 + 60 + 7, "input_tokens": input, "output_tokens": output,
        "done_cards": {"count": 4, "median_minutes": 20, "p90_minutes": 40},
    }), "{p}");

    // the Time tracking tab's footer: the runs, the agent time and the tokens of all its rows
    let rows = t.tab(&t.kade);
    let now = ids::now_ms();
    let tt = &p["time_tracking"];
    assert_eq!(tt["runs"].as_i64(), Some(rows.iter().map(|c| c.runs).sum::<i64>()));
    assert_eq!(tt["agent_minutes"].as_i64(), Some(usage::whole_minutes(rows.iter().map(|c| usage::agent_ms(c, now)).sum::<i64>())));
    assert_eq!(tt["input_tokens"].as_i64(), Some(rows.iter().map(|c| c.input_tokens).sum::<i64>()));
    assert_eq!(tt["output_tokens"].as_i64(), Some(rows.iter().map(|c| c.output_tokens).sum::<i64>()));
    assert_eq!(tt["cards_with_runs"].as_i64(), Some(rows.iter().filter(|c| c.runs > 0).count() as i64));

    // MOBI: its one card alone
    let m = t.ok("get_project", json!({"project": "MOBI"})).await;
    assert_eq!(m["time_tracking"], json!({"cards_with_runs": 1, "runs": 1, "agent_minutes": 1_000, "input_tokens": 50_000,
        "output_tokens": 5_000, "done_cards": {"count": 1, "median_minutes": 1_000, "p90_minutes": 1_000}}), "{m}");
}

#[tokio::test]
async fn done_cards_has_null_median_and_p90_without_a_done_card_that_had_runs() {
    let t = setup();
    // no cards at all
    let p = t.ok("get_project", json!({"project": "KADE"})).await;
    assert_eq!(p["time_tracking"], json!({"cards_with_runs": 0, "runs": 0, "agent_minutes": 0, "input_tokens": 0, "output_tokens": 0,
        "done_cards": {"count": 0, "median_minutes": null, "p90_minutes": null}}), "{p}");
    // a card in Done without runs and one in progress with a run: still none
    t.card("Done", "Done without runs");
    let (open, _) = t.card("In progress", "Open");
    let base = ids::now_ms() - 24 * HOUR;
    t.ended(&t.backend, &open, base, base + 9 * MIN, 90, 9);
    let p = t.ok("get_project", json!({"project": "KADE"})).await;
    assert_eq!(p["time_tracking"]["done_cards"], json!({"count": 0, "median_minutes": null, "p90_minutes": null}), "{p}");
    assert_eq!((p["time_tracking"]["cards_with_runs"].clone(), p["time_tracking"]["agent_minutes"].clone()), (json!(1), json!(9)));
    // a Done card whose only run never started had no runs either
    let (queued_only, _) = t.card("Done", "Only queued");
    t.queued(&t.backend, &queued_only, base);
    let p = t.ok("get_project", json!({"project": "KADE"})).await;
    assert_eq!(p["time_tracking"]["done_cards"]["count"], 0, "{p}");
}

#[test]
fn the_median_and_p90_are_the_nearest_rank_of_the_minutes_low_to_high() {
    assert_eq!((usage::nearest_rank(&[10, 20, 30, 40], 50), usage::nearest_rank(&[10, 20, 30, 40], 90)), (Some(20), Some(40)));
    assert_eq!((usage::nearest_rank(&[], 50), usage::nearest_rank(&[], 90)), (None, None));
    assert_eq!((usage::nearest_rank(&[15], 50), usage::nearest_rank(&[15], 90)), (Some(15), Some(15)));
    // ranks ceil(0.5 × 5) = 3 and ceil(0.9 × 5) = 5
    assert_eq!((usage::nearest_rank(&[1, 2, 3, 4, 5], 50), usage::nearest_rank(&[1, 2, 3, 4, 5], 90)), (Some(3), Some(5)));
    // ranks 5 and 9 of 10
    let ten: Vec<i64> = (1..=10).collect();
    assert_eq!((usage::nearest_rank(&ten, 50), usage::nearest_rank(&ten, 90)), (Some(5), Some(9)));
    // ranks 6 and 10 of 11
    let eleven: Vec<i64> = (1..=11).collect();
    assert_eq!((usage::nearest_rank(&eleven, 50), usage::nearest_rank(&eleven, 90)), (Some(6), Some(10)));
}

#[tokio::test]
async fn the_median_and_p90_use_each_done_cards_rounded_minutes_and_one_working_now() {
    let t = setup();
    let now = ids::now_ms();
    let base = now - 24 * HOUR;
    // 89 seconds (1 minute) and 90 seconds (2 minutes)
    let (a, _) = t.card("Done", "A");
    t.ended(&t.backend, &a, base, base + 89 * SEC, 1, 1);
    let (b, _) = t.card("Done", "B");
    t.ended(&t.backend, &b, base, base + 90 * SEC, 1, 1);
    let p = t.ok("get_project", json!({"project": "KADE"})).await;
    assert_eq!(p["time_tracking"]["done_cards"], json!({"count": 2, "median_minutes": 1, "p90_minutes": 2}), "{p}");
    // the total adds the time first and rounds once: 179 seconds are 3 minutes
    assert_eq!(p["time_tracking"]["agent_minutes"], 3);
    // a QA run working now on a card in Done counts up to now
    let (c, _) = t.card("Done", "C");
    t.running(&t.qa, &c, now - 50 * MIN);
    let p = t.ok("get_project", json!({"project": "KADE"})).await;
    assert_eq!(p["time_tracking"]["done_cards"], json!({"count": 3, "median_minutes": 2, "p90_minutes": 50}), "{p}");
}

// ---- chat turns and board checks -----------------------------------------------------------------------------------

#[tokio::test]
async fn the_chiefs_chat_turns_and_board_checks_count_nowhere() {
    let t = setup();
    let (task, ident) = t.card("Done", "With a chat around it");
    let base = ids::now_ms() - 24 * HOUR;
    t.ended(&t.backend, &task, base, base + 10 * MIN, 100, 10);
    let before = (t.ok("get_task", json!({"task": ident})).await["task"]["agent_time"].clone(),
                  t.ok("list_tasks", json!({"include_done": true})).await["tasks"].clone(),
                  t.ok("get_project", json!({"project": "KADE"})).await["time_tracking"].clone());

    // a chat turn about the card and a board check, both long and expensive
    let p = t.path();
    let th = chat::create_thread(t.db(), &t.st.you_id, &t.lead, &format!("How long did {ident} take?")).unwrap();
    let turn = runs::create_chat(t.db(), &t.lead, &th, &ids::new_id(), &p, &format!("{p}/r.jsonl")).unwrap();
    runs::set_running(t.db(), &turn, 7).unwrap();
    runs::finish_chat(t.db(), &turn, "succeeded", 9_000, 90_000, 9_000, None).unwrap();
    let check = board::create_run(t.db(), &t.lead, &ids::new_id(), &p, &format!("{p}/c.jsonl"), &[]).unwrap();
    board::finish_run(t.db(), &check, "succeeded", 5_000, 40_000, 4_000, None, None).unwrap();
    for id in [&turn, &check] {
        t.sql(&format!("UPDATE runs SET started_at={}, ended_at={} WHERE id=?1", base, base + 5 * HOUR), id);
    }
    // and one working now
    let now_turn = runs::create_chat(t.db(), &t.lead, &th, &ids::new_id(), &p, &format!("{p}/r2.jsonl")).unwrap();
    runs::set_running(t.db(), &now_turn, 8).unwrap();
    t.sql(&format!("UPDATE runs SET started_at={} WHERE id=?1", ids::now_ms() - 3 * HOUR), &now_turn);

    let got = t.ok("get_task", json!({"task": ident})).await;
    assert_eq!(got["task"]["agent_time"], before.0, "{got}");
    assert_eq!(got["task"]["agent_time"], json!({"runs": 1, "minutes": 10, "working_now": false, "input_tokens": 100, "output_tokens": 10}));
    assert_eq!(got["runs"].as_array().unwrap().len(), 1, "{got}");
    assert_eq!(t.ok("list_tasks", json!({"include_done": true})).await["tasks"], before.1);
    let tt = t.ok("get_project", json!({"project": "KADE"})).await["time_tracking"].clone();
    assert_eq!(tt, before.2);
    assert_eq!(tt, json!({"cards_with_runs": 1, "runs": 1, "agent_minutes": 10, "input_tokens": 100, "output_tokens": 10,
        "done_cards": {"count": 1, "median_minutes": 10, "p90_minutes": 10}}));
}

// ---- descriptions --------------------------------------------------------------------------------------------------

#[test]
fn the_three_descriptions_end_with_the_cards_sentences_word_for_word() {
    let cat = tools::catalog();
    let desc = |name: &str| cat.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("no tool {name}")).description.clone();
    for (name, sentence) in [("get_task", GET_TASK_SENTENCE), ("list_tasks", LIST_TASKS_SENTENCE), ("get_project", GET_PROJECT_SENTENCE)] {
        let d = desc(name);
        assert!(d.ends_with(&format!(" {sentence}")), "{name} doesn't end with the card's sentence: {d}");
        assert_eq!(d.matches(sentence).count(), 1, "{name}: {d}");
    }
    // what they said before is still there, in front of the new sentence
    assert!(desc("get_task").starts_with("One task with its description, acceptance criteria, labels, hold, builds_on, needed_by, latest comments, agent runs"));
    assert!(desc("list_tasks").starts_with("Tasks, newest columns first."));
    assert!(desc("list_tasks").contains("A task that waits for cards it builds on shows them in waits_for (not merged yet). Each task has agent_minutes"));
    assert!(desc("get_project").starts_with("One project with its goal, team (whose board its cards are on), repository, Test runs at once, tasks per column, docs and files."));
}

// ---- every field the tools gave before ------------------------------------------------------------------------------

#[tokio::test]
async fn every_field_the_three_tools_gave_before_is_still_there_and_unchanged() {
    let t = setup();
    let base = ids::now_ms() - 24 * HOUR;
    let (task, ident) = t.card("In progress", "Export invoices");
    tasks::update(t.db(), &t.st.you_id, &task, TaskPatch { description_md: Some("Make a CSV".into()), acceptance_md: Some("- [ ] A header row".into()),
        ..Default::default() }).unwrap();
    t.ended_as(&t.backend, &task, base, base + 10 * MIN, 1_000, 100, "failed", Some("exit 1"));
    let (old, old_id) = t.card("In progress", "Archived");
    t.ended(&t.backend, &old, base, base + MIN, 1, 1);
    t.archive(&old);

    let got = t.ok("get_task", json!({"task": ident})).await;
    assert_eq!(keys(&got), sorted(&["task", "comments", "runs", "files"]));
    assert_eq!(keys(&got["task"]), sorted(&["id", "task", "title", "project", "column", "priority", "assignee", "labels", "hold", "hold_reason",
        "testing", "description_md", "acceptance_md", "branch", "with_team_lead", "by_hand", "waiting_for_ok", "created", "updated", "archived",
        "archived_on", "archived_by", "builds_on", "needed_by", "agent_time"]), "{got}");
    assert_eq!((got["task"]["id"].clone(), got["task"]["task"].clone(), got["task"]["title"].clone(), got["task"]["column"].clone(),
                got["task"]["description_md"].clone(), got["task"]["acceptance_md"].clone(), got["task"]["archived"].clone()),
               (json!(task), json!(ident), json!("Export invoices"), json!("In progress"), json!("Make a CSV"), json!("- [ ] A header row"), json!(false)));
    let run = &got["runs"][0];
    assert_eq!(keys(run), sorted(&["agent", "status", "outcome", "trigger", "at", "summary", "error", "refused", "minutes", "tokens"]), "{got}");
    assert_eq!((run["agent"].clone(), run["status"].clone(), run["trigger"].clone(), run["error"].clone(), run["refused"].clone()),
               (json!("Backend Agent"), json!("failed"), json!("routed"), json!("exit 1"), json!([])));
    assert!(run["at"].is_string(), "{run}");

    let list = t.ok("list_tasks", json!({"project": "KADE"})).await;
    assert_eq!(keys(&list), sorted(&["count", "shown", "tasks"]));
    assert_eq!((list["count"].clone(), list["shown"].clone()), (json!(1), json!(1)));
    let l = line(&list, &ident);
    assert_eq!(keys(l), sorted(&["task", "title", "project", "column", "priority", "assignee", "labels", "hold", "testing",
        "agent_minutes", "tokens"]), "{list}");
    assert_eq!((l["title"].clone(), l["project"].clone(), l["column"].clone()), (json!("Export invoices"), json!("Kade portal"), json!("In progress")));
    let archived = t.ok("list_tasks", json!({"archived": true})).await;
    let a = line(&archived, &old_id);
    assert_eq!(keys(a), sorted(&["task", "title", "project", "column", "priority", "assignee", "labels", "hold", "testing",
        "archived", "archived_on", "archived_by", "agent_minutes", "tokens"]), "{archived}");
    assert_eq!((a["archived"].clone(), a["column"].clone()), (json!(true), json!("Done")));

    let p = t.ok("get_project", json!({"project": "KADE"})).await;
    assert_eq!(keys(&p), sorted(&["project", "tasks_by_column", "time_tracking", "docs", "files"]));
    assert_eq!(keys(&p["project"]), sorted(&["id", "key", "number", "name", "client", "status", "goal_md", "repo_path", "default_branch",
        "color", "repository", "provider", "team_lead_may_merge", "test_runs_at_once", "team"]), "{p}");
    assert_eq!((p["project"]["key"].clone(), p["project"]["name"].clone()), (json!("KADE"), json!("Kade portal")));
    // tasks per column as before: the archived card isn't in it
    assert_eq!(p["tasks_by_column"], json!({"In progress": 1}), "{p}");
    assert_eq!((p["docs"].clone(), p["files"].clone()), (json!([]), json!([])));
    assert_eq!(keys(&p["time_tracking"]), sorted(&["cards_with_runs", "runs", "agent_minutes", "input_tokens", "output_tokens", "done_cards"]));
    assert_eq!(keys(&p["time_tracking"]["done_cards"]), sorted(&["count", "median_minutes", "p90_minutes"]));
}
