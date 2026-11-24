use crate::{storage::{SecretStore, Snapshot, Storage, SystemVault}, Error, Field, Fields, Format, Result, Tool, ToolInfo};
use crate::sources::{ConfigInspection, ConfigObservation};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs::{self, File, Permissions}, io::{Read, Write}, path::{Path, PathBuf}};
use zeroize::Zeroizing;
use crate::{Profile, ProfileArchive, ProfilePreview};

pub const MAX_CONFIG: u64 = 2 * 1024 * 1024;
pub const MAX_ARCHIVE: u64 = 64 * 1024;
const MISSING: &str = "missing";

pub struct Engine {
    storage: Storage,
    pub(crate) vault: Box<dyn SecretStore>,
    paths: Option<[PathBuf; 3]>,
}

#[derive(Serialize)]
pub struct View {
    pub tool: Tool,
    pub path: String,
    pub format: Format,
    pub exists: bool,
    pub revision: String,
    pub fields: Fields,
    pub issue: Option<Error>,
}

#[derive(Serialize)]
pub struct Change { pub field: Field, pub before: Option<String>, pub after: Option<String> }

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Export {
    pub version: u16,
    pub tool: Tool,
    pub fields: Fields,
}

struct FileState {
    bytes: Option<Zeroizing<Vec<u8>>>,
    permissions: Option<Permissions>,
}

impl FileState {
    fn revision(&self) -> String {
        self.bytes.as_ref().map_or_else(|| MISSING.to_owned(), |bytes| digest(bytes))
    }

    fn text(&self, format: Format) -> Result<&str> {
        match &self.bytes {
            Some(bytes) => decode(bytes),
            None => Ok(if format == Format::Toml { "" } else { "{}" }),
        }
    }
}

impl Engine {
    pub fn new(app_data: PathBuf) -> Self {
        Self { storage: Storage::new(app_data), vault: Box::new(SystemVault), paths: None }
    }

    #[doc(hidden)]
    pub fn isolated(app_data: PathBuf, paths: [PathBuf; 3], vault: Box<dyn SecretStore>) -> Self {
        Self { storage: Storage::new(app_data), vault, paths: Some(paths) }
    }

    fn target(&self, tool: Tool) -> Result<PathBuf> {
        if let Some(paths) = &self.paths {
            return Ok(paths[match tool { Tool::Codex => 0, Tool::Claude => 1, Tool::Opencode => 2 }].clone());
        }
        tool.path()
    }

    pub fn discover(&self) -> Vec<ToolInfo> {
        Tool::ALL.into_iter().map(|tool| {
            let executable = tool.executable();
            let (path, exists, issue) = match self.target(tool) {
                Ok(path) => {
                    let (exists, issue) = match metadata(&path) {
                        Ok(meta) => (meta.is_some(), None),
                        Err(e) => (false, Some(e)),
                    };
                    (Some(path.to_string_lossy().into_owned()), exists, issue)
                }
                Err(e) => (None, false, Some(e)),
            };
            ToolInfo { tool, installed: executable.is_some(), executable: executable.map(|p| p.to_string_lossy().into_owned()), path, exists, issue }
        }).collect()
    }

    pub fn view(&self, tool: Tool) -> Result<View> {
        let path = self.target(tool)?;
        read_config_view(tool, &path)
    }

    pub fn inspect_config(&self, tool: Tool) -> ConfigInspection {
        let config = match self.target(tool) {
            Ok(path) => match read_config_view(tool, &path) {
                Ok(view) => ConfigObservation::View { view },
                Err(issue) => ConfigObservation::Unavailable { path: path.to_str().map(str::to_owned), issue },
            },
            Err(issue) => ConfigObservation::Unavailable { path: None, issue },
        };
        let active_env = if self.paths.is_none() { tool.active_config_env() } else { Vec::new() };
        ConfigInspection { config, active_env }
    }

    pub fn preview(&self, tool: Tool, target: &str, revision: &str, patch: &Fields) -> Result<Vec<Change>> {
        let path = self.bound_target(tool, target)?;
        let state = read_state(&path, MAX_CONFIG)?;
        if state.revision() != revision { return Err(Error::Conflict); }
        let format = tool.format(&path);
        let text = state.text(format)?;
        let before = tool.read(format, text)?;
        let relevant = changed_patch(&before, patch);
        let after = tool.apply(format, text, &relevant)?;
        tool.validate(patch)?;
        Ok(difference(&before, &tool.read(format, &after)?))
    }

