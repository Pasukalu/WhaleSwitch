use crate::engine::{directory_exists, fingerprint_text_file, unsafe_meta};
use crate::{Error, Result, Tool};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, io, path::Path};

const MAX_ENTRIES_PER_SOURCE: usize = 256;
pub(crate) const MAX_MANIFEST_BYTES: u64 = 128 * 1024;

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillLocation { Agents, Claude, Opencode }

impl SkillLocation {
    const ALL: [Self; 3] = [Self::Agents, Self::Claude, Self::Opencode];

    pub(crate) fn directory(self) -> &'static str {
        match self {
            Self::Agents => ".agents/skills",
            Self::Claude => ".claude/skills",
            Self::Opencode => ".opencode/skills",
        }
    }

    fn tools(self) -> &'static [Tool] {
        match self {
            Self::Agents => &[Tool::Codex, Tool::Opencode],
            Self::Claude => &[Tool::Claude, Tool::Opencode],
            Self::Opencode => &[Tool::Opencode],
        }
    }
}

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

impl SkillInventory {
    pub(crate) fn complete(&self) -> bool { !self.limited && self.skipped == 0 }

    pub(crate) fn fingerprints(&self) -> BTreeMap<String, String> {
        self.entries.iter().map(|entry| (entry.path.clone(), entry.fingerprint.clone())).collect()
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillChangeKind { Added, Modified, Removed }

#[derive(Serialize)]
pub struct SkillChange {
    pub path: String,
    pub kind: SkillChangeKind,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Serialize)]
pub struct ProjectSkills {
    pub inventory: SkillInventory,
    pub revision: String,
    pub baseline_at: Option<u64>,
    pub changes: Vec<SkillChange>,
}

pub(crate) type SkillFingerprints = BTreeMap<String, String>;

pub(crate) fn project_changes(before: &SkillFingerprints, after: &SkillFingerprints, complete: bool) -> Vec<SkillChange> {
    let mut changes: Vec<_> = after.iter().filter_map(|(path, fingerprint)| {
        let previous = before.get(path);
        (previous != Some(fingerprint)).then(|| SkillChange {
            path: path.clone(),
            kind: if previous.is_some() { SkillChangeKind::Modified } else { SkillChangeKind::Added },
            before: previous.cloned(),
            after: Some(fingerprint.clone()),
        })
    }).collect();
    if complete {
        changes.extend(before.iter().filter(|(path, _)| !after.contains_key(*path)).map(|(path, fingerprint)| {
            SkillChange { path: path.clone(), kind: SkillChangeKind::Removed, before: Some(fingerprint.clone()), after: None }
        }));
    }
    changes.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    changes
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

    for source in SkillLocation::ALL {
        let location = source.directory();
        let directory = root.join(location);
        let children = match directory_exists(&directory) {
            Ok(false) => continue,
            Ok(true) => fs::read_dir(&directory).map_err(|_| Error::Io),
            Err(error) => Err(error),
        };
        match children {
            Ok(children) => scan_location(children, location, source.tools(), &mut inventory),
            Err(_) => inventory.skipped += 1,
        }
    }

    inventory.entries.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    let mut matches = BTreeMap::new();
    for entry in &inventory.entries {
        for tool in entry.tools {
            *matches.entry((entry.name.to_ascii_lowercase(), tool.id())).or_insert(0usize) += 1;
        }
    }
    for entry in &mut inventory.entries {
        let key = entry.name.to_ascii_lowercase();
        entry.same_name = entry.tools.iter().any(|tool| matches[&(key.clone(), tool.id())] > 1);
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
