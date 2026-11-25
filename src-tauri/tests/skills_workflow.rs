use std::{fs, path::{Path, PathBuf}, sync::{Arc, Mutex}};
use tempfile::TempDir;
use whaleswitch_lib::{Engine, Error, Result, SecretStore, SkillChangeKind, SkillLocation};
use zeroize::Zeroizing;

struct NoVault;

#[derive(Clone, Default)]
struct MemoryVault(Arc<Mutex<Option<Zeroizing<String>>>>);

impl SecretStore for MemoryVault {
    fn get(&self, _: &str) -> Result<Option<Zeroizing<String>>> { Ok(self.0.lock().unwrap().clone()) }
    fn set(&self, _: &str, secret: &str) -> Result<()> {
        *self.0.lock().unwrap() = Some(Zeroizing::new(secret.into()));
        Ok(())
    }
    fn remove(&self, _: &str) -> Result<()> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

impl SecretStore for NoVault {
    fn get(&self, _: &str) -> Result<Option<Zeroizing<String>>> { Err(Error::VaultUnavailable) }
    fn set(&self, _: &str, _: &str) -> Result<()> { Err(Error::VaultUnavailable) }
    fn remove(&self, _: &str) -> Result<()> { Err(Error::VaultUnavailable) }
}

fn sandbox() -> (TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    (temp, root)
}

fn engine(root: &Path) -> Engine {
    with_vault(root, NoVault)
}

fn with_vault(root: &Path, vault: impl SecretStore + 'static) -> Engine {
    let files = ["config.toml", "settings.json", "opencode.jsonc"].map(|name| root.join(name));
    Engine::isolated(root.join("data"), files, Box::new(vault))
}

fn write(root: &Path, path: &str, content: impl AsRef<[u8]>) {
    let file = root.join(path);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, content).unwrap();
}

const MANIFEST: &str = "\u{feff}---\r\nname: review-pr\r\ndescription: Review local changes\r\n---\r\nRead references/quick.md.\r\n";
const EXISTING_MANIFEST: &str = "\u{feff}---\r\nname: review-pr\r\ndescription: confidential fixture 67\r\n---\r\nReview previous changes.\r\n";
const DESTINATION: &str = ".agents/skills/review-pr";

fn upgrade_fixture(root: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let project = root.join("workspace");
    let source = root.join("review-pr");
    let target = project.join(DESTINATION);
    write(&source, "SKILL.md", MANIFEST);
    write(&source, "references/quick.md", "new quick reference");
    write(&source, "references/new/extra.txt", "new reference");
    write(&source, "references/unselected.md", "proposed content");
    write(&source, "scripts/task.js", "source script content");
    write(&target, "SKILL.md", EXISTING_MANIFEST);
    write(&target, "references/quick.md", "old quick reference\r\n");
    write(&target, "references/unselected.md", "keep this text");
    write(&target, "scripts/task.js", "keep this script");
    (project, source, target)
}

