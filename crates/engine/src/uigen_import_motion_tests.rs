//! `uigen.importComponent`: the state transitions of the imported set become `ChangeTo`
//! interactions between its variants, and the ripple and spinner layers are flagged so the Lit
//! export animates them. The service is a scripted backend; the snapshots are synthetic buttons.

use std::collections::BTreeMap;

use figcraft_doc::proto::{Action, Easing, Interaction, Transition, Trigger};
use figcraft_doc::{Document, NodeId};
use serde_json::{Value, json};

use crate::capture::{CaptureBackend, CaptureErrorKind, CaptureJob, CaptureOutcome, JobId};
use crate::params::id_of;
use crate::uigen_import_support::{Bench, IMPORT, PACKAGE, button_inspect, service_failure, synthetic_button, synthetic_source};

const NOTE: &str = "UIGEN import:";
const EFFECT: &str = "uigen.effect";

const STANDARD_CURVE: &str = "cubic-bezier(0.4, 0, 0.2, 1)";
const PRESS_CURVE: &str = "cubic-bezier(0.2, 0, 0, 1)";
const PRESSED_FILL: &str = "rgb(13, 71, 161)";

type Declarations = Vec<(&'static str, &'static str)>;

fn standard() -> Easing {
    Easing::CustomBezier { x1: 0.4, y1: 0.0, x2: 0.2, y2: 1.0 }
}

fn press() -> Easing {
    Easing::CustomBezier { x1: 0.2, y1: 0.0, x2: 0.0, y2: 1.0 }
}

fn timed(duration: &'static str, curve: &'static str) -> Declarations {
    vec![("transitionDuration", duration), ("transitionTimingFunction", curve)]
}

#[derive(Default)]
struct Look {
    root: BTreeMap<&'static str, Declarations>,
    icon: BTreeMap<&'static str, Declarations>,
    ripple: bool,
    spinner: bool,
    wobble: bool,
}

impl Look {
    fn arriving(mut self, state: &'static str, declarations: Declarations) -> Self {
        self.root.insert(state, declarations);
        self
    }

    fn icon_arriving(mut self, state: &'static str, declarations: Declarations) -> Self {
        self.icon.insert(state, declarations);
        self
    }

    fn with_effects(mut self) -> Self {
        self.ripple = true;
        self.spinner = true;
        self
    }
}

fn state_key(state: &str) -> &'static str {
    let lower = state.to_ascii_lowercase();
    if lower.contains("hover") {
        "hover"
    } else if lower.contains("press") || lower.contains("active") {
        "pressed"
    } else if lower.contains("focus") {
        "focus"
    } else {
        "default"
    }
}

fn element(label: &str, class_name: Option<&str>, style: Value, animations: Value, box_: [f64; 4]) -> Value {
    let [x, y, width, height] = box_;
    let mut node = json!({
        "type": "element", "tag": "span", "label": label,
        "box": {"x": x, "y": y, "width": width, "height": height},
        "style": style, "animations": animations
    });
    if let (Some(class_name), Some(object)) = (class_name, node.as_object_mut()) {
        object.insert("className".to_owned(), json!(class_name));
    }
    node
}

fn snapshot(look: &Look, style: &str, size: &str, state: &str, theme: &str) -> Value {
    let key = state_key(state);
    let mut snapshot = synthetic_button(style, size, state, theme);
    let root = snapshot["root"].as_object_mut().unwrap();
    let css = root.get_mut("style").and_then(Value::as_object_mut).unwrap();
    for (property, value) in look.root.get(key).into_iter().flatten() {
        css.insert((*property).to_owned(), json!(value));
    }
    let children = root.get_mut("children").and_then(Value::as_array_mut).unwrap();
    let mut icon_style = json!({"backgroundColor": "rgb(238, 238, 238)"});
    for (property, value) in look.icon.get(key).into_iter().flatten() {
        icon_style[*property] = json!(value);
    }
    children.push(element("Icon", None, icon_style, json!([]), [30.0, 26.0, 16.0, 16.0]));
    if look.ripple {
        children.push(element("Ripple", Some("syn-ripple"), json!({}), json!([]), [20.0, 20.0, 100.0, 36.0]));
    }
    if look.spinner {
        let turning = json!([{"name": "syn-spin", "durationMs": 1000, "easing": "linear", "iterations": "infinite", "properties": ["transform"]}]);
        children.push(element("Spinner", None, json!({}), turning, [90.0, 28.0, 16.0, 16.0]));
    }
    if look.wobble {
        let wobbling = json!([{"name": "wobble", "durationMs": 400, "easing": "ease", "iterations": 3, "properties": ["opacity"]}]);
        children.push(element("Pulse", None, json!({}), wobbling, [70.0, 28.0, 8.0, 8.0]));
    }
    snapshot
}

