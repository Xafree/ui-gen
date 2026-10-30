//! Writing an import into the document: every variant translated from its snapshot, combined in a
//! component set laid out as a grid, and the variable collection of the package, all in one undo step.

use figcraft_doc::{Document, NodeId, NodeKind};
use figcraft_dom::{ImportError, LayerFacts, Snapshot, TranslateOptions};
use figcraft_geom::Affine;
use serde_json::{Value, json};

use super::motion;
use super::pending::{COMMAND, Destination, Request};
use super::plan::State;
use super::staging::{self, SOURCE_KEY};
use super::tokens::{self, Theming, VariantTokens};
use crate::capture::{CaptureError, CaptureErrorKind};
use crate::params::id_of;
use crate::{EngineError, Result, Session, bad};

const GAP: f64 = 20.0;
const LABEL: &str = "Import component";
const EFFECT_KEY: &str = "uigen.effect";
const MAX_WARNINGS: usize = 200;

pub(super) struct Variant {
    pub label: String,
    pub style: Option<String>,
    pub size: Option<String>,
    pub state: State,
    pub row: usize,
    pub column: usize,
    pub snapshot: Snapshot,
    pub others: Vec<Snapshot>,
}

pub(super) struct Finished {
    pub variants: Vec<Variant>,
    pub rows: usize,
    pub columns: usize,
    pub warnings: Vec<String>,
}

fn failed(kind: CaptureErrorKind, message: impl Into<String>) -> CaptureError {
    CaptureError { kind, message: message.into() }
}

/// Write the import into the tab it was started in, whichever tab is in front now. Nothing is
/// written when that tab is gone or the write fails.
pub(super) fn write(s: &mut Session, request: &Request, destination: &Destination, finished: Finished) -> std::result::Result<Value, CaptureError> {
    let Some(tab) = s.docs.iter().position(|ds| ds.uid == destination.uid) else {
        return Err(failed(CaptureErrorKind::InvalidRequest, "the document was closed before the import finished: nothing was imported"));
    };
    let in_front = s.active;
    s.grouped(LABEL, |s| {
        s.active = tab;
        let written = build(s, request, destination, &finished);
        s.active = in_front;
        written
    })
    .map_err(|e: EngineError| failed(CaptureErrorKind::Internal, format!("the layers could not be written: {e}")))
}

struct Target {
    parent: NodeId,
    page: NodeId,
    at: Option<(f64, f64)>,
}

/// Where the set goes. A staged import takes the import page of its package and version, and the
/// place of the set of the same component that page already holds.
fn target(s: &mut Session, request: &Request, destination: &Destination) -> Result<Target> {
    if destination.stage {
        let doc = s.doc_mut()?;
        let page = staging::page_for(doc, request)?;
        let previous = staging::previous_sets(doc, page, request);
        let at = previous.first().and_then(|set| staging::position(doc, *set));
        for set in previous {
            doc.remove(set)?;
        }
        return Ok(Target { parent: page, page, at });
    }
    let parent = destination.parent;
    let allowed = s.doc()?.node(parent).is_some_and(|node| node.is_page() || matches!(node.kind, NodeKind::Frame | NodeKind::Component { .. }));
    if !allowed {
        return Err(bad(COMMAND, "the layer the import was started in is gone or is no longer a frame or a page"));
    }
    Ok(Target { parent, page: destination.page, at: destination.at })
}

fn build(s: &mut Session, request: &Request, destination: &Destination, finished: &Finished) -> Result<Value> {
    let kept_selection = s.ds()?.selection.clone();
    let Target { parent, page, at } = target(s, request, destination)?;
    figcraft_text::scan_system_fonts();
    let (x, y) = origin(s.doc()?, parent, page, at);
    let (collection, added_modes) = collection(s, request)?;
    let mut warnings: Vec<String> = finished.warnings.clone();
    let mut placed: Vec<(NodeId, usize, usize)> = Vec::with_capacity(finished.variants.len());
    let mut themed: Vec<VariantTokens> = Vec::with_capacity(finished.variants.len());
    for variant in &finished.variants {
        let mut translated = figcraft_dom::translate_snapshot(s.doc_mut()?, &variant.snapshot, parent, 0.0, 0.0, &TranslateOptions::default())
            .map_err(|e| bad(COMMAND, format!("{}: {e}", variant.label)))?;
        if !request.animations {
            drop_effects(s.doc_mut()?, &mut translated.layers);
        }
        for warning in translated.warnings {
            if warnings.len() < MAX_WARNINGS && !warnings.contains(&warning) {
                warnings.push(warning);
            }
        }
        let others = variant
            .others
            .iter()
            .map(scratch_facts)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| bad(COMMAND, format!("{}: {e}", variant.label)))?;
        let axes = [variant.style.clone(), variant.size.clone(), Some(variant.state.label().to_lowercase())];
        themed.push(VariantTokens { axes, layers: translated.layers, others });
        let made = s.execute("component.create", &json!({"ids": [translated.root]}))?;
        let id = id_of(&made["id"]).ok_or_else(|| bad(COMMAND, format!("{}: the variant could not become a component", variant.label)))?;
        s.doc_mut()?.get_mut(id)?.name.clone_from(&variant.label);
        placed.push((id, variant.row, variant.column));
    }
    let ids: Vec<NodeId> = placed.iter().map(|(id, ..)| *id).collect();
    let combined = s.execute("component.combineVariants", &json!({"ids": ids}))?;
    let set = id_of(&combined["id"]).ok_or_else(|| bad(COMMAND, "the variants could not be combined into a component set"))?;
    lay_out(s.doc_mut()?, set, &placed, (finished.rows, finished.columns), (x, y))?;
    let node = s.doc_mut()?.get_mut(set)?;
    node.name.clone_from(&request.component);
    let source = json!({"package": request.package, "version": request.version, "component": request.component});
    node.plugin_data.insert(SOURCE_KEY.to_string(), source.to_string());
    for warning in tokens::apply(
        s,
        &Theming { collection: &collection, component: &request.component, added_modes: &added_modes, themes: &request.themes },
        &themed,
    )? {
        if warnings.len() < MAX_WARNINGS && !warnings.contains(&warning) {
            warnings.push(warning);
        }
    }
    let linked: Vec<motion::Variant> = finished
        .variants
        .iter()
        .zip(&placed)
        .zip(&themed)
        .map(|((variant, (id, ..)), themed)| motion::Variant {
            id: *id,
            style: variant.style.as_deref(),
            size: variant.size.as_deref(),
            state: variant.state,
            layers: &themed.layers,
        })
        .collect();
    if request.animations {
        motion::apply(s.doc_mut()?, &linked)?;
    }
    let mut result = json!({"set": set, "collection": collection, "variants": placed.len(), "warnings": warnings});
    if destination.stage {
        result["page"] = json!(parent);
        s.set_selection(kept_selection);
    } else {
        s.set_selection(vec![set]);
    }
    Ok(result)
}

