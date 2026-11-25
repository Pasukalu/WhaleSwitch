use super::{ensure_directory, import_draft, portable_component, remove_empty_directories, skill_name,
    text_extension, MAX_BYTES, MAX_NODES, MAX_REFERENCE_DEPTH};
use super::super::{digest, directory_exists, metadata, persist, project_skills::project_key, read_state, stage_file,
    sync_parent, Engine, FileState, MISSING};
use crate::{skills::MAX_MANIFEST_BYTES, Error, Result, SkillBackup, SkillLocation};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, path::{Path, PathBuf}};
use zeroize::{Zeroize, Zeroizing};

#[derive(Serialize)]
pub struct SkillTextChange {
    pub path: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub blocked: bool,
}

#[derive(Serialize)]
pub struct SkillEditPreview {
    pub destination: String,
    pub revision: String,
    pub changes: Vec<SkillTextChange>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchiveFile {
    path: String,
    before: Option<String>,
    after_revision: String,
}

impl Drop for ArchiveFile {
    fn drop(&mut self) {
        if let Some(text) = &mut self.before { text.zeroize(); }
    }
}

struct Edit {
    path: String,
    before: FileState,
    before_revision: String,
    after: Option<Zeroizing<Vec<u8>>>,
    after_revision: String,
    blocked: bool,
}

struct EditDraft {
    project: String,
    destination: String,
    target: PathBuf,
    revision: String,
    edits: Vec<Edit>,
}

impl Engine {
    pub fn preview_skill_upgrade(&self, root: &Path, source: &Path, location: SkillLocation) -> Result<SkillEditPreview> {
        upgrade_draft(root, source, location)?.preview()
    }

    pub fn apply_skill_upgrade(&self, root: &Path, source: &Path, location: SkillLocation, revision: &str,
        selected: &[String]) -> Result<SkillBackup> {
        let _lock = self.storage.lock()?;
        self.commit_skill_edits(upgrade_draft(root, source, location)?, "upgrade", revision, selected, sync_parent)
    }

    pub fn skill_backups(&self, root: &Path) -> Result<Vec<SkillBackup>> {
        self.storage.skill_backups(&checked_project(root)?)
    }

    pub fn preview_skill_restore(&self, root: &Path, id: i64) -> Result<SkillEditPreview> {
        self.restore_skill_draft(root, id)?.preview()
    }

    pub fn apply_skill_restore(&self, root: &Path, id: i64, revision: &str, selected: &[String]) -> Result<SkillBackup> {
        let _lock = self.storage.lock()?;
        self.commit_skill_edits(self.restore_skill_draft(root, id)?, "restore", revision, selected, sync_parent)
    }

    fn restore_skill_draft(&self, root: &Path, id: i64) -> Result<EditDraft> {
        let project = checked_project(root)?;
        let (saved, bytes) = self.storage.read_skill_backup(self.vault.as_ref(), &project, id)?;
        let target = existing_target(root, &saved.destination)?;
        let files = serde_json::from_slice::<Vec<ArchiveFile>>(&bytes).map_err(|_| Error::BackupFailed)?;
        if files.is_empty() || files.len() > MAX_NODES || files.len() != saved.file_count as usize {
            return Err(Error::BackupFailed);
        }
        let mut names = BTreeSet::new();
        let mut size = 0;
        let mut edits = Vec::new();
        for file in files {
            if !editable_path(&file.path) || !names.insert(file.path.to_ascii_lowercase())
                || (file.path == "SKILL.md" && file.before.is_none()) {
                return Err(Error::BackupFailed);
            }
            let after = file.before.as_ref().map(|text| Zeroizing::new(text.as_bytes().to_vec()));
            if let Some(bytes) = &after {
                size += bytes.len();
                if bytes.len() as u64 > MAX_MANIFEST_BYTES || size > MAX_BYTES {
                    return Err(Error::BackupFailed);
                }
            }
            edits.push(read_edit(&target, file.path.clone(), after, Some(&file.after_revision))?);
        }
        EditDraft::new(project, saved.destination, target, &format!("restore:{id}"), edits)
    }

