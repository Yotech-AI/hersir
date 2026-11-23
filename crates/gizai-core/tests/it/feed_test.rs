//! GA-344 QA: the change feed in core. `Db::change_seq` is the `seq` of the newest row in `changes`, moved on by every
//! write that logs a change and read again when the database opens; `feed::since` gives the counter now and the cards
//! that changed after a counter (their own row, a comment, a run, a file on the card or on one of its comments, and a
//! card that builds on a card whose row changed), the one changed last first; `board` for columns, labels and the like;
//! `reload` past the counter or past 200 cards. The writes that logged nothing before this card (a run's session,
//! commits, refusals, memory notes, screenshot note, verdict and an unstarted run) now move the counter and show their
//! card; a write that changes no card (a setting) leaves it.
use gizai_core::db::Db;
use gizai_core::feed::{self, Changes, MAX_CARDS};
use gizai_core::model::*;
use gizai_core::{builds_on, comments, files, ids, labels, memory, projects, runs, screenshots, seed, settings, tasks, team, workflow};

struct F {
    db: Db,
    you: String,
    team: String,
    be: String,
    project: String,
    tmp: tempfile::TempDir,
}

/// Project "Giz AI" (GA) with a Backend Agent, in memory.
fn board() -> F {
    let db = Db::open_in_memory().unwrap();
    let s = seed::ensure_seed(&db, "Jeffrey").unwrap();
    let project = projects::create(&db, &s.you_id, ProjectInput { name: "Giz AI".into(), key: "GA".into(), ..Default::default() }).unwrap();
    let be = team::add_agent(&db, &s.you_id, &s.team_id, AgentInput { name: "Backend Agent".into(), role_key: "backend".into(),
        ..Default::default() }).unwrap();
    F { db, you: s.you_id, team: s.team_id, be, project, tmp: tempfile::tempdir().unwrap() }
}

