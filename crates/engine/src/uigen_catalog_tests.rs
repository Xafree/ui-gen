//! `uigen.listComponents` (the components of a source, read by the capture service and kept in the
//! session) and `uigen.importWindow` (asks the app for the import window). The service is a
//! scripted backend; the answers are synthetic.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::capture::{CaptureBackend, CaptureErrorKind, CaptureEvent, CaptureJob, CaptureOutcome, JobId};
use crate::uigen_import::{MAX_RENDERS, render_count};
use crate::uigen_import_support::{Bench, PACKAGE, VERSION, button_inspect, service_failure, synthetic_source};
use crate::{EngineError, Result, Session};

const LIST: &str = "uigen.listComponents";
const WINDOW: &str = "uigen.importWindow";
const DIALOG: &str = "dialog.importComponent";
const NEXT_VERSION: &str = "1.1.0";
const MAX_PUMPS: usize = 64;
const QUIET_PUMPS: usize = 3;

struct Shared {
    submitted: Vec<CaptureJob>,
    ready: Vec<(JobId, CaptureOutcome)>,
    inspect: CaptureOutcome,
}

struct Backend {
    shared: Arc<Mutex<Shared>>,
}

impl CaptureBackend for Backend {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let mut shared = self.shared.lock().unwrap();
        let outcome = match &job {
            CaptureJob::Inspect { .. } => shared.inspect.clone(),
            CaptureJob::Install { package, version, .. } => {
                Ok(json!({"package": package, "version": version, "framework": "react", "license": "MIT"}))
            }
            CaptureJob::Uninstall { .. } => Ok(json!({})),
            CaptureJob::Render { .. } => Err(service_failure(CaptureErrorKind::Internal, "unexpected job")),
        };
        shared.submitted.push(job);
        let id = u64::try_from(shared.submitted.len()).unwrap();
        shared.ready.push((id, outcome));
        id
    }

    fn poll(&mut self) -> Vec<(JobId, CaptureOutcome)> {
        std::mem::take(&mut self.shared.lock().unwrap().ready)
    }

    fn shutdown(&mut self) {}
}

type PropShape = (String, String, Vec<String>);
type ComponentShape = (String, Vec<PropShape>);

struct Catalog {
    bench: Bench,
    shared: Arc<Mutex<Shared>>,
}

impl Catalog {
    fn new(tag: &str, sources: &[Value], inspect: CaptureOutcome) -> Self {
        let mut bench = Bench::without_backend(tag, sources);
        let shared = Arc::new(Mutex::new(Shared { submitted: Vec::new(), ready: Vec::new(), inspect }));
        bench.session.set_capture_backend(Box::new(Backend { shared: Arc::clone(&shared) }));
        Self { bench, shared }
    }

    fn nominal(tag: &str) -> Self {
        Self::new(tag, &[synthetic_source(false)], button_inspect(true, false))
    }

    fn answering(tag: &str, inspect: CaptureOutcome) -> Self {
        Self::new(tag, &[synthetic_source(false)], inspect)
    }

    fn silent(tag: &str) -> Self {
        let bench = Bench::without_backend(tag, &[synthetic_source(false)]);
        let shared = Arc::new(Mutex::new(Shared { submitted: Vec::new(), ready: Vec::new(), inspect: button_inspect(true, false) }));
        Self { bench, shared }
    }

    fn session(&mut self) -> &mut Session {
        &mut self.bench.session
    }

    fn run(&mut self, command: &str, params: &Value) -> Result<Value> {
        self.bench.session.execute(command, params)
    }

    fn list(&mut self) -> Result<Value> {
        self.run(LIST, &json!({"package": PACKAGE}))
    }

    fn answer(&self, inspect: CaptureOutcome) {
        self.shared.lock().unwrap().inspect = inspect;
    }

    fn submitted(&self) -> Vec<CaptureJob> {
        self.shared.lock().unwrap().submitted.clone()
    }

    fn inspect_jobs(&self) -> usize {
        self.submitted().iter().filter(|job| matches!(job, CaptureJob::Inspect { package, .. } if package == PACKAGE)).count()
    }

    fn text(&self) -> String {
        self.bench.document_text(self.bench.session.active)
    }

