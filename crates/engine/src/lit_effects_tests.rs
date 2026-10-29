//! Lit motion: prototype transitions become CSS transitions, ripple and spinner layers become
//! keyframe animations, reduced motion switches them off, and what CSS can't reproduce is noted
//! instead of invented.

use crate::Session;
use crate::components::tests::{ex, idv};
use crate::lit;
use base64::Engine as _;
use figcraft_doc::NodeId;
use figcraft_doc::proto::Easing;
use serde_json::{Value, json};

const REDUCED_MOTION: &str = "@media (prefers-reduced-motion: reduce)";
const EASE_OUT: &str = "cubic-bezier(0.33, 1, 0.68, 1)";

fn component_code(s: &mut Session, id: NodeId) -> Value {
    ex(s, "inspect.componentCode", json!({"id": id, "language": "lit"}))
}

fn compact(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").replace("> <", "><")
}

fn definition_of(v: &Value) -> String {
    v["definition"].as_str().unwrap_or_default().to_string()
}

/// Every CSS rule written before the reduced-motion block: (selector, body).
fn rules(def: &str) -> Vec<(String, String)> {
    let text = compact(def);
    let before = text.split(REDUCED_MOTION).next().unwrap_or_default().to_string();
    before
        .split('}')
        .filter_map(|piece| {
            let (head, body) = piece.rsplit_once('{')?;
            let selector = head.rsplit(['`', ';', '{']).next()?.trim().to_string();
            Some((selector, body.trim().to_string()))
        })
        .collect()
}

fn body_of(def: &str, selector: &str) -> String {
    rules(def).into_iter().find(|(s, _)| s == selector).map(|(_, body)| body).unwrap_or_else(|| panic!("no rule {selector} in:\n{def}"))
}

fn has_rule(def: &str, selector: &str) -> bool {
    rules(def).iter().any(|(s, _)| s == selector)
}

fn reduced_motion_block(def: &str) -> String {
    let text = compact(def);
    text.find(REDUCED_MOTION).and_then(|at| text.get(at..)).unwrap_or_default().to_string()
}

fn notes_of(v: &Value) -> Vec<Value> {
    v["notes"].as_array().cloned().unwrap_or_default()
}

fn note_with<'a>(notes: &'a [Value], code: &str) -> Option<&'a Value> {
    notes.iter().find(|n| n["code"] == code)
}

/// A set named "Control": one variant per `(variant name, background colour)` row, each with a "Bg"
/// rectangle and whatever `extra` adds to the variant frame. The first row is the default variant.
fn motion_set(s: &mut Session, rows: &[(&str, &str)], extra: &dyn Fn(&mut Session, NodeId)) -> (NodeId, Vec<NodeId>) {
    let mut variants = Vec::new();
    for (k, (variant, fill)) in rows.iter().enumerate() {
        let f = idv(&ex(s, "frame.create", json!({"x": k as f64 * 150.0, "y": 0, "width": 100, "height": 40, "name": variant})));
        ex(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 40, "parent": f, "props": {"name": "Bg", "fill": fill}}));
        extra(s, f);
        variants.push(idv(&ex(s, "component.create", json!({"ids": [f]}))));
    }
    let set = idv(&ex(s, "component.combineVariants", json!({"ids": variants.clone()})));
    ex(s, "node.set", json!({"id": set, "props": {"name": "Control"}}));
    (set, variants)
}

fn nothing_extra(_: &mut Session, _: NodeId) {}

fn add_ellipse(s: &mut Session, parent: NodeId, name: &str) -> NodeId {
    idv(&ex(s, "shape.ellipse", json!({"x": 40, "y": 10, "width": 20, "height": 20, "parent": parent, "props": {"name": name}})))
}

fn layer_named(s: &Session, variant: NodeId, name: &str) -> NodeId {
    let doc = s.doc().unwrap();
    doc.node(variant).unwrap().children.iter().copied().find(|k| doc.node(*k).unwrap().name == name).unwrap()
}

fn set_interactions(s: &mut Session, node: NodeId, interactions: &[Value]) {
    ex(s, "node.set", json!({"id": node, "props": {"interactions": interactions}}));
}

