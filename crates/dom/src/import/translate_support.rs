use figcraft_doc::{Document, Node, NodeId};
use serde_json::Value;

use crate::{Snapshot, TranslateOptions, Translated, parse_snapshot, translate_snapshot};

pub(super) const FLEX_ROW: &str = include_str!("../../fixtures/flex-row.json");
pub(super) const FLEX_COLUMN: &str = include_str!("../../fixtures/flex-column.json");
pub(super) const WRAPPER: &str = include_str!("../../fixtures/wrapper.json");

pub(super) const NOTE_PREFIX: &str = "UIGEN import: ";

pub(super) struct Translation {
    pub doc: Document,
    pub page: NodeId,
    pub translated: Translated,
}

impl Translation {
    pub fn node(&self, id: NodeId) -> &Node {
        self.doc.get(id).unwrap()
    }

    pub fn root(&self) -> &Node {
        self.node(self.translated.root)
    }

    pub fn children_of(&self, node: &Node) -> Vec<&Node> {
        node.children.iter().map(|id| self.node(*id)).collect()
    }

    pub fn child_names(&self, node: &Node) -> Vec<String> {
        self.children_of(node).into_iter().map(|child| child.name.clone()).collect()
    }

    pub fn by_name(&self, name: &str) -> &Node {
        let id = self.doc.subtree(self.translated.root).into_iter().find(|id| self.node(*id).name == name);
        self.node(id.unwrap_or_else(|| panic!("no layer named {name:?}")))
    }

    pub fn note_labels(&self, node: &Node) -> Vec<String> {
        node.annotations.iter().map(|annotation| annotation.label.clone()).filter(|label| label.starts_with(NOTE_PREFIX)).collect()
    }

    pub fn all_note_labels(&self) -> Vec<String> {
        let ids = self.doc.subtree(self.translated.root);
        ids.into_iter().flat_map(|id| self.note_labels(self.node(id))).collect()
    }

    pub fn lay_out(&mut self) {
        figcraft_layout::layout_page(&mut self.doc, self.page, &figcraft_layout::NoMeasure);
    }
}

pub(super) fn parse(json: &str) -> Snapshot {
    parse_snapshot(json).unwrap()
}

pub(super) fn translate_with(snapshot: &Snapshot, options: &TranslateOptions) -> Translation {
    let mut doc = Document::new("t");
    let page = doc.first_page().unwrap();
    let translated = translate_snapshot(&mut doc, snapshot, page, 100.0, 50.0, options).unwrap();
    Translation { doc, page, translated }
}

pub(super) fn translate(snapshot: &Snapshot) -> Translation {
    translate_with(snapshot, &TranslateOptions::default())
}

pub(super) fn translate_root(root: Value) -> Translation {
    translate(&super::support::snapshot(root))
}

pub(super) fn offset(node: &Node) -> (f64, f64) {
    let coefficients = node.transform.as_coeffs();
    (coefficients[4], coefficients[5])
}
