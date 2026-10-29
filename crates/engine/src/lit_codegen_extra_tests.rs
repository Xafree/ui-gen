//! Lit codegen beyond the locked tests: tag names, member and attribute uniqueness, instances
//! with slots and swaps, escaping in attribute values, and the `prefix` parameter.

use crate::Session;
use crate::components::tests::{ex, idv};
use crate::lit::{self, LitOptions, TagIssue};
use figcraft_doc::*;
use serde_json::{Value, json};

fn component_code(s: &mut Session, id: NodeId, extra: Value) -> Value {
    let mut p = json!({"id": id, "language": "lit"});
    if let (Some(base), Some(more)) = (p.as_object_mut(), extra.as_object()) {
        base.extend(more.clone());
    }
    ex(s, "inspect.componentCode", p)
}

fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_string()
}

fn compact(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").replace("> <", "><")
}

fn card_with_slot(s: &mut Session) -> NodeId {
    let frame = idv(&ex(s, "frame.create", json!({"x": 0, "y": 0, "width": 200, "height": 100, "name": "Shell"})));
    let body = idv(&ex(s, "frame.create", json!({"x": 0, "y": 0, "width": 200, "height": 100, "parent": frame, "name": "Body"})));
    ex(s, "text.create", json!({"text": "Empty", "x": 0, "y": 0, "parent": body, "props": {"name": "Hint"}}));
    let shell = idv(&ex(s, "component.create", json!({"ids": [frame]})));
    ex(s, "component.makeSlot", json!({"id": body, "name": "Content"}));
    shell
}

#[test]
fn tag_names_are_prefixed_slugs_and_say_why_they_fail() {
    // covers: E6-1.1
    assert_eq!(lit::tag_name("Button / Primary", "ui-"), Ok("ui-button-primary".to_string()));
    assert_eq!(lit::tag_name("Über Größe", "x-"), Ok("x-uber-grosse".to_string()));
    assert_eq!(lit::tag_name("###", "ui-"), Err(TagIssue::Empty));
    assert_eq!(lit::tag_name("3D Card", ""), Err(TagIssue::LeadingDigit));
    assert_eq!(lit::tag_name("3D Card", "ui-"), Ok("ui-3d-card".to_string()));
    assert_eq!(lit::tag_name("Font Face", ""), Err(TagIssue::Reserved));
    assert_eq!(lit::tag_name("Button", ""), Err(TagIssue::NoHyphen));
    assert_eq!(lit::tag_name(&"a".repeat(80), "ui-"), Err(TagIssue::TooLong));
    assert_eq!(lit::tag_name("Button", "UI_"), Err(TagIssue::NoHyphen), "characters outside [a-z0-9-] are dropped");
    assert!(lit::valid_prefix("") && lit::valid_prefix("acme-") && !lit::valid_prefix("Acme-") && !lit::valid_prefix("a b"));
    assert_eq!(LitOptions::new("acme-").token_prefix, "acme");
}

#[test]
fn the_prefix_parameter_names_the_tag_and_is_validated() {
    // covers: E6-1.1
    let mut s = Session::new();
    let set = card_with_slot(&mut s);
    let v = component_code(&mut s, set, json!({"prefix": "acme-"}));
    assert_eq!(v["tag"], "acme-shell");
    assert_eq!(v["className"], "AcmeShell");
    assert!(text(&v, "definition").contains("@customElement(\"acme-shell\")"));
    assert!(v["notes"].as_array().is_some_and(Vec::is_empty));
    for bad in ["Acme", "a b", "a\"b", &"x".repeat(30)] {
        assert!(s.execute("inspect.componentCode", &json!({"id": set, "language": "lit", "prefix": bad})).is_err(), "{bad:?} is refused");
    }
    let other = ex(&mut s, "inspect.componentCode", json!({"id": set, "language": "react"}));
    assert!(other["tag"].is_null() && other["notes"].is_null(), "only Lit reports a tag");
}

