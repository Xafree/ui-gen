//! `uigen.importComponent`: what the lot 3 review found. An import that ends during a gesture, a
//! collection reused with other modes, variables shared between components, a dark render equal to the
//! light one, requests refused early, the page chosen at submission. The service is a scripted backend.

use std::collections::BTreeSet;

use figcraft_color::Color;
use figcraft_doc::{Document, NodeId, NodeKind, Variable, VariableValue};
use serde_json::{Value, json};

use crate::capture::{CaptureError, CaptureErrorKind, CaptureEvent};
use crate::params::id_of;
use crate::uigen_import_support::{Bench, IMPORT, PACKAGE, Scenario, button_inspect, synthetic_source};
use crate::{EngineError, Result};

const COLLECTION: &str = "synthetic-ui 1.0.0";
const PRIMARY: &str = "rgb(25, 118, 210)";
const PRIMARY_DARK: &str = "rgb(144, 202, 249)";
const IDENTICAL: &str = "the dark render is identical to the light one: the library does not follow prefers-color-scheme";

fn bench(tag: &str) -> Bench {
    Bench::new(tag, &[synthetic_source(false)], Scenario::new(button_inspect(true, false)))
}

fn params(extra: &Value) -> Value {
    let mut all =
        json!({"package": PACKAGE, "component": "Button", "styles": ["solid"], "sizes": ["md"], "states": ["Default"], "themes": ["light"]});
    if let (Some(base), Some(more)) = (all.as_object_mut(), extra.as_object()) {
        base.extend(more.clone());
    }
    all
}

fn import_events(events: &[CaptureEvent]) -> Vec<&CaptureEvent> {
    events.iter().filter(|event| event.command == IMPORT).collect()
}

fn import_ok(bench: &mut Bench, params: &Value) -> Value {
    bench.import(params).unwrap();
    let events = bench.pump_to_end();
    match import_events(&events).as_slice() {
        [event] => event.outcome.clone().unwrap_or_else(|error| panic!("the import failed: {}", error.message)),
        other => panic!("one import event expected: {other:?}"),
    }
}

fn import_err(bench: &mut Bench, params: &Value) -> CaptureError {
    bench.import(params).unwrap();
    let events = bench.pump_to_end();
    match import_events(&events).as_slice() {
        [event] => event.outcome.clone().expect_err("the import succeeded"),
        other => panic!("one import event expected: {other:?}"),
    }
}

fn doc(bench: &Bench) -> &Document {
    bench.session.docs[0].doc.as_ref()
}

fn set_of(result: &Value) -> NodeId {
    id_of(&result["set"]).unwrap_or_else(|| panic!("no set in {result}"))
}

fn component_sets(doc: &Document) -> usize {
    doc.nodes().filter(|node| matches!(node.kind, NodeKind::ComponentSet { .. })).count()
}