#[test]
fn upgrade_and_restore_selected_skill_files_from_encrypted_history() {
    let (_temp, root) = sandbox();
    let (project, source, target) = upgrade_fixture(&root);
    let vault = MemoryVault::default();
    let session = with_vault(&root, vault.clone());
    let preview = session.preview_skill_upgrade(&project, &source, SkillLocation::Agents).unwrap();
    assert_eq!(preview.destination, DESTINATION);
    assert_eq!(preview.changes.len(), 4);
    assert_eq!(preview.changes[0].before.as_deref(), Some(EXISTING_MANIFEST));
    assert_eq!(preview.changes[0].after.as_deref(), Some(MANIFEST));
    let selection = ["SKILL.md".into(), "references/quick.md".into(), "references/new/extra.txt".into()];
    let backup = session.apply_skill_upgrade(&project, &source, SkillLocation::Agents, &preview.revision, &selection).unwrap();
    assert_eq!(backup.file_count, 3);
    assert_eq!(backup.action, "upgrade");
    assert_eq!(fs::read(target.join("SKILL.md")).unwrap(), MANIFEST.as_bytes());
    assert_eq!(fs::read(target.join("references/new/extra.txt")).unwrap(), b"new reference");
    assert_eq!(fs::read(target.join("references/unselected.md")).unwrap(), b"keep this text");
    assert_eq!(fs::read(target.join("scripts/task.js")).unwrap(), b"keep this script");
    let db = fs::read(root.join("data/snapshots.sqlite3")).unwrap();
    let marker = b"confidential fixture 67";
    assert!(!db.windows(marker.len()).any(|bytes| bytes == marker));
    drop(session);

    let reopened = with_vault(&root, vault);
    assert_eq!(reopened.skill_backups(&project).unwrap()[0].id, backup.id);
    let other_project = root.join("other");
    fs::create_dir(&other_project).unwrap();
    assert!(reopened.skill_backups(&other_project).unwrap().is_empty());
    assert!(matches!(reopened.preview_skill_restore(&other_project, backup.id), Err(Error::SnapshotMissing)));
    let restore = reopened.preview_skill_restore(&project, backup.id).unwrap();
    assert!(restore.changes.iter().all(|change| !change.blocked));
    assert!(restore.changes.iter().any(|change| change.path == "references/new/extra.txt" && change.after.is_none()));
    let reverse = reopened.apply_skill_restore(&project, backup.id, &restore.revision, &selection).unwrap();
    assert_eq!(reverse.action, "restore");
    assert_eq!(fs::read(target.join("SKILL.md")).unwrap(), EXISTING_MANIFEST.as_bytes());
    assert_eq!(fs::read(target.join("references/quick.md")).unwrap(), b"old quick reference\r\n");
    assert!(!target.join("references/new/extra.txt").exists());
    assert_eq!(reopened.skill_backups(&project).unwrap().len(), 2);

    let redo = reopened.preview_skill_restore(&project, reverse.id).unwrap();
    reopened.apply_skill_restore(&project, reverse.id, &redo.revision, &selection).unwrap();
    assert_eq!(fs::read(target.join("SKILL.md")).unwrap(), MANIFEST.as_bytes());
    assert_eq!(fs::read(target.join("references/new/extra.txt")).unwrap(), b"new reference");
}

#[test]
fn skill_upgrade_refuses_unreviewed_paths_and_a_changed_source_or_destination() {
    let (_temp, root) = sandbox();
    let (project, source, target) = upgrade_fixture(&root);
    let session = with_vault(&root, MemoryVault::default());
    let preview = session.preview_skill_upgrade(&project, &source, SkillLocation::Agents).unwrap();
    for choice in [vec!["../outside.md".into()], vec!["SKILL.md".into(), "SKILL.md".into()]] {
        assert!(matches!(
            session.apply_skill_upgrade(&project, &source, SkillLocation::Agents, &preview.revision, &choice),
            Err(Error::SkillEditSelection)));
    }
    write(&source, "references/quick.md", "revised source");
    assert!(matches!(session.apply_skill_upgrade(&project, &source, SkillLocation::Agents,
        &preview.revision, &["SKILL.md".into()]), Err(Error::Conflict)));
    let preview = session.preview_skill_upgrade(&project, &source, SkillLocation::Agents).unwrap();
    write(&target, "references/quick.md", "external target change");
    assert!(matches!(session.apply_skill_upgrade(&project, &source, SkillLocation::Agents,
        &preview.revision, &["SKILL.md".into()]), Err(Error::Conflict)));
    assert_eq!(fs::read(target.join("SKILL.md")).unwrap(), EXISTING_MANIFEST.as_bytes());
    assert!(session.skill_backups(&project).unwrap().is_empty());
}

#[test]
fn skill_restore_blocks_external_edits_and_recovers_the_other_selected_files() {
    let (_temp, root) = sandbox();
    let (project, source, target) = upgrade_fixture(&root);
    let session = with_vault(&root, MemoryVault::default());
    let preview = session.preview_skill_upgrade(&project, &source, SkillLocation::Agents).unwrap();
    let selection = ["SKILL.md".into(), "references/quick.md".into()];
    let saved = session.apply_skill_upgrade(&project, &source, SkillLocation::Agents, &preview.revision, &selection).unwrap();
    let outdated = session.preview_skill_restore(&project, saved.id).unwrap();
    write(&target, "references/quick.md", "external edit");
    assert!(matches!(session.apply_skill_restore(&project, saved.id, &outdated.revision, &selection), Err(Error::Conflict)));
    let current = session.preview_skill_restore(&project, saved.id).unwrap();
    assert!(current.changes.iter().any(|change| change.path == "references/quick.md" && change.blocked));
    assert!(matches!(session.apply_skill_restore(&project, saved.id, &current.revision, &selection), Err(Error::SkillEditSelection)));
    session.apply_skill_restore(&project, saved.id, &current.revision, &["SKILL.md".into()]).unwrap();
    assert_eq!(fs::read(target.join("SKILL.md")).unwrap(), EXISTING_MANIFEST.as_bytes());
    assert_eq!(fs::read(target.join("references/quick.md")).unwrap(), b"external edit");
    assert_eq!(session.skill_backups(&project).unwrap().len(), 2);
}

