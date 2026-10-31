//! Snapshot → layers: an element becomes a frame placed relative to its parent, a text run a text
//! layer, an `<svg>` an imported vector layer. A value that cannot be read is left out and named
//! in the warnings; nothing is made up in its place.

mod baseline;
mod cleanup;
mod css;
mod effects;
mod faces;
mod facts;
mod fonts;
mod frame;
mod layout;
mod motion;
mod names;
mod notes;
mod paints;
mod shadow;
mod stacking;
mod svg;
mod text;
mod translate;

#[cfg(test)]
mod cleanup_tests;
#[cfg(test)]
mod css_tests;
#[cfg(test)]
mod faces_tests;
#[cfg(test)]
mod facts_tests;
#[cfg(test)]
mod fidelity_tests;
#[cfg(test)]
mod frame_tests;
#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod motion_tests;
#[cfg(test)]
mod notes_tests;
#[cfg(test)]
mod review_tests;
#[cfg(test)]
mod shadow_tests;
#[cfg(test)]
mod support;
#[cfg(test)]
mod svg_tests;
#[cfg(test)]
mod text_tests;
#[cfg(test)]
mod translate_support;
#[cfg(test)]
mod walk_tests;

use figcraft_doc::{Annotation, Document, NodeId};

use crate::error::ImportError;
use crate::model::{DomBox, DomNode, Element, Snapshot, SvgNode};

pub use faces::{TextFace, text_faces};
pub use fonts::nearest_weight;
pub use motion::css_easing;
pub use names::ROLE_NAMES;
pub use notes::NOTE_PREFIX;
pub use translate::{Effect, LayerFacts, TokenFact, TokenValue, TransitionFact, TranslateOptions, Translated, translate_snapshot};

const MAX_WARNINGS: usize = 200;
const MAX_NAME_CHARS: usize = 128;
const MAX_VALUE_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    pub root: NodeId,
    pub warnings: Vec<String>,
}

/// Import `snapshot` under `parent`, its root frame at `(x, y)`; what could not be represented is listed in the warnings.
pub fn import_snapshot(doc: &mut Document, snapshot: &Snapshot, parent: NodeId, x: f64, y: f64) -> Result<Imported, ImportError> {
    if doc.node(parent).is_none() {
        return Err(ImportError::ParentNotFound(parent));
    }
    let mut walk = Walk::new(doc, false);
    let root = walk.run(&snapshot.root, parent, (finite_or_zero(x), finite_or_zero(y)))?;
    Ok(Imported { root, warnings: walk.warnings.into_messages() })
}

/// Where the root layer of `snapshot` falls in a capture cut on the rounded edges of its box, top-left corner first:
/// nowhere but the corner for a box on the pixel grid, at a fraction of a pixel for a box the browser scaled.
pub fn capture_phase(snapshot: &Snapshot) -> [f64; 2] {
    match &snapshot.root {
        DomNode::Element(element) if effects::is_scaled(&element.style) => {
            let exact = Snapped::exact(&element.r#box);
            [exact.x - exact.x.round(), exact.y - exact.y.round()]
        }
        _ => [0.0, 0.0],
    }
}

#[derive(Default)]
struct Warnings {
    messages: Vec<String>,
    left_out: usize,
    fresh: Vec<String>,
}

impl Warnings {
    fn push(&mut self, name: &str, message: &str) {
        if self.messages.len() < MAX_WARNINGS {
            self.messages.push(format!("{name}: {message}"));
        } else {
            self.left_out += 1;
        }
        self.fresh.push(message.to_owned());
    }

    fn take_fresh(&mut self) -> Vec<String> {
        std::mem::take(&mut self.fresh)
    }

    fn into_messages(mut self) -> Vec<String> {
        if self.left_out > 0 {
            self.messages.push(format!("{} more warnings were left out", self.left_out));
        }
        self.messages
    }
}

struct Report<'a> {
    name: &'a str,
    warnings: &'a mut Warnings,
}

impl<'a> Report<'a> {
    fn new(name: &'a str, warnings: &'a mut Warnings) -> Self {
        Self { name, warnings }
    }

    fn unreadable(&mut self, property: &str, value: &str) {
        self.note(&format!("{property} {}", shortened(value)));
    }

    fn note(&mut self, message: &str) {
        self.warnings.push(self.name, message);
    }
}

fn capped(text: &str) -> String {
    text.chars().take(MAX_NAME_CHARS).collect()
}

fn shortened(value: &str) -> String {
    crate::error::capped(value, MAX_VALUE_CHARS)
}

fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

