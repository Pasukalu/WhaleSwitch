use super::{digest, directory_exists, fingerprint_text_file, persist, read_state, stage_file, sync_parent, unsafe_meta,
    validate_ancestors, Engine};
use crate::{skills::MAX_MANIFEST_BYTES, Error, Result, SkillLocation};
use serde::Serialize;
use std::{collections::BTreeSet, fs, io, path::{Path, PathBuf}};
use zeroize::Zeroizing;

mod edits;
pub use edits::{SkillEditPreview, SkillTextChange};

const MAX_NODES: usize = 96;
const MAX_REFERENCE_DEPTH: usize = 3;
const MAX_BYTES: usize = 512 * 1024;

#[derive(Serialize)]
pub struct SkillImportFile {
    pub path: String,
    pub fingerprint: String,
    pub bytes: usize,
    pub text: String,
}

#[derive(Serialize)]
pub struct SkillImportPreview {
    pub source: String,
    pub destination: String,
    pub name: String,
    pub revision: String,
    pub existing: bool,
    pub skipped: usize,
    pub excluded: Vec<String>,
    pub files: Vec<SkillImportFile>,
}

struct ImportDocument {
    path: String,
    fingerprint: String,
    bytes: Zeroizing<Vec<u8>>,
}

struct ImportDraft {
    name: String,
    destination: String,
    revision: String,
    existing: bool,
    ignored: usize,
    excluded: Vec<String>,
    documents: Vec<ImportDocument>,
}

impl Engine {
    pub fn preview_skill_import(&self, root: &Path, source: &Path, location: SkillLocation) -> Result<SkillImportPreview> {
        let draft = import_draft(root, source, location)?;
        let files = draft.documents.iter().rev().map(|document| SkillImportFile {
            path: document.path.clone(),
            fingerprint: document.fingerprint.clone(),
            bytes: document.bytes.len(),
            text: String::from_utf8_lossy(&document.bytes).into_owned(),
        }).collect();
        Ok(SkillImportPreview {
            source: label(source)?.to_owned(), destination: draft.destination, name: draft.name,
            revision: draft.revision, existing: draft.existing, skipped: draft.ignored, excluded: draft.excluded, files,
        })
    }

    pub fn apply_skill_import(&self, root: &Path, source: &Path, location: SkillLocation, revision: &str) -> Result<()> {
        let _lock = self.storage.lock()?;
        let draft = import_draft(root, source, location)?;
        if draft.existing { return Err(Error::SkillImportExists); }
        if draft.revision != revision { return Err(Error::Conflict); }
        install(root, &draft, sync_parent)
    }
}

fn label(path: &Path) -> Result<&str> { path.to_str().ok_or(Error::UnsafePath) }

fn import_draft(root: &Path, source: &Path, location: SkillLocation) -> Result<ImportDraft> {
    if !directory_exists(root)? || !directory_exists(source)? { return Err(Error::PathUnavailable); }
    let name = source.file_name().and_then(|name| name.to_str()).ok_or(Error::SkillImportName)?;
    if !skill_name(name) { return Err(Error::SkillImportName); }
    let destination = format!("{}/{name}", location.directory());
    let target = root.join(&destination);
    validate_ancestors(&target)?;
    let existing = match target.symlink_metadata() {
        Ok(_) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(_) => return Err(Error::Io),
    };
    let manifest = read_document(source, "SKILL.md")?;
    let mut scan = References {
        count: 0, ignored: 0, bytes: manifest.bytes.len(), has_manifest: false, documents: Vec::new(), excluded: Vec::new(),
    };
    scan.read_dir(source, "", 0)?;
    if !scan.has_manifest { return Err(Error::SkillImportManifest); }
    scan.documents.sort_unstable_by(|a, b| b.path.cmp(&a.path));
    // Other tools can discover this directory once the manifest exists.
    scan.documents.push(manifest);
    let fingerprints: Vec<_> = scan.documents.iter().map(|file| (&file.path, &file.fingerprint)).collect();
    scan.excluded.sort_unstable();
    let binding = serde_json::to_vec(&(label(root)?, label(source)?, &destination, scan.ignored, &scan.excluded, fingerprints))
        .map_err(|_| Error::SkillImportManifest)?;
    Ok(ImportDraft { name: name.to_owned(), destination, revision: digest(&binding), existing,
        ignored: scan.ignored, excluded: scan.excluded, documents: scan.documents })
}

struct References {
    count: usize,
    ignored: usize,
    bytes: usize,
    has_manifest: bool,
    documents: Vec<ImportDocument>,
    excluded: Vec<String>,
}

