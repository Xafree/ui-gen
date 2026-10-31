//! The UIGEN library project file `uigen.json`: model, validation, deterministic JSON and
//! component naming. Pure data, no disk access.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod npm;

pub const FILE_NAME: &str = "uigen.json";
pub const WORK_DIR: &str = ".uigen";
pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_JSON_BYTES: usize = 1 << 20;
pub const MAX_SOURCES: usize = 200;
pub const MAX_DESIGN_FILES: usize = 20;
pub const ROLE_KEY: &str = "uigen.role";
pub const MAX_NAME_CHARS: usize = 200;
pub const DEFAULT_COMPONENT_TEMPLATE: &str = "Component {n}";

const DESIGN_FILE_EXTENSION: &str = ".figcraft";
const COUNTER_PLACEHOLDER: &str = "{n}";
const MAX_NAME_ATTEMPTS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ProjectError {
    #[error("{bytes} bytes is over the limit of {max}")]
    TooLarge { bytes: usize, max: usize },
    #[error("not valid: {0}")]
    Json(String),
    #[error("{}", version_message(*.found))]
    Version { found: u32 },
    #[error("{field}: {reason}")]
    Invalid { field: String, reason: String },
}

fn version_message(found: u32) -> String {
    if found > SCHEMA_VERSION {
        format!("version {found} is newer than this app supports ({SCHEMA_VERSION}); update UIGEN")
    } else {
        format!("version {found} is not supported (expected {SCHEMA_VERSION})")
    }
}

fn invalid(field: impl Into<String>, reason: impl Into<String>) -> ProjectError {
    ProjectError::Invalid { field: field.into(), reason: reason.into() }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    #[serde(default)]
    pub package: String,
    #[serde(default)]
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default)]
    pub removed: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub version: u32,
    pub name: String,
    #[serde(default = "default_design_files")]
    pub design_files: Vec<String>,
    #[serde(default = "default_tag_prefix")]
    pub tag_prefix: String,
    #[serde(default = "default_lit_out_dir")]
    pub lit_out_dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_theme: Option<String>,
    #[serde(default)]
    pub sources: Vec<Source>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn default_design_files() -> Vec<String> {
    vec!["lib.figcraft".to_string()]
}

fn default_tag_prefix() -> String {
    "ui-".to_string()
}

fn default_lit_out_dir() -> String {
    "lit".to_string()
}

impl Project {
    pub fn new(name: &str) -> Self {
        Self {
            version: SCHEMA_VERSION,
            name: name.to_string(),
            design_files: default_design_files(),
            tag_prefix: default_tag_prefix(),
            lit_out_dir: default_lit_out_dir(),
            applied_theme: None,
            sources: Vec::new(),
            extra: BTreeMap::new(),
        }
    }

    pub fn parse(input: &str) -> Result<Self, ProjectError> {
        if input.len() > MAX_JSON_BYTES {
            return Err(ProjectError::TooLarge { bytes: input.len(), max: MAX_JSON_BYTES });
        }
        let root: Value = serde_json::from_str(input).map_err(|e| ProjectError::Json(e.to_string()))?;
        let Value::Object(fields) = &root else {
            return Err(ProjectError::Json("the root must be a JSON object".to_string()));
        };
        let found = fields.get("version").and_then(Value::as_u64).ok_or_else(|| invalid("version", "must be a non-negative integer"))?;
        let found = u32::try_from(found).unwrap_or(u32::MAX);
        if found != SCHEMA_VERSION {
            return Err(ProjectError::Version { found });
        }
        let project: Self = serde_json::from_value(root).map_err(|e| ProjectError::Json(e.to_string()))?;
        project.validate()?;
        Ok(project)
    }

    pub fn to_json(&self) -> Result<String, ProjectError> {
        let mut text = serde_json::to_string_pretty(self).map_err(|e| ProjectError::Json(e.to_string()))?;
        text.push('\n');
        Ok(text)
    }

    pub fn validate(&self) -> Result<(), ProjectError> {
        if self.version != SCHEMA_VERSION {
            return Err(ProjectError::Version { found: self.version });
        }
        validate_name(&self.name)?;
        validate_tag_prefix(&self.tag_prefix)?;
        validate_lit_out_dir(&self.lit_out_dir)?;
        validate_design_files(&self.design_files)?;
        if self.applied_theme.as_deref() == Some("") {
            return Err(invalid("appliedTheme", "must not be empty"));
        }
        validate_sources(&self.sources)
    }
}

fn validate_name(name: &str) -> Result<(), ProjectError> {
    let reason = if name.trim().is_empty() {
        "must not be empty"
    } else if name.chars().any(char::is_control) {
        "must not contain control characters"
    } else if name.chars().count() > MAX_NAME_CHARS {
        "is too long"
    } else {
        return Ok(());
    };
    Err(invalid("name", reason))
}

