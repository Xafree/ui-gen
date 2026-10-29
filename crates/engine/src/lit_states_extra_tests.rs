//! Lit states beyond the locked tests: the Focus and Loading plumbing, instances of unrecognised
//! states, the properties `inspect.componentCode` lists, rest variants and the fallback base.

use crate::Session;
use crate::components::tests::{ex, idv};
use figcraft_doc::*;
use serde_json::{Value, json};

fn component_code(s: &mut Session, id: NodeId, language: &str) -> Value {
    ex(s, "inspect.componentCode", json!({"id": id, "language": language}))
}

fn definition(s: &mut Session, id: NodeId) -> String {
    component_code(s, id, "lit")["definition"].as_str().unwrap_or_default().to_string()
}

fn compact(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").replace("> <", "><")
}

fn state_set(s: &mut Session, rows: &[(&str, &str)]) -> NodeId {
    let mut variants = Vec::new();
    for (k, (variant, fill)) in rows.iter().enumerate() {
        let f = idv(&ex(s, "frame.create", json!({"x": k as f64 * 150.0, "y": 0, "width": 100, "height": 40, "name": variant})));
        ex(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 40, "parent": f, "props": {"name": "Bg", "fill": fill}}));
        variants.push(idv(&ex(s, "component.create", json!({"ids": [f]}))));
    }
    let set = idv(&ex(s, "component.combineVariants", json!({"ids": variants})));
    ex(s, "node.set", json!({"id": set, "props": {"name": "Control"}}));
    set
}

fn usage_of(s: &mut Session, set: NodeId, state: &str) -> String {
    let instance = idv(&ex(s, "instance.create", json!({"component": set, "x": 400, "y": 0})));
    ex(s, "instance.setProperties", json!({"ids": [instance], "properties": {"State": state}}));
    component_code(s, instance, "lit")["usage"].as_str().unwrap_or_default().to_string()
}

fn property<'a>(v: &'a Value, name: &str) -> &'a Value {
    v["properties"].as_array().and_then(|all| all.iter().find(|p| p["name"] == name)).unwrap_or(&Value::Null)
}

#[test]
fn focus_with_a_disabled_state_syncs_tabindex_and_aria_disabled() {
    // covers: E6-2.8
    let mut s = Session::new();
    let set = state_set(&mut s, &[("State=Default", "#ff0000"), ("State=Focus", "#00ff00"), ("State=Disabled", "#808080")]);
    let def = compact(&definition(&mut s, set));
    for want in [
        "connectedCallback() { super.connectedCallback(); this.syncFocus(); }",
        "updated(changed: Map<PropertyKey, unknown>) { if (changed.has(\"disabled\")) this.syncFocus(); }",
        "this.setAttribute(\"tabindex\", this.disabled ? \"-1\" : \"0\");",
        "this.setAttribute(\"aria-disabled\", String(this.disabled));",
    ] {
        assert!(def.contains(want), "{want:?} in:\n{def}");
    }
}

#[test]
fn focus_without_a_disabled_state_only_sets_a_tabindex() {
    // covers: E6-2.8
    let mut s = Session::new();
    let set = state_set(&mut s, &[("State=Default", "#ff0000"), ("State=Focus", "#00ff00")]);
    let def = compact(&definition(&mut s, set));
    assert!(def.contains("if (!this.hasAttribute(\"tabindex\")) this.setAttribute(\"tabindex\", \"0\");"), "{def}");
    assert!(!def.contains("aria-disabled") && !def.contains("syncFocus"), "{def}");
}

#[test]
fn an_instance_loading_sets_the_loading_attribute_and_pressed_sets_none() {
    // covers: E6-2.10
    let mut s = Session::new();
    let set = state_set(&mut s, &[("State=Default", "#ff0000"), ("State=Pressed", "#ffff00"), ("State=Loading", "#00ffff")]);
    assert!(usage_of(&mut s, set, "Loading").starts_with("<ui-control loading></ui-control>"));
    assert!(usage_of(&mut s, set, "Pressed").starts_with("<ui-control></ui-control>"));
    assert!(usage_of(&mut s, set, "Default").starts_with("<ui-control></ui-control>"));
}

