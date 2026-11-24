use crate::{jsonc, Error, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, env, path::{Path, PathBuf}};
use toml_edit::{value, DocumentMut};
use url::Url;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tool { Codex, Claude, Opencode }

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    Model, ModelProvider, OpenaiBaseUrl, ModelReasoningEffort,
    ModelReasoningSummary, ModelVerbosity, AnthropicBaseUrl, DefaultAgent,
}

pub type Fields = BTreeMap<Field, Option<String>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Format { Toml, Json, Jsonc }

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConfigEnv {
    CodexHome,
    ClaudeConfigDir,
    XdgConfigHome,
    OpencodeConfig,
    OpencodeConfigDir,
    OpencodeConfigContent,
}

impl ConfigEnv {
    fn name(self) -> &'static str {
        match self {
            Self::CodexHome => "CODEX_HOME",
            Self::ClaudeConfigDir => "CLAUDE_CONFIG_DIR",
            Self::XdgConfigHome => "XDG_CONFIG_HOME",
            Self::OpencodeConfig => "OPENCODE_CONFIG",
            Self::OpencodeConfigDir => "OPENCODE_CONFIG_DIR",
            Self::OpencodeConfigContent => "OPENCODE_CONFIG_CONTENT",
        }
    }

    fn active(self) -> bool {
        env::var_os(self.name()).is_some_and(|value| !value.is_empty())
    }

    fn path(self) -> Result<Option<PathBuf>> {
        let path = env::var_os(self.name()).filter(|value| !value.is_empty()).map(PathBuf::from);
        if path.as_ref().is_some_and(|path| !path.is_absolute()) {
            return Err(Error::PathUnavailable);
        }
        Ok(path)
    }
}

#[derive(Serialize)]
pub struct ToolInfo {
    pub tool: Tool,
    pub installed: bool,
    pub executable: Option<String>,
    pub path: Option<String>,
    pub exists: bool,
    pub issue: Option<Error>,
}

impl Field {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::ModelProvider => "model_provider",
            Self::OpenaiBaseUrl => "openai_base_url",
            Self::ModelReasoningEffort => "model_reasoning_effort",
            Self::ModelReasoningSummary => "model_reasoning_summary",
            Self::ModelVerbosity => "model_verbosity",
            Self::AnthropicBaseUrl => "ANTHROPIC_BASE_URL",
            Self::DefaultAgent => "default_agent",
        }
    }
}

impl Tool {
    pub const ALL: [Self; 3] = [Self::Codex, Self::Claude, Self::Opencode];

