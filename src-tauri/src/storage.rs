use crate::{Error, Result, Tool};
use chacha20poly1305::{aead::{Aead, Payload}, KeyInit, XChaCha20Poly1305, XNonce};
use rand::{rngs::OsRng, RngCore};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::{fs::{self, File, OpenOptions}, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};
use zeroize::Zeroizing;

mod config_activity;
mod skill_backups;
mod snapshot_cleanup;
pub use config_activity::{ActivityAction, ActivityStatus, ConfigActivity};
pub(crate) use config_activity::ACTIVITY_LIMIT;
pub use skill_backups::SkillBackup;
pub use snapshot_cleanup::{SnapshotCleanupPreview, SnapshotCleanupResult};

#[cfg(test)]
mod native_test;

pub trait SecretStore: Send + Sync {
    fn get(&self, account: &str) -> Result<Option<Zeroizing<String>>>;
    fn set(&self, account: &str, secret: &str) -> Result<()>;
    fn remove(&self, account: &str) -> Result<()>;
}

pub struct SystemVault;

impl SecretStore for SystemVault {
    fn get(&self, account: &str) -> Result<Option<Zeroizing<String>>> {
        let entry = keyring::Entry::new("org.whaleswitch.desktop", account).map_err(|_| Error::VaultUnavailable)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(Zeroizing::new(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(Error::VaultUnavailable),
        }
    }

    fn set(&self, account: &str, secret: &str) -> Result<()> {
        keyring::Entry::new("org.whaleswitch.desktop", account)
            .and_then(|entry| entry.set_password(secret)).map_err(|_| Error::VaultUnavailable)
    }

    fn remove(&self, account: &str) -> Result<()> {
        let entry = keyring::Entry::new("org.whaleswitch.desktop", account).map_err(|_| Error::VaultUnavailable)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(Error::VaultUnavailable),
        }
    }
}

#[derive(Serialize)]
pub struct Snapshot {
    pub id: i64,
    pub timestamp: u64,
    pub reason: String,
    pub pinned: bool,
    pub stored_bytes: u64,
}

pub struct Storage {
    dir: PathBuf,
    lock_dir: PathBuf,
}

impl Storage {
    pub fn new(dir: PathBuf) -> Self { Self::with_lock_dir(dir.clone(), dir) }

    pub fn with_lock_dir(dir: PathBuf, lock_dir: PathBuf) -> Self { Self { dir, lock_dir } }

    pub fn exists(&self) -> bool { self.dir.join("snapshots.sqlite3").is_file() }

    pub fn lock(&self) -> Result<File> {
        fs::create_dir_all(&self.lock_dir).map_err(|_| Error::Io)?;
        let file = OpenOptions::new().create(true).truncate(false).read(true).write(true)
            .open(self.lock_dir.join("write.lock")).map_err(|_| Error::Io)?;
        fs2::FileExt::try_lock_exclusive(&file).map_err(|e| {
            let lock_error = fs2::lock_contended_error().raw_os_error();
            if e.kind() == std::io::ErrorKind::WouldBlock || (lock_error.is_some() && e.raw_os_error() == lock_error) {
                Error::Conflict
            } else {
                Error::Io
            }
        })?;
        Ok(file)
    }