    fn depth(&self) -> (usize, usize) {
        self.bench.session.undo_depth()
    }

    fn pump(&mut self) -> Vec<CaptureEvent> {
        let mut events = Vec::new();
        for _ in 0..MAX_PUMPS {
            events.extend(self.bench.session.pump_capture());
            if !events.is_empty() {
                break;
            }
        }
        for _ in 0..QUIET_PUMPS {
            events.extend(self.bench.session.pump_capture());
        }
        events
    }

    fn listing(&mut self) -> CaptureEvent {
        let started = self.list().unwrap();
        let events = self.pump();
        let event = listing_event(&events);
        assert_eq!(Some(event.job), started["job"].as_u64(), "{started}");
        event
    }

    fn shape(&mut self) -> Option<Vec<ComponentShape>> {
        let inspected = self.session().inspected(PACKAGE)?;
        let components = inspected
            .components
            .iter()
            .map(|component| {
                let props = component.props.iter().map(|prop| (prop.name.clone(), format!("{:?}", prop.kind), prop.values.clone())).collect();
                (component.name.clone(), props)
            })
            .collect();
        Some(components)
    }

    fn names(&mut self) -> Vec<String> {
        self.shape().unwrap_or_else(|| panic!("nothing in the cache")).into_iter().map(|(name, _)| name).collect()
    }

    fn notes(&mut self) -> Vec<String> {
        self.session().inspected(PACKAGE).unwrap_or_else(|| panic!("nothing in the cache")).notes.clone()
    }

    fn cached_version(&mut self) -> Option<String> {
        self.session().inspected(PACKAGE).map(|inspected| inspected.version.clone())
    }
}

fn listing_event(events: &[CaptureEvent]) -> CaptureEvent {
    let mut found = events.iter().filter(|event| event.command == LIST);
    let event = found.next().unwrap_or_else(|| panic!("no listing event in {events:?}")).clone();
    assert!(found.next().is_none(), "one listing event: {events:?}");
    assert_eq!(event.package, PACKAGE);
    event
}

fn prop(name: &str, kind: &str, values: &[&str]) -> Value {
    json!({"name": name, "type": kind, "values": values})
}

fn component(name: &str, props: Vec<Value>) -> Value {
    json!({"name": name, "props": props})
}

fn bare(names: impl IntoIterator<Item = String>) -> Vec<Value> {
    names.into_iter().map(|name| component(&name, Vec::new())).collect()
}

fn inspected_with(version: &str, components: Vec<Value>, notes: Vec<Value>) -> CaptureOutcome {
    Ok(json!({"package": PACKAGE, "version": version, "framework": "react", "components": components, "notes": notes}))
}

fn only(components: Vec<Value>) -> CaptureOutcome {
    inspected_with(VERSION, components, Vec::new())
}

fn button_shape() -> ComponentShape {
    let variant = ("variant".to_owned(), "Enum".to_owned(), vec!["solid".to_owned(), "outline".to_owned()]);
    let size = ("size".to_owned(), "Enum".to_owned(), vec!["sm".to_owned(), "md".to_owned()]);
    let disabled = ("disabled".to_owned(), "Boolean".to_owned(), Vec::new());
    ("Button".to_owned(), vec![variant, size, disabled])
}

fn listed_names(result: &Value) -> Vec<String> {
    result["components"]
        .as_array()
        .unwrap_or_else(|| panic!("no components in {result}"))
        .iter()
        .filter_map(|c| c["name"].as_str())
        .map(str::to_owned)
        .collect()
}

