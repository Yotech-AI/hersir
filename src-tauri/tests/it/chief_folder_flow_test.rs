// GA-258 QA: the Chief's Memory folder is Chief/, in the app. A data folder from before GA-258, with the Chief's notes in
// Team Lead/, links between them and from a shared note, and the Chief's saved instructions naming Team Lead/Notes: the
// next start backs the database up (backups/gizai-before-chief-folder-<time>.db), then moves the notes to Chief/ with
// their versions and rewrites the links and the instructions, once. When the backup can't be made, nothing moves and the
// start after tries again.
#[path = "../support/data_lock.rs"]
mod data_lock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gizai_core::db::Db;
use gizai_core::memory::{self, Who};
use gizai_lib::AppState;

fn open(data: &Path) -> AppState {
    gizai_lib::open_state(data.to_path_buf(), Arc::new(|_| {})).unwrap()
}

/// A note's (path, text, current version, versions kept).
fn row(db: &Db, id: &str) -> (String, String, i64, i64) {
    db.read(|c| Ok(c.query_row(
        "SELECT path, body_md, current_version, (SELECT count(*) FROM doc_versions v WHERE v.doc_id = d.id) FROM docs d WHERE id = ?1",
        [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?)).unwrap()
}

fn backups(data: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(data.join("backups")).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("gizai-before-chief-folder-")).collect()).unwrap_or_default();
    v.sort();
    v
}

/// The ids of: the Chief, Team Lead/Notes, Team Lead/Ideas (two versions) and Standards/Rust style, in a data folder as a
/// Hersir from before GA-258 left it (the upgrade not run yet).
fn old_data(data: &Path) -> (String, String, String, String) {
    let st = open(data);
    let lead = gizai_core::team::chat_agent(&st.db).unwrap().expect("the Chief").actor_id;
    let who = Who::Lead(lead.clone());
    let w = |path: &str, body: &str, v: Option<i64>| memory::write(&st.db, &who, path, body, v, None).unwrap().id;
    let notes = w("Chief/Notes", "---\ntype: note\ntags: [team-lead]\n---\n# Notes\n\nPlans: [[Team Lead/Ideas#Plan|the plan]].\n", None);
    let ideas = w("Chief/Ideas", "# Ideas\n\nv1", None);
    w("Chief/Ideas", "# Ideas\n\n## Plan\n\nv2", Some(1));
    let rust = w("Standards/Rust style", "Use thiserror. Ask the Chief: [[Team Lead/Notes]], ![[Team Lead/Ideas]].", None);
    st.db.write(None, |w| {
        let c = w.conn();
        c.execute("UPDATE docs SET path = 'Team Lead/' || substr(path, 7) WHERE kind = 'memory' AND path LIKE 'Chief/%'", [])?;
        c.execute("UPDATE agent_configs SET instructions_md = replace(instructions_md, 'Chief/Notes', 'Team Lead/Notes') WHERE actor_id = ?1", [&lead])?;
        c.execute("DELETE FROM settings WHERE key = 'lead_folder_chief'", [])?;
        Ok(())
    }).unwrap();
    assert_eq!(row(&st.db, &ideas).0, "Team Lead/Ideas");
    let instructions = gizai_core::team::agent(&st.db, &lead).unwrap().instructions_md.unwrap();
    assert!(instructions.contains("memory_append to Team Lead/Notes"), "{instructions}");
    (lead, notes, ideas, rust)
}

#[tokio::test]
async fn at_start_the_chiefs_notes_move_from_team_lead_to_chief_once_after_a_backup_and_the_links_follow() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let (lead, notes, ideas, rust) = old_data(&data);
    data_lock::released(&data).await;
    let kept_before = backups(&data).len();
    let ideas_before = {
        let db = Db::open(&data.join("gizai.db")).unwrap();
        row(&db, &ideas)
    };
    assert_eq!((ideas_before.2, ideas_before.3), (2, 2), "two versions");

    let st = open(&data);
    // one backup, made before anything moved: it still has the notes in Team Lead/
    let made = backups(&data);
    assert_eq!(made.len(), kept_before + 1, "{made:?}");
    let copy = dir.path().join("backup-copy.db");
    std::fs::copy(made.last().unwrap(), &copy).unwrap();
    let backup = Db::open(&copy).unwrap();
    assert_eq!(row(&backup, &ideas).0, "Team Lead/Ideas");
    assert!(row(&backup, &rust).1.contains("[[Team Lead/Notes]]"));
    drop(backup);

    // the notes are in Chief/, with their text and versions; the links and the Chief's instructions follow
    let (path, text, version, versions) = row(&st.db, &ideas);
    assert_eq!((path.as_str(), text.as_str(), version, versions), ("Chief/Ideas", ideas_before.1.as_str(), ideas_before.2, ideas_before.3));
    assert_eq!(row(&st.db, &notes).0, "Chief/Notes");
    assert_eq!(row(&st.db, &notes).1, "---\ntype: note\ntags: [team-lead]\n---\n# Notes\n\nPlans: [[Chief/Ideas#Plan|the plan]].\n");
    assert_eq!(row(&st.db, &rust).1, "Use thiserror. Ask the Chief: [[Chief/Notes]], ![[Chief/Ideas]].");
    let paths: Vec<String> = memory::list(&st.db, &Who::Person(st.you_id.clone())).unwrap().into_iter().map(|n| n.path).collect();
    assert!(paths.iter().all(|p| !p.starts_with("Team Lead/")), "{paths:?}");
    let instructions = gizai_core::team::agent(&st.db, &lead).unwrap().instructions_md.unwrap();
    assert!(instructions.contains("memory_append to Chief/Notes") && !instructions.contains("Team Lead/"), "{instructions}");
    // the Chief's own notes are Chief/Notes: the ones it had
    assert_eq!(memory::ensure_lead_notes(&st.db, &lead, "Jefsev").unwrap(), notes);
    let done: serde_json::Value = gizai_core::settings::get(&st.db, "lead_folder_chief").unwrap().expect("marked done");
    assert_eq!((done["moved"].as_u64(), done["instructions"].as_u64()), (Some(2), Some(1)));

    // the next start changes nothing and makes no backup
    let after = (row(&st.db, &notes), row(&st.db, &ideas), row(&st.db, &rust));
    drop(st);
    data_lock::released(&data).await;
    let st = open(&data);
    assert_eq!((row(&st.db, &notes), row(&st.db, &ideas), row(&st.db, &rust)), after);
    assert_eq!(backups(&data).len(), kept_before + 1);
}

