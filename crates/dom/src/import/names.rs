//! Sibling names are made unique and every layer gets its path, so two translations of the same structure line up.

use std::collections::{BTreeMap, BTreeSet};

use figcraft_doc::{Document, NodeId};

use super::translate::LayerFacts;

const SEPARATOR: &str = " / ";

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

pub(super) fn facts(doc: &Document, root: NodeId) -> Vec<LayerFacts> {
    let mut paths: BTreeMap<NodeId, String> = BTreeMap::new();
    let mut layers = Vec::new();
    for id in doc.subtree(root) {
        let Some(node) = doc.node(id) else { continue };
        let parent_path = node.parent.filter(|_| id != root).and_then(|parent| paths.get(&parent));
        let path = parent_path.map_or_else(|| node.name.clone(), |parent| format!("{parent}{SEPARATOR}{}", node.name));
        paths.insert(id, path.clone());
        layers.push(LayerFacts { id, path, tokens: Vec::new(), transition: None, effect: None });
    }
    layers
}
