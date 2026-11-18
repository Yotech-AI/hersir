//! GA-306 QA: moving, archiving and restoring several cards at once (`tasks::move_many`, `archive_many`,
//! `restore_many`), the board's Move all, Archive all and a selection's Move to and Archive, and their Undo. Each is one
//! write: the cards go in the order given, each with the activity its single command writes; a card that can't go is
//! skipped with the reason its single command gives, writes nothing, and the others go ahead.
use gizai_core::{Error, db::Db, model::*, projects, runs, seed::ensure_seed, tasks, team};

struct B { db: Db, you: String, project: String }

fn board() -> B {
    let db = Db::open_in_memory().unwrap();
    let s = ensure_seed(&db, "Jeffrey").unwrap();
    let project = projects::create(&db, &s.you_id, ProjectInput { name: "Kade portal".into(), key: "KADE".into(),
        repo_path: Some("/home/j/Code/kade".into()), ..Default::default() }).unwrap();
    B { db, you: s.you_id, project }
}

impl B {
    fn state(&self, name: &str) -> String {
        self.db.read(|c| Ok(c.query_row("SELECT id FROM workflow_states WHERE name=?1 ORDER BY created_at LIMIT 1", [name], |r| r.get(0))?)).unwrap()
    }
    fn card(&self, column: &str, title: &str) -> String {
        tasks::create(&self.db, &self.you, TaskInput { project_id: self.project.clone(), title: title.into(), state_id: Some(self.state(column)),
            ..Default::default() }).unwrap()
    }
    fn cards(&self, column: &str, n: usize) -> Vec<String> {
        (0..n).map(|i| self.card(column, &format!("Card {i}"))).collect()
    }
    fn task(&self, id: &str) -> Task { tasks::get(&self.db, id).unwrap() }
    fn ident(&self, id: &str) -> String { self.task(id).identifier }
    /// The column's cards on the board, top to bottom.
    fn column(&self, name: &str) -> Vec<String> {
        let state = self.state(name);
        self.db.read(|c| {
            let mut st = c.prepare("SELECT id FROM tasks WHERE state_id=?1 AND deleted_at IS NULL ORDER BY sort_key")?;
            let ids = st.query_map([&state], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(ids)
        }).unwrap()
    }
    fn completed_at(&self, id: &str) -> Option<i64> {
        self.db.read(|c| Ok(c.query_row("SELECT completed_at FROM tasks WHERE id=?1", [id], |r| r.get(0))?)).unwrap()
    }
    fn deleted_at(&self, id: &str) -> Option<i64> {
        self.db.read(|c| Ok(c.query_row("SELECT deleted_at FROM tasks WHERE id=?1", [id], |r| r.get(0))?)).unwrap()
    }
    /// Every row in the change log (the activity and what sync sends).
    fn changes(&self) -> i64 {
        self.db.read(|c| Ok(c.query_row("SELECT count(*) FROM changes", [], |r| r.get(0))?)).unwrap()
    }
    fn activity(&self, id: &str) -> Vec<ChangeEntry> { tasks::activity(&self.db, id).unwrap() }
    fn last(&self, id: &str) -> ChangeEntry { self.activity(id).pop().unwrap() }
    fn skip(&self, id: &str, reason: &str) -> Skipped { Skipped { identifier: self.ident(id), reason: reason.into() } }
}

fn s(ids: &[&String]) -> Vec<String> { ids.iter().map(|x| x.to_string()).collect() }

const WORKING: &str = "An agent is working on this card";

// ---- move_many ----

#[test]
fn move_many_puts_the_cards_at_the_bottom_of_done_in_the_order_given_with_completed_at_and_each_cards_activity() {
    let b = board();
    let done_before = b.cards("Done", 2);
    let deploy = b.cards("Deploy", 4);
    let (a, c, d, e) = (&deploy[0], &deploy[1], &deploy[2], &deploy[3]);
    for x in &deploy { assert_eq!(b.completed_at(x), None); }
    let changes = b.changes();

    // not in board order: the order given is the order they land in; `c` stays in Deploy
    let r = tasks::move_many(&b.db, &b.you, &s(&[d, a, e]), &b.state("Done"), &[]).unwrap();
    assert_eq!(r, BulkResult { done: s(&[d, a, e]), skipped: vec![] });
    let mut want = done_before.clone();
    want.extend(s(&[d, a, e]));
    assert_eq!(b.column("Done"), want, "at the bottom of Done, in the order given, under the cards that were there");
    assert_eq!(b.column("Deploy"), s(&[c]), "the card not given stays");
    for x in [d, a, e] {
        let t = b.task(x);
        assert_eq!((t.state_name.as_str(), t.state_category.as_str()), ("Done", "done"));
        assert!(b.completed_at(x).is_some(), "completed_at is set in Done");
        let last = b.last(x);
        assert_eq!((last.table.as_str(), last.op.as_str(), last.actor_name.as_deref(), &last.diff),
                   ("tasks", "update", Some("Jeffrey"), &serde_json::json!({"column": ["Deploy", "Done"]})),
                   "the activity says the column change and who made it, as a single move's does");
    }
    assert_eq!(b.completed_at(c), None);
    assert_eq!(b.changes() - changes, 3, "one activity entry per card moved, nothing else");
}

#[test]
fn move_many_back_to_deploy_is_the_undo_the_cards_return_to_the_bottom_of_deploy_in_their_order_without_completed_at() {
    let b = board();
    let deploy = b.cards("Deploy", 3);
    let stays = b.card("Deploy", "Stays");
    let moving = s(&[&deploy[0], &deploy[1], &deploy[2]]);
    tasks::move_many(&b.db, &b.you, &moving, &b.state("Done"), &[]).unwrap();
    assert_eq!(b.column("Deploy"), [stays.clone()]);

    let r = tasks::move_many(&b.db, &b.you, &moving, &b.state("Deploy"), &[]).unwrap();
    assert_eq!(r.done, moving);
    let mut want = vec![stays.clone()];
    want.extend(moving.clone());
    assert_eq!(b.column("Deploy"), want, "back at the bottom of Deploy, in their order");
    for x in &moving {
        assert_eq!(b.completed_at(x), None, "out of Done, completed_at is cleared again");
        assert_eq!(b.last(x).diff, serde_json::json!({"column": ["Done", "Deploy"]}));
    }
    assert!(b.column("Done").is_empty());
}

#[test]
fn move_many_skips_a_card_an_agent_is_working_on_and_moves_the_others() {
    let b = board();
    let deploy = b.cards("Deploy", 3);
    let busy = &deploy[1];
    let before = b.task(busy);
    let busy_activity = b.activity(busy).len();

    let r = tasks::move_many(&b.db, &b.you, &deploy, &b.state("Done"), &[busy.clone()]).unwrap();
    assert_eq!(r, BulkResult { done: s(&[&deploy[0], &deploy[2]]), skipped: vec![b.skip(busy, WORKING)] });
    assert_eq!(r.skipped[0].identifier, "KADE-2", "named by its identifier");
    assert_eq!(b.column("Done"), s(&[&deploy[0], &deploy[2]]));
    let after = b.task(busy);
    assert_eq!((after.state_name.as_str(), after.sort_key.as_str()), ("Deploy", before.sort_key.as_str()), "it stays where it was");
    assert_eq!(b.completed_at(busy), None);
    assert_eq!(b.activity(busy).len(), busy_activity, "and writes nothing");
}

#[test]
fn move_many_skips_an_archived_card_a_missing_one_and_a_column_of_another_team_with_their_single_move_reasons() {
    let b = board();
    let deploy = b.cards("Deploy", 2);
    let archived = b.card("Done", "Archived");
    tasks::archive(&b.db, &b.you, &archived).unwrap();
    let changes = b.changes();

    let ids = vec![deploy[0].clone(), archived.clone(), "no-such-card".to_string(), deploy[1].clone()];
    let r = tasks::move_many(&b.db, &b.you, &ids, &b.state("Done"), &[]).unwrap();
    assert_eq!(r.done, s(&[&deploy[0], &deploy[1]]), "the others go ahead");
    assert_eq!(r.skipped, vec![
        b.skip(&archived, "KADE-3 is archived: restore it first"),
        Skipped { identifier: "no-such-card".into(), reason: "not found: task no-such-card".into() },
    ]);
    assert!(b.deleted_at(&archived).is_some(), "the archived card stays archived");
    assert_eq!(b.changes() - changes, 2, "only the moved cards wrote");
    let single = tasks::move_to(&b.db, &b.you, &archived, &b.state("Deploy"), "");
    assert!(matches!(&single, Err(Error::Invalid(m)) if m == &r.skipped[0].reason), "the same reason as a single move: {single:?}");

    // a column of another team: every card is skipped, nothing moves
    let other = team::add_team(&b.db, &b.you, "Second team").unwrap();
    let theirs = team::get(&b.db, &other).unwrap().states.into_iter().find(|x| x.category == "done").unwrap().id;
    let r = tasks::move_many(&b.db, &b.you, &s(&[&deploy[0]]), &theirs, &[]).unwrap();
    assert_eq!(r, BulkResult { done: vec![], skipped: vec![b.skip(&deploy[0], "that column doesn't belong to this project's team")] });
    assert_eq!(b.task(&deploy[0]).state_name, "Done");
}

#[test]
fn move_many_counts_a_card_given_twice_once() {
    let b = board();
    let deploy = b.cards("Deploy", 2);
    let ids = vec![deploy[0].clone(), deploy[1].clone(), deploy[0].clone()];
    let r = tasks::move_many(&b.db, &b.you, &ids, &b.state("Done"), &[]).unwrap();
    assert_eq!(r, BulkResult { done: s(&[&deploy[0], &deploy[1]]), skipped: vec![] });
    assert_eq!(b.column("Done"), s(&[&deploy[0], &deploy[1]]), "its first place counts");
    assert_eq!(b.activity(&deploy[0]).iter().filter(|x| x.diff.get("column").is_some()).count(), 1);
}

#[test]
fn move_many_by_a_person_into_to_do_answers_a_hold_as_a_single_move_does() {
    let b = board();
    let t = b.card("Review", "On hold");
    tasks::update(&b.db, &b.you, &t, TaskPatch { hold: Some("blocked".into()), hold_reason: Some("Which API?".into()), ..Default::default() }).unwrap();
    assert!(b.task(&t).hold.is_some());
    tasks::move_many(&b.db, &b.you, &s(&[&t]), &b.state("To do"), &[]).unwrap();
    let after = b.task(&t);
    assert_eq!((after.state_name.as_str(), after.hold.as_deref()), ("To do", None));
    assert_eq!(b.last(&t).diff, serde_json::json!({"hold": null, "released": "moved by hand"}));
}

// ---- archive_many ----

#[test]
fn archive_many_archives_each_card_in_done_says_who_and_skips_the_rest_with_why() {
    let b = board();
    let be = team::add_agent(&b.db, &b.you, &team::list(&b.db).unwrap()[0].id,
        AgentInput { name: "Backend Agent".into(), role_key: "backend".into(), ..Default::default() }).unwrap();
    let done = b.cards("Done", 4);
    let review = b.card("Review", "In review");
    let deploy = b.card("Deploy", "In deploy");
    let gone = b.card("Done", "Archived before");
    tasks::archive(&b.db, &b.you, &gone).unwrap();
    // a live run the app doesn't know of yet (the database's own check) and a card the app says an agent works on
    let live = &done[1];
    runs::create(&b.db, &be, live, "backend", "S", "/tmp", "/tmp", "gizai/x", "/tmp/r.jsonl").unwrap();
    let working = &done[2];
    let changes = b.changes();

    let ids = vec![done[0].clone(), review.clone(), live.clone(), working.clone(), deploy.clone(), gone.clone(), done[3].clone()];
    let r = tasks::archive_many(&b.db, &b.you, &ids, &[working.clone()]).unwrap();
    assert_eq!(r.done, s(&[&done[0], &done[3]]), "the others go ahead");
    assert_eq!(r.skipped, vec![
        b.skip(&review, "Only a card in Done can be archived, and KADE-5 is in Review"),
        b.skip(live, WORKING),
        b.skip(working, WORKING),
        b.skip(&deploy, "Only a card in Done can be archived, and KADE-6 is in Deploy"),
        b.skip(&gone, "KADE-7 is already archived"),
    ]);
    for x in [&done[0], &done[3]] {
        assert!(b.deleted_at(x).is_some());
        assert_eq!(b.task(x).archived_by.as_deref(), Some("Jeffrey"));
        let last = b.last(x);
        assert_eq!((last.table.as_str(), last.op.as_str(), last.actor_name.as_deref()), ("tasks", "delete", Some("Jeffrey")),
                   "the activity says who archived it, as a single archive's does");
    }
    for x in [&review, live, working, &deploy] {
        assert_eq!(b.deleted_at(x), None, "a skipped card stays on the board");
    }
    assert_eq!(b.column("Done"), s(&[live, working]));
    assert_eq!(b.changes() - changes, 2, "only the archived cards wrote");
    let archived: Vec<String> = tasks::archived(&b.db, None).unwrap().into_iter().map(|t| t.id).collect();
    assert!(archived.contains(&done[0]) && archived.contains(&done[3]), "they are in the bin");
}

// ---- restore_many ----

#[test]
fn restore_many_puts_the_cards_back_at_the_bottom_of_done_in_the_order_given_and_skips_one_that_isnt_archived() {
    let b = board();
    let done = b.cards("Done", 3);
    let kept = b.card("Done", "Kept");
    tasks::archive_many(&b.db, &b.you, &done, &[]).unwrap();
    let later = b.card("Done", "Added while they were archived");
    let changes = b.changes();

    let ids = vec![done[2].clone(), kept.clone(), done[0].clone(), done[1].clone()];
    let r = tasks::restore_many(&b.db, &b.you, &ids).unwrap();
    assert_eq!(r.done, s(&[&done[2], &done[0], &done[1]]));
    assert_eq!(r.skipped, vec![b.skip(&kept, "KADE-4 isn't archived")]);
    assert_eq!(b.column("Done"), s(&[&kept, &later, &done[2], &done[0], &done[1]]), "at the bottom of Done, in the order given");
    for x in &done {
        let t = b.task(x);
        assert_eq!((t.archived_at, t.archived_by.as_deref(), t.state_name.as_str()), (None, None, "Done"));
        let last = b.last(x);
        assert_eq!((last.op.as_str(), last.actor_name.as_deref(), &last.diff), ("update", Some("Jeffrey"), &serde_json::json!({"archived": false})),
                   "the activity says who restored it");
    }
    assert_eq!(b.changes() - changes, 3);
}

// ---- one write each ----

#[test]
fn each_bulk_command_is_one_write_a_failure_partway_leaves_every_card_as_it_was() {
    let b = board();
    let deploy = b.cards("Deploy", 4);
    let done = b.cards("Done", 4);
    // the third card's write fails as a disk would (not a refusal): the whole write is undone
    let fail_on = |id: &str| b.db.write(None, |w| {
        w.conn().execute_batch(&format!(
            "CREATE TRIGGER qa_bulk BEFORE UPDATE ON tasks WHEN NEW.id = '{id}' BEGIN SELECT RAISE(ABORT, 'qa: disk full'); END;"))?;
        Ok(())
    }).unwrap();
    let unfail = || b.db.write(None, |w| { w.conn().execute_batch("DROP TRIGGER qa_bulk;")?; Ok(()) }).unwrap();
    let changes = b.changes();

    fail_on(&deploy[2]);
    let e = tasks::move_many(&b.db, &b.you, &deploy, &b.state("Done"), &[]).unwrap_err().to_string();
    assert!(e.contains("qa: disk full"), "{e}");
    assert_eq!(b.column("Deploy"), deploy, "the first two cards didn't move either");
    assert_eq!(b.column("Done"), done);
    unfail();

    fail_on(&done[2]);
    assert!(tasks::archive_many(&b.db, &b.you, &done, &[]).is_err());
    assert!(done.iter().all(|x| b.deleted_at(x).is_none()), "no card was archived");
    unfail();
    assert_eq!(b.changes(), changes, "and nothing is in the change log");

    tasks::archive_many(&b.db, &b.you, &done, &[]).unwrap();
    fail_on(&done[2]);
    assert!(tasks::restore_many(&b.db, &b.you, &done).is_err());
    assert!(done.iter().all(|x| b.deleted_at(x).is_some()), "no card was restored");
    unfail();
    assert_eq!(tasks::restore_many(&b.db, &b.you, &done).unwrap().done, done);
}

#[test]
fn the_answer_is_done_and_skipped_as_the_board_reads_it() {
    let r = BulkResult { done: vec!["t1".into()], skipped: vec![Skipped { identifier: "GA-303".into(), reason: WORKING.into() }] };
    assert_eq!(serde_json::to_value(&r).unwrap(),
               serde_json::json!({"done": ["t1"], "skipped": [{"identifier": "GA-303", "reason": "An agent is working on this card"}]}));
    assert_eq!(tasks::WORKING, WORKING, "the reason a single archive gives");
}