    fn commit_skill_edits(&self, draft: EditDraft, action: &str, revision: &str, selected: &[String],
        sync: impl FnMut(&Path) -> Result<()>) -> Result<SkillBackup> {
        if revision != draft.revision { return Err(Error::Conflict); }
        let choice: BTreeSet<_> = selected.iter().map(String::as_str).collect();
        if choice.is_empty() || choice.len() != selected.len() || choice.len() > MAX_NODES {
            return Err(Error::SkillEditSelection);
        }
        let edits: Vec<_> = draft.edits.iter().filter(|edit| !edit.blocked && choice.contains(edit.path.as_str())).collect();
        if edits.len() != choice.len() { return Err(Error::SkillEditSelection); }
        let files: Vec<ArchiveFile> = edits.iter().map(|edit| Ok(ArchiveFile {
            path: edit.path.clone(),
            before: text(&edit.before.bytes)?.map(str::to_owned),
            after_revision: edit.after_revision.clone(),
        })).collect::<Result<_>>()?;
        let serialized = Zeroizing::new(serde_json::to_vec(&files).map_err(|_| Error::BackupFailed)?);
        let backup = self.storage.insert_skill_backup(self.vault.as_ref(), &draft.project, &draft.destination,
            action, files.len() as u32, &serialized)?;
        execute(&draft.target, &edits, sync)?;
        Ok(backup)
    }
}

impl EditDraft {
    fn new(project: String, destination: String, target: PathBuf, binding: &str, mut edits: Vec<Edit>) -> Result<Self> {
        let size: usize = edits.iter().filter_map(|edit| edit.before.bytes.as_ref()).map(|bytes| bytes.len()).sum();
        if size > MAX_BYTES { return Err(Error::SkillImportLimit); }
        edits.sort_unstable_by(|left, right| {
            (left.path == "SKILL.md", &left.path).cmp(&(right.path == "SKILL.md", &right.path))
        });
        let fingerprints: Vec<_> = edits.iter().map(|edit| (&edit.path, &edit.before_revision, &edit.after_revision, edit.blocked)).collect();
        let encoded = serde_json::to_vec(&(&project, &destination, binding, fingerprints)).map_err(|_| Error::BackupFailed)?;
        edits.retain(|edit| edit.before_revision != edit.after_revision);
        Ok(Self { project, destination, target, revision: digest(&encoded), edits })
    }