// covers: E4-2.1
#[test]
fn listing_a_source_submits_an_inspect_and_the_session_keeps_the_components_with_their_props() {
    let mut catalog = Catalog::nominal("list-nominal");
    let (before, depth) = (catalog.text(), catalog.depth());
    assert!(catalog.session().inspected(PACKAGE).is_none(), "nothing is known before the first listing");

    let started = catalog.list().unwrap();

    assert!(started["job"].as_u64().is_some(), "{started}");
    assert!(started.get("cached").is_none(), "{started}");
    assert!(
        matches!(catalog.submitted().as_slice(), [CaptureJob::Inspect { package, root }] if package == PACKAGE && !root.is_empty()),
        "{:?}",
        catalog.submitted()
    );
    assert_eq!(catalog.text(), before, "the document is not touched");
    assert_eq!(catalog.depth(), depth);

    let events = catalog.pump();
    let event = listing_event(&events);
    assert_eq!(Some(event.job), started["job"].as_u64());
    let result = match event.outcome {
        Ok(result) => result,
        Err(error) => panic!("the listing failed: {}", error.message),
    };
    assert_eq!(
        (result["package"].as_str(), result["version"].as_str(), result["framework"].as_str()),
        (Some(PACKAGE), Some(VERSION), Some("react")),
        "{result}"
    );
    assert_eq!(listed_names(&result), ["Button", "x-chip"]);
    assert_eq!(result["notes"], json!([]));

    let shape = catalog.shape().unwrap_or_else(|| panic!("the components are not kept"));
    assert_eq!(shape.first(), Some(&button_shape()));
    assert_eq!(shape.get(1).map(|(name, _)| name.as_str()), Some("x-chip"));
    assert_eq!(catalog.cached_version().as_deref(), Some(VERSION));
    assert_eq!(catalog.text(), before, "the document is still untouched");
    assert_eq!(catalog.depth(), depth);
}

// covers: E4-2.1
#[test]
fn a_second_listing_answers_from_the_session_and_submits_no_work() {
    let mut catalog = Catalog::nominal("list-cached");
    catalog.listing();
    assert_eq!(catalog.submitted().len(), 1);

    let again = catalog.list().unwrap();

    assert_eq!(again["cached"], json!(true), "{again}");
    assert!(again.get("job").is_none(), "{again}");
    assert_eq!(
        (again["package"].as_str(), again["version"].as_str(), again["framework"].as_str()),
        (Some(PACKAGE), Some(VERSION), Some("react")),
        "{again}"
    );
    assert_eq!(listed_names(&again), ["Button", "x-chip"]);
    assert_eq!(again["notes"], json!([]));
    assert_eq!(catalog.submitted().len(), 1, "the service saw no new work");
    assert!(catalog.pump().is_empty(), "no event for a cached answer");
}

// covers: E4-2.2
#[test]
fn refresh_asks_the_service_again_in_spite_of_the_cache_and_keeps_the_new_answer() {
    let mut catalog = Catalog::nominal("list-refresh");
    catalog.listing();
    catalog.answer(only(vec![component("Card", Vec::new())]));

    let plain = catalog.run(LIST, &json!({"package": PACKAGE, "refresh": false})).unwrap();
    assert_eq!(plain["cached"], json!(true), "{plain}");
    assert_eq!(catalog.inspect_jobs(), 1);

    let refreshed = catalog.run(LIST, &json!({"package": PACKAGE, "refresh": true})).unwrap();
    assert!(refreshed["job"].as_u64().is_some(), "{refreshed}");
    assert!(refreshed.get("cached").is_none(), "{refreshed}");
    assert_eq!(catalog.inspect_jobs(), 2, "a new inspect was submitted");

    let events = catalog.pump();
    assert!(listing_event(&events).outcome.is_ok());
    assert_eq!(catalog.names(), ["Card"]);
    let after = catalog.list().unwrap();
    assert_eq!(after["cached"], json!(true), "{after}");
    assert_eq!(listed_names(&after), ["Card"]);
}

// covers: E4-2.2
#[test]
fn a_finished_version_change_empties_the_cache_until_the_next_listing() {
    let mut catalog = Catalog::nominal("list-version");
    catalog.listing();
    assert!(catalog.session().inspected(PACKAGE).is_some());

    let started = catalog.run("project.setSourceVersion", &json!({"package": PACKAGE, "version": NEXT_VERSION})).unwrap();
    assert!(started["job"].as_u64().is_some(), "{started}");
    let events = catalog.pump();
    assert!(events.iter().any(|event| event.command == "project.setSourceVersion" && event.outcome.is_ok()), "{events:?}");
    assert!(catalog.session().inspected(PACKAGE).is_none(), "the old version's list is gone");

    catalog.answer(inspected_with(NEXT_VERSION, vec![component("Card", Vec::new())], Vec::new()));
    let again = catalog.list().unwrap();
    assert!(again["job"].as_u64().is_some(), "not served from the cache: {again}");
    assert_eq!(catalog.inspect_jobs(), 2);
    assert!(listing_event(&catalog.pump()).outcome.is_ok());
    assert_eq!(catalog.cached_version().as_deref(), Some(NEXT_VERSION));
    assert_eq!(catalog.names(), ["Card"]);
}

