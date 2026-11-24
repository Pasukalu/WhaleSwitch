mod adapters;
mod engine;
mod jsonc;
mod probe;
mod profiles;
mod skills;
mod sources;
mod storage;

#[cfg(feature = "desktop")]
mod desktop;
#[cfg(feature = "desktop")]
pub use desktop::run;

pub use adapters::{ConfigEnv, Field, Fields, Format, Tool, ToolInfo};
pub use engine::{Change, Engine, Export, View};
pub use probe::{ProbeInput, ProbeResult, Protocol};
pub use profiles::{Profile, ProfileArchive, ProfilePreview};
pub use skills::{scan_project_skills, SkillEntry, SkillInventory};
pub use sources::{scan_project_config, ConfigInspection, ConfigObservation, ProjectConfigReport, ProjectConfigSource, ProjectScope};
pub use storage::{SecretStore, Snapshot};

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
    Io,
    WriteFailed,
    RolledBack,
    RecoveryRequired,
    Cancelled,
}

pub type Result<T> = std::result::Result<T, Error>;