#[tokio::test]
async fn when_the_backup_cant_be_made_the_notes_stay_in_team_lead_and_the_next_start_moves_them() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let (_, notes, ideas, rust) = old_data(&data);
    data_lock::released(&data).await;
    // backups/ can't be made: a file is in its way
    let _ = std::fs::remove_dir_all(data.join("backups"));
    std::fs::write(data.join("backups"), "in the way").unwrap();

    let st = open(&data);
    assert_eq!((row(&st.db, &notes).0, row(&st.db, &ideas).0), ("Team Lead/Notes".to_string(), "Team Lead/Ideas".to_string()));
    assert_eq!(row(&st.db, &rust).1, "Use thiserror. Ask the Chief: [[Team Lead/Notes]], ![[Team Lead/Ideas]].");
    assert!(gizai_core::settings::get::<serde_json::Value>(&st.db, "lead_folder_chief").unwrap().is_none(), "not marked done");
    drop(st);
    data_lock::released(&data).await;

    std::fs::remove_file(data.join("backups")).unwrap();
    let st = open(&data);
    assert_eq!((row(&st.db, &notes).0, row(&st.db, &ideas).0), ("Chief/Notes".to_string(), "Chief/Ideas".to_string()));
    assert_eq!(row(&st.db, &rust).1, "Use thiserror. Ask the Chief: [[Chief/Notes]], ![[Chief/Ideas]].");
    assert_eq!(backups(&data).len(), 1);
}

#[tokio::test]
async fn after_the_move_the_chiefs_tools_with_an_old_team_lead_path_find_the_notes_in_chief() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let (lead, _, ideas, _) = old_data(&data);
    data_lock::released(&data).await;
    let st = open(&data);
    assert_eq!(row(&st.db, &ideas).0, "Chief/Ideas");
    let call = |name: &'static str, args: serde_json::Value| {
        let (st, lead) = (&st, &lead);
        async move { gizai_lib::tools::call_in(st, lead, None, name, args).await.unwrap_or_else(|e| panic!("{name}: {e}")) }
    };
    // memory_list with the old folder lists the Chief's notes
    let listed = call("memory_list", serde_json::json!({"folder": "Team Lead"})).await;
    let paths: Vec<&str> = listed["notes"].as_array().unwrap().iter().map(|n| n["path"].as_str().unwrap()).collect();
    assert_eq!(paths, ["Chief/Ideas", "Chief/Notes"], "{listed}");
    // memory_read by its old path reads the moved note, with the notes that link to it
    let read = call("memory_read", serde_json::json!({"note": "Team Lead/Ideas"})).await;
    assert_eq!(read["note"]["path"], "Chief/Ideas", "{read}");
    let from: Vec<&str> = read["note"]["linked_from"].as_array().unwrap().iter().map(|p| p.as_str().unwrap()).collect();
    assert!(from.contains(&"Chief/Notes") && from.contains(&"Standards/Rust style"), "{read}");
    // memory_search with path:"Team Lead" keeps to Chief/
    let found = call("memory_search", serde_json::json!({"query": "path:\"Team Lead\" ideas"})).await;
    let hits: Vec<&str> = found["notes"].as_array().unwrap().iter().map(|n| n["path"].as_str().unwrap()).collect();
    assert!(!hits.is_empty() && hits.iter().all(|p| p.starts_with("Chief/")), "{found}");
    // memory_append to Team Lead/Notes, as the Chief's old instructions said, adds to Chief/Notes
    let appended = call("memory_append", serde_json::json!({"note": "Team Lead/Notes", "heading": "Open threads", "text": "- KADE waits."})).await;
    assert_eq!(appended["note"]["path"], "Chief/Notes", "{appended}");
    let paths: Vec<String> = memory::list(&st.db, &Who::Person(st.you_id.clone())).unwrap().into_iter().map(|n| n.path).collect();
    assert!(paths.iter().all(|p| !p.starts_with("Team Lead/")), "no note was made in Team Lead/: {paths:?}");
}
