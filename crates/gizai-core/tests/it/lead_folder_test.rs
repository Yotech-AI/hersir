//! GA-258 QA: the Chief's own Memory folder is `Chief/` (`Chief/Notes`, `Chief/Imported/`), and once per database the
//! notes under `Team Lead/` move there (`memory::lead_folder_to_chief`): with their text, properties and versions, every
//! link to them following in every note and doc, a note already at a target path kept (the moved one says where it came
//! from), the agents' saved instructions saying `Chief/`, and nothing changed when the upgrade fails. Agents' folders,
//! shared folders and who may read and write what stay.
use gizai_core::db::Db;
use gizai_core::memory::{self, Who};
use gizai_core::model::*;
use gizai_core::{docs, projects, seed, settings, team};
use std::cell::Cell;

struct F {
    db: Db,
    you: String,
    lead: String,
    be: String,
}

/// The Chief (Chat on) and a Backend Agent, as an install from before GA-258 has them.
fn setup() -> F {
    let db = Db::open_in_memory().unwrap();
    let s = seed::ensure_seed(&db, "Jeffrey").unwrap();
    let add = |name: &str, role: &str, chat: bool, instructions: Option<&str>| team::add_agent(&db, &s.you_id, &s.team_id, AgentInput {
        name: name.into(), role_key: role.into(), chat_enabled: chat.then_some(true), instructions_md: instructions.map(Into::into),
        ..Default::default() }).unwrap();
    let lead = add("Chief", "lead", true, None);
    let be = add("Backend Agent", "backend", false, Some("Build it. Read Standards/ first."));
    F { you: s.you_id, lead, be, db }
}

