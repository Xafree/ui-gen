//! `uigen.importComponent`: a capture job per variant, one undo step when they all came back.
//! The service is a scripted backend and the snapshots are synthetic buttons.

use figcraft_doc::{Document, NodeId, NodeKind};
use serde_json::{Value, json};

use crate::capture::{CaptureError, CaptureErrorKind, CaptureEvent, CaptureJob};
use crate::params::id_of;
use crate::uigen_import_support::{Bench, FailingRender, IMPORT, PACKAGE, Scenario, VERSION, button_inspect, service_failure, synthetic_source};
use crate::{EngineError, Result};

const GAP: f64 = 20.0;

fn params(extra: &Value) -> Value {
    let mut all = json!({"package": PACKAGE, "component": "Button"});
    if let (Some(base), Some(more)) = (all.as_object_mut(), extra.as_object()) {
        base.extend(more.clone());
    }
    all
}

fn two_states() -> Value {
    params(&json!({"states": ["Default", "Hover"], "themes": ["light"]}))
}

fn button_bench(tag: &str) -> Bench {
    Bench::new(tag, &[synthetic_source(false)], Scenario::new(button_inspect(true, false)))
}

fn job_of(started: &Value) -> u64 {
    started["job"].as_u64().unwrap_or_else(|| panic!("no job id in {started}"))
}

fn the_event(started: &Value, events: &[CaptureEvent]) -> CaptureEvent {
    assert_eq!(events.len(), 1, "one event per import: {events:?}");
    let event = events[0].clone();
    assert_eq!(event.command, IMPORT);
    assert_eq!(event.package, PACKAGE);
    assert_eq!(event.job, job_of(started));
    event
}

fn import_ok(bench: &mut Bench, params: &Value) -> Value {
    let started = bench.import(params).unwrap();
    let events = bench.pump_to_end();
    match the_event(&started, &events).outcome {
        Ok(result) => result,
        Err(error) => panic!("the import failed: {}", error.message),
    }
}

fn import_err(bench: &mut Bench, params: &Value) -> CaptureError {
    let started = bench.import(params).unwrap();
    let events = bench.pump_to_end();
    match the_event(&started, &events).outcome {
        Err(error) => error,
        Ok(result) => panic!("the import succeeded: {result}"),
    }
}

fn component_sets(doc: &Document) -> Vec<NodeId> {
    doc.nodes().filter(|node| matches!(node.kind, NodeKind::ComponentSet { .. })).map(|node| node.id).collect()
}

fn variant_names(doc: &Document, set: NodeId) -> Vec<String> {
    let mut names: Vec<String> = doc.node(set).unwrap().children.iter().map(|id| doc.node(*id).unwrap().name.clone()).collect();
    names.sort();
    names
}

fn sorted(names: &[&str]) -> Vec<String> {
    let mut names: Vec<String> = names.iter().map(ToString::to_string).collect();
    names.sort();
    names
}

fn layer_paths(doc: &Document, root: NodeId) -> Vec<String> {
    fn walk(doc: &Document, id: NodeId, prefix: &str, out: &mut Vec<String>) {
        let node = doc.node(id).unwrap();
        let path = if prefix.is_empty() { node.name.clone() } else { format!("{prefix} / {}", node.name) };
        out.push(path.clone());
        for child in &node.children {
            walk(doc, *child, &path, out);
        }
    }
    let mut out = Vec::new();
    for child in &doc.node(root).unwrap().children {
        walk(doc, *child, "", &mut out);
    }
    out
}