    pub fn save(&self, tool: Tool, target: &str, revision: &str, patch: &Fields) -> Result<View> {
        tool.validate(patch)?;
        let _lock = self.storage.lock()?;
        let path = self.bound_target(tool, target)?;
        let state = read_state(&path, MAX_CONFIG)?;
        if state.revision() != revision { return Err(Error::Conflict); }
        let format = tool.format(&path);
        let text = state.text(format)?;
        let before = tool.read(format, text)?;
        let relevant = changed_patch(&before, patch);
        if relevant.is_empty() { return read_config_view(tool, &path); }
        let edited = Zeroizing::new(tool.apply(format, text, &relevant)?);
        tool.read(format, &edited)?;
        let mut bytes = Zeroizing::new(Vec::new());
        if state.bytes.as_ref().is_some_and(|b| b.starts_with(&[0xef, 0xbb, 0xbf])) { bytes.extend_from_slice(&[0xef, 0xbb, 0xbf]); }
        bytes.extend_from_slice(edited.as_bytes());
        self.commit(tool, &path, &state, &bytes, "before_save")?;
        read_config_view(tool, &path)
    }

    pub fn snapshots(&self, tool: Tool) -> Result<Vec<Snapshot>> {
        self.storage.list(tool, &self.target(tool)?)
    }

    pub fn snapshots_for(&self, tool: Tool, target: &str) -> Result<Vec<Snapshot>> {
        self.storage.list(tool, &self.bound_target(tool, target)?)
    }

    pub fn profiles(&self, tool: Tool) -> Result<Vec<Profile>> {
        self.storage.profiles(tool)
    }

    pub fn rename_profile(&self, tool: Tool, id: i64, name: &str) -> Result<Profile> {
        let _lock = self.storage.lock()?;
        self.storage.rename_profile(tool, id, name)
    }

    pub fn duplicate_profile(&self, tool: Tool, id: i64, name: &str) -> Result<Profile> {
        let _lock = self.storage.lock()?;
        let source = self.storage.profile(tool, id)?;
        self.storage.create_profile(tool, name, source.fields)
    }

    pub fn create_profile(&self, tool: Tool, target: &str, revision: &str, name: &str, patch: &Fields) -> Result<Profile> {
        let _lock = self.storage.lock()?;
        let path = self.bound_target(tool, target)?;
        let state = read_state(&path, MAX_CONFIG)?;
        if state.revision() != revision { return Err(Error::Conflict); }
        let format = tool.format(&path);
        let text = state.text(format)?;
        let current = tool.read(format, text)?;
        tool.validate(patch)?;
        let edited = Zeroizing::new(tool.apply(format, text, &changed_patch(&current, patch))?);
        self.storage.create_profile(tool, name, tool.read(format, &edited)?)
    }

    pub fn preview_profile(&self, tool: Tool, target: &str, revision: &str, id: i64) -> Result<ProfilePreview> {
        let profile = self.storage.profile(tool, id)?;
        Ok(ProfilePreview { name: profile.name, changes: self.preview(tool, target, revision, &profile.fields)? })
    }

    pub fn apply_profile(&self, tool: Tool, target: &str, revision: &str, id: i64) -> Result<View> {
        let profile = self.storage.profile(tool, id)?;
        self.save(tool, target, revision, &profile.fields)
    }

    pub fn apply_profile_selection(&self, tool: Tool, target: &str, revision: &str, id: i64, selection: &Fields) -> Result<View> {
        if selection.is_empty() { return Err(Error::InvalidField); }
        tool.validate(selection)?;
        let profile = self.storage.profile(tool, id)?;
        if selection.iter().any(|(field, value)| profile.fields.get(field) != Some(value)) {
            return Err(Error::Conflict);
        }
        self.save(tool, target, revision, selection)
    }

    pub fn delete_profile(&self, tool: Tool, id: i64) -> Result<()> {
        let _lock = self.storage.lock()?;
        self.storage.delete_profile(tool, id)
    }

    pub fn read_profile_archive(&self, tool: Tool, source: &Path) -> Result<ProfileArchive> {
        let archive: ProfileArchive = read_archive(source)?;
        archive.validate(tool)?;
        Ok(archive)
    }

    pub fn import_profile(&self, tool: Tool, archive: ProfileArchive, name: &str) -> Result<Profile> {
        archive.validate(tool)?;
        let _lock = self.storage.lock()?;
        self.storage.create_profile(tool, name, archive.fields)
    }

    pub fn export_profile_archive(&self, tool: Tool, id: i64, destination: &Path) -> Result<()> {
        let profile = self.storage.profile(tool, id)?;
        let archive = ProfileArchive { version: 1, tool, name: profile.name, fields: profile.fields };
        write_archive(destination, &archive)
    }