// covers: E4-2.2
#[test]
fn a_removed_source_empties_the_cache_and_can_no_longer_be_listed() {
    let mut catalog = Catalog::nominal("list-removed");
    catalog.listing();
    assert!(catalog.session().inspected(PACKAGE).is_some());

    catalog.run("project.removeSource", &json!({"package": PACKAGE})).unwrap();
    let events = catalog.pump();
    assert!(events.iter().any(|event| event.command == "project.removeSource" && event.outcome.is_ok()), "{events:?}");
    assert!(catalog.session().inspected(PACKAGE).is_none());

    let error = catalog.list().unwrap_err();
    assert!(matches!(error, EngineError::BadParams { .. }), "{error:?}");
    assert!(error.to_string().contains(PACKAGE), "{error}");
    assert_eq!(catalog.inspect_jobs(), 1);
}

fn assert_refused(catalog: &mut Catalog, command: &str, what: &str, params: &Value, fragments: &[&str]) {
    let (before, depth, submitted) = (catalog.text(), catalog.depth(), catalog.submitted().len());
    let requests = catalog.session().ui_requests.len();

    let result = catalog.run(command, params);

    let Err(error) = result else { panic!("{what}: accepted") };
    assert!(matches!(error, EngineError::BadParams { .. }), "{what}: {error:?}");
    let message = error.to_string();
    assert!(fragments.iter().any(|fragment| message.contains(fragment)), "{what}: {message}");
    assert_eq!(catalog.submitted().len(), submitted, "{what}: a job was submitted");
    assert_eq!(catalog.text(), before, "{what}: the document changed");
    assert_eq!(catalog.depth(), depth, "{what}: an undo step appeared");
    assert_eq!(catalog.session().ui_requests.len(), requests, "{what}: a window was asked for");
}

// covers: E4-2.3
#[test]
fn bad_listing_requests_are_refused_at_once_with_the_cause_and_no_work_is_submitted() {
    let mut catalog = Catalog::nominal("list-refusals");
    let cases = [
        ("no package", json!({}), vec!["package"]),
        ("package that is not text", json!({"package": 5}), vec!["package"]),
        ("empty package", json!({"package": ""}), vec!["package"]),
        ("package that is not a source", json!({"package": "other-ui"}), vec!["other-ui"]),
        ("invalid package name", json!({"package": "../evil"}), vec!["package", "../evil"]),
        ("refresh that is a string", json!({"package": PACKAGE, "refresh": "yes"}), vec!["refresh"]),
        ("refresh that is a number", json!({"package": PACKAGE, "refresh": 1}), vec!["refresh"]),
    ];
    for (what, params, fragments) in &cases {
        assert_refused(&mut catalog, LIST, what, params, fragments);
    }
}

// covers: E4-2.3
#[test]
fn a_removed_source_a_file_outside_a_project_and_a_missing_service_refuse_the_listing_too() {
    let mut removed = Catalog::new("list-gone", &[synthetic_source(true)], button_inspect(true, false));
    assert_refused(&mut removed, LIST, "removed source", &json!({"package": PACKAGE}), &[PACKAGE]);

    let mut outside = Catalog::nominal("list-outside");
    outside.session().execute("file.new", &json!({})).unwrap();
    assert_refused(&mut outside, LIST, "outside a project", &json!({"package": PACKAGE}), &["project"]);

    let mut silent = Catalog::silent("list-silent");
    assert_refused(&mut silent, LIST, "no backend", &json!({"package": PACKAGE}), &["capture service not available"]);
    assert!(silent.session().inspected(PACKAGE).is_none());
}

