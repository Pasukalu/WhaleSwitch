use super::{digest, Engine};
use crate::{
    scan_project_skills,
    skills::{project_changes, SkillFingerprints},
    Error, ProjectSkills, Result, SkillInventory,
};
use rusqlite::{params, OptionalExtension};
use std::{path::Path, time::{SystemTime, UNIX_EPOCH}};

impl Engine {
    pub fn project_skills(&self, root: &Path) -> Result<ProjectSkills> {
        let inventory = scan_project_skills(root)?;
        let fingerprints = inventory.fingerprints();
        let encoded = encode(&fingerprints)?;
        let baseline = if self.storage.exists() {
            let key = project_key(root)?;
            let db = self.storage.db().map_err(|_| Error::SkillStorage)?;
            db.query_row("SELECT timestamp,fingerprints FROM skill_baselines WHERE project=?1", [key],
                |row| Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?)))
                .optional().map_err(|_| Error::SkillStorage)?
        } else {
            None
        };
        let changes = match &baseline {
            Some((_, stored)) => {
                let before = serde_json::from_str(stored).map_err(|_| Error::SkillStorage)?;
                project_changes(&before, &fingerprints, inventory.complete())
            }
            None => Vec::new(),
        };
        Ok(ProjectSkills { inventory, revision: digest(encoded.as_bytes()), baseline_at: baseline.map(|(at, _)| at), changes })
    }

    pub fn capture_skill_baseline(&self, root: &Path, revision: &str) -> Result<ProjectSkills> {
        let _lock = self.storage.lock()?;
        let inventory = scan_project_skills(root)?;
        if !inventory.complete() { return Err(Error::SkillBaselineIncomplete); }
        let encoded = encode(&inventory.fingerprints())?;
        if digest(encoded.as_bytes()) != revision { return Err(Error::Conflict); }
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| Error::SkillStorage)?.as_millis() as u64;
        self.storage.db().map_err(|_| Error::SkillStorage)?.execute(
            "INSERT INTO skill_baselines(project,timestamp,fingerprints) VALUES(?1,?2,?3)
             ON CONFLICT(project) DO UPDATE SET timestamp=excluded.timestamp,fingerprints=excluded.fingerprints",
            params![project_key(root)?, timestamp, encoded],
        ).map_err(|_| Error::SkillStorage)?;
        Ok(ProjectSkills { inventory, revision: revision.to_owned(), baseline_at: Some(timestamp), changes: Vec::new() })
    }
}

fn encode(fingerprints: &SkillFingerprints) -> Result<String> {
    serde_json::to_string(fingerprints).map_err(|_| Error::SkillStorage)
}

fn project_key(root: &Path) -> Result<String> {
    let canonical = root.canonicalize().map_err(|_| Error::Io)?;
    let label = canonical.to_str().ok_or(Error::UnsafePath)?;
    Ok(digest(label.as_bytes()))
}
