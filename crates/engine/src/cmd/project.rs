//! Library projects: open or create the folder of a library, read and change its `uigen.json`.

use std::path::Path;
use std::sync::Arc;

use figcraft_doc::{Document, NodeKind};
use figcraft_project::{DEFAULT_COMPONENT_TEMPLATE, FILE_NAME, MAX_NAME_CHARS, Project, WORK_DIR, component_name};
use serde_json::{Value, json};

use super::CommandSpec;
use crate::file_access::{self, Access};
use crate::project::{self, ProjectBinding};
use crate::{DocState, EngineError, Result, Session, bad, spec};

const OPEN: &str = "project.open";
const UPDATE: &str = "project.update";
const UPDATABLE: [&str; 4] = ["name", "tagPrefix", "litOutDir", "appliedTheme"];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "project.open",
            "Open library project",
            "{path (the project folder), create? (a folder without uigen.json: write it, its work folder and a first design file, never overwriting), firstComponent? (name of the first component created, default \"Component 1\")}  opens every design file of the project in a tab bound to it; returns {opened: true, created, root, name, index (the first design file's tab), files: [{name, index}]}, or {opened: false, missing: true, root} for a folder without uigen.json (and no `create`)",
            false,
            open
        ),
        spec!(
            "project.openFolder",
            "Open project folder",
            "{}  asks the interface for a folder to open as a library project (no dialog from a programmatic call)",
            false,
            |s, _| ask(s, "dialog.projectOpen")
        ),
        spec!(
            "project.newFolder",
            "New library project",
            "{}  asks the interface for a folder to start a library project in (no dialog from a programmatic call)",
            false,
            |s, _| ask(s, "dialog.projectNew")
        ),
        spec!(
            "project.settings",
            "Project settings",
            "{}  asks the interface to open the settings window of the current tab's library project (no dialog from a programmatic call)",
            false,
            settings
        ),
        spec!(
            "project.info",
            "Library project info",
            "{}  the library project of the current tab: {project: null | {root, name, designFiles, tagPrefix, litOutDir, appliedTheme, sources}, components: [{page, name, empty, layers}]}  (components: the pages that are no divider, in order; layers: top-level layers without a reference or note role; [] outside a project)",
            false,
            info
        ),
        spec!(
            "project.update",
            "Update library project",
            "{name?, tagPrefix?, litOutDir?, appliedTheme? (a string, or null to clear)}  rewrites the uigen.json of the current tab's project (sources and other keys are kept); every tab of the project sees the change. Returns {project}",
            false,
            update
        ),
    ]
}

fn ask(s: &mut Session, dialog: &str) -> Result<Value> {
    s.request_ui(dialog, json!({}));
    super::ok()
}

fn settings(s: &mut Session, _: &Value) -> Result<Value> {
    if !s.in_lib_project() {
        return Err(bad("project.settings", "the current file is not in a library project"));
    }
    ask(s, "dialog.projectSettings")
}

fn text(cmd: &str, field: &str, value: &str) -> Result<String> {
    let value = value.trim();
    let reason = if value.is_empty() {
        "must not be empty"
    } else if value.chars().count() > MAX_NAME_CHARS {
        "is too long"
    } else if value.chars().any(char::is_control) {
        "must not contain control characters"
    } else {
        return Ok(value.to_string());
    };
    Err(bad(cmd, format!("{field} {reason}")))
}

fn string_field<'a>(cmd: &str, field: &str, value: &'a Value) -> Result<&'a str> {
    value.as_str().ok_or_else(|| bad(cmd, format!("{field} must be a string")))
}

fn component(value: &str) -> Result<String> {
    let name = text(OPEN, "firstComponent", value)?;
    if name.starts_with('-') {
        return Err(bad(OPEN, "firstComponent must not start with a hyphen (that makes a divider)"));
    }
    Ok(name)
}

fn flag(p: &Value, key: &str) -> Result<bool> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(on)) => Ok(*on),
        Some(_) => Err(bad(OPEN, format!("{key} must be true or false"))),
    }
}

