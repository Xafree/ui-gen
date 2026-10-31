use serde_json::{Value, json};

use super::facts_tests::facts_of;
use super::support::bounds;
use super::translate_support::{Translation, translate_root};
use crate::{DomNode, Effect, SnapshotError, parse_snapshot};

const EFFECT_KEY: &str = "uigen.effect";
const SQUARE: &str =
    r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><rect width="24" height="24" fill="#ff0000"/></svg>"##;

fn turning(property: &str) -> Value {
    json!({ "name": "spin", "durationMs": 1400, "easing": "linear", "iterations": "infinite", "properties": [property] })
}

fn svg_with(animations: Value) -> Translation {
    translate_root(json!({
        "type": "element", "tag": "button", "label": "Host", "box": bounds(0.0, 0.0, 200.0, 100.0),
        "style": { "backgroundColor": "rgb(25, 118, 210)" },
        "children": [{ "type": "svg", "markup": SQUARE, "box": bounds(10.0, 10.0, 24.0, 24.0), "animations": animations }]
    }))
}

fn element_with(animations: Value) -> Translation {
    translate_root(json!({
        "type": "element", "tag": "span", "label": "Wheel", "box": bounds(0.0, 0.0, 40.0, 40.0),
        "style": { "backgroundColor": "rgb(25, 118, 210)" }, "animations": animations
    }))
}

fn root_json(root: &Value) -> String {
    json!({ "version": 1, "viewport": { "width": 400, "height": 200, "deviceScaleFactor": 1 }, "root": root }).to_string()
}

fn element_json(fields: Value) -> String {
    let mut root = json!({ "type": "element", "tag": "div", "box": bounds(0.0, 0.0, 10.0, 10.0) });
    if let (Some(target), Some(extra)) = (root.as_object_mut(), fields.as_object()) {
        target.extend(extra.clone());
    }
    root_json(&root)
}

fn svg_json(animations: Value) -> String {
    root_json(&json!({ "type": "svg", "markup": SQUARE, "box": bounds(0.0, 0.0, 24.0, 24.0), "animations": animations }))
}

fn refused(json: &str) -> SnapshotError {
    parse_snapshot(json).expect_err("the snapshot must be refused")
}

fn animation_with(field: &str, value: Value) -> Value {
    let mut animation = turning("transform");
    animation[field] = value;
    animation
}

#[test]
fn an_svg_turning_forever_is_a_spinner() {
    let run = svg_with(json!([turning("transform")]));

    assert_eq!(facts_of(&run, "Icon").effect, Some(Effect::Spinner));
    assert_eq!(run.by_name("Icon").plugin_data.get(EFFECT_KEY).map(String::as_str), Some("spinner"));
    assert_eq!(run.by_name("Host").plugin_data.get(EFFECT_KEY), None);
}

#[test]
fn the_css_rotate_property_turns_an_element_or_an_svg_into_a_spinner() {
    let on_svg = svg_with(json!([turning("rotate")]));
    let on_element = element_with(json!([turning("rotate")]));

    assert_eq!(facts_of(&on_svg, "Icon").effect, Some(Effect::Spinner));
    assert_eq!(facts_of(&on_element, "Wheel").effect, Some(Effect::Spinner));
}

#[test]
fn an_svg_with_another_animation_is_not_a_spinner_and_gets_a_note() {
    let fade = json!({ "name": "fade-in", "durationMs": 300, "easing": "ease", "iterations": "infinite", "properties": ["opacity"] });
    let run = svg_with(json!([fade]));

    assert_eq!(facts_of(&run, "Icon").effect, None);
    let labels = run.note_labels(run.by_name("Icon"));
    assert!(labels.iter().any(|label| label.contains("fade-in")), "{labels:?}");
}

#[test]
fn the_animations_of_an_svg_node_are_read_and_capped_like_those_of_an_element() {
    let DomNode::Svg(svg) = parse_snapshot(&svg_json(json!([turning("rotate")]))).unwrap().root else { panic!("an svg root was expected") };
    assert_eq!(svg.animations.len(), 1);
    assert_eq!(svg.animations[0].properties, ["rotate"]);

    let too_many = svg_json(json!(vec![turning("transform"); 17]));
    assert!(matches!(refused(&too_many), SnapshotError::TooManyAnimations { count: 17, .. }));

    let negative = svg_json(json!([animation_with("durationMs", json!(-1))]));
    assert!(parse_snapshot(&negative).is_err());
}