impl References {
    fn read_dir(&mut self, source: &Path, relative: &str, depth: usize) -> Result<()> {
        let directory = source.join(relative);
        if !directory_exists(&directory)? { return Err(Error::PathUnavailable); }
        let mut names = BTreeSet::new();
        for child in fs::read_dir(directory).map_err(|_| Error::Io)? {
            self.count += 1;
            if self.count > MAX_NODES { return Err(Error::SkillImportLimit); }
            let child = child.map_err(|_| Error::Io)?;
            let filename = child.file_name();
            let Some(name) = filename.to_str().filter(|name| portable_component(name)) else {
                self.ignored += 1;
                continue;
            };
            if relative.is_empty() && name == "SKILL.md" {
                self.has_manifest = true;
                continue;
            }
            if relative.is_empty() && name != "references" {
                self.ignored += 1;
                self.excluded.push(name.to_owned());
                continue;
            }
            let info = child.path().symlink_metadata().map_err(|_| Error::Io)?;
            let path = if relative.is_empty() { name.to_owned() } else { format!("{relative}/{name}") };
            let is_directory = info.is_dir() && depth < MAX_REFERENCE_DEPTH;
            let is_text = info.is_file() && depth > 0 && text_extension(name);
            if unsafe_meta(&info) || !(is_directory || is_text) {
                self.ignored += 1;
                self.excluded.push(path);
                continue;
            }
            if !names.insert(name.to_ascii_lowercase()) { return Err(Error::SkillImportName); }
            if is_directory {
                self.read_dir(source, &path, depth + 1)?;
            } else {
                let document = read_document(source, &path)?;
                self.bytes += document.bytes.len();
                if self.bytes > MAX_BYTES { return Err(Error::SkillImportLimit); }
                self.documents.push(document);
            }
        }
        Ok(())
    }
}

fn read_document(source: &Path, path: &str) -> Result<ImportDocument> {
    let file = source.join(path);
    let bytes = read_state(&file, MAX_MANIFEST_BYTES)?.bytes.ok_or(Error::SkillImportManifest)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| Error::SkillImportManifest)?;
    if text.contains('\0') || (path == "SKILL.md" && !has_frontmatter(text)) {
        return Err(Error::SkillImportManifest);
    }
    let fingerprint = digest(&bytes);
    Ok(ImportDocument { path: path.into(), fingerprint, bytes })
}

fn has_frontmatter(text: &str) -> bool {
    let mut lines = text.trim_start_matches('\u{feff}').lines().map(str::trim_end);
    if lines.next() != Some("---") { return false; }
    let mut metadata = false;
    for line in lines {
        if line == "---" { return metadata; }
        metadata |= !line.is_empty() && !line.trim_start().starts_with('#');
    }
    false
}

fn text_extension(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("txt"))
}

fn skill_name(name: &str) -> bool {
    portable_component(name) && name.bytes().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == b'-')
        && !name.ends_with('-') && !name.contains("--")
}

fn portable_component(name: &str) -> bool {
    if name.len() > 64 || !name.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        || name.ends_with('.') || !name.bytes().all(|ch| ch.is_ascii_alphanumeric() || b"-_.".contains(&ch)) {
        return false;
    }
    let stem = name.split('.').next().unwrap_or_default().to_ascii_uppercase();
    !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !stem.strip_prefix("COM").or_else(|| stem.strip_prefix("LPT")).is_some_and(|number| {
            number.len() == 1 && number.as_bytes()[0].is_ascii_digit()
        })
}

fn install(root: &Path, draft: &ImportDraft, mut sync: impl FnMut(&Path) -> Result<()>) -> Result<()> {
    let destination = root.join(&draft.destination);
    let mut directories = Vec::new();
    let mut files = Vec::new();
    let result = (|| {
        let parent = destination.parent().ok_or(Error::UnsafePath)?;
        ensure_directory(root, parent, &mut directories)?;
        validate_ancestors(&destination)?;
        fs::create_dir(&destination).map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists { Error::SkillImportExists } else { Error::WriteFailed }
        })?;
        directories.push(destination.clone());
        for doc in &draft.documents {
            if doc.path == "SKILL.md" { verify_files(&files)?; }
            let target = destination.join(&doc.path);
            ensure_directory(&destination, target.parent().ok_or(Error::UnsafePath)?, &mut directories)?;
            let staged = stage_file(&target, &doc.bytes, None)?;
            validate_ancestors(&target)?;
            persist(staged, &target, false)?;
            files.push((target, doc));
            sync(&files.last().ok_or(Error::WriteFailed)?.0)?;
        }
        verify_files(&files)?;
        // Persist the directory entries as well as the contents.
        for path in directories.iter().rev() { sync_parent(path)?; }
        Ok(())
    })();
    if let Err(error) = result {
        let restored = rollback(&files, &directories);
        return Err(if !restored { Error::SkillImportCleanup } else if files.is_empty() { error } else { Error::SkillImportRolledBack });
    }
    result
}

