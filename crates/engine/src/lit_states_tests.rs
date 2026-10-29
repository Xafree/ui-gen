//! Lit states: variant values such as Hover, Focus, Pressed, Disabled and Loading become CSS
//! selectors and reflected booleans instead of a `state` property, in English and in French.

use crate::Session;
use crate::components::tests::{ex, idv};
use crate::lit::{self, StateKind};
use figcraft_doc::*;
use serde_json::{Value, json};

fn component_code(s: &mut Session, id: NodeId) -> Value {
    ex(s, "inspect.componentCode", json!({"id": id, "language": "lit"}))
}

fn definition(s: &mut Session, id: NodeId) -> String {
    component_code(s, id)["definition"].as_str().unwrap_or_default().to_string()
}

fn compact(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").replace("> <", "><")
}

/// The first CSS rule whose selector starts with `start`: (selector, body).
fn rule_for(def: &str, start: &str) -> Option<(String, String)> {
    let text = compact(def);
    let rest = text.get(text.find(start)?..)?;
    let open = rest.find('{')?;
    let selector = rest.get(..open)?.trim().to_string();
    let body = rest.get(open + 1..)?.split('}').next()?.trim().to_string();
    Some((selector, body))
}

fn body_of(def: &str, start: &str) -> String {
    rule_for(def, start).map(|(_, body)| body).unwrap_or_else(|| panic!("no rule starting with {start} in:\n{def}"))
}

/// A set named `name`: one variant per `(variant name, background colour)` row, each with a "Bg"
/// rectangle and an "Icon" ellipse. The first row is the default variant.
fn state_set(s: &mut Session, name: &str, rows: &[(&str, &str)]) -> NodeId {
    let mut variants = Vec::new();
    for (k, (variant, fill)) in rows.iter().enumerate() {
        let f = idv(&ex(s, "frame.create", json!({"x": k as f64 * 150.0, "y": 0, "width": 100, "height": 40, "name": variant})));
        ex(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 40, "parent": f, "props": {"name": "Bg", "fill": fill}}));
        ex(s, "shape.ellipse", json!({"x": 80, "y": 10, "width": 10, "height": 10, "parent": f, "props": {"name": "Icon"}}));
        variants.push(idv(&ex(s, "component.create", json!({"ids": [f]}))));
    }
    let set = idv(&ex(s, "component.combineVariants", json!({"ids": variants})));
    ex(s, "node.set", json!({"id": set, "props": {"name": name}}));
    set
}

fn kind_name(kind: Option<StateKind>) -> &'static str {
    match kind {
        None => "none",
        Some(StateKind::Rest) => "rest",
        Some(StateKind::Hover) => "hover",
        Some(StateKind::FocusVisible) => "focus-visible",
        Some(StateKind::Active) => "active",
        Some(StateKind::Disabled) => "disabled",
        Some(StateKind::Loading) => "loading",
        Some(StateKind::Other) => "other",
    }
}

fn icon_layer(s: &Session, set: NodeId) -> NodeId {
    let doc = s.doc().unwrap();
    let first = doc.node(set).unwrap().children.first().copied().unwrap();
    doc.node(first).unwrap().children.iter().copied().find(|k| doc.node(*k).unwrap().name == "Icon").unwrap()
}

#[test]
fn hover_becomes_a_host_pseudo_class_with_only_the_differences() {
    // covers: E6-2.1
    let mut s = Session::new();
    let set = state_set(&mut s, "Control", &[("State=Default", "#ff0000"), ("State=Hover", "#0000ff")]);
    let v = component_code(&mut s, set);
    let def = v["definition"].as_str().unwrap_or_default();
    assert_eq!(body_of(def, ":host(:hover)"), "background: #0000FF;", "only what differs:\n{def}");
    let text = compact(def);
    assert!(!text.contains(") state") && !text.contains("[state="), "no state property:\n{def}");
    assert!(!def.contains("data-state") && !def.contains("data-"), "no data attribute:\n{def}");
    let attributes: Vec<&str> =
        v["properties"].as_array().map(|all| all.iter().filter_map(|p| p["attribute"].as_str()).collect()).unwrap_or_default();
    assert!(!attributes.contains(&"state"), "{attributes:?}");
}

