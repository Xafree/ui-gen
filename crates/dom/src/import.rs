//! Snapshot → layers: an element becomes a frame placed relative to its parent, a text run a text
//! layer, an `<svg>` an imported vector layer. A value that cannot be read is left out and named
//! in the warnings; nothing is made up in its place.

mod css;
mod frame;
mod shadow;
mod svg;
mod text;

#[cfg(test)]
mod css_tests;
#[cfg(test)]
mod fidelity_tests;
#[cfg(test)]
mod frame_tests;
#[cfg(test)]
mod shadow_tests;
#[cfg(test)]
mod support;
#[cfg(test)]
mod svg_tests;
#[cfg(test)]
mod text_tests;
#[cfg(test)]
mod walk_tests;

use figcraft_doc::{Document, NodeId};

use crate::error::ImportError;
use crate::model::{DomBox, DomNode, Element, Snapshot};

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
    let mut walk = Walk { doc, warnings: Warnings::default() };
    let root = walk.run(&snapshot.root, parent, (finite_or_zero(x), finite_or_zero(y)))?;
    Ok(Imported { root, warnings: walk.warnings.into_messages() })
}

#[derive(Default)]
struct Warnings {
    messages: Vec<String>,
    left_out: usize,
}

impl Warnings {
    fn push(&mut self, message: String) {
        if self.messages.len() < MAX_WARNINGS {
            self.messages.push(message);
        } else {
            self.left_out += 1;
        }
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
        self.warnings.push(format!("{}: {message}", self.name));
    }
}

fn capped(text: &str) -> String {
    text.chars().take(MAX_NAME_CHARS).collect()
}

fn shortened(value: &str) -> String {
    let mut chars = value.chars();
    let head: String = chars.by_ref().take(MAX_VALUE_CHARS).collect();
    if chars.next().is_some() { format!("{head}…") } else { head }
}

fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

struct Pending<'s> {
    node: &'s DomNode,
    parent: NodeId,
    origin: (f64, f64),
    at: Option<(f64, f64)>,
}

impl Pending<'_> {
    fn offset(&self, dom_box: &DomBox) -> (f64, f64) {
        self.at.unwrap_or((finite_or_zero(dom_box.x - self.origin.0), finite_or_zero(dom_box.y - self.origin.1)))
    }
}

struct Walk<'d> {
    doc: &'d mut Document,
    warnings: Warnings,
}

impl Walk<'_> {
    fn run(&mut self, root: &DomNode, parent: NodeId, at: (f64, f64)) -> Result<NodeId, ImportError> {
        let mut pending = vec![Pending { node: root, parent, origin: (0.0, 0.0), at: Some(at) }];
        let mut root_id = None;
        while let Some(item) = pending.pop() {
            let id = self.visit(&item, &mut pending)?;
            if item.at.is_some() {
                root_id = id;
            }
        }
        root_id.ok_or(ImportError::NothingToImport("the root is hidden or draws nothing"))
    }

    fn visit<'s>(&mut self, item: &Pending<'s>, pending: &mut Vec<Pending<'s>>) -> Result<Option<NodeId>, ImportError> {
        match item.node {
            DomNode::Element(element) => self.element(element, item, pending),
            DomNode::Text(run) => {
                let id = self.doc.alloc_id();
                let node = text::layer(id, run, item.offset(&run.r#box), &mut self.warnings);
                Ok(Some(self.doc.insert(node, item.parent, None)?))
            }
            DomNode::Svg(svg) => {
                let mut report = Report::new("svg", &mut self.warnings);
                Ok(svg::layer(self.doc, svg, item.parent, item.offset(&svg.r#box), &mut report))
            }
        }
    }

    /// A `display: contents` element has no box: its children go straight into its parent.
    fn element<'s>(&mut self, element: &'s Element, item: &Pending<'s>, pending: &mut Vec<Pending<'s>>) -> Result<Option<NodeId>, ImportError> {
        if css::is_hidden(&element.style) {
            return Ok(None);
        }
        let (parent, origin, id) = if item.at.is_none() && css::is_contents(&element.style) {
            (item.parent, item.origin, None)
        } else {
            let node = frame::build(self.doc.alloc_id(), element, item.offset(&element.r#box), &mut self.warnings);
            let id = self.doc.insert(node, item.parent, None)?;
            (id, (element.r#box.x, element.r#box.y), Some(id))
        };
        pending.extend(element.children.iter().rev().map(|child| Pending { node: child, parent, origin, at: None }));
        Ok(id)
    }
}