struct Backend {
    look: Look,
    loading: bool,
    submitted: u64,
    ready: Vec<(JobId, CaptureOutcome)>,
}

impl CaptureBackend for Backend {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let outcome = match &job {
            CaptureJob::Inspect { .. } => button_inspect(true, self.loading),
            CaptureJob::Render { props, state, theme, .. } => {
                let style = props.get("variant").and_then(Value::as_str).unwrap_or("solid");
                let size = props.get("size").and_then(Value::as_str).unwrap_or("md");
                Ok(json!({"snapshot": snapshot(&self.look, style, size, state, theme), "png": ".uigen/captures/synthetic.png"}))
            }
            CaptureJob::Install { .. } | CaptureJob::Uninstall { .. } | CaptureJob::Playground { .. } => {
                Err(service_failure(CaptureErrorKind::Internal, "unexpected job"))
            }
        };
        self.submitted += 1;
        self.ready.push((self.submitted, outcome));
        self.submitted
    }

    fn poll(&mut self) -> Vec<(JobId, CaptureOutcome)> {
        std::mem::take(&mut self.ready)
    }

    fn shutdown(&mut self) {}
}

fn bench(tag: &str, look: Look, loading: bool) -> Bench {
    let mut bench = Bench::without_backend(tag, &[synthetic_source(false)]);
    bench.session.set_capture_backend(Box::new(Backend { look, loading, submitted: 0, ready: Vec::new() }));
    bench
}

fn params(states: &[&str]) -> Value {
    json!({"package": PACKAGE, "component": "Button", "states": states, "themes": ["light"]})
}

fn one_style(states: &[&str]) -> Value {
    json!({"package": PACKAGE, "component": "Button", "states": states, "themes": ["light"], "styles": ["solid"], "sizes": ["md"]})
}

fn import_ok(bench: &mut Bench, params: &Value) -> Value {
    bench.import(params).unwrap();
    let events = bench.pump_to_end();
    let event = events.iter().find(|event| event.command == IMPORT).unwrap_or_else(|| panic!("no import event in {events:?}"));
    match &event.outcome {
        Ok(result) => result.clone(),
        Err(error) => panic!("the import failed: {}", error.message),
    }
}

fn document_of(bench: &Bench) -> &Document {
    bench.session.docs[0].doc.as_ref()
}

fn set_of(result: &Value) -> NodeId {
    id_of(&result["set"]).unwrap_or_else(|| panic!("no set in {result}"))
}

fn walk(doc: &Document, id: NodeId, out: &mut Vec<NodeId>) {
    out.push(id);
    for child in &doc.node(id).unwrap().children {
        walk(doc, *child, out);
    }
}

fn subtree(doc: &Document, id: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    walk(doc, id, &mut out);
    out
}

fn variants(doc: &Document, set: NodeId) -> Vec<NodeId> {
    doc.node(set).unwrap().children.clone()
}

fn variant(doc: &Document, set: NodeId, name: &str) -> NodeId {
    variants(doc, set).into_iter().find(|id| doc.node(*id).unwrap().name == name).unwrap_or_else(|| panic!("no variant {name:?}"))
}

fn name_of(doc: &Document, id: NodeId) -> String {
    doc.node(id).unwrap().name.clone()
}

