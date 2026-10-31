use figcraft_doc::{Document, NodeId, NodeKind};
use serde_json::{Value, json};

use super::support::{Run, bounds, element_with, import, snapshot};
use crate::{DomBox, DomNode, Element, ImportError, Snapshot, import_snapshot};

fn span(x: f64, y: f64) -> Value {
    json!({ "type": "element", "tag": "span", "box": bounds(x, y, 4.0, 4.0) })
}

fn offset(run: &Run, index: usize) -> (f64, f64) {
    let coefficients = run.child(index).transform.as_coeffs();
    (coefficients[4], coefficients[5])
}

// covers: T1
#[test]
fn siblings_keep_their_order_and_each_nesting_level_is_relative_to_its_own_parent() {
    let grandchild = json!({ "type": "element", "tag": "i", "box": bounds(33.0, 29.0, 1.0, 1.0) });
    let nested = json!({ "type": "element", "tag": "b", "box": bounds(30.0, 26.0, 10.0, 10.0), "children": [grandchild] });
    let run = import(element_with(json!({}), json!([span(21.0, 22.0), nested, span(25.0, 24.0)])));
    let names: Vec<&str> = run.root_node().children.iter().map(|id| run.node(*id).name.as_str()).collect();
    assert_eq!(names, ["span", "b", "span"]);
    assert_eq!(offset(&run, 0), (1.0, 2.0));
    assert_eq!(offset(&run, 1), (10.0, 6.0));
    let inner = run.node(run.child(1).children[0]);
    assert_eq!(inner.transform.as_coeffs()[4..], [3.0, 3.0]);
}

// covers: T6
#[test]
fn a_hidden_element_takes_its_whole_subtree_with_it() {
    let hidden = json!({ "type": "element", "tag": "div", "box": bounds(20.0, 20.0, 5.0, 5.0), "style": { "display": "none" }, "children": [span(21.0, 21.0)] });
    let run = import(element_with(json!({}), json!([hidden, span(22.0, 22.0)])));
    assert_eq!(run.root_node().children.len(), 1);
    assert_eq!(run.doc.node_count(), 3, "page, root and the one span");
}

// covers: T1
#[test]
fn display_contents_has_no_frame_and_its_children_go_to_the_grandparent() {
    let wrapper = json!({ "type": "element", "tag": "div", "box": bounds(0.0, 0.0, 0.0, 0.0), "style": { "display": "contents", "overflow": "hidden" }, "children": [span(30.0, 26.0)] });
    let run = import(element_with(json!({}), json!([wrapper])));
    assert_eq!(run.root_node().children.len(), 1);
    assert_eq!(run.child(0).name, "span");
    assert_eq!(offset(&run, 0), (10.0, 6.0));
}

// covers: T1
#[test]
fn a_root_with_display_contents_still_becomes_a_frame() {
    let root = json!({ "type": "element", "tag": "div", "box": bounds(0.0, 0.0, 10.0, 10.0), "style": { "display": "contents" } });
    assert_eq!(import(root).root_node().name, "div");
}

// covers: T1
#[test]
fn the_root_goes_where_asked_whatever_its_page_position() {
    let run = import(element_with(json!({}), json!([])));
    assert_eq!(run.root_node().transform.as_coeffs()[4..], [100.0, 50.0]);
    assert_eq!(run.node(run.root_node().parent.unwrap()).name, "Page 1");
}

// covers: T1
#[test]
fn a_tree_deeper_than_the_parser_allows_does_not_overflow_the_stack() {
    let mut node = DomNode::Element(Element {
        tag: "leaf".into(),
        label: None,
        r#box: DomBox { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
        style: Default::default(),
        children: vec![],
        ..Default::default()
    });
    for _ in 0..1000 {
        node = DomNode::Element(Element {
            tag: "div".into(),
            label: None,
            r#box: DomBox { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
            style: Default::default(),
            children: vec![node],
            ..Default::default()
        });
    }
    let mut snapshot: Snapshot = snapshot(json!({ "type": "element", "tag": "x", "box": bounds(0.0, 0.0, 1.0, 1.0) }));
    snapshot.root = node;
    let mut doc = Document::new("t");
    let page = doc.first_page().unwrap();
    import_snapshot(&mut doc, &snapshot, page, 0.0, 0.0).unwrap();
    assert_eq!(doc.node_count(), 1 + 1001);
}