fn change_to(trigger: &str, dest: NodeId, duration_ms: u32) -> Value {
    json!({
        "trigger": {"type": trigger},
        "actions": [{"type": "changeTo", "dest": dest}],
        "transition": {"type": "smartAnimate"},
        "durationMs": duration_ms,
        "easing": "easeOut"
    })
}

fn hover_set(s: &mut Session) -> (NodeId, Vec<NodeId>) {
    motion_set(s, &[("State=Default", "#ff0000"), ("State=Hover", "#0000ff")], &nothing_extra)
}

fn image_fills(s: &mut Session) -> Value {
    let image = image::RgbaImage::from_pixel(8, 8, image::Rgba([10, 120, 200, 255]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    let data = base64::engine::general_purpose::STANDARD.encode(bytes.into_inner());
    let layer = idv(&ex(s, "file.import", json!({"data": data, "name": "photo.png", "x": 0, "y": 600})));
    serde_json::to_value(&s.doc().unwrap().node(layer).unwrap().fills).unwrap()
}

fn spring_points(css: &str) -> Vec<f64> {
    let inner = css.strip_prefix("linear(").and_then(|rest| rest.strip_suffix(')')).unwrap_or_default();
    inner.split(',').filter_map(|point| point.split_whitespace().next()?.parse::<f64>().ok()).collect()
}

fn bezier_numbers(css: &str) -> Vec<f64> {
    let inner = css.strip_prefix("cubic-bezier(").and_then(|rest| rest.strip_suffix(')')).unwrap_or_default();
    inner.split(',').filter_map(|n| n.trim().parse::<f64>().ok()).collect()
}

#[test]
fn mouse_enter_to_hover_becomes_a_base_transition_on_the_changed_property() {
    // covers: E6-4.1
    let mut s = Session::new();
    let (set, variants) = hover_set(&mut s);
    let enter = change_to("mouseEnter", variants[1], 200);
    set_interactions(&mut s, variants[0], &[enter]);
    let def = definition_of(&component_code(&mut s, set));
    let base = body_of(&def, ".bg");
    assert!(base.contains(&format!("transition: background 200ms {EASE_OUT}")), "{base}\n{def}");
    assert_eq!(base.matches("transition:").count(), 1, "{base}");
    assert!(!base.contains("all"), "only the properties that change:\n{base}");
    assert_eq!(body_of(&def, ":host(:hover) .bg"), "background: #0000FF;", "{def}");
}

#[test]
fn every_easing_has_its_css_value() {
    // covers: E6-4.2
    let cases = [
        (Easing::Linear, "linear"),
        (Easing::EaseIn, "cubic-bezier(0.32, 0, 0.67, 0)"),
        (Easing::EaseOut, "cubic-bezier(0.33, 1, 0.68, 1)"),
        (Easing::EaseInOut, "cubic-bezier(0.65, 0, 0.35, 1)"),
        (Easing::CustomBezier { x1: 0.1, y1: 0.2, x2: 0.3, y2: 0.4 }, "cubic-bezier(0.1, 0.2, 0.3, 0.4)"),
    ];
    for (easing, want) in cases {
        assert_eq!(lit::easing_css(easing), want, "{easing:?}");
    }
}

#[test]
fn a_spring_becomes_a_linear_function_from_zero_to_one() {
    // covers: E6-4.2
    for easing in [Easing::Gentle, Easing::Bouncy, Easing::CUSTOM_SPRING] {
        let css = lit::easing_css(easing);
        assert!(css.starts_with("linear("), "{easing:?}: {css}");
        let points = spring_points(&css);
        assert!(points.len() >= 8, "{easing:?}: {} points in {css}", points.len());
        assert!(points.first().is_some_and(|p| p.abs() < 1e-6), "starts at 0: {css}");
        assert!(points.last().is_some_and(|p| (p - 1.0).abs() < 1e-3), "ends at 1: {css}");
    }
}

#[test]
fn a_back_easing_is_a_bezier_that_overshoots() {
    // covers: E6-4.2
    for easing in [Easing::EaseInBack, Easing::EaseOutBack, Easing::EaseInOutBack] {
        let css = lit::easing_css(easing);
        assert!(css.starts_with("cubic-bezier("), "{easing:?}: {css}");
        let numbers = bezier_numbers(&css);
        assert_eq!(numbers.len(), 4, "{css}");
        assert!(numbers.get(1).into_iter().chain(numbers.get(3)).any(|y| !(0.0..=1.0).contains(y)), "{easing:?} overshoots: {css}");
    }
}

#[test]
fn the_delay_and_the_two_durations_sit_on_the_state_and_on_the_base() {
    // covers: E6-4.3
    let mut s = Session::new();
    let (set, variants) = hover_set(&mut s);
    let mut enter = change_to("mouseEnter", variants[1], 200);
    enter["delayMs"] = json!(150);
    set_interactions(&mut s, variants[0], &[enter]);
    set_interactions(&mut s, variants[1], &[change_to("mouseLeave", variants[0], 400)]);
    let def = definition_of(&component_code(&mut s, set));
    let hover = body_of(&def, ":host(:hover) .bg");
    assert!(hover.contains(&format!("transition: background 200ms {EASE_OUT} 150ms")), "going to hover:\n{hover}\n{def}");
    assert!(hover.contains("background: #0000FF;"), "{hover}");
    let base = body_of(&def, ".bg");
    assert!(base.contains(&format!("transition: background 400ms {EASE_OUT}")), "coming back:\n{base}\n{def}");
    assert!(!base.contains("150ms"), "the delay belongs to the way in:\n{base}");
}

fn press_rows() -> [(&'static str, &'static str); 2] {
    [("State=Default", "#ff0000"), ("State=Pressed", "#ffff00")]
}

fn with_ripple(s: &mut Session, parent: NodeId) {
    add_ellipse(s, parent, "Ripple");
}

#[test]
fn a_ripple_layer_animates_while_pressed_and_defaults_to_600ms_with_a_note() {
    // covers: E6-4.4
    let mut s = Session::new();
    let (set, _) = motion_set(&mut s, &press_rows(), &with_ripple);
    let v = component_code(&mut s, set);
    let def = definition_of(&v);
    assert!(compact(&def).contains("@keyframes"), "{def}");
    let pressed = body_of(&def, ":host(:active) .ripple");
    assert!(pressed.contains("animation:") && pressed.contains("600ms"), "{pressed}\n{def}");
    assert!(compact(&def).contains("overflow: hidden"), "the ripple stays inside:\n{def}");
    let notes = notes_of(&v);
    let defaulted = notes.iter().find(|n| n["layer"] == "Ripple");
    assert!(defaulted.is_some_and(|n| n["component"] == "Control" && n["message"].as_str().is_some_and(|m| !m.is_empty())), "{notes:?}");
}

#[test]
fn a_ripple_lasts_as_long_as_the_pressing_interaction() {
    // covers: E6-4.4
    let mut s = Session::new();
    let (set, variants) = motion_set(&mut s, &press_rows(), &with_ripple);
    set_interactions(&mut s, variants[0], &[change_to("mouseDown", variants[1], 450)]);
    let def = definition_of(&component_code(&mut s, set));
    let pressed = body_of(&def, ":host(:active) .ripple");
    assert!(pressed.contains("450ms") && !pressed.contains("600ms"), "{pressed}\n{def}");
}

#[test]
fn a_layer_flagged_by_plugin_data_is_a_ripple_and_an_unflagged_one_is_not() {
    // covers: E6-4.4
    let mut s = Session::new();
    let (plain, _) = motion_set(&mut s, &press_rows(), &|s, f| {
        add_ellipse(s, f, "Ink");
    });
    let def = definition_of(&component_code(&mut s, plain));
    assert!(!compact(&def).contains("@keyframes") && !has_rule(&def, ":host(:active) .ink"), "an ordinary layer is no effect:\n{def}");

    let (flagged, variants) = motion_set(&mut s, &press_rows(), &|s, f| {
        add_ellipse(s, f, "Ink");
    });
    for variant in variants {
        let ink = layer_named(&s, variant, "Ink");
        ex(&mut s, "node.set", json!({"id": ink, "props": {"pluginData": {"uigen.effect": "ripple"}}}));
    }
    let def = definition_of(&component_code(&mut s, flagged));
    assert!(compact(&def).contains("@keyframes"), "{def}");
    assert!(body_of(&def, ":host(:active) .ink").contains("animation:"), "{def}");
}

#[test]
fn a_spinner_layer_turns_forever_while_loading() {
    // covers: E6-4.5
    let mut s = Session::new();
    let rows = [("State=Default", "#ff0000"), ("State=Loading", "#00ffff")];
    let (set, _) = motion_set(&mut s, &rows, &|s, f| {
        add_ellipse(s, f, "Spinner");
    });
    let def = definition_of(&component_code(&mut s, set));
    assert!(compact(&def).contains("@keyframes"), "{def}");
    let loading = body_of(&def, ":host([loading]) .spinner");
    assert!(loading.contains("animation:") && loading.contains("infinite"), "{loading}\n{def}");
    assert!(!body_of(&def, ".spinner").contains("animation"), "at rest it stands still:\n{def}");
}

#[test]
fn reduced_motion_switches_transitions_and_animations_off() {
    // covers: E6-4.6
    let mut s = Session::new();
    let rows = [("State=Default", "#ff0000"), ("State=Hover", "#0000ff"), ("State=Loading", "#00ffff")];
    let (set, variants) = motion_set(&mut s, &rows, &|s, f| {
        add_ellipse(s, f, "Spinner");
    });
    set_interactions(&mut s, variants[0], &[change_to("mouseEnter", variants[1], 200)]);
    let def = definition_of(&component_code(&mut s, set));
    let block = reduced_motion_block(&def);
    assert!(!block.is_empty(), "no {REDUCED_MOTION} block in:\n{def}");
    assert!(block.contains("transition: none") || block.contains("transition-duration"), "{block}");
    assert!(block.contains("animation: none") || block.contains("animation-duration"), "{block}");
    assert!(!block.contains("infinite"), "{block}");
}

#[test]
fn a_transition_css_can_not_play_is_noted_and_not_declared() {
    // covers: E6-4.7
    for kind in ["push", "moveIn", "moveOut", "slideIn", "slideOut"] {
        let mut s = Session::new();
        let (set, variants) = hover_set(&mut s);
        let mut enter = change_to("mouseEnter", variants[1], 200);
        enter["transition"] = json!({"type": kind, "direction": "left"});
        set_interactions(&mut s, variants[0], &[enter]);
        let v = component_code(&mut s, set);
        let notes = notes_of(&v);
        let note = note_with(&notes, "unsupported-transition");
        assert!(note.is_some_and(|n| n["component"] == "Control"), "{kind}: {notes:?}");
        let def = definition_of(&v);
        assert!(!body_of(&def, ".bg").contains("transition"), "{kind}: nothing invented:\n{def}");
    }
}

#[test]
fn an_action_other_than_change_to_is_noted_and_not_declared() {
    // covers: E6-4.7
    for action in [json!({"type": "openUrl", "url": "https://example.com"}), json!({"type": "back"})] {
        let mut s = Session::new();
        let (set, variants) = hover_set(&mut s);
        let interaction = json!({"trigger": {"type": "mouseEnter"}, "actions": [action.clone()], "transition": {"type": "smartAnimate"}});
        set_interactions(&mut s, variants[0], &[interaction]);
        let v = component_code(&mut s, set);
        let notes = notes_of(&v);
        let note = note_with(&notes, "unsupported-action");
        assert!(note.is_some_and(|n| n["component"] == "Control"), "{action}: {notes:?}");
        let def = definition_of(&v);
        assert!(!body_of(&def, ".bg").contains("transition"), "{action}: nothing invented:\n{def}");
    }
}

#[test]
fn a_progressive_blur_is_noted_with_its_layer_and_not_declared() {
    // covers: E6-4.7
    let mut s = Session::new();
    let (set, _) = motion_set(&mut s, &[("State=Default", "#ff0000"), ("State=Hover", "#0000ff")], &|s, f| {
        let glass = idv(&ex(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 50, "height": 40, "parent": f, "props": {"name": "Glass"}})));
        let blur = json!({"type": "layerBlur", "radius": 12, "blurType": "progressive", "startRadius": 2, "startOffset": [0.5, 0.1], "endOffset": [0.5, 0.9]});
        ex(s, "effect.add", json!({"ids": [glass], "effect": blur}));
    });
    let v = component_code(&mut s, set);
    let notes = notes_of(&v);
    let note = note_with(&notes, "unsupported-effect");
    assert!(note.is_some_and(|n| n["component"] == "Control" && n["layer"] == "Glass"), "{notes:?}");
    assert!(!definition_of(&v).contains("blur("), "nothing invented:\n{}", definition_of(&v));
}

