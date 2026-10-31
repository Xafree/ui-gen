//! What the panels show of a variant: its own values in the Design panel, the values that tell it from the others in
//! the layer list, whole value chips, and the place of a file that belongs to a project.

use std::collections::BTreeMap;

use figcraft_doc::NodeId;
use figcraft_engine::Session;
use figcraft_engine::components::variant_set;
use serde_json::{Value, json};

use crate::panels::component_panel::{chip_rows, renamed_variant, shown_values};
use crate::panels::layers::{file_place, variant_row_label};

fn id(value: &Value) -> NodeId {
    figcraft_engine::params::id_of(&value["id"]).unwrap_or_else(|| panic!("no id in {value}"))
}

fn run(s: &mut Session, command: &str, params: Value) -> Value {
    s.execute(command, &params).unwrap_or_else(|error| panic!("{command}: {error}"))
}

fn set_of(s: &mut Session, names: &[&str]) -> Vec<NodeId> {
    let mut ids = Vec::new();
    for (k, name) in names.iter().enumerate() {
        let frame = run(s, "frame.create", json!({"x": k * 150, "y": 0, "width": 100, "height": 40, "name": name}));
        ids.push(id(&run(s, "component.create", json!({"ids": [frame["id"]]}))));
    }
    run(s, "component.combineVariants", json!({"ids": ids}));
    ids
}

const NAMES: [&str; 3] = ["Style=danger, Size=lg, State=Default", "Style=primary, Size=md, State=Default", "Style=primary, Size=lg, State=Default"];

#[test]
fn a_selected_variant_shows_its_own_values_and_the_set_its_defaults() {
    let mut s = Session::new();
    let ids = set_of(&mut s, &NAMES);
    let doc = s.doc().unwrap();
    let set = variant_set(doc, ids[1]).unwrap();
    let own = shown_values(doc, set, Some(ids[1]));
    assert_eq!(own.get("Style").map(String::as_str), Some("primary"));
    assert_eq!(own.get("Size").map(String::as_str), Some("md"));
    let defaults: BTreeMap<String, String> = shown_values(doc, set, None);
    assert_eq!(defaults.get("Style").map(String::as_str), Some("danger"));
}

#[test]
fn choosing_a_value_for_a_selected_variant_renames_only_that_axis() {
    assert_eq!(renamed_variant("Style=primary, Size=md, State=Default", "Size", "lg"), "Style=primary, Size=lg, State=Default");
    assert_eq!(renamed_variant("Style=primary", "Size", "lg"), "Style=primary");
}

#[test]
fn a_variant_row_names_the_values_that_differ_and_any_other_layer_keeps_its_name() {
    let mut s = Session::new();
    let ids = set_of(&mut s, &NAMES);
    let doc = s.doc().unwrap();
    assert_eq!(variant_row_label(doc, doc.node(ids[1]).unwrap()).as_deref(), Some("primary md"));
    assert_eq!(variant_row_label(doc, doc.node(doc.pages[0]).unwrap()), None);
}

#[test]
fn value_chips_wrap_whole_and_a_chip_wider_than_the_row_stays_alone() {
    assert_eq!(chip_rows(&[60.0, 60.0, 90.0, 40.0, 300.0, 10.0], 170.0, 4.0), vec![0..2, 2..4, 4..5, 5..6]);
    assert_eq!(chip_rows(&[], 170.0, 4.0), Vec::<std::ops::Range<usize>>::new());
}

#[test]
fn a_file_of_a_project_sits_in_that_project_and_any_other_in_the_drafts() {
    let dir = std::env::temp_dir().join(format!("figcraft-file-place-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = Session::empty();
    assert_eq!(file_place(&s), None);
    run(&mut s, "project.open", json!({"path": dir.display().to_string(), "create": true}));
    let name = s.project().map(|project| project.config.name.clone());
    assert!(name.as_deref().is_some_and(|name| !name.is_empty()));
    assert_eq!(file_place(&s), name);
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}