    fn preview(self) -> Result<SkillEditPreview> {
        let changes = self.edits.into_iter().rev().map(|edit| Ok(SkillTextChange {
            path: edit.path,
            before: text(&edit.before.bytes)?.map(str::to_owned),
            after: text(&edit.after)?.map(str::to_owned),
            blocked: edit.blocked,
        })).collect::<Result<_>>()?;
        Ok(SkillEditPreview { destination: self.destination, revision: self.revision, changes })
    }
}

fn upgrade_draft(root: &Path, source: &Path, location: SkillLocation) -> Result<EditDraft> {
    let project = checked_project(root)?;
    let import = import_draft(root, source, location)?;
    let target = existing_target(root, &import.destination)?;
    let edits = import.documents.into_iter().map(|doc| read_edit(&target, doc.path, Some(doc.bytes), None))
        .collect::<Result<_>>()?;
    EditDraft::new(project, import.destination, target, &import.revision, edits)
}

fn checked_project(root: &Path) -> Result<String> {
    if !directory_exists(root)? { return Err(Error::PathUnavailable); }
    project_key(root)
}

fn existing_target(root: &Path, destination: &str) -> Result<PathBuf> {
    let (prefix, name) = destination.rsplit_once('/').ok_or(Error::UnsafePath)?;
    if !skill_name(name) || !SkillLocation::ALL.iter().any(|location| location.directory() == prefix) {
        return Err(Error::UnsafePath);
    }
    let target = root.join(destination);
    if !directory_exists(&target)? || metadata(&target.join("SKILL.md"))?.is_none() {
        return Err(Error::SkillEditTarget);
    }
    Ok(target)
}

fn editable_path(path: &str) -> bool {
    if path == "SKILL.md" { return true; }
    let Some(reference) = path.strip_prefix("references/") else { return false; };
    reference.split('/').count() <= MAX_REFERENCE_DEPTH
        && reference.split('/').all(portable_component)
        && text_extension(reference)
}

fn text(bytes: &Option<Zeroizing<Vec<u8>>>) -> Result<Option<&str>> {
    bytes.as_ref().map(|bytes| {
        let value = std::str::from_utf8(bytes).map_err(|_| Error::SkillImportManifest)?;
        if value.contains('\0') { return Err(Error::SkillImportManifest); }
        Ok(value)
    }).transpose()
}

fn read_edit(target: &Path, path: String, after: Option<Zeroizing<Vec<u8>>>, restore_from: Option<&str>) -> Result<Edit> {
    let before = read_state(&target.join(&path), MAX_MANIFEST_BYTES)?;
    if path == "SKILL.md" && before.bytes.is_none() { return Err(Error::SkillEditTarget); }
    text(&before.bytes)?;
    text(&after)?;
    let before_revision = before.revision();
    let after_revision = after.as_ref().map_or_else(|| MISSING.into(), |bytes| digest(bytes));
    let blocked = restore_from.is_some_and(|original| original != before_revision.as_str());
    Ok(Edit { path, before, before_revision, after, after_revision, blocked })
}

fn execute(target: &Path, edits: &[&Edit], mut sync: impl FnMut(&Path) -> Result<()>) -> Result<()> {
    let mut written = Vec::new();
    let mut directories = Vec::new();
    let result = (|| {
        for edit in edits {
            let path = target.join(&edit.path);
            if edit.after.is_some() {
                ensure_directory(target, path.parent().ok_or(Error::UnsafePath)?, &mut directories)?;
            }
            write_checked(&path, &edit.before_revision, &edit.after, edit.before.permissions.as_ref())?;
            written.push(*edit);
            sync(&path)?;
        }
        for edit in edits {
            if read_state(&target.join(&edit.path), MAX_MANIFEST_BYTES)?.revision() != edit.after_revision {
                return Err(Error::Conflict);
            }
        }
        for directory in directories.iter().rev() { sync_parent(directory)?; }
        Ok(())
    })();
    if let Err(error) = result {
        let mut recovered = true;
        for edit in written.iter().rev() {
            let path = target.join(&edit.path);
            recovered &= write_checked(&path, &edit.after_revision, &edit.before.bytes, edit.before.permissions.as_ref()).is_ok()
                && sync_parent(&path).is_ok()
                && read_state(&path, MAX_MANIFEST_BYTES).is_ok_and(|state| state.revision() == edit.before_revision);
        }
        if !remove_empty_directories(&directories) || !recovered { return Err(Error::SkillEditRecovery); }
        return Err(if written.is_empty() { error } else { Error::SkillEditRolledBack });
    }
    Ok(())
}

fn write_checked(path: &Path, expected: &str, replacement: &Option<Zeroizing<Vec<u8>>>,
    permissions: Option<&fs::Permissions>) -> Result<()> {
    if read_state(path, MAX_MANIFEST_BYTES)?.revision() != expected { return Err(Error::Conflict); }
    if let Some(bytes) = replacement {
        let stage = stage_file(path, bytes, permissions)?;
        if read_state(path, MAX_MANIFEST_BYTES)?.revision() != expected { return Err(Error::Conflict); }
        persist(stage, path, expected != MISSING)
    } else {
        fs::remove_file(path).map_err(|_| Error::WriteFailed)
    }
}

#[cfg(test)]
mod tests;
