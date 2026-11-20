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
    let (repo, files) = tracked(st, project_id)?;
    Ok(gizai_docs::reference(&repo, &files))
}

/// Docs → API: the routes the project answers on, read from the files git tracks, without running anything.
#[tauri::command]
pub async fn docs_api(st: State<'_, AppState>, project_id: String) -> Result<Vec<gizai_docs::api::Route>, String> {
    let st = st.inner().clone();
    tauri::async_runtime::spawn_blocking(move || api_of(&st, &project_id)).await.map_err(|e| e.to_string())?
}

/// `docs_api`. Blocking.
pub fn api_of(st: &AppState, project_id: &str) -> Result<Vec<gizai_docs::api::Route>, String> {
    let (repo, files) = tracked(st, project_id)?;
    Ok(gizai_docs::api::api(&repo, &files))
}

/// Docs → Guides: the project's own Markdown docs, the README first.
#[tauri::command]
pub async fn docs_guides(st: State<'_, AppState>, project_id: String) -> Result<Vec<gizai_docs::guides::Guide>, String> {
    let st = st.inner().clone();
    tauri::async_runtime::spawn_blocking(move || guides_of(&st, &project_id)).await.map_err(|e| e.to_string())?
}

/// `docs_guides`. Blocking.
pub fn guides_of(st: &AppState, project_id: &str) -> Result<Vec<gizai_docs::guides::Guide>, String> {
    let (repo, files) = tracked(st, project_id)?;
    Ok(gizai_docs::guides::guides(&repo, &files))
}

/// The project's repository and the files git tracks in it.
fn tracked(st: &AppState, project_id: &str) -> Result<(std::path::PathBuf, Vec<String>), String> {
    let (project, repo) = crate::changes::docs_repo(st, project_id)?;
    let files = gizai_agents::worktree::tracked_files(&repo)
        .map_err(|e| format!("Hersir can't list {}'s files: {}", project.name, crate::pulls::plain(e)))?;
    Ok((repo, files))
}