fn interactions(doc: &Document, variant: NodeId) -> Vec<Interaction> {
    doc.node(variant).unwrap().interactions.clone()
}

fn target(doc: &Document, interaction: &Interaction) -> String {
    match interaction.actions.as_slice() {
        [Action::ChangeTo { dest }] => name_of(doc, *dest),
        other => panic!("expected a single ChangeTo, got {other:?}"),
    }
}

fn on(doc: &Document, variant: NodeId, wanted: &Trigger) -> Interaction {
    let all = interactions(doc, variant);
    all.iter().find(|interaction| interaction.trigger == *wanted).cloned().unwrap_or_else(|| panic!("no {wanted:?} among {all:?}"))
}

fn axes_of(name: &str) -> (String, String) {
    let part = |prefix: &str| name.split(", ").find_map(|piece| piece.strip_prefix(prefix)).unwrap_or_default().to_owned();
    (part("Style="), part("Size="))
}

fn is_note(label: &str) -> bool {
    label.starts_with(NOTE)
}

fn notes_naming(doc: &Document, root: NodeId, word: &str) -> usize {
    subtree(doc, root)
        .into_iter()
        .map(|id| doc.node(id).unwrap().annotations.iter().filter(|note| is_note(&note.label) && note.label.to_lowercase().contains(word)).count())
        .sum()
}

fn effect_of(doc: &Document, variant: NodeId, layer: &str) -> Option<String> {
    let id = subtree(doc, variant)
        .into_iter()
        .find(|id| doc.node(*id).unwrap().name == layer)
        .unwrap_or_else(|| panic!("no layer {layer:?} in {}", name_of(doc, variant)));
    doc.node(id).unwrap().plugin_data.get(EFFECT).cloned()
}

// covers: E3-7.1
#[test]
fn hover_and_press_change_to_the_same_style_and_size_in_smart_animate_with_the_timing_of_the_arrival_state() {
    let look = Look::default().arriving("hover", timed("250ms", STANDARD_CURVE)).arriving("pressed", timed("120ms", PRESS_CURVE));
    let mut bench = bench("arrival-timing", look, false);
    let result = import_ok(&mut bench, &params(&["Default", "Hover", "Pressed"]));

    let doc = document_of(&bench);
    let set = set_of(&result);
    assert_eq!(variants(doc, set).len(), 12);
    for style in ["solid", "outline"] {
        for size in ["sm", "md"] {
            let rest = variant(doc, set, &format!("Style={style}, Size={size}, State=Default"));
            assert_eq!(interactions(doc, rest).len(), 2, "{style} {size}: one interaction for hover, one for press");

            let hover = on(doc, rest, &Trigger::OnHover);
            assert_eq!(target(doc, &hover), format!("Style={style}, Size={size}, State=Hover"));
            assert_eq!(hover.transition, Transition::SmartAnimate);
            assert_eq!(hover.duration_ms, 250);
            assert_eq!(hover.easing, standard());

            let pressed = on(doc, rest, &Trigger::OnPress);
            assert_eq!(target(doc, &pressed), format!("Style={style}, Size={size}, State=Pressed"));
            assert_eq!(pressed.transition, Transition::SmartAnimate);
            assert_eq!(pressed.duration_ms, 120);
            assert_eq!(pressed.easing, press());

            for arrival in ["Hover", "Pressed"] {
                let state = variant(doc, set, &format!("Style={style}, Size={size}, State={arrival}"));
                assert!(interactions(doc, state).is_empty(), "only the Default variants carry interactions");
            }
        }
    }
}

