//! GA-369 QA, end to end with a fake `claude`: no new run from the queue while this computer is busy.
//! - CPU pressure over 50% (a fake reading, as the start check reads it): the card the queue would start waits, its
//!   activity says "Waiting: this computer is busy (…)" once per busy spell, by Hersir; it starts by itself at the first
//!   pull once the pressure eases. A later busy spell says so again. The load average reads the same way.
//! - Meanwhile check_board's slots say why (computer_busy) with no free slot, and the board check has no free_slot
//!   finding, but a busy one once the wait passes an hour.
//! - Your own Run isn't held. (A chat answer isn't either: chat_flow_test.rs.)
//! - A test's Hersir never reads the test machine's CPU: without a fake reading the queue starts as always.
// Linux and macOS only: these tests run shell scripts as fake programs, which Windows can't start.
#![cfg(unix)]
use std::path::{Path, PathBuf};
use std::time::Duration;

use gizai_agents::run_cpu::Reading;
use gizai_core::model::*;
use gizai_core::{projects, runs as core_runs, settings, tasks, team};
use gizai_lib::{AppState, runs};

const FAKE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/gizai-agents/tests/fake-claude.sh");
const BUSY_72: &str = "Waiting: this computer is busy (CPU pressure 72% over the last minute). The run starts once that eases.";

fn git_repo(dir: &Path, name: &str) -> PathBuf {
    let repo = dir.join(name);
    std::fs::create_dir(&repo).unwrap();
    for a in [&["init", "-q", "-b", "main"][..], &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "i"][..]] {
        assert!(std::process::Command::new("git").args(a).current_dir(&repo).status().unwrap().success());
    }
    repo
}

struct App { st: AppState, ga: String, _tmp: tempfile::TempDir }

/// Project GA with a local git repository, the QA Agent with three cards at once on Testing (Auto), "Runs at once" 10.
fn app() -> App {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    settings::set(&st.db, "claude_bin", &FAKE.to_string()).unwrap();
    settings::set(&st.db, "max_concurrent_runs", &10u32).unwrap();
    let ga = projects::create(&st.db, &st.you_id, ProjectInput { name: "Giz AI".into(), key: "GA".into(),
        repo_path: Some(git_repo(tmp.path(), "ga").to_string_lossy().into_owned()), ..Default::default() }).unwrap();
    let team_id = team::list(&st.db).unwrap()[0].id.clone();
    team::add_agent(&st.db, &st.you_id, &team_id, AgentInput { name: "QA Agent".into(), role_key: "qa".into(), max_runs: Some(3),
        ..Default::default() }).unwrap();
    App { st, ga, _tmp: tmp }
}

impl App {
    fn card(&self, priority: i64) -> String {
        let team = team::get(&self.st.db, &team::list(&self.st.db).unwrap()[0].id).unwrap();
        let testing = team.states.iter().find(|s| s.name == "Testing").unwrap().id.clone();
        tasks::create(&self.st.db, &self.st.you_id, TaskInput { project_id: self.ga.clone(), title: format!("A card {priority}"),
            description_md: "FAKE_HANG".into(), state_id: Some(testing), priority, ..Default::default() }).unwrap()
    }
    /// What Hersir noted in the card's activity.
    fn notes(&self, t: &str) -> Vec<(Option<String>, String)> {
        tasks::activity(&self.st.db, t).unwrap().into_iter()
            .filter_map(|e| e.diff.get("cleanup").and_then(|c| c.as_str()).map(|c| (e.actor_name.clone(), c.to_string()))).collect()
    }
    fn runs_of(&self, t: &str) -> usize { core_runs::list_for_task(&self.st.db, t).unwrap().len() }
    fn live(&self, t: &str) -> bool { runs::live(&self.st).iter().any(|l| l.task_id == t) }
    async fn pull(&self) -> Vec<String> { runs::pull(&self.st).await.into_iter().map(|(t, _)| t).collect() }
    fn busy(&self, r: Option<Reading>) { self.st.run_memory.fake_cpu_reading(r); }
    /// The board check's findings for card `t`, `at` ms.
    fn findings(&self, t: &str, at: i64) -> Vec<(String, String, String)> {
        gizai_lib::board::findings(&self.st, at).unwrap().into_iter().filter(|f| f.task_id == t).map(|f| (f.kind, f.code, f.reason)).collect()
    }
}

