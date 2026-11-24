use crate::engine::{directory_exists, read_config_view};
use crate::{ConfigEnv, Error, Field, Result, Tool, View};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct ConfigInspection {
    pub config: ConfigObservation,
    pub active_env: Vec<ConfigEnv>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConfigObservation {
    View { view: View },
    Unavailable { path: Option<String>, issue: Error },
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectScope { Project, ProjectLocal }

use ProjectScope::{Project, ProjectLocal};

const CANDIDATES: [(Tool, ProjectScope, &str); 7] = [
    (Tool::Codex, Project, ".codex/config.toml"),
    (Tool::Claude, Project, ".claude/settings.json"),
    (Tool::Claude, ProjectLocal, ".claude/settings.local.json"),
    (Tool::Opencode, Project, "opencode.json"),
    (Tool::Opencode, Project, "opencode.jsonc"),
    (Tool::Opencode, Project, ".opencode/opencode.json"),
    (Tool::Opencode, Project, ".opencode/opencode.jsonc"),
];

#[derive(Serialize)]
pub struct ProjectConfigSource {
    pub tool: Tool,
    pub scope: ProjectScope,
    pub path: &'static str,
    pub exists: bool,
    pub issue: Option<Error>,
    pub fields: Vec<Field>,
}

#[derive(Serialize)]
pub struct ProjectConfigReport {
    pub root: String,
    pub sources: Vec<ProjectConfigSource>,
}

/// Inspects fixed project configuration candidates without resolving session precedence.
pub fn scan_project_config(root: &Path) -> Result<ProjectConfigReport> {
    if !directory_exists(root)? {
        return Err(Error::PathUnavailable);
    }
    let sources = CANDIDATES.into_iter().map(|(tool, scope, path)| {
        let (exists, issue, fields) = match read_config_view(tool, &root.join(path)) {
            Ok(view) => {
                let fields = view.fields.into_iter().filter_map(|(field, value)| {
                    let user_only = tool == Tool::Codex && matches!(field, Field::ModelProvider | Field::OpenaiBaseUrl);
                    (value.is_some() && !user_only).then_some(field)
                }).collect();
                (view.exists, view.issue, fields)
            }
            Err(error) => (false, Some(error), Vec::new()),
        };
        ProjectConfigSource { tool, scope, path, exists, issue, fields }
    }).collect();
    Ok(ProjectConfigReport { root: root.to_string_lossy().into_owned(), sources })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn exposes_candidate_paths_and_supported_field_names_only() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        fs::create_dir(root.join(".codex")).unwrap();
        fs::create_dir(root.join(".claude")).unwrap();
        fs::create_dir(root.join(".opencode")).unwrap();
        fs::write(root.join(".codex/config.toml"), r#"
            model = "distinct-model-name"
            model_provider = "user-only-provider"
            openai_base_url = "https://example.invalid/v1"
            api_key = "fixture-secret"
        "#).unwrap();
        fs::write(root.join(".claude/settings.json"), "{invalid").unwrap();
        fs::write(root.join(".claude/settings.local.json"), r#"{"env":{"ANTHROPIC_BASE_URL":"https://example.invalid","ANTHROPIC_API_KEY":"fixture-secret"}}"#).unwrap();
        fs::write(root.join(".opencode/opencode.jsonc"), "{ /* JSONC */ \"model\": \"openai/test\" }").unwrap();
        fs::write(root.join("opencode.json"), r#"{"default_agent":"build"}"#).unwrap();

        let report = scan_project_config(&root).unwrap();
        assert_eq!(report.sources.len(), 7);
        assert_eq!(report.sources[0].fields, vec![Field::Model]);
        assert_eq!(report.sources[1].issue, Some(Error::InvalidConfig));
        assert_eq!(report.sources[2].fields, vec![Field::AnthropicBaseUrl]);
        assert_eq!(report.sources[3].fields, vec![Field::DefaultAgent]);
        assert!(!report.sources[4].exists);
        assert!(report.sources[4].issue.is_none());
        assert_eq!(report.sources[6].fields, vec![Field::Model]);
        let ipc = serde_json::to_string(&report).unwrap();
        assert!(ipc.contains(r#""scope":"project_local""#));
        for value in ["distinct-model-name", "user-only-provider", "example.invalid", "fixture-secret", "openai/test"] {
            assert!(!ipc.contains(value));
        }
        assert_eq!(fs::read_to_string(root.join(".claude/settings.json")).unwrap(), "{invalid");
    }

    #[cfg(unix)]
    #[test]
    fn refuses_project_symlinks() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        fs::create_dir(root.join("elsewhere")).unwrap();
        fs::write(root.join("elsewhere/settings.json"), r#"{"model":"fruit"}"#).unwrap();
        symlink(root.join("elsewhere"), root.join(".claude")).unwrap();
        let report = scan_project_config(&root).unwrap();
        assert_eq!(report.sources[1].issue, Some(Error::UnsafePath));
        assert!(report.sources[1].fields.is_empty());
        assert!(matches!(scan_project_config(&root.join(".claude")), Err(Error::UnsafePath)));
    }
}