// covers: E3-7.1
#[test]
fn of_several_transitions_the_longest_one_sets_the_interaction_and_the_others_become_notes() {
    let longest = Look::default().arriving("hover", timed("250ms", STANDARD_CURVE)).icon_arriving("hover", timed("400ms", "linear"));
    let mut bench = bench("longest", longest, false);
    let result = import_ok(&mut bench, &one_style(&["Default", "Hover"]));
    let doc = document_of(&bench);
    let set = set_of(&result);
    let hover = on(doc, variant(doc, set, "Style=solid, Size=md, State=Default"), &Trigger::OnHover);
    assert_eq!(hover.duration_ms, 400, "the icon's transition is the longest of the layers");
    assert_eq!(hover.easing, Easing::Linear);
    assert!(notes_naming(doc, set, "transition") > 0, "the 250ms transition of the other layer is left out and says so");

    let mut alone = hover_bench("alone");
    let result = import_ok(&mut alone, &one_style(&["Default", "Hover"]));
    assert_eq!(notes_naming(document_of(&alone), set_of(&result), "transition"), 0, "nothing is left out, nothing is noted");
}

fn hover_bench(tag: &str) -> Bench {
    bench(tag, Look::default().arriving("hover", timed("250ms", STANDARD_CURVE)), false)
}

// covers: E3-7.2
#[test]
fn without_a_transition_read_the_interactions_exist_and_are_instant() {
    let mut bench = bench("instant", Look::default(), false);
    let result = import_ok(&mut bench, &one_style(&["Default", "Hover", "Pressed"]));

    let doc = document_of(&bench);
    let rest = variant(doc, set_of(&result), "Style=solid, Size=md, State=Default");
    for (trigger, arrival) in [(Trigger::OnHover, "Hover"), (Trigger::OnPress, "Pressed")] {
        let interaction = on(doc, rest, &trigger);
        assert_eq!(target(doc, &interaction), format!("Style=solid, Size=md, State={arrival}"));
        assert_eq!(interaction.transition, Transition::Instant, "{arrival}");
    }
}

// covers: E3-7.2
#[test]
fn a_focus_transition_is_a_note_on_the_focus_variant_and_no_interaction_goes_to_another_style_or_size() {
    let look = Look::default().arriving("hover", timed("250ms", STANDARD_CURVE)).arriving("focus", timed("150ms", "linear"));
    let mut bench = bench("focus-note", look, false);
    let result = import_ok(&mut bench, &params(&["Default", "Hover", "Focus", "Pressed"]));

    let doc = document_of(&bench);
    let set = set_of(&result);
    let mut interactive = 0;
    for source in variants(doc, set) {
        let name = name_of(doc, source);
        let all = interactions(doc, source);
        if name.ends_with("State=Default") {
            interactive += 1;
            assert_eq!(all.len(), 2, "{name}: hover and press only, focus has no prototype trigger");
        } else {
            assert!(all.is_empty(), "{name}");
        }
        for interaction in &all {
            let destination = target(doc, interaction);
            assert_eq!(axes_of(&destination), axes_of(&name), "{name} -> {destination}");
            assert!(!destination.ends_with("State=Focus"), "{name} -> {destination}");
        }
        if name.ends_with("State=Focus") {
            assert!(notes_naming(doc, source, "transition") > 0, "{name} carries the note of its transition");
        }
    }
    assert_eq!(interactive, 4);

    let mut calm = hover_bench("focus-calm");
    let calm_result = import_ok(&mut calm, &params(&["Default", "Hover", "Focus", "Pressed"]));
    let calm_doc = document_of(&calm);
    for focus in variants(calm_doc, set_of(&calm_result)).into_iter().filter(|id| name_of(calm_doc, *id).ends_with("State=Focus")) {
        assert_eq!(notes_naming(calm_doc, focus, "transition"), 0, "{}", name_of(calm_doc, focus));
    }
}

// covers: E3-7.2
#[test]
fn a_state_that_was_not_imported_gets_no_interaction() {
    let mut hover_only = bench("hover-only", Look::default(), false);
    let result = import_ok(&mut hover_only, &one_style(&["Default", "Hover"]));
    let doc = document_of(&hover_only);
    let rest = variant(doc, set_of(&result), "Style=solid, Size=md, State=Default");
    let all = interactions(doc, rest);
    assert_eq!(all.len(), 1, "{all:?}");
    assert_eq!(all[0].trigger, Trigger::OnHover);

    let mut alone = bench("default-only", Look::default(), false);
    let result = import_ok(&mut alone, &one_style(&["Default"]));
    let doc = document_of(&alone);
    assert!(interactions(doc, variant(doc, set_of(&result), "Style=solid, Size=md, State=Default")).is_empty());
}