// covers: E4-2.3
#[test]
fn absurd_parameters_never_panic_never_submit_work_and_never_touch_the_document() {
    let mut catalog = Catalog::nominal("list-junk");
    let (before, depth) = (catalog.text(), catalog.depth());
    let long = "p".repeat(300);
    let junk = [
        json!(null),
        json!([1, 2, 3]),
        json!({"package": ["a"]}),
        json!({"package": {"x": 1}}),
        json!({"package": "\u{0}"}),
        json!({"package": long}),
        json!({"package": "x[\u{7}", "refresh": 1e308}),
        json!({"package": PACKAGE, "refresh": {"x": 1}}),
        json!({"package": PACKAGE, "refresh": [true]}),
    ];
    for params in &junk {
        let result = catalog.run(LIST, params);
        let Err(error) = result else { panic!("{params}: accepted") };
        assert!(matches!(error, EngineError::BadParams { .. }), "{params}: {error:?}");
    }
    let window_junk =
        [json!({"package": 5}), json!({"package": ["a"]}), json!({"package": "\u{0}"}), json!({"package": "other-ui"}), json!({"package": long})];
    for params in &window_junk {
        let result = catalog.run(WINDOW, params);
        let Err(error) = result else { panic!("{params}: accepted") };
        assert!(matches!(error, EngineError::BadParams { .. }), "{params}: {error:?}");
    }
    assert!(catalog.submitted().is_empty());
    assert_eq!(catalog.text(), before);
    assert_eq!(catalog.depth(), depth);
    assert!(catalog.session().ui_requests.iter().all(|request| request.kind != DIALOG));
}

fn failed_listing(what: &str, inspect: CaptureOutcome) {
    let mut catalog = Catalog::answering("list-protocol", inspect);

    let event = catalog.listing();

    let Err(error) = event.outcome else { panic!("{what}: the listing succeeded") };
    assert_eq!(error.kind, CaptureErrorKind::Protocol, "{what}: {}", error.message);
    assert!(catalog.session().inspected(PACKAGE).is_none(), "{what}: something was cached");
    assert!(catalog.list().unwrap()["job"].as_u64().is_some(), "{what}: the next listing must ask the service again");
    assert_eq!(catalog.inspect_jobs(), 2, "{what}");
}

// covers: E4-2.4
#[test]
fn a_result_without_a_components_array_is_a_protocol_failure_and_nothing_is_cached() {
    failed_listing("missing", Ok(json!({"package": PACKAGE, "version": VERSION, "framework": "react", "notes": []})));
    failed_listing("null", Ok(json!({"package": PACKAGE, "version": VERSION, "framework": "react", "components": null, "notes": []})));
    failed_listing("object", Ok(json!({"package": PACKAGE, "version": VERSION, "framework": "react", "components": {"Button": {}}, "notes": []})));
}

// covers: E4-2.4
#[test]
fn a_component_with_an_empty_too_long_or_control_character_name_is_dropped_with_a_note_that_names_it() {
    let too_long = format!("Zyx{}", "y".repeat(126));
    let longest_kept = "K".repeat(128);
    assert_eq!((too_long.chars().count(), longest_kept.chars().count()), (129, 128));
    let components = vec![
        component("Button", Vec::new()),
        component("", Vec::new()),
        component(&too_long, Vec::new()),
        component("Bad\u{7}Name", Vec::new()),
        component(&longest_kept, Vec::new()),
    ];
    let mut catalog = Catalog::answering("list-names", only(components));

    let event = catalog.listing();

    let result = event.outcome.unwrap_or_else(|error| panic!("the listing failed: {}", error.message));
    assert_eq!(catalog.names(), ["Button".to_owned(), longest_kept.clone()]);
    assert_eq!(listed_names(&result), ["Button".to_owned(), longest_kept]);
    let notes = catalog.notes();
    assert!(notes.iter().any(|note| note.contains("Zyx")), "no note names the long name: {notes:?}");
    assert!(notes.len() >= 2, "the other dropped entries are reported too: {notes:?}");
    let reported: Vec<String> =
        result["notes"].as_array().unwrap_or_else(|| panic!("no notes in {result}")).iter().filter_map(|n| n.as_str().map(str::to_owned)).collect();
    assert_eq!(reported, notes);
}

