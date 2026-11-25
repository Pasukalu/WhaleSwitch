use super::{blocking, pick_project_folder};
use crate::{Engine, Error, ProjectSkills, Result, SkillBackup, SkillEditPreview, SkillImportPreview, SkillLocation};
use std::{path::PathBuf, sync::Mutex};
use tauri::{AppHandle, Manager};

#[derive(Default)]
pub(super) struct SkillWorkspace(Mutex<Option<Selection>>);

struct Selection {
    project: PathBuf,
    source: Option<PathBuf>,
}

impl SkillWorkspace {
    fn with_project<T>(&self, root: &str, action: impl FnOnce(&mut Selection) -> Result<T>) -> Result<T> {
        let mut guard = self.0.lock().map_err(|_| Error::Io)?;
        let project = guard.as_mut().ok_or(Error::PathUnavailable)?;
        if project.project.to_str() != Some(root) { return Err(Error::Conflict); }
        action(project)
    }
}

#[tauri::command]
pub(super) async fn pick_project_skills(app: AppHandle) -> Result<ProjectSkills> {
    blocking(move || {
        let path = pick_project_folder(&app)?;
        path.to_str().ok_or(Error::UnsafePath)?;
        let report = app.state::<Engine>().project_skills(&path)?;
        *app.state::<SkillWorkspace>().0.lock().map_err(|_| Error::Io)? = Some(Selection { project: path, source: None });
        Ok(report)
    }).await
}

#[tauri::command]
pub(super) async fn refresh_project_skills(app: AppHandle, root: String) -> Result<ProjectSkills> {
    blocking(move || app.state::<SkillWorkspace>().with_project(&root,
        |selection| app.state::<Engine>().project_skills(&selection.project))).await
}

#[tauri::command]
pub(super) async fn capture_skill_baseline(app: AppHandle, root: String, revision: String) -> Result<ProjectSkills> {
    blocking(move || app.state::<SkillWorkspace>().with_project(&root,
        |selection| app.state::<Engine>().capture_skill_baseline(&selection.project, &revision))).await
}

#[tauri::command]
pub(super) async fn pick_skill_import(app: AppHandle, root: String, location: SkillLocation) -> Result<SkillImportPreview> {
    blocking(move || {
        app.state::<SkillWorkspace>().with_project(&root, |_| Ok(()))?;
        let source = pick_project_folder(&app)?;
        app.state::<SkillWorkspace>().with_project(&root, |selection| {
            let report = app.state::<Engine>().preview_skill_import(&selection.project, &source, location)?;
            selection.source = Some(source);
            Ok(report)
        })
    }).await
}

#[tauri::command]
pub(super) async fn preview_skill_import(app: AppHandle, root: String, location: SkillLocation) -> Result<SkillImportPreview> {
    blocking(move || app.state::<SkillWorkspace>().with_project(&root, |selection| {
        let source = selection.source.as_deref().ok_or(Error::PathUnavailable)?;
        app.state::<Engine>().preview_skill_import(&selection.project, source, location)
    })).await
}

#[tauri::command]
pub(super) async fn apply_skill_import(app: AppHandle, root: String, location: SkillLocation, revision: String) -> Result<()> {
    blocking(move || app.state::<SkillWorkspace>().with_project(&root, |selection| {
        let source = selection.source.as_deref().ok_or(Error::PathUnavailable)?;
        app.state::<Engine>().apply_skill_import(&selection.project, source, location, &revision)
    })).await
}

#[tauri::command]
pub(super) async fn preview_skill_upgrade(app: AppHandle, root: String, location: SkillLocation) -> Result<SkillEditPreview> {
    blocking(move || app.state::<SkillWorkspace>().with_project(&root, |selection| {
        let source = selection.source.as_deref().ok_or(Error::PathUnavailable)?;
        app.state::<Engine>().preview_skill_upgrade(&selection.project, source, location)
    })).await
}

#[tauri::command]
pub(super) async fn apply_skill_upgrade(app: AppHandle, root: String, location: SkillLocation,
    revision: String, selected: Vec<String>) -> Result<SkillBackup> {
    blocking(move || app.state::<SkillWorkspace>().with_project(&root, |selection| {
        let source = selection.source.as_deref().ok_or(Error::PathUnavailable)?;
        app.state::<Engine>().apply_skill_upgrade(&selection.project, source, location, &revision, &selected)
    })).await
}

#[tauri::command]
pub(super) async fn skill_backups(app: AppHandle, root: String) -> Result<Vec<SkillBackup>> {
    blocking(move || app.state::<SkillWorkspace>().with_project(&root,
        |selection| app.state::<Engine>().skill_backups(&selection.project))).await
}

#[tauri::command]
pub(super) async fn preview_skill_restore(app: AppHandle, root: String, id: i64) -> Result<SkillEditPreview> {
    blocking(move || app.state::<SkillWorkspace>().with_project(&root,
        |selection| app.state::<Engine>().preview_skill_restore(&selection.project, id))).await
}

#[tauri::command]
pub(super) async fn apply_skill_restore(app: AppHandle, root: String, id: i64,
    revision: String, selected: Vec<String>) -> Result<SkillBackup> {
    blocking(move || app.state::<SkillWorkspace>().with_project(&root, |selection| {
        app.state::<Engine>().apply_skill_restore(&selection.project, id, &revision, &selected)
    })).await
}
