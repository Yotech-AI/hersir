//! Docs: the Chief's record_decision, a numbered decision record on a project's Docs page, after the project's own
//! records in docs/decisions, one replacing an older one; refused for a project whose Docs page keeps no decisions,
//! which points at Memory.
use std::path::Path;
use std::process::Command;

use gizai_core::model::*;
use gizai_core::{projects, team};
use gizai_lib::docs_server::{route, Reply};
use gizai_lib::tools;
use serde_json::{json, Value};

fn git(dir: &Path, args: &[&str]) {
    let o = Command::new("git").args(["-c", "user.email=t@t", "-c", "user.name=Tess", "-c", "commit.gpgsign=false"]).args(args).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
}

#[tokio::test]
async fn the_chief_records_decisions_numbered_after_the_projects_own() {
    let tmp = tempfile::tempdir().unwrap();
    let st = gizai_lib::test_state(tmp.path());
    let repo = tmp.path().join("kade");
    std::fs::create_dir_all(repo.join("docs/decisions")).unwrap();
    std::fs::write(repo.join("docs/decisions/0007-use-sqlite.md"), "# Use SQLite\n\nStatus: superseded\n").unwrap();
    std::fs::write(repo.join("docs/decisions/README.md"), "# How we decide\n").unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "Decisions"]);
    let kade = projects::create(&st.db, &st.you_id, ProjectInput { name: "Kade".into(), key: "KADE".into(), dev_docs: Some(true),
        repo_path: Some(repo.to_string_lossy().into()), default_branch: Some("main".into()), ..Default::default() }).unwrap();
    projects::create(&st.db, &st.you_id, ProjectInput { name: "Shop".into(), key: "SHOP".into(), ..Default::default() }).unwrap();
    gizai_lib::test_task(&st, repo.to_str().unwrap(), "backend");
    let team_id = team::list(&st.db).unwrap()[0].id.clone();
    let chief = team::add_agent(&st.db, &st.you_id, &team_id, AgentInput { name: "Chief".into(), role_key: "lead".into(), ..Default::default() }).unwrap();
    let call = |args: Value| { let (st, chief) = (&st, &chief); async move { tools::call(st, chief, "record_decision", args).await } };

    let first = call(json!({"project": "KADE", "title": "Use Postgres", "decision": "Postgres, not MySQL.", "context": "JSON queries.",
        "task": "KADE-1", "related_files": ["config/database.php"]})).await.unwrap();
    let d = &first["decision"];
    assert_eq!((d["number"].as_i64(), d["status"].as_str(), d["task"].as_str(), d["decidedBy"].as_str()), (Some(8), Some("accepted"), Some("KADE-1"), Some("Chief")),
               "after the project's own 0007: {first}");
    assert_eq!(first["shows"], "Kade's Docs page, Decisions 0008");
    let second = call(json!({"project": "kade", "title": "Use Postgres 17", "decision": "Postgres 17.", "status": "proposed", "supersedes": 8})).await.unwrap();
    assert_eq!(second["decision"]["number"].as_i64(), Some(9));

    // on the page, one log with the project's own
    let Reply::Ok(_, body) = route(&st, &format!("/p/{kade}/index.json")) else { panic!("the index") };
    let index: Value = serde_json::from_slice(&body).unwrap();
    let log: Vec<(&str, &str)> = index["decisions"].as_array().unwrap().iter().map(|d| (d["number"].as_str().unwrap(), d["status"].as_str().unwrap())).collect();
    assert_eq!(log, [("0009", "proposed"), ("0008", "superseded"), ("0007", "superseded")]);

    // a project whose page keeps no decisions points at Memory; a decision needs its title and its text
    let off = call(json!({"project": "SHOP", "title": "Use Stripe", "decision": "Stripe."})).await.unwrap_err();
    assert!(off.contains("save the decision in Memory instead, in Decisions/ with project: SHOP"), "{off}");
    assert!(call(json!({"project": "KADE", "title": "No decision"})).await.is_err());
    assert!(call(json!({"project": "KADE", "title": "Rejected", "decision": "No.", "status": "rejected"})).await.is_err());
}