    pub(crate) fn db(&self) -> Result<Connection> {
        fs::create_dir_all(&self.dir).map_err(|_| Error::Io)?;
        let db = Connection::open(self.dir.join("snapshots.sqlite3")).map_err(|_| Error::BackupFailed)?;
        db.busy_timeout(std::time::Duration::from_secs(4)).map_err(|_| Error::BackupFailed)?;
        db.execute_batch("PRAGMA synchronous=FULL;
            PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS snapshots (
              id INTEGER PRIMARY KEY, tool TEXT NOT NULL, path TEXT NOT NULL,
              timestamp INTEGER NOT NULL, reason TEXT NOT NULL,
              nonce BLOB NOT NULL, sealed BLOB NOT NULL
            );
            CREATE INDEX IF NOT EXISTS snapshot_target ON snapshots(tool, path, id);
            CREATE TABLE IF NOT EXISTS snapshot_pins (
              snapshot_id INTEGER PRIMARY KEY REFERENCES snapshots(id) ON DELETE CASCADE
            );
            CREATE TABLE IF NOT EXISTS config_activity (
              id INTEGER PRIMARY KEY AUTOINCREMENT, tool TEXT NOT NULL, event TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS config_activity_tool ON config_activity(tool, id);
            CREATE TABLE IF NOT EXISTS profiles (
              id INTEGER PRIMARY KEY AUTOINCREMENT, tool TEXT NOT NULL,
              name TEXT NOT NULL COLLATE NOCASE, timestamp INTEGER NOT NULL, fields TEXT NOT NULL,
              UNIQUE(tool, name)
            );
            CREATE TABLE IF NOT EXISTS config_plans (
              id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL COLLATE NOCASE UNIQUE,
              timestamp INTEGER NOT NULL, entries TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS plan_runs (
              id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, timestamp INTEGER NOT NULL,
              finished INTEGER NOT NULL, entries TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS skill_baselines (
              project TEXT PRIMARY KEY, timestamp INTEGER NOT NULL, fingerprints TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS skill_backups (
              id INTEGER PRIMARY KEY AUTOINCREMENT, project TEXT NOT NULL, destination TEXT NOT NULL,
              timestamp INTEGER NOT NULL, action TEXT NOT NULL CHECK(action IN ('upgrade', 'restore')),
              file_count INTEGER NOT NULL, nonce BLOB NOT NULL, sealed BLOB NOT NULL
            );
            CREATE INDEX IF NOT EXISTS skill_backups_project ON skill_backups(project, id);")
            .map_err(|_| Error::BackupFailed)?;
        Ok(db)
    }

    fn cipher(db: &Connection, vault: &dyn SecretStore, allow_new: bool) -> Result<XChaCha20Poly1305> {
        let existing = vault.get("snapshot-encryption-v1")?;
        let encoded = match existing {
            Some(key) => key,
            None if allow_new => {
                let empty = db.query_row(
                    "SELECT NOT EXISTS(SELECT 1 FROM snapshots) AND NOT EXISTS(SELECT 1 FROM skill_backups)", [], |r| r.get::<_, bool>(0))
                    .map_err(|_| Error::BackupFailed)?;
                if !empty { return Err(Error::VaultUnavailable); }
                let mut key = Zeroizing::new([0u8; 32]);
                OsRng.try_fill_bytes(key.as_mut()).map_err(|_| Error::BackupFailed)?;
                let encoded = Zeroizing::new(hex::encode(&*key));
                vault.set("snapshot-encryption-v1", &encoded)?;
                if vault.get("snapshot-encryption-v1")?.as_deref() != Some(&*encoded) {
                    return Err(Error::VaultUnavailable);
                }
                encoded
            }
            None => return Err(Error::VaultUnavailable),
        };
        let bytes = Zeroizing::new(hex::decode(encoded.as_bytes()).map_err(|_| Error::VaultUnavailable)?);
        XChaCha20Poly1305::new_from_slice(&bytes).map_err(|_| Error::VaultUnavailable)
    }

    pub fn insert(&self, vault: &dyn SecretStore, tool: Tool, path: &Path, reason: &str, bytes: &[u8]) -> Result<Snapshot> {
        let db = self.db()?;
        let target = path.to_str().ok_or(Error::UnsafePath)?;
        let cipher = Self::cipher(&db, vault, true)?;
        let mut nonce = [0u8; 24];
        OsRng.try_fill_bytes(&mut nonce).map_err(|_| Error::BackupFailed)?;
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| Error::BackupFailed)?.as_millis() as u64;
        let aad = associated(tool, target);
        let sealed = cipher.encrypt(XNonce::from_slice(&nonce), Payload { msg: bytes, aad: aad.as_bytes() })
            .map_err(|_| Error::BackupFailed)?;
        let stored_bytes = sealed.len() as u64;
        db.execute("INSERT INTO snapshots(tool,path,timestamp,reason,nonce,sealed) VALUES(?1,?2,?3,?4,?5,?6)",
            params![tool.id(), target, timestamp, reason, nonce.as_slice(), sealed]).map_err(|_| Error::BackupFailed)?;
        let id = db.last_insert_rowid();
        drop(db);
        if self.get(vault, tool, path, id)?.as_slice() != bytes { return Err(Error::BackupFailed); }
        Ok(Snapshot { id, timestamp, reason: reason.into(), pinned: false, stored_bytes })
    }

    pub fn list(&self, tool: Tool, path: &Path) -> Result<Vec<Snapshot>> {
        if !self.exists() { return Ok(Vec::new()); }
        let target = path.to_str().ok_or(Error::UnsafePath)?;
        list_snapshots(&self.db()?, tool, target)
    }

    pub fn get(&self, vault: &dyn SecretStore, tool: Tool, path: &Path, id: i64) -> Result<Zeroizing<Vec<u8>>> {
        if !self.exists() { return Err(Error::SnapshotMissing); }
        let db = self.db()?;
        let row = db.query_row("SELECT tool,path,nonce,sealed FROM snapshots WHERE id=?1", [id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Vec<u8>>(2)?, r.get::<_, Vec<u8>>(3)?))
        }).optional().map_err(|_| Error::BackupFailed)?.ok_or(Error::SnapshotMissing)?;
        let (saved_tool, saved_path, nonce, sealed) = row;
        let target = path.to_str().ok_or(Error::UnsafePath)?;
        if saved_tool != tool.id() || saved_path != target { return Err(Error::SnapshotMismatch); }
        if nonce.len() != 24 { return Err(Error::BackupFailed); }
        let cipher = Self::cipher(&db, vault, false)?;
        let aad = associated(tool, target);
        cipher.decrypt(XNonce::from_slice(&nonce), Payload { msg: &sealed, aad: aad.as_bytes() })
            .map(Zeroizing::new).map_err(|_| Error::BackupFailed)
    }
}

fn list_snapshots(db: &Connection, tool: Tool, target: &str) -> Result<Vec<Snapshot>> {
    let mut statement = db.prepare(
        "SELECT s.id,s.timestamp,s.reason,p.snapshot_id IS NOT NULL,length(s.sealed)
         FROM snapshots s LEFT JOIN snapshot_pins p ON p.snapshot_id=s.id
         WHERE s.tool=?1 AND s.path=?2 ORDER BY s.id DESC",
    ).map_err(|_| Error::BackupFailed)?;
    let rows = statement.query_map(params![tool.id(), target], |row| Ok(Snapshot {
        id: row.get(0)?,
        timestamp: row.get(1)?,
        reason: row.get(2)?,
        pinned: row.get(3)?,
        stored_bytes: row.get(4)?,
    })).map_err(|_| Error::BackupFailed)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|_| Error::BackupFailed)
}

fn associated(tool: Tool, path: &str) -> String { format!("WhaleSwitch snapshot v1\0{}\0{path}", tool.id()) }