// covers: E4-2.4
#[test]
fn a_thousand_components_are_kept_and_the_rest_is_cut_with_a_note() {
    let named = |count: usize| bare((0..count).map(|index| format!("C{index}")));

    let mut exact = Catalog::answering("list-1000", only(named(1000)));
    exact.listing();
    assert_eq!(exact.names().len(), 1000);
    assert!(exact.notes().is_empty(), "{:?}", exact.notes());

    let mut over = Catalog::answering("list-1001", only(named(1001)));
    over.listing();
    let names = over.names();
    assert_eq!((names.len(), names.last().map(String::as_str)), (1000, Some("C999")));
    assert!(!over.notes().is_empty(), "the cut is said");
}

// covers: E4-2.4
#[test]
fn props_and_values_are_capped_at_64_and_an_unknown_type_becomes_other() {
    let many_props: Vec<Value> = (0..65).map(|index| prop(&format!("p{index}"), "string", &[])).collect();
    let many_values: Vec<String> = (0..65).map(|index| format!("v{index}")).collect();
    let values: Vec<&str> = many_values.iter().map(String::as_str).collect();
    let exact_values: Vec<&str> = values.iter().copied().take(64).collect();
    let kinds = vec![
        prop("tone", "enum", &values),
        prop("exact", "enum", &exact_values),
        prop("on", "boolean", &[]),
        prop("label", "string", &[]),
        prop("count", "number", &[]),
        prop("render", "function", &[]),
        prop("icon", "other", &[]),
    ];
    let mut catalog = Catalog::answering("list-caps", only(vec![component("Many", many_props), component("Kinds", kinds)]));

    catalog.listing();

    let shape = catalog.shape().unwrap_or_else(|| panic!("nothing cached"));
    let many = &shape[0].1;
    assert_eq!(many.len(), 64);
    assert_eq!(many.last().map(|(name, ..)| name.as_str()), Some("p63"));
    let kinds = &shape[1].1;
    let kind_of = |name: &str| kinds.iter().find(|(prop, ..)| prop == name).map(|(_, kind, _)| kind.as_str());
    assert_eq!(kinds[0].2.len(), 64);
    assert_eq!(kinds[0].2.last().map(String::as_str), Some("v63"));
    assert_eq!(kinds[1].2.len(), 64, "64 values fit");
    assert_eq!(
        [kind_of("tone"), kind_of("on"), kind_of("label"), kind_of("count"), kind_of("render"), kind_of("icon")],
        [Some("Enum"), Some("Boolean"), Some("String"), Some("Number"), Some("Other"), Some("Other")]
    );
    assert!(kinds.iter().filter(|(_, kind, _)| kind != "Enum").all(|(.., values)| values.is_empty()));
}

// covers: E4-2.4
#[test]
fn the_notes_of_the_service_are_kept_within_sixteen_notes_of_256_characters() {
    let notes: Vec<Value> = (0..20).map(|index| json!(format!("{index}{}", "n".repeat(300)))).collect();
    let mut catalog = Catalog::answering("list-notes", inspected_with(VERSION, vec![component("Button", Vec::new())], notes));

    catalog.listing();

    let kept = catalog.notes();
    assert!(!kept.is_empty() && kept.len() <= 16, "{} notes", kept.len());
    assert!(kept.iter().all(|note| note.chars().count() <= 256));
    assert!(kept[0].starts_with('0'));
}

// covers: E4-2.5
#[test]
fn a_failed_inspect_ends_the_listing_with_the_cause_of_the_service_and_caches_nothing() {
    let mut catalog = Catalog::answering("list-failed", Err(service_failure(CaptureErrorKind::NotInstalled, "package is gone")));
    let (before, depth) = (catalog.text(), catalog.depth());

    let event = catalog.listing();

    let Err(error) = event.outcome else { panic!("the listing succeeded") };
    assert!(error.message.contains("package is gone"), "{}", error.message);
    assert!(catalog.session().inspected(PACKAGE).is_none());
    assert_eq!(catalog.text(), before);
    assert_eq!(catalog.depth(), depth);

    catalog.answer(button_inspect(true, false));
    let again = catalog.list().unwrap();
    assert!(again["job"].as_u64().is_some(), "the failure was not cached: {again}");
    assert_eq!(catalog.inspect_jobs(), 2, "a new inspect was submitted");
    assert!(listing_event(&catalog.pump()).outcome.is_ok());
    assert_eq!(catalog.names(), ["Button", "x-chip"]);
}