    pub(crate) fn id(self) -> &'static str {
        match self { Self::Codex => "codex", Self::Claude => "claude", Self::Opencode => "opencode" }
    }

    pub(crate) fn fields(self) -> &'static [Field] {
        match self {
            Self::Codex => &[Field::Model, Field::ModelProvider, Field::OpenaiBaseUrl,
                Field::ModelReasoningEffort, Field::ModelReasoningSummary, Field::ModelVerbosity],
            Self::Claude => &[Field::Model, Field::AnthropicBaseUrl],
            Self::Opencode => &[Field::Model, Field::DefaultAgent],
        }
    }

    pub(crate) fn format(self, path: &Path) -> Format {
        match self {
            Self::Codex => Format::Toml,
            Self::Opencode if path.extension().is_some_and(|ext| ext == "jsonc") => Format::Jsonc,
            _ => Format::Json,
        }
    }

    pub(crate) fn executable(self) -> Option<PathBuf> {
        let path = env::var_os("PATH")?;
        #[cfg(windows)]
        let extensions = [".exe", ".cmd", ".bat", ".com"];
        #[cfg(not(windows))]
        let extensions = [""];
        for folder in env::split_paths(&path).filter(|folder| folder.is_absolute()) {
            for extension in extensions {
                let candidate = folder.join(format!("{}{extension}", self.id()));
                if let Ok(meta) = candidate.metadata() {
                    if !meta.is_file() { continue; }
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if meta.permissions().mode() & 0o111 == 0 { continue; }
                    }
                    return Some(candidate);
                }
            }
        }
        None
    }

    pub(crate) fn path(self) -> Result<PathBuf> {
        use ConfigEnv::*;
        let home = dirs::home_dir().ok_or(Error::PathUnavailable)?;
        match self {
            Self::Codex => Ok(CodexHome.path()?.unwrap_or_else(|| home.join(".codex")).join("config.toml")),
            Self::Claude => Ok(ClaudeConfigDir.path()?.unwrap_or_else(|| home.join(".claude")).join("settings.json")),
            Self::Opencode => {
                if OpencodeConfigContent.active() || OpencodeConfigDir.active() {
                    return Err(Error::AmbiguousConfig);
                }
                if let Some(path) = OpencodeConfig.path()? {
                    return match path.extension().and_then(|ext| ext.to_str()) {
                        Some("json" | "jsonc") => Ok(path),
                        _ => Err(Error::PathUnavailable),
                    };
                }
                let dir = XdgConfigHome.path()?.unwrap_or_else(|| home.join(".config")).join("opencode");
                opencode_default_target(&dir)
            }
        }
    }

    pub(crate) fn active_config_env(self) -> Vec<ConfigEnv> {
        use ConfigEnv::*;
        let variables: &[ConfigEnv] = match self {
            Self::Codex => &[CodexHome],
            Self::Claude => &[ClaudeConfigDir],
            Self::Opencode => &[XdgConfigHome, OpencodeConfig, OpencodeConfigDir, OpencodeConfigContent],
        };
        variables.iter().copied().filter(|variable| variable.active()).collect()
    }

    pub(crate) fn validate(self, fields: &Fields) -> Result<()> {
        for (&field, value) in fields {
            if !self.fields().contains(&field) || value.as_ref().is_some_and(|s| !self.valid_value(field, s)) {
                return Err(Error::InvalidField);
            }
        }
        Ok(())
    }

    fn valid_value(self, field: Field, s: &str) -> bool {
        let lower = s.to_ascii_lowercase();
        if lower.split(['/', ':', '=']).any(|part| ["sk-", "sk_", "ghp_", "github_pat_", "xoxb-", "eyjhb", "sess-"]
            .iter().any(|prefix| part.starts_with(*prefix))) {
            return false;
        }
        match field {
            Field::OpenaiBaseUrl | Field::AnthropicBaseUrl => s.len() <= 512 && !s.contains('%') && Url::parse(s).is_ok_and(|u| {
                matches!(u.scheme(), "https" | "http") && u.host().is_some()
                    && u.username().is_empty() && u.password().is_none() && u.query().is_none()
                    && u.fragment().is_none() && !s.chars().any(char::is_whitespace)
            }),
            Field::ModelReasoningEffort => matches!(s, "minimal" | "low" | "medium" | "high" | "xhigh" | "none" | "max" | "ultra"),
            Field::ModelReasoningSummary => matches!(s, "auto" | "concise" | "detailed" | "none"),
            Field::ModelVerbosity => matches!(s, "low" | "medium" | "high"),
            Field::ModelProvider | Field::DefaultAgent => identifier(s),
            Field::Model if self == Self::Opencode => s.split_once('/').is_some_and(|(p, m)| {
                identifier(p) && m.split('/').all(identifier) && s.len() <= 180
            }),
            Field::Model => s.len() <= 180 && s.split('/').all(identifier),
        }
    }

    pub(crate) fn read(self, format: Format, text: &str) -> Result<Fields> {
        let mut fields = self.read_verified(format, text)?;
        for &field in self.fields() {
            fields.entry(field).or_insert(None);
        }
        Ok(fields)
    }

    /// Omits fields with unsupported values. An absent key is represented by None.
    pub(crate) fn read_verified(self, format: Format, text: &str) -> Result<Fields> {
        let mut fields = Fields::new();
        match format {
            Format::Toml => {
                let doc = text.parse::<DocumentMut>().map_err(|_| Error::InvalidConfig)?;
                for &field in self.fields() {
                    match doc.get(field.name()) {
                        None => { fields.insert(field, None); }
                        Some(item) => {
                            if let Some(value) = item.as_str().filter(|s| self.valid_value(field, s)) {
                                fields.insert(field, Some(value.to_owned()));
                            }
                        }
                    }
                }
            }
            _ => {
                let comments = format == Format::Jsonc;
                let members = jsonc::parse(text, comments)?;
                for &field in self.fields() {
                    let read = |source: &str, object: &jsonc::Object| -> Option<Option<String>> {
                        let Some(member) = object.members.iter().find(|m| m.key == field.name()) else {
                            return Some(None);
                        };
                        serde_json::from_str::<String>(&source[member.value.clone()]).ok()
                            .filter(|s| self.valid_value(field, s)).map(Some)
                    };
                    let value = if field == Field::AnthropicBaseUrl {
                        match members.members.iter().find(|m| m.key == "env") {
                            Some(member) => {
                                let source = &text[member.value.clone()];
                                jsonc::parse(source, comments).ok().and_then(|object| read(source, &object))
                            }
                            None => Some(None),
                        }
                    } else { read(text, &members) };
                    if let Some(value) = value { fields.insert(field, value); }
                }
            }
        }
        Ok(fields)
    }

    pub(crate) fn apply(self, format: Format, text: &str, fields: &Fields) -> Result<String> {
        self.validate(fields)?;
        match format {
            Format::Toml => {
                let mut doc = text.parse::<DocumentMut>().map_err(|_| Error::InvalidConfig)?;
                for (&field, next) in fields {
                    match next {
                        Some(next) => {
                            let mut replacement = value(next.as_str());
                            if let (Some(previous), Some(new)) = (
                                doc.get(field.name()).and_then(|v| v.as_value()),
                                replacement.as_value_mut(),
                            ) {
                                *new.decor_mut() = previous.decor().clone();
                            }
                            if let Some(item) = doc.get_mut(field.name()) {
                                if !item.is_value() { return Err(Error::InvalidField); }
                                *item = replacement;
                            } else {
                                doc.insert(field.name(), replacement);
                            }
                        }
                        None => { doc.remove(field.name()); }
                    }
                }
                Ok(doc.to_string())
            }
            _ => {
                let mut edited = text.to_owned();
                for (&field, next) in fields {
                    let comments = format == Format::Jsonc;
                    edited = if field == Field::AnthropicBaseUrl {
                        jsonc::replace_nested(&edited, comments, "env", field.name(), next.as_deref())?
                    } else {
                        jsonc::replace(&edited, comments, field.name(), next.as_deref())?
                    };
                }
                Ok(edited)
            }
        }
    }
}

