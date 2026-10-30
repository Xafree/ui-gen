//! `uigen.importComponent` with `stage: true`: the set lands in an import page of its own, one
//! per package and version, instead of on the current page, and `animations: false` leaves out the
//! transitions and the effects. The service is a scripted backend; the snapshots are synthetic.

use figcraft_doc::{Document, NodeId, NodeKind};
use serde_json::{Value, json};

use crate::capture::{CaptureBackend, CaptureErrorKind, CaptureJob, CaptureOutcome, JobId};
use crate::params::id_of;
use crate::uigen_import::{IMPORT_PAGE_KEY, import_page_name, is_import_page};
use crate::uigen_import_support::{
    Bench, FailingRender, IMPORT, PACKAGE, Scenario, VERSION, button_inspect, service_failure, synthetic_button, synthetic_source,
};
use crate::{EngineError, Result};

const EFFECT: &str = "uigen.effect";
const NEXT_VERSION: &str = "1.1.0";
const MAX_PUMPS: usize = 64;

fn inspected(version: &str) -> CaptureOutcome {
    let props = json!([
        {"name": "variant", "type": "enum", "values": ["solid", "outline"]},
        {"name": "size", "type": "enum", "values": ["sm", "md"]}
    ]);
    Ok(json!({
        "package": PACKAGE,
        "version": version,
        "framework": "react",
        "components": [{"name": "Button", "props": props}, {"name": "Chip", "props": props}],
        "notes": []
    }))
}

fn element(label: &str, class_name: Option<&str>, animations: Value, box_: [f64; 4]) -> Value {
    let [x, y, width, height] = box_;
    let mut node = json!({
        "type": "element", "tag": "span", "label": label,
        "box": {"x": x, "y": y, "width": width, "height": height},
        "style": {}, "animations": animations
    });
    if let (Some(class_name), Some(object)) = (class_name, node.as_object_mut()) {
        object.insert("className".to_owned(), json!(class_name));
    }
    node
}

fn snapshot(with_effects: bool, style: &str, size: &str, state: &str, theme: &str) -> Value {
    let mut snapshot = synthetic_button(style, size, state, theme);
    if !with_effects {
        return snapshot;
    }
    let Some(root) = snapshot["root"].as_object_mut() else { return snapshot };
    if state == "hover"
        && let Some(css) = root.get_mut("style").and_then(Value::as_object_mut)
    {
        css.insert("transitionDuration".to_owned(), json!("250ms"));
        css.insert("transitionTimingFunction".to_owned(), json!("cubic-bezier(0.4, 0, 0.2, 1)"));
    }
    if let Some(children) = root.get_mut("children").and_then(Value::as_array_mut) {
        children.push(element("Ripple", Some("syn-ripple"), json!([]), [20.0, 20.0, 100.0, 36.0]));
        let turning = json!([{"name": "syn-spin", "durationMs": 1000, "easing": "linear", "iterations": "infinite", "properties": ["transform"]}]);
        children.push(element("Spinner", None, turning, [90.0, 28.0, 16.0, 16.0]));
    }
    snapshot
}

struct Backend {
    version: String,
    with_effects: bool,
    submitted: u64,
    ready: Vec<(JobId, CaptureOutcome)>,
}