fn warnings_of(result: &Value) -> Vec<String> {
    result["warnings"].as_array().map(|list| list.iter().filter_map(|w| w.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

fn variables(doc: &Document) -> Vec<&Variable> {
    let id = &doc.variables.collections.iter().find(|collection| collection.name == COLLECTION).expect("no collection").id;
    doc.variables.variables.iter().filter(|variable| variable.collection == *id).collect()
}

fn mode_names(doc: &Document) -> Vec<String> {
    doc.variables
        .collections
        .iter()
        .find(|collection| collection.name == COLLECTION)
        .map(|c| c.modes.iter().map(|m| m.name.clone()).collect())
        .unwrap()
}

fn color_in(doc: &Document, variable: &Variable, mode: &str) -> Option<String> {
    let owner = doc.variables.collection(&variable.collection)?;
    let mode = owner.modes.iter().find(|candidate| candidate.name == mode)?;
    match variable.values.get(&mode.id)? {
        VariableValue::Color(color) => Some(color.to_hex()),
        _ => None,
    }
}

fn css(value: &str) -> String {
    Color::parse_css(value).unwrap().to_hex()
}

fn named<'a>(doc: &'a Document, name: &str) -> &'a Variable {
    variables(doc).into_iter().find(|variable| variable.name == name).unwrap_or_else(|| panic!("no variable {name:?}"))
}

fn names_bound_under(doc: &Document, root: NodeId) -> BTreeSet<String> {
    fn walk(doc: &Document, id: NodeId, out: &mut BTreeSet<String>) {
        let node = doc.node(id).unwrap();
        out.extend(node.bound.values().filter_map(|variable| doc.variables.get(variable)).map(|variable| variable.name.clone()));
        for child in &node.children {
            walk(doc, *child, out);
        }
    }
    let mut out = BTreeSet::new();
    walk(doc, root, &mut out);
    out
}

fn translation_x(doc: &Document, id: NodeId) -> f64 {
    doc.node(id).unwrap().transform.as_coeffs()[4]
}

#[test]
fn an_import_that_ends_during_a_gesture_waits_for_it_and_lands_as_its_own_undo_step() {
    let mut bench = bench("during-gesture");
    let depth = bench.session.undo_depth();
    bench.import(&params(&json!({}))).unwrap();
    bench.session.begin_interaction("Move").unwrap();
    let before = bench.document_text(0);

    let events = bench.pump_to_end();
    assert!(import_events(&events).is_empty(), "no event while the gesture lasts: {events:?}");
    assert_eq!(bench.document_text(0), before, "nothing is written during the gesture");
    assert_eq!(component_sets(doc(&bench)), 0);

    bench.session.commit_interaction().unwrap();
    let events = bench.pump_to_end();
    let [event] = import_events(&events)[..] else { panic!("one event once the gesture is over: {events:?}") };
    let result = event.outcome.clone().unwrap_or_else(|error| panic!("the import failed: {}", error.message));
    assert_eq!(component_sets(doc(&bench)), 1);
    assert_eq!(set_of(&result), doc(&bench).nodes().find(|node| matches!(node.kind, NodeKind::ComponentSet { .. })).unwrap().id);
    assert_eq!(bench.session.undo_depth().0, depth.0 + 1, "the gesture made no step, the import made one");
    assert_eq!(bench.session.undo_labels(1), ["Import component"]);
    assert!(bench.session.undo().unwrap());
    assert_eq!(component_sets(doc(&bench)), 0, "one undo takes the whole import back");
}

#[test]
fn an_import_that_ends_while_a_version_is_shown_waits_until_it_is_closed() {
    let mut bench = bench("during-version");
    bench.import(&params(&json!({}))).unwrap();
    bench.session.docs[0].text_preview = Some(bench.session.docs[0].doc.clone());

    let events = bench.pump_to_end();
    assert!(import_events(&events).is_empty(), "{events:?}");
    bench.session.docs[0].text_preview = None;
    let events = bench.pump_to_end();
    assert_eq!(import_events(&events).len(), 1, "{events:?}");
    assert_eq!(component_sets(doc(&bench)), 1);
}

#[test]
fn a_dark_import_then_a_light_one_adds_the_light_mode_and_fills_it_without_touching_the_dark_one() {
    let mut bench = bench("dark-then-light");
    import_ok(&mut bench, &params(&json!({"themes": ["dark"]})));
    assert_eq!(mode_names(doc(&bench)), ["Dark"]);
    let dark_before: Vec<(String, Option<String>)> =
        variables(doc(&bench)).iter().map(|v| (v.name.clone(), color_in(doc(&bench), v, "Dark"))).collect();

    import_ok(&mut bench, &params(&json!({"themes": ["light"]})));
    let doc = doc(&bench);
    assert_eq!(mode_names(doc), ["Dark", "Light"]);
    for (name, value) in &dark_before {
        assert_eq!(&color_in(doc, named(doc, name), "Dark"), value, "{name} keeps its Dark value");
    }
    let background = named(doc, "couleur/button/fond");
    assert_eq!(color_in(doc, background, "Dark"), Some(css(PRIMARY_DARK)));
    assert_eq!(color_in(doc, background, "Light"), Some(css(PRIMARY)));
}

#[test]
fn a_light_import_then_a_light_and_dark_one_adds_the_dark_mode_and_keeps_every_light_value() {
    let mut bench = bench("light-then-both");
    import_ok(&mut bench, &params(&json!({"themes": ["light"]})));
    let light_before: Vec<Variable> = variables(doc(&bench)).into_iter().cloned().collect();

    let result = import_ok(&mut bench, &params(&json!({"themes": ["light", "dark"]})));
    let doc = doc(&bench);
    assert_eq!(mode_names(doc), ["Light", "Dark"]);
    for before in &light_before {
        let now = variables(doc).into_iter().find(|variable| variable.id == before.id).unwrap();
        assert_eq!(color_in(doc, now, "Light"), color_in(doc, before, "Light"), "{} keeps its Light value", before.name);
    }
    let background = named(doc, "couleur/button/fond");
    assert_eq!(color_in(doc, background, "Dark"), Some(css(PRIMARY_DARK)));
    assert!(variables(doc).iter().all(|variable| variable.values.len() == 2), "every variable has both modes");
    assert!(!warnings_of(&result).iter().any(|warning| warning == IDENTICAL));
}

#[test]
fn two_components_whose_root_layers_share_a_name_share_no_role_variable() {
    let mut bench = bench("two-components");
    let button = import_ok(&mut bench, &params(&json!({})));
    let chip = import_ok(&mut bench, &params(&json!({"component": "x-chip", "styles": null, "sizes": null})));

    let doc = doc(&bench);
    let (button_names, chip_names) = (names_bound_under(doc, set_of(&button)), names_bound_under(doc, set_of(&chip)));
    assert!(!button_names.is_empty() && !chip_names.is_empty());
    assert_eq!(button_names.intersection(&chip_names).count(), 0, "{button_names:?} / {chip_names:?}");
    assert!(button_names.iter().all(|name| name.contains("/button/")), "{button_names:?}");
    assert!(chip_names.iter().all(|name| name.contains("/x-chip/")), "{chip_names:?}");
}

#[test]
fn a_dark_render_equal_to_the_light_one_is_reported_in_the_warnings() {
    let mut same = bench("dark-same");
    let result = import_ok(&mut same, &params(&json!({"styles": ["outline"], "themes": ["light", "dark"]})));
    assert!(warnings_of(&result).iter().any(|warning| warning == IDENTICAL), "{:?}", warnings_of(&result));

    let mut differs = bench("dark-differs");
    let result = import_ok(&mut differs, &params(&json!({"themes": ["light", "dark"]})));
    assert!(!warnings_of(&result).iter().any(|warning| warning.contains("identical")), "{:?}", warnings_of(&result));

    let mut light_only = bench("light-only-no-warning");
    let result = import_ok(&mut light_only, &params(&json!({"styles": ["outline"]})));
    assert!(warnings_of(&result).is_empty(), "{:?}", warnings_of(&result));
}

fn assert_refused(bench: &mut Bench, what: &str, request: &Value, fragment: &str) {
    let before = bench.document_text(0);
    let submitted = bench.submitted().len();
    let result: Result<Value> = bench.import(request);
    let Err(error) = result else { panic!("{what}: accepted") };
    assert!(matches!(error, EngineError::BadParams { .. }), "{what}: {error:?}");
    assert!(error.to_string().contains(fragment), "{what}: {error}");
    assert_eq!(bench.submitted().len(), submitted, "{what}: a job was submitted");
    assert_eq!(bench.document_text(0), before, "{what}: the document changed");
}

#[test]
fn a_single_coordinate_is_refused_at_once() {
    let mut bench = bench("one-coordinate");
    assert_refused(&mut bench, "x alone", &params(&json!({"x": 10})), "x and y");
    assert_refused(&mut bench, "y alone", &params(&json!({"y": 10})), "x and y");
    let started = bench.import(&params(&json!({"x": 10, "y": 20}))).unwrap();
    assert!(started["job"].as_u64().is_some());
}

#[test]
fn a_parent_inside_an_instance_is_refused_at_once() {
    let mut bench = bench("parent-in-instance");
    let mut make = |command: &str, request: Value| id_of(&bench.session.execute(command, &request).unwrap()["id"]).unwrap();
    let outer = make("frame.create", json!({"x": 0, "y": 0, "width": 100, "height": 100}));
    make("frame.create", json!({"parent": outer, "x": 0, "y": 0, "width": 10, "height": 10}));
    let component = make("component.create", json!({"ids": [outer]}));
    let instance = make("instance.create", json!({"component": component, "x": 300, "y": 0}));
    let inside = doc(&bench)
        .node(instance)
        .unwrap()
        .children
        .iter()
        .copied()
        .find(|id| matches!(doc(&bench).node(*id).unwrap().kind, NodeKind::Frame))
        .unwrap();

    assert_refused(&mut bench, "frame inside an instance", &params(&json!({"parent": inside})), "instance");
}

fn inspect_with(prop: Value) -> Scenario {
    let listing = json!({
        "package": PACKAGE, "version": "1.0.0", "framework": "react",
        "components": [{"name": "Button", "props": [prop]}],
        "notes": []
    });
    Scenario::new(Ok(listing))
}

#[test]
fn values_given_for_a_prop_the_component_does_not_have_are_refused_by_name() {
    let mut bench =
        Bench::new("unknown-prop", &[synthetic_source(false)], inspect_with(json!({"name": "variant", "type": "enum", "values": ["solid"]})));
    let error = import_err(&mut bench, &params(&json!({"sizeProp": "scale", "sizes": ["xl"]})));
    assert_eq!(error.kind, CaptureErrorKind::InvalidRequest);
    assert!(error.message.contains("scale"), "{}", error.message);
    assert_eq!(bench.submitted().len(), 1, "no render after the refusal");
    assert_eq!(component_sets(doc(&bench)), 0);
}

#[test]
fn an_unusable_value_read_from_inspect_is_left_out_with_a_warning_instead_of_failing_the_import() {
    let prop = json!({"name": "variant", "type": "enum", "values": ["solid", "out,line", "a=b", "", "outline"]});
    let mut bench = Bench::new("unusable-value", &[synthetic_source(false)], inspect_with(prop));
    let result = import_ok(&mut bench, &params(&json!({"styles": null, "sizes": null})));

    let doc = doc(&bench);
    let set = set_of(&result);
    let mut names: Vec<String> = doc.node(set).unwrap().children.iter().map(|id| doc.node(*id).unwrap().name.clone()).collect();
    names.sort();
    assert_eq!(names, ["Style=outline, State=Default", "Style=solid, State=Default"]);
    let warnings = warnings_of(&result);
    assert!(warnings.iter().any(|warning| warning.contains("out,line")), "{warnings:?}");
    assert!(warnings.iter().any(|warning| warning.contains("a=b")), "{warnings:?}");
}

#[test]
fn the_set_is_placed_on_the_page_the_import_was_started_on_whichever_page_is_open_when_it_ends() {
    let mut bench = bench("page-at-submission");
    let first = bench.session.page().unwrap();
    bench.session.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 100, "height": 50})).unwrap();
    let started = bench.import(&params(&json!({}))).unwrap();
    assert!(started["job"].as_u64().is_some());

    let other = id_of(&bench.session.execute("page.add", &json!({"name": "Other"})).unwrap()["id"]).unwrap();
    bench.session.execute("page.select", &json!({"id": other})).unwrap();
    bench.session.execute("shape.rectangle", &json!({"x": 5000, "y": 0, "width": 800, "height": 50})).unwrap();
    let events = bench.pump_to_end();
    let [event] = import_events(&events)[..] else { panic!("{events:?}") };
    let result = event.outcome.clone().unwrap_or_else(|error| panic!("the import failed: {}", error.message));

    let doc = doc(&bench);
    let set = set_of(&result);
    assert_eq!(doc.node(set).unwrap().parent, Some(first));
    assert!((translation_x(doc, set) - 180.0).abs() < 1e-6, "next to the first page's content, not the open page's: {}", translation_x(doc, set));
}
