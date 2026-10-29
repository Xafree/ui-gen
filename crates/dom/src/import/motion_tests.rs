use figcraft_doc::proto::Easing;
use serde_json::{Map, Value, json};

use super::facts_tests::facts_of;
use super::support::bounds;
use super::translate_support::{Translation, translate_root};
use crate::{Effect, css_easing};

const EASE: Easing = Easing::CustomBezier { x1: 0.25, y1: 0.1, x2: 0.25, y2: 1.0 };
const EFFECT_KEY: &str = "uigen.effect";
const SQUARE: &str =
    r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><rect width="24" height="24" fill="#ff0000"/></svg>"##;

fn bezier(x1: f64, y1: f64, x2: f64, y2: f64) -> Option<Easing> {
    Some(Easing::CustomBezier { x1, y1, x2, y2 })
}

// covers: E3-4.2
#[test]
fn css_keywords_and_cubic_bezier_become_the_easing_of_a_transition() {
    assert_eq!(css_easing("linear"), Some(Easing::Linear));
    assert_eq!(css_easing("ease"), bezier(0.25, 0.1, 0.25, 1.0));
    assert_eq!(css_easing("ease-in"), bezier(0.42, 0.0, 1.0, 1.0));
    assert_eq!(css_easing("ease-out"), bezier(0.0, 0.0, 0.58, 1.0));
    assert_eq!(css_easing("ease-in-out"), bezier(0.42, 0.0, 0.58, 1.0));
    assert_eq!(css_easing("cubic-bezier(0.4, 0, 0.2, 1)"), bezier(0.4, 0.0, 0.2, 1.0));
}

// covers: E3-4.2
#[test]
fn a_curve_with_no_equivalent_gives_none() {
    for value in ["steps(4)", "linear(0, 1)", "bogus", "cubic-bezier(1, 2)", ""] {
        assert_eq!(css_easing(value), None, "{value:?}");
    }
}

fn host(transition: Value) -> Translation {
    let mut style = Map::new();
    style.insert("backgroundColor".to_owned(), json!("rgb(25, 118, 210)"));
    style.extend(transition.as_object().cloned().unwrap_or_default());
    let child = json!({ "type": "element", "tag": "span", "label": "Kid", "box": bounds(12.0, 12.0, 10.0, 10.0), "style": { "backgroundColor": "rgb(9, 9, 9)" } });
    translate_root(json!({
        "type": "element", "tag": "button", "label": "Host", "box": bounds(0.0, 0.0, 200.0, 100.0), "style": style, "children": [child]
    }))
}

fn notes(run: &Translation) -> Vec<String> {
    run.note_labels(run.by_name("Host"))
}

// covers: E3-4.3
#[test]
fn the_longest_of_several_transitions_is_kept_and_the_other_is_named_in_a_note() {
    let run = host(json!({
        "transitionProperty": "background-color, color",
        "transitionDuration": "250ms, 150ms",
        "transitionTimingFunction": "ease, ease-in",
        "transitionDelay": "0s"
    }));

    let transition = facts_of(&run, "Host").transition.clone().expect("a transition");
    assert_eq!(transition.duration_ms, 250);
    assert_eq!(transition.delay_ms, 0);
    assert_eq!(transition.easing, EASE);
    assert_eq!(transition.properties, ["background-color"]);
    let labels = notes(&run);
    assert!(labels.iter().any(|label| label.contains("150") && label.contains("color")), "{labels:?}");
    assert!(run.translated.warnings.iter().any(|warning| warning.contains("150")), "{:?}", run.translated.warnings);
}

// covers: E3-4.3
#[test]
fn a_single_transition_in_seconds_is_kept_without_a_note() {
    let run = host(json!({ "transitionProperty": "all", "transitionDuration": "0.3s", "transitionTimingFunction": "ease", "transitionDelay": "0s" }));

    let transition = facts_of(&run, "Host").transition.clone().expect("a transition");
    assert_eq!((transition.duration_ms, transition.delay_ms, transition.easing), (300, 0, EASE));
    assert_eq!(transition.properties, ["all"]);
    assert!(notes(&run).is_empty(), "{:?}", notes(&run));
}

