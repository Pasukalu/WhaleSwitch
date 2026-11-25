use super::{SecretStore, Storage};
use crate::{Error, Result};
use chacha20poly1305::{aead::{Aead, Payload}, XNonce};
use rand::{rngs::OsRng, RngCore};
use rusqlite::{params, OptionalExtension, Row};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

const MAX_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct SkillBackup {
    pub id: i64,
    pub destination: String,
    pub timestamp: u64,
    pub action: String,
    pub file_count: u32,
}

impl Storage {
    pub(crate) fn skill_backups(&self, project: &str) -> Result<Vec<SkillBackup>> {
        if !self.exists() { return Ok(Vec::new()); }
        let db = self.db()?;
        let mut select = db.prepare(
            "SELECT id,destination,timestamp,action,file_count FROM skill_backups WHERE project=?1 ORDER BY id DESC")
            .map_err(|_| Error::BackupFailed)?;
        let rows = select.query_map([project], backup_row).map_err(|_| Error::BackupFailed)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|_| Error::BackupFailed)
    }

    pub(crate) fn insert_skill_backup(&self, vault: &dyn SecretStore, project: &str, destination: &str,
        action: &str, file_count: u32, bytes: &[u8]) -> Result<SkillBackup> {
        if bytes.len() > MAX_PAYLOAD_BYTES { return Err(Error::BackupFailed); }
        let mut db = self.db()?;
        let cipher = Self::cipher(&db, vault, true)?;
        let mut nonce = [0u8; 24];
        OsRng.try_fill_bytes(&mut nonce).map_err(|_| Error::BackupFailed)?;
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| Error::BackupFailed)?.as_millis() as u64;

        let tx = db.transaction().map_err(|_| Error::BackupFailed)?;
        tx.execute("INSERT INTO skill_backups(project,destination,timestamp,action,file_count,nonce,sealed)
            VALUES(?1,?2,?3,?4,?5,?6,X'')", params![project, destination, timestamp, action, file_count, nonce.as_slice()])
            .map_err(|_| Error::BackupFailed)?;
        let metadata = SkillBackup { id: tx.last_insert_rowid(), destination: destination.into(),
            timestamp, action: action.into(), file_count };
        let aad = associated(project, &metadata);
        let sealed = cipher.encrypt(XNonce::from_slice(&nonce), Payload { msg: bytes, aad: aad.as_bytes() })
            .map_err(|_| Error::BackupFailed)?;
        tx.execute("UPDATE skill_backups SET sealed=?1 WHERE id=?2", params![sealed, metadata.id])
            .map_err(|_| Error::BackupFailed)?;
        tx.commit().map_err(|_| Error::BackupFailed)?;

        let (_, saved) = self.read_skill_backup(vault, project, metadata.id)?;
        if saved.as_slice() != bytes { return Err(Error::BackupFailed); }
        Ok(metadata)
    }

    pub(crate) fn read_skill_backup(&self, vault: &dyn SecretStore, project: &str, id: i64)
        -> Result<(SkillBackup, Zeroizing<Vec<u8>>)> {
        if !self.exists() { return Err(Error::SnapshotMissing); }
        let db = self.db()?;
        let row = db.query_row("SELECT id,destination,timestamp,action,file_count,nonce,sealed FROM skill_backups
            WHERE id=?1 AND project=?2 AND length(sealed)<=?3",
            params![id, project, (MAX_PAYLOAD_BYTES + 16) as u32], |row| {
                Ok((backup_row(row)?, row.get::<_, Vec<u8>>(5)?, row.get::<_, Vec<u8>>(6)?))
            }).optional().map_err(|_| Error::BackupFailed)?.ok_or(Error::SnapshotMissing)?;
        let (metadata, nonce, sealed) = row;
        if nonce.len() != 24 { return Err(Error::BackupFailed); }
        let aad = associated(project, &metadata);
        let cipher = Self::cipher(&db, vault, false)?;
        let bytes = cipher.decrypt(XNonce::from_slice(&nonce), Payload { msg: &sealed, aad: aad.as_bytes() })
            .map(Zeroizing::new).map_err(|_| Error::BackupFailed)?;
        Ok((metadata, bytes))
    }
}

fn backup_row(row: &Row<'_>) -> rusqlite::Result<SkillBackup> {
    Ok(SkillBackup {
        id: row.get(0)?,
        destination: row.get(1)?,
        timestamp: row.get(2)?,
        action: row.get(3)?,
        file_count: row.get(4)?,
    })
}

fn associated(project: &str, record: &SkillBackup) -> String {
    format!("WhaleSwitch skill backup v1\0{project}\0{}\0{}\0{}\0{}\0{}",
        record.id, record.destination, record.timestamp, record.action, record.file_count)
}
