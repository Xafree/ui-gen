//! `uigen.importDom` without `full` keeps the lot 0 translation; `full: true` is the complete one.
//! The snapshot is the synthetic flex row of the DOM crate.

use crate::Session;
use crate::params::id_of;
use figcraft_doc::{Document, LayoutMode, NodeId};
use serde_json::{Value, json};

const FLEX_ROW: &str = include_str!("../../dom/fixtures/flex-row.json");

fn outline(doc: &Document, id: NodeId) -> Vec<String> {
    doc.subtree(id)
        .into_iter()
        .map(|node_id| {
            let node = doc.node(node_id).unwrap();
            let [a, b, c, d, e, f] = node.transform.as_coeffs();
            format!(
                "{} {:?} {}x{} [{a} {b} {c} {d} {e} {f}] fills={} auto_layout={} notes={}",
                node.name,
                node.kind,
                node.width,
                node.height,
                node.fills.len(),
                node.layout.is_some(),
                node.annotations.len()
            )
        })
        .collect()
}

fn import_with(params: &Value) -> (Session, NodeId) {
    let mut session = Session::new();
    let result = session.execute("uigen.importDom", params).unwrap();
    let root = id_of(&result["root"]).unwrap_or_else(|| panic!("no `root` id in {result}"));
    (session, root)
}

// covers: E3-3.6
#[test]
fn without_full_the_tree_is_exactly_the_one_of_the_lot_0_import() {
    let snapshot = figcraft_dom::parse_snapshot(FLEX_ROW).unwrap();
    let mut reference = Document::new("t");
    let page = reference.first_page().unwrap();
    let imported = figcraft_dom::import_snapshot(&mut reference, &snapshot, page, 100.0, 50.0).unwrap();

    let (session, root) = import_with(&json!({ "snapshot": FLEX_ROW, "x": 100, "y": 50 }));

    let expected = outline(&reference, imported.root);
    assert_eq!(outline(session.doc().unwrap(), root), expected);
    assert!(expected.iter().all(|line| line.contains("auto_layout=false") && line.contains("notes=0")), "{expected:?}");
}

// covers: E3-3.6
#[test]
fn with_full_the_flex_row_gets_its_auto_layout() {
    let (session, root) = import_with(&json!({ "snapshot": FLEX_ROW, "full": true }));

    let layout = session.doc().unwrap().node(root).unwrap().layout.clone().expect("the full translation reads the flex properties");
    assert_eq!(layout.mode, LayoutMode::Horizontal);
    assert_eq!(layout.gap, 12.0);
}
