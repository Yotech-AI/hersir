//! Docs → Changelog and Decisions (`gizai_core::devdocs`). Docs → Commits reads git, so it is in `changes`.
use gizai_core::devdocs::{self, ChangelogEntry, DecisionNote};
use gizai_core::projects;
use tauri::State;

use crate::AppState;

/// The project's merged cards, newest first.
#[tauri::command]
pub async fn docs_changelog(st: State<'_, AppState>, project_id: String) -> Result<Vec<ChangelogEntry>, String> {
    let st = st.inner().clone();
    tauri::async_runtime::spawn_blocking(move || devdocs::changelog(&st.db, &project_id).map_err(|e| e.to_string()))
        .await.map_err(|e| e.to_string())?
}

/// The shared memory notes about the project's decisions, newest first.
#[tauri::command]
pub async fn docs_decisions(st: State<'_, AppState>, project_id: String) -> Result<Vec<DecisionNote>, String> {
    let st = st.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let project = projects::get(&st.db, &project_id).map_err(|e| e.to_string())?;
        devdocs::decisions(&st.db, &project).map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?
}

/// Docs → Reference: the declarations in the files git tracks in the project's repository, as they are on disk.
#[tauri::command]
pub async fn docs_reference(st: State<'_, AppState>, project_id: String) -> Result<gizai_docs::Reference, String> {
    let st = st.inner().clone();
    tauri::async_runtime::spawn_blocking(move || reference_of(&st, &project_id)).await.map_err(|e| e.to_string())?
}

/// `docs_reference`. Blocking.
pub fn reference_of(st: &AppState, project_id: &str) -> Result<gizai_docs::Reference, String> {
    let (project, repo) = crate::changes::docs_repo(st, project_id)?;
    let files = gizai_agents::worktree::tracked_files(&repo)
        .map_err(|e| format!("Hersir can't list {}'s files: {}", project.name, crate::pulls::plain(e)))?;
    Ok(gizai_docs::reference(&repo, &files))
}
