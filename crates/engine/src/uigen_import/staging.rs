//! The import pages: a staged import lands in the page of its package and version, away from the
//! components of the library. The page is found by the marker it carries, never by its name, which
//! the user may change.

use figcraft_doc::{Document, NodeId, NodeKind};
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

#[cfg(test)]
pub(crate) fn is_import_page(doc: &Document, page: NodeId) -> bool {
    marker(doc, page).is_some()
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
