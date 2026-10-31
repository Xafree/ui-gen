//! A variant rule of a Lit component adds to the rule of the default variant, so a variant that lacks what the default
//! one sets has to take it back, or it shows through (a ghost button with the background of the default one).

use crate::Session;
use crate::components::tests::{ex, idv};
use figcraft_doc::NodeId;
use serde_json::{Value, json};

fn variant(s: &mut Session, name: &str, x: f64, props: Value, label: Value) -> NodeId {
    let f = idv(&ex(s, "frame.create", json!({"x": x, "y": 0, "width": 100, "height": 40, "name": name})));
    ex(s, "node.set", json!({"id": f, "props": props}));
    ex(s, "text.create", json!({"text": "Go", "x": 8, "y": 8, "parent": f, "props": label}));
    idv(&ex(s, "component.create", json!({"ids": [f]})))
}

fn definition(s: &mut Session, ids: Vec<NodeId>) -> String {
    let set = idv(&ex(s, "component.combineVariants", json!({"ids": ids})));
    ex(s, "node.set", json!({"id": set, "props": {"name": "Chip"}}));
    ex(s, "inspect.componentCode", json!({"id": set, "language": "lit"}))["definition"].as_str().unwrap_or_default().to_string()
}

fn compact(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn body_of(def: &str, selector: &str) -> String {
    let text = compact(def);
    let start = text.find(&format!("{selector} {{")).unwrap_or_else(|| panic!("no rule {selector} in:\n{def}"));
    let rest = &text[start..];
    rest[rest.find('{').unwrap() + 1..].split('}').next().unwrap().trim().to_string()
}

fn solid() -> Value {
    json!({
        "fill": "#cc0000",
        "stroke": "#00cc00",
        "cornerRadius": 8,
        "opacity": 0.5,
        "effects": [{"type": "dropShadow", "offset": [0, 2], "radius": 4, "color": "#00000080"}],
    })
}

#[test]
fn a_variant_without_what_the_default_variant_sets_takes_each_property_back() {
    let mut s = Session::new();
    let base = variant(&mut s, "Tone=solid", 0.0, solid(), json!({"name": "Label", "fill": "#111111"}));
    let ghost = variant(&mut s, "Tone=ghost", 200.0, json!({"fills": []}), json!({"name": "Label", "fills": []}));
    let def = definition(&mut s, vec![base, ghost]);
    let frame = body_of(&def, ":host([tone=\"ghost\"]) .chip");
    for wanted in ["background: transparent;", "border: none;", "box-shadow: none;", "opacity: 1;", "border-radius: 0;"] {
        assert!(frame.contains(wanted), "{wanted} is missing from: {frame}");
    }
    let label = body_of(&def, ":host([tone=\"ghost\"]) .label");
    assert!(label.contains("color: transparent;"), "{label}");
}

#[test]
fn a_state_without_what_an_earlier_state_of_the_variant_sets_takes_it_back() {
    let mut s = Session::new();
    let plain = || json!({"fills": []});
    let label = || json!({"name": "Label", "fill": "#111111"});
    let rest = variant(&mut s, "State=Default", 0.0, plain(), label());
    let hover = variant(&mut s, "State=Hover", 200.0, json!({"fill": "#eeeeee"}), label());
    let pressed = variant(&mut s, "State=Pressed", 400.0, plain(), label());
    let def = definition(&mut s, vec![rest, hover, pressed]);
    assert!(body_of(&def, ":host(:hover) .chip").contains("background: #EEEEEE;"), "{def}");
    let pressed = body_of(&def, ":host(:active) .chip");
    assert!(pressed.contains("background: transparent;"), "pressing a hovered element must clear the hover fill: {pressed}");
}

#[test]
fn a_variant_that_only_adds_resets_nothing() {
    let mut s = Session::new();
    let base = variant(&mut s, "Tone=ghost", 0.0, json!({"fills": []}), json!({"name": "Label", "fill": "#111111"}));
    let solid = variant(&mut s, "Tone=solid", 200.0, json!({"fill": "#cc0000"}), json!({"name": "Label", "fill": "#111111"}));
    let def = definition(&mut s, vec![base, solid]);
    let frame = body_of(&def, ":host([tone=\"solid\"]) .chip");
    assert_eq!(frame, "background: #CC0000;");
}

#[test]
fn the_box_of_every_element_includes_its_padding_and_a_paragraph_keeps_no_margin() {
    let mut s = Session::new();
    let base = variant(&mut s, "Tone=solid", 0.0, solid(), json!({"name": "Label", "fill": "#111111"}));
    let def = compact(&definition(&mut s, vec![base]));
    assert!(def.contains("* { box-sizing: border-box; margin: 0; }"), "{def}");
}

#[test]
fn an_opacity_variable_holds_a_percentage_and_the_css_divides_it() {
    let mut s = Session::new();
    ex(&mut s, "variable.createCollection", json!({"name": "Theme", "modes": ["Light"]}));
    ex(&mut s, "variable.create", json!({"collection": "Theme", "name": "disabled", "type": "number", "values": {"Light": 50}}));
    let f = variant(&mut s, "Tone=solid", 0.0, json!({"fill": "#cc0000"}), json!({"name": "Label", "fill": "#111111"}));
    ex(&mut s, "variable.bind", json!({"ids": [f], "field": "opacity", "variable": "disabled"}));
    let def = compact(&definition(&mut s, vec![f]));
    assert!(def.contains("opacity: calc(var(--ui-disabled) / 100);"), "{def}");
}

#[test]
fn the_shadow_drawn_on_top_comes_first_in_css() {
    let mut s = Session::new();
    let ring = |spread: u32, color: &str| json!({"type": "dropShadow", "offset": [0, 0], "radius": 0, "spread": spread, "color": color});
    let effects = json!({"effects": [ring(4, "#0000ff"), ring(2, "#ffffff")]});
    let f = variant(&mut s, "Tone=solid", 0.0, effects, json!({"name": "Label", "fill": "#111111"}));
    let def = compact(&definition(&mut s, vec![f]));
    let (inner, outer) = (def.find("2px #FFFFFF").unwrap_or_else(|| panic!("{def}")), def.find("4px #0000FF").unwrap_or_else(|| panic!("{def}")));
    assert!(inner < outer, "the canvas paints the last effect on top: {def}");
}

#[test]
fn a_component_with_a_focus_state_draws_its_focus_itself() {
    let mut s = Session::new();
    let label = || json!({"name": "Label", "fill": "#111111"});
    let rest = variant(&mut s, "State=Default", 0.0, json!({"fill": "#cc0000"}), label());
    let focus = variant(&mut s, "State=Focus", 200.0, json!({"fill": "#aa0000"}), label());
    let def = compact(&definition(&mut s, vec![rest, focus]));
    assert!(def.contains(":host(:focus-visible) { outline: none; }"), "the browser ring would cover the design's: {def}");
}

fn rectangle(s: &mut Session, parent: NodeId, name: &str, fill: &str) {
    ex(s, "shape.rectangle", json!({"x": 4, "y": 4, "width": 20, "height": 20, "parent": parent, "props": {"name": name, "fill": fill}}));
}

#[test]
fn a_layer_only_some_variants_have_is_in_the_markup_and_shows_only_in_them() {
    let mut s = Session::new();
    let label = || json!({"name": "Label", "fill": "#111111"});
    let rest = variant(&mut s, "State=Default", 0.0, json!({"fill": "#cc0000"}), label());
    let f = idv(&ex(&mut s, "frame.create", json!({"x": 200, "y": 0, "width": 100, "height": 40, "name": "State=Pressed"})));
    rectangle(&mut s, f, "Ripple", "#ffffff");
    ex(&mut s, "text.create", json!({"text": "Go", "x": 8, "y": 8, "parent": f, "props": label()}));
    let pressed = idv(&ex(&mut s, "component.create", json!({"ids": [f]})));
    let def = definition(&mut s, vec![rest, pressed]);
    let text = compact(&def);
    assert!(text.contains("<div class=\"ripple\"></div>"), "the layer has an element: {def}");
    assert!(body_of(&def, ".ripple").contains("display: none;"), "hidden by default: {def}");
    assert!(body_of(&def, ":host(:active) .ripple").contains("display: revert;"), "shown while pressed: {def}");
    assert!(text.find("class=\"ripple\"") < text.find("class=\"label\""), "it keeps its place under the label: {def}");
}

#[test]
fn a_variant_without_a_layer_of_the_default_variant_hides_it() {
    let mut s = Session::new();
    let f = idv(&ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 100, "height": 40, "name": "Icon=on"})));
    rectangle(&mut s, f, "Glyph", "#ffffff");
    let on = idv(&ex(&mut s, "component.create", json!({"ids": [f]})));
    let off = variant(&mut s, "Icon=off", 200.0, json!({"fill": "#cc0000"}), json!({"name": "Label", "fill": "#111111"}));
    let def = definition(&mut s, vec![on, off]);
    assert!(body_of(&def, ":host([icon=\"off\"]) .glyph").contains("display: none;"), "{def}");
}