fn validate_tag_prefix(prefix: &str) -> Result<(), ProjectError> {
    let starts_with_letter = prefix.chars().next().is_some_and(|c| c.is_ascii_lowercase());
    let allowed = prefix.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if starts_with_letter && allowed && prefix.len() >= 2 && prefix.ends_with('-') {
        Ok(())
    } else {
        Err(invalid("tagPrefix", "must match [a-z][a-z0-9-]*- (for example \"ui-\")"))
    }
}

fn validate_lit_out_dir(dir: &str) -> Result<(), ProjectError> {
    let reason = if dir.is_empty() {
        "must not be empty"
    } else if dir.starts_with('/') || dir.contains(':') {
        "must be a relative path"
    } else if dir.contains('\\') {
        "must use / as separator"
    } else if dir.split('/').any(|segment| segment.is_empty() || segment == "." || segment == "..") {
        "must not contain empty, . or .. segments"
    } else {
        return Ok(());
    };
    Err(invalid("litOutDir", reason))
}

fn validate_design_files(files: &[String]) -> Result<(), ProjectError> {
    if files.is_empty() {
        return Err(invalid("designFiles", "must list at least one file"));
    }
    if files.len() > MAX_DESIGN_FILES {
        return Err(invalid("designFiles", format!("lists {} files, the limit is {MAX_DESIGN_FILES}", files.len())));
    }
    let mut seen = HashSet::new();
    for file in files {
        validate_design_file(file)?;
        if !seen.insert(file.to_lowercase()) {
            return Err(invalid("designFiles", format!("\"{file}\" is listed twice")));
        }
    }
    Ok(())
}

fn validate_design_file(file: &str) -> Result<(), ProjectError> {
    let is_path = file.contains(['/', '\\', ':']) || file.contains("..") || file.chars().any(char::is_control);
    let lowered = file.to_lowercase();
    let reason = if is_path {
        "must be a plain file name, not a path"
    } else if lowered.len() <= DESIGN_FILE_EXTENSION.len() || !lowered.ends_with(DESIGN_FILE_EXTENSION) {
        "must be a name ending in .figcraft"
    } else {
        return Ok(());
    };
    Err(invalid("designFiles", format!("\"{file}\" {reason}")))
}

fn validate_sources(sources: &[Source]) -> Result<(), ProjectError> {
    if sources.len() > MAX_SOURCES {
        return Err(invalid("sources", format!("lists {} sources, the limit is {MAX_SOURCES}", sources.len())));
    }
    let mut seen = HashSet::new();
    for (index, source) in sources.iter().enumerate() {
        if source.package.is_empty() {
            return Err(invalid(format!("sources[{index}].package"), "is required"));
        }
        if source.version.is_empty() {
            return Err(invalid(format!("sources[{index}].version"), "is required"));
        }
        npm::validate_package_name(&source.package).map_err(|error| in_source(index, "package", error))?;
        npm::validate_version(&source.version, false).map_err(|error| in_source(index, "version", error))?;
        if !seen.insert(source.package.as_str()) {
            return Err(invalid("sources", format!("package \"{}\" is listed twice", source.package)));
        }
    }
    Ok(())
}

fn in_source(index: usize, field: &str, error: ProjectError) -> ProjectError {
    match error {
        ProjectError::Invalid { reason, .. } => invalid(format!("sources[{index}].{field}"), reason),
        other => other,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Reference,
    Note,
}

impl Role {
    pub fn of(plugin_data: &BTreeMap<String, String>) -> Option<Self> {
        match plugin_data.get(ROLE_KEY).map(String::as_str) {
            Some("reference") => Some(Self::Reference),
            Some("note") => Some(Self::Note),
            _ => None,
        }
    }
}

pub fn component_name(template: &str, n: usize) -> String {
    let template = if template.contains(COUNTER_PLACEHOLDER) { template } else { DEFAULT_COMPONENT_TEMPLATE };
    template.replace(COUNTER_PLACEHOLDER, &n.to_string())
}

pub fn next_component_name<'a>(template: &str, existing: impl IntoIterator<Item = &'a str>, start: usize) -> String {
    let taken: HashSet<&str> = existing.into_iter().collect();
    (0..MAX_NAME_ATTEMPTS)
        .map(|offset| component_name(template, start.saturating_add(offset)))
        .find(|name| !taken.contains(name.as_str()))
        .unwrap_or_else(|| component_name(template, start.saturating_add(MAX_NAME_ATTEMPTS)))
}

#[cfg(test)]
mod npm_line_tests;
#[cfg(test)]
mod npm_tests;

#[cfg(test)]
mod project_tests;

#[cfg(test)]
mod validation_tests;
