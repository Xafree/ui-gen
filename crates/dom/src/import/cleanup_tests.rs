use serde_json::{Value, json};

use super::support::bounds;
use super::translate_support::{WRAPPER, offset, parse, translate, translate_root, translate_with};
use crate::TranslateOptions;

fn element(label: &str, window: Value, style: Value, children: Value) -> Value {
    json!({ "type": "element", "tag": "div", "label": label, "box": window, "style": style, "children": children })
}

fn wrapping_one_child_of_the_same_box(wrapper_style: Value) -> Value {
    let inner = element("Inner", bounds(10.0, 10.0, 80.0, 30.0), json!({ "backgroundColor": "rgb(25, 118, 210)" }), json!([]));
    let wrapper = element("Wrapper", bounds(10.0, 10.0, 80.0, 30.0), wrapper_style, json!([inner]));
    element("Card", bounds(0.0, 0.0, 200.0, 100.0), json!({}), json!([wrapper]))
}

// covers: E3-3.3
#[test]
fn a_plain_wrapper_with_one_child_of_the_same_box_disappears_and_the_child_takes_its_place() {
    let run = translate(&parse(WRAPPER));

    let names = run.child_names(run.root());
    assert!(!names.contains(&"Wrapper".to_owned()), "{names:?}");
    let inner = run.by_name("Inner");
    assert_eq!(inner.parent, Some(run.translated.root), "the child now sits directly under the wrapper's parent");
    assert_eq!(offset(inner), (10.0, 10.0));
    assert_eq!((inner.width, inner.height), (80.0, 30.0));
    assert_eq!(inner.fills.len(), 1, "the child keeps its own fill");
}

// covers: E3-3.3
#[test]
fn an_element_with_nothing_visible_and_no_children_disappears() {
    let run = translate(&parse(WRAPPER));

    assert_eq!(run.child_names(run.root()), ["Inner", "Ripple", "Filled", "Dim"]);
}

// covers: E3-3.3
#[test]
fn an_element_whose_class_name_contains_ripple_is_never_removed() {
    let run = translate(&parse(WRAPPER));

    assert!(run.child_names(run.root()).contains(&"Ripple".to_owned()));

    let wrapper_with_ripple = json!({
        "type": "element", "tag": "span", "label": "Ripple wrapper", "className": "touch-ripple",
        "box": bounds(10.0, 10.0, 80.0, 30.0), "style": {},
        "children": [element("Inner", bounds(10.0, 10.0, 80.0, 30.0), json!({ "backgroundColor": "rgb(25, 118, 210)" }), json!([]))]
    });
    let kept = translate_root(element("Card", bounds(0.0, 0.0, 200.0, 100.0), json!({}), json!([wrapper_with_ripple])));
    assert_eq!(kept.child_names(kept.root()), ["Ripple wrapper"]);
}

// covers: E3-3.3
#[test]
fn a_wrapper_with_something_visible_stays() {
    let border = json!({ "borderTopStyle": "solid", "borderTopWidth": "1px", "borderTopColor": "rgb(255, 0, 0)" });
    let styles = [
        json!({ "backgroundColor": "rgb(0, 0, 255)" }),
        border,
        json!({ "boxShadow": "0px 1px 2px rgba(0, 0, 0, 0.2)" }),
        json!({ "overflow": "hidden" }),
        json!({ "opacity": "0.5" }),
    ];
    for style in styles {
        let run = translate_root(wrapping_one_child_of_the_same_box(style.clone()));

        assert_eq!(run.child_names(run.root()), ["Wrapper"], "{style}");
        assert_eq!(run.child_names(run.by_name("Wrapper")), ["Inner"], "{style}");
    }
    let run = translate(&parse(WRAPPER));
    assert_eq!(run.child_names(run.by_name("Dim")), ["Dimmed"], "a translucent wrapper keeps its child inside");
}

// covers: E3-3.3
#[test]
fn a_wrapper_whose_child_has_another_box_stays() {
    let inner = element("Inner", bounds(12.0, 10.0, 70.0, 30.0), json!({ "backgroundColor": "rgb(25, 118, 210)" }), json!([]));
    let wrapper = element("Wrapper", bounds(10.0, 10.0, 80.0, 30.0), json!({}), json!([inner]));

    let run = translate_root(element("Card", bounds(0.0, 0.0, 200.0, 100.0), json!({}), json!([wrapper])));

    assert_eq!(run.child_names(run.root()), ["Wrapper"]);
}

// covers: E3-3.3
#[test]
fn without_the_cleanup_option_every_element_stays() {
    let snapshot = parse(WRAPPER);

    let run = translate_with(&snapshot, &TranslateOptions { auto_layout: true, cleanup: false });

    assert_eq!(run.child_names(run.root()), ["Wrapper", "Empty", "Ripple", "Filled", "Dim"]);
}

// covers: E3-3.3
#[test]
fn removed_layers_leave_no_trace_in_the_layer_facts() {
    let run = translate(&parse(WRAPPER));

    let listed: Vec<_> = run.translated.layers.iter().map(|layer| layer.id).collect();
    assert_eq!(listed, run.doc.subtree(run.translated.root), "one fact per remaining layer, in tree order");
    let paths: Vec<&str> = run.translated.layers.iter().map(|layer| layer.path.as_str()).collect();
    assert!(paths.contains(&"Card / Inner"), "{paths:?}");
    assert!(paths.iter().all(|path| !path.contains("Wrapper") && !path.contains("Empty")), "{paths:?}");
}