#[test]
fn an_image_fill_is_noted_with_its_layer_and_not_declared() {
    // covers: E6-4.7
    let mut s = Session::new();
    let fills = image_fills(&mut s);
    let (set, _) = motion_set(&mut s, &[("State=Default", "#ff0000"), ("State=Hover", "#0000ff")], &|s, f| {
        let photo = idv(&ex(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 50, "height": 40, "parent": f, "props": {"name": "Photo"}})));
        ex(s, "node.set", json!({"id": photo, "props": {"fills": fills.clone()}}));
    });
    let v = component_code(&mut s, set);
    let notes = notes_of(&v);
    let note = note_with(&notes, "image-fill");
    assert!(note.is_some_and(|n| n["component"] == "Control" && n["layer"] == "Photo"), "{notes:?}");
    let def = definition_of(&v);
    assert!(!def.contains("url(") && !def.contains("background-image"), "nothing invented:\n{def}");
}

#[test]
fn a_vector_layer_is_an_inline_svg_in_render_and_is_noted() {
    // covers: E6-4.7
    // covers: E6-4.8
    let mut s = Session::new();
    let (set, _) = motion_set(&mut s, &[("State=Default", "#ff0000"), ("State=Hover", "#0000ff")], &|s, f| {
        ex(s, "vector.create", json!({"path": "M0 0 L10 0 L10 10 Z", "parent": f, "props": {"name": "Mark"}}));
    });
    let v = component_code(&mut s, set);
    let def = compact(&definition_of(&v));
    let render = def.find("render()").unwrap_or(usize::MAX);
    let svg = def.find("<svg").unwrap_or(0);
    assert!(render < svg, "an inline <svg in render():\n{def}");
    let notes = notes_of(&v);
    let note = note_with(&notes, "vector-fallback");
    assert!(note.is_some_and(|n| n["component"] == "Control" && n["layer"] == "Mark"), "{notes:?}");
}