const SOURCE: &str = "{\"package\":\"@acme/ui\",\"version\":\"1.0.0\",\"component\":\"Button\"}";

fn imported_button(s: &mut Session, frame: Value, label: Value) -> String {
    let f = variant(s, "Size=md", 0.0, frame, label);
    let set = idv(&ex(s, "component.combineVariants", json!({"ids": [f]})));
    ex(s, "node.set", json!({"id": set, "props": {"name": "Button", "pluginData": {"uigen.source": SOURCE}}}));
    ex(s, "layout.add", json!({"ids": [f]}));
    ex(s, "node.set", json!({"id": f, "props": {"sizingH": "fixed", "sizingV": "fixed", "width": 100, "height": 40}}));
    ex(s, "inspect.componentCode", json!({"id": set, "language": "lit"}))["definition"].as_str().unwrap_or_default().to_string()
}

#[test]
fn an_imported_button_grows_with_a_longer_text_and_is_never_smaller_than_captured() {
    let mut s = Session::new();
    let def = imported_button(&mut s, json!({"fill": "#cc0000"}), json!({"name": "Label", "fill": "#111111"}));
    let body = body_of(&def, ".button");
    assert!(body.contains("min-width: 100px;") && body.contains("min-height: 40px;"), "{body}");
    assert!(!body.contains(" width: 100px;") && !body.starts_with("width:"), "{body}");
    assert!(compact(&def).contains(":host { display: inline-block; }"), "{def}");
}