fn open(s: &mut Session, p: &Value) -> Result<Value> {
    let root = p.get("path").and_then(Value::as_str).filter(|r| !r.is_empty()).ok_or_else(|| bad(OPEN, "path must be the project folder"))?;
    let create = flag(p, "create")?;
    let first_component = match p.get("firstComponent") {
        None | Some(Value::Null) => component_name(DEFAULT_COMPONENT_TEMPLATE, 1),
        Some(value) => component(string_field(OPEN, "firstComponent", value)?)?,
    };
    file_access::allow(root, Access::Read)?;
    if !file_access::is_dir(root) {
        return Err(bad(OPEN, format!("{root} is not a folder")));
    }
    let (config, created) = if file_access::exists(project::manifest_path(root)) {
        (project::load(root).map_err(|e| bad(OPEN, e.to_string()))?, false)
    } else if create {
        (create_project(s, root, &first_component)?, true)
    } else {
        return Ok(json!({"opened": false, "missing": true, "root": root}));
    };
    let binding = Arc::new(ProjectBinding { root: root.to_string(), config });
    let files = open_design_files(s, &binding)?;
    let index = files.first().map_or(s.active, |(_, i)| *i);
    s.active = index;
    for (_, tab) in files.iter().rev() {
        if let Some(path) = s.docs.get(*tab).and_then(|d| d.path.clone()) {
            super::file::remember(s, &path);
        }
    }
    let files: Vec<Value> = files.iter().map(|(name, i)| json!({"name": name, "index": i})).collect();
    Ok(json!({"opened": true, "created": created, "root": root, "name": binding.config.name, "index": index, "files": files}))
}

/// Make the manifest, the work folder and the first design file of the empty project folder `root`.
/// The manifest comes last: a failure before it leaves a folder that is not a project yet.
fn create_project(s: &Session, root: &str, first_component: &str) -> Result<Project> {
    let folder =
        Path::new(root).file_name().and_then(|n| n.to_str()).ok_or_else(|| bad(OPEN, format!("{root} has no folder name to name the project")))?;
    let config = Project::new(&text(OPEN, "name", folder)?);
    file_access::create_dir_all(Path::new(root).join(WORK_DIR))?;
    for file in &config.design_files {
        let path = Path::new(root).join(file);
        if !file_access::exists(&path) {
            file_access::write(&path, &first_design_file(s, first_component)?)?;
        }
    }
    project::save(root, &config)?;
    Ok(config)
}

fn first_design_file(s: &Session, component: &str) -> Result<Vec<u8>> {
    let mut doc = Document::new("lib");
    let background = s.new_page_background();
    for page in doc.pages.clone() {
        if let Some(node) = doc.node_mut(page) {
            node.name = component.to_string();
            if let NodeKind::Page { background: b, .. } = &mut node.kind {
                *b = background;
            }
        }
    }
    figcraft_format::save_bytes(&doc, None).map_err(|e| EngineError::Other(e.to_string()))
}

/// Open the design files of `binding` in tabs bound to it (an open one is reused) and give back
/// each file's name and tab. Every file is read before any tab opens, so a failure opens nothing.
fn open_design_files(s: &mut Session, binding: &Arc<ProjectBinding>) -> Result<Vec<(String, usize)>> {
    enum Tab {
        Open(usize),
        New(Box<DocState>),
    }
    let mut message = None;
    let mut tabs = Vec::new();
    for name in &binding.config.design_files {
        let path = Path::new(&binding.root).join(name).to_string_lossy().into_owned();
        if !file_access::metadata(&path).is_ok_and(|m| m.is_file()) {
            return Err(bad(OPEN, format!("{FILE_NAME}: designFiles: \"{name}\" does not exist in {}", binding.root)));
        }
        let open = s.docs.iter().position(|d| d.path.as_deref().is_some_and(|p| file_access::same_file(p, &path)));
        tabs.push((
            name.clone(),
            match open {
                Some(i) => Tab::Open(i),
                None => {
                    let mut read = super::file::read_tab(&path, None)?;
                    message = read.message.take().or(message);
                    Tab::New(Box::new(read.ds))
                }
            },
        ));
    }
    let mut files = Vec::new();
    for (name, tab) in tabs {
        let index = match tab {
            Tab::Open(i) => i,
            Tab::New(ds) => {
                s.docs.push(*ds);
                s.docs.len() - 1
            }
        };
        if let Some(ds) = s.docs.get_mut(index) {
            ds.project = Some(Arc::clone(binding));
        }
        files.push((name, index));
    }
    if message.is_some() {
        s.message = message;
    }
    Ok(files)
}

