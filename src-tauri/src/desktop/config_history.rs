use super::{blocking, save_archive_as};
use crate::{ConfigActivity, Engine, Result, SnapshotCleanupPreview, SnapshotCleanupResult, Tool};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub(super) async fn pin_snapshot(app: AppHandle, tool: Tool, path: String, id: i64, pinned: bool) -> Result<()> {
    blocking(move || app.state::<Engine>().pin_snapshot(tool, &path, id, pinned)).await
}

#[tauri::command]
pub(super) async fn preview_snapshot_cleanup(app: AppHandle, tool: Tool, path: String) -> Result<SnapshotCleanupPreview> {
    blocking(move || app.state::<Engine>().preview_snapshot_cleanup(tool, &path)).await
}

#[tauri::command]
pub(super) async fn apply_snapshot_cleanup(app: AppHandle, tool: Tool, path: String, revision: String,
    selected: Vec<i64>) -> Result<SnapshotCleanupResult> {
    blocking(move || app.state::<Engine>().apply_snapshot_cleanup(tool, &path, &revision, &selected)).await
}

#[tauri::command]
pub(super) async fn config_activity(app: AppHandle, tool: Tool) -> Result<Vec<ConfigActivity>> {
    blocking(move || app.state::<Engine>().config_activity(tool)).await
}

#[tauri::command]
pub(super) async fn export_config_activity(app: AppHandle, tool: Tool) -> Result<()> {
    blocking(move || {
        let name = format!("whaleswitch-{}-activity.json", tool.id());
        let path = save_archive_as(&app, name, "WhaleSwitch Activity JSON")?;
        app.state::<Engine>().export_config_activity(tool, &path)
    }).await
}
