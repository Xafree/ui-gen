//! The import pages: a staged import lands in the page of its package and version, away from the
//! components of the library. The page is found by the marker it carries, never by its name, which
//! the user may change.

use figcraft_doc::{Document, NodeId, NodeKind};
use figcraft_project::Project;
use serde_json::{Value, json};

use super::pending::Request;
use crate::Result;

pub(crate) const IMPORT_PAGE_KEY: &str = "uigen.importPage";
pub(super) const SOURCE_KEY: &str = "uigen.source";

pub(crate) fn import_page_name(package: &str, version: &str) -> String {
    format!("- Imported · {package} {version}")
}

fn marker(doc: &Document, page: NodeId) -> Option<(String, String)> {
    let node = doc.node(page).filter(|node| node.is_page())?;
    let value: Value = serde_json::from_str(node.plugin_data.get(IMPORT_PAGE_KEY)?).ok()?;
    Some((value.get("package")?.as_str()?.to_owned(), value.get("version")?.as_str()?.to_owned()))
}

pub fn is_import_page(doc: &Document, page: NodeId) -> bool {
    marker(doc, page).is_some()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedSet {
    pub set: NodeId,
    pub name: String,
    pub variants: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSection {
    pub package: String,
    pub version: String,
    pub page: Option<NodeId>,
    pub removed: bool,
    pub components: Vec<ImportedSet>,
}

fn imported_sets(doc: &Document, page: NodeId) -> Vec<ImportedSet> {
    let Some(node) = doc.node(page) else { return Vec::new() };
    node.children
        .iter()
        .filter_map(|child| {
            let set = doc.node(*child).filter(|set| matches!(set.kind, NodeKind::ComponentSet { .. }))?;
            set.plugin_data.get(SOURCE_KEY).and_then(|text| serde_json::from_str::<Value>(text).ok()).filter(Value::is_object)?;
            let variants = set
                .children
                .iter()
                .filter(|variant| doc.node(**variant).is_some_and(|variant| matches!(variant.kind, NodeKind::Component { .. })))
                .count();
            Some(ImportedSet { set: *child, name: set.name.clone(), variants })
        })
        .collect()
}

fn section(doc: &Document, package: &str, version: &str, page: Option<NodeId>, removed: bool) -> ImportSection {
    let components = page.map(|page| imported_sets(doc, page)).unwrap_or_default();
    ImportSection { package: package.to_owned(), version: version.to_owned(), page, removed, components }
}

/// One section per source of the project that is not removed, in the order of the manifest, then one
/// per import page that belongs to an older version or to a removed source.
pub fn sections(doc: &Document, project: &Project) -> Vec<ImportSection> {
    let pages: Vec<(NodeId, (String, String))> = doc.pages.iter().filter_map(|page| Some((*page, marker(doc, *page)?))).collect();
    let mut found: Vec<ImportSection> = project
        .sources
        .iter()
        .filter(|source| !source.removed)
        .map(|source| {
            let page = pages.iter().find(|(_, (package, version))| *package == source.package && *version == source.version).map(|(page, _)| *page);
            section(doc, &source.package, &source.version, page, false)
        })
        .collect();
    for (page, (package, version)) in &pages {
        if !found.iter().any(|listed| listed.page == Some(*page)) {
            found.push(section(doc, package, version, Some(*page), true));
        }
    }
    found
}

/// The import page of the request's package and version, added at the end of the file when it has none.
pub(super) fn page_for(doc: &mut Document, request: &Request) -> Result<NodeId> {
    let wanted = Some((request.package.clone(), request.version.clone()));
    if let Some(page) = doc.pages.iter().copied().find(|page| marker(doc, *page) == wanted) {
        return Ok(page);
    }
    let page = doc.add_page(&import_page_name(&request.package, &request.version));
    let marker = json!({"package": request.package, "version": request.version});
    doc.get_mut(page)?.plugin_data.insert(IMPORT_PAGE_KEY.to_owned(), marker.to_string());
    Ok(page)
}

/// The sets of the page that come from this very component: a new import takes their place.
pub(super) fn previous_sets(doc: &Document, page: NodeId, request: &Request) -> Vec<NodeId> {
    let wanted = json!({"package": request.package, "version": request.version, "component": request.component});
    let Some(node) = doc.node(page) else { return Vec::new() };
    node.children
        .iter()
        .copied()
        .filter(|child| {
            doc.node(*child).is_some_and(|set| {
                matches!(set.kind, NodeKind::ComponentSet { .. })
                    && set.plugin_data.get(SOURCE_KEY).and_then(|text| serde_json::from_str::<Value>(text).ok()).as_ref() == Some(&wanted)
            })
        })
        .collect()
}

pub(super) fn position(doc: &Document, id: NodeId) -> Option<(f64, f64)> {
    let coeffs = doc.node(id)?.transform.as_coeffs();
    Some((coeffs[4], coeffs[5]))
}
