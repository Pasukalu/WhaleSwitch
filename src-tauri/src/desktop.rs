use crate::{Change, Engine, Error, Fields, ProbeInput, ProbeResult, Profile, ProfileArchive, ProfilePreview, Result, Snapshot, Tool, ToolInfo, View};
use serde::Deserialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use zeroize::Zeroizing;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target { tool: Tool, path: String, revision: String }

#[tauri::command]
fn discover(engine: tauri::State<'_, Engine>) -> Vec<ToolInfo> { engine.discover() }

#[tauri::command]
fn view(engine: tauri::State<'_, Engine>, tool: Tool) -> Result<View> { engine.view(tool) }

#[tauri::command]
fn preview(engine: tauri::State<'_, Engine>, target: Target, patch: Fields) -> Result<Vec<Change>> {
    engine.preview(target.tool, &target.path, &target.revision, &patch)
}

#[tauri::command]
async fn save(app: AppHandle, target: Target, patch: Fields) -> Result<View> {
    blocking(move || app.state::<Engine>().save(target.tool, &target.path, &target.revision, &patch)).await
}

#[tauri::command]
fn snapshots(engine: tauri::State<'_, Engine>, tool: Tool) -> Result<Vec<Snapshot>> {
    engine.snapshots(tool)
}

#[tauri::command]
async fn profiles(app: AppHandle, tool: Tool) -> Result<Vec<Profile>> {
    blocking(move || app.state::<Engine>().profiles(tool)).await
}

#[tauri::command]
async fn create_profile(app: AppHandle, target: Target, name: String, patch: Fields) -> Result<Profile> {
    blocking(move || app.state::<Engine>().create_profile(target.tool, &target.path, &target.revision, &name, &patch)).await
}

#[tauri::command]
async fn preview_profile(app: AppHandle, target: Target, id: i64) -> Result<ProfilePreview> {
    blocking(move || app.state::<Engine>().preview_profile(target.tool, &target.path, &target.revision, id)).await
}

#[tauri::command]
async fn apply_profile(app: AppHandle, target: Target, id: i64) -> Result<View> {
    blocking(move || app.state::<Engine>().apply_profile(target.tool, &target.path, &target.revision, id)).await
}

#[tauri::command]
async fn delete_profile(app: AppHandle, tool: Tool, id: i64) -> Result<()> {
    blocking(move || app.state::<Engine>().delete_profile(tool, id)).await
}

#[tauri::command]
async fn rename_profile(app: AppHandle, tool: Tool, id: i64, name: String) -> Result<Profile> {
    blocking(move || app.state::<Engine>().rename_profile(tool, id, &name)).await
}

#[tauri::command]
async fn duplicate_profile(app: AppHandle, tool: Tool, id: i64, name: String) -> Result<Profile> {
    blocking(move || app.state::<Engine>().duplicate_profile(tool, id, &name)).await
}

#[tauri::command]
async fn snapshot(app: AppHandle, target: Target) -> Result<Snapshot> {
    blocking(move || app.state::<Engine>().snapshot(target.tool, &target.path, &target.revision)).await
}

#[tauri::command]
async fn compare_snapshot(app: AppHandle, target: Target, id: i64) -> Result<Vec<Change>> {
    blocking(move || app.state::<Engine>().compare_snapshot(target.tool, &target.path, &target.revision, id)).await
}

#[tauri::command]
async fn restore(app: AppHandle, target: Target, id: i64) -> Result<View> {
    blocking(move || app.state::<Engine>().restore(target.tool, &target.path, &target.revision, id)).await
}

#[tauri::command]
async fn import_archive(app: AppHandle, tool: Tool) -> Result<Fields> {
    blocking(move || {
        let path = pick_archive(&app, "WhaleSwitch Configuration JSON")?;
        app.state::<Engine>().import_archive(tool, &path)
    }).await
}

#[tauri::command]
async fn export_archive(app: AppHandle, target: Target) -> Result<()> {
    blocking(move || {
        let name = format!("whaleswitch-{}.json", target.tool.id());
        let path = save_archive_as(&app, name, "WhaleSwitch Configuration JSON")?;
        app.state::<Engine>().export_archive(target.tool, &target.path, &target.revision, &path)
    }).await
}

#[tauri::command]
async fn read_profile_archive(app: AppHandle, tool: Tool) -> Result<ProfileArchive> {
    blocking(move || {
        let path = pick_archive(&app, "WhaleSwitch Profile JSON")?;
        app.state::<Engine>().read_profile_archive(tool, &path)
    }).await
}

#[tauri::command]
async fn import_profile(app: AppHandle, tool: Tool, archive: ProfileArchive, name: String) -> Result<Profile> {
    blocking(move || app.state::<Engine>().import_profile(tool, archive, &name)).await
}

#[tauri::command]
async fn export_profile_archive(app: AppHandle, tool: Tool, id: i64) -> Result<()> {
    blocking(move || {
        let name = format!("whaleswitch-{}-profile.json", tool.id());
        let path = save_archive_as(&app, name, "WhaleSwitch Profile JSON")?;
        app.state::<Engine>().export_profile_archive(tool, id, &path)
    }).await
}

fn pick_archive(app: &AppHandle, label: &str) -> Result<PathBuf> {
    app.dialog().file().add_filter(label, &["json"]).blocking_pick_file().ok_or(Error::Cancelled)?
        .into_path().map_err(|_| Error::UnsafePath)
}

fn save_archive_as(app: &AppHandle, name: String, label: &str) -> Result<PathBuf> {
    app.dialog().file().set_file_name(name).add_filter(label, &["json"]).blocking_save_file().ok_or(Error::Cancelled)?
        .into_path().map_err(|_| Error::UnsafePath)
}

#[tauri::command]
async fn has_key(app: AppHandle, input: ProbeInput) -> Result<bool> {
    blocking(move || app.state::<Engine>().has_key(&input)).await
}

#[tauri::command]
async fn set_key(app: AppHandle, input: ProbeInput, secret: String) -> Result<()> {
    let secret = Zeroizing::new(secret);
    blocking(move || app.state::<Engine>().set_key(&input, secret)).await
}

#[tauri::command]
async fn remove_key(app: AppHandle, input: ProbeInput) -> Result<()> {
    blocking(move || app.state::<Engine>().remove_key(&input)).await
}

#[tauri::command]
async fn probe(app: AppHandle, input: ProbeInput) -> Result<ProbeResult> {
    blocking(move || app.state::<Engine>().probe(&input)).await
}

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tauri::async_runtime::spawn_blocking(work).await.map_err(|_| Error::Io)?
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(Engine::new(app.path().app_local_data_dir()?));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            discover, view, preview, save, snapshot, snapshots, compare_snapshot, restore,
            profiles, create_profile, preview_profile, apply_profile, delete_profile, rename_profile, duplicate_profile,
            import_archive, export_archive, read_profile_archive, import_profile, export_profile_archive,
            has_key, set_key, remove_key, probe,
        ])
        .run(tauri::generate_context!())
        .expect("WhaleSwitch desktop runtime failed");
}