// covers: T9
#[test]
fn a_flood_of_warnings_is_cut_with_a_count_of_what_was_left_out() {
    let noisy =
        |_: usize| json!({ "type": "element", "tag": "p", "box": bounds(20.0, 20.0, 5.0, 5.0), "style": { "backgroundColor": "nonsense(1)" } });
    let children: Vec<Value> = (0..250).map(noisy).collect();
    let run = import(element_with(json!({}), json!(children)));
    assert_eq!(run.warnings.len(), 201, "{:?}", run.warnings.last());
    assert_eq!(run.warnings.last().map(String::as_str), Some("50 more warnings were left out"));
    assert_eq!(run.root_node().children.len(), 250, "the import itself is complete");
}

// covers: T1
#[test]
fn a_parent_that_does_not_exist_is_refused_before_anything_is_added() {
    let snapshot = snapshot(element_with(json!({}), json!([])));
    let mut doc = Document::new("t");
    let before = doc.node_count();
    assert_eq!(import_snapshot(&mut doc, &snapshot, NodeId(9999), 0.0, 0.0), Err(ImportError::ParentNotFound(NodeId(9999))));
    assert_eq!(doc.node_count(), before);
}

// covers: T1
#[test]
fn a_parent_that_cannot_hold_layers_is_refused() {
    let mut doc = Document::new("t");
    let page = doc.first_page().unwrap();
    let rectangle = doc.create(page, NodeKind::Rectangle, "r", 0.0, 0.0, 10.0, 10.0).unwrap();
    let error = import_snapshot(&mut doc, &snapshot(element_with(json!({}), json!([]))), rectangle, 0.0, 0.0).unwrap_err();
    assert!(matches!(error, ImportError::Insert(_)), "{error:?}");
    assert!(error.to_string().contains("can't contain"), "{error}");
}

// covers: T6
#[test]
fn a_hidden_root_or_one_with_nothing_to_draw_is_an_error() {
    let mut doc = Document::new("t");
    let page = doc.first_page().unwrap();
    let before = doc.node_count();
    let hidden = snapshot(json!({ "type": "element", "tag": "div", "box": bounds(0.0, 0.0, 1.0, 1.0), "style": { "visibility": "hidden" } }));
    assert!(matches!(import_snapshot(&mut doc, &hidden, page, 0.0, 0.0), Err(ImportError::NothingToImport(_))));
    let broken = snapshot(json!({ "type": "svg", "markup": "<nope", "box": bounds(0.0, 0.0, 1.0, 1.0) }));
    let error = import_snapshot(&mut doc, &broken, page, 0.0, 0.0).unwrap_err();
    assert!(error.to_string().starts_with("nothing to import"), "{error}");
    assert_eq!(doc.node_count(), before);
}

// covers: T1
#[test]
fn a_position_that_is_not_a_number_falls_back_to_the_origin() {
    let snapshot = snapshot(element_with(json!({}), json!([])));
    let mut doc = Document::new("t");
    let page = doc.first_page().unwrap();
    let imported = import_snapshot(&mut doc, &snapshot, page, f64::NAN, f64::INFINITY).unwrap();
    assert_eq!(doc.get(imported.root).unwrap().transform.as_coeffs()[4..], [0.0, 0.0]);
}

fn stacked(name: &str, x: f64, style: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "type": "element", "tag": "div", "label": name, "box": super::support::bounds(x, 0.0, 10.0, 10.0), "style": style })
}

fn stacking_order(children: serde_json::Value) -> Vec<String> {
    let run = super::support::import(
        serde_json::json!({ "type": "element", "tag": "div", "box": super::support::bounds(0.0, 0.0, 40.0, 10.0), "style": {}, "children": children }),
    );
    run.root_node().children.iter().map(|id| run.node(*id).name.clone()).collect()
}

