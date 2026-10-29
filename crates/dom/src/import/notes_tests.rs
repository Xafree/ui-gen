use serde_json::{Value, json};

use super::support::bounds;
use super::translate_support::{NOTE_PREFIX, WRAPPER, parse, translate, translate_root};

fn element(label: &str, window: Value, style: Value, children: Value) -> Value {
    json!({ "type": "element", "tag": "div", "label": label, "box": window, "style": style, "children": children })
}

fn card(style: Value, children: Value) -> Value {
    element("Card", bounds(0.0, 0.0, 200.0, 100.0), style, children)
}

fn kept_by_a_child(label: &str, style: Value) -> Value {
    let child = element("Kid", bounds(12.0, 12.0, 10.0, 10.0), json!({ "backgroundColor": "rgb(9, 9, 9)" }), json!([]));
    element(label, bounds(10.0, 10.0, 30.0, 20.0), style, json!([child]))
}

fn sibling(label: &str, color: &str) -> Value {
    element(label, bounds(10.0, 10.0, 30.0, 20.0), json!({ "backgroundColor": color }), json!([]))
}

// covers: E3-3.4
#[test]
fn an_unreadable_value_becomes_a_note_on_its_layer_and_a_warning() {
    let broken = kept_by_a_child("Broken", json!({ "backgroundColor": "nonsense(1, 2)" }));

    let run = translate_root(card(json!({}), json!([broken, sibling("Fine", "rgb(0, 128, 0)")])));

    let broken = run.by_name("Broken");
    let labels = run.note_labels(broken);
    assert!(labels.iter().any(|label| label.starts_with(&format!("{NOTE_PREFIX}backgroundColor nonsense(1, 2)"))), "{labels:?}");
    assert!(run.translated.warnings.iter().any(|warning| warning.contains("backgroundColor nonsense(1, 2)")), "{:?}", run.translated.warnings);
    assert!(run.note_labels(run.root()).is_empty(), "the note sits on the layer concerned, not on its parent");
    assert!(run.note_labels(run.by_name("Fine")).is_empty());
    assert_eq!(run.by_name("Fine").fills.len(), 1, "the translation went on");
}

// covers: E3-3.4
#[test]
fn every_warning_has_its_note_and_every_note_its_warning() {
    let broken = kept_by_a_child("Broken", json!({ "backgroundColor": "nonsense(1, 2)", "borderTopLeftRadius": "wide", "opacity": "abc" }));

    let run = translate_root(card(json!({}), json!([broken])));

    let labels = run.all_note_labels();
    assert_eq!(labels.len(), 3, "{labels:?}");
    assert_eq!(run.translated.warnings.len(), labels.len(), "{:?}", run.translated.warnings);
    for label in &labels {
        let message = label.trim_start_matches(NOTE_PREFIX);
        assert!(run.translated.warnings.iter().any(|warning| warning.contains(message)), "{label:?} has no warning in {:?}", run.translated.warnings);
    }
    for property in ["backgroundColor", "borderTopLeftRadius", "opacity"] {
        assert!(run.note_labels(run.by_name("Broken")).iter().any(|label| label.contains(property)), "{property}");
    }
}

// covers: E3-3.4
#[test]
fn an_unreadable_value_leaves_the_default_of_the_layer_never_an_invented_one() {
    let broken = kept_by_a_child("Broken", json!({ "backgroundColor": "nonsense(1, 2)", "borderTopLeftRadius": "wide", "opacity": "abc" }));

    let run = translate_root(card(json!({}), json!([broken])));

    let node = run.by_name("Broken");
    assert!(node.fills.is_empty());
    assert_eq!(node.corners.radii, [0.0; 4]);
    assert_eq!(node.opacity, 1.0);
}

// covers: E3-3.4
#[test]
fn a_snapshot_whose_values_are_all_readable_has_no_note_and_no_warning() {
    let run = translate(&parse(WRAPPER));

    assert!(run.all_note_labels().is_empty(), "{:?}", run.all_note_labels());
    assert!(run.translated.warnings.is_empty(), "{:?}", run.translated.warnings);
}

// covers: E3-3.5
#[test]
fn homonymous_siblings_are_numbered_from_the_second_one() {
    let items =
        json!([sibling("Item", "rgb(1, 1, 1)"), sibling("Item", "rgb(2, 2, 2)"), sibling("Item", "rgb(3, 3, 3)"), sibling("Other", "rgb(4, 4, 4)")]);

    let run = translate_root(card(json!({}), items));

    assert_eq!(run.child_names(run.root()), ["Item", "Item 2", "Item 3", "Other"]);
    let paths: Vec<&str> = run.translated.layers.iter().map(|layer| layer.path.as_str()).collect();
    assert_eq!(paths, ["Card", "Card / Item", "Card / Item 2", "Card / Item 3", "Card / Other"]);
}

// covers: E3-3.5
#[test]
fn two_translations_of_the_same_structure_give_the_same_paths_in_the_same_order() {
    let original = WRAPPER.to_owned();
    let recolored = original.replace("rgb(25, 118, 210)", "rgb(9, 9, 9)").replace("\"x\": 100", "\"x\": 101");

    let first = translate(&parse(&original));
    let second = translate(&parse(&recolored));

    let paths =
        |run: &super::translate_support::Translation| -> Vec<String> { run.translated.layers.iter().map(|layer| layer.path.clone()).collect() };
    assert!(paths(&first).len() > 1);
    assert_eq!(paths(&first), paths(&second));
    assert_eq!(paths(&first)[0], "Card");
}
