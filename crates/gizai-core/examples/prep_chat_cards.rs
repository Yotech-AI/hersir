//! Adds what the UI test of cards beside the chat (GA-297) needs to data prep_chats made: "Check KADE-1 and GFW-2", the newest
//! chat, a question the Chief started about KADE-1 and GFW-2 (ChatThread.tasks), with an answer whose chips link KADE-3,
//! KADE-1 and the project KADE. A paused QA Agent. KADE-1 (In progress): a long description, five acceptance criteria (two
//! checked), a hold (Needs your decision) with a reason longer than 60 characters that its last comment repeats, you as its
//! assignee, three comments and two runs. GFW-2 (Review): an open pull request, QA's run that passed and its hand-over as a
//! comment. Prints the chat's id.
//! usage: cargo run -p gizai-core --example prep_chat_cards -- <data dir>
use gizai_core::{chat, comments, db::Db, ids, model::*, runs, seed::ensure_seed, tasks, team};

const REASON: &str = "Which export format do you want for the invoices: CSV with semicolons, or the Exact Online import file?";

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let db = Db::open(&std::path::Path::new(&a[1]).join("gizai.db")).unwrap();
    let s = ensure_seed(&db, "Jeffrey").unwrap();
    let you = s.you_id.clone();
    let lead = team::chat_agent(&db).unwrap().expect("prep_chats' Chief").actor_id;
    let qa = team::add_agent(&db, &you, &s.team_id, AgentInput { name: "QA Agent".into(), role_key: "qa".into(), ..Default::default() }).unwrap();
    team::set_agent_status(&db, &you, &qa, "paused").unwrap();
    let k1 = tasks::id_of(&db, "KADE-1").unwrap();
    let g2 = tasks::id_of(&db, "GFW-2").unwrap();

    // KADE-1: the fields, folded text, criteria, comments and runs
    let description = (1..=8).map(|n| format!("Paragraph {n} of the export: the portal lists the invoices of a month and exports them.")).collect::<Vec<_>>().join("\n\n");
    let acceptance = "- [x] The Export button is on the invoices page.\n- [ ] The file has one row per invoice.\n- [x] Amounts use a comma.\n\
                      - [ ] The file name has the month.\n- [ ] An empty month exports a header only.";
    tasks::update(&db, &you, &k1, TaskPatch { description_md: Some(description), acceptance_md: Some(acceptance.into()), assignee_id: Some(you.clone()),
                                              ..Default::default() }).unwrap();
    let run = |task: &str, agent: &str, role: &str, outcome: &str, summary: &str| {
        let r = runs::create(&db, agent, task, role, &ids::new_id(), "/tmp", "/tmp", "b", "/dev/null").unwrap();
        runs::finish(&db, &r, "succeeded", Some(&Outcome { outcome: outcome.into(), summary: summary.into(), issues: vec![] }), 0, 0, 0, None).unwrap();
        r
    };
    run(&k1, &qa, "qa", "qa_fail", "The CSV export misses the VAT column.");
    comments::add(&db, &you, &k1, "First comment: the export should follow the bookkeeper's sheet.", None).unwrap();
    comments::add(&db, &you, &k1, "Second comment: [KADE-3 - Filament table](gizai:task/KADE-3) has the same table.", None).unwrap();
    let asked = run(&k1, &qa, "qa", "needs_decision", REASON);
    comments::add(&db, &qa, &k1, &format!("Third comment, from QA: {REASON}"), Some(&asked)).unwrap();
    tasks::update(&db, &you, &k1, TaskPatch { hold: Some("needs_decision".into()), hold_reason: Some(REASON.into()), ..Default::default() }).unwrap();

    // GFW-2 in Review: its pull request, and QA's verdict with its hand-over
    db.write(None, |w| {
        w.conn().execute("UPDATE tasks SET pr_url='https://github.com/demo/gfw/pull/231', pr_state='open', branch='gizai/gfw-2-price-slider' WHERE id=?1",
                         rusqlite::params![g2])?;
        Ok(())
    }).unwrap();
    let passed = run(&g2, &qa, "qa", "qa_pass", "QA passed: the slider filters by price, 14 tests added.");
    comments::add(&db, &qa, &g2, "QA passed: the slider filters by price, 14 tests added.", Some(&passed)).unwrap();

    let (th, _) = chat::start_lead_chat(&db, &lead, "Check KADE-1 and GFW-2", "question", &[k1, g2],
                                        "KADE-1 needs your decision, and GFW-2 is in Review.", None).unwrap();
    chat::add_message(&db, chat::NewMessage { thread_id: th.clone(), role: "agent".into(), author_id: Some(lead.clone()),
                                              body_md: Some("Next up: [KADE-3 - Filament table](gizai:task/KADE-3), after [KADE-1 - Export invoices](gizai:task/KADE-1) in [Kade](gizai:project/KADE).".into()),
                                              ..Default::default() }).unwrap();
    println!("{th}");
}