#[test]
fn a_layer_of_a_fixed_size_that_was_not_imported_keeps_it() {
    let mut s = Session::new();
    let f = variant(&mut s, "Size=md", 0.0, json!({"fill": "#cc0000"}), json!({"name": "Label", "fill": "#111111"}));
    ex(&mut s, "layout.add", json!({"ids": [f]}));
    ex(&mut s, "node.set", json!({"id": f, "props": {"sizingH": "fixed", "sizingV": "fixed", "width": 100}}));
    let def = compact(&definition(&mut s, vec![f]));
    assert!(!def.contains("min-width"), "{def}");
}

#[test]
fn a_full_pill_stays_a_pill_at_any_height_and_a_rounded_box_keeps_its_radius() {
    let mut s = Session::new();
    let pill = variant(&mut s, "Shape=pill", 0.0, json!({"fill": "#cc0000", "cornerRadius": 20}), json!({"name": "Label", "fill": "#111111"}));
    let round = variant(&mut s, "Shape=round", 200.0, json!({"fill": "#cc0000", "cornerRadius": 8}), json!({"name": "Label", "fill": "#111111"}));
    let def = definition(&mut s, vec![pill, round]);
    assert!(body_of(&def, ".chip").contains("border-radius: 9999px;"), "{def}");
    assert!(body_of(&def, ":host([shape=\"round\"]) .chip").contains("border-radius: 8px;"), "{def}");
}

#[test]
fn a_text_takes_the_font_stack_of_its_library_and_a_hostile_one_is_ignored() {
    let mut s = Session::new();
    let stack = "system-ui, -apple-system, \"Segoe UI\", Roboto, sans-serif";
    let label = json!({"name": "Label", "fill": "#111111", "pluginData": {"uigen.fontStack": stack}});
    let f = variant(&mut s, "Size=md", 0.0, json!({"fill": "#cc0000"}), label);
    let def = definition(&mut s, vec![f]);
    assert!(compact(&def).contains(&format!("font-family: {stack};")), "{def}");

    let mut s = Session::new();
    let label = json!({"name": "Label", "fill": "#111111", "pluginData": {"uigen.fontStack": "Arial; } body { display: none"}});
    let f = variant(&mut s, "Size=md", 0.0, json!({"fill": "#cc0000"}), label);
    let def = definition(&mut s, vec![f]);
    assert!(!def.contains("display: none"), "{def}");
}
