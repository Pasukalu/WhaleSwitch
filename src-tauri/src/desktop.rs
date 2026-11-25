use crate::{Change, Engine, Error, Fields, ProbeInput, ProbeResult, Profile, ProfileArchive, ProfilePreview, Result, Snapshot, Tool, ToolInfo, View};
use crate::{scan_project_config, ConfigInspection, ProjectConfigReport};
use crate::{ConfigPlan, PlanArchive, PlanPreview, PlanProfile, PlanRun, PlanSelection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use zeroize::Zeroizing;

mod project_skills;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target { tool: Tool, path: String, revision: String }

#[derive(Clone, Serialize)]
struct RuntimeInfo {
    portable: bool,
    data_dir: String,
}

#[tauri::command]
fn runtime_info(info: tauri::State<'_, RuntimeInfo>) -> RuntimeInfo { info.inner().clone() }

#[tauri::command]
fn discover(engine: tauri::State<'_, Engine>) -> Vec<ToolInfo> { engine.discover() }

#[tauri::command]
async fn pick_project_config(app: AppHandle) -> Result<ProjectConfigReport> {
    blocking(move || scan_project_config(&pick_project_folder(&app)?)).await
}

fn pick_project_folder(app: &AppHandle) -> Result<PathBuf> {
    app.dialog().file().blocking_pick_folder().ok_or(Error::Cancelled)?
        .into_path().map_err(|_| Error::UnsafePath)
}

#[tauri::command]
fn view(engine: tauri::State<'_, Engine>, tool: Tool) -> Result<View> { engine.view(tool) }

#[tauri::command]
async fn inspect_config(app: AppHandle, tool: Tool) -> Result<ConfigInspection> {
    blocking(move || Ok(app.state::<Engine>().inspect_config(tool))).await
}

#[tauri::command]
fn preview(engine: tauri::State<'_, Engine>, target: Target, patch: Fields) -> Result<Vec<Change>> {
    engine.preview(target.tool, &target.path, &target.revision, &patch)
}

#[tauri::command]
async fn save(app: AppHandle, target: Target, patch: Fields) -> Result<View> {
    blocking(move || app.state::<Engine>().save(target.tool, &target.path, &target.revision, &patch)).await
}

#[tauri::command]
fn snapshots(engine: tauri::State<'_, Engine>, tool: Tool, path: Option<String>) -> Result<Vec<Snapshot>> {
    match path.as_deref() {
        Some(path) => engine.snapshots_for(tool, path),
        None => engine.snapshots(tool),
    }
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
async fn apply_profile(app: AppHandle, target: Target, id: i64, selection: Option<Fields>) -> Result<View> {
    blocking(move || {
        let engine = app.state::<Engine>();
        match selection {
            Some(fields) => engine.apply_profile_selection(target.tool, &target.path, &target.revision, id, &fields),
            None => engine.apply_profile(target.tool, &target.path, &target.revision, id),
        }
    }).await
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
async fn compare_snapshot_fields(app: AppHandle, target: Target, id: i64) -> Result<Vec<Change>> {
    blocking(move || app.state::<Engine>().compare_snapshot_fields(target.tool, &target.path, &target.revision, id)).await
}

#[tauri::command]
async fn restore_snapshot_fields(app: AppHandle, target: Target, id: i64, selection: Fields) -> Result<View> {
    blocking(move || app.state::<Engine>().restore_snapshot_fields(target.tool, &target.path, &target.revision, id, &selection)).await
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

#[tauri::command]
async fn plans(app: AppHandle) -> Result<Vec<ConfigPlan>> {
    blocking(move || app.state::<Engine>().plans()).await
}

#[tauri::command]
async fn create_plan(app: AppHandle, name: String, profiles: Vec<PlanProfile>) -> Result<ConfigPlan> {
    blocking(move || app.state::<Engine>().create_plan(&name, &profiles)).await
}

#[tauri::command]
async fn delete_plan(app: AppHandle, id: i64) -> Result<()> {
    blocking(move || app.state::<Engine>().delete_plan(id)).await
}

#[tauri::command]
async fn read_plan_archive(app: AppHandle) -> Result<PlanArchive> {
    blocking(move || {
        let path = pick_archive(&app, "WhaleSwitch Plan JSON")?;
        app.state::<Engine>().read_plan_archive(&path)
    }).await
}

#[tauri::command]
async fn import_plan(app: AppHandle, archive: PlanArchive, name: String) -> Result<ConfigPlan> {
    blocking(move || app.state::<Engine>().import_plan(archive, &name)).await
}

#[tauri::command]
async fn export_plan(app: AppHandle, id: i64) -> Result<()> {
    blocking(move || {
        let path = save_archive_as(&app, "whaleswitch-plan.json".to_string(), "WhaleSwitch Plan JSON")?;
        app.state::<Engine>().export_plan(id, &path)
    }).await
}

#[tauri::command]
async fn preview_plan(app: AppHandle, id: i64) -> Result<Vec<PlanPreview>> {
    blocking(move || app.state::<Engine>().preview_plan(id)).await
}

#[tauri::command]
async fn apply_plan(app: AppHandle, id: i64, selections: Vec<PlanSelection>) -> Result<PlanRun> {
    blocking(move || app.state::<Engine>().apply_plan(id, &selections)).await
}

#[tauri::command]
async fn plan_runs(app: AppHandle) -> Result<Vec<PlanRun>> {
    blocking(move || app.state::<Engine>().plan_runs()).await
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
        .manage(project_skills::SkillWorkspace::default())
        .setup(|app| {
            let config = &app.config().app.windows[0];
            let builder = tauri::WebviewWindowBuilder::from_config(app.handle(), config)?;
            let system_data = app.path().app_local_data_dir()?;
            #[cfg(windows)]
            let (builder, data, portable) = match crate::portable::prepare(&std::env::current_exe()?) {
                Ok(Some(layout)) => (builder.data_directory(layout.webview), layout.data, true),
                Ok(None) => (builder, system_data.clone(), false),
                Err(_) => return Err(std::io::Error::other(
                    "Portable storage is unavailable. Extract WhaleSwitch to a regular writable folder.",
                ).into()),
            };
            #[cfg(not(windows))]
            let (data, portable) = (system_data.clone(), false);
            app.manage(RuntimeInfo { portable, data_dir: data.to_string_lossy().into_owned() });
            app.manage(Engine::with_lock_dir(data, system_data));
            builder.build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            discover, runtime_info, view, inspect_config, preview, save, snapshot, snapshots, compare_snapshot, restore,
            compare_snapshot_fields, restore_snapshot_fields,
            profiles, create_profile, preview_profile, apply_profile, delete_profile, rename_profile, duplicate_profile,
            import_archive, export_archive, read_profile_archive, import_profile, export_profile_archive,
            plans, create_plan, delete_plan, read_plan_archive, import_plan, export_plan, preview_plan, apply_plan, plan_runs,
            has_key, set_key, remove_key, probe, pick_project_config,
            project_skills::pick_project_skills, project_skills::refresh_project_skills,
            project_skills::capture_skill_baseline, project_skills::pick_skill_import,
            project_skills::preview_skill_import, project_skills::apply_skill_import,
            project_skills::preview_skill_upgrade, project_skills::apply_skill_upgrade,
            project_skills::skill_backups, project_skills::preview_skill_restore, project_skills::apply_skill_restore,
        ])
        .run(tauri::generate_context!())
        .expect("WhaleSwitch desktop runtime failed");
}
