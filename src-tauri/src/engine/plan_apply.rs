use super::{difference, digest, patched_config, persist, read_archive, read_state, stage_file, sync_parent, write_archive,
    Engine, FileState, MAX_CONFIG};
use crate::{plans::PlanJournalEntry, ConfigPlan, Error, Fields, PlanArchive, PlanEntry, PlanFileResult, PlanFileStatus,
    PlanPreview, PlanProfile, PlanRun, PlanSelection, Result, Tool};
use std::{fs, io, path::{Path, PathBuf}};
use tempfile::NamedTempFile;
use zeroize::Zeroizing;

struct PlanWrite {
    path: PathBuf,
    state: FileState,
    bytes: Zeroizing<Vec<u8>>,
    stage: Option<NamedTempFile>,
    written: bool,
    journal: PlanJournalEntry,
}

impl Engine {
    pub fn plans(&self) -> Result<Vec<ConfigPlan>> { self.storage.plans() }

    pub fn create_plan(&self, name: &str, profiles: &[PlanProfile]) -> Result<ConfigPlan> {
        if profiles.is_empty() || profiles.len() > Tool::ALL.len() { return Err(Error::InvalidPlan); }
        let _lock = self.storage.lock()?;
        let mut entries = Vec::with_capacity(profiles.len());
        for profile in profiles {
            let saved = self.storage.profile(profile.tool, profile.profile_id)?;
            entries.push(PlanEntry { tool: profile.tool, fields: saved.fields });
        }
        self.storage.create_plan(name, entries)
    }

    pub fn delete_plan(&self, id: i64) -> Result<()> {
        let _lock = self.storage.lock()?;
        self.storage.delete_plan(id)
    }

    pub fn read_plan_archive(&self, source: &Path) -> Result<PlanArchive> {
        let archive: PlanArchive = read_archive(source)?;
        archive.validate()?;
        Ok(archive)
    }

    pub fn import_plan(&self, archive: PlanArchive, name: &str) -> Result<ConfigPlan> {
        archive.validate()?;
        let _lock = self.storage.lock()?;
        self.storage.create_plan(name, archive.entries)
    }

    pub fn export_plan(&self, id: i64, destination: &Path) -> Result<()> {
        let plan = self.storage.plan(id)?;
        write_archive(destination, &PlanArchive { version: 1, name: plan.name, entries: plan.entries })
    }

    pub fn preview_plan(&self, id: i64) -> Result<Vec<PlanPreview>> {
        let plan = self.storage.plan(id)?;
        Ok(plan.entries.iter().map(|entry| self.preview_plan_file(entry)).collect())
    }

    fn preview_plan_file(&self, entry: &PlanEntry) -> PlanPreview {
        let mut preview = PlanPreview { tool: entry.tool, path: None, revision: None, changes: Vec::new(), issue: None };
        let read = (|| -> Result<(String, Vec<crate::Change>)> {
            let path = self.target(entry.tool)?;
            preview.path = Some(path.to_str().ok_or(Error::UnsafePath)?.to_owned());
            let state = read_state(&path, MAX_CONFIG)?;
            let changes = plan_changes(entry, &path, &state)?;
            if !changes.is_empty() {
                let patch = changes.iter().map(|change| (change.field, change.after.clone())).collect();
                patched_config(entry.tool, &path, &state, &patch)?;
            }
            Ok((state.revision(), changes))
        })();
        match read {
            Ok((revision, changes)) => {
                preview.revision = Some(revision);
                preview.changes = changes;
            }
            Err(error) => preview.issue = Some(error),
        }
        preview
    }

    pub fn apply_plan(&self, id: i64, selections: &[PlanSelection]) -> Result<PlanRun> {
        self.apply_plan_with_sync(id, selections, sync_parent)
    }