fn render_jobs(bench: &Bench) -> Vec<(Value, String, String)> {
    bench
        .submitted()
        .into_iter()
        .filter_map(|job| match job {
            CaptureJob::Render { props, state, theme, component, package, .. } => {
                assert_eq!((component.as_str(), package.as_str()), ("Button", PACKAGE));
                Some((props, state, theme))
            }
            _ => None,
        })
        .collect()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn expected_names() -> Vec<String> {
    let mut names = Vec::new();
    for style in ["solid", "outline"] {
        for size in ["sm", "md"] {
            for state in ["Default", "Hover"] {
                names.push(format!("Style={style}, Size={size}, State={state}"));
            }
        }
    }
    names.sort();
    names
}

fn expected_renders() -> Vec<String> {
    let mut renders = Vec::new();
    for style in ["solid", "outline"] {
        for size in ["sm", "md"] {
            for state in ["default", "hover"] {
                renders.push(format!("{style} {size} {state} light"));
            }
        }
    }
    renders.sort();
    renders
}

fn rendered_combinations(bench: &Bench) -> Vec<String> {
    let mut renders: Vec<String> = render_jobs(bench)
        .into_iter()
        .map(|(props, state, theme)| format!("{} {} {state} {theme}", props["variant"].as_str().unwrap(), props["size"].as_str().unwrap()))
        .collect();
    renders.sort();
    renders
}

// covers: E3-5.1
#[test]
fn the_command_answers_with_a_job_and_the_events_bring_the_set_of_eight_variants() {
    let mut bench = button_bench("nominal");
    let before = bench.document_text(0);
    let depth = bench.session.undo_depth();

    let started = bench.import(&two_states()).unwrap();
    assert!(started["job"].as_u64().is_some(), "{started}");
    assert_eq!(bench.document_text(0), before, "nothing is written before the renders are back");
    assert_eq!(bench.session.undo_depth(), depth);
    assert!(matches!(bench.submitted().as_slice(), [CaptureJob::Inspect { package, .. }] if package == PACKAGE), "{:?}", bench.submitted());

    let events = bench.pump_to_end();
    let result = match the_event(&started, &events).outcome {
        Ok(result) => result,
        Err(error) => panic!("the import failed: {}", error.message),
    };
    assert_eq!(result["variants"], json!(8), "{result}");
    assert!(result["warnings"].is_array(), "{result}");
    assert!(result["collection"].is_string(), "{result}");

    let page = bench.session.docs[0].page;
    let doc = bench.session.docs[0].doc.as_ref();
    let set = id_of(&result["set"]).unwrap_or_else(|| panic!("no set in {result}"));
    let node = doc.node(set).unwrap();
    assert!(matches!(node.kind, NodeKind::ComponentSet { .. }), "{:?}", node.kind);
    assert_eq!(node.name, "Button");
    assert_eq!(node.parent, Some(page));
    assert_eq!(component_sets(doc), vec![set]);
    assert!(node.children.iter().all(|id| matches!(doc.node(*id).unwrap().kind, NodeKind::Component { .. })));

    assert_eq!(variant_names(doc, set), expected_names());
    assert_eq!(rendered_combinations(&bench), expected_renders());
    assert_eq!(bench.session.undo_depth().0, depth.0 + 1, "one undo step");
}

// covers: E3-5.2
#[test]
fn the_variants_sit_in_a_grid_of_states_by_style_then_size_and_the_set_remembers_its_source() {
    let mut bench = button_bench("grid");
    let result = import_ok(&mut bench, &two_states());

    let doc = bench.session.docs[0].doc.as_ref();
    let set = id_of(&result["set"]).unwrap();
    let states = ["Default", "Hover"];
    let rows = [("solid", "sm"), ("solid", "md"), ("outline", "sm"), ("outline", "md")];
    let mut cells = Vec::new();
    for child in &doc.node(set).unwrap().children {
        let node = doc.node(*child).unwrap();
        let coeffs = node.transform.as_coeffs();
        let (x, y) = (coeffs[4], coeffs[5]);
        let column = states.iter().position(|state| node.name.ends_with(&format!("State={state}"))).unwrap_or_else(|| panic!("{}", node.name));
        let row = rows
            .iter()
            .position(|(style, size)| node.name.starts_with(&format!("Style={style}, Size={size},")))
            .unwrap_or_else(|| panic!("{}", node.name));
        cells.push((column, row, x, y, node.width, node.height));
    }
    assert_eq!(cells.len(), 8);

    let column_width: Vec<f64> =
        (0..states.len()).map(|c| cells.iter().filter(|cell| cell.0 == c).fold(0.0, |m, cell| f64::max(m, cell.4))).collect();
    let row_height: Vec<f64> = (0..rows.len()).map(|r| cells.iter().filter(|cell| cell.1 == r).fold(0.0, |m, cell| f64::max(m, cell.5))).collect();
    let start = |sizes: &[f64], index: usize| GAP + sizes.iter().take(index).map(|size| size + GAP).sum::<f64>();
    for (column, row, x, y, ..) in &cells {
        assert!(near(*x, start(&column_width, *column)), "column {column}: x = {x}, widths {column_width:?}");
        assert!(near(*y, start(&row_height, *row)), "row {row}: y = {y}, heights {row_height:?}");
    }
    let node = doc.node(set).unwrap();
    assert!(near(node.width, start(&column_width, states.len())), "set width {} for {column_width:?}", node.width);
    assert!(near(node.height, start(&row_height, rows.len())), "set height {} for {row_height:?}", node.height);

    let paths: Vec<Vec<String>> = node.children.iter().map(|id| layer_paths(doc, *id)).collect();
    assert!(!paths[0].is_empty());
    assert!(paths.iter().all(|path| *path == paths[0]), "{paths:?}");

    let source: Value = serde_json::from_str(node.plugin_data.get("uigen.source").unwrap_or_else(|| panic!("{:?}", node.plugin_data))).unwrap();
    assert_eq!(source, json!({"package": PACKAGE, "version": VERSION, "component": "Button"}));
}

// covers: E3-5.3
#[test]
fn one_undo_removes_everything_the_import_added_and_redo_puts_it_back() {
    let mut bench = button_bench("undo");
    let before = bench.document_text(0);
    let depth = bench.session.undo_depth();

    import_ok(&mut bench, &two_states());
    let after = bench.document_text(0);
    assert_ne!(after, before);
    assert_eq!(bench.session.undo_depth().0, depth.0 + 1);

    bench.session.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(bench.document_text(0), before);
    assert_eq!(bench.session.undo_depth().0, depth.0);

    bench.session.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(bench.document_text(0), after);
}

// covers: E3-5.4
#[test]
fn a_failed_render_leaves_the_document_alone_and_names_the_step_and_the_service_message() {
    let failing = FailingRender {
        variant: "outline",
        size: "md",
        state: "hover",
        theme: "light",
        error: service_failure(CaptureErrorKind::Render, "page crashed: boom"),
    };
    let mut bench = Bench::new("render-fails", &[synthetic_source(false)], Scenario::new(button_inspect(true, false)).failing(failing));
    let before = bench.document_text(0);
    let depth = bench.session.undo_depth();

    let error = import_err(&mut bench, &two_states());

    assert!(error.message.contains("render Button Style=outline, Size=md, State=Hover light"), "{}", error.message);
    assert!(error.message.contains("page crashed: boom"), "{}", error.message);
    assert_eq!(bench.document_text(0), before);
    assert_eq!(bench.session.undo_depth(), depth);
}

// covers: E3-5.4
#[test]
fn an_inspect_that_fails_or_does_not_list_the_component_stops_the_import_at_that_step() {
    let without_button = Ok(json!({
        "package": PACKAGE, "version": VERSION, "framework": "react",
        "components": [{"name": "x-chip", "props": []}], "notes": []
    }));
    let cases = [
        (without_button, Some(CaptureErrorKind::ComponentNotFound), "Button"),
        (Err(service_failure(CaptureErrorKind::NotInstalled, "package is gone")), None, "package is gone"),
    ];
    for (inspect, kind, fragment) in cases {
        let mut bench = Bench::new("inspect-fails", &[synthetic_source(false)], Scenario::new(inspect));
        let before = bench.document_text(0);
        let depth = bench.session.undo_depth();

        let error = import_err(&mut bench, &two_states());

        assert!(error.message.contains("inspect"), "{}", error.message);
        assert!(error.message.contains(fragment), "{}", error.message);
        if let Some(kind) = kind {
            assert_eq!(error.kind, kind);
        }
        assert_eq!(bench.submitted().len(), 1, "no render after a failed inspect");
        assert_eq!(bench.document_text(0), before);
        assert_eq!(bench.session.undo_depth(), depth);
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

// covers: E3-5.5
#[test]
fn bad_requests_are_refused_at_once_with_the_cause_and_no_work_is_submitted() {
    let mut bench = button_bench("refusals");
    let rectangle = id_of(&bench.session.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 10, "height": 10})).unwrap()["id"]).unwrap();
    let many_styles: Vec<String> = (0..70).map(|i| format!("style-{i}")).collect();
    let cases = [
        ("unknown package", json!({"package": "other-ui", "component": "Button"}), vec!["other-ui"]),
        ("empty component", params(&json!({"component": ""})), vec!["component"]),
        ("long component", params(&json!({"component": "B".repeat(129)})), vec!["component"]),
        ("unknown state", params(&json!({"states": ["Hover", "Wobble"]})), vec!["Wobble"]),
        ("unknown theme", params(&json!({"themes": ["sepia"]})), vec!["sepia"]),
        ("too many renders", params(&json!({"styles": many_styles, "sizes": ["sm", "md"], "states": ["Default"]})), vec!["280", "128"]),
        ("parent is a rectangle", params(&json!({"parent": rectangle})), vec!["parent"]),
    ];
    for (what, request, fragments) in &cases {
        assert_refused(&mut bench, what, request, fragments);
    }
}

