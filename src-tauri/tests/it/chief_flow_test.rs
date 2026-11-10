// GA-255 QA: the lead agent is the Chief, in the app. A new install starts with an agent named Chief; at start, once per
// database, a lead still named exactly "Team Lead" is named Chief, keeping its instructions, its handle and its Memory
// folder Team Lead/; and the MCP tools' descriptions say the Chief, naming Team Lead only as that Memory folder.
#[path = "../support/data_lock.rs"]
mod data_lock;
use std::path::Path;
use std::sync::Arc;

use gizai_core::model::AgentInput;
use gizai_core::team;
use gizai_lib::AppState;

fn open(data: &Path) -> AppState {
    gizai_lib::open_state(data.to_path_buf(), Arc::new(|_| {})).unwrap()
}

/// Every agent as saved: (name, handle, instructions), by id.
fn agents(st: &AppState) -> Vec<(String, (String, String, Option<String>))> {
    let mut v: Vec<_> = team::all_agents(&st.db).unwrap().into_iter()
        .map(|(_, m)| (m.actor_id.clone(), (m.name, m.handle, m.instructions_md))).collect();
    v.sort();
    v
}

#[test]
fn a_new_install_starts_with_the_chief_and_is_marked_so_it_never_renames_again() {
    let dir = tempfile::tempdir().unwrap();
    let st = open(&dir.path().join("data"));
    let chief = team::chat_agent(&st.db).unwrap().expect("the lead agent has Chat");
    assert_eq!((chief.name.as_str(), chief.role_key.as_str(), chief.handle.as_str()), ("Chief", "lead", "chief"));
    assert!(chief.instructions_md.as_deref().unwrap().starts_with("You are the Chief in Hersir's Software team."));
    assert!(!agents(&st).iter().any(|(_, (n, _, _))| n == "Team Lead"));
    let done: serde_json::Value = gizai_core::settings::get(&st.db, "lead_named_chief").unwrap().expect("marked done at the first start");
    assert_eq!(done["renamed"], 0);
}

#[tokio::test]
async fn at_start_a_lead_still_named_team_lead_becomes_chief_once_and_nothing_else_changes() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    // An install from before GA-255: its lead is named Team Lead (handle team-lead) with instructions of its own, and the
    // upgrade hasn't run on it yet. A second team's lead has a name of its own.
    let (lead, ada, before) = {
        let st = open(&data);
        let lead = team::chat_agent(&st.db).unwrap().unwrap().actor_id;
        let own = "You are the Team Lead in Gizai's Software team. Ask the Team Lead first.";
        st.db.write(None, |w| {
            w.conn().execute("UPDATE actors SET name='Team Lead', handle='team-lead' WHERE id=?1", [&lead])?;
            w.conn().execute("UPDATE agent_configs SET instructions_md=?2 WHERE actor_id=?1", [&lead, &own.to_string()])?;
            w.conn().execute("DELETE FROM settings WHERE key='lead_named_chief'", [])?;
            Ok(())
        }).unwrap();
        let second = team::add_team(&st.db, &st.you_id, "Second team").unwrap();
        let ada = team::add_agent(&st.db, &st.you_id, &second, AgentInput { name: "Ada".into(), role_key: "lead".into(), ..Default::default() }).unwrap();
        let before = agents(&st);
        assert!(before.iter().any(|(id, (n, _, _))| id == &lead && n == "Team Lead"), "{before:?}");
        (lead, ada, before)
    };
    data_lock::released(&data).await;

    // The next start names it Chief: only its name changes, by you.
    let st = open(&data);
    let after = agents(&st);
    for ((id, b), (id2, a)) in before.iter().zip(&after) {
        assert_eq!(id, id2);
        if *id == lead {
            assert_eq!(a, &("Chief".to_string(), b.1.clone(), b.2.clone()), "only the name changes");
        } else {
            assert_eq!(a, b, "{} is unchanged", b.0);
        }
    }
    assert_eq!(team::agent(&st.db, &ada).unwrap().name, "Ada");
    let by: String = st.db.read(|c| Ok(c.query_row("SELECT updated_by FROM actors WHERE id=?1", [&lead], |r| r.get(0))?)).unwrap();
    assert_eq!(by, st.you_id);
    assert_eq!(gizai_core::memory::lead_notes_path(), "Team Lead/Notes", "its notes stay in Team Lead/");
    let done: serde_json::Value = gizai_core::settings::get(&st.db, "lead_named_chief").unwrap().unwrap();
    assert_eq!(done["renamed"], 1);

    // You name it Team Lead again: it keeps that name at the next start.
    let m = team::agent(&st.db, &lead).unwrap();
    team::update_agent(&st.db, &st.you_id, &lead, AgentInput { name: "Team Lead".into(), role_key: m.role_key, ..Default::default() }).unwrap();
    drop(st);
    data_lock::released(&data).await;
    let st = open(&data);
    assert_eq!(team::agent(&st.db, &lead).unwrap().name, "Team Lead", "once per database");
}

#[test]
fn the_mcp_tools_descriptions_say_the_chief_and_name_team_lead_only_as_the_memory_folder() {
    let all = gizai_lib::tools::catalog();
    let mut chief = 0;
    for t in &all {
        let text = format!("{} {}", t.description, t.input_schema);
        // the Memory folder stays: Team Lead/, Team Lead/Notes and path:"Team Lead" (escaped in the schema's JSON)
        let rest = text.replace("Team Lead/", "").replace("path:\"Team Lead\"", "").replace("path:\\\"Team Lead\\\"", "");
        assert!(!rest.contains("Team Lead"), "{}: {text}", t.name);
        chief += text.matches("Chief").count();
    }
    assert!(chief > 0, "some descriptions name the Chief");
    let update_agent = all.iter().find(|t| t.name == "update_agent").expect("update_agent");
    assert!(update_agent.input_schema.to_string().contains("Only for the agent with Chat on (the Chief)"), "{}", update_agent.input_schema);
    let merge = all.iter().find(|t| t.name == "merge_pull_request").unwrap();
    assert!(merge.description.contains("its project's Chief may merge switch is on"), "{}", merge.description);
    let search = all.iter().find(|t| t.name == "memory_search").unwrap();
    assert!(search.description.contains("path:\"Team Lead\""), "the Memory path stays: {}", search.description);
}