    fn apply_plan_with_sync(&self, id: i64, selections: &[PlanSelection],
        mut sync: impl FnMut(&Path) -> Result<()>) -> Result<PlanRun> {
        if selections.is_empty() || selections.len() > Tool::ALL.len() { return Err(Error::InvalidPlan); }
        let _lock = self.storage.lock()?;
        let plan = self.storage.plan(id)?;
        let mut writes = Vec::with_capacity(selections.len());
        let mut identities = Vec::with_capacity(selections.len());
        for (index, selection) in selections.iter().enumerate() {
            if selection.fields.is_empty() || selections[..index].iter().any(|other| other.tool == selection.tool) {
                return Err(Error::InvalidPlan);
            }
            let entry = plan.entries.iter().find(|entry| entry.tool == selection.tool).ok_or(Error::InvalidPlan)?;
            let write = self.prepare_plan_write(entry, selection)?;
            let identity = target_identity(&write.path)?;
            if identities.contains(&identity) { return Err(Error::PlanOverlap); }
            identities.push(identity);
            writes.push(write);
        }

        for write in &mut writes {
            self.check_plan_revision(write, &write.journal.before)?;
            let parent = write.path.parent().ok_or(Error::UnsafePath)?;
            fs::create_dir_all(parent).map_err(|_| Error::WriteFailed)?;
            super::validate_ancestors(&write.path)?;
            write.stage = Some(stage_file(&write.path, &write.bytes, write.state.permissions.as_ref())?);
            if let Some(previous) = &write.state.bytes {
                let snapshot = self.storage.insert(self.vault.as_ref(), write.journal.file.tool,
                    &write.path, "before_plan", previous)?;
                write.journal.file.snapshot_id = Some(snapshot.id);
            }
        }
        for write in &writes { self.check_plan_revision(write, &write.journal.before)?; }
        let mut run = self.storage.begin_plan_run(plan.name, writes.iter().map(|write| write.journal.clone()).collect())?;

        let mut failed = false;
        for write in &mut writes {
            if failed {
                write.journal.file.status = PlanFileStatus::Skipped;
                continue;
            }
            match self.commit_plan_file(write, &mut sync) {
                Ok(()) => write.journal.file.status = PlanFileStatus::Applied,
                Err(error) => {
                    failed = true;
                    write.journal.file.status = PlanFileStatus::Failed;
                    write.journal.file.issue = Some(error);
                }
            }
        }
        if !failed {
            for write in &mut writes {
                if let Err(error) = self.check_plan_revision(write, &write.journal.after) {
                    failed = true;
                    write.journal.file.issue = Some(error);
                }
            }
        }
        if failed {
            for write in writes.iter_mut().rev().filter(|write| write.written) {
                match self.compensate_plan_file(write) {
                    Ok(()) => write.journal.file.status = PlanFileStatus::Restored,
                    Err(error) => {
                        write.journal.file.status = PlanFileStatus::RecoveryRequired;
                        write.journal.file.issue = Some(error);
                    }
                }
            }
        }
        run.entries = writes.into_iter().map(|write| write.journal).collect();
        run.finished = self.storage.finish_plan_run(&run).is_ok();
        Ok(run.report())
    }

    fn prepare_plan_write(&self, entry: &PlanEntry, selection: &PlanSelection) -> Result<PlanWrite> {
        let tool = selection.tool;
        tool.validate(&selection.fields)?;
        let path = self.bound_target(tool, &selection.path)?;
        let state = read_state(&path, MAX_CONFIG)?;
        if state.revision() != selection.revision { return Err(Error::Conflict); }
        let changes = plan_changes(entry, &path, &state)?;
        if selection.fields.iter().any(|(field, value)| {
            !changes.iter().any(|change| change.field == *field && change.after == *value)
        }) {
            return Err(Error::Conflict);
        }
        let bytes = patched_config(tool, &path, &state, &selection.fields)?;
        let journal = PlanJournalEntry {
            file: PlanFileResult { tool, snapshot_id: None, status: PlanFileStatus::Pending, issue: None },
            path_digest: digest(selection.path.as_bytes()),
            before: selection.revision.clone(),
            after: digest(&bytes),
        };
        Ok(PlanWrite { path, state, bytes, stage: None, written: false, journal })
    }

    fn check_plan_revision(&self, write: &PlanWrite, expected: &str) -> Result<()> {
        if self.target(write.journal.file.tool)? != write.path || read_state(&write.path, MAX_CONFIG)?.revision() != expected {
            return Err(Error::Conflict);
        }
        Ok(())
    }

    fn commit_plan_file(&self, write: &mut PlanWrite, sync: &mut impl FnMut(&Path) -> Result<()>) -> Result<()> {
        self.check_plan_revision(write, &write.journal.before)?;
        let stage = write.stage.take().ok_or(Error::PlanStorage)?;
        persist(stage, &write.path, write.state.bytes.is_some())?;
        write.written = true;
        self.check_plan_revision(write, &write.journal.after)?;
        sync(&write.path)
    }

