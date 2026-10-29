//! Elements that only wrap or draw nothing are dropped: the layer tree keeps what a designer would draw.

use std::collections::BTreeMap;

use figcraft_doc::{DocResult, Document, Node, NodeId};

use crate::model::Element;

const SAME_BOX: f64 = 1e-6;

enum Fate {
    Keep,
    Drop,
    Dissolve(NodeId),
}

pub(super) fn run(doc: &mut Document, root: NodeId, sources: &BTreeMap<NodeId, &Element>) -> DocResult<()> {
    for id in doc.subtree(root).into_iter().rev().filter(|id| *id != root) {
        let Some(element) = sources.get(&id) else { continue };
        match fate(doc, id, element) {
            Fate::Keep => {}
            Fate::Drop => doc.remove(id)?,
            Fate::Dissolve(child) => dissolve(doc, id, child)?,
        }
    }
    Ok(())
}

fn fate(doc: &Document, id: NodeId, element: &Element) -> Fate {
    let Some(node) = doc.node(id) else { return Fate::Keep };
    if is_ripple(element) || !element.animations.is_empty() || !node.annotations.is_empty() || shows_something(node) {
        return Fate::Keep;
    }
    match node.children.as_slice() {
        [] => Fate::Drop,
        [only] if doc.node(*only).is_some_and(|child| fills_box_of(node, child)) => Fate::Dissolve(*only),
        _ => Fate::Keep,
    }
}

pub(super) fn is_ripple(element: &Element) -> bool {
    element.class_name.as_deref().is_some_and(|class_name| class_name.to_ascii_lowercase().contains("ripple"))
}

fn shows_something(node: &Node) -> bool {
    !node.fills.is_empty() || !node.strokes.is_empty() || !node.effects.is_empty() || node.clip || node.opacity < 1.0
}

fn fills_box_of(wrapper: &Node, child: &Node) -> bool {
    let [a, b, c, d, e, f] = child.transform.as_coeffs();
    let at_origin = [a - 1.0, b, c, d - 1.0, e, f].into_iter().all(|value| value.abs() <= SAME_BOX);
    at_origin && (wrapper.width - child.width).abs() <= SAME_BOX && (wrapper.height - child.height).abs() <= SAME_BOX
}

fn dissolve(doc: &mut Document, wrapper: NodeId, child: NodeId) -> DocResult<()> {
    let (Some(parent), Some(index)) = (doc.node(wrapper).and_then(|node| node.parent), doc.index_in_parent(wrapper)) else { return Ok(()) };
    doc.reparent(child, parent, Some(index))?;
    doc.remove(wrapper)
}