// covers: E4-2.6
#[test]
fn the_import_window_command_only_asks_the_app_for_the_window_with_the_package() {
    let mut catalog = Catalog::nominal("window-nominal");
    let (before, depth) = (catalog.text(), catalog.depth());
    catalog.session().ui_requests.clear();

    let result = catalog.run(WINDOW, &json!({"package": PACKAGE})).unwrap();

    assert_eq!(result, json!({}));
    let requests = catalog.session().ui_requests.clone();
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert_eq!((requests[0].kind.as_str(), &requests[0].data), (DIALOG, &json!({"package": PACKAGE})));
    assert!(requests[0].kind.starts_with("dialog."), "the control channel drops what starts with dialog.");
    assert_eq!(catalog.text(), before);
    assert_eq!(catalog.depth(), depth);
    assert!(catalog.submitted().is_empty(), "the window asks for no work");
}

// covers: E4-2.6
#[test]
fn the_import_window_without_a_package_asks_for_the_window_with_no_package() {
    let mut catalog = Catalog::nominal("window-anywhere");
    catalog.session().ui_requests.clear();

    catalog.run(WINDOW, &json!({})).unwrap();

    let requests = catalog.session().ui_requests.clone();
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert_eq!(requests[0].kind, DIALOG);
    assert!(requests[0].data["package"].is_null(), "{}", requests[0].data);
}

// covers: E4-2.6
#[test]
fn the_import_window_is_refused_outside_a_project_and_for_an_unknown_or_removed_package_with_no_request() {
    let mut catalog = Catalog::nominal("window-refusals");
    catalog.session().ui_requests.clear();
    assert_refused(&mut catalog, WINDOW, "unknown package", &json!({"package": "other-ui"}), &["other-ui"]);
    assert!(catalog.session().ui_requests.is_empty());

    let mut removed = Catalog::new("window-gone", &[synthetic_source(true)], button_inspect(true, false));
    removed.session().ui_requests.clear();
    assert_refused(&mut removed, WINDOW, "removed source", &json!({"package": PACKAGE}), &[PACKAGE]);
    assert!(removed.session().ui_requests.is_empty());

    let mut outside = Catalog::nominal("window-outside");
    outside.session().execute("file.new", &json!({})).unwrap();
    outside.session().ui_requests.clear();
    for params in [json!({"package": PACKAGE}), json!({})] {
        assert_refused(&mut outside, WINDOW, "outside a project", &params, &["project"]);
    }
    assert!(outside.session().ui_requests.is_empty());
}

// covers: E4-2.7
#[test]
fn the_render_count_is_the_product_of_the_axes_and_an_empty_style_or_size_axis_counts_for_one() {
    assert_eq!(render_count(3, 2, 4, 2), 48);
    assert_eq!(render_count(0, 2, 4, 2), 16, "no style axis");
    assert_eq!(render_count(3, 0, 4, 2), 24, "no size axis");
    assert_eq!(render_count(0, 0, 4, 2), 8);
    assert_eq!(render_count(1, 1, 1, 1), 1);
    assert_eq!(render_count(0, 0, 1, 1), 1);
}

// covers: E4-2.7
#[test]
fn the_render_count_saturates_and_refusal_starts_one_render_above_the_maximum() {
    assert_eq!(render_count(usize::MAX, 2, 1, 1), usize::MAX);
    assert_eq!(render_count(usize::MAX, usize::MAX, usize::MAX, usize::MAX), usize::MAX);
    assert!(render_count(usize::MAX, 2, 1, 1) > MAX_RENDERS);

    assert_eq!(MAX_RENDERS, 128);
    assert!(render_count(4, 4, 4, 2) <= MAX_RENDERS, "128 renders are allowed");
    assert!(render_count(129, 1, 1, 1) > MAX_RENDERS, "129 renders are refused");
    assert!(render_count(4, 4, 4, 3) > MAX_RENDERS);
}