#[test]
fn overlapping_children_follow_the_stacking_order() {
    let positioned = serde_json::json!({ "position": "relative", "zIndex": "auto", "backgroundColor": "rgb(1, 1, 1)" });
    let plain = serde_json::json!({ "position": "static", "backgroundColor": "rgb(2, 2, 2)" });
    let behind = serde_json::json!({ "position": "absolute", "zIndex": "-1", "backgroundColor": "rgb(3, 3, 3)" });
    let order = stacking_order(serde_json::json!([stacked("over", 0.0, positioned), stacked("plain", 2.0, plain), stacked("behind", 4.0, behind)]));
    assert_eq!(order, ["behind", "plain", "over"]);
}

#[test]
fn children_that_do_not_overlap_keep_the_document_order() {
    let positioned = serde_json::json!({ "position": "relative", "zIndex": "2", "backgroundColor": "rgb(1, 1, 1)" });
    let plain = serde_json::json!({ "position": "static", "backgroundColor": "rgb(2, 2, 2)" });
    let order = stacking_order(serde_json::json!([stacked("over", 0.0, positioned), stacked("plain", 20.0, plain)]));
    assert_eq!(order, ["over", "plain"]);
}

fn scaled_root(transform: &str, children: Value) -> Value {
    json!({ "type": "element", "tag": "button", "box": bounds(21.25, 20.54, 74.5, 34.92), "style": { "transform": transform }, "children": children })
}

// covers: T8
#[test]
fn what_a_scaled_element_holds_stays_where_it_falls() {
    let text = json!({ "type": "text", "text": "Bouton", "box": bounds(37.75, 28.3, 41.5, 18.43), "baseline": 42.85,
        "lineBox": bounds(37.75, 27.5, 41.5, 20.0), "style": { "fontFamily": "Arial", "fontSize": "13.58px" } });
    let run = import(scaled_root("matrix(0.97, 0, 0, 0.97, 0, 0)", json!([text, span(40.5, 30.25)])));
    let [.., x, y] = run.child(0).transform.as_coeffs();
    assert!((x - 16.5).abs() < 1e-9 && (y - 7.76).abs() < 1e-9, "{x} {y}");
    assert_eq!(run.child(0).width, 41.5);
    let NodeKind::Text(data) = &run.child(0).kind else { panic!("a text layer") };
    let baseline = figcraft_text::layout_cached(data, None, None).lines[0].baseline;
    assert!((baseline - (43.0 - 28.3)).abs() < 1e-9, "the baseline stays on a whole pixel of the page: {baseline}");
    let [.., x, y] = run.child(1).transform.as_coeffs();
    assert!((x - 19.25).abs() < 1e-9 && (y - 9.71).abs() < 1e-9, "{x} {y}");
    assert_eq!(run.child(1).width, 4.0);
}

// covers: T8
#[test]
fn the_phase_of_a_capture_is_where_a_scaled_root_falls_in_it() {
    let scaled = snapshot(scaled_root("matrix(0.97, 0, 0, 0.97, 0, 0)", json!([])));
    let [x, y] = crate::capture_phase(&scaled);
    assert!((x - 0.25).abs() < 1e-9 && (y + 0.46).abs() < 1e-9, "{x} {y}");
    assert_eq!(crate::capture_phase(&snapshot(scaled_root("none", json!([])))), [0.0, 0.0]);
    assert_eq!(crate::capture_phase(&snapshot(scaled_root("matrix(1, 0, 0, 1, 3.5, 0)", json!([])))), [0.0, 0.0]);
}

#[test]
fn a_scaled_element_keeps_its_fractional_box() {
    let child = serde_json::json!({ "type": "element", "tag": "div", "label": "scaled", "box": super::support::bounds(0.6, 0.5, 65.2, 31.0),
        "style": { "transform": "matrix(0.97, 0, 0, 0.97, 0, 0)", "backgroundColor": "rgb(1, 1, 1)" } });
    let run = super::support::import(
        serde_json::json!({ "type": "element", "tag": "div", "box": super::support::bounds(0.0, 0.0, 68.0, 32.0), "style": {}, "children": [child] }),
    );
    let node = run.child(0);
    let [.., e, f] = node.transform.as_coeffs();
    assert_eq!((e, f, node.width, node.height), (0.6, 0.5, 65.2, 31.0));
}
