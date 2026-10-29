//! `uigen.importDom` `parent`: only a frame or a page receives the layers, as in `node.create`,
//! and a text layer is sized by the text engine, never by the snapshot.

use crate::params::id_of;
use crate::{EngineError, Session};
use serde_json::{Value, json};

fn button_snapshot(text_width: f64) -> Value {
    json!({
        "version": 1,
        "viewport": { "width": 400, "height": 200, "deviceScaleFactor": 1 },
        "root": {
            "type": "element",
            "tag": "button",
            "box": { "x": 20, "y": 20, "width": 92, "height": 36 },
            "style": { "backgroundColor": "rgb(25, 118, 210)" },
            "children": [{
                "type": "text",
                "text": "Bouton",
                "box": { "x": 30, "y": 26, "width": text_width, "height": 24 },
                "lineBox": { "x": 30, "y": 26, "width": text_width, "height": 24 },
                "style": { "color": "rgb(255, 255, 255)", "fontSize": "14px" }
            }]
        }
    })
}

fn created(s: &mut Session, command: &str, params: &Value) -> figcraft_doc::NodeId {
    id_of(&s.execute(command, params).unwrap()["id"]).unwrap()
}

fn assert_refused_without_change(s: &mut Session, parent: figcraft_doc::NodeId, what: &str, cause: &str) {
    let before = s.doc().unwrap().node_count();
    let depth = s.undo_depth();

    let err = s.execute("uigen.importDom", &json!({ "snapshot": button_snapshot(50.0), "parent": parent })).unwrap_err();

    assert!(matches!(err, EngineError::BadParams { .. }), "{what}: a bad-parameters error, got {err:?}");
    let message = err.to_string();
    assert!(message.contains("`parent`") && message.contains(cause), "{what}: the error names the parent and the cause: {message}");
    assert_eq!(s.doc().unwrap().node_count(), before, "{what}: the document is unchanged");
    assert_eq!(s.undo_depth(), depth, "{what}: no undo step");
}

// covers: C1
#[test]
fn a_frame_and_a_component_receive_the_imported_layers() {
    let mut s = Session::new();
    let frame = created(&mut s, "frame.create", &json!({"x": 0, "y": 0, "width": 500, "height": 300}));
    let inner = created(&mut s, "frame.create", &json!({"x": 0, "y": 0, "width": 50, "height": 50}));
    let component = created(&mut s, "component.create", &json!({"ids": [inner]}));

    for parent in [frame, component] {
        let result = s.execute("uigen.importDom", &json!({ "snapshot": button_snapshot(50.0), "parent": parent })).unwrap();
        let root = id_of(&result["root"]).unwrap();
        assert_eq!(s.doc().unwrap().node(root).unwrap().parent, Some(parent));
    }
}

// covers: C2
#[test]
fn a_group_is_refused_as_parent() {
    let mut s = Session::new();
    let a = created(&mut s, "node.create", &json!({"type": "rectangle", "x": 0, "y": 0, "width": 10, "height": 10}));
    let b = created(&mut s, "node.create", &json!({"type": "rectangle", "x": 20, "y": 0, "width": 10, "height": 10}));
    let group = created(&mut s, "object.group", &json!({"ids": [a, b]}));
    assert!(matches!(s.doc().unwrap().node(group).unwrap().kind, figcraft_doc::NodeKind::Group), "the fixture is a group");

    assert_refused_without_change(&mut s, group, "group", "only a frame or a page");
}

// covers: C2
#[test]
fn an_instance_is_refused_as_parent() {
    let mut s = Session::new();
    let inner = created(&mut s, "frame.create", &json!({"x": 0, "y": 0, "width": 50, "height": 50}));
    let component = created(&mut s, "component.create", &json!({"ids": [inner]}));
    let instance = created(&mut s, "instance.create", &json!({"component": component, "x": 100, "y": 0}));

    assert_refused_without_change(&mut s, instance, "instance", "only a frame or a page");
}

// covers: C2
#[test]
fn a_leaf_layer_and_a_missing_layer_are_refused_as_parent() {
    let mut s = Session::new();
    let rectangle = created(&mut s, "node.create", &json!({"type": "rectangle", "x": 0, "y": 0, "width": 10, "height": 10}));

    assert_refused_without_change(&mut s, rectangle, "rectangle", "only a frame or a page");
    assert_refused_without_change(&mut s, figcraft_doc::NodeId(99_999), "missing layer", "does not exist");
}

// covers: C1
#[test]
fn the_text_layer_width_is_measured_by_the_text_engine_not_taken_from_the_snapshot() {
    let mut s = Session::new();
    let narrow = s.execute("uigen.importDom", &json!({ "snapshot": button_snapshot(40.0) })).unwrap();
    let wide = s.execute("uigen.importDom", &json!({ "snapshot": button_snapshot(300.0) })).unwrap();

    let width_of = |s: &Session, result: &Value| {
        let root = id_of(&result["root"]).unwrap();
        let text = s.doc().unwrap().node(root).unwrap().children[0];
        s.doc().unwrap().node(text).unwrap().width
    };
    assert!((width_of(&s, &narrow) - width_of(&s, &wide)).abs() < 1e-9, "the same text has the same measured width");
    assert!(width_of(&s, &wide) < 100.0, "far from the 300 px the snapshot claims");
}