impl F {
    fn card(&self, title: &str) -> String {
        tasks::create(&self.db, &self.you, TaskInput { project_id: self.project.clone(), title: title.into(), ..Default::default() }).unwrap()
    }
    fn ident(&self, id: &str) -> String {
        tasks::get(&self.db, id).unwrap().identifier
    }
    fn since(&self, n: i64) -> Changes {
        feed::since(&self.db, Some(n)).unwrap()
    }
    /// The `seq` of the newest row in `changes`, as the table has it.
    fn newest(&self) -> i64 {
        self.db.read(|c| Ok(c.query_row("SELECT COALESCE(max(seq), 0) FROM changes", [], |r| r.get(0))?)).unwrap()
    }
    /// A run of the Backend Agent on the card, queued.
    fn run(&self, task: &str) -> String {
        runs::create(&self.db, &self.be, task, "backend", "S", "/tmp", "/tmp", "hersir/x", "/tmp/r.jsonl").unwrap()
    }
    fn state(&self, name: &str) -> String {
        team::get(&self.db, &self.team).unwrap().states.into_iter().find(|s| s.name == name).unwrap_or_else(|| panic!("no column {name}")).id
    }
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

#[test]
fn without_since_the_answer_is_only_the_counter_which_is_the_newest_change() {
    let f = board();
    f.card("Export invoices");
    let got = feed::since(&f.db, None).unwrap();
    assert!(got.counter > 0, "the seed and the card logged changes");
    assert_eq!(got, Changes { counter: f.newest(), tasks: vec![], board: false, reload: false });
    assert_eq!(f.db.change_seq(), f.newest());
}

#[test]
fn with_the_latest_counter_no_cards_come_back() {
    let f = board();
    let a = f.card("Export invoices");
    comments::add(&f.db, &f.you, &a, "A comment", None).unwrap();
    let now = feed::since(&f.db, None).unwrap().counter;
    assert_eq!(f.since(now), Changes { counter: now, tasks: vec![], board: false, reload: false });
    // the counter an answer gives, asked with again, gives nothing either
    let first = f.since(0);
    assert!(first.tasks.contains(&f.ident(&a)), "{first:?}");
    assert_eq!(f.since(first.counter), Changes { counter: first.counter, tasks: vec![], board: false, reload: false });
}

#[test]
fn a_card_write_moves_the_counter_and_shows_its_card_the_one_changed_last_first() {
    let f = board();
    let (a, b, c) = (f.card("A"), f.card("B"), f.card("C"));
    let start = f.db.change_seq();
    tasks::update(&f.db, &f.you, &a, TaskPatch { title: Some("A, renamed".into()), ..Default::default() }).unwrap();
    let after_a = f.db.change_seq();
    assert!(after_a > start);
    assert_eq!(f.since(start).tasks, vec![f.ident(&a)]);
    comments::add(&f.db, &f.you, &b, "A comment on B", None).unwrap();
    assert_eq!(f.since(after_a).tasks, vec![f.ident(&b)], "a comment shows its card");
    tasks::set_labels(&f.db, &f.you, &a, vec![team::get(&f.db, &f.team).unwrap().labels[0].id.clone()]).unwrap();
    let got = f.since(start);
    assert_eq!(got.tasks, vec![f.ident(&a), f.ident(&b)], "A changed last, each card once");
    assert!(!got.tasks.contains(&f.ident(&c)), "C didn't change");
    assert_eq!(got.counter, f.db.change_seq());
    assert_eq!(got.counter, f.newest());
    assert!(!got.board && !got.reload);
}

#[test]
fn a_file_on_a_card_or_on_one_of_its_comments_shows_the_card() {
    let f = board();
    let (a, b) = (f.card("A"), f.card("B"));
    let comment = comments::add(&f.db, &f.you, &b, "See the file", None).unwrap();
    let blob = files::store_bytes(f.tmp.path(), "notes.txt", b"hello").unwrap();
    let start = f.db.change_seq();
    f.db.write(Some(&f.you), |w| files::insert(w, Some(&f.you), "task", &a, &blob)).unwrap();
    assert_eq!(f.since(start).tasks, vec![f.ident(&a)]);
    let mid = f.db.change_seq();
    f.db.write(Some(&f.you), |w| files::insert(w, Some(&f.you), "comment", &comment, &blob)).unwrap();
    assert_eq!(f.since(mid).tasks, vec![f.ident(&b)]);
}

#[test]
fn a_card_that_builds_on_a_card_that_changed_shows_too() {
    let f = board();
    let (a, b, c) = (f.card("A"), f.card("B"), f.card("C"));
    builds_on::set(&f.db, &f.you, &b, std::slice::from_ref(&a)).unwrap();
    let start = f.db.change_seq();
    tasks::move_to(&f.db, &f.you, &a, &f.state("Done"), "").unwrap();
    let got = f.since(start);
    assert_eq!(sorted(got.tasks), sorted(vec![f.ident(&a), f.ident(&b)]), "B waits for A");
    // a change on B doesn't name A
    let mid = f.db.change_seq();
    comments::add(&f.db, &f.you, &b, "On B", None).unwrap();
    assert_eq!(f.since(mid).tasks, vec![f.ident(&b)]);
    let _ = c;
}

#[test]
fn the_writes_that_logged_nothing_before_now_move_the_counter_and_show_their_card() {
    let f = board();
    let (a, other) = (f.card("A"), f.card("B"));
    let run = f.run(&a);
    let refusal = Refusal { tool: "Bash".into(), input: "rm -rf build".into(), reason: "not allowed".into() };
    let given = memory::Given { path: "Projects/GA".into(), chars: 120, shown: 120 };
    let steps: Vec<(&str, Box<dyn Fn() + '_>)> = vec![
        ("runs::set_session", Box::new(|| runs::set_session(&f.db, &run, "session-2").unwrap())),
        ("runs::set_base_sha", Box::new(|| runs::set_base_sha(&f.db, &run, "1111111").unwrap())),
        ("runs::set_head_sha", Box::new(|| runs::set_head_sha(&f.db, &run, "2222222").unwrap())),
        ("runs::set_refused", Box::new(|| runs::set_refused(&f.db, &run, std::slice::from_ref(&refusal)).unwrap())),
        ("memory::record_given", Box::new(|| memory::record_given(&f.db, &run, std::slice::from_ref(&given)).unwrap())),
        ("screenshots::begin", Box::new(|| screenshots::begin(&f.db, &run).unwrap())),
        ("screenshots::set_left_out", Box::new(|| screenshots::set_left_out(&f.db, &run, &["/settings".into()]).unwrap())),
        ("screenshots::set_preview_why", Box::new(|| screenshots::set_preview_why(&f.db, &run, "no preview command").unwrap())),
        ("screenshots::set_preview_pid", Box::new(|| screenshots::set_preview_pid(&f.db, &run, Some(4242)).unwrap())),
    ];
    for (what, step) in &steps {
        let before = f.db.change_seq();
        step();
        assert!(f.db.change_seq() > before, "{what} moves the counter");
        let got = f.since(before);
        assert_eq!(got.tasks, vec![f.ident(&a)], "{what} shows its card, and only it");
        assert_eq!(got.counter, f.newest(), "{what}");
    }
    // housekeeping clears the run's screenshot set: its card changed too
    let before = f.db.change_seq();
    let pruned = screenshots::prune(&f.db, ids::now_ms(), 0, 0).unwrap();
    assert_eq!(pruned.sets, 1);
    assert_eq!(f.since(before).tasks, vec![f.ident(&a)], "screenshots::prune");
    // the run's verdict
    let before = f.db.change_seq();
    workflow::apply_outcome(&f.db, &run, Some(&Outcome { outcome: "needs_decision".into(), summary: "Which CSV dialect?".into(),
        issues: vec![] })).unwrap();
    assert_eq!(f.since(before).tasks, vec![f.ident(&a)], "workflow::apply_outcome");
    assert!(f.db.read(|c| Ok(c.query_row("SELECT count(*) FROM changes WHERE table_name='runs' AND row_id=?1 AND seq > ?2",
        rusqlite::params![run, before], |r| r.get::<_, i64>(0))?)).unwrap() > 0, "the verdict is logged on the run itself");
    // a run that never started: its claim comes off, with or without a hold
    let unstarted = f.run(&other);
    let before = f.db.change_seq();
    workflow::release_unstarted(&f.db, &unstarted).unwrap();
    assert!(f.db.change_seq() > before, "workflow::release_unstarted moves the counter");
    assert_eq!(f.since(before).tasks, vec![f.ident(&other)], "workflow::release_unstarted");
    let held = f.run(&other);
    let before = f.db.change_seq();
    workflow::hold_unstarted(&f.db, &held, "No worktree").unwrap();
    assert_eq!(f.since(before).tasks, vec![f.ident(&other)], "workflow::hold_unstarted");
}

#[test]
fn a_write_on_a_run_that_isnt_there_logs_nothing() {
    let f = board();
    f.card("A");
    let before = f.db.change_seq();
    runs::set_session(&f.db, "no-such-run", "s").unwrap();
    runs::set_base_sha(&f.db, "no-such-run", "1").unwrap();
    runs::set_head_sha(&f.db, "no-such-run", "2").unwrap();
    memory::record_given(&f.db, "no-such-run", &[]).unwrap();
    assert_eq!(f.db.change_seq(), before);
    assert_eq!(f.newest(), before);
}

#[test]
fn a_write_that_changes_no_card_leaves_the_counter_where_it_is() {
    let f = board();
    f.card("A");
    let before = f.db.change_seq();
    settings::set(&f.db, "ga344_qa", &"on").unwrap();
    assert_eq!(f.db.change_seq(), before);
    assert_eq!(f.since(before), Changes { counter: before, tasks: vec![], board: false, reload: false });
}

#[test]
fn a_change_to_labels_or_columns_says_board_and_names_no_card() {
    let f = board();
    let a = f.card("A");
    let before = f.db.change_seq();
    labels::save(&f.db, &f.you, None, "urgent", None).unwrap();
    let got = f.since(before);
    assert_eq!((got.tasks.is_empty(), got.board, got.reload), (true, true, false), "{got:?}");
    let before = f.db.change_seq();
    team::add_state(&f.db, &f.you, &f.team, "Cancelled", &f.state("Done"), "cancelled").unwrap();
    assert!(f.since(before).board, "a new column");
    // a card's own change alone isn't the board's
    let before = f.db.change_seq();
    comments::add(&f.db, &f.you, &a, "Only the card", None).unwrap();
    let got = f.since(before);
    assert_eq!((got.tasks, got.board), (vec![f.ident(&a)], false));
}

#[test]
fn since_past_the_counter_says_reload_with_the_counter_now_and_below_zero_counts_from_the_start() {
    let f = board();
    let a = f.card("A");
    let now = f.db.change_seq();
    assert_eq!(f.since(now + 10), Changes { counter: now, tasks: vec![], board: false, reload: true });
    let all = f.since(-5);
    assert_eq!(all, f.since(0));
    assert!(all.tasks.contains(&f.ident(&a)) && !all.reload);
}

#[test]
fn more_than_200_changed_cards_says_reload_instead_of_naming_them() {
    let f = board();
    let start = f.db.change_seq();
    for i in 0..MAX_CARDS {
        f.card(&format!("Card {i}"));
    }
    let got = f.since(start);
    assert_eq!((got.tasks.len(), got.reload), (MAX_CARDS, false));
    let mid = f.db.change_seq();
    let last = f.card("One more");
    let got = f.since(start);
    assert_eq!((got.tasks.len(), got.reload, got.counter), (0, true, f.newest()), "201 cards");
    // from a later counter, only the last one
    assert_eq!(f.since(mid).tasks, vec![f.ident(&last)]);
}

#[test]
fn a_logged_change_a_savepoint_rolled_back_doesnt_push_the_counter_past_the_table() {
    let f = board();
    let a = f.card("A");
    let before = f.db.change_seq();
    f.db.write(Some(&f.you), |w| {
        w.conn().execute_batch("SAVEPOINT qa")?;
        w.update("tasks", &a, serde_json::json!({"title": "never"}))?;
        w.conn().execute_batch("ROLLBACK TO qa; RELEASE qa")?;
        Ok(())
    }).unwrap();
    assert_eq!(f.db.change_seq(), before);
    assert_eq!(f.newest(), before);
    // a bulk move that skips a card ends with the counter at the table's newest change, with both moved cards
    let b = f.card("B");
    let before = f.db.change_seq();
    let r = tasks::move_many(&f.db, &f.you, &[a.clone(), "no-such-card".into(), b.clone()], &f.state("To do"), &[]).unwrap();
    assert_eq!((r.done.len(), r.skipped.len()), (2, 1), "{r:?}");
    assert_eq!(f.db.change_seq(), f.newest());
    assert_eq!(sorted(f.since(before).tasks), sorted(vec![f.ident(&a), f.ident(&b)]));
}

#[test]
fn the_counter_is_read_again_when_the_database_file_opens() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("gizai.db");
    let (counter, card) = {
        let db = Db::open(&path).unwrap();
        let s = seed::ensure_seed(&db, "Jeffrey").unwrap();
        let p = projects::create(&db, &s.you_id, ProjectInput { name: "Giz AI".into(), key: "GA".into(), ..Default::default() }).unwrap();
        let t = tasks::create(&db, &s.you_id, TaskInput { project_id: p, title: "A".into(), ..Default::default() }).unwrap();
        let start = db.change_seq();
        comments::add(&db, &s.you_id, &t, "A comment", None).unwrap();
        // on a file, the read-only connections see the write the counter counts
        assert_eq!(feed::since(&db, Some(start)).unwrap().tasks, vec![tasks::get(&db, &t).unwrap().identifier]);
        (db.change_seq(), t)
    };
    assert!(counter > 0);
    let db = Db::open(&path).unwrap();
    assert_eq!(db.change_seq(), counter, "the same counter after a restart");
    assert_eq!(feed::since(&db, Some(counter)).unwrap().tasks, Vec::<String>::new());
    assert_eq!(feed::since(&db, Some(counter - 1)).unwrap().tasks, vec![tasks::get(&db, &card).unwrap().identifier]);
}
