//! A Lit component that can hold content gets a default `<slot>`: without it, the children a
//! template puts inside the tag never show.

use crate::Session;
use crate::components::tests::{ex, idv};
use figcraft_doc::NodeId;
use serde_json::json;

const DEFAULT_SLOT: &str = "<slot></slot>";

fn definition(s: &mut Session, id: NodeId) -> String {
    let code = ex(s, "inspect.componentCode", json!({"id": id, "language": "lit"}));
    code["definition"].as_str().unwrap_or_default().split_whitespace().collect::<Vec<_>>().join(" ").replace("> <", "><")
}

fn frame(s: &mut Session, name: &str) -> NodeId {
    idv(&ex(s, "frame.create", json!({"x": 0, "y": 0, "width": 200, "height": 100, "name": name})))
}

fn component(s: &mut Session, frame: NodeId) -> NodeId {
    idv(&ex(s, "component.create", json!({"ids": [frame]})))
}

#[test]
fn an_auto_layout_component_with_children_ends_with_a_default_slot() {
    let mut s = Session::new();
    let f = frame(&mut s, "Card");
    ex(&mut s, "text.create", json!({"text": "Title", "x": 0, "y": 0, "parent": f}));
    ex(&mut s, "layout.add", json!({"ids": [f]}));
    let card = component(&mut s, f);
    let def = definition(&mut s, card);
    assert_eq!(def.matches(DEFAULT_SLOT).count(), 1, "{def}");
    assert!(def.find("Title") < def.find(DEFAULT_SLOT), "the slot follows the component's own layers:\n{def}");
    assert!(def.contains(&format!("{DEFAULT_SLOT}</div>")), "the slot sits in the root element:\n{def}");
}

#[test]
fn an_empty_auto_layout_component_is_a_root_holding_a_default_slot() {
    let mut s = Session::new();
    let f = frame(&mut s, "Box");
    ex(&mut s, "layout.add", json!({"ids": [f]}));
    let boxed = component(&mut s, f);
    let def = definition(&mut s, boxed);
    assert!(def.contains(&format!("part=\"root\">{DEFAULT_SLOT}</div>")), "{def}");
}

#[test]
fn a_component_without_auto_layout_but_with_a_text_layer_takes_content() {
    let mut s = Session::new();
    let f = frame(&mut s, "Button");
    ex(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 40, "parent": f}));
    ex(&mut s, "text.create", json!({"text": "Click", "x": 10, "y": 10, "parent": f}));
    let button = component(&mut s, f);
    assert_eq!(definition(&mut s, button).matches(DEFAULT_SLOT).count(), 1);
}

#[test]
fn an_icon_made_of_shapes_takes_no_content() {
    let mut s = Session::new();
    let f = frame(&mut s, "Heart");
    ex(&mut s, "shape.star", json!({"x": 0, "y": 0, "width": 24, "height": 24, "parent": f}));
    let heart = component(&mut s, f);
    let def = definition(&mut s, heart);
    assert!(!def.contains("<slot"), "{def}");
}

#[test]
fn a_childless_component_without_auto_layout_takes_no_content() {
    let mut s = Session::new();
    let f = frame(&mut s, "Input");
    let input = component(&mut s, f);
    let def = definition(&mut s, input);
    assert!(!def.contains("<slot"), "{def}");
}

#[test]
fn a_named_slot_keeps_its_name_beside_the_default_slot() {
    let mut s = Session::new();
    let f = frame(&mut s, "Shell");
    let body = idv(&ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 200, "height": 100, "parent": f, "name": "Body"})));
    ex(&mut s, "text.create", json!({"text": "Empty", "x": 0, "y": 0, "parent": body}));
    let shell = component(&mut s, f);
    ex(&mut s, "component.makeSlot", json!({"id": body, "name": "Content"}));
    let def = definition(&mut s, shell);
    assert!(def.contains("<slot name=\"content\">"), "{def}");
    assert_eq!(def.matches(DEFAULT_SLOT).count(), 1, "{def}");
}

#[test]
fn the_card_staged_for_the_playground_shows_what_the_template_puts_in_its_tag() {
    use crate::playground::Template;
    use crate::playground_support::{Bench, Library};

    let mut bench = Bench::new("default-slot", Library::Full);
    bench.run("playground.open", &json!({"template": "dashboard"})).unwrap();
    let page = std::fs::read_to_string(bench.staged("template.html")).unwrap();
    assert_eq!(page, bench.composed(Template::Dashboard).html);
    let card = std::fs::read_to_string(bench.staged("lib/components/ui-card.ts")).unwrap();
    assert_eq!(card.matches(DEFAULT_SLOT).count(), 1, "{card}");
    let open = page.find("<ui-card").unwrap_or(usize::MAX);
    let close = page.find("</ui-card>").unwrap_or(0);
    assert!(open < close, "the page has no ui-card:\n{page}");
}
