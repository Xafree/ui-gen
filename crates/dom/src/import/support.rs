use figcraft_doc::{Document, Node, NodeId};
use serde_json::{Value, json};

use crate::{Snapshot, import_snapshot, parse_snapshot};

pub(super) fn snapshot(root: Value) -> Snapshot {
    let json = json!({ "version": 1, "viewport": { "width": 400, "height": 200, "deviceScaleFactor": 1 }, "root": root });
    parse_snapshot(&json.to_string()).unwrap()
}

pub(super) struct Run {
    pub doc: Document,
    pub root: NodeId,
    pub warnings: Vec<String>,
}

impl Run {
    pub fn node(&self, id: NodeId) -> &Node {
        self.doc.get(id).unwrap()
    }

    pub fn root_node(&self) -> &Node {
        self.node(self.root)
    }

    pub fn child(&self, index: usize) -> &Node {
        self.node(*self.root_node().children.get(index).unwrap())
    }
}

pub(super) fn import(root: Value) -> Run {
    let snapshot = snapshot(root);
    let mut doc = Document::new("t");
    let page = doc.first_page().unwrap();
    let imported = import_snapshot(&mut doc, &snapshot, page, 100.0, 50.0).unwrap();
    Run { doc, root: imported.root, warnings: imported.warnings }
}

pub(super) fn bounds(x: f64, y: f64, width: f64, height: f64) -> Value {
    json!({ "x": x, "y": y, "width": width, "height": height })
}

pub(super) fn element_with(style: Value, children: Value) -> Value {
    json!({ "type": "element", "tag": "button", "label": "Bouton", "box": bounds(20.0, 20.0, 92.0, 36.0), "style": style, "children": children })
}

pub(super) fn frame(style: Value) -> Run {
    import(element_with(style, json!([])))
}

pub(super) fn text_run(style: Value) -> Run {
    let text = json!({ "type": "text", "text": "Bouton", "box": bounds(30.0, 26.0, 50.0, 20.0), "style": style });
    import(element_with(json!({}), json!([text])))
}

pub(super) fn mentions(warnings: &[String], name: &str, property: &str) -> bool {
    warnings.iter().any(|warning| warning.starts_with(&format!("{name}: {property} ")))
}