#[test]
fn missing_skill_backup_keys_stop_writes_without_resetting_the_shared_key() {
    let (_temp, root) = sandbox();
    let (project, source, target) = upgrade_fixture(&root);
    let session = engine(&root);
    let preview = session.preview_skill_upgrade(&project, &source, SkillLocation::Agents).unwrap();
    assert!(matches!(session.apply_skill_upgrade(&project, &source, SkillLocation::Agents, &preview.revision,
        &["references/new/extra.txt".into()]), Err(Error::VaultUnavailable)));
    assert!(!target.join("references/new").exists());
    assert!(session.skill_backups(&project).unwrap().is_empty());

    let vault = MemoryVault::default();
    let session = with_vault(&root, vault.clone());
    session.apply_skill_upgrade(&project, &source, SkillLocation::Agents, &preview.revision, &["SKILL.md".into()]).unwrap();
    vault.remove("snapshot-encryption-v1").unwrap();
    let next = session.preview_skill_upgrade(&project, &source, SkillLocation::Agents).unwrap();
    assert!(matches!(session.apply_skill_upgrade(&project, &source, SkillLocation::Agents, &next.revision,
        &["references/quick.md".into()]), Err(Error::VaultUnavailable)));
    assert!(vault.get("snapshot-encryption-v1").unwrap().is_none());
    assert_eq!(fs::read(target.join("references/quick.md")).unwrap(), b"old quick reference\r\n");
    assert_eq!(session.skill_backups(&project).unwrap().len(), 1);
}

#[test]
fn saves_a_project_baseline_and_tracks_manifest_drift_after_reopening() {
    let (_temp, root) = sandbox();
    let project = root.join("workspace");
    let first = ".agents/skills/first/SKILL.md";
    let second = ".claude/skills/second/SKILL.md";
    let third = ".opencode/skills/third/SKILL.md";
    write(&project, first, "first revision");
    write(&project, second, "previously present");
    let session = engine(&root);
    let initial = session.project_skills(&project).unwrap();
    assert!(initial.baseline_at.is_none());
    let saved = session.capture_skill_baseline(&project, &initial.revision).unwrap();
    assert!(saved.baseline_at.is_some() && saved.changes.is_empty());
    drop(session);
    assert!(!fs::read(root.join("data/snapshots.sqlite3")).unwrap()
        .windows(project.to_str().unwrap().len()).any(|data| data == project.to_str().unwrap().as_bytes()));

    write(&project, first, "second revision");
    fs::remove_file(project.join(second)).unwrap();
    write(&project, third, "new");
    let reopened = engine(&root);
    let current = reopened.project_skills(&project).unwrap();
    assert_eq!(current.baseline_at, saved.baseline_at);
    assert_eq!(current.changes.len(), 3);
    assert_eq!(current.changes[0].path, first);
    assert!(matches!(current.changes[0].kind, SkillChangeKind::Modified));
    assert!(matches!(current.changes[1].kind, SkillChangeKind::Removed));
    assert!(matches!(current.changes[2].kind, SkillChangeKind::Added));
    assert_ne!(current.changes[0].before, current.changes[0].after);
    assert!(matches!(reopened.capture_skill_baseline(&project, &initial.revision), Err(Error::Conflict)));
    let other = root.join("other-project");
    write(&other, first, "same relative path, different project");
    assert!(reopened.project_skills(&other).unwrap().baseline_at.is_none());

    write(&project, ".claude/skills/invalid/SKILL.md", [0xff]);
    let partial = reopened.project_skills(&project).unwrap();
    assert_eq!(partial.inventory.skipped, 1);
    assert_eq!(partial.changes.len(), 2);
    assert!(partial.changes.iter().all(|row| !matches!(row.kind, SkillChangeKind::Removed)));
    assert!(matches!(reopened.capture_skill_baseline(&project, &partial.revision), Err(Error::SkillBaselineIncomplete)));
}

