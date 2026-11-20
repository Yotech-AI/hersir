//! Docs in core: Changelog, a project's merged cards (in a Deploy or Done column, archived ones too) newest first, by when
//! they reached Done; Decisions, the shared memory notes about the project's decisions (in `Decisions/`, or of type
//! decision) whose `project:` is its key or name, newest first, without their properties; and the Chief's Memory block,
//! which says where a decision about a project goes.
use gizai_core::db::Db;
use gizai_core::memory::{self, Context, Who};
use gizai_core::{devdocs, model::*, projects, seed, tasks};

/// The id of the seeded board's first column of `category`.
fn column(db: &Db, category: &str) -> String {
    db.read(|c| Ok(c.query_row("SELECT id FROM workflow_states WHERE category = ?1 AND deleted_at IS NULL ORDER BY sort_key LIMIT 1", [category], |r| r.get(0))?)).unwrap()
}

struct Set { db: Db, you: String, kade: String, shop: String }

fn set_up() -> Set {
    let db = Db::open_in_memory().unwrap();
    let s = seed::ensure_seed(&db, "Jeffrey").unwrap();
    let project = |name: &str, key: &str| projects::create(&db, &s.you_id, ProjectInput { name: name.into(), key: key.into(), dev_docs: Some(true),
        ..Default::default() }).unwrap();
    let (kade, shop) = (project("Kade portal", "KADE"), project("Shop", "SHOP"));
    Set { db, you: s.you_id, kade, shop }
}

impl Set {
    fn card(&self, project: &str, title: &str, to: Option<&str>) -> String {
        let id = tasks::create(&self.db, &self.you, TaskInput { project_id: project.into(), title: title.into(), ..Default::default() }).unwrap();
        if let Some(category) = to {
            tasks::move_to(&self.db, &self.you, &id, &column(&self.db, category), "a0").unwrap();
        }
        id
    }
    fn note(&self, path: &str, body: &str) {
        memory::write(&self.db, &Who::Person(self.you.clone()), path, body, None, None).unwrap();
    }
}

#[test]
fn the_changelog_is_the_projects_merged_cards_newest_first() {
    let s = set_up();
    let first = s.card(&s.kade, "Invoices export as CSV", Some("done"));
    std::thread::sleep(std::time::Duration::from_millis(5));
    let second = s.card(&s.kade, "The contact form says when it sent", Some("deploy"));
    s.card(&s.kade, "Still being built", Some("in_progress"));
    s.card(&s.kade, "Not started", None);
    s.card(&s.shop, "Another project's card", Some("done"));
    std::thread::sleep(std::time::Duration::from_millis(5));
    let archived = s.card(&s.kade, "Archived after it was merged", Some("done"));
    tasks::archive(&s.db, &s.you, &archived).unwrap();

    let log = devdocs::changelog(&s.db, &s.kade).unwrap();
    assert_eq!(log.iter().map(|e| e.title.as_str()).collect::<Vec<_>>(),
               ["Archived after it was merged", "The contact form says when it sent", "Invoices export as CSV"]);
    assert_eq!(log.iter().map(|e| e.task_id.as_str()).collect::<Vec<_>>(), [archived.as_str(), second.as_str(), first.as_str()]);
    assert!(log.iter().all(|e| e.identifier.starts_with("KADE-")), "{log:?}");
    assert!(devdocs::changelog(&s.db, "no-such-project").unwrap().is_empty());
}

#[test]
fn decisions_are_the_shared_decision_notes_about_the_project() {
    let s = set_up();
    s.note("Decisions/Kade portal", "---\ntype: decision\nproject: KADE\n---\n# Kade portal\n\n- 2026-11-20 (KADE-3): Postgres, not MySQL.\n");
    std::thread::sleep(std::time::Duration::from_millis(5));
    s.note("Standards/Exports", "---\ntype: decision\nproject: kade portal\n---\n# Exports\n\nCSV with a header row.\n");
    s.note("Decisions/Shop", "---\ntype: decision\nproject: SHOP\n---\n# Shop\n\nStripe.\n");
    s.note("Standards/Rust style", "---\ntype: standard\nproject: KADE\n---\n# Rust style\n\nrustfmt.\n");
    s.note("Decisions/Without a project", "# Without a project\n\nNothing about Kade.\n");
    s.note("Chief/Kade decisions", "---\ntype: decision\nproject: KADE\n---\n# Kade decisions\n\nThe Chief's own note.\n");

    let kade = projects::get(&s.db, &s.kade).unwrap();
    let notes = devdocs::decisions(&s.db, &kade).unwrap();
    assert_eq!(notes.iter().map(|n| n.path.as_str()).collect::<Vec<_>>(), ["Standards/Exports", "Decisions/Kade portal"],
               "by key or name in any case, in Decisions/ or of type decision, shared notes only, newest first");
    assert_eq!(notes[1].title, "Kade portal");
    assert_eq!(notes[1].body_md, "# Kade portal\n\n- 2026-11-20 (KADE-3): Postgres, not MySQL.\n", "without its properties");
}

#[test]
fn the_chiefs_memory_block_says_where_a_decision_about_a_project_goes() {
    let s = set_up();
    s.note("Standards/Rust style", "# Rust style\n\nrustfmt.\n");
    let block = memory::prompt_block(&s.db, &Who::Person(s.you.clone()), &Context::default()).unwrap();
    assert!(block.text.contains("A decision about one project goes in `Decisions/`, with that project's key as `project:`: its agents get it, \
and its Docs page shows it."), "{}", block.text);
}
