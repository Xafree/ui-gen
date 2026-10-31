//! An `<svg>` becomes an imported vector layer (usvg), stretched to the box the page gave it.

use figcraft_doc::geom::Affine;
use figcraft_doc::{Document, NodeId};

use super::{Placement, Report};
use crate::model::SvgNode;

const SAME_SIZE: f64 = 1e-3;

pub(super) fn layer(doc: &mut Document, svg: &SvgNode, parent: NodeId, placement: &Placement, report: &mut Report) -> Option<NodeId> {
    let (width, height) = placement.size();
    let offset = placement.offset;
    if !(width > 0.0 && height > 0.0) {
        return None;
    }
    match figcraft_svg::import_svg_with_notes(doc, svg.markup.as_bytes(), parent, offset.0, offset.1, "svg") {
        Ok(imported) => {
            for note in &imported.notes {
                report.note(note);
            }
            stretch(doc, imported.frame, offset, (width, height));
            rotate(doc, imported.frame, svg.rotation);
            Some(imported.frame)
        }
        Err(error) => {
            report.unreadable("markup", &error.to_string());
            None
        }
    }
}

/// A transformed icon turns about the centre of its box, which keeps its place for the quarter turns icons use.
fn rotate(doc: &mut Document, frame: NodeId, degrees: f64) {
    let Some(node) = doc.node_mut(frame).filter(|_| degrees.is_finite() && degrees != 0.0) else { return };
    let center = (node.width / 2.0, node.height / 2.0);
    node.transform = node.transform * Affine::translate(center) * Affine::rotate(degrees.to_radians()) * Affine::translate((-center.0, -center.1));
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
