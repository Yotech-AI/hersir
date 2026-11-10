// GA-255 QA: the lead agent is the Chief. A new install names it Chief, and its instructions and the role templates say
// the Chief (its Memory path is `Chief/Notes`, GA-258). Once per database, an agent still named exactly "Team Lead" with
// the lead role (the old default) is named Chief: any other name stays, and no saved instructions, handle or Memory
// folder change.
use gizai_core::db::Db;
use gizai_core::model::AgentInput;
use gizai_core::seed::{self, ensure_seed, ensure_seed_with_agents, old_lead_to_chief, role_template, CHIEF};
use gizai_core::{memory, settings, team};

const ROLES: [&str; 5] = ["lead", "backend", "frontend", "qa", "devops"];

fn add(db: &Db, you: &str, team_id: &str, name: &str, role: &str, instructions: Option<&str>) -> String {
    team::add_agent(db, you, team_id, AgentInput { name: name.into(), role_key: role.into(), instructions_md: instructions.map(Into::into),
                                                 chat_enabled: Some(role == "lead"), ..Default::default() }).unwrap()
}

/// The agent as saved: (name, handle, instructions, role).
fn saved(db: &Db, id: &str) -> (String, String, Option<String>, String) {
    let m = team::agent(db, id).unwrap();
    (m.name, m.handle, m.instructions_md, m.role_key)
}

fn name_of(db: &Db, id: &str) -> String {
    db.read(|c| Ok(c.query_row("SELECT name FROM actors WHERE id=?1", [id], |r| r.get(0))?)).unwrap()
}

#[test]
fn a_new_install_names_its_lead_agent_chief_with_chat_on_and_the_handle_chief() {
    let db = Db::open_in_memory().unwrap();
    let s = ensure_seed_with_agents(&db, "Sanne", || None).unwrap();
    assert_eq!(CHIEF, "Chief");
    let t = team::get(&db, &s.team_id).unwrap();
    let leads: Vec<_> = t.members.iter().filter(|m| m.kind == "agent" && m.role_key == "lead").collect();
    assert_eq!(leads.len(), 1, "one lead agent");
    assert_eq!((leads[0].name.as_str(), leads[0].handle.as_str()), ("Chief", "chief"));
    assert!(!t.members.iter().any(|m| m.name == "Team Lead"), "no agent is named Team Lead");
    let chat = team::chat_agent(&db).unwrap().unwrap();
    assert_eq!(chat.name, "Chief", "the Chief has Chat");
    assert_eq!(chat.instructions_md.as_deref(), Some(role_template("lead").as_str()));
    // The upgrade that runs at every start leaves a new install's Chief alone.
    assert_eq!(old_lead_to_chief(&db, &s.you_id).unwrap(), 0);
    assert_eq!(team::chat_agent(&db).unwrap().unwrap().name, "Chief");
}

#[test]
fn the_leads_instructions_say_the_chief_and_its_memory_path_chief_notes() {
    let lead = role_template("lead");
    assert!(lead.starts_with("You are the Chief in Hersir's Software team.\n"), "{lead}");
    assert!(lead.contains("in projects whose Chief may merge switch is on"), "{lead}");
    assert!(lead.contains("memory_append to Chief/Notes"), "the Memory path is Chief/ (GA-258): {lead}");
    assert_eq!(lead.matches("Team Lead").count(), 0, "nothing names Team Lead: {lead}");
    assert_eq!(memory::LEAD, "Chief", "the Memory folder is Chief/ (GA-258)");
    assert_eq!(memory::lead_notes_path(), "Chief/Notes");
}

#[test]
fn every_role_template_says_the_chief_and_names_team_lead_only_as_the_memory_folder() {
    for role in ROLES {
        let t = role_template(role);
        let without_paths = t.replace("Team Lead/", "");
        assert!(!without_paths.contains("Team Lead"), "{role}'s template still says Team Lead: {t}");
        assert!(!without_paths.to_lowercase().contains("team lead"), "{role}'s template: {t}");
    }
    let devops = role_template("devops");
    assert!(devops.contains("The Chief and the user write it; you report what changed."), "{devops}");
    assert!(devops.contains("The Chief folds them into the deploy note."), "{devops}");
    assert!(devops.contains("so the Chief can make Deployments/<KEY> from it."), "{devops}");
}