#[test]
fn a_name_that_makes_no_tag_falls_back_to_component_and_hugging_roots_are_inline_blocks() {
    // covers: E6-1.1
    let mut s = Session::new();
    let f = idv(&ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 100, "height": 40, "name": "###"})));
    ex(&mut s, "layout.add", json!({"ids": [f]}));
    ex(&mut s, "node.set", json!({"id": f, "props": {"sizingH": "hug"}}));
    let comp = idv(&ex(&mut s, "component.create", json!({"ids": [f]})));
    let v = component_code(&mut s, comp, json!({}));
    assert_eq!(v["tag"], "ui-component");
    let def = compact(&text(&v, "definition"));
    assert!(def.contains(":host { display: inline-block; }"), "{def}");
}

#[test]
fn colliding_names_stay_unique_and_acronyms_make_readable_attributes() {
    // covers: E6-1.3
    let mut s = Session::new();
    let f = idv(&ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 100, "height": 100, "name": "Probe"})));
    let layers: Vec<NodeId> = ["A", "B", "C", "D"]
        .iter()
        .enumerate()
        .map(|(k, n)| idv(&ex(&mut s, "text.create", json!({"text": "x", "x": 0, "y": k as f64 * 10.0, "parent": f, "props": {"name": n}}))))
        .collect();
    let comp = idv(&ex(&mut s, "component.create", json!({"ids": [f]})));
    for (name, layer) in [("Title", layers[0]), ("Title value", layers[1]), ("Show URL", layers[2]), ("Show Url", layers[3])] {
        ex(&mut s, "component.addProperty", json!({"id": comp, "name": name, "type": "text", "layer": layer}));
    }
    let v = component_code(&mut s, comp, json!({}));
    let attrs: Vec<String> = v["properties"].as_array().unwrap_or(&Vec::new()).iter().map(|p| text(p, "attribute")).collect();
    let mut unique = attrs.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), attrs.len(), "attributes are distinct: {attrs:?}");
    assert!(attrs.iter().any(|a| a == "show-url") && attrs.iter().any(|a| a == "show-url-2"), "{attrs:?}");
    let def = text(&v, "definition");
    for member in ["titleValue", "titleValue2"] {
        assert_eq!(def.matches(&format!(") {member} =")).count(), 1, "{member} once:\n{def}");
    }
}