impl F {
    fn lead(&self) -> Who { Who::Lead(self.lead.clone()) }
    fn be(&self) -> Who { Who::Agent(self.be.clone()) }
    fn write(&self, path: &str, body: &str) -> String {
        memory::write(&self.db, &self.lead(), path, body, None, None).unwrap().id
    }
    /// A note in the Chief's old folder, as the database before GA-258 holds it: written in its folder, then given its
    /// old path `Team Lead/<rest>`. With `versions` texts, each saved as a version of its own.
    fn old(&self, rest: &str, versions: &[&str]) -> String {
        let id = self.write(&format!("Chief/{rest}"), versions[0]);
        for (i, text) in versions.iter().enumerate().skip(1) {
            memory::write(&self.db, &self.lead(), &format!("Chief/{rest}"), text, Some(i as i64), None).unwrap();
        }
        self.db.write(None, |w| {
            w.conn().execute("UPDATE docs SET path = ?2 WHERE id = ?1", [&id, &format!("Team Lead/{rest}")])?;
            Ok(())
        }).unwrap();
        id
    }
    /// A doc's or note's row as saved: (path, title, scope, owner, text, current version, how many versions).
    fn row(&self, id: &str) -> (Option<String>, String, Option<String>, Option<String>, String, i64, i64) {
        self.db.read(|c| Ok(c.query_row(
            "SELECT path, title, scope, owner_actor_id, body_md, current_version, (SELECT count(*) FROM doc_versions v WHERE v.doc_id = d.id)
             FROM docs d WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?)).unwrap()
    }
    fn path(&self, id: &str) -> String { self.row(id).0.unwrap() }
    fn body(&self, id: &str) -> String { self.row(id).4 }
    /// Every memory note's path, as the Chief lists them.
    fn paths(&self) -> Vec<String> {
        memory::list(&self.db, &self.lead()).unwrap().into_iter().map(|n| n.path).collect()
    }
    /// The texts of every doc and note that is there, by id.
    fn texts(&self) -> Vec<(String, String)> {
        self.db.read(|c| {
            let mut st = c.prepare("SELECT id, body_md FROM docs WHERE deleted_at IS NULL ORDER BY id")?;
            Ok(st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?)
        }).unwrap()
    }
    fn instructions(&self, id: &str) -> (Option<String>, i64) {
        self.db.read(|c| Ok(c.query_row("SELECT instructions_md, version FROM agent_configs WHERE actor_id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap()
    }
    /// The whole state the upgrade may change: every doc's row, every agent's instructions and the setting.
    fn state(&self) -> (Vec<(String, String, String, i64, i64)>, Vec<(String, Option<String>)>, Option<serde_json::Value>) {
        let docs = self.db.read(|c| {
            let mut st = c.prepare("SELECT id, COALESCE(path, ''), body_md, current_version, (SELECT count(*) FROM doc_versions v WHERE v.doc_id = d.id)
                                    FROM docs d ORDER BY id")?;
            Ok(st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.collect::<rusqlite::Result<Vec<_>>>()?)
        }).unwrap();
        let agents = self.db.read(|c| {
            let mut st = c.prepare("SELECT actor_id, instructions_md FROM agent_configs ORDER BY actor_id")?;
            Ok(st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?)
        }).unwrap();
        (docs, agents, settings::get(&self.db, "lead_folder_chief").unwrap())
    }
    fn upgrade(&self) -> gizai_core::Result<Option<memory::LeadFolderMoved>> {
        memory::lead_folder_to_chief(&self.db, &self.you, || Ok(()))
    }
}

/// Whether a text still has a wikilink into the old folder, case ignored.
fn links_into_team_lead(text: &str) -> bool {
    memory::wikilinks(text).iter().any(|l| l.target.to_lowercase().starts_with("team lead/"))
}

#[test]
fn the_chiefs_folder_is_chief_with_its_notes_and_imported_and_an_old_path_finds_the_note_there() {
    assert_eq!((memory::LEAD, memory::OLD_LEAD, memory::lead_notes_path().as_str()), ("Chief", "Team Lead", "Chief/Notes"));
    let f = setup();
    let notes = memory::ensure_lead_notes(&f.db, &f.lead, "Jeffrey").unwrap();
    assert_eq!(f.path(&notes), "Chief/Notes");
    // A path in Team Lead/ (an old habit, saved instructions, a call that waits for approval) finds or makes the note in Chief/.
    let s = memory::write(&f.db, &f.lead(), "Team Lead/Ideas", "An idea.", None, None).unwrap();
    assert_eq!(s.path, "Chief/Ideas");
    assert_eq!(memory::get(&f.db, &f.lead(), "team lead/ideas").unwrap().id, s.id);
    memory::append(&f.db, &f.lead(), "Team Lead/Notes", Some("Open threads"), "- KADE waits.", None).unwrap();
    assert!(f.body(&notes).contains("- KADE waits."));
    let imported = memory::write(&f.db, &f.lead(), "Team Lead/Imported/proj/file", "From Claude Code.", None, None).unwrap();
    assert_eq!(imported.path, "Chief/Imported/proj/file");
    let hits = memory::search(&f.db, &f.lead(), "path:\"Team Lead\"", 20).unwrap();
    assert!(!hits.is_empty(), "path:\"Team Lead\" looks in Chief/");
    assert!(f.paths().iter().all(|p| !p.starts_with("Team Lead")), "{:?}", f.paths());
    // Only the Chief and people read and write it; an agent neither.
    assert!(memory::get(&f.db, &f.be(), "Chief/Ideas").is_err());
    assert!(memory::write(&f.db, &f.be(), "Chief/Mine", "x", None, None).is_err());
    assert!(memory::write(&f.db, &f.be(), "Team Lead/Mine", "x", None, None).is_err());
}

#[test]
fn the_upgrade_moves_every_note_to_the_same_path_under_chief_with_its_text_properties_and_versions() {
    let f = setup();
    let notes_text = "---\ntype: note\ntags: [team-lead]\n---\n# Notes\n\nSee [[Team Lead/Ideas]], [[Ideas#Next|next]] and ![[Team Lead/Sub/Diagram]].\n";
    let notes = f.old("Notes", &[notes_text]);
    let ideas = f.old("Ideas", &["---\ntype: decision\nproject: KADE\n---\n# Ideas\n\nv1", "---\ntype: decision\nproject: KADE\n---\n# Ideas\n\n## Next\n\nv2"]);
    let deep = f.old("Sub/Diagram", &["A diagram."]);
    let imported = f.old("Imported/proj/file", &["---\nsource: claude-code\n---\nFrom Claude Code."]);
    let before: Vec<_> = [&notes, &ideas, &deep, &imported].iter().map(|id| f.row(id)).collect();
    assert_eq!(before.iter().map(|r| r.0.clone().unwrap()).collect::<Vec<_>>(),
               ["Team Lead/Notes", "Team Lead/Ideas", "Team Lead/Sub/Diagram", "Team Lead/Imported/proj/file"]);
    let agent_notes = memory::find(&f.db, "Agents/Backend Agent/Notes").unwrap().expect("the agent's own notes").id;
    let agent_before = f.row(&agent_notes);

    let called = Cell::new(0);
    let done = memory::lead_folder_to_chief(&f.db, &f.you, || { called.set(called.get() + 1); Ok(()) }).unwrap().expect("it ran");
    assert_eq!(called.get(), 1, "the database is backed up first, once");
    assert_eq!((done.moved, done.renamed.clone()), (4, vec![]));

    for (id, (old_path, title, scope, owner, text, version, versions)) in [&notes, &ideas, &deep, &imported].iter().zip(before) {
        let (path, t, s, o, now_text, now_version, now_versions) = f.row(id);
        assert_eq!(path.unwrap(), old_path.unwrap().replacen("Team Lead/", "Chief/", 1), "the same path under Chief/");
        assert_eq!((t, s, o), (title, scope, owner), "the same name, scope and owner");
        if *id == &notes {
            // its own links follow: a link that still finds its note stays as it was
            assert_eq!(now_text, "---\ntype: note\ntags: [team-lead]\n---\n# Notes\n\nSee [[Chief/Ideas]], [[Ideas#Next|next]] and ![[Chief/Sub/Diagram]].\n");
            assert_eq!((now_version, now_versions), (version + 1, versions + 1), "the rewritten links are a new version");
        } else {
            assert_eq!(now_text, text, "the same text");
            assert_eq!((now_version, now_versions), (version, versions), "the same versions");
        }
    }
    // the older versions are kept as they were
    assert_eq!(docs::version_body(&f.db, &ideas, 1).unwrap(), "---\ntype: decision\nproject: KADE\n---\n# Ideas\n\nv1");
    assert_eq!(docs::version_body(&f.db, &notes, 1).unwrap(), notes_text);
    assert_eq!(memory::properties(&f.body(&ideas)).get("project").cloned(), Some(vec!["KADE".to_string()]));
    // Team Lead/ is gone; an agent's own folder is as it was
    assert!(f.paths().iter().all(|p| !p.to_lowercase().starts_with("team lead")), "{:?}", f.paths());
    assert!(memory::tree(&f.db, &f.lead()).unwrap().iter().all(|folder| !folder.path.to_lowercase().starts_with("team lead")));
    assert_eq!(f.row(&agent_notes), agent_before);
    // by you, and marked done
    let by: String = f.db.read(|c| Ok(c.query_row("SELECT updated_by FROM docs WHERE id = ?1", [&ideas], |r| r.get(0))?)).unwrap();
    assert_eq!(by, f.you);
    let mark: serde_json::Value = settings::get(&f.db, "lead_folder_chief").unwrap().expect("marked done");
    assert_eq!((mark["moved"].as_u64(), mark["renamed"].as_u64()), (Some(4), Some(0)));
    assert!(mark["at"].as_i64().unwrap() > 0, "{mark}");
}

#[test]
fn every_link_to_a_moved_note_points_to_its_new_path_in_every_note_and_doc() {
    let f = setup();
    let notes = f.old("Notes", &["# Notes"]);
    let ideas = f.old("Ideas", &["# Ideas\n\n## Plan\n\nShip it."]);
    let rust = f.write("Standards/Rust style",
        "Use thiserror. See [[Team Lead/Notes]], [[team lead/ideas#Plan|the plan]], ![[Team Lead/Ideas]] and [[Team Lead/Ideas.md]].\n\
         Not there yet: [[Team Lead/Missing]]. Elsewhere: [[Standards/Go style]] and [[Rust style]].");
    let agent = memory::find(&f.db, "Agents/Backend Agent/Notes").unwrap().unwrap();
    memory::write(&f.db, &f.be(), "Agents/Backend Agent/Notes", &format!("{}\n- Asked the Chief, see [[Team Lead/Notes]].", agent.body_md),
                  Some(agent.current_version), None).unwrap();
    let project = projects::create(&f.db, &f.you, ProjectInput { name: "Kade".into(), key: "KADE".into(), ..Default::default() }).unwrap();
    let doc = docs::create(&f.db, &f.you, &project, "Plan").unwrap();
    docs::save(&f.db, &f.you, &doc, "The plan is in [[Team Lead/Ideas#Plan]].", 1).unwrap();

    let done = f.upgrade().unwrap().unwrap();
    assert_eq!(done.moved, 2);
    assert_eq!(done.relinked, 3, "the shared note, the agent's note and the project's doc");
    assert_eq!(f.body(&rust),
        "Use thiserror. See [[Chief/Notes]], [[Chief/Ideas#Plan|the plan]], ![[Chief/Ideas]] and [[Chief/Ideas]].\n\
         Not there yet: [[Chief/Missing]]. Elsewhere: [[Standards/Go style]] and [[Rust style]].");
    assert!(f.body(&agent.id).ends_with("- Asked the Chief, see [[Chief/Notes]]."), "{}", f.body(&agent.id));
    assert_eq!(docs::get(&f.db, &doc).unwrap().body_md, "The plan is in [[Chief/Ideas#Plan]].");
    // no link into Team Lead/ is left anywhere, and the links still find the notes they found
    let left: Vec<_> = f.texts().into_iter().filter(|(_, t)| links_into_team_lead(t)).collect();
    assert!(left.is_empty(), "{left:?}");
    let to = |id: &str| memory::links_from(&f.db, id).unwrap();
    assert!(format!("{:?}", to(&rust)).contains(&notes) && format!("{:?}", to(&rust)).contains(&ideas), "{:?}", to(&rust));
    let back: Vec<String> = memory::backlinks(&f.db, &f.lead(), "Chief/Ideas").unwrap().into_iter().map(|n| n.path).collect();
    assert!(back.contains(&"Standards/Rust style".to_string()), "{back:?}");
    // the shared note's old version is kept as written
    assert!(docs::version_body(&f.db, &rust, 1).unwrap().contains("[[Team Lead/Notes]]"));
    // the agent still can't read the Chief's notes, and still reads and writes its own
    assert!(memory::get(&f.db, &f.be(), "Chief/Notes").is_err());
    assert!(memory::get(&f.db, &f.be(), "Agents/Backend Agent/Notes").is_ok());
    assert!(memory::get(&f.db, &f.be(), "Standards/Rust style").is_ok());
}

#[test]
fn a_note_already_at_a_target_path_stays_and_the_moved_note_says_where_it_came_from() {
    let f = setup();
    let old_notes = f.old("Notes", &["# Notes\n\nThe old notes."]);
    let old_ideas = f.old("Ideas", &["# Ideas\n\nThe old ideas."]);
    let plain = f.old("Plain", &["Nothing in its way."]);
    // Chief/ has Notes and Ideas already, and Ideas (Team Lead) too
    let new_notes = memory::ensure_lead_notes(&f.db, &f.lead, "Jeffrey").unwrap();
    let new_ideas = f.write("Chief/Ideas", "# Ideas\n\nThe new ideas.");
    let taken = f.write("Chief/Ideas (Team Lead)", "Someone's note.");
    let rust = f.write("Standards/Rust style", "Old: [[Team Lead/Notes]] and [[Team Lead/Ideas#Ideas]]. New: [[Chief/Notes]] and [[Chief/Ideas]].");
    let kept = [&new_notes, &new_ideas, &taken].map(|id| f.row(id));

    let done = f.upgrade().unwrap().unwrap();
    assert_eq!(done.moved, 3);
    let mut renamed = done.renamed.clone();
    renamed.sort();
    assert_eq!(renamed, [("Team Lead/Ideas".to_string(), "Chief/Ideas (Team Lead 2)".to_string()),
                         ("Team Lead/Notes".to_string(), "Chief/Notes (Team Lead)".to_string())]);
    assert_eq!((f.path(&old_notes), f.path(&old_ideas), f.path(&plain)),
               ("Chief/Notes (Team Lead)".to_string(), "Chief/Ideas (Team Lead 2)".to_string(), "Chief/Plain".to_string()));
    assert_eq!(f.body(&old_notes), "# Notes\n\nThe old notes.");
    // the notes that were there are kept as they were, and the Chief keeps using its Chief/Notes
    assert_eq!([&new_notes, &new_ideas, &taken].map(|id| f.row(id)), kept);
    assert_eq!(memory::ensure_lead_notes(&f.db, &f.lead, "Jeffrey").unwrap(), new_notes);
    // each link still finds the note it found: the old ones the renamed notes, the new ones the kept notes
    assert_eq!(f.body(&rust), "Old: [[Chief/Notes (Team Lead)]] and [[Chief/Ideas (Team Lead 2)#Ideas]]. New: [[Chief/Notes]] and [[Chief/Ideas]].");
    let mark: serde_json::Value = settings::get(&f.db, "lead_folder_chief").unwrap().unwrap();
    assert_eq!(mark["renamed"].as_u64(), Some(2));
}

#[test]
fn the_agents_saved_instructions_say_chief_where_they_said_the_team_lead_folder() {
    let f = setup();
    f.old("Notes", &["# Notes"]);
    let own = "You are the Chief.\n- Save decisions (memory_append to Team Lead/Notes) and read Team Lead/Imported/ with the user.\n- Ask the Team Lead first.";
    f.db.write(None, |w| { w.conn().execute("UPDATE agent_configs SET instructions_md = ?2 WHERE actor_id = ?1", [&f.lead, &own.to_string()])?; Ok(()) }).unwrap();
    let (_, lead_version) = f.instructions(&f.lead);
    let be_before = f.instructions(&f.be);

    let done = f.upgrade().unwrap().unwrap();
    assert_eq!(done.instructions, 1);
    let (text, version) = f.instructions(&f.lead);
    assert_eq!(text.as_deref(), Some("You are the Chief.\n- Save decisions (memory_append to Chief/Notes) and read Chief/Imported/ with the user.\n- Ask the Team Lead first."),
               "only the folder's path changes");
    assert_eq!(version, lead_version + 1);
    assert_eq!(f.instructions(&f.be), be_before, "an agent whose instructions don't name the folder is unchanged");
}

#[test]
fn a_backup_that_fails_moves_nothing_and_the_next_start_tries_again() {
    let f = setup();
    let ideas = f.old("Ideas", &["# Ideas"]);
    f.write("Standards/Rust style", "See [[Team Lead/Ideas]].");
    let before = f.state();

    let e = memory::lead_folder_to_chief(&f.db, &f.you, || Err("disk full".into())).unwrap_err().to_string();
    assert!(e.contains("disk full") && e.contains("Team Lead/"), "{e}");
    assert_eq!(f.state(), before, "nothing changed, nor marked done");
    assert_eq!(f.path(&ideas), "Team Lead/Ideas");

    assert_eq!(f.upgrade().unwrap().unwrap().moved, 1, "the next start moves them");
    assert_eq!(f.path(&ideas), "Chief/Ideas");
}

#[test]
fn an_upgrade_that_fails_halfway_leaves_the_notes_links_and_instructions_as_they_were() {
    let f = setup();
    let notes = f.old("Notes", &["# Notes\n\n[[Team Lead/Ideas]]"]);
    let ideas = f.old("Ideas", &["# Ideas"]);
    let rust = f.write("Standards/Rust style", "See [[Team Lead/Ideas]] and [[Team Lead/Notes]].");
    let own = "Keep notes with memory_append to Team Lead/Notes.";
    f.db.write(None, |w| { w.conn().execute("UPDATE agent_configs SET instructions_md = ?2 WHERE actor_id = ?1", [&f.lead, &own.to_string()])?; Ok(()) }).unwrap();
    // The last step fails (the instructions, after the notes moved and the links were rewritten), like a full disk.
    f.db.write(None, |w| {
        w.conn().execute_batch("CREATE TRIGGER qa_full_disk BEFORE UPDATE OF instructions_md ON agent_configs BEGIN SELECT RAISE(ABORT, 'disk full'); END;")?;
        Ok(())
    }).unwrap();
    let before = f.state();

    let e = f.upgrade().unwrap_err().to_string();
    assert!(e.contains("disk full"), "{e}");
    assert_eq!(f.state(), before, "the notes, their texts, versions and links, the instructions: all as they were, and not marked done");
    assert_eq!((f.path(&notes), f.path(&ideas)), ("Team Lead/Notes".to_string(), "Team Lead/Ideas".to_string()));
    assert_eq!(f.body(&rust), "See [[Team Lead/Ideas]] and [[Team Lead/Notes]].");

    // Once the disk has room, the next start does it all.
    f.db.write(None, |w| { w.conn().execute_batch("DROP TRIGGER qa_full_disk;")?; Ok(()) }).unwrap();
    let done = f.upgrade().unwrap().unwrap();
    assert_eq!((done.moved, done.instructions), (2, 1));
    assert_eq!(f.body(&rust), "See [[Chief/Ideas]] and [[Chief/Notes]].");
    assert_eq!(f.instructions(&f.lead).0.as_deref(), Some("Keep notes with memory_append to Chief/Notes."));
}

#[test]
fn it_runs_once_per_database_and_with_nothing_to_move_needs_no_backup() {
    let f = setup();
    memory::ensure_lead_notes(&f.db, &f.lead, "Jeffrey").unwrap();
    let before = f.state();
    let done = memory::lead_folder_to_chief(&f.db, &f.you, || panic!("no backup when no note waits")).unwrap().expect("it ran");
    assert_eq!(done, memory::LeadFolderMoved::default());
    assert_eq!(f.state().0, before.0, "no note changed");
    assert_eq!(settings::get::<serde_json::Value>(&f.db, "lead_folder_chief").unwrap().unwrap()["moved"].as_u64(), Some(0));
    // Later starts change nothing, whatever the database holds then.
    let late = f.old("Late", &["Put back by hand."]);
    assert_eq!(memory::lead_folder_to_chief(&f.db, &f.you, || panic!("no backup once done")).unwrap(), None);
    assert_eq!(f.path(&late), "Team Lead/Late");
}

#[test]
fn a_shared_note_saved_before_a_secret_check_knew_its_kind_still_has_its_links_rewritten() {
    let f = setup();
    f.old("Ideas", &["# Ideas"]);
    let old = f.write("Lessons/Old", "See [[Team Lead/Ideas]].");
    // a token kind the check knows now, saved before it did
    let text = "See [[Team Lead/Ideas]]. Token: ghp_abcdefghijklmnopqrstuvwxyz0123456789";
    assert!(memory::secret_in(text).is_some());
    f.db.write(None, |w| { w.conn().execute("UPDATE docs SET body_md = ?2 WHERE id = ?1", [&old, &text.to_string()])?; Ok(()) }).unwrap();

    assert_eq!(f.upgrade().unwrap().unwrap().moved, 1);
    assert!(f.body(&old).starts_with("See [[Chief/Ideas]]."), "{}", f.body(&old));
    // a new save with a secret is still refused
    assert!(memory::write(&f.db, &f.lead(), "Lessons/New", text, None, None).is_err());
}
