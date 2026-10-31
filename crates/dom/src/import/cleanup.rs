//! Elements that only wrap or draw nothing are dropped: the layer tree keeps what a designer would draw.

use std::collections::BTreeMap;

use figcraft_doc::{DocResult, Document, Node, NodeId, NodeKind};

use super::css::keyword;
use crate::model::Element;

const SAME_BOX: f64 = 1.0;

enum Fate {
    Keep,
    Drop,
    Dissolve(NodeId),
}

/// Returns the root of what remains: a root that only wraps one frame of its own box gives way to it.
pub(super) fn run(doc: &mut Document, root: NodeId, sources: &BTreeMap<NodeId, &Element>) -> DocResult<NodeId> {
    for id in doc.subtree(root).into_iter().rev().filter(|id| *id != root) {
        let Some(element) = sources.get(&id) else { continue };
        match fate(doc, id, element, sources) {
            Fate::Keep => {}
            Fate::Drop => doc.remove(id)?,
            Fate::Dissolve(child) => dissolve(doc, id, child)?,
        }
    }
    let Some(element) = sources.get(&root) else { return Ok(root) };
    if let Fate::Dissolve(child) = fate(doc, root, element, sources)
        && doc.node(child).is_some_and(|node| matches!(node.kind, NodeKind::Frame))
        && doc.node(root).zip(doc.node(child)).is_some_and(|(wrapper, only)| fills_box_of(wrapper, only))
    {
        dissolve(doc, root, child)?;
        return Ok(child);
    }
    Ok(root)
}

fn fate(doc: &Document, id: NodeId, element: &Element, sources: &BTreeMap<NodeId, &Element>) -> Fate {
    let Some(node) = doc.node(id) else { return Fate::Keep };
    if is_ripple(element) || !element.animations.is_empty() || !node.annotations.is_empty() || shows_something(node) {
        return Fate::Keep;
    }
    match node.children.as_slice() {
        [] => Fate::Drop,
        [only] if doc.node(*only).is_some_and(|child| fills_box_of(node, child)) || (!lays_out(element) && !parent_lays_out(node, sources)) => {
            Fate::Dissolve(*only)
        }
        _ => Fate::Keep,
    }
}

pub(super) fn is_ripple(element: &Element) -> bool {
    element.class_name.as_deref().is_some_and(|class_name| class_name.to_ascii_lowercase().contains("ripple"))
}

fn lays_out(element: &Element) -> bool {
    keyword(&element.style, "display").is_some_and(|display| display.contains("flex") || display.contains("grid"))
}

fn parent_lays_out(node: &Node, sources: &BTreeMap<NodeId, &Element>) -> bool {
    node.parent.and_then(|parent| sources.get(&parent)).is_some_and(|parent| lays_out(parent))
}

fn shows_something(node: &Node) -> bool {
    !node.fills.is_empty() || !node.strokes.is_empty() || !node.effects.is_empty() || node.clip || node.opacity < 1.0
}

fn fills_box_of(wrapper: &Node, child: &Node) -> bool {
    let [a, b, c, d, e, f] = child.transform.as_coeffs();
    let at_origin = [a - 1.0, b, c, d - 1.0].into_iter().all(|value| value.abs() <= 1e-6) && e.abs() <= SAME_BOX && f.abs() <= SAME_BOX;
    at_origin && (wrapper.width - child.width).abs() <= SAME_BOX && (wrapper.height - child.height).abs() <= SAME_BOX
}

fn dissolve(doc: &mut Document, wrapper: NodeId, child: NodeId) -> DocResult<()> {
    let (Some(parent), Some(index)) = (doc.node(wrapper).and_then(|node| node.parent), doc.index_in_parent(wrapper)) else { return Ok(()) };
    doc.reparent(child, parent, Some(index))?;
    doc.remove(wrapper)
}