#[test]
fn an_instance_passes_booleans_swaps_and_slot_content_as_markup() {
    // covers: E6-1.8
    let mut s = Session::new();
    let shell = card_with_slot(&mut s);
    let heart = {
        let f = idv(&ex(&mut s, "frame.create", json!({"x": 0, "y": 400, "width": 24, "height": 24, "name": "Heart"})));
        ex(&mut s, "shape.star", json!({"x": 0, "y": 0, "width": 24, "height": 24, "parent": f}));
        idv(&ex(&mut s, "component.create", json!({"ids": [f]})))
    };
    let star = {
        let f = idv(&ex(&mut s, "frame.create", json!({"x": 40, "y": 400, "width": 24, "height": 24, "name": "Star"})));
        ex(&mut s, "shape.star", json!({"x": 0, "y": 0, "width": 24, "height": 24, "parent": f}));
        idv(&ex(&mut s, "component.create", json!({"ids": [f]})))
    };
    let f = idv(&ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 120, "height": 40, "name": "Chip"})));
    let nested = idv(&ex(&mut s, "instance.create", json!({"component": heart, "x": 8, "y": 8, "parent": f})));
    let dot = idv(&ex(&mut s, "shape.ellipse", json!({"x": 80, "y": 0, "width": 10, "height": 10, "parent": f, "props": {"name": "Dot"}})));
    let label = idv(&ex(&mut s, "text.create", json!({"text": "x", "x": 0, "y": 20, "parent": f, "props": {"name": "Label"}})));
    let chip = idv(&ex(&mut s, "component.create", json!({"ids": [f]})));
    ex(&mut s, "component.addProperty", json!({"id": chip, "name": "Icon", "type": "instanceSwap", "default": heart, "layer": nested}));
    ex(&mut s, "component.addProperty", json!({"id": chip, "name": "Dot", "type": "boolean", "layer": dot, "default": true}));
    ex(&mut s, "component.addProperty", json!({"id": chip, "name": "Mute", "type": "boolean", "layer": dot, "default": false}));
    ex(&mut s, "component.addProperty", json!({"id": chip, "name": "Label", "type": "text", "layer": label}));
    let i = idv(&ex(&mut s, "instance.create", json!({"component": chip, "x": 400, "y": 0})));
    ex(&mut s, "instance.setProperties", json!({"ids": [i], "properties": {"Icon": star, "Dot": false, "Mute": true, "Label": "a\"<b>&`${x}\\"}}));
    let v = component_code(&mut s, i, json!({}));
    let usage = text(&v, "usage");
    assert!(usage.starts_with("<ui-chip "), "{usage}");
    for want in [" dot=\"false\"", " mute", "label=\"a&quot;&lt;b&gt;&amp;`${x}\\\"", "<ui-star slot=\"icon\"></ui-star>"] {
        assert!(usage.contains(want), "{want:?} in:\n{usage}");
    }

    let instance = idv(&ex(&mut s, "instance.create", json!({"component": shell, "x": 400, "y": 200})));
    let slot_layer = s.doc().unwrap().node(instance).unwrap().children.first().copied().unwrap();
    ex(&mut s, "text.create", json!({"text": "In `${slot}` \\", "x": 0, "y": 0, "parent": slot_layer, "props": {"name": "Mine"}}));
    let usage = text(&component_code(&mut s, instance, json!({})), "usage");
    assert!(usage.starts_with("<ui-shell>\n") && usage.contains("<div slot=\"content\" style=\"display: contents\">"), "{usage}");
    assert!(usage.contains("In `${slot}` \\"), "plain HTML outside a template:\n{usage}");

    let host = idv(&ex(&mut s, "frame.create", json!({"x": 0, "y": 600, "width": 300, "height": 300, "name": "Host"})));
    ex(&mut s, "instance.create", json!({"component": shell, "x": 0, "y": 0, "parent": host}));
    let nested_slot = s.doc().unwrap().node(host).unwrap().children.first().copied().unwrap();
    let inner = s.doc().unwrap().node(nested_slot).unwrap().children.first().copied().unwrap();
    ex(&mut s, "text.create", json!({"text": "In `${slot}` \\", "x": 0, "y": 0, "parent": inner, "props": {"name": "Mine"}}));
    let host_comp = idv(&ex(&mut s, "component.create", json!({"ids": [host]})));
    let def = text(&component_code(&mut s, host_comp, json!({})), "definition");
    assert!(def.contains("In \\`\\${slot}\\` \\\\"), "escaped in a template:\n{def}");
}

#[test]
fn a_plain_layer_tree_is_a_lit_element_too() {
    // covers: E6-1.1
    let mut s = Session::new();
    let f = idv(&ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 100, "height": 40, "name": "Plain frame"})));
    ex(&mut s, "text.create", json!({"text": "Hi `${x}` \\ <b>", "x": 0, "y": 0, "parent": f, "props": {"name": "T"}}));
    let out = ex(&mut s, "inspect.code", json!({"id": f, "language": "lit"}));
    let code = text(&out, "code");
    assert!(code.contains("@customElement(\"ui-component\")") || code.contains("@customElement(\"ui-plain-frame\")"), "{code}");
    assert!(code.contains("Hi \\`\\${x}\\` \\\\ &lt;b&gt;"), "{code}");
    assert!(!code.contains("@property"), "{code}");
}

#[test]
fn absurd_lit_parameters_never_panic() {
    // covers: E6-1.8
    let mut s = Session::new();
    let shell = card_with_slot(&mut s);
    for params in [
        json!({"id": 999_999, "language": "lit"}),
        json!({"id": shell, "language": 7}),
        json!({"id": shell, "language": "lit", "prefix": 3}),
        json!({"id": shell, "language": "lit", "prefix": null}),
        json!({"language": "lit"}),
    ] {
        for cmd in ["inspect.componentCode", "inspect.code"] {
            let _ = s.execute(cmd, &params);
        }
    }
}