    pub fn snapshot(&self, tool: Tool, target: &str, revision: &str) -> Result<Snapshot> {
        let _lock = self.storage.lock()?;
        let path = self.bound_target(tool, target)?;
        let state = read_state(&path, MAX_CONFIG)?;
        if state.revision() != revision { return Err(Error::Conflict); }
        let format = tool.format(&path);
        tool.read(format, state.text(format)?)?;
        let bytes = state.bytes.ok_or(Error::SnapshotMissing)?;
        self.storage.insert(self.vault.as_ref(), tool, &path, "manual", &bytes)
    }

    pub fn compare_snapshot(&self, tool: Tool, target: &str, revision: &str, id: i64) -> Result<Vec<Change>> {
        let path = self.bound_target(tool, target)?;
        let state = read_state(&path, MAX_CONFIG)?;
        if state.revision() != revision { return Err(Error::Conflict); }
        let format = tool.format(&path);
        let bytes = self.storage.get(self.vault.as_ref(), tool, &path, id)?;
        let before = match state.text(format).and_then(|text| tool.read(format, text)) {
            Ok(fields) => fields,
            Err(Error::InvalidConfig) => Fields::new(),
            Err(error) => return Err(error),
        };
        let after = tool.read(format, decode(&bytes)?)?;
        Ok(difference(&before, &after))
    }

    pub fn restore(&self, tool: Tool, target: &str, revision: &str, id: i64) -> Result<View> {
        let _lock = self.storage.lock()?;
        let path = self.bound_target(tool, target)?;
        let state = read_state(&path, MAX_CONFIG)?;
        if state.revision() != revision { return Err(Error::Conflict); }
        let bytes = self.storage.get(self.vault.as_ref(), tool, &path, id)?;
        tool.read(tool.format(&path), decode(&bytes)?)?;
        if state.bytes.as_ref().is_none_or(|old| old.as_slice() != bytes.as_slice()) {
            self.commit(tool, &path, &state, &bytes, "before_restore")?;
        }
        read_config_view(tool, &path)
    }

    pub fn import_archive(&self, tool: Tool, source: &Path) -> Result<Fields> {
        let archive: Export = read_archive(source)?;
        if archive.version != 1 { return Err(Error::InvalidArchive); }
        if archive.tool != tool { return Err(Error::WrongTool); }
        tool.validate(&archive.fields).map_err(|_| Error::InvalidArchive)?;
        Ok(archive.fields)
    }

    pub fn export_archive(&self, tool: Tool, target: &str, revision: &str, destination: &Path) -> Result<()> {
        let path = self.bound_target(tool, target)?;
        let view = read_config_view(tool, &path)?;
        if view.revision != revision { return Err(Error::Conflict); }
        if !view.exists || view.issue.is_some() { return Err(Error::InvalidArchive); }
        let archive = Export { version: 1, tool, fields: view.fields.into_iter().filter(|(_, value)| value.is_some()).collect() };
        write_archive(destination, &archive)
    }

    fn bound_target(&self, tool: Tool, expected: &str) -> Result<PathBuf> {
        let path = self.target(tool)?;
        if path.to_str() != Some(expected) { return Err(Error::Conflict); }
        validate_ancestors(&path)?;
        Ok(path)
    }

    fn commit(&self, tool: Tool, path: &Path, state: &FileState, bytes: &[u8], reason: &str) -> Result<()> {
        self.commit_with_sync(tool, path, state, bytes, reason, sync_parent)
    }

    fn commit_with_sync(&self, tool: Tool, path: &Path, state: &FileState, bytes: &[u8], reason: &str,
        sync: impl FnOnce(&Path) -> Result<()>) -> Result<()> {
        if bytes.len() as u64 > MAX_CONFIG { return Err(Error::FileTooLarge); }
        if let Some(previous) = &state.bytes {
            self.storage.insert(self.vault.as_ref(), tool, path, reason, previous)?;
        }
        let parent = path.parent().ok_or(Error::UnsafePath)?;
        fs::create_dir_all(parent).map_err(|_| Error::WriteFailed)?;
        validate_ancestors(path)?;
        let revision = state.revision();
        if read_state(path, MAX_CONFIG)?.revision() != revision { return Err(Error::Conflict); }
        let stage = stage_file(path, bytes, state.permissions.as_ref())?;
        if self.target(tool)?.as_path() != path || read_state(path, MAX_CONFIG)?.revision() != revision { return Err(Error::Conflict); }
        persist(stage, path, state.bytes.is_some())?;
        let verified = || read_state(path, MAX_CONFIG).is_ok_and(|fresh| fresh.bytes.is_some_and(|b| b.as_slice() == bytes));
        if (verified() || verified()) && sync(path).is_ok() { return Ok(()); }
        if let Some(previous) = &state.bytes {
            if let Ok(recovery) = stage_file(path, previous, state.permissions.as_ref()) {
                if verified() && persist(recovery, path, true).is_ok() && sync_parent(path).is_ok()
                    && read_state(path, MAX_CONFIG).is_ok_and(|restored| restored.revision() == revision) {
                    return Err(Error::RolledBack);
                }
            }
        }
        Err(Error::RecoveryRequired)
    }
}