impl CaptureBackend for Backend {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let outcome = match &job {
            CaptureJob::Inspect { .. } => inspected(&self.version),
            CaptureJob::Render { props, state, theme, .. } => {
                let style = props.get("variant").and_then(Value::as_str).unwrap_or("solid");
                let size = props.get("size").and_then(Value::as_str).unwrap_or("md");
                Ok(json!({"snapshot": snapshot(self.with_effects, style, size, state, theme), "png": ".uigen/captures/synthetic.png"}))
            }
            CaptureJob::Install { package, version, .. } => {
                self.version = version.clone();
                Ok(json!({"package": package, "version": version, "framework": "react", "license": "MIT"}))
            }
            CaptureJob::Uninstall { .. } => Err(service_failure(CaptureErrorKind::Internal, "unexpected job")),
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

fn bench(tag: &str, with_effects: bool) -> Bench {
    let mut bench = Bench::without_backend(tag, &[synthetic_source(false)]);
    bench.session.set_capture_backend(Box::new(Backend { version: VERSION.to_owned(), with_effects, submitted: 0, ready: Vec::new() }));
    bench
}

fn staged(component: &str, states: &[&str]) -> Value {
    json!({"package": PACKAGE, "component": component, "states": states, "themes": ["light"], "stage": true})
}

fn plain(states: &[&str]) -> Value {
    json!({"package": PACKAGE, "component": "Button", "states": states, "themes": ["light"]})
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

fn undo(bench: &mut Bench) {
    bench.session.execute("edit.undo", &json!({})).unwrap();
}

fn id_in(result: &Value, key: &str) -> NodeId {
    id_of(&result[key]).unwrap_or_else(|| panic!("no {key} in {result}"))
}

fn name_of(doc: &Document, id: NodeId) -> String {
    doc.node(id).unwrap().name.clone()
}

fn sets_on(doc: &Document, page: NodeId) -> Vec<NodeId> {
    doc.node(page).unwrap().children.iter().copied().filter(|id| matches!(doc.node(*id).unwrap().kind, NodeKind::ComponentSet { .. })).collect()
}

fn variant_count(doc: &Document, set: NodeId) -> usize {
    doc.node(set).unwrap().children.len()
}

fn position(doc: &Document, id: NodeId) -> (f64, f64) {
    let coeffs = doc.node(id).unwrap().transform.as_coeffs();
    (coeffs[4], coeffs[5])
}

fn overlap(doc: &Document, a: NodeId, b: NodeId) -> bool {
    let (ax, ay) = position(doc, a);
    let (bx, by) = position(doc, b);
    let (a, b) = (doc.node(a).unwrap(), doc.node(b).unwrap());
    ax < bx + b.width && bx < ax + a.width && ay < by + b.height && by < ay + a.height
}

fn subtree(doc: &Document, id: NodeId) -> Vec<NodeId> {
    let mut out = vec![id];
    for child in &doc.node(id).unwrap().children {
        out.extend(subtree(doc, *child));
    }
    out
}

fn project_components(bench: &mut Bench) -> Value {
    bench.session.execute("project.info", &json!({})).unwrap()["components"].clone()
}

// covers: E4-1.1
#[test]
fn a_staged_import_fills_an_import_page_and_leaves_the_current_page_and_the_components_alone() {
    let mut bench = bench("stage-nominal", false);
    let before = bench.document_text(0);
    let depth = bench.session.undo_depth();
    let current = bench.session.docs[0].page;
    let pages = document_of(&bench).pages.clone();
    let components = project_components(&mut bench);

    let started = bench.import(&staged("Button", &["Default", "Hover"])).unwrap();
    assert!(started["job"].as_u64().is_some(), "{started}");
    assert_eq!(bench.document_text(0), before, "nothing is written before the renders are back");
    assert_eq!(bench.session.undo_depth(), depth);

    let events = bench.pump_to_end();
    let event = events.iter().find(|event| event.command == IMPORT).unwrap_or_else(|| panic!("no import event in {events:?}"));
    let result = match &event.outcome {
        Ok(result) => result.clone(),
        Err(error) => panic!("the import failed: {}", error.message),
    };
    assert_eq!(result["variants"], json!(8), "{result}");

    let doc = document_of(&bench);
    let page = id_in(&result, "page");
    assert_eq!(name_of(doc, page), "- Imported · synthetic-ui 1.0.0");
    assert_eq!(name_of(doc, page), import_page_name(PACKAGE, VERSION));
    assert_eq!(IMPORT_PAGE_KEY, "uigen.importPage");
    let marker: Value =
        serde_json::from_str(doc.node(page).unwrap().plugin_data.get(IMPORT_PAGE_KEY).unwrap_or_else(|| panic!("no marker"))).unwrap();
    assert_eq!(marker, json!({"package": PACKAGE, "version": VERSION}));
    assert!(is_import_page(doc, page));
    assert!(!is_import_page(doc, current));
    assert_eq!(doc.pages.len(), pages.len() + 1);

    let set = id_in(&result, "set");
    assert_eq!(sets_on(doc, page), vec![set]);
    assert_eq!(name_of(doc, set), "Button");
    assert_eq!(variant_count(doc, set), 8);

    assert_eq!(bench.session.docs[0].page, current, "the current page does not change");
    assert!(doc.node(current).unwrap().children.is_empty(), "the current page received no layer");
    assert_eq!(project_components(&mut bench), components, "the import page is no component of the library");

    undo(&mut bench);
    assert_eq!(bench.document_text(0), before, "one undo removes the page and all the import added");
    assert_eq!(bench.session.undo_depth().0, depth.0);
}

// covers: E4-1.2
#[test]
fn importing_the_same_component_again_replaces_the_set_in_place_and_one_undo_brings_the_first_back() {
    let mut bench = bench("stage-replace", false);
    let first = import_ok(&mut bench, &staged("Button", &["Default", "Hover"]));
    let page = id_in(&first, "page");
    let first_set = id_in(&first, "set");
    let first_position = position(document_of(&bench), first_set);
    assert_eq!(variant_count(document_of(&bench), first_set), 8);

    let second = import_ok(&mut bench, &staged("Button", &["Default"]));

    let doc = document_of(&bench);
    assert_eq!(id_in(&second, "page"), page, "the page is reused");
    let sets = sets_on(doc, page);
    assert_eq!(sets.len(), 1, "one Button, not two");
    assert_eq!(name_of(doc, sets[0]), "Button");
    assert_eq!(variant_count(doc, sets[0]), 4);
    assert_eq!(position(doc, sets[0]), first_position, "the new set takes the place of the old one");
    assert_eq!(doc.pages.iter().filter(|id| is_import_page(doc, **id)).count(), 1);

    undo(&mut bench);
    let doc = document_of(&bench);
    assert_eq!(sets_on(doc, page), vec![first_set], "the previous set is back");
    assert_eq!(variant_count(doc, first_set), 8);
}

// covers: E4-1.2
#[test]
fn another_component_is_added_next_to_the_first_without_overlap_and_one_undo_removes_only_it() {
    let mut bench = bench("stage-beside", false);
    let first = import_ok(&mut bench, &staged("Button", &["Default", "Hover"]));
    let page = id_in(&first, "page");
    let button = id_in(&first, "set");
    let button_position = position(document_of(&bench), button);

    let second = import_ok(&mut bench, &staged("Chip", &["Default"]));

    let doc = document_of(&bench);
    assert_eq!(id_in(&second, "page"), page);
    let chip = id_in(&second, "set");
    assert_eq!(name_of(doc, chip), "Chip");
    let sets = sets_on(doc, page);
    assert_eq!(sets.len(), 2, "{sets:?}");
    assert!(sets.contains(&button) && sets.contains(&chip));
    assert_eq!(variant_count(doc, button), 8, "the first set is untouched");
    assert_eq!(position(doc, button), button_position);
    assert!(!overlap(doc, button, chip), "the two sets must not overlap");

    undo(&mut bench);
    assert_eq!(sets_on(document_of(&bench), page), vec![button]);
}

fn pump_until(bench: &mut Bench, command: &str) {
    for _ in 0..MAX_PUMPS {
        if bench.session.pump_capture().iter().any(|event| event.command == command) {
            return;
        }
    }
    panic!("no {command} event");
}

// covers: E4-1.3
#[test]
fn a_new_version_of_the_source_opens_a_new_import_page_and_leaves_the_old_one_intact() {
    let mut bench = bench("stage-version", false);
    let first = import_ok(&mut bench, &staged("Button", &["Default", "Hover"]));
    let old_page = id_in(&first, "page");
    let old_children = document_of(&bench).node(old_page).unwrap().children.clone();

    let started = bench.session.execute("project.setSourceVersion", &json!({"package": PACKAGE, "version": NEXT_VERSION})).unwrap();
    assert!(started["job"].as_u64().is_some(), "{started}");
    pump_until(&mut bench, "project.setSourceVersion");
    let info = bench.session.execute("project.info", &json!({})).unwrap();
    assert_eq!(info["project"]["sources"][0]["version"], json!(NEXT_VERSION), "{info}");

    let second = import_ok(&mut bench, &staged("Button", &["Default", "Hover"]));

    let doc = document_of(&bench);
    let new_page = id_in(&second, "page");
    assert_ne!(new_page, old_page);
    assert_eq!(name_of(doc, new_page), "- Imported · synthetic-ui 1.1.0");
    assert_eq!(name_of(doc, new_page), import_page_name(PACKAGE, NEXT_VERSION));
    assert_eq!(sets_on(doc, new_page).len(), 1);
    assert_eq!(name_of(doc, old_page), "- Imported · synthetic-ui 1.0.0");
    assert_eq!(doc.node(old_page).unwrap().children, old_children, "the 1.0.0 page is untouched");
    assert_eq!(sets_on(doc, old_page), vec![id_in(&first, "set")]);
    assert_eq!(variant_count(doc, id_in(&first, "set")), 8);
}

fn failing_bench(tag: &str) -> Bench {
    let failing = FailingRender {
        variant: "outline",
        size: "md",
        state: "hover",
        theme: "light",
        error: service_failure(CaptureErrorKind::Render, "page crashed: boom"),
    };
    Bench::new(tag, &[synthetic_source(false)], Scenario::new(button_inspect(true, false)).failing(failing))
}

// covers: E4-1.4
#[test]
fn a_staged_import_whose_render_fails_creates_no_import_page_and_leaves_the_document_identical() {
    let mut bench = failing_bench("stage-fails");
    let before = bench.document_text(0);
    let depth = bench.session.undo_depth();

    bench.import(&staged("Button", &["Default", "Hover"])).unwrap();
    let events = bench.pump_to_end();

    let event = events.iter().find(|event| event.command == IMPORT).unwrap_or_else(|| panic!("no import event in {events:?}"));
    let Err(error) = &event.outcome else { panic!("the import succeeded: {:?}", event.outcome) };
    assert!(error.message.contains("page crashed: boom"), "{}", error.message);
    assert_eq!(bench.document_text(0), before, "no page, no layer");
    assert_eq!(bench.session.undo_depth(), depth);
}

// covers: E4-1.4
#[test]
fn without_stage_the_set_goes_on_the_current_page_and_the_result_has_no_page() {
    let mut bench = bench("stage-off", false);
    let current = bench.session.docs[0].page;
    let pages = document_of(&bench).pages.clone();

    let result = import_ok(&mut bench, &plain(&["Default", "Hover"]));

    let doc = document_of(&bench);
    assert!(result.get("page").is_none(), "{result}");
    assert_eq!(sets_on(doc, current), vec![id_in(&result, "set")]);
    assert_eq!(doc.pages, pages, "no page was created");
}

fn interaction_count(doc: &Document, set: NodeId) -> usize {
    doc.node(set).unwrap().children.iter().map(|variant| doc.node(*variant).unwrap().interactions.len()).sum()
}

fn effects(doc: &Document, set: NodeId) -> Vec<(String, String)> {
    subtree(doc, set)
        .into_iter()
        .filter_map(|id| {
            let node = doc.node(id).unwrap();
            node.plugin_data.get(EFFECT).map(|effect| (node.name.clone(), effect.clone()))
        })
        .collect()
}

fn has_layer(doc: &Document, set: NodeId, name: &str) -> bool {
    subtree(doc, set).into_iter().any(|id| doc.node(id).unwrap().name == name)
}

// covers: E4-1.5
#[test]
fn without_animations_there_are_no_interactions_and_no_effects_but_the_ripple_layer_stays() {
    let mut bench = bench("anim-off", true);
    let mut request = plain(&["Default", "Hover"]);
    request["animations"] = json!(false);

    let result = import_ok(&mut bench, &request);

    let doc = document_of(&bench);
    let set = id_in(&result, "set");
    assert_eq!(variant_count(doc, set), 8);
    assert_eq!(interaction_count(doc, set), 0);
    assert_eq!(effects(doc, set), Vec::<(String, String)>::new());
    assert!(has_layer(doc, set, "Ripple"), "the ripple is still a layer");
}

// covers: E4-1.5
#[test]
fn animations_omitted_or_true_keep_the_interactions_and_the_effects_of_lot_3() {
    for animations in [None, Some(true)] {
        let mut bench = bench("anim-on", true);
        let mut request = plain(&["Default", "Hover"]);
        if let Some(on) = animations {
            request["animations"] = json!(on);
        }

        let result = import_ok(&mut bench, &request);

        let doc = document_of(&bench);
        let set = id_in(&result, "set");
        assert_eq!(interaction_count(doc, set), 4, "{animations:?}: one hover interaction per Default variant");
        let found = effects(doc, set);
        assert!(found.contains(&("Ripple".to_owned(), "ripple".to_owned())), "{animations:?}: {found:?}");
        assert!(found.contains(&("Spinner".to_owned(), "spinner".to_owned())), "{animations:?}: {found:?}");
    }
}

fn assert_refused(bench: &mut Bench, what: &str, params: &Value, fragments: &[&str]) {
    let before = bench.document_text(bench.session.active);
    let depth = bench.session.undo_depth();
    let submitted = bench.submitted().len();

    let result: Result<Value> = bench.import(params);

    let Err(error) = result else { panic!("{what}: accepted") };
    assert!(matches!(error, EngineError::BadParams { .. }), "{what}: {error:?}");
    let message = error.to_string();
    assert!(fragments.iter().any(|fragment| message.contains(fragment)), "{what}: {message}");
    assert_eq!(bench.submitted().len(), submitted, "{what}: a job was submitted");
    assert_eq!(bench.document_text(bench.session.active), before, "{what}: the document changed");
    assert_eq!(bench.session.undo_depth(), depth, "{what}: an undo step appeared");
}

fn with(extra: &Value) -> Value {
    let mut all = json!({"package": PACKAGE, "component": "Button"});
    if let (Some(base), Some(more)) = (all.as_object_mut(), extra.as_object()) {
        base.extend(more.clone());
    }
    all
}

// covers: E4-1.6
#[test]
fn absurd_values_of_stage_and_animations_and_a_stage_with_a_place_are_refused_at_once() {
    let mut bench = Bench::new("stage-refusals", &[synthetic_source(false)], Scenario::new(button_inspect(true, false)));
    let frame = id_of(&bench.session.execute("frame.create", &json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap()["id"]).unwrap();
    let mut cases = vec![
        ("stage as text", with(&json!({"stage": "true"})), "stage"),
        ("stage as a number", with(&json!({"stage": 1})), "stage"),
        ("stage as a list", with(&json!({"stage": [true]})), "stage"),
        ("stage as an object", with(&json!({"stage": {"on": true}})), "stage"),
        ("animations as text", with(&json!({"animations": "false"})), "animations"),
        ("animations as a number", with(&json!({"animations": 0})), "animations"),
        ("animations as a list", with(&json!({"animations": []})), "animations"),
        ("animations as an object", with(&json!({"animations": {}})), "animations"),
        ("stage with a parent", with(&json!({"stage": true, "parent": frame})), "stag"),
        ("stage with x", with(&json!({"stage": true, "x": 10})), "stag"),
        ("stage with y", with(&json!({"stage": true, "y": 10})), "stag"),
    ];
    cases.push(("stage with x, y and a parent", with(&json!({"stage": true, "x": 1, "y": 2, "parent": frame})), "stag"));
    for (what, request, fragment) in &cases {
        assert_refused(&mut bench, what, request, &[fragment]);
    }
}

// covers: E4-1.6
#[test]
fn an_import_page_name_of_more_than_200_characters_is_refused_at_once() {
    let package = "a".repeat(190);
    assert!(import_page_name(&package, VERSION).chars().count() > 200);
    let source = json!({"package": package, "version": VERSION, "framework": "react", "license": "MIT", "removed": false});
    let mut bench = Bench::new("stage-long-name", &[source], Scenario::new(button_inspect(true, false)));

    assert_refused(&mut bench, "long page name", &json!({"package": package, "component": "Button", "stage": true}), &["200", "page"]);
}
