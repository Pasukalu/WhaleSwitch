use crate::{profiles::validate_name, storage::Storage, Change, Error, Fields, Result, Tool};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanEntry {
    pub tool: Tool,
    pub fields: Fields,
}

#[derive(Serialize)]
pub struct ConfigPlan {
    pub id: i64,
    pub name: String,
    pub timestamp: u64,
    pub entries: Vec<PlanEntry>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanArchive {
    pub version: u16,
    pub name: String,
    pub entries: Vec<PlanEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanProfile {
    pub tool: Tool,
    pub profile_id: i64,
}

#[derive(Serialize)]
pub struct PlanPreview {
    pub tool: Tool,
    pub path: Option<String>,
    pub revision: Option<String>,
    pub changes: Vec<Change>,
    pub issue: Option<Error>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanSelection {
    pub tool: Tool,
    pub path: String,
    pub revision: String,
    pub fields: Fields,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanFileStatus {
    Pending,
    Applied,
    Restored,
    Unchanged,
    Failed,
    Skipped,
    RecoveryRequired,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct PlanFileResult {
    pub tool: Tool,
    pub snapshot_id: Option<i64>,
    pub status: PlanFileStatus,
    pub issue: Option<Error>,
}

#[derive(Serialize)]
pub struct PlanRun {
    pub id: i64,
    pub name: String,
    pub timestamp: u64,
    pub finished: bool,
    pub files: Vec<PlanFileResult>,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct PlanJournalEntry {
    pub file: PlanFileResult,
    pub path_digest: String,
    pub before: String,
    pub after: String,
}

pub(crate) struct PlanJournal {
    pub id: i64,
    pub name: String,
    pub timestamp: u64,
    pub finished: bool,
    pub entries: Vec<PlanJournalEntry>,
}

impl PlanJournal {
    pub(crate) fn report(self) -> PlanRun {
        PlanRun { id: self.id, name: self.name, timestamp: self.timestamp, finished: self.finished,
            files: self.entries.into_iter().map(|entry| entry.file).collect() }
    }
}

impl PlanArchive {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.version != 1 { return Err(Error::InvalidArchive); }
        validate_plan_name(&self.name).map_err(|_| Error::InvalidArchive)?;
        validate_entries(&self.entries).map_err(|_| Error::InvalidArchive)
    }
}

fn validate_plan_name(name: &str) -> Result<&str> {
    validate_name(name).map_err(|_| Error::InvalidPlan)
}

fn validate_entries(entries: &[PlanEntry]) -> Result<()> {
    if entries.is_empty() || entries.len() > Tool::ALL.len() { return Err(Error::InvalidPlan); }
    for (index, entry) in entries.iter().enumerate() {
        if entries[..index].iter().any(|existing| existing.tool == entry.tool) {
            return Err(Error::InvalidPlan);
        }
        entry.tool.validate(&entry.fields)?;
    }
    Ok(())
}

fn timestamp() -> Result<u64> {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64).map_err(|_| Error::PlanStorage)
}

impl Storage {
    pub(crate) fn create_plan(&self, name: &str, entries: Vec<PlanEntry>) -> Result<ConfigPlan> {
        let name = validate_plan_name(name)?;
        validate_entries(&entries)?;
        let timestamp = timestamp()?;
        let encoded = serde_json::to_string(&entries).map_err(|_| Error::PlanStorage)?;
        let db = self.db().map_err(|_| Error::PlanStorage)?;
        let inserted = db.execute(
            "INSERT INTO config_plans(name,timestamp,entries) VALUES (?1,?2,?3) ON CONFLICT(name) DO NOTHING",
            params![name, timestamp, encoded],
        ).map_err(|_| Error::PlanStorage)?;
        if inserted == 0 { return Err(Error::PlanNameExists); }
        Ok(ConfigPlan { id: db.last_insert_rowid(), name: name.to_owned(), timestamp, entries })
    }

    pub(crate) fn plans(&self) -> Result<Vec<ConfigPlan>> {
        if !self.exists() { return Ok(Vec::new()); }
        let db = self.db().map_err(|_| Error::PlanStorage)?;
        let mut statement = db.prepare("SELECT id,name,timestamp,entries FROM config_plans ORDER BY id DESC")
            .map_err(|_| Error::PlanStorage)?;
        let rows = statement.query_map([], plan_row).map_err(|_| Error::PlanStorage)?;
        rows.map(|row| checked_plan(row.map_err(|_| Error::PlanStorage)?)).collect()
    }

    pub(crate) fn plan(&self, id: i64) -> Result<ConfigPlan> {
        if !self.exists() { return Err(Error::PlanMissing); }
        let db = self.db().map_err(|_| Error::PlanStorage)?;
        let row = db.query_row("SELECT id,name,timestamp,entries FROM config_plans WHERE id=?1", [id], plan_row)
            .optional().map_err(|_| Error::PlanStorage)?.ok_or(Error::PlanMissing)?;
        checked_plan(row)
    }

    pub(crate) fn delete_plan(&self, id: i64) -> Result<()> {
        if !self.exists() { return Err(Error::PlanMissing); }
        let db = self.db().map_err(|_| Error::PlanStorage)?;
        let count = db.execute("DELETE FROM config_plans WHERE id=?1", [id]).map_err(|_| Error::PlanStorage)?;
        if count == 0 { return Err(Error::PlanMissing); }
        Ok(())
    }

    pub(crate) fn begin_plan_run(&self, name: String, entries: Vec<PlanJournalEntry>) -> Result<PlanJournal> {
        let timestamp = timestamp()?;
        let encoded = serde_json::to_string(&entries).map_err(|_| Error::PlanStorage)?;
        let db = self.db().map_err(|_| Error::PlanStorage)?;
        db.execute("INSERT INTO plan_runs(name,timestamp,finished,entries) VALUES(?1,?2,0,?3)",
            params![name, timestamp, encoded]).map_err(|_| Error::PlanStorage)?;
        Ok(PlanJournal { id: db.last_insert_rowid(), name, timestamp, finished: false, entries })
    }

    pub(crate) fn finish_plan_run(&self, journal: &PlanJournal) -> Result<()> {
        let encoded = serde_json::to_string(&journal.entries).map_err(|_| Error::PlanStorage)?;
        let db = self.db().map_err(|_| Error::PlanStorage)?;
        let count = db.execute("UPDATE plan_runs SET finished=1,entries=?1 WHERE id=?2",
            params![encoded, journal.id]).map_err(|_| Error::PlanStorage)?;
        if count != 1 { return Err(Error::PlanStorage); }
        Ok(())
    }

    pub(crate) fn plan_runs(&self) -> Result<Vec<PlanJournal>> {
        if !self.exists() { return Ok(Vec::new()); }
        let db = self.db().map_err(|_| Error::PlanStorage)?;
        let mut statement = db.prepare(
            "SELECT id,name,timestamp,finished,entries FROM plan_runs ORDER BY finished,id DESC LIMIT 20",
        ).map_err(|_| Error::PlanStorage)?;
        let rows = statement.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get::<_, String>(4)?))
        }).map_err(|_| Error::PlanStorage)?;
        rows.map(|row| {
            let (id, name, timestamp, finished, encoded) = row.map_err(|_| Error::PlanStorage)?;
            let entries = serde_json::from_str(&encoded).map_err(|_| Error::PlanStorage)?;
            Ok(PlanJournal { id, name, timestamp, finished, entries })
        }).collect()
    }
}

type PlanRow = (i64, String, u64, String);

fn plan_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PlanRow> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
}

fn checked_plan((id, name, timestamp, encoded): PlanRow) -> Result<ConfigPlan> {
    let entries: Vec<PlanEntry> = serde_json::from_str(&encoded).map_err(|_| Error::PlanStorage)?;
    validate_plan_name(&name).map_err(|_| Error::PlanStorage)?;
    validate_entries(&entries).map_err(|_| Error::PlanStorage)?;
    Ok(ConfigPlan { id, name, timestamp, entries })
}
