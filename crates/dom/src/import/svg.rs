//! An `<svg>` becomes an imported vector layer (usvg), stretched to the box the page gave it.

use figcraft_doc::geom::Affine;
use figcraft_doc::{Document, NodeId};

use super::Report;
use crate::model::SvgNode;

const SAME_SIZE: f64 = 1e-3;

pub(super) fn layer(doc: &mut Document, svg: &SvgNode, parent: NodeId, offset: (f64, f64), report: &mut Report) -> Option<NodeId> {
    let (width, height) = (svg.r#box.width, svg.r#box.height);
    if !(width > 0.0 && height > 0.0) {
        return None;
    }
    match figcraft_svg::import_svg_with_notes(doc, svg.markup.as_bytes(), parent, offset.0, offset.1, "svg") {
        Ok(imported) => {
            for note in &imported.notes {
                report.note(note);
            }
            stretch(doc, imported.frame, offset, (width, height));
            Some(imported.frame)
        }
        Err(error) => {
            report.unreadable("markup", &error.to_string());
            None
        }
    }
}

/// The browser scales an icon, strokes included, to the box the page gave it.
fn stretch(doc: &mut Document, frame: NodeId, offset: (f64, f64), (width, height): (f64, f64)) {
    let Some(node) = doc.node_mut(frame) else { return };
    let (own_width, own_height) = (node.width, node.height);
    let differs = (own_width - width).abs() > SAME_SIZE || (own_height - height).abs() > SAME_SIZE;
    if own_width > 0.0 && own_height > 0.0 && differs {
        node.transform = Affine::translate(offset) * Affine::scale_non_uniform(width / own_width, height / own_height);
    }
}