    fn compensate_plan_file(&self, write: &PlanWrite) -> Result<()> {
        if self.check_plan_revision(write, &write.journal.before).is_ok() { return Ok(()); }
        self.check_plan_revision(write, &write.journal.after)?;
        if let Some(previous) = &write.state.bytes {
            let staged = stage_file(&write.path, previous, write.state.permissions.as_ref())?;
            self.check_plan_revision(write, &write.journal.after)?;
            persist(staged, &write.path, true)?;
        } else {
            fs::remove_file(&write.path).map_err(|_| Error::RecoveryRequired)?;
        }
        sync_parent(&write.path)?;
        self.check_plan_revision(write, &write.journal.before)
    }

    pub fn plan_runs(&self) -> Result<Vec<PlanRun>> {
        let mut stored = self.storage.plan_runs()?;
        if stored.iter().any(|run| !run.finished) {
            match self.storage.lock() {
                Ok(_lock) => {
                    stored = self.storage.plan_runs()?;
                    for run in stored.iter_mut().filter(|run| !run.finished) {
                        for entry in &mut run.entries {
                            entry.file.status = self.inspect_interrupted_plan_file(entry);
                        }
                    }
                }
                Err(Error::Conflict) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(stored.into_iter().map(|run| run.report()).collect())
    }

    fn inspect_interrupted_plan_file(&self, entry: &PlanJournalEntry) -> PlanFileStatus {
        let revision = self.target(entry.file.tool).ok().and_then(|path| {
            let label = path.to_str()?;
            if digest(label.as_bytes()) != entry.path_digest { return None; }
            read_state(&path, MAX_CONFIG).ok().map(|state| state.revision())
        });
        match revision.as_deref() {
            Some(value) if value == entry.before => PlanFileStatus::Unchanged,
            Some(value) if value == entry.after => PlanFileStatus::Applied,
            _ => PlanFileStatus::RecoveryRequired,
        }
    }
}

fn plan_changes(entry: &PlanEntry, path: &Path, state: &FileState) -> Result<Vec<crate::Change>> {
    let format = entry.tool.format(path);
    let before = entry.tool.read_verified(format, state.text(format)?)?;
    let after = entry.fields.iter().filter(|(field, _)| before.contains_key(*field))
        .map(|(&field, value)| (field, value.clone())).collect();
    Ok(difference(&before, &after))
}

fn target_identity(path: &Path) -> Result<String> {
    let mut suffix = Vec::new();
    let mut cursor = path;
    let mut resolved = loop {
        match cursor.canonicalize() {
            Ok(path) => break path,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                suffix.push(cursor.file_name().ok_or(Error::UnsafePath)?);
                cursor = cursor.parent().ok_or(Error::UnsafePath)?;
            }
            Err(_) => return Err(Error::Io),
        }
    };
    for part in suffix.into_iter().rev() { resolved.push(part); }
    let text = resolved.to_str().ok_or(Error::UnsafePath)?;
    #[cfg(windows)]
    return Ok(text.to_lowercase());
    #[cfg(not(windows))]
    Ok(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Field, SecretStore};
    use std::sync::Mutex;
    use tempfile::TempDir;

    #[derive(Default)]
    struct Vault(Mutex<Option<Zeroizing<String>>>);

    impl SecretStore for Vault {
        fn get(&self, _: &str) -> Result<Option<Zeroizing<String>>> { Ok(self.0.lock().unwrap().clone()) }
        fn set(&self, _: &str, secret: &str) -> Result<()> {
            *self.0.lock().unwrap() = Some(Zeroizing::new(secret.to_owned()));
            Ok(())
        }
        fn remove(&self, _: &str) -> Result<()> {
            *self.0.lock().unwrap() = None;
            Ok(())
        }
    }

    const ORIGINAL: [&str; 3] = [
        "model = 'old'\n# preserve this comment\n",
        r#"{"model":"sonnet","other":true}"#,
        "{\n  // leave this comment\n  \"model\": \"openai/old\",\n}\n",
    ];

    struct Fixture { engine: Engine, files: [PathBuf; 3], _temp: TempDir }

    fn fixture() -> Fixture {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let files = [root.join("config.toml"), root.join("settings.json"), root.join("opencode.jsonc")];
        for (file, contents) in files.iter().zip(ORIGINAL) {
            fs::write(file, contents).unwrap();
        }
        let engine = Engine::isolated(root.join("data"), files.clone(), Box::new(Vault::default()));
        Fixture { engine, files, _temp: temp }
    }

    fn model(value: &str) -> Fields { [(Field::Model, Some(value.to_owned()))].into_iter().collect() }

    fn archive() -> PlanArchive {
        let names = ["new-model", "opus", "openai/new"];
        let entries = Tool::ALL.into_iter().zip(names)
            .map(|(tool, value)| PlanEntry { tool, fields: model(value) }).collect();
        PlanArchive { version: 1, name: "Local inference".into(), entries }
    }

    fn selections(engine: &Engine, id: i64) -> Vec<PlanSelection> {
        engine.preview_plan(id).unwrap().into_iter().map(|preview| PlanSelection {
            tool: preview.tool, path: preview.path.unwrap(), revision: preview.revision.unwrap(),
            fields: preview.changes.into_iter().map(|change| (change.field, change.after)).collect(),
        }).collect()
    }

    #[test]
    fn round_trips_a_plan_and_applies_a_selected_subset_after_backing_up_all_targets() {
        let f = fixture();
        let view = f.engine.view(Tool::Codex).unwrap();
        let profile = f.engine.create_profile(Tool::Codex, &view.path, &view.revision, "Local model", &model("new-model")).unwrap();
        let copied = f.engine.create_plan("Copied profile", &[PlanProfile { tool: Tool::Codex, profile_id: profile.id }]).unwrap();
        f.engine.delete_profile(Tool::Codex, profile.id).unwrap();
        assert_eq!(selections(&f.engine, copied.id)[0].fields, model("new-model"));
        let mut portable_archive = archive();
        portable_archive.entries[0].fields.insert(Field::OpenaiBaseUrl, None);
        let portable = f.engine.import_plan(portable_archive, "Local inference").unwrap();
        let exported = f.files[0].with_file_name("plan.json");
        f.engine.export_plan(portable.id, &exported).unwrap();
        let json = fs::read_to_string(&exported).unwrap();
        assert!(!json.contains("config.toml") && !json.contains("revision"));
        let imported = f.engine.read_plan_archive(&exported).unwrap();
        assert_eq!(imported.entries.len(), 3);
        assert_eq!(imported.entries[0].fields.get(&Field::OpenaiBaseUrl), Some(&None));
        let imported = f.engine.import_plan(imported, "Imported plan").unwrap();
        let mut reviewed = selections(&f.engine, imported.id);
        reviewed.remove(1);
        let report = f.engine.apply_plan_with_sync(imported.id, &reviewed, |_| {
            assert_eq!(f.engine.snapshots(Tool::Codex).unwrap()[0].reason, "before_plan");
            assert_eq!(f.engine.snapshots(Tool::Opencode).unwrap()[0].reason, "before_plan");
            Ok(())
        }).unwrap();
        assert!(report.finished);
        assert!(report.files.iter().all(|file| file.status == PlanFileStatus::Applied && file.snapshot_id.is_some()));
        assert!(fs::read_to_string(&f.files[0]).unwrap().contains("# preserve this comment"));
        assert_eq!(fs::read_to_string(&f.files[1]).unwrap(), ORIGINAL[1]);
        assert!(fs::read_to_string(&f.files[2]).unwrap().contains("// leave this comment"));
        assert_eq!(f.engine.plan_runs().unwrap()[0].id, report.id);
    }

    #[test]
    fn preflight_rejects_stale_revisions_before_writing_any_file() {
        let f = fixture();
        let plan = f.engine.import_plan(archive(), "Stale plan").unwrap();
        let reviewed = selections(&f.engine, plan.id);
        fs::write(&f.files[1], r#"{"model":"external"}"#).unwrap();
        assert!(matches!(f.engine.apply_plan(plan.id, &reviewed), Err(Error::Conflict)));
        assert_eq!(fs::read_to_string(&f.files[0]).unwrap(), ORIGINAL[0]);
        assert_eq!(fs::read_to_string(&f.files[2]).unwrap(), ORIGINAL[2]);
        assert!(f.engine.plan_runs().unwrap().is_empty());
    }

    #[test]
    fn rejects_multiple_tools_targeting_the_same_file() {
        let f = fixture();
        let files = [f.files[0].clone(), f.files[1].clone(), f.files[1].clone()];
        let engine = Engine::isolated(f.files[0].with_file_name("overlap"), files, Box::new(Vault::default()));
        let mut plan = archive();
        plan.entries[2].fields = [(Field::DefaultAgent, Some("build".into()))].into_iter().collect();
        let stored = engine.import_plan(plan, "Overlap").unwrap();
        let reviewed = selections(&engine, stored.id);
        assert!(matches!(engine.apply_plan(stored.id, &reviewed), Err(Error::PlanOverlap)));
        assert_eq!(fs::read_to_string(&f.files[0]).unwrap(), ORIGINAL[0]);
        assert_eq!(fs::read_to_string(&f.files[1]).unwrap(), ORIGINAL[1]);
    }

    #[test]
    fn reverses_prior_writes_when_a_later_file_fails() {
        let f = fixture();
        fs::remove_file(&f.files[0]).unwrap();
        let plan = f.engine.import_plan(archive(), "Failure case").unwrap();
        let report = f.engine.apply_plan_with_sync(plan.id, &selections(&f.engine, plan.id), |path| {
            if path == f.files[1].as_path() { Err(Error::WriteFailed) } else { Ok(()) }
        }).unwrap();
        assert!(report.finished);
        assert_eq!(report.files[0].status, PlanFileStatus::Restored);
        assert_eq!(report.files[0].snapshot_id, None);
        assert_eq!(report.files[1].status, PlanFileStatus::Restored);
        assert_eq!(report.files[1].issue, Some(Error::WriteFailed));
        assert_eq!(report.files[2].status, PlanFileStatus::Skipped);
        assert!(!f.files[0].exists());
        assert_eq!(fs::read_to_string(&f.files[1]).unwrap(), ORIGINAL[1]);
        assert_eq!(fs::read_to_string(&f.files[2]).unwrap(), ORIGINAL[2]);
    }

    #[test]
    fn leaves_external_edits_intact_during_compensation() {
        let f = fixture();
        let plan = f.engine.import_plan(archive(), "Concurrent editor").unwrap();
        let external = "model = 'external'\n";
        let report = f.engine.apply_plan_with_sync(plan.id, &selections(&f.engine, plan.id), |path| {
            if path == f.files[1].as_path() {
                fs::write(&f.files[0], external).unwrap();
                Err(Error::WriteFailed)
            } else { Ok(()) }
        }).unwrap();
        assert_eq!(report.files[0].status, PlanFileStatus::RecoveryRequired);
        assert!(report.files[0].snapshot_id.is_some());
        assert_eq!(fs::read_to_string(&f.files[0]).unwrap(), external);
    }

    #[test]
    fn an_unfinished_run_can_be_inspected_without_changing_files() {
        let f = fixture();
        let mut plan = archive();
        plan.entries.truncate(2);
        let stored = f.engine.import_plan(plan, "Interrupted").unwrap();
        let reviewed = selections(&f.engine, stored.id);
        let entries = f.engine.storage.plan(stored.id).unwrap().entries;
        let mut writes = entries.iter().zip(&reviewed)
            .map(|(entry, selection)| f.engine.prepare_plan_write(entry, selection).unwrap()).collect::<Vec<_>>();
        for write in &mut writes {
            let snapshot = f.engine.storage.insert(f.engine.vault.as_ref(), write.journal.file.tool,
                &write.path, "before_plan", write.state.bytes.as_ref().unwrap()).unwrap();
            write.journal.file.snapshot_id = Some(snapshot.id);
        }
        f.engine.storage.begin_plan_run(stored.name, writes.iter().map(|write| write.journal.clone()).collect()).unwrap();
        fs::write(&f.files[0], writes[0].bytes.as_slice()).unwrap();
        let runs = f.engine.plan_runs().unwrap();
        assert!(!runs[0].finished);
        assert_eq!(runs[0].files[0].status, PlanFileStatus::Applied);
        assert_eq!(runs[0].files[1].status, PlanFileStatus::Unchanged);
        assert_eq!(fs::read_to_string(&f.files[1]).unwrap(), ORIGINAL[1]);
    }
}