// covers: E3-5.5
#[test]
fn a_removed_source_a_file_outside_a_project_and_a_missing_service_are_refused_too() {
    let mut removed = Bench::new("removed", &[synthetic_source(true)], Scenario::new(button_inspect(true, false)));
    assert_refused(&mut removed, "removed source", &two_states(), &[PACKAGE]);

    let mut outside = button_bench("outside");
    outside.session.execute("file.new", &json!({})).unwrap();
    assert_refused(&mut outside, "outside a project", &two_states(), &["project"]);

    let mut silent = Bench::without_backend("silent", &[synthetic_source(false)]);
    assert_refused(&mut silent, "no backend", &two_states(), &["capture service not available"]);
}

// covers: E3-5.6
#[test]
fn a_tab_closed_before_the_end_fails_the_import_without_writing_anywhere() {
    let mut bench = button_bench("closed");
    bench.session.execute("file.new", &json!({})).unwrap();
    bench.session.active = 0;
    let other = bench.document_text(1);

    let started = bench.import(&two_states()).unwrap();
    bench.session.pump_capture();
    bench.session.execute("file.close", &json!({"index": 0, "force": true})).unwrap();
    let events = bench.pump_to_end();

    let event = the_event(&started, &events);
    assert!(event.outcome.is_err(), "{:?}", event.outcome);
    assert_eq!(bench.session.docs.len(), 1);
    assert_eq!(bench.document_text(0), other);
    assert!(component_sets(bench.session.docs[0].doc.as_ref()).is_empty());
}