#[test]
fn focus_pressed_disabled_and_loading_each_get_their_selector() {
    // covers: E6-2.2
    let mut s = Session::new();
    let rows = [
        ("State=Default", "#ff0000"),
        ("State=Focus", "#00ff00"),
        ("State=Pressed", "#ffff00"),
        ("State=Disabled", "#808080"),
        ("State=Loading", "#00ffff"),
    ];
    let set = state_set(&mut s, "Control", &rows);
    let def = definition(&mut s, set);
    assert_eq!(body_of(&def, ":host(:focus-visible)"), "background: #00FF00;", "{def}");
    assert_eq!(body_of(&def, ":host(:active)"), "background: #FFFF00;", "{def}");
    assert_eq!(body_of(&def, ":host([disabled])"), "background: #808080;", "{def}");
    assert_eq!(body_of(&def, ":host([loading])"), "background: #00FFFF;", "{def}");
    let text = compact(&def);
    for want in ["@property({ type: Boolean, reflect: true }) disabled = false;", "@property({ type: Boolean, reflect: true }) loading = false;"] {
        assert!(text.contains(want), "{want:?} in:\n{def}");
    }
    assert!(!text.contains(") state") && !text.contains("[state="), "no state property:\n{def}");
}

#[test]
fn rest_values_write_no_rule() {
    // covers: E6-2.3
    let mut s = Session::new();
    let set = state_set(&mut s, "Control", &[("State=Default", "#ff0000"), ("State=Hover", "#0000ff"), ("State=Normal", "#00ff00")]);
    let def = definition(&mut s, set);
    let text = compact(&def);
    assert_eq!(text.matches(":host(").count(), 1, "only the hover rule:\n{def}");
    assert!(!text.contains("#00FF00"), "a rest value has no rule:\n{def}");
    assert!(!text.contains(") state"), "{def}");
}

#[test]
fn style_and_state_combine_and_the_difference_is_against_the_same_style_at_rest() {
    // covers: E6-2.4
    let mut s = Session::new();
    let rows = [
        ("Style=Primary, State=Default", "#ff0000"),
        ("Style=Primary, State=Hover", "#0000ff"),
        ("Style=Secondary, State=Default", "#00ff00"),
        ("Style=Secondary, State=Hover", "#ff0000"),
    ];
    let set = state_set(&mut s, "Control", &rows);
    let def = definition(&mut s, set);
    let selector = ":host([style-variant=\"Secondary\"]:hover)";
    assert_eq!(body_of(&def, selector), "background: #FF0000;", "against (Secondary, Default), not the default variant:\n{def}");
    assert!(!compact(&def).contains(") state"), "{def}");
}

#[test]
fn an_unrecognised_value_stays_a_property_and_is_reported() {
    // covers: E6-2.5
    let mut s = Session::new();
    let set = state_set(&mut s, "Control", &[("State=Default", "#ff0000"), ("State=Hover", "#0000ff"), ("State=Error", "#ff00ff")]);
    let v = component_code(&mut s, set);
    let def = v["definition"].as_str().unwrap_or_default();
    assert!(compact(def).contains("state: \"Default\" | \"Error\" = \"Default\";"), "{def}");
    assert_eq!(body_of(def, ":host([state=\"Error\"])"), "background: #FF00FF;", "{def}");
    assert_eq!(body_of(def, ":host(:hover)"), "background: #0000FF;", "the recognised value keeps its pseudo-class:\n{def}");
    let notes = v["notes"].as_array().cloned().unwrap_or_default();
    assert!(notes.iter().any(|n| n["code"] == "unrecognized-state"), "{notes:?}");
}

#[test]
fn french_names_and_values_with_accents_are_recognised() {
    // covers: E6-2.6
    let mut s = Session::new();
    let rows = [("État=Repos", "#ff0000"), ("État=Survol", "#0000ff"), ("État=Désactivé", "#808080"), ("État=Chargement", "#00ffff")];
    let set = state_set(&mut s, "Control", &rows);
    let def = definition(&mut s, set);
    assert_eq!(body_of(&def, ":host(:hover)"), "background: #0000FF;", "{def}");
    assert_eq!(body_of(&def, ":host([disabled])"), "background: #808080;", "{def}");
    assert_eq!(body_of(&def, ":host([loading])"), "background: #00FFFF;", "{def}");
    let text = compact(&def).to_lowercase();
    assert!(!text.contains(") etat") && !text.contains(") état") && !text.contains("[etat=") && !text.contains("[état="), "{def}");
}

