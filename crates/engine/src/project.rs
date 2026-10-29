//! Library projects: the `uigen.json` of a folder bound to the tabs of its design files. The file is
//! read and written only through [`crate::file_access`], so the automation roots confine it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use figcraft_project::{FILE_NAME, MAX_JSON_BYTES, Project, ProjectError, SCHEMA_VERSION};

use crate::{EngineError, Result, Session, file_access};

/// A library project a tab belongs to: its folder and the manifest as the tab last read it.
#[derive(Clone, Debug)]
pub struct ProjectBinding {
    pub root: String,
    pub config: Project,
}

impl Session {
    /// The library project of the current tab.
    pub fn project(&self) -> Option<&Arc<ProjectBinding>> {
        self.docs.get(self.active)?.project.as_ref()
    }

    /// Whether the current tab belongs to a library project.
    pub fn in_lib_project(&self) -> bool {
        self.project().is_some()
    }
}

pub(crate) fn manifest_path(root: &str) -> PathBuf {
    Path::new(root).join(FILE_NAME)
}

/// Why a manifest is refused, without the file name (messages say `uigen.json: <cause>`).
pub(crate) fn cause(error: &ProjectError) -> String {
    match error {
        ProjectError::TooLarge { bytes, max } => format!("{bytes} bytes is over the limit of {max}"),
        ProjectError::Json(why) => format!("not valid: {why}"),
        ProjectError::Version { found } if *found > SCHEMA_VERSION => {
            format!("version {found} is newer than this app supports ({SCHEMA_VERSION}); update UIGEN")
        }
        ProjectError::Version { found } => format!("version {found} is not supported (expected {SCHEMA_VERSION})"),
        ProjectError::Invalid { field, reason } => format!("{field}: {reason}"),
    }
}

fn read_cause(root: &str) -> std::result::Result<Project, String> {
    let limit = u64::try_from(MAX_JSON_BYTES).unwrap_or(u64::MAX);
    let bytes = file_access::read_capped(manifest_path(root), limit).map_err(|e| e.to_string())?;
    let text = String::from_utf8(bytes).map_err(|_| "not valid UTF-8 text".to_string())?;
    Project::parse(&text).map_err(|e| cause(&e))
}

/// The manifest of project folder `root`, read and validated.
pub fn load(root: &str) -> Result<Project> {
    read_cause(root).map_err(|why| EngineError::Other(format!("{FILE_NAME}: {why}")))
}

/// Write `project` as the manifest of folder `root`, after validating it.
pub fn save(root: &str, project: &Project) -> Result<()> {
    project.validate().map_err(|e| EngineError::Other(format!("{FILE_NAME}: {}", cause(&e))))?;
    let text = project.to_json().map_err(|e| EngineError::Other(format!("{FILE_NAME}: {}", cause(&e))))?;
    file_access::write(manifest_path(root), text.as_bytes())
}

/// The project that lists design file `path` in the manifest of its folder: `None` for a file
/// that is no project's, `Some(Err(why))` when the manifest there can't be read.
pub fn binding_for_file(path: &str) -> Option<std::result::Result<ProjectBinding, String>> {
    let file = Path::new(path);
    let name = file.file_name()?.to_str()?;
    let root = match file.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_string_lossy().into_owned(),
        _ => ".".to_string(),
    };
    if !file_access::exists(manifest_path(&root)) {
        return None;
    }
    match read_cause(&root) {
        Ok(config) if config.design_files.iter().any(|listed| listed == name) => Some(Ok(ProjectBinding { root, config })),
        Ok(_) => None,
        Err(why) => Some(Err(why)),
    }
}
