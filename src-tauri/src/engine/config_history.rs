use super::{write_archive, Engine};
use crate::{
    storage::ACTIVITY_LIMIT, ActivityAction, ActivityStatus, ConfigActivity, Error, PlanFileStatus, Result,
    SnapshotCleanupPreview, SnapshotCleanupResult, Tool,
};
use serde::Serialize;
use std::{cmp::Reverse, path::Path};

#[derive(Serialize)]
struct ActivityArchive {
    version: u16,
    entries: Vec<ConfigActivity>,
}

impl Engine {
    pub fn pin_snapshot(&self, tool: Tool, target: &str, id: i64, pinned: bool) -> Result<()> {
        let _lock = self.storage.lock()?;
        self.bound_target(tool, target)?;
        self.storage.pin_snapshot(tool, target, id, pinned)
    }

    pub fn preview_snapshot_cleanup(&self, tool: Tool, target: &str) -> Result<SnapshotCleanupPreview> {
        self.bound_target(tool, target)?;
        self.storage.preview_snapshot_cleanup(tool, target)
    }

    pub fn apply_snapshot_cleanup(&self, tool: Tool, target: &str, revision: &str, selected: &[i64])
        -> Result<SnapshotCleanupResult> {
        let _lock = self.storage.lock()?;
        self.bound_target(tool, target)?;
        self.record_config_activity(tool, ActivityAction::Cleanup, |_| {
            self.storage.apply_snapshot_cleanup(tool, target, revision, selected)
        })
    }

    pub fn config_activity(&self, tool: Tool) -> Result<Vec<ConfigActivity>> {
        let mut events = self.storage.config_activity(tool)?;
        for run in self.storage.plan_runs()? {
            if let Some(entry) = run.entries.into_iter().find(|entry| entry.file.tool == tool) {
                let status = if run.finished {
                    match entry.file.status {
                        PlanFileStatus::Pending => ActivityStatus::Unconfirmed,
                        PlanFileStatus::Applied => ActivityStatus::Succeeded,
                        PlanFileStatus::Restored => ActivityStatus::RolledBack,
                        PlanFileStatus::Unchanged => ActivityStatus::Unchanged,
                        PlanFileStatus::Failed => ActivityStatus::Failed,
                        PlanFileStatus::Skipped => ActivityStatus::Skipped,
                        PlanFileStatus::RecoveryRequired => ActivityStatus::RecoveryRequired,
                    }
                } else {
                    ActivityStatus::Unconfirmed
                };
                events.push(ConfigActivity {
                    timestamp: run.timestamp,
                    tool,
                    action: ActivityAction::Plan,
                    status,
                    snapshot_id: entry.file.snapshot_id,
                });
            }
        }
        events.sort_by_key(|event| Reverse(event.timestamp));
        events.truncate(ACTIVITY_LIMIT);
        Ok(events)
    }

    pub fn export_config_activity(&self, tool: Tool, destination: &Path) -> Result<()> {
        write_archive(destination, &ActivityArchive { version: 1, entries: self.config_activity(tool)? })
    }

    pub(super) fn record_config_activity<T>(&self, tool: Tool, action: ActivityAction,
        perform: impl FnOnce(&mut Option<i64>) -> Result<T>) -> Result<T> {
        let Ok(mut event) = ConfigActivity::new(tool, action) else { return perform(&mut None); };
        // A history write cannot change the result of an already applied configuration operation.
        let record = self.storage.begin_activity(&event).ok();
        let result = perform(&mut event.snapshot_id);
        if let Some(id) = record {
            event.status = match &result {
                Ok(_) => ActivityStatus::Succeeded,
                Err(Error::RolledBack) => ActivityStatus::RolledBack,
                Err(Error::RecoveryRequired) => ActivityStatus::RecoveryRequired,
                Err(_) => ActivityStatus::Failed,
            };
            let _ = self.storage.finish_activity(id, &event);
        }
        result
    }
}