#[test]
fn state_kind_reads_the_dictionary_in_english_and_french() {
    // covers: E6-2.6
    let cases = [
        ("State", "Default", Some(StateKind::Rest)),
        ("state", "ENABLED", Some(StateKind::Rest)),
        ("État", "Défaut", Some(StateKind::Rest)),
        ("State", "Hover", Some(StateKind::Hover)),
        ("État", "Survolé", Some(StateKind::Hover)),
        ("Etat", "survol", Some(StateKind::Hover)),
        ("State", "Focus", Some(StateKind::FocusVisible)),
        ("State", "Focus Clavier", Some(StateKind::FocusVisible)),
        ("Status", "focus-visible", Some(StateKind::FocusVisible)),
        ("State", "Pressed", Some(StateKind::Active)),
        ("État", "Appuyé", Some(StateKind::Active)),
        ("État", "Enfoncé", Some(StateKind::Active)),
        ("State", "Disabled", Some(StateKind::Disabled)),
        ("Etat", "DÉSACTIVÉ", Some(StateKind::Disabled)),
        ("State", "Loading", Some(StateKind::Loading)),
        ("État", "Chargement", Some(StateKind::Loading)),
        ("State", "Error", Some(StateKind::Other)),
        ("État", "Inconnu", Some(StateKind::Other)),
        ("Style", "Hover", None),
        ("Size", "Large", None),
        ("", "Hover", None),
    ];
    for (prop, value, want) in cases {
        let got = lit::state_kind(prop, value);
        assert_eq!(kind_name(got), kind_name(want), "state_kind({prop:?}, {value:?})");
    }
}

#[test]
fn a_disabled_boolean_and_a_disabled_state_make_one_property() {
    // covers: E6-2.7
    let mut s = Session::new();
    let set = state_set(&mut s, "Control", &[("State=Default", "#ff0000"), ("State=Disabled", "#808080")]);
    let icon = icon_layer(&s, set);
    ex(&mut s, "component.addProperty", json!({"id": set, "name": "Disabled", "type": "boolean", "layer": icon, "default": false}));
    let v = component_code(&mut s, set);
    let def = v["definition"].as_str().unwrap_or_default();
    assert_eq!(compact(def).matches(") disabled").count(), 1, "one member:\n{def}");
    let disabled = v["properties"].as_array().map(|all| all.iter().filter(|p| p["attribute"] == "disabled").count()).unwrap_or_default();
    assert_eq!(disabled, 1, "{v}");
    assert_eq!(body_of(def, ":host([disabled])"), "background: #808080;", "{def}");
}

#[test]
fn focus_adds_a_tabindex_and_other_sets_do_not() {
    // covers: E6-2.8
    let mut s = Session::new();
    let with_focus = state_set(&mut s, "Field", &[("State=Default", "#ff0000"), ("State=Focus", "#00ff00")]);
    let def = definition(&mut s, with_focus);
    assert!(def.contains("tabindex"), "{def}");
    let without = state_set(&mut s, "Chip", &[("State=Default", "#ff0000"), ("State=Hover", "#0000ff")]);
    let def = definition(&mut s, without);
    assert!(!def.contains("tabindex"), "{def}");
}

#[test]
fn a_set_without_a_state_has_no_state_selector() {
    // covers: E6-2.9
    let mut s = Session::new();
    let set = state_set(&mut s, "Badge", &[("Tone=Soft", "#ff0000"), ("Tone=Loud", "#0000ff")]);
    let def = definition(&mut s, set);
    for absent in [":hover", ":focus-visible", ":active", "[disabled]", "[loading]", "tabindex"] {
        assert!(!def.contains(absent), "{absent:?} in:\n{def}");
    }
    assert!(compact(&def).contains("tone: \"Soft\" | \"Loud\" = \"Soft\";"), "{def}");
}

#[test]
fn an_instance_in_a_pseudo_class_state_passes_no_state_attribute() {
    // covers: E6-2.10
    let mut s = Session::new();
    let set = state_set(&mut s, "Control", &[("State=Default", "#ff0000"), ("State=Hover", "#0000ff"), ("State=Disabled", "#808080")]);
    let hovered = idv(&ex(&mut s, "instance.create", json!({"component": set, "x": 400, "y": 0})));
    ex(&mut s, "instance.setProperties", json!({"ids": [hovered], "properties": {"State": "Hover"}}));
    let usage = component_code(&mut s, hovered)["usage"].as_str().unwrap_or_default().to_string();
    assert!(usage.starts_with("<ui-control></ui-control>"), "{usage}");

    let disabled = idv(&ex(&mut s, "instance.create", json!({"component": set, "x": 400, "y": 100})));
    ex(&mut s, "instance.setProperties", json!({"ids": [disabled], "properties": {"State": "Disabled"}}));
    let usage = component_code(&mut s, disabled)["usage"].as_str().unwrap_or_default().to_string();
    assert!(usage.starts_with("<ui-control disabled></ui-control>"), "{usage}");
}