#[test]
fn an_svg_without_animations_still_reads() {
    let json = root_json(&json!({ "type": "svg", "markup": SQUARE, "box": bounds(0.0, 0.0, 24.0, 24.0) }));
    let DomNode::Svg(svg) = parse_snapshot(&json).unwrap().root else { panic!("an svg root was expected") };
    assert!(svg.animations.is_empty());
}

#[test]
fn the_facts_of_an_svg_node_are_validated_like_those_of_an_element() {
    let long_name = svg_json(json!([animation_with("name", json!("n".repeat(257)))]));
    assert!(matches!(refused(&long_name), SnapshotError::FieldTooLong { limit: 256, .. }));
}

#[test]
fn a_class_name_longer_than_256_characters_is_refused_and_256_is_kept() {
    assert!(parse_snapshot(&element_json(json!({ "className": "c".repeat(256) }))).is_ok());

    let long = element_json(json!({ "className": "c".repeat(257) }));
    assert!(matches!(refused(&long), SnapshotError::FieldTooLong { field: "className", limit: 256, .. }));
}

#[test]
fn an_animation_name_or_easing_longer_than_256_characters_is_refused() {
    for field in ["name", "easing"] {
        let exact = element_json(json!({ "animations": [animation_with(field, json!("e".repeat(256)))] }));
        assert!(parse_snapshot(&exact).is_ok(), "{field} at 256");

        let long = element_json(json!({ "animations": [animation_with(field, json!("e".repeat(257)))] }));
        assert!(matches!(refused(&long), SnapshotError::FieldTooLong { limit: 256, .. }), "{field} at 257");
    }
}

#[test]
fn animated_properties_are_at_most_64_entries_of_128_characters() {
    let exact = element_json(json!({ "animations": [animation_with("properties", json!(vec!["p"; 64]))] }));
    assert!(parse_snapshot(&exact).is_ok());

    let many = element_json(json!({ "animations": [animation_with("properties", json!(vec!["p"; 65]))] }));
    assert!(matches!(refused(&many), SnapshotError::TooManyProperties { count: 65, .. }));

    let wide = element_json(json!({ "animations": [animation_with("properties", json!(["w".repeat(128)]))] }));
    assert!(parse_snapshot(&wide).is_ok());

    let long = element_json(json!({ "animations": [animation_with("properties", json!(["w".repeat(129)]))] }));
    assert!(matches!(refused(&long), SnapshotError::FieldTooLong { limit: 128, .. }));
}

fn leaf(label: &str) -> Value {
    json!({ "type": "element", "tag": "i", "label": label, "box": bounds(1.0, 1.0, 5.0, 5.0), "style": { "backgroundColor": "rgb(1, 2, 3)" } })
}

#[test]
fn a_slash_in_a_layer_name_cannot_make_two_layers_share_a_path() {
    let nested = json!({
        "type": "element", "tag": "div", "label": "A", "box": bounds(0.0, 0.0, 40.0, 40.0),
        "style": { "backgroundColor": "rgb(9, 9, 9)" }, "children": [leaf("B")]
    });
    let run = translate_root(json!({
        "type": "element", "tag": "div", "label": "R", "box": bounds(0.0, 0.0, 100.0, 100.0),
        "style": { "backgroundColor": "rgb(7, 7, 7)" }, "children": [nested, leaf("A / B")]
    }));

    let mut paths: Vec<&str> = run.translated.layers.iter().map(|layer| layer.path.as_str()).collect();
    let total = paths.len();
    paths.sort_unstable();
    paths.dedup();
    assert_eq!(paths.len(), total, "paths must be unique: {paths:?}");
    assert_eq!(run.by_name("A / B").name, "A / B", "the displayed name keeps its slash");
}

#[test]
fn a_path_of_plain_names_is_unchanged() {
    let run = translate_root(json!({
        "type": "element", "tag": "div", "label": "R", "box": bounds(0.0, 0.0, 100.0, 100.0),
        "style": { "backgroundColor": "rgb(7, 7, 7)" }, "children": [leaf("Kid")]
    }));

    assert_eq!(facts_of(&run, "Kid").path, "R / Kid");
}

#[test]
fn the_note_prefix_is_exported_by_the_crate() {
    assert_eq!(crate::NOTE_PREFIX, "UIGEN import: ");
}