fn verify_files(files: &[(PathBuf, &ImportDocument)]) -> Result<()> {
    for (target, document) in files {
        if fingerprint_text_file(target, MAX_MANIFEST_BYTES)?.as_deref() != Some(document.fingerprint.as_str()) {
            return Err(Error::Conflict);
        }
    }
    Ok(())
}

fn ensure_directory(root: &Path, path: &Path, created: &mut Vec<PathBuf>) -> Result<()> {
    let relative = path.strip_prefix(root).map_err(|_| Error::UnsafePath)?;
    let mut current = root.to_owned();
    if !directory_exists(root)? { return Err(Error::PathUnavailable); }
    for component in relative.components() {
        current.push(component);
        if directory_exists(&current)? { continue; }
        match fs::create_dir(&current) {
            Ok(()) => created.push(current.clone()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists && directory_exists(&current)? => {}
            Err(_) => return Err(Error::WriteFailed),
        }
    }
    Ok(())
}

fn rollback(files: &[(PathBuf, &ImportDocument)], directories: &[PathBuf]) -> bool {
    let mut clean = true;
    for (path, doc) in files.iter().rev() {
        match fingerprint_text_file(path, MAX_MANIFEST_BYTES) {
            Ok(None) => {}
            Ok(Some(current)) if current == doc.fingerprint => {
                clean &= fs::remove_file(path).is_ok() && sync_parent(path).is_ok();
            }
            _ => clean = false,
        }
    }
    remove_empty_directories(directories) && clean
}

fn remove_empty_directories(directories: &[PathBuf]) -> bool {
    let mut clean = true;
    for directory in directories.iter().rev() {
        match directory_exists(directory) {
            Ok(false) => {}
            Ok(true) => match fs::remove_dir(directory) {
                Ok(()) => clean &= sync_parent(directory).is_ok(),
                Err(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => {}
                Err(_) => clean = false,
            },
            Err(_) => clean = false,
        }
    }
    clean
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, PathBuf, ImportDraft) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let source = root.join("review-pr");
        fs::create_dir_all(source.join("references")).unwrap();
        fs::write(source.join("SKILL.md"), "---\nname: review-pr\ndescription: Review changes\n---\n").unwrap();
        fs::write(source.join("references/guide.md"), "Original reference").unwrap();
        let project = root.join("workspace");
        fs::create_dir_all(project.join(".agents/skills")).unwrap();
        fs::write(project.join(".agents/skills/existing.txt"), "Other content").unwrap();
        let draft = import_draft(&project, &source, SkillLocation::Agents).unwrap();
        (temp, project, draft)
    }

    #[test]
    fn publishes_the_manifest_after_references_and_reverts_completed_writes() {
        let (_temp, project, draft) = fixture();
        let destination = project.join(&draft.destination);
        let result = install(&project, &draft, |file| {
            if file.ends_with("guide.md") {
                assert!(!destination.join("SKILL.md").exists());
                Ok(())
            } else {
                assert!(destination.join("references/guide.md").exists());
                Err(Error::WriteFailed)
            }
        });
        assert!(matches!(result, Err(Error::SkillImportRolledBack)));
        assert!(!destination.exists());
        assert_eq!(fs::read(project.join(".agents/skills/existing.txt")).unwrap(), b"Other content");
    }

    #[test]
    fn preserves_an_external_edit_on_failed_import() {
        let (_temp, project, draft) = fixture();
        let destination = project.join(&draft.destination);
        let result = install(&project, &draft, |file| {
            fs::write(file, "External bytes").unwrap();
            Err(Error::WriteFailed)
        });
        assert!(matches!(result, Err(Error::SkillImportCleanup)));
        assert_eq!(fs::read(destination.join("references/guide.md")).unwrap(), b"External bytes");
        assert!(!destination.join("SKILL.md").exists());
    }

    #[test]
    fn rejects_nonportable_names_and_requires_a_frontmatter_block() {
        for name in ["../review", "test\\extra", "COM1", "prn.txt", "hello:there", "trailing."] {
            assert!(!portable_component(name), "{name}");
        }
        assert!(skill_name("review-pr"));
        assert!(!skill_name("review--pr"));
        assert!(!has_frontmatter("# No header"));
        assert!(!has_frontmatter("---\n# no metadata\n---\n"));
        assert!(has_frontmatter("\u{feff}---\r\nname: review\r\n---\r\n# Review"));
    }
}
