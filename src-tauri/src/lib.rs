mod adapters;
mod engine;
mod jsonc;
mod plans;
mod probe;
mod profiles;
#[cfg(any(all(windows, feature = "desktop"), test))]
mod portable;
mod skills;
mod sources;
mod storage;

#[cfg(feature = "desktop")]
mod desktop;
#[cfg(feature = "desktop")]
pub use desktop::run;

pub use adapters::{ConfigEnv, Field, Fields, Format, Tool, ToolInfo};
pub use engine::{Change, Engine, Export, SkillEditPreview, SkillImportFile, SkillImportPreview, SkillTextChange, View};
pub use plans::{ConfigPlan, PlanArchive, PlanEntry, PlanFileResult, PlanFileStatus, PlanPreview, PlanProfile, PlanRun, PlanSelection};
pub use probe::{ProbeInput, ProbeOutcome, ProbeResult, ProbeStage, Protocol};
pub use profiles::{Profile, ProfileArchive, ProfilePreview};
pub use skills::{scan_project_skills, ProjectSkills, SkillChange, SkillChangeKind, SkillEntry, SkillInventory, SkillLocation};
pub use sources::{scan_project_config, ConfigInspection, ConfigObservation, ProjectConfigReport, ProjectConfigSource, ProjectScope};
pub use storage::{SecretStore, SkillBackup, Snapshot};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    PathUnavailable,
    AmbiguousConfig,
    UnsafePath,
    FileTooLarge,
    InvalidConfig,
    InvalidField,
    InvalidArchive,
    WrongTool,
    Conflict,
    BackupFailed,
    SnapshotMissing,
    SnapshotMismatch,
    VaultUnavailable,
    InvalidSecret,
    MissingSecret,
    InvalidEndpoint,
    LocalOnly,
    InvalidProfileName,
    ProfileNameExists,
    ProfileMissing,
    ProfileStorage,
    InvalidPlan,
    PlanNameExists,
    PlanMissing,
    PlanStorage,
    PlanOverlap,
    SkillBaselineIncomplete,
    SkillStorage,
    SkillImportName,
    SkillImportManifest,
    SkillImportLimit,
    SkillImportExists,
    SkillImportRolledBack,
    SkillImportCleanup,
    SkillEditTarget,
    SkillEditSelection,
    SkillEditRolledBack,
    SkillEditRecovery,
    Io,
    WriteFailed,
    RolledBack,
    RecoveryRequired,
    Cancelled,
}

pub type Result<T> = std::result::Result<T, Error>;
