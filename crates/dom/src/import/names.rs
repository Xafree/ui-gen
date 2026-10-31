//! Sibling names are made unique and every layer gets its path, so two translations of the same structure line up.

use std::collections::{BTreeMap, BTreeSet};

use figcraft_doc::{Document, Node, NodeId, NodeKind};

use super::translate::{LayerFacts, TranslateOptions};
use crate::model::{DomNode, Element};

const SEPARATOR: &str = " / ";
const SPANS_PARENT: f64 = 1.0;

const BACKGROUND: &str = "Background";
const LABEL: &str = "Label";
const ICON: &str = "Icon";
const CONTENT: &str = "Content";
const SHAPE: &str = "Shape";
const TAG_ROLES: [(&[&str], &str); 7] = [
    (&["button"], "Button"),
    (&["a"], "Link"),
    (&["input", "select", "textarea"], "Input"),
    (&["img", "picture", "video", "canvas"], "Image"),
    (&["ul", "ol", "menu"], "List"),
    (&["li"], "Item"),
    (&["h1", "h2", "h3", "h4", "h5", "h6"], "Heading"),
];

/// The English layer names a translation gives; the interface passes them in its language.
pub const ROLE_NAMES: [&str; 12] = [BACKGROUND, LABEL, ICON, CONTENT, SHAPE, "Button", "Link", "Input", "Image", "List", "Item", "Heading"];

/// A layer is named after what it does, not after the text it holds: a label an element only inherits from its text says nothing, an `aria-label` does.
pub(super) fn by_role(doc: &mut Document, root: NodeId, sources: &BTreeMap<NodeId, &Element>, options: &TranslateOptions) {
    for id in doc.subtree(root).into_iter().filter(|id| *id != root) {
        let Some(role) = role_of(doc, id, sources) else { continue };
        let name = options.role_names.get(role).map_or(role, String::as_str).to_owned();
        if let Some(node) = doc.node_mut(id) {
            node.name = name;
        }
    }
}

fn role_of(doc: &Document, id: NodeId, sources: &BTreeMap<NodeId, &Element>) -> Option<&'static str> {
    let node = doc.node(id)?;
    let Some(element) = sources.get(&id) else {
        let in_element = node.parent.is_some_and(|parent| sources.contains_key(&parent));
        return in_element.then_some(if matches!(node.kind, NodeKind::Text(_)) { LABEL } else { ICON });
    };
    if element.label.as_deref().is_some_and(|label| !label.is_empty() && !repeats_its_text(label, element)) {
        return None;
    }
    let by_tag = TAG_ROLES.iter().find(|(tags, _)| tags.contains(&element.tag.as_str())).map(|(_, role)| *role);
    by_tag.or_else(|| {
        Some(match node.children.as_slice() {
            [] if spans_its_parent(doc, node) => BACKGROUND,
            [] => SHAPE,
            _ => CONTENT,
        })
    })
}

fn spans_its_parent(doc: &Document, node: &Node) -> bool {
    let Some(parent) = node.parent.and_then(|parent| doc.node(parent)) else { return false };
    let [.., x, y] = node.transform.as_coeffs();
    [x, y, parent.width - node.width, parent.height - node.height].into_iter().all(|gap| gap.abs() <= SPANS_PARENT)
}

/// The capture labels an element with its rendered text, shortened and transformed: letters alone tell them apart.
fn repeats_its_text(label: &str, element: &Element) -> bool {
    let mut text = String::new();
    collect_text(&element.children, &mut text);
    let (label, text) = (letters(label.trim_end_matches('…')), letters(&text));
    !label.is_empty() && text.starts_with(&label)
}

fn collect_text(children: &[DomNode], into: &mut String) {
    for child in children {
        match child {
            DomNode::Text(run) => into.push_str(&run.text),
            DomNode::Element(element) => collect_text(&element.children, into),
            DomNode::Svg(_) => {}
        }
    }
}

fn letters(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_lowercase).collect()
}

pub(super) fn make_unique(doc: &mut Document, root: NodeId) {
    for id in doc.subtree(root) {
        let children = doc.node(id).map(|node| node.children.clone()).unwrap_or_default();
        let mut taken = BTreeSet::new();
        for child in children {
            let Some(node) = doc.node_mut(child) else { continue };
            node.name = first_free(&node.name, &taken);
            taken.insert(node.name.clone());
        }
    }
}

fn first_free(name: &str, taken: &BTreeSet<String>) -> String {
    if !taken.contains(name) {
        return name.to_owned();
    }
    (2u32..).map(|number| format!("{name} {number}")).find(|candidate| !taken.contains(candidate)).unwrap_or_else(|| name.to_owned())
}

/// A slash inside a name is escaped so that the separator alone splits a path and two distinct layers never share one.
fn escaped(name: &str) -> String {
    name.replace('\\', "\\\\").replace('/', "\\/")
}

pub(super) fn facts(doc: &Document, root: NodeId) -> Vec<LayerFacts> {
    let mut paths: BTreeMap<NodeId, String> = BTreeMap::new();
    let mut layers = Vec::new();
    for id in doc.subtree(root) {
        let Some(node) = doc.node(id) else { continue };
        let parent_path = node.parent.filter(|_| id != root).and_then(|parent| paths.get(&parent));
        let name = escaped(&node.name);
        let path = parent_path.map_or_else(|| name.clone(), |parent| format!("{parent}{SEPARATOR}{name}"));
        paths.insert(id, path.clone());
        layers.push(LayerFacts { id, path, tokens: Vec::new(), transition: None, effect: None });
    }
    layers
}