#[tokio::test]
async fn a_card_the_queue_would_start_waits_while_this_computer_is_busy_says_so_once_and_starts_once_it_eases() {
    let a = app();
    let c1 = a.card(1);
    a.busy(Some(Reading::Pressure(72.0)));
    assert!(a.pull().await.is_empty(), "no new run while the computer is busy");
    assert_eq!(a.runs_of(&c1), 0);
    let t = tasks::get(&a.st.db, &c1).unwrap();
    assert_eq!((t.state_name.as_str(), t.hold.as_deref()), ("Testing", None), "it only waits");
    assert_eq!(a.notes(&c1), [(None, BUSY_72.to_string())], "its activity says so, as Hersir");
    // the next pulls (a run's end, the minute's) start nothing either, and say it once
    assert!(a.pull().await.is_empty());
    assert!(a.pull().await.is_empty());
    assert_eq!(a.notes(&c1).len(), 1, "once per busy spell: {:?}", a.notes(&c1));

    // check_board: why, and no free slot
    let slots = gizai_lib::board::slots_json(&a.st);
    assert_eq!(slots["computer_busy"], BUSY_72, "{slots}");
    assert_eq!(slots["runs_at_once"]["free"], 0, "{slots}");
    assert_eq!(slots["runs_at_once"]["max"], 10, "{slots}");
    // the board check: no free_slot finding meanwhile, a busy one after an hour
    let now = gizai_core::ids::now_ms();
    assert!(a.findings(&c1, now + 5 * 60_000).is_empty(), "{:?}", a.findings(&c1, now + 5 * 60_000));
    assert_eq!(a.findings(&c1, now + 61 * 60_000), [("waiting".to_string(), "busy".to_string(),
        "Hersir has held new runs for over an hour: this computer is busy (CPU pressure 72% over the last minute). The run starts once that eases.".to_string())]);

    // your own Run isn't held
    let c2 = a.card(2);
    runs::start(&a.st, &c2, None, Some(FAKE.into()), "manual").await.unwrap();
    assert!(a.live(&c2), "Run starts it while the computer is busy");
    assert!(a.notes(&c2).is_empty());

    // the pressure eases: the next pull starts the card that waited, and nothing says busy any more
    a.busy(Some(Reading::Pressure(12.0)));
    assert_eq!(a.pull().await, [c1.clone()]);
    assert!(a.live(&c1));
    let slots = gizai_lib::board::slots_json(&a.st);
    assert!(slots["computer_busy"].is_null(), "{slots}");
    assert_eq!(slots["runs_at_once"]["free"], 8, "{slots}");
    assert!(a.findings(&c1, now + 61 * 60_000).is_empty(), "it runs");
    assert_eq!(a.notes(&c1).len(), 1, "it was said once");

    // a busy spell later (the load average where there is no CPU pressure) says so again, for the card that waits then
    let c3 = a.card(3);
    a.busy(Some(Reading::Load { avg: 38.2, cores: 16 }));
    assert!(a.pull().await.is_empty());
    assert_eq!(a.notes(&c3), [(None,
        "Waiting: this computer is busy (a load of 38.2 on 16 cores over the last minute). The run starts once that eases.".to_string())]);
    a.busy(Some(Reading::Pressure(0.5)));
    assert_eq!(a.pull().await, [c3.clone()]);
    runs::stop_all(&a.st, Duration::from_secs(12)).await;
}

#[tokio::test]
async fn at_50_percent_or_without_a_reading_the_queue_starts_as_always_and_a_tests_hersir_never_reads_the_test_machine() {
    let a = app();
    let c1 = a.card(1);
    // a test's Hersir has no reading (it never reads this machine's CPU), so its queue never waits for it
    assert_eq!(a.st.run_memory.cpu_reading(), None);
    a.busy(Some(Reading::Pressure(50.0)));
    assert_eq!(a.pull().await, [c1.clone()], "busy only over 50%");
    a.busy(None);
    let c2 = a.card(2);
    assert_eq!(a.pull().await, [c2.clone()]);
    assert!(a.notes(&c1).is_empty() && a.notes(&c2).is_empty());
    assert!(gizai_lib::board::slots_json(&a.st)["computer_busy"].is_null());
    runs::stop_all(&a.st, Duration::from_secs(12)).await;
}