fn identifier(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.:@+".contains(&b))
}

fn opencode_default_target(dir: &Path) -> Result<PathBuf> {
    let json = dir.join("opencode.json");
    let jsonc = dir.join("opencode.jsonc");
    if present(&dir.join("config.json"))? {
        return Err(Error::AmbiguousConfig);
    }
    let has_jsonc = present(&jsonc)?;
    if has_jsonc && present(&json)? {
        return Err(Error::AmbiguousConfig);
    }
    Ok(if has_jsonc { jsonc } else { json })
}

fn present(path: &Path) -> Result<bool> {
    match path.symlink_metadata() {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(Error::Io),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opencode_resolves_a_new_target_only_when_unambiguous() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();
        let json = dir.join("opencode.json");
        let jsonc = dir.join("opencode.jsonc");
        assert_eq!(opencode_default_target(dir).unwrap(), json);
        std::fs::write(&json, "{}").unwrap();
        std::fs::write(&jsonc, "{}").unwrap();
        assert_eq!(opencode_default_target(dir), Err(Error::AmbiguousConfig));
        std::fs::remove_file(&json).unwrap();
        assert_eq!(opencode_default_target(dir).unwrap(), jsonc);
        std::fs::write(dir.join("config.json"), "{}").unwrap();
        assert_eq!(opencode_default_target(dir), Err(Error::AmbiguousConfig));
    }
}