// covers: E3-4.3
#[test]
fn a_zero_duration_everywhere_is_no_transition_and_no_note() {
    let run = host(json!({ "transitionProperty": "all", "transitionDuration": "0s", "transitionTimingFunction": "ease", "transitionDelay": "0s" }));

    assert_eq!(facts_of(&run, "Host").transition, None);
    assert!(notes(&run).is_empty(), "{:?}", notes(&run));
    assert!(run.translated.warnings.is_empty(), "{:?}", run.translated.warnings);
}

// covers: E3-4.3
#[test]
fn a_curve_that_cannot_be_translated_is_a_note_never_an_invented_transition() {
    let run =
        host(json!({ "transitionProperty": "all", "transitionDuration": "200ms", "transitionTimingFunction": "steps(4)", "transitionDelay": "0s" }));

    assert_eq!(facts_of(&run, "Host").transition, None);
    let labels = notes(&run);
    assert!(labels.iter().any(|label| label.contains("steps(4)")), "{labels:?}");
}

fn spin() -> Value {
    json!({ "name": "spin", "durationMs": 1400, "easing": "linear", "iterations": "infinite", "properties": ["transform"] })
}

fn marked(run: &Translation, name: &str) -> (Option<Effect>, Option<String>) {
    (facts_of(run, name).effect, run.by_name(name).plugin_data.get(EFFECT_KEY).cloned())
}

fn inks(inks: &[Value]) -> Translation {
    translate_root(json!({
        "type": "element", "tag": "button", "label": "Host", "box": bounds(0.0, 0.0, 200.0, 100.0),
        "style": { "backgroundColor": "rgb(25, 118, 210)" }, "children": inks
    }))
}

fn ink(label: &str, class_name: &str, animations: Value, children: Value) -> Value {
    json!({
        "type": "element", "tag": "span", "label": label, "className": class_name, "box": bounds(10.0, 10.0, 50.0, 50.0),
        "style": { "backgroundColor": "rgb(9, 9, 9)" }, "animations": animations, "children": children
    })
}

// covers: E3-4.4
#[test]
fn an_element_whose_class_contains_ripple_in_any_case_is_a_ripple() {
    let run = inks(&[ink("Ink", "MuiTouchRipple-root css-1", json!([]), json!([])), ink("Plain", "MuiButton-root", json!([]), json!([]))]);

    assert_eq!(marked(&run, "Ink"), (Some(Effect::Ripple), Some("ripple".to_owned())));
    assert_eq!(marked(&run, "Plain"), (None, None));
}

// covers: E3-4.4
#[test]
fn an_element_or_a_wrapper_of_an_svg_turning_forever_is_a_spinner() {
    let svg = json!({ "type": "svg", "markup": SQUARE, "box": bounds(10.0, 10.0, 24.0, 24.0) });
    let run = inks(&[ink("Wheel", "", json!([spin()]), json!([])), ink("Loader", "", json!([spin()]), json!([svg]))]);

    assert_eq!(marked(&run, "Wheel"), (Some(Effect::Spinner), Some("spinner".to_owned())));
    assert_eq!(marked(&run, "Loader"), (Some(Effect::Spinner), Some("spinner".to_owned())));
    let drawing = run.children_of(run.by_name("Loader"));
    assert!(drawing.iter().all(|node| !node.plugin_data.contains_key(EFFECT_KEY)), "only the turning element is flagged");
}

// covers: E3-4.4
#[test]
fn any_other_animation_is_a_note_naming_it_and_sets_no_effect() {
    let fade = json!({ "name": "fade-in", "durationMs": 300, "easing": "ease", "iterations": 1, "properties": ["opacity"] });
    let breathe = json!({ "name": "breathe", "durationMs": 2000, "easing": "ease", "iterations": "infinite", "properties": ["opacity"] });
    let run = inks(&[ink("Fading", "", json!([fade]), json!([])), ink("Breathing", "", json!([breathe]), json!([]))]);

    for (name, animation) in [("Fading", "fade-in"), ("Breathing", "breathe")] {
        assert_eq!(marked(&run, name), (None, None), "{name}");
        let labels = run.note_labels(run.by_name(name));
        assert!(labels.iter().any(|label| label.contains(animation)), "{name}: {labels:?}");
        assert!(run.translated.warnings.iter().any(|warning| warning.contains(animation)), "{:?}", run.translated.warnings);
    }
}