#[test]
fn only_installs_reviewed_text_and_checks_source_and_destination_again() {
    let (_temp, root) = sandbox();
    let project = root.join("workspace");
    let source = root.join("review-pr");
    fs::create_dir(&project).unwrap();
    write(&source, "SKILL.md", MANIFEST);
    write(&source, "references/quick.md", "Read this guide.");
    write(&source, "references/nested/notes.TXT", "Some notes.");
    write(&source, "scripts/run.sh", "script fixture");
    write(&source, "references/tool.js", "other script fixture");
    write(&source, "assets/theme.png", [0, 1, 2, 3]);
    write(&source, ".env", "fixture value");
    let session = engine(&root);
    let preview = session.preview_skill_import(&project, &source, SkillLocation::Agents).unwrap();
    assert_eq!(preview.destination, ".agents/skills/review-pr");
    assert_eq!(preview.skipped, 4);
    assert_eq!(preview.excluded, ["assets", "references/tool.js", "scripts"]);
    assert_eq!(preview.files[0].text, MANIFEST);
    assert_eq!(preview.files.iter().map(|file| file.path.as_str()).collect::<Vec<_>>(),
        ["SKILL.md", "references/nested/notes.TXT", "references/quick.md"]);
    assert!(!project.join(".agents").exists());
    assert!(matches!(session.apply_skill_import(&project, &source, SkillLocation::Claude, &preview.revision), Err(Error::Conflict)));

    write(&source, "references/quick.md", "Read a new note.");
    assert!(matches!(session.apply_skill_import(&project, &source, SkillLocation::Agents, &preview.revision), Err(Error::Conflict)));
    assert!(!project.join(".agents").exists());
    let current = session.preview_skill_import(&project, &source, SkillLocation::Agents).unwrap();
    session.apply_skill_import(&project, &source, SkillLocation::Agents, &current.revision).unwrap();
    let destination = project.join(&current.destination);
    assert_eq!(fs::read(destination.join("SKILL.md")).unwrap(), MANIFEST.as_bytes());
    assert_eq!(fs::read(destination.join("references/quick.md")).unwrap(), b"Read a new note.");
    assert!(!destination.join("scripts").exists() && !destination.join("references/tool.js").exists());
    assert!(!destination.join(".env").exists());
    assert_eq!(session.project_skills(&project).unwrap().inventory.entries.len(), 1);

    let conflict = session.preview_skill_import(&project, &source, SkillLocation::Agents).unwrap();
    assert!(conflict.existing);
    assert!(matches!(session.apply_skill_import(&project, &source, SkillLocation::Agents, &current.revision), Err(Error::SkillImportExists)));
    assert_eq!(fs::read(destination.join("SKILL.md")).unwrap(), MANIFEST.as_bytes());
}

#[cfg(unix)]
#[test]
fn stops_at_symlinked_targets_and_ignores_symlinked_attachments() {
    use std::os::unix::fs::symlink;
    let (_temp, root) = sandbox();
    let project = root.join("workspace");
    let source = root.join("review-pr");
    let other = root.join("other");
    write(&source, "SKILL.md", MANIFEST);
    write(&other, "private.md", "test data outside the skill");
    fs::create_dir(source.join("references")).unwrap();
    symlink(other.join("private.md"), source.join("references/linked.md")).unwrap();
    fs::create_dir(&project).unwrap();
    symlink(&other, project.join(".agents")).unwrap();
    let session = engine(&root);
    assert!(matches!(session.preview_skill_import(&project, &source, SkillLocation::Agents), Err(Error::UnsafePath)));
    let preview = session.preview_skill_import(&project, &source, SkillLocation::Opencode).unwrap();
    assert_eq!(preview.skipped, 1);
    assert_eq!(preview.files.len(), 1);
    session.apply_skill_import(&project, &source, SkillLocation::Opencode, &preview.revision).unwrap();
    assert!(!project.join(preview.destination).join("references").exists());
    assert_eq!(fs::read(other.join("private.md")).unwrap(), b"test data outside the skill");
}
