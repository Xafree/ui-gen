//! Lit tokens beyond the locked tests: the variants of a set read the same prefixed custom
//! properties, and the mode options that name one mode twice.

use crate::Session;
use crate::components::tests::{ex, idv};
use crate::lit_export::{TokenOptions, tokens_css};
use figcraft_doc::{Collection, NodeId};
use serde_json::{Value, json};

fn style_set(s: &mut Session) -> NodeId {
    ex(s, "variable.createCollection", json!({"name": "Colors", "modes": ["Base"]}));
    ex(s, "variable.create", json!({"collection": "Colors", "name": "Colors/Accent", "type": "color", "value": "#3366FF"}));
    ex(s, "variable.update", json!({"variable": "Colors/Accent", "codeSyntax": {"WEB": "var(--accent)"}}));
    let mut variants = Vec::new();
    for variant in ["Style=Primary", "Style=Secondary"] {
        let f = idv(&ex(s, "frame.create", json!({"x": 0, "y": 0, "width": 100, "height": 40, "name": variant})));
        let rect = idv(&ex(
            s,
            "shape.rectangle",
            json!({"x": 0, "y": 0, "width": 100, "height": 40, "parent": f, "props": {"name": "Bg", "fill": "#ff0000"}}),
        ));
        if variant.ends_with("Secondary") {
            ex(s, "variable.bind", json!({"ids": [rect], "field": "fill", "variable": "Colors/Accent"}));
        }
        variants.push(idv(&ex(s, "component.create", json!({"ids": [f]}))));
    }
    let set = idv(&ex(s, "component.combineVariants", json!({"ids": variants})));
    ex(s, "node.set", json!({"id": set, "props": {"name": "Chip"}}));
    set
}

fn compact(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn a_variant_reads_the_prefixed_property_and_its_ignored_syntax_is_noted() {
    // covers: E6-3.1
    // covers: E6-3.2
    let mut s = Session::new();
    let set = style_set(&mut s);
    let v: Value = ex(&mut s, "inspect.componentCode", json!({"id": set, "language": "lit"}));
    let def = compact(v["definition"].as_str().unwrap_or_default());
    assert!(def.contains(":host([style-variant=\"Secondary\"]) .bg { background: var(--ui-colors-accent); }"), "{def}");
    assert!(!def.contains("--accent)"), "{def}");
    let notes = v["notes"].as_array().cloned().unwrap_or_default();
    let ignored: Vec<&Value> = notes.iter().filter(|n| n["code"] == "code-syntax-ignored").collect();
    assert_eq!(ignored.len(), 1, "one note for the variant layer: {notes:?}");
    assert_eq!(ignored.first().map(|n| n["layer"].clone()), Some(json!("Bg")), "{notes:?}");
}

#[test]
fn a_dark_mode_that_is_the_light_mode_is_no_dark_mode() {
    // covers: E6-3.5
    let mut s = Session::new();
    ex(&mut s, "variable.createCollection", json!({"name": "Theme", "modes": ["Light", "Dark"]}));
    ex(&mut s, "variable.create", json!({"collection": "Theme", "name": "Bg", "type": "color", "values": {"Light": "#FFFFFF", "Dark": "#000000"}}));
    let doc = s.doc().unwrap();
    let found: Vec<&Collection> = doc.variables.collection("Theme").into_iter().collect();
    let options = TokenOptions { prefix: "ui".to_string(), light: Some("Dark".to_string()), dark: Some("Dark".to_string()) };
    let tokens = tokens_css(doc, &found, &options);
    assert!(!tokens.text.contains("prefers-color-scheme") && !tokens.text.contains("[data-theme=\"dark\"]"), "{}", tokens.text);
    assert!(tokens.notes.iter().any(|n| n.code == "no-dark-mode"), "{:?}", tokens.notes);
    assert!(tokens.text.contains("--ui-bg: #000000;"), "{}", tokens.text);
}
