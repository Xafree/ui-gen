//! `uigen.imported`: the component sets of the import pages. `uigen.placeImported`: an independent
//! copy of one of them as the component of a page of the library.

use figcraft_doc::{Document, NodeId, NodeKind};
use figcraft_dom::MAX_NODES;
use figcraft_geom::Affine;
use serde_json::{Value, json};

use super::CommandSpec;
use super::project_sources::binding;
use crate::params::{get, id_of, req_id};
use crate::project::{component_roots, free_component_name, free_component_name_except, is_component_page};
use crate::uigen_import::{ImportSection, ImportedSet, is_import_page, sections};
use crate::{Result, Session, bad, spec};

const LIST: &str = "uigen.imported";
const PLACE: &str = "uigen.placeImported";

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            LIST,
            "List imported components",
            "{}  the components imported into this file, by source: {sections: [{package, version, page (the import page, or null when nothing is imported yet), removed (the source is removed or the page belongs to an older version), components: [{set, name, variants}]}]}. One section per source of the project that is not removed, in the order of uigen.json, then one per import page of an older version or of a removed source. The copies placed in the library are not listed",
            false,
            imported
        ),
        spec!(
            PLACE,
            "Use imported component",
            "{set (a component set of an import page), page? (a component page of the library; default the current page)}  copies the set onto the page as its component and returns {page, set (the copy), name, replaced: null | {page (the page that took the former content), name (its former name)}}. The copy is independent (new ids, its transitions aim at its own variants, variable bindings and uigen.source kept); the import page is not touched. The page takes the name of the set (\"Name 2\" when another component has it). A page that already holds a component hands all its layers, notes and measures included, to a new page right after it that keeps the former name (\"Name 2\" when that name is the new one). The copy is selected and the page becomes the current one. One undo step",
            true,
            place
        ),
    ]
}

fn set_json(set: &ImportedSet) -> Value {
    json!({"set": set.set, "name": set.name, "variants": set.variants})
}

fn section_json(section: &ImportSection) -> Value {
    let components: Vec<Value> = section.components.iter().map(set_json).collect();
    json!({"package": section.package, "version": section.version, "page": section.page, "removed": section.removed, "components": components})
}

fn imported(s: &mut Session, _: &Value) -> Result<Value> {
    let binding = binding(s, LIST)?;
    let listed: Vec<Value> = sections(s.doc()?, &binding.config).iter().map(section_json).collect();
    Ok(json!({"sections": listed}))
}

fn imported_set(s: &Session, p: &Value) -> Result<NodeId> {
    let doc = s.doc()?;
    let set = req_id(PLACE, p, "set")?;
    let node = doc.node(set).ok_or_else(|| bad(PLACE, format!("`set` {set} does not exist")))?;
    if !matches!(node.kind, NodeKind::ComponentSet { .. }) {
        return Err(bad(PLACE, format!("`set` {set} is a {}: only an imported component set can be copied", node.kind.type_name())));
    }
    if !node.parent.is_some_and(|parent| is_import_page(doc, parent)) {
        return Err(bad(PLACE, format!("`set` {set} is not in an import page: only an imported component set can be copied")));
    }
    if node.name.trim().is_empty() || node.name.starts_with('-') {
        return Err(bad(PLACE, format!("`set` {set} is named \"{}\": a component needs a name that does not start with a hyphen", node.name)));
    }
    let layers = doc.subtree(set).len();
    if layers > MAX_NODES {
        return Err(bad(PLACE, format!("`set` {set} has {layers} layers: a copy of more than {MAX_NODES} layers is too large")));
    }
    Ok(set)
}

fn target_page(s: &Session, p: &Value) -> Result<NodeId> {
    let doc = s.doc()?;
    let page = match get(p, "page") {
        None => s.page()?,
        Some(value) => id_of(value).ok_or_else(|| bad(PLACE, "`page` must be a page id"))?,
    };
    if !doc.pages.contains(&page) {
        return Err(bad(PLACE, format!("`page` {page} is not a page")));
    }
    if is_import_page(doc, page) {
        return Err(bad(PLACE, format!("`page` {page} is an import page: choose a component page of the library")));
    }
    if !is_component_page(doc, page) {
        return Err(bad(PLACE, format!("`page` {page} is a divider, not a component page")));
    }
    Ok(page)
}

/// The content of `target` moves, with its flows, to a new page right after it that keeps `name`.
fn archive(doc: &mut Document, target: NodeId, name: &str) -> Result<NodeId> {
    let archive = doc.add_page(name);
    if let Some(at) = doc.pages.iter().position(|page| *page == target) {
        doc.pages.retain(|page| *page != archive);
        doc.pages.insert(at + 1, archive);
    }
    if let NodeKind::Page { background, flows, .. } = doc.get(target)?.kind.clone() {
        if let NodeKind::Page { background: archived_background, flows: archived_flows, .. } = &mut doc.get_mut(archive)?.kind {
            *archived_background = background;
            *archived_flows = flows;
        }
        if let NodeKind::Page { flows, .. } = &mut doc.get_mut(target)?.kind {
            flows.clear();
        }
    }
    for layer in doc.get(target)?.children.clone() {
        doc.reparent(layer, archive, None)?;
    }
    Ok(archive)
}

fn place(s: &mut Session, p: &Value) -> Result<Value> {
    binding(s, PLACE)?;
    let set = imported_set(s, p)?;
    let target = target_page(s, p)?;
    let doc = s.doc_mut()?;
    let title = free_component_name_except(doc, &doc.get(set)?.name, Some(target));
    let former = doc.get(target)?.name.clone();
    let occupied = !component_roots(doc, target).is_empty();
    let (x, y) = if occupied { doc.page_content_bounds(target).map_or((0.0, 0.0), |bounds| (bounds.x0, bounds.y0)) } else { (0.0, 0.0) };
    let replaced = if occupied {
        let archive_name =
            if former.trim().to_lowercase() == title.trim().to_lowercase() { free_component_name(doc, &title) } else { former.clone() };
        Some(archive(doc, target, &archive_name)?)
    } else {
        None
    };
    let copy = doc.duplicate_all_into(&[set], target)?.first().copied().ok_or_else(|| bad(PLACE, "the component could not be copied"))?;
    doc.get_mut(copy)?.transform = Affine::translate((x, y));
    doc.get_mut(target)?.name.clone_from(&title);
    let ds = s.ds_mut()?;
    ds.text_edit = None;
    if ds.page != target {
        ds.page = target;
        ds.zoom_to_fit();
    }
    s.set_selection(vec![copy]);
    let replaced = replaced.map(|page| json!({"page": page, "name": former}));
    Ok(json!({"page": target, "set": copy, "name": title, "replaced": replaced}))
}
