use crate::engine::{directory_exists, fingerprint_text_file, unsafe_meta};
use crate::{Error, Result, Tool};
use serde::Serialize;
use std::{fs, io, path::Path};

const MAX_ENTRIES_PER_SOURCE: usize = 256;
const MAX_MANIFEST_BYTES: u64 = 128 * 1024;
const LOCATIONS: [(&str, &[Tool]); 3] = [
    (".agents/skills", &[Tool::Codex, Tool::Opencode]),
    (".claude/skills", &[Tool::Claude, Tool::Opencode]),
    (".opencode/skills", &[Tool::Opencode]),
];

#[derive(Serialize)]
pub struct SkillEntry {
    pub name: String,
    pub path: String,
    pub tools: &'static [Tool],
    pub fingerprint: String,
    pub same_name: bool,
}

#[derive(Serialize)]
pub struct SkillInventory {
    pub root: String,
    pub entries: Vec<SkillEntry>,
    pub skipped: usize,
    pub limited: bool,
}

/// Inspects the documented project-root directories for folder-based skill manifests.
pub fn scan_project_skills(root: &Path) -> Result<SkillInventory> {
    if !directory_exists(root)? {
        return Err(Error::PathUnavailable);
    }
    let mut inventory = SkillInventory {
        root: root.to_string_lossy().into_owned(),
        entries: Vec::new(),
        skipped: 0,
        limited: false,
    };

    for (location, tools) in LOCATIONS {
        let directory = root.join(location);
        let children = match directory_exists(&directory) {
            Ok(false) => continue,
            Ok(true) => fs::read_dir(&directory).map_err(|_| Error::Io),
            Err(error) => Err(error),
        };
        match children {
            Ok(children) => scan_location(children, location, tools, &mut inventory),
            Err(_) => inventory.skipped += 1,
        }
    }

    inventory.entries.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    for index in 0..inventory.entries.len() {
        let entry = &inventory.entries[index];
        let same_name = inventory.entries.iter().enumerate().any(|(other_index, other)| {
            index != other_index && entry.name.eq_ignore_ascii_case(&other.name)
                && entry.tools.iter().any(|tool| other.tools.contains(tool))
        });
        inventory.entries[index].same_name = same_name;
    }
    Ok(inventory)
}

fn scan_location(
    children: fs::ReadDir,
    location: &str,
    tools: &'static [Tool],
    inventory: &mut SkillInventory,
) {
    for (index, child) in children.enumerate() {
        if index == MAX_ENTRIES_PER_SOURCE {
            inventory.limited = true;
            break;
        }
        match read_entry(child, location, tools) {
            Ok(Some(entry)) => inventory.entries.push(entry),
            Ok(None) => {},
            Err(_) => inventory.skipped += 1,
        }
    }
}

fn read_entry(child: io::Result<fs::DirEntry>, location: &str, tools: &'static [Tool]) -> Result<Option<SkillEntry>> {
    let child = child.map_err(|_| Error::Io)?;
    let metadata = child.path().symlink_metadata().map_err(|_| Error::Io)?;
    if unsafe_meta(&metadata) { return Err(Error::UnsafePath); }
    if !metadata.is_dir() { return Ok(None); }
    let name = child.file_name().into_string().map_err(|_| Error::UnsafePath)?;
    if name.chars().any(|ch| ch.is_control() || matches!(ch, '\\' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')) {
        return Err(Error::UnsafePath);
    }
    let Some(fingerprint) = fingerprint_text_file(&child.path().join("SKILL.md"), MAX_MANIFEST_BYTES)? else {
        return Ok(None);
    };
    Ok(Some(SkillEntry { path: format!("{location}/{name}/SKILL.md"), name, tools, fingerprint, same_name: false }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(root: &Path, name: &str, bytes: impl AsRef<[u8]>) {
        let file = root.join(name);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, bytes).unwrap();
    }

    #[test]
    fn finds_only_root_manifests_and_marks_overlapping_names() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        manifest(&root, ".agents/skills/review/SKILL.md", "same bytes");
        manifest(&root, ".claude/skills/Review/SKILL.md", "same bytes");
        manifest(&root, ".opencode/skills/review/SKILL.md", "different bytes");
        manifest(&root, ".agents/skills/review/scripts/run.sh", "echo ignored");
        manifest(&root, ".agents/skills/flat.md", "ignored");
        manifest(&root, ".agents/skills/group/nested/SKILL.md", "ignored");
        manifest(&root, "child/.agents/skills/hidden/SKILL.md", "ignored");
        let result = scan_project_skills(&root).unwrap();
        let entries = &result.entries;
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].tools, &[Tool::Codex, Tool::Opencode]);
        assert_eq!(entries[1].tools, &[Tool::Claude, Tool::Opencode]);
        assert_eq!(entries[2].tools, &[Tool::Opencode]);
        assert!(entries.iter().all(|entry| entry.same_name));
        assert_eq!(entries[0].fingerprint.len(), 64);
        assert_eq!(entries[0].fingerprint, entries[1].fingerprint);
        assert_ne!(entries[0].fingerprint, entries[2].fingerprint);
        assert_eq!(result.skipped, 0);
        assert!(!result.limited);
    }

    #[test]
    fn caps_io_and_reports_invalid_manifests() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let empty = scan_project_skills(&root).unwrap();
        assert!(empty.entries.is_empty());
        assert_eq!(empty.skipped, 0);
        for index in 0..=MAX_ENTRIES_PER_SOURCE {
            manifest(&root, &format!(".agents/skills/skill-{index}/SKILL.md"), "manifest");
        }
        manifest(&root, ".claude/skills/oversize/SKILL.md", vec![b'x'; MAX_MANIFEST_BYTES as usize + 1]);
        manifest(&root, ".claude/skills/binary/SKILL.md", [0xff]);
        let result = scan_project_skills(&root).unwrap();
        assert_eq!(result.entries.len(), MAX_ENTRIES_PER_SOURCE);
        assert!(result.entries.iter().all(|entry| !entry.same_name));
        assert_eq!(result.skipped, 2);
        assert!(result.limited);
    }

    #[cfg(unix)]
    #[test]
    fn skips_links_in_sources_entries_and_manifests() {
        use std::os::unix::fs::symlink;
        let project = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let root = project.path().canonicalize().unwrap();
        let other = other.path().canonicalize().unwrap();
        manifest(&other, "outside/SKILL.md", "outside");
        fs::create_dir_all(root.join(".agents/skills/linked-manifest")).unwrap();
        fs::create_dir(root.join(".claude")).unwrap();
        symlink(&other, root.join(".claude/skills")).unwrap();
        symlink(other.join("outside"), root.join(".agents/skills/linked-folder")).unwrap();
        symlink(other.join("outside/SKILL.md"), root.join(".agents/skills/linked-manifest/SKILL.md")).unwrap();
        let result = scan_project_skills(&root).unwrap();
        assert!(result.entries.is_empty());
        assert_eq!(result.skipped, 3);
        assert!(matches!(scan_project_skills(&root.join(".claude/skills")), Err(Error::UnsafePath)));
    }
}
