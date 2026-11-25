use super::{list_snapshots, Snapshot, Storage};
use crate::{Error, Result, Tool};
use rusqlite::{params, TransactionBehavior};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const KEEP_RECENT: usize = 5;
const CLEANUP_BATCH: usize = 50;

#[derive(Serialize)]
pub struct SnapshotCleanupPreview {
    pub revision: String,
    pub keep_recent: usize,
    pub protected: usize,
    pub eligible: usize,
    pub candidates: Vec<Snapshot>,
}

#[derive(Serialize)]
pub struct SnapshotCleanupResult {
    pub removed: usize,
    pub stored_bytes: u64,
}

impl Storage {
    pub(crate) fn pin_snapshot(&self, tool: Tool, path: &str, id: i64, pinned: bool) -> Result<()> {
        if !self.exists() { return Err(Error::SnapshotMissing); }
        let mut db = self.db()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_| Error::BackupFailed)?;
        let belongs = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM snapshots WHERE id=?1 AND tool=?2 AND path=?3)",
            params![id, tool.id(), path], |row| row.get::<_, bool>(0),
        ).map_err(|_| Error::BackupFailed)?;
        if !belongs { return Err(Error::SnapshotMissing); }
        if pinned {
            tx.execute("INSERT OR IGNORE INTO snapshot_pins(snapshot_id) VALUES(?1)", [id])
        } else {
            tx.execute("DELETE FROM snapshot_pins WHERE snapshot_id=?1", [id])
        }.map_err(|_| Error::BackupFailed)?;
        tx.commit().map_err(|_| Error::BackupFailed)
    }

    pub(crate) fn preview_snapshot_cleanup(&self, tool: Tool, path: &str) -> Result<SnapshotCleanupPreview> {
        let snapshots = if self.exists() { list_snapshots(&self.db()?, tool, path)? } else { Vec::new() };
        cleanup_preview(tool, path, snapshots)
    }

    pub(crate) fn apply_snapshot_cleanup(&self, tool: Tool, path: &str, revision: &str, selected: &[i64])
        -> Result<SnapshotCleanupResult> {
        if selected.is_empty() || selected.len() > CLEANUP_BATCH { return Err(Error::SnapshotCleanupSelection); }
        let selected_ids: BTreeSet<_> = selected.iter().copied().collect();
        if selected_ids.len() != selected.len() { return Err(Error::SnapshotCleanupSelection); }
        if !self.exists() { return Err(Error::Conflict); }

        let mut db = self.db()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_| Error::BackupFailed)?;
        let preview = cleanup_preview(tool, path, list_snapshots(&tx, tool, path)?)?;
        if preview.revision != revision { return Err(Error::Conflict); }
        let chosen: Vec<_> = preview.candidates.into_iter().filter(|snapshot| selected_ids.contains(&snapshot.id)).collect();
        if chosen.len() != selected_ids.len() { return Err(Error::SnapshotCleanupSelection); }

        let mut stored_bytes = 0;
        for snapshot in chosen {
            let deleted = tx.execute(
                "DELETE FROM snapshots WHERE id=?1 AND tool=?2 AND path=?3",
                params![snapshot.id, tool.id(), path],
            ).map_err(|_| Error::BackupFailed)?;
            if deleted != 1 { return Err(Error::Conflict); }
            stored_bytes += snapshot.stored_bytes;
        }
        tx.commit().map_err(|_| Error::BackupFailed)?;
        Ok(SnapshotCleanupResult { removed: selected_ids.len(), stored_bytes })
    }
}

fn cleanup_preview(tool: Tool, path: &str, snapshots: Vec<Snapshot>) -> Result<SnapshotCleanupPreview> {
    let encoded = serde_json::to_vec(&("snapshot-cleanup-v1", tool, path, &snapshots)).map_err(|_| Error::BackupFailed)?;
    let revision = hex::encode(Sha256::digest(encoded));
    let total = snapshots.len();
    // Keeping the newest row for every target also prevents an existing snapshot ID from being reused.
    let mut candidates: Vec<_> = snapshots.into_iter().skip(KEEP_RECENT)
        .filter(|snapshot| !snapshot.pinned && snapshot.reason != "before_plan")
        .collect();
    let eligible = candidates.len();
    candidates.reverse();
    candidates.truncate(CLEANUP_BATCH);
    Ok(SnapshotCleanupPreview { revision, keep_recent: KEEP_RECENT, protected: total - eligible, eligible, candidates })
}