#[test]
fn an_old_lead_named_team_lead_becomes_chief_keeping_its_instructions_handle_and_memory_folder() {
    let db = Db::open_in_memory().unwrap();
    let s = ensure_seed(&db, "Sanne").unwrap();
    // An install from before GA-255: its lead agent has the old default name, and instructions of its own.
    let own = "You are the Team Lead in Gizai's Software team.\nAsk the Team Lead first.";
    let lead = add(&db, &s.you_id, &s.team_id, "Team Lead", "lead", Some(own));
    assert_eq!(saved(&db, &lead).1, "team-lead");
    let notes = memory::ensure_lead_notes(&db, &lead, "Sanne").unwrap();

    assert_eq!(old_lead_to_chief(&db, &s.you_id).unwrap(), 1, "one agent renamed");
    let (name, handle, instructions, role) = saved(&db, &lead);
    assert_eq!(name, "Chief");
    assert_eq!(handle, "team-lead", "its handle stays");
    assert_eq!(instructions.as_deref(), Some(own), "its saved instructions don't change");
    assert_eq!(role, "lead");
    assert_eq!(team::chat_agent(&db).unwrap().unwrap().actor_id, lead, "it keeps Chat");
    // Its notes stay in its own folder, and it still finds them there.
    assert_eq!(memory::ensure_lead_notes(&db, &lead, "Sanne").unwrap(), notes);
    // The rename is a change like any other: by you, in the change log (for sync), with the actor's version up.
    let (by, version, logged): (String, i64, i64) = db.read(|c| Ok(c.query_row(
        "SELECT a.updated_by, a.version, (SELECT count(*) FROM changes ch WHERE ch.table_name='actors' AND ch.row_id=a.id AND ch.op='update'
                                           AND ch.diff_json LIKE '%\"name\":\"Chief\"%')
         FROM actors a WHERE a.id=?1", [&lead], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?)).unwrap();
    assert_eq!((by.as_str(), logged), (s.you_id.as_str(), 1));
    assert!(version >= 2, "version {version}");
    // It is marked done: when, and how many it renamed.
    let done: serde_json::Value = settings::get(&db, "lead_named_chief").unwrap().expect("marked done");
    assert_eq!(done["renamed"], 1);
    assert!(done["at"].as_i64().unwrap() > 0, "{done}");
}

#[test]
fn any_other_name_stays_and_so_does_a_team_lead_that_isnt_the_lead() {
    let db = Db::open_in_memory().unwrap();
    let s = ensure_seed(&db, "Sanne").unwrap();
    let ada = add(&db, &s.you_id, &s.team_id, "Ada", "lead", None);
    let builder = add(&db, &s.you_id, &s.team_id, "Team Lead", "backend", Some("Build it."));
    let other = team::add_team(&db, &s.you_id, "Second team").unwrap();
    let lower = add(&db, &s.you_id, &other, "team lead", "lead", None);
    let third = team::add_team(&db, &s.you_id, "Third team").unwrap();
    let longer = add(&db, &s.you_id, &third, "Team Lead 2", "lead", None);
    let before: Vec<_> = [&ada, &builder, &lower, &longer].iter().map(|id| saved(&db, id)).collect();

    assert_eq!(old_lead_to_chief(&db, &s.you_id).unwrap(), 0, "only exactly Team Lead, with the lead role");
    let after: Vec<_> = [&ada, &builder, &lower, &longer].iter().map(|id| saved(&db, id)).collect();
    assert_eq!(after, before, "names, handles, instructions and roles are unchanged");
    assert_eq!(name_of(&db, &builder), "Team Lead", "a Backend agent named Team Lead keeps its name");
}

#[test]
fn it_runs_once_per_database_so_a_lead_named_team_lead_later_keeps_that_name() {
    let db = Db::open_in_memory().unwrap();
    let s = ensure_seed(&db, "Sanne").unwrap();
    let first = add(&db, &s.you_id, &s.team_id, "Team Lead", "lead", None);
    assert_eq!(old_lead_to_chief(&db, &s.you_id).unwrap(), 1);
    assert_eq!(name_of(&db, &first), "Chief");

    // You name it Team Lead again (or add one with that name) after the upgrade: it stays so, start after start.
    let second_team = team::add_team(&db, &s.you_id, "Second team").unwrap();
    let later = add(&db, &s.you_id, &second_team, "Team Lead", "lead", None);
    assert_eq!(old_lead_to_chief(&db, &s.you_id).unwrap(), 0);
    assert_eq!(old_lead_to_chief(&db, &s.you_id).unwrap(), 0);
    assert_eq!(name_of(&db, &later), "Team Lead");
    let done: serde_json::Value = settings::get(&db, "lead_named_chief").unwrap().unwrap();
    assert_eq!(done["renamed"], 1, "the first run's count is kept");
}

#[test]
fn an_install_with_no_old_lead_is_marked_done_too() {
    let db = Db::open_in_memory().unwrap();
    let s = ensure_seed(&db, "Sanne").unwrap();
    assert_eq!(old_lead_to_chief(&db, &s.you_id).unwrap(), 0);
    let done: serde_json::Value = settings::get(&db, "lead_named_chief").unwrap().expect("marked done");
    assert_eq!(done["renamed"], 0);
    let later = add(&db, &s.you_id, &s.team_id, "Team Lead", "lead", None);
    assert_eq!(old_lead_to_chief(&db, &s.you_id).unwrap(), 0);
    assert_eq!(name_of(&db, &later), "Team Lead");
}

#[test]
fn a_removed_agent_or_a_lead_membership_that_ended_isnt_renamed() {
    let db = Db::open_in_memory().unwrap();
    let s = ensure_seed(&db, "Sanne").unwrap();
    let removed = add(&db, &s.you_id, &s.team_id, "Team Lead", "lead", None);
    let second_team = team::add_team(&db, &s.you_id, "Second team").unwrap();
    let left = add(&db, &s.you_id, &second_team, "Team Lead", "lead", None);
    db.write(None, |w| {
        w.conn().execute("UPDATE actors SET deleted_at=1 WHERE id=?1", [&removed])?;
        w.conn().execute("UPDATE team_members SET deleted_at=1 WHERE actor_id=?1", [&left])?;
        Ok(())
    }).unwrap();
    assert_eq!(old_lead_to_chief(&db, &s.you_id).unwrap(), 0);
    assert_eq!((name_of(&db, &removed), name_of(&db, &left)), ("Team Lead".into(), "Team Lead".into()));
}

#[test]
fn seed_handle_for_the_chief_is_chief() {
    assert_eq!(seed::handle_for(CHIEF), "chief");
    assert_eq!(seed::DEFAULT_AGENTS[0], ("Chief", "lead"), "the Chief comes first");
    assert!(!seed::DEFAULT_AGENTS.iter().any(|(n, _)| *n == "Team Lead"));
}
