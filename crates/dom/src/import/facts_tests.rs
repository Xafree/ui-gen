use figcraft_color::Color;
use serde_json::{Map, Value, json};

use super::support::bounds;
use super::translate_support::{Translation, translate_root};
use crate::{LayerFacts, TokenFact, TokenValue};

const PRIMARY: &str = "rgb(25, 118, 210)";
const BORDER: &str = "rgb(13, 71, 161)";
const WHITE: &str = "rgb(255, 255, 255)";

pub(super) fn facts_of<'a>(run: &'a Translation, name: &str) -> &'a LayerFacts {
    let id = run.by_name(name).id;
    run.translated.layers.iter().find(|layer| layer.id == id).unwrap_or_else(|| panic!("no facts for the layer {name:?}"))
}

fn fact<'a>(layer: &'a LayerFacts, field: &str) -> &'a TokenFact {
    layer.tokens.iter().find(|fact| fact.field == field).unwrap_or_else(|| panic!("no {field:?} fact among {:?}", layer.tokens))
}

fn colour(css: &str) -> TokenValue {
    TokenValue::Color(Color::parse_css(css).unwrap())
}

fn button(overrides: Value, vars: Value) -> Value {
    let mut style = Map::new();
    for (key, value) in [
        ("display", "flex"),
        ("flexDirection", "row"),
        ("justifyContent", "flex-start"),
        ("alignItems", "flex-start"),
        ("columnGap", "8px"),
        ("rowGap", "0px"),
        ("paddingTop", "8px"),
        ("paddingRight", "16px"),
        ("paddingBottom", "8px"),
        ("paddingLeft", "16px"),
        ("backgroundColor", PRIMARY),
        ("borderTopLeftRadius", "6px"),
        ("borderTopRightRadius", "6px"),
        ("borderBottomRightRadius", "6px"),
        ("borderBottomLeftRadius", "6px"),
    ] {
        style.insert(key.to_owned(), json!(value));
    }
    for side in ["Top", "Right", "Bottom", "Left"] {
        style.insert(format!("border{side}Width"), json!("1px"));
        style.insert(format!("border{side}Style"), json!("solid"));
        style.insert(format!("border{side}Color"), json!(BORDER));
    }
    style.extend(overrides.as_object().cloned().unwrap_or_default());
    let icon =
        json!({ "type": "element", "tag": "span", "label": "Icon", "box": bounds(36.0, 28.0, 16.0, 16.0), "style": { "backgroundColor": WHITE } });
    let text = json!({ "type": "text", "text": "Go", "box": bounds(60.0, 28.0, 50.0, 20.0), "style": { "color": WHITE, "fontSize": "14px" } });
    json!({
        "type": "element", "tag": "button", "label": "Button", "box": bounds(20.0, 20.0, 106.0, 36.0),
        "style": style, "vars": vars, "children": [icon, text]
    })
}

// covers: E3-4.1
#[test]
fn a_button_gives_one_token_fact_per_value_with_the_field_to_bind_and_the_value_of_the_layer() {
    let run = translate_root(button(json!({}), json!({})));

    let root = facts_of(&run, "Button");
    let radius = TokenValue::Number(6.0);
    let expected = [
        ("fill", colour(PRIMARY)),
        ("stroke", colour(BORDER)),
        ("strokeWeight", TokenValue::Number(1.0)),
        ("topLeftRadius", radius),
        ("topRightRadius", radius),
        ("bottomRightRadius", radius),
        ("bottomLeftRadius", radius),
        ("paddingTop", TokenValue::Number(8.0)),
        ("paddingRight", TokenValue::Number(16.0)),
        ("paddingBottom", TokenValue::Number(8.0)),
        ("paddingLeft", TokenValue::Number(16.0)),
        ("gap", TokenValue::Number(8.0)),
    ];
    for (field, value) in expected {
        assert_eq!(fact(root, field).value, value, "{field}");
    }
    let mut fields: Vec<&str> = root.tokens.iter().map(|fact| fact.field.as_str()).collect();
    let count = fields.len();
    fields.sort_unstable();
    fields.dedup();
    assert_eq!(fields.len(), count, "a field has one fact: {fields:?}");
    assert!(root.tokens.iter().all(|fact| fact.exposed.is_none()), "{:?}", root.tokens);
    assert_eq!(fact(facts_of(&run, "Label"), "fill").value, colour(WHITE), "the text colour binds the fill of the text layer");
}

// covers: E3-4.1
#[test]
fn a_variable_named_by_vars_is_exposed_on_the_fact_of_that_value_only() {
    let run = translate_root(button(json!({}), json!({ "backgroundColor": "--syn-primary" })));

    let root = facts_of(&run, "Button");
    let background = fact(root, "fill");
    assert_eq!(background.exposed.as_deref(), Some("--syn-primary"));
    assert_eq!(background.value, colour(PRIMARY));
    for other in root.tokens.iter().filter(|fact| fact.field != "fill") {
        assert_eq!(other.exposed, None, "{}", other.field);
    }
    for name in ["Icon", "Label"] {
        assert!(facts_of(&run, name).tokens.iter().all(|fact| fact.exposed.is_none()), "{name}");
    }
}

// covers: E3-4.1
#[test]
fn a_value_the_layer_could_not_read_gives_no_fact_and_the_others_stay() {
    let run = translate_root(button(json!({ "backgroundColor": "nonsense(1, 2)" }), json!({})));

    let root = facts_of(&run, "Button");
    assert!(root.tokens.iter().all(|fact| fact.field != "fill"), "{:?}", root.tokens);
    assert_eq!(fact(root, "stroke").value, colour(BORDER));
    assert_eq!(fact(root, "gap").value, TokenValue::Number(8.0));
}