// covers: E3-5.6
#[test]
fn the_import_writes_into_the_tab_it_was_started_in_whatever_tab_is_active_at_the_end() {
    let mut bench = button_bench("switched");
    bench.session.execute("file.new", &json!({})).unwrap();
    let other = bench.document_text(1);
    bench.session.active = 0;

    let started = bench.import(&two_states()).unwrap();
    bench.session.active = 1;
    let events = bench.pump_to_end();

    let result = match the_event(&started, &events).outcome {
        Ok(result) => result,
        Err(error) => panic!("the import failed: {}", error.message),
    };
    let set = id_of(&result["set"]).unwrap();
    assert_eq!(component_sets(bench.session.docs[0].doc.as_ref()), vec![set]);
    assert!(component_sets(bench.session.docs[1].doc.as_ref()).is_empty());
    assert_eq!(bench.document_text(1), other);
}

// covers: E3-5.7
#[test]
fn loading_is_rendered_by_default_with_loading_true_when_the_component_reports_the_prop() {
    let mut bench = Bench::new("loading", &[synthetic_source(false)], Scenario::new(button_inspect(true, true)));
    let request = params(&json!({"styles": ["solid"], "sizes": ["sm"], "states": ["Default", "Loading"], "themes": ["light"]}));

    let result = import_ok(&mut bench, &request);

    assert_eq!(result["variants"], json!(2), "{result}");
    let doc = bench.session.docs[0].doc.as_ref();
    let set = id_of(&result["set"]).unwrap();
    assert_eq!(variant_names(doc, set), sorted(&["Style=solid, Size=sm, State=Default", "Style=solid, Size=sm, State=Loading"]));
    let renders = render_jobs(&bench);
    let loading: Vec<_> = renders.iter().filter(|(props, ..)| props.get("loading") == Some(&json!(true))).collect();
    assert_eq!(loading.len(), 1, "{renders:?}");
    assert_eq!(loading[0].1, "default");
    assert_eq!(renders.len(), 2);
}

// covers: E3-5.7
#[test]
fn asking_for_loading_when_the_component_has_no_loading_prop_fails_the_import() {
    let mut bench = button_bench("no-loading");
    let before = bench.document_text(0);
    let request = params(&json!({"styles": ["solid"], "sizes": ["sm"], "states": ["Default", "Loading"], "themes": ["light"]}));

    let error = import_err(&mut bench, &request);

    assert!(error.message.to_lowercase().contains("loading"), "{}", error.message);
    assert_eq!(bench.document_text(0), before);
}

// covers: E3-5.7
#[test]
fn an_axis_without_values_is_left_out_of_the_names_and_of_the_render_props() {
    let mut bench = Bench::new("no-size", &[synthetic_source(false)], Scenario::new(button_inspect(false, false)));

    let result = import_ok(&mut bench, &two_states());

    assert_eq!(result["variants"], json!(4), "{result}");
    let doc = bench.session.docs[0].doc.as_ref();
    let set = id_of(&result["set"]).unwrap();
    assert_eq!(
        variant_names(doc, set),
        sorted(&["Style=solid, State=Default", "Style=solid, State=Hover", "Style=outline, State=Default", "Style=outline, State=Hover"])
    );
    assert!(render_jobs(&bench).iter().all(|(props, ..)| props.get("size").is_none()));
}