// covers: E3-7.3
#[test]
fn the_ripple_and_the_spinner_layers_carry_their_effect_and_an_unknown_animation_is_a_note() {
    let mut effects = Look::default().with_effects();
    effects.wobble = true;
    let mut bench = bench("effect-flags", effects, false);
    let result = import_ok(&mut bench, &params(&["Default", "Hover"]));

    let doc = document_of(&bench);
    let set = set_of(&result);
    assert_eq!(variants(doc, set).len(), 8);
    for source in variants(doc, set) {
        let name = name_of(doc, source);
        assert_eq!(effect_of(doc, source, "Ripple").as_deref(), Some("ripple"), "{name}");
        assert_eq!(effect_of(doc, source, "Spinner").as_deref(), Some("spinner"), "{name}");
        assert_eq!(effect_of(doc, source, "Icon"), None, "{name}: an ordinary layer is no effect");
        assert_eq!(effect_of(doc, source, "Pulse"), None, "{name}: an animation that is neither is no effect");
        assert!(notes_naming(doc, source, "wobble") > 0, "{name}: the animation nobody recognised is named in a note");
    }
}

fn compact(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").replace("> <", "><")
}

fn rules(definition: &str) -> Vec<(String, String)> {
    let text = compact(definition);
    let before = text.split("@media (prefers-reduced-motion").next().unwrap_or_default().to_string();
    before
        .split('}')
        .filter_map(|piece| {
            let (head, body) = piece.rsplit_once('{')?;
            let selector = head.rsplit(['`', ';', '{']).next()?.trim().to_string();
            Some((selector, body.trim().to_string()))
        })
        .collect()
}

fn rule_starting(definition: &str, prefix: &str) -> String {
    rules(definition).into_iter().find(|(selector, _)| selector.starts_with(prefix)).map(|(_, body)| body).unwrap_or_else(|| {
        panic!("no rule starting with {prefix:?} in:\n{definition}");
    })
}

// covers: E3-7.3
#[test]
fn the_lit_export_of_the_set_transitions_with_the_read_timing_and_plays_the_ripple_and_the_spinner() {
    let look = Look::default()
        .arriving("hover", timed("250ms", STANDARD_CURVE))
        .arriving("pressed", [timed("250ms", STANDARD_CURVE), vec![("backgroundColor", PRESSED_FILL)]].concat())
        .with_effects();
    let mut bench = bench("lit-export", look, true);
    let result = import_ok(&mut bench, &one_style(&["Default", "Hover", "Pressed", "Loading"]));

    let code = bench.session.execute("inspect.componentCode", &json!({"id": set_of(&result), "language": "lit"})).unwrap();
    let definition = code["definition"].as_str().unwrap_or_default().to_string();
    let text = compact(&definition);

    let motion = text.split("@media (prefers-reduced-motion").next().unwrap_or_default();
    assert!(motion.contains("transition:"), "the hover transition is exported:\n{definition}");
    assert!(motion.contains(&format!("250ms {STANDARD_CURVE}")), "its duration and its curve are the ones read:\n{definition}");

    assert!(text.contains("@keyframes"), "{definition}");
    let ripple = rule_starting(&definition, ":host(:active) .");
    assert!(ripple.contains("animation:") && ripple.contains("250ms"), "the ripple lasts as long as the press transition: {ripple}");
    let spinner = rule_starting(&definition, ":host([loading]) .");
    assert!(spinner.contains("animation:") && spinner.contains("infinite"), "the spinner turns while loading: {spinner}");

    let notes = code["notes"].as_array().cloned().unwrap_or_default();
    let unsupported: Vec<&Value> = notes.iter().filter(|note| note["code"].as_str().is_some_and(|code| code.starts_with("unsupported"))).collect();
    assert!(unsupported.is_empty(), "nothing the import produced is unsupported by the export: {unsupported:?}");
}