#[test]
fn a_note_never_carries_executable_text() {
    // covers: E6-4.9
    let mut s = Session::new();
    let fills = image_fills(&mut s);
    let (set, variants) = motion_set(&mut s, &[("State=Default", "#ff0000"), ("State=Hover", "#0000ff")], &|s, f| {
        let photo = idv(&ex(
            s,
            "shape.rectangle",
            json!({"x": 0, "y": 0, "width": 50, "height": 40, "parent": f, "props": {"name": "Photo`${alert(1)}<script>alert(1)</script>"}}),
        ));
        ex(s, "node.set", json!({"id": photo, "props": {"fills": fills.clone()}}));
    });
    let hostile = json!({"trigger": {"type": "mouseEnter"}, "actions": [{"type": "openUrl", "url": "javascript:alert(1)"}]});
    set_interactions(&mut s, variants[0], &[hostile]);
    let notes = notes_of(&component_code(&mut s, set));
    assert!(note_with(&notes, "image-fill").is_some() && note_with(&notes, "unsupported-action").is_some(), "{notes:?}");
    for note in &notes {
        let message = note["message"].as_str().unwrap_or_default();
        for executable in ["${", "`", "<script", "alert(1)", "javascript:"] {
            assert!(!message.contains(executable), "{executable:?} in note: {note}");
        }
    }
}