fn describe(binding: &ProjectBinding) -> Result<Value> {
    let c = &binding.config;
    let sources = serde_json::to_value(&c.sources).map_err(|e| EngineError::Other(e.to_string()))?;
    Ok(json!({
        "root": binding.root,
        "name": c.name,
        "designFiles": c.design_files,
        "tagPrefix": c.tag_prefix,
        "litOutDir": c.lit_out_dir,
        "appliedTheme": c.applied_theme,
        "sources": sources,
    }))
}

fn info(s: &mut Session, _: &Value) -> Result<Value> {
    let project = s.project().map(|b| describe(b)).transpose()?;
    let components: Vec<Value> = if project.is_some() {
        project::component_pages(s.doc()?)
            .into_iter()
            .map(|c| json!({"page": c.page, "name": c.name, "empty": c.empty, "layers": c.layers}))
            .collect()
    } else {
        Vec::new()
    };
    Ok(json!({"project": project, "components": components}))
}

pub(super) fn remember_theme(s: &mut Session, cmd: &str, theme: &str) -> Result<()> {
    let binding = s.project().cloned().ok_or_else(|| bad(cmd, "the current file is not in a library project"))?;
    let mut config = project::load(&binding.root).map_err(|e| bad(cmd, e.to_string()))?;
    config.applied_theme = Some(theme.to_string());
    config.validate().map_err(|e| bad(cmd, format!("{FILE_NAME}: {e}")))?;
    project::save(&binding.root, &config)?;
    s.rebind_project(&binding.root, config);
    Ok(())
}

fn update(s: &mut Session, p: &Value) -> Result<Value> {
    let binding = s.project().cloned().ok_or_else(|| bad(UPDATE, "the current file is not in a library project"))?;
    let fields = p.as_object().ok_or_else(|| bad(UPDATE, "params must be an object"))?;
    if let Some(unknown) = fields.keys().find(|k| !UPDATABLE.contains(&k.as_str())) {
        return Err(bad(UPDATE, format!("unknown field {unknown} (can change: {})", UPDATABLE.join(", "))));
    }
    // Read again: what the file holds now is what changes, not what the tab read when it opened.
    let mut config = project::load(&binding.root).map_err(|e| bad(UPDATE, e.to_string()))?;
    if let Some(value) = fields.get("name") {
        config.name = text(UPDATE, "name", string_field(UPDATE, "name", value)?)?;
    }
    if let Some(value) = fields.get("tagPrefix") {
        config.tag_prefix = string_field(UPDATE, "tagPrefix", value)?.to_string();
    }
    if let Some(value) = fields.get("litOutDir") {
        config.lit_out_dir = string_field(UPDATE, "litOutDir", value)?.to_string();
    }
    if let Some(value) = fields.get("appliedTheme") {
        config.applied_theme = match value {
            Value::Null => None,
            other => Some(string_field(UPDATE, "appliedTheme", other)?.to_string()),
        };
    }
    config.validate().map_err(|e| bad(UPDATE, format!("{FILE_NAME}: {e}")))?;
    project::save(&binding.root, &config)?;
    let updated = s.rebind_project(&binding.root, config);
    Ok(json!({"project": describe(&updated)?}))
}
