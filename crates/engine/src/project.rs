//! Library projects: the `uigen.json` of a folder bound to the tabs of its design files. The file is
//! read and written only through [`crate::file_access`], so the automation roots confine it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use figcraft_doc::{Document, NodeId};
use figcraft_project::{FILE_NAME, MAX_JSON_BYTES, Project, Role};

use crate::uigen_import::is_import_page;
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

    /// Give every tab of the project in folder `root` the manifest `config`.
    pub(crate) fn rebind_project(&mut self, root: &str, config: Project) -> Arc<ProjectBinding> {
        let updated = Arc::new(ProjectBinding { root: root.to_string(), config });
        for ds in &mut self.docs {
            if ds.project.as_ref().is_some_and(|b| file_access::same_file(&b.root, root)) {
                ds.project = Some(Arc::clone(&updated));
            }
        }
        updated
    }
}

pub(crate) fn manifest_path(root: &str) -> PathBuf {
    Path::new(root).join(FILE_NAME)
}

fn read_cause(root: &str) -> std::result::Result<Project, String> {
    let limit = u64::try_from(MAX_JSON_BYTES).unwrap_or(u64::MAX);
    let bytes = file_access::read_capped(manifest_path(root), limit).map_err(|e| e.to_string())?;
    let text = String::from_utf8(bytes).map_err(|_| "not valid UTF-8 text".to_string())?;
    Project::parse(&text).map_err(|e| e.to_string())
}

/// The manifest of project folder `root`, read and validated.
pub fn load(root: &str) -> Result<Project> {
    read_cause(root).map_err(|why| EngineError::Other(format!("{FILE_NAME}: {why}")))
}

/// Write `project` as the manifest of folder `root`, after validating it.
pub fn save(root: &str, project: &Project) -> Result<()> {
    project.validate().map_err(|e| EngineError::Other(format!("{FILE_NAME}: {e}")))?;
    let text = project.to_json().map_err(|e| EngineError::Other(format!("{FILE_NAME}: {e}")))?;
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
        Ok(config) if config.design_files.iter().any(|listed| listed.eq_ignore_ascii_case(name)) => Some(Ok(ProjectBinding { root, config })),
        Ok(_) => None,
        Err(why) => Some(Err(why)),
    }
}

/// A page of a library project seen as a component: its layers, annotations left out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentInfo {
    pub page: NodeId,
    pub name: String,
    pub empty: bool,
    pub layers: usize,
}

/// The top-level layers of `page` that make up its component: those with no `reference` or `note` role.
pub fn component_roots(doc: &Document, page: NodeId) -> Vec<NodeId> {
    let Some(node) = doc.node(page) else { return Vec::new() };
    node.children.iter().copied().filter(|child| doc.node(*child).is_some_and(|layer| Role::of(&layer.plugin_data).is_none())).collect()
}

/// Whether `page` is a component of a library project: any page whose name doesn't start with a
/// hyphen (a page that does is a divider, whether it holds layers or not) and that is no import
/// page, whatever its name.
pub fn is_component_page(doc: &Document, page: NodeId) -> bool {
    doc.node(page).is_some_and(|node| node.is_page() && !node.name.starts_with('-')) && !is_import_page(doc, page)
}

/// Whether a component of `doc` other than `except` is already named `name`, case apart.
pub fn component_name_in_use(doc: &Document, name: &str, except: Option<NodeId>) -> bool {
    let wanted = name.trim().to_lowercase();
    doc.pages
        .iter()
        .filter(|page| Some(**page) != except && is_component_page(doc, **page))
        .filter_map(|page| doc.node(*page))
        .any(|node| node.name.trim().to_lowercase() == wanted)
}

/// `base` if no component has that name, else `base 2`, `base 3`… the first free one.
pub fn free_component_name(doc: &Document, base: &str) -> String {
    free_component_name_except(doc, base, None)
}

/// [`free_component_name`] where the page `except` doesn't count: it is about to take the name.
pub fn free_component_name_except(doc: &Document, base: &str, except: Option<NodeId>) -> String {
    if !component_name_in_use(doc, base, except) {
        return base.to_string();
    }
    (2_usize..).map(|n| format!("{base} {n}")).find(|name| !component_name_in_use(doc, name, except)).unwrap_or_else(|| base.to_string())
}

/// The components of a library project's file: its pages in order, dividers left out.
pub fn component_pages(doc: &Document) -> Vec<ComponentInfo> {
    doc.pages
        .iter()
        .filter(|page| is_component_page(doc, **page))
        .filter_map(|page| {
            let node = doc.node(*page)?;
            let layers = component_roots(doc, *page).len();
            Some(ComponentInfo { page: *page, name: node.name.clone(), empty: layers == 0, layers })
        })
        .collect()
}