#[test]
fn an_instance_in_an_unrecognised_state_keeps_the_attribute_and_a_recognised_one_drops_it() {
    // covers: E6-2.5
    let mut s = Session::new();
    let set =
        state_set(&mut s, &[("State=Default", "#ff0000"), ("State=Hover", "#0000ff"), ("State=Disabled", "#808080"), ("State=Error", "#ff00ff")]);
    assert!(usage_of(&mut s, set, "Error").starts_with("<ui-control state=\"Error\"></ui-control>"));
    assert!(usage_of(&mut s, set, "Hover").starts_with("<ui-control></ui-control>"));
    assert!(usage_of(&mut s, set, "Disabled").starts_with("<ui-control disabled></ui-control>"));
    let def = compact(&definition(&mut s, set));
    assert!(def.contains("state: \"Default\" | \"Error\" = \"Default\";"), "{def}");
}

#[test]
fn the_properties_list_the_booleans_the_states_add() {
    // covers: E6-2.2
    let mut s = Session::new();
    let set = state_set(&mut s, &[("State=Default", "#ff0000"), ("State=Disabled", "#808080"), ("State=Loading", "#00ffff")]);
    let v = component_code(&mut s, set, "lit");
    let names: Vec<&str> = v["properties"].as_array().map(|all| all.iter().filter_map(|p| p["name"].as_str()).collect()).unwrap_or_default();
    assert_eq!(names, ["disabled", "loading"], "{v}");
    assert_eq!(property(&v, "disabled")["attribute"], "disabled");
    assert_eq!(property(&v, "disabled")["reflect"], true);
    assert_eq!(property(&v, "loading")["default"], false);

    let instance = idv(&ex(&mut s, "instance.create", json!({"component": set, "x": 400, "y": 0})));
    ex(&mut s, "instance.setProperties", json!({"ids": [instance], "properties": {"State": "Loading"}}));
    let v = component_code(&mut s, instance, "lit");
    assert_eq!(property(&v, "loading")["value"], true, "{v}");
    assert_eq!(property(&v, "disabled")["value"], false, "{v}");
}

#[test]
fn a_rest_variant_of_another_style_gets_the_style_rule_only_once() {
    // covers: E6-2.3
    let mut s = Session::new();
    let rows =
        [("Style=Primary, State=Default", "#ff0000"), ("Style=Secondary, State=Default", "#00ff00"), ("Style=Secondary, State=Normal", "#0000ff")];
    let set = state_set(&mut s, &rows);
    let text = compact(&definition(&mut s, set));
    assert_eq!(text.matches(":host([style-variant=\"Secondary\"]) .").count(), 1, "{text}");
    assert!(text.contains("background: #00FF00;") && !text.contains("#0000FF"), "{text}");
}

#[test]
fn a_state_without_a_rest_sibling_differs_from_the_default_variant() {
    // covers: E6-2.4
    let mut s = Session::new();
    let rows = [("Style=Primary, State=Default", "#ff0000"), ("Style=Primary, State=Hover", "#0000ff"), ("Style=Secondary, State=Hover", "#00ff00")];
    let set = state_set(&mut s, &rows);
    let text = compact(&definition(&mut s, set));
    assert!(text.contains(":host([style-variant=\"Secondary\"]:hover) .") && text.contains("background: #00FF00;"), "{text}");
    assert!(text.contains(":host([style-variant=\"Primary\"]:hover) ."), "{text}");
}

#[test]
fn an_unrecognised_state_is_compared_to_its_style_at_rest() {
    // covers: E6-2.5
    let mut s = Session::new();
    let rows = [
        ("Style=Primary, State=Default", "#ff0000"),
        ("Style=Secondary, State=Default", "#00ff00"),
        ("Style=Secondary, State=Error", "#00ff00"),
        ("Style=Primary, State=Error", "#ff00ff"),
    ];
    let set = state_set(&mut s, &rows);
    let text = compact(&definition(&mut s, set));
    assert!(text.contains(":host([style-variant=\"Primary\"][state=\"Error\"]) .") && text.contains("#FF00FF"), "{text}");
    assert!(!text.contains(":host([style-variant=\"Secondary\"][state=\"Error\"])"), "same as Secondary at rest: no rule\n{text}");
}

#[test]
fn the_other_languages_keep_the_state_as_a_property() {
    // covers: E6-1.10
    let mut s = Session::new();
    let set = state_set(&mut s, &[("State=Default", "#ff0000"), ("State=Hover", "#0000ff")]);
    let v = component_code(&mut s, set, "react");
    assert!(v["definition"].as_str().unwrap_or_default().contains("data-state"), "{v}");
    assert_eq!(property(&v, "State")["type"], "variant");
    assert!(v["properties"][0].get("attribute").is_none(), "{v}");
}
