use super::Storage;
use crate::{Error, Result, Tool};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const ACTIVITY_LIMIT: usize = 100;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityAction {
    Save,
    Snapshot,
    RestoreFile,
    RestoreFields,
    Cleanup,
    Plan,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityStatus {
    Unconfirmed,
    Succeeded,
    Failed,
    RolledBack,
    RecoveryRequired,
    Unchanged,
    Skipped,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigActivity {
    pub timestamp: u64,
    pub tool: Tool,
    pub action: ActivityAction,
    pub status: ActivityStatus,
    pub snapshot_id: Option<i64>,
}

impl ConfigActivity {
    pub(crate) fn new(tool: Tool, action: ActivityAction) -> Result<Self> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|_| Error::ActivityStorage)?.as_millis() as u64;
        Ok(Self { timestamp, tool, action, status: ActivityStatus::Unconfirmed, snapshot_id: None })
    }
}

impl Storage {
    pub(crate) fn begin_activity(&self, event: &ConfigActivity) -> Result<i64> {
        let db = self.db().map_err(|_| Error::ActivityStorage)?;
        let json = serde_json::to_string(event).map_err(|_| Error::ActivityStorage)?;
        db.execute("INSERT INTO config_activity(tool,event) VALUES(?1,?2)", params![event.tool.id(), json])
            .map_err(|_| Error::ActivityStorage)?;
        Ok(db.last_insert_rowid())
    }

    pub(crate) fn finish_activity(&self, id: i64, event: &ConfigActivity) -> Result<()> {
        let db = self.db().map_err(|_| Error::ActivityStorage)?;
        let json = serde_json::to_string(event).map_err(|_| Error::ActivityStorage)?;
        db.execute("UPDATE config_activity SET event=?1 WHERE tool=?2 AND id=?3", params![json, event.tool.id(), id])
            .map_err(|_| Error::ActivityStorage)?;
        Ok(())
    }

    pub(crate) fn config_activity(&self, tool: Tool) -> Result<Vec<ConfigActivity>> {
        if !self.exists() { return Ok(Vec::new()); }
        let db = self.db().map_err(|_| Error::ActivityStorage)?;
        let mut statement = db.prepare("SELECT event FROM config_activity WHERE tool=?1 ORDER BY id DESC LIMIT ?2")
            .map_err(|_| Error::ActivityStorage)?;
        let rows = statement.query_map(params![tool.id(), ACTIVITY_LIMIT as u32], |row| row.get::<_, String>(0))
            .map_err(|_| Error::ActivityStorage)?;
        rows.map(|row| {
            let json = row.map_err(|_| Error::ActivityStorage)?;
            let event: ConfigActivity = serde_json::from_str(&json).map_err(|_| Error::ActivityStorage)?;
            if event.tool != tool { return Err(Error::ActivityStorage); }
            Ok(event)
        }).collect()
    }
}
