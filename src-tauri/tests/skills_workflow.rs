use std::{fs, path::{Path, PathBuf}};
use tempfile::TempDir;
use whaleswitch_lib::{Engine, Error, Result, SecretStore, SkillChangeKind, SkillLocation};
use zeroize::Zeroizing;

struct NoVault;

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
    let files = ["config.toml", "settings.json", "opencode.jsonc"].map(|name| root.join(name));
    Engine::isolated(root.join("data"), files, Box::new(NoVault))
}

fn write(root: &Path, path: &str, content: impl AsRef<[u8]>) {
    let file = root.join(path);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, content).unwrap();
}

const MANIFEST: &str = "\u{feff}---\r\nname: review-pr\r\ndescription: Review local changes\r\n---\r\nRead references/quick.md.\r\n";

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