pub(crate) fn read_config_view(tool: Tool, path: &Path) -> Result<View> {
    let path_text = path.to_str().ok_or(Error::UnsafePath)?;
    let state = read_state(path, MAX_CONFIG)?;
    let format = tool.format(path);
    let (fields, issue) = match state.text(format).and_then(|text| tool.read(format, text)) {
        Ok(fields) => (fields, None),
        Err(Error::InvalidConfig) => (Fields::new(), Some(Error::InvalidConfig)),
        Err(error) => return Err(error),
    };
    Ok(View {
        tool, path: path_text.to_owned(), format, exists: state.bytes.is_some(),
        revision: state.revision(), fields, issue,
    })
}

fn changed_patch(before: &Fields, patch: &Fields) -> Fields {
    patch.iter().filter(|(key, value)| before.get(*key) != Some(*value))
        .map(|(&field, value)| (field, value.clone())).collect()
}

fn difference(before: &Fields, after: &Fields) -> Vec<Change> {
    after.iter().filter_map(|(&field, value)| {
        let old = before.get(&field).cloned().flatten();
        (old != *value).then(|| Change { field, before: old, after: value.clone() })
    }).collect()
}

fn decode(bytes: &[u8]) -> Result<&str> {
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    std::str::from_utf8(bytes).map_err(|_| Error::InvalidConfig)
}

fn read_archive<T: DeserializeOwned>(source: &Path) -> Result<T> {
    let state = read_state(source, MAX_ARCHIVE)?;
    let bytes = state.bytes.ok_or(Error::InvalidArchive)?;
    let text = decode(&bytes).map_err(|_| Error::InvalidArchive)?;
    crate::jsonc::parse(text, false).map_err(|_| Error::InvalidArchive)?;
    serde_json::from_str(text).map_err(|_| Error::InvalidArchive)
}

fn write_archive(destination: &Path, archive: &impl Serialize) -> Result<()> {
    if metadata(destination)?.is_some() { return Err(Error::Conflict); }
    let bytes = serde_json::to_vec_pretty(archive).map_err(|_| Error::InvalidArchive)?;
    atomic_write(destination, &bytes, None, false)
}

fn digest(bytes: &[u8]) -> String { hex::encode(Sha256::digest(bytes)) }

pub(crate) fn unsafe_meta(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 { return true; }
    }
    meta.file_type().is_symlink()
}

pub(crate) fn validate_ancestors(path: &Path) -> Result<()> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err(Error::UnsafePath);
    }
    for dir in path.ancestors().skip(1) {
        match dir.symlink_metadata() {
            Ok(meta) if unsafe_meta(&meta) || !meta.is_dir() => return Err(Error::UnsafePath),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Error::Io),
        }
    }
    Ok(())
}

pub(crate) fn directory_exists(path: &Path) -> Result<bool> {
    validate_ancestors(path)?;
    match path.symlink_metadata() {
        Ok(metadata) if unsafe_meta(&metadata) || !metadata.is_dir() => Err(Error::UnsafePath),
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(Error::Io),
    }
}