fn drop_effects(doc: &mut Document, layers: &mut [LayerFacts]) {
    for layer in layers {
        layer.effect = None;
        if let Some(node) = doc.node_mut(layer.id) {
            node.plugin_data.remove(EFFECT_KEY);
        }
    }
}

/// The layer facts of another theme's render, translated in a scratch document: the document keeps only the layers of the first theme.
fn scratch_facts(snapshot: &Snapshot) -> std::result::Result<Vec<LayerFacts>, ImportError> {
    let mut scratch = Document::new("scratch");
    let page = scratch.pages.first().copied().ok_or(ImportError::NothingToImport("the scratch document has no page"))?;
    figcraft_dom::translate_snapshot(&mut scratch, snapshot, page, 0.0, 0.0, &TranslateOptions::default()).map(|translated| translated.layers)
}

fn origin(doc: &Document, parent: NodeId, page: NodeId, at: Option<(f64, f64)>) -> (f64, f64) {
    match at {
        Some(at) => at,
        None if parent == page => doc.page_content_bounds(page).map_or((0.0, 0.0), |bounds| (bounds.x1 + 4.0 * GAP, bounds.y0)),
        None => (0.0, 0.0),
    }
}

/// The variable collection of the package at this version and the ids of the modes this import had to
/// add to it: the collection the document has, with a mode added for each theme it lacks, else a new
/// one with a mode per theme.
fn collection(s: &mut Session, request: &Request) -> Result<(String, Vec<String>)> {
    let name = format!("{} {}", request.package, request.version);
    let existing = s.doc()?.variables.collections.iter().find(|collection| collection.name == name).cloned();
    let Some(existing) = existing else {
        let modes: Vec<&str> = request.themes.iter().map(|theme| theme.mode()).collect();
        let made = s.execute("variable.createCollection", &json!({"name": name, "modes": modes}))?;
        let id = made["id"].as_str().map(str::to_string).ok_or_else(|| bad(COMMAND, "the variable collection could not be created"))?;
        return Ok((id, Vec::new()));
    };
    let mut added = Vec::new();
    for theme in &request.themes {
        if existing.modes.iter().any(|mode| mode.name == theme.mode()) {
            continue;
        }
        let made = s.execute("variable.addMode", &json!({"collection": existing.id, "name": theme.mode()}))?;
        added.push(made["id"].as_str().map(str::to_string).ok_or_else(|| bad(COMMAND, "the variable collection could not get its mode"))?);
    }
    Ok((existing.id, added))
}

fn finite(size: f64) -> f64 {
    if size.is_finite() { size.max(0.0) } else { 0.0 }
}

/// Starts of the cells along one axis, then the total: a gap before each cell and after the last.
fn starts(sizes: &[f64]) -> Vec<f64> {
    let mut starts = vec![GAP];
    for size in sizes {
        starts.push(starts.last().copied().unwrap_or(GAP) + size + GAP);
    }
    starts
}

/// Columns are the widest variant of their column, rows the tallest of their row; each variant sits
/// at the top left of its cell and the set wraps them all.
fn lay_out(doc: &mut Document, set: NodeId, placed: &[(NodeId, usize, usize)], (rows, columns): (usize, usize), (x, y): (f64, f64)) -> Result<()> {
    let mut widths = vec![0.0_f64; columns];
    let mut heights = vec![0.0_f64; rows];
    for (id, row, column) in placed {
        let node = doc.get(*id)?;
        if let Some(width) = widths.get_mut(*column) {
            *width = width.max(finite(node.width));
        }
        if let Some(height) = heights.get_mut(*row) {
            *height = height.max(finite(node.height));
        }
    }
    let (xs, ys) = (starts(&widths), starts(&heights));
    for (id, row, column) in placed {
        let (Some(left), Some(top)) = (xs.get(*column), ys.get(*row)) else { continue };
        doc.get_mut(*id)?.transform = Affine::translate((*left, *top));
    }
    let node = doc.get_mut(set)?;
    node.width = xs.last().copied().unwrap_or(GAP);
    node.height = ys.last().copied().unwrap_or(GAP);
    node.transform = Affine::translate((x, y));
    Ok(())
}
