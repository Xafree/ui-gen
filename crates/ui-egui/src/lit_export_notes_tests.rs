//! The Lit export window lists each note of the plan by its message.

use figcraft_doc::NodeId;
use figcraft_engine::Session;
use serde_json::{Value, json};

use crate::dialogs::lit_export;
use crate::import_report_tests::shown_texts;
use crate::layers_drag_tests::Ui;

fn made(s: &mut Session, cmd: &str, p: Value) -> NodeId {
    let v = s.execute(cmd, &p).unwrap();
    NodeId(v["id"].as_u64().unwrap())
}

#[test]
fn a_dependency_note_is_listed_with_its_message() {
    let mut s = Session::new();
    let frame = made(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 24, "height": 24, "name": "Heart"}));
    made(&mut s, "shape.star", json!({"x": 0, "y": 0, "width": 24, "height": 24, "parent": frame}));
    let heart = made(&mut s, "component.create", json!({"ids": [frame]}));
    let host = made(&mut s, "frame.create", json!({"x": 200, "y": 0, "width": 120, "height": 40, "name": "Chip"}));
    made(&mut s, "instance.create", json!({"component": heart, "x": 8, "y": 8, "parent": host}));
    made(&mut s, "component.create", json!({"ids": [host]}));
    let mut u = Ui::new(s);
    assert!(u.app.run("export.litWindow", json!({})).is_some());
    lit_export::set_checked(&mut u.app, heart, false);
    u.frame(vec![]);
    let notes: Vec<String> = u.app.ui.lit_export.as_ref().unwrap().notes().into_iter().map(str::to_string).collect();
    let dependency = notes.iter().find(|n| n.to_lowercase().contains("heart")).expect("the dependency is noted");
    u.frame(vec![]);
    let shown = shown_texts(u.frame_output(vec![]).shapes);
    assert!(shown.iter().any(|t| t.contains(dependency.as_str())), "{dependency:?} is on screen: {shown:?}");
}