fn metadata(path: &Path) -> Result<Option<fs::Metadata>> {
    validate_ancestors(path)?;
    match path.symlink_metadata() {
        Ok(meta) if unsafe_meta(&meta) || !meta.is_file() => Err(Error::UnsafePath),
        Ok(meta) => Ok(Some(meta)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(Error::Io),
    }
}

pub(crate) fn fingerprint_text_file(path: &Path, limit: u64) -> Result<Option<String>> {
    let state = read_state(path, limit)?;
    let Some(bytes) = state.bytes else { return Ok(None); };
    std::str::from_utf8(&bytes).map_err(|_| Error::InvalidConfig)?;
    Ok(Some(digest(&bytes)))
}

fn read_state(path: &Path, limit: u64) -> Result<FileState> {
    let Some(meta) = metadata(path)? else { return Ok(FileState { bytes: None, permissions: None }); };
    if meta.len() > limit { return Err(Error::FileTooLarge); }
    let file = File::open(path).map_err(|_| Error::Io)?;
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(limit + 1).read_to_end(&mut bytes).map_err(|_| Error::Io)?;
    if bytes.len() as u64 > limit { return Err(Error::FileTooLarge); }
    Ok(FileState { bytes: Some(bytes), permissions: Some(meta.permissions()) })
}

fn stage_file(path: &Path, bytes: &[u8], permissions: Option<&Permissions>) -> Result<tempfile::NamedTempFile> {
    let parent = path.parent().ok_or(Error::UnsafePath)?;
    let mut temp = tempfile::Builder::new().prefix(".whaleswitch-").tempfile_in(parent).map_err(|_| Error::WriteFailed)?;
    temp.write_all(bytes).map_err(|_| Error::WriteFailed)?;
    if let Some(permissions) = permissions {
        temp.as_file().set_permissions(permissions.clone()).map_err(|_| Error::WriteFailed)?;
    }
    temp.as_file().sync_all().map_err(|_| Error::WriteFailed)?;
    Ok(temp)
}

fn persist(temp: tempfile::NamedTempFile, path: &Path, replace: bool) -> Result<()> {
    let result = if replace { temp.persist(path) } else { temp.persist_noclobber(path) };
    result.map(|_| ()).map_err(|err| {
        if err.error.kind() == std::io::ErrorKind::AlreadyExists { Error::Conflict } else { Error::WriteFailed }
    })
}

fn sync_parent(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path.parent().ok_or(Error::UnsafePath)?).and_then(|parent| parent.sync_all()).map_err(|_| Error::WriteFailed)?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8], permissions: Option<&Permissions>, replace: bool) -> Result<()> {
    validate_ancestors(path)?;
    let temp = stage_file(path, bytes, permissions)?;
    persist(temp, path, replace)?;
    sync_parent(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, sync::Mutex};

    #[derive(Default)]
    struct MemoryVault(Mutex<HashMap<String, Zeroizing<String>>>);

    impl SecretStore for MemoryVault {
        fn get(&self, account: &str) -> Result<Option<Zeroizing<String>>> {
            Ok(self.0.lock().unwrap().get(account).cloned())
        }
        fn set(&self, account: &str, secret: &str) -> Result<()> {
            self.0.lock().unwrap().insert(account.into(), Zeroizing::new(secret.into()));
            Ok(())
        }
        fn remove(&self, account: &str) -> Result<()> {
            self.0.lock().unwrap().remove(account);
            Ok(())
        }
    }

    #[test]
    fn a_post_replace_failure_recovers_original_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let path = base.join("config.toml");
        let original = b"\xef\xbb\xbf# original\r\nmodel = 'before'\r\n";
        let update = b"model = 'after'\n";
        fs::write(&path, original).unwrap();
        let engine = Engine::isolated(base.join("data"), [path.clone(), base.join("settings.json"), base.join("opencode.json")], Box::new(MemoryVault::default()));
        let _lock = engine.storage.lock().unwrap();
        let state = read_state(&path, MAX_CONFIG).unwrap();
        let result = engine.commit_with_sync(Tool::Codex, &path, &state, update, "before_save", |file| {
            assert_eq!(fs::read(file).unwrap(), update.as_slice());
            assert_eq!(engine.snapshots(Tool::Codex).unwrap().len(), 1);
            Err(Error::WriteFailed)
        });
        assert_eq!(result, Err(Error::RolledBack));
        assert_eq!(fs::read(path).unwrap(), original.as_slice());
    }

    #[test]
    fn recovery_preserves_a_third_party_write() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let path = base.join("config.toml");
        fs::write(&path, "model = 'before'").unwrap();
        let engine = Engine::isolated(base.join("data"), [path.clone(), base.join("settings.json"), base.join("opencode.json")], Box::new(MemoryVault::default()));
        let _lock = engine.storage.lock().unwrap();
        let state = read_state(&path, MAX_CONFIG).unwrap();
        let result = engine.commit_with_sync(Tool::Codex, &path, &state, b"model = 'after'", "before_save", |file| {
            fs::write(file, "model = 'external'").unwrap();
            Err(Error::WriteFailed)
        });
        assert_eq!(result, Err(Error::RecoveryRequired));
        assert_eq!(fs::read_to_string(path).unwrap(), "model = 'external'");
    }
}
