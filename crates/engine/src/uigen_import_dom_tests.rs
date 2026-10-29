//! `uigen.importDom`: a DOM snapshot becomes editable layers on the current page (or under
//! `parent`), in one undo step; an invalid snapshot is refused and the document stays as it was.
//! Snapshots are synthetic.

use crate::cmd::{all_commands, find_command};
use crate::params::id_of;
use crate::{EngineError, Session};
use serde_json::{Value, json};

fn button_snapshot(version: u32) -> Value {
    json!({
        "version": version,
        "viewport": { "width": 400, "height": 200, "deviceScaleFactor": 1 },
        "root": {
            "type": "element",
            "tag": "button",
            "label": "Bouton",
            "box": { "x": 20, "y": 20, "width": 92, "height": 36 },
            "style": { "backgroundColor": "rgb(25, 118, 210)" },
            "children": [{
                "type": "text",
                "text": "Bouton",
                "box": { "x": 30, "y": 26, "width": 50, "height": 24 },
                "style": { "color": "rgb(255, 255, 255)", "fontSize": "14px" }
            }]
        }
    })
}

fn root_of(result: &Value) -> figcraft_doc::NodeId {
    id_of(&result["root"]).unwrap_or_else(|| panic!("no `root` id in {result}"))
}

// covers: C1
#[test]
fn command_is_registered_with_its_label_and_makes_an_undo_step() {
    let spec = find_command("uigen.importDom").expect("uigen.importDom is registered");
    assert_eq!(spec.label, "Import DOM snapshot");
    assert!(spec.history, "the command edits the document");
}

// covers: C1
#[test]
fn snapshot_object_becomes_layers_on_the_current_page_in_one_undo_step() {
    let mut s = Session::new();
    let before = s.doc().unwrap().node_count();
    let depth = s.undo_depth();

    let result = s.execute("uigen.importDom", &json!({ "snapshot": button_snapshot(1) })).unwrap();

    let root = root_of(&result);
    assert!(result["warnings"].is_array(), "warnings is an array: {result}");
    let page = s.page().unwrap();
    let doc = s.doc().unwrap();
    let node = doc.node(root).expect("the root layer is in the document");
    assert_eq!(node.parent, Some(page), "child of the current page");
    assert_eq!(node.name, "Bouton");
    assert_eq!(node.children.len(), 1, "the text run is a child layer");
    assert!(doc.node_count() > before);
    assert_eq!(s.undo_depth(), (depth.0 + 1, 0), "exactly one undo step");

    assert!(s.undo().unwrap());
    assert!(s.doc().unwrap().node(root).is_none(), "the layers are gone after one undo");
    assert_eq!(s.doc().unwrap().node_count(), before);
}

// covers: C1
#[test]
fn snapshot_as_json_text_is_placed_at_x_y() {
    let mut s = Session::new();
    let text = button_snapshot(1).to_string();

    let result = s.execute("uigen.importDom", &json!({ "snapshot": text, "x": 100, "y": 50 })).unwrap();

    let root = root_of(&result);
    let origin = s.doc().unwrap().world_transform(root).translation();
    assert_eq!((origin.x, origin.y), (100.0, 50.0));
}

// covers: C1
#[test]
fn parent_receives_the_imported_layers() {
    let mut s = Session::new();
    let frame = id_of(&s.execute("frame.create", &json!({"x": 0, "y": 0, "width": 500, "height": 300})).unwrap()["id"]).unwrap();

    let result = s.execute("uigen.importDom", &json!({ "snapshot": button_snapshot(1), "parent": frame })).unwrap();

    let root = root_of(&result);
    assert_eq!(s.doc().unwrap().node(root).unwrap().parent, Some(frame));
}

// covers: C2
#[test]
fn unsupported_version_is_refused_naming_the_cause_and_the_document_is_unchanged() {
    let mut s = Session::new();
    let before = s.doc().unwrap().node_count();
    let depth = s.undo_depth();

    let err = s.execute("uigen.importDom", &json!({ "snapshot": button_snapshot(2) })).unwrap_err();

    assert!(matches!(err, EngineError::BadParams { .. }), "a bad-parameters error, got {err:?}");
    assert!(err.to_string().to_lowercase().contains("version"), "the error cites the cause: {err}");
    assert_eq!(s.doc().unwrap().node_count(), before);
    assert_eq!(s.undo_depth(), depth, "no undo step for a refused import");
}

// covers: C2
#[test]
fn text_that_is_not_json_is_refused_and_the_document_is_unchanged() {
    let mut s = Session::new();
    let before = s.doc().unwrap().node_count();

    let err = s.execute("uigen.importDom", &json!({ "snapshot": "{ not json" })).unwrap_err();

    assert!(matches!(err, EngineError::BadParams { .. }), "a bad-parameters error, got {err:?}");
    assert_eq!(s.doc().unwrap().node_count(), before);
}

// covers: C3
#[test]
fn command_is_in_the_registry_the_junk_parameter_sweep_walks() {
    assert!(all_commands().iter().any(|spec| spec.id == "uigen.importDom"));
    let junk = [json!({}), json!({"snapshot": null}), json!({"snapshot": [1], "parent": "x", "x": 1e308, "y": "NaN"}), json!({"snapshot": 5})];
    for j in &junk {
        let mut s = Session::new();
        let outcome = s.execute("uigen.importDom", j);
        assert!(!matches!(outcome, Err(EngineError::Internal { .. }) | Err(EngineError::UnknownCommand(_))), "{j}: {outcome:?}");
    }
}