/// A box on the pixel grid, as Chromium paints it at a device scale factor of 1: its edges are rounded, its size is the distance between them.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Snapped {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Snapped {
    /// A scaled element is not on the pixel grid: the browser paints its edges where they fall.
    fn exact(dom_box: &DomBox) -> Self {
        let [x, y] = [dom_box.x, dom_box.y].map(finite_or_zero);
        Self { x, y, width: finite_or_zero(dom_box.width).max(0.0), height: finite_or_zero(dom_box.height).max(0.0) }
    }

    fn of(dom_box: &DomBox) -> Self {
        let (x, y) = (finite_or_zero(dom_box.x.round()), finite_or_zero(dom_box.y.round()));
        let right = finite_or_zero((dom_box.x + dom_box.width).round());
        let bottom = finite_or_zero((dom_box.y + dom_box.height).round());
        Self { x, y, width: (right - x).max(0.0), height: (bottom - y).max(0.0) }
    }

    fn size(&self) -> (f64, f64) {
        (self.width, self.height)
    }
}

/// Where a node goes: its offset in its parent and its snapped size.
struct Placement {
    offset: (f64, f64),
    snapped: Snapped,
    on_grid: bool,
}

impl Placement {
    fn size(&self) -> (f64, f64) {
        self.snapped.size()
    }
}

struct Pending<'s> {
    node: &'s DomNode,
    parent: NodeId,
    origin: (f64, f64),
    at: Option<(f64, f64)>,
    under_scale: bool,
}

impl Pending<'_> {
    fn place(&self, dom_box: &DomBox) -> Placement {
        if self.under_scale { self.placed(Snapped::exact(dom_box), false) } else { self.placed(Snapped::of(dom_box), true) }
    }

    fn placed(&self, snapped: Snapped, on_grid: bool) -> Placement {
        let offset = self.at.unwrap_or((snapped.x - self.origin.0, snapped.y - self.origin.1));
        Placement { offset, snapped, on_grid }
    }
}

struct Walk<'d, 's> {
    doc: &'d mut Document,
    warnings: Warnings,
    annotate: bool,
    elements: Vec<(NodeId, &'s Element)>,
    svgs: Vec<(NodeId, &'s SvgNode)>,
}

impl<'d, 's> Walk<'d, 's> {
    fn new(doc: &'d mut Document, annotate: bool) -> Self {
        Self { doc, warnings: Warnings::default(), annotate, elements: Vec::new(), svgs: Vec::new() }
    }

    fn take_notes(&mut self) -> Vec<Annotation> {
        let fresh = self.warnings.take_fresh();
        if self.annotate { notes::annotations(fresh) } else { Vec::new() }
    }

    fn run(&mut self, root: &'s DomNode, parent: NodeId, at: (f64, f64)) -> Result<NodeId, ImportError> {
        let mut pending = vec![Pending { node: root, parent, origin: (0.0, 0.0), at: Some(at), under_scale: false }];
        let mut root_id = None;
        while let Some(item) = pending.pop() {
            let id = self.visit(&item, &mut pending)?;
            if item.at.is_some() {
                root_id = id;
            }
        }
        root_id.ok_or(ImportError::NothingToImport("the root is hidden or draws nothing"))
    }

    fn visit(&mut self, item: &Pending<'s>, pending: &mut Vec<Pending<'s>>) -> Result<Option<NodeId>, ImportError> {
        match item.node {
            DomNode::Element(element) => self.element(element, item, pending),
            DomNode::Text(run) => {
                let id = self.doc.alloc_id();
                let placement = item.place(baseline::line_box(run).as_ref().unwrap_or(&run.r#box));
                let mut node = text::layer(id, run, &placement, &mut self.warnings);
                node.annotations = self.take_notes();
                Ok(Some(self.doc.insert(node, item.parent, None)?))
            }
            DomNode::Svg(svg) => {
                let mut report = Report::new("svg", &mut self.warnings);
                let id = svg::layer(self.doc, svg, item.parent, &item.place(&svg.r#box), &mut report);
                let notes = self.take_notes();
                if let Some(node) = id.and_then(|id| self.doc.node_mut(id)) {
                    node.annotations = notes;
                }
                self.svgs.extend(id.map(|id| (id, svg)));
                Ok(id)
            }
        }
    }

    /// A `display: contents` element has no box: its children go straight into its parent.
    fn element(&mut self, element: &'s Element, item: &Pending<'s>, pending: &mut Vec<Pending<'s>>) -> Result<Option<NodeId>, ImportError> {
        if css::is_hidden(&element.style) {
            return Ok(None);
        }
        let under_scale = item.under_scale || effects::is_scaled(&element.style);
        let (parent, origin, id) = if item.at.is_none() && css::is_contents(&element.style) {
            (item.parent, item.origin, None)
        } else {
            let placement =
                if effects::is_scaled(&element.style) { item.placed(Snapped::exact(&element.r#box), false) } else { item.place(&element.r#box) };
            let mut node = frame::build(self.doc.alloc_id(), element, &placement, &mut self.warnings);
            node.annotations = self.take_notes();
            let id = self.doc.insert(node, item.parent, None)?;
            self.elements.push((id, element));
            (id, (placement.snapped.x, placement.snapped.y), Some(id))
        };
        pending.extend(stacking::painting_order(&element.children).into_iter().rev().map(|child| Pending {
            node: child,
            parent,
            origin,
            at: None,
            under_scale,
        }));
        Ok(id)
    }
}
