use crate::{storage::Storage, Change, Error, Fields, Result, Tool};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub timestamp: u64,
    pub fields: Fields,
}

#[derive(Serialize)]
pub struct ProfilePreview {
    pub name: String,
    pub changes: Vec<Change>,
}

impl Storage {
    pub(crate) fn create_profile(&self, tool: Tool, name: &str, fields: Fields) -> Result<Profile> {
        let name = name.trim();
        let lower = name.to_ascii_lowercase();
        if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control)
            || ["sk-", "sk_", "ghp_", "github_pat_", "xoxb-", "sess-", "eyjhb"]
                .iter().any(|prefix| lower.contains(*prefix)) {
            return Err(Error::InvalidProfileName);
        }
        tool.validate(&fields)?;
        let encoded = serde_json::to_string(&fields).map_err(|_| Error::ProfileStorage)?;
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|_| Error::ProfileStorage)?.as_millis() as u64;
        let db = self.db().map_err(|_| Error::ProfileStorage)?;
        let inserted = db.execute(
            "INSERT INTO profiles(tool,name,timestamp,fields) VALUES (?1,?2,?3,?4)
             ON CONFLICT(tool,name) DO NOTHING",
            params![tool.id(), name, timestamp, encoded],
        ).map_err(|_| Error::ProfileStorage)?;
        if inserted == 0 { return Err(Error::ProfileNameExists); }
        Ok(Profile { id: db.last_insert_rowid(), name: name.to_owned(), timestamp, fields })
    }

    pub(crate) fn profiles(&self, tool: Tool) -> Result<Vec<Profile>> {
        if !self.exists() { return Ok(Vec::new()); }
        let db = self.db().map_err(|_| Error::ProfileStorage)?;
        let mut statement = db.prepare(
            "SELECT id,name,timestamp,fields FROM profiles WHERE tool=?1 ORDER BY id DESC",
        ).map_err(|_| Error::ProfileStorage)?;
        let rows = statement.query_map([tool.id()], profile_row).map_err(|_| Error::ProfileStorage)?;
        rows.map(|row| checked_profile(tool, row.map_err(|_| Error::ProfileStorage)?)).collect()
    }

    pub(crate) fn profile(&self, tool: Tool, id: i64) -> Result<Profile> {
        if !self.exists() { return Err(Error::ProfileMissing); }
        let db = self.db().map_err(|_| Error::ProfileStorage)?;
        let row = db.query_row(
            "SELECT id,name,timestamp,fields FROM profiles WHERE tool=?1 AND id=?2",
            params![tool.id(), id], profile_row,
        ).optional().map_err(|_| Error::ProfileStorage)?.ok_or(Error::ProfileMissing)?;
        checked_profile(tool, row)
    }

    pub(crate) fn delete_profile(&self, tool: Tool, id: i64) -> Result<()> {
        if !self.exists() { return Err(Error::ProfileMissing); }
        let db = self.db().map_err(|_| Error::ProfileStorage)?;
        let changes = db.execute("DELETE FROM profiles WHERE tool=?1 AND id=?2", params![tool.id(), id])
            .map_err(|_| Error::ProfileStorage)?;
        if changes == 0 { return Err(Error::ProfileMissing); }
        Ok(())
    }
}

type ProfileRow = (i64, String, u64, String);

fn profile_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProfileRow> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
}

fn checked_profile(tool: Tool, (id, name, timestamp, encoded): ProfileRow) -> Result<Profile> {
    let fields = serde_json::from_str(&encoded).map_err(|_| Error::ProfileStorage)?;
    tool.validate(&fields).map_err(|_| Error::ProfileStorage)?;
    Ok(Profile { id, name, timestamp, fields })
}
