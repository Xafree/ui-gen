//! `uigen.updateSource` (recapture the imported components into the import page of the current
//! version), `uigen.sourceDiff` (what changed between two captures, against the copy in the library)
//! and `uigen.applyDifferences` (take those changes into the copy). The service is a scripted backend
//! that renders another snapshot per installed version; the copy is made by `uigen.placeImported`.

use figcraft_doc::{Document, NodeId, NodeKind};
use serde_json::{Value, json};

use crate::capture::{CaptureEvent, CaptureJob};
use crate::params::id_of;
use crate::uigen_import::import_page_name;
use crate::uigen_import_support::{Bench, Evolving, IMPORT, NEXT_VERSION, PACKAGE, VERSION, crowded_button, evolving_button};
use crate::{EngineError, OPTIONS_KEY};

const UPDATE: &str = "uigen.updateSource";
const DIFF: &str = "uigen.sourceDiff";
const APPLY: &str = "uigen.applyDifferences";
const PLACE: &str = "uigen.placeImported";
const SOURCE: &str = "uigen.source";
const MAX_PUMPS: usize = 64;
const MAX_DIFFERENCES: usize = 2000;
const USER_FILL: &str = "#FF00FF";

fn run(bench: &mut Bench, command: &str, params: &Value) -> Value {
    bench.session.execute(command, params).unwrap_or_else(|error| panic!("{command} {params}: {error}"))
}

fn doc(bench: &Bench) -> &Document {
    bench.session.docs[0].doc.as_ref()
}

fn id_in(result: &Value, key: &str) -> NodeId {
    id_of(&result[key]).unwrap_or_else(|| panic!("no {key} in {result}"))
}

fn text_in(result: &Value, key: &str) -> String {
    result[key].as_str().unwrap_or_else(|| panic!("no text {key} in {result}")).to_owned()
}

fn pump_all(bench: &mut Bench) -> Vec<CaptureEvent> {
    (0..MAX_PUMPS).flat_map(|_| bench.session.pump_capture()).collect()
}

fn name_of(doc: &Document, id: NodeId) -> String {
    doc.node(id).unwrap().name.clone()
}

fn children_of(doc: &Document, id: NodeId) -> Vec<NodeId> {
    doc.node(id).unwrap().children.clone()
}

fn subtree(doc: &Document, id: NodeId) -> Vec<NodeId> {
    let mut out = vec![id];
    for child in children_of(doc, id) {
        out.extend(subtree(doc, child));
    }
    out
}

fn dump(doc: &Document, root: NodeId) -> String {
    subtree(doc, root).into_iter().map(|id| format!("{:?}", doc.node(id).unwrap())).collect::<Vec<_>>().join("\n")
}

fn layer_named(doc: &Document, root: NodeId, name: &str) -> Option<NodeId> {
    subtree(doc, root).into_iter().find(|id| name_of(doc, *id) == name)
}

fn variant_in(doc: &Document, set: NodeId, state: &str) -> NodeId {
    let suffix = format!("State={state}");
    children_of(doc, set).into_iter().find(|id| name_of(doc, *id).ends_with(&suffix)).unwrap_or_else(|| panic!("no {state} variant in the set"))
}

fn icon_of(doc: &Document, set: NodeId) -> NodeId {
    layer_named(doc, variant_in(doc, set, "Default"), "Icon").unwrap_or_else(|| panic!("no Icon in the default variant"))
}

fn fills_of(doc: &Document, id: NodeId) -> String {
    format!("{:?}", doc.node(id).unwrap().fills)
}

fn page_named(doc: &Document, name: &str) -> NodeId {
    doc.pages.iter().copied().find(|page| name_of(doc, *page) == name).unwrap_or_else(|| panic!("no page {name:?}"))
}

fn set_named(doc: &Document, page: NodeId, name: &str) -> NodeId {
    children_of(doc, page)
        .into_iter()
        .find(|id| matches!(doc.node(*id).unwrap().kind, NodeKind::ComponentSet { .. }) && name_of(doc, *id) == name)
        .unwrap_or_else(|| panic!("no set {name:?} on the page"))
}

fn set_names_on(doc: &Document, page: NodeId) -> Vec<String> {
    let mut names: Vec<String> = children_of(doc, page)
        .into_iter()
        .filter(|id| matches!(doc.node(*id).unwrap().kind, NodeKind::ComponentSet { .. }))
        .map(|id| name_of(doc, id))
        .collect();
    names.sort();
    names
}

fn source_version(doc: &Document, copy: NodeId) -> String {
    let raw = doc.node(copy).unwrap().plugin_data.get(SOURCE).unwrap_or_else(|| panic!("no {SOURCE} on the copy"));
    text_in(&serde_json::from_str::<Value>(raw).unwrap(), "version")
}

fn options_of(doc: &Document, set: NodeId) -> Value {
    let raw = doc.node(set).unwrap().plugin_data.get(OPTIONS_KEY).unwrap_or_else(|| panic!("no {OPTIONS_KEY} on the set"));
    serde_json::from_str(raw).unwrap()
}

fn old_collection_dump(doc: &Document) -> String {
    let collection = doc.variables.collections.iter().find(|collection| collection.name == "synthetic-ui 1.0.0").expect("the 1.0.0 collection");
    let variables: Vec<String> =
        doc.variables.variables.iter().filter(|variable| variable.collection == collection.id).map(|variable| format!("{variable:?}")).collect();
    format!("{collection:?}\n{}", variables.join("\n"))
}

fn staged(component: &str, states: &[&str]) -> Value {
    json!({"package": PACKAGE, "component": component, "styles": ["solid"], "sizes": ["md"], "states": states, "themes": ["light"], "stage": true})
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

fn button_service() -> Evolving {
    Evolving { snapshot: evolving_button, failing_at: None }
}

struct Lab {
    bench: Bench,
    old_page: NodeId,
    old_set: NodeId,
    library_page: NodeId,
    copy: NodeId,
}

fn lab_at_first_version(tag: &str, service: Evolving, states: &[&str]) -> Lab {
    let mut bench = Bench::evolving(tag, service);
    let imported = import_ok(&mut bench, &staged("Button", states));
    let (old_page, old_set) = (id_in(&imported, "page"), id_in(&imported, "set"));
    let placed = run(&mut bench, PLACE, &json!({"set": old_set}));
    Lab { bench, old_page, old_set, library_page: id_in(&placed, "page"), copy: id_in(&placed, "set") }
}

fn move_to_next_version(bench: &mut Bench) {
    run(bench, "project.setSourceVersion", &json!({"package": PACKAGE, "version": NEXT_VERSION}));
    let events = pump_all(bench);
    assert!(events.iter().any(|event| event.command == "project.setSourceVersion" && event.outcome.is_ok()), "{events:?}");
}

fn lab_after_release(tag: &str, states: &[&str]) -> Lab {
    let mut lab = lab_at_first_version(tag, button_service(), states);
    move_to_next_version(&mut lab.bench);
    lab
}

fn recapture(bench: &mut Bench) -> Value {
    let started = run(bench, UPDATE, &json!({"package": PACKAGE}));
    let events = pump_all(bench);
    let failures: Vec<&CaptureEvent> = events.iter().filter(|event| event.outcome.is_err()).collect();
    assert!(failures.is_empty(), "{failures:?}");
    started
}

fn recaptured(tag: &str, states: &[&str]) -> Lab {
    let mut lab = lab_after_release(tag, states);
    recapture(&mut lab.bench);
    lab
}

fn modified_lab(tag: &str) -> (Lab, NodeId) {
    let mut lab = recaptured(tag, &["Default", "Hover"]);
    let icon = icon_of(doc(&lab.bench), lab.copy);
    run(&mut lab.bench, "fill.set", &json!({"ids": [icon], "index": 0, "color": USER_FILL}));
    (lab, icon)
}

fn list(bench: &mut Bench, copy: NodeId) -> Value {
    run(bench, DIFF, &json!({"copy": copy}))
}

struct Found {
    id: String,
    variant: String,
    path: String,
    field: String,
    state: String,
}

fn difference_of(raw: &Value) -> Found {
    Found {
        id: text_in(raw, "id"),
        variant: text_in(raw, "variant"),
        path: text_in(raw, "path"),
        field: text_in(raw, "field"),
        state: text_in(raw, "state"),
    }
}

fn raw_differences(result: &Value) -> &Vec<Value> {
    result["differences"].as_array().unwrap_or_else(|| panic!("no differences in {result}"))
}

fn found(result: &Value) -> Vec<Found> {
    raw_differences(result).iter().map(difference_of).collect()
}

fn of_kind(result: &Value, kind: &str) -> Found {
    let matching: Vec<&Value> = raw_differences(result).iter().filter(|raw| raw["kind"] == json!(kind)).collect();
    assert_eq!(matching.len(), 1, "one {kind} difference expected in {result}");
    difference_of(matching[0])
}

fn sorted_ids(result: &Value) -> Vec<String> {
    let mut ids: Vec<String> = found(result).into_iter().map(|difference| difference.id).collect();
    ids.sort();
    ids
}

fn strings(result: &Value, key: &str) -> Vec<String> {
    let list = result[key].as_array().unwrap_or_else(|| panic!("no {key} in {result}"));
    let mut found: Vec<String> = list.iter().filter_map(|item| item.as_str().map(str::to_owned)).collect();
    found.sort();
    found
}

fn renders(jobs: &[CaptureJob]) -> Vec<String> {
    let mut found: Vec<String> = jobs
        .iter()
        .filter_map(|job| match job {
            CaptureJob::Render { component, props, state, theme, .. } => Some(format!("{component} {props} {state} {theme}")),
            _ => None,
        })
        .collect();
    found.sort();
    found
}

struct Frozen {
    old_page: String,
    library_page: String,
    pages: Vec<NodeId>,
    old_collection: String,
    copy: String,
}

fn freeze(lab: &Lab) -> Frozen {
    let doc = doc(&lab.bench);
    Frozen {
        old_page: dump(doc, lab.old_page),
        library_page: dump(doc, lab.library_page),
        pages: doc.pages.clone(),
        old_collection: old_collection_dump(doc),
        copy: dump(doc, lab.copy),
    }
}

// covers: E5-5.1
#[test]
fn updating_a_source_submits_one_import_per_imported_component_with_the_saved_options() {
    let mut lab = lab_at_first_version("update-nominal", button_service(), &["Default", "Hover"]);
    import_ok(&mut lab.bench, &staged("Chip", &["Default"]));
    move_to_next_version(&mut lab.bench);
    let first_renders = renders(&lab.bench.submitted());
    let jobs_before = lab.bench.submitted().len();
    let before = lab.bench.document_text(0);

    let started = run(&mut lab.bench, UPDATE, &json!({"package": PACKAGE}));

    assert_eq!(started["jobs"].as_array().map(Vec::len), Some(2), "{started}");
    assert_eq!(strings(&started, "components"), ["Button", "Chip"]);
    assert_eq!(lab.bench.document_text(0), before, "nothing is written before the renders are back");
    let events = pump_all(&mut lab.bench);
    let imports: Vec<&CaptureEvent> = events.iter().filter(|event| event.command == IMPORT).collect();
    assert_eq!(imports.len(), 2, "{events:?}");
    assert!(imports.iter().all(|event| event.outcome.is_ok()), "{events:?}");
    assert_eq!(renders(&lab.bench.submitted()[jobs_before..]), first_renders, "the same variants are rendered again");
}

// covers: E5-5.1
#[test]
fn the_recaptured_sets_fill_the_import_page_of_the_current_version_with_the_options_of_the_first_import() {
    let mut lab = lab_at_first_version("update-page", button_service(), &["Default", "Hover"]);
    import_ok(&mut lab.bench, &staged("Chip", &["Default"]));
    let old_button_options = options_of(doc(&lab.bench), lab.old_set);
    move_to_next_version(&mut lab.bench);

    recapture(&mut lab.bench);

    let doc = doc(&lab.bench);
    let page = page_named(doc, &import_page_name(PACKAGE, NEXT_VERSION));
    assert_eq!(page_named(doc, "- Imported · synthetic-ui 1.1.0"), page);
    assert_eq!(set_names_on(doc, page), ["Button", "Chip"]);
    let button = set_named(doc, page, "Button");
    assert_eq!(children_of(doc, button).len(), 2, "only the two states of the first import, not every variant of the component");
    assert_eq!(options_of(doc, button), old_button_options);
    assert!(doc.variables.collections.iter().any(|collection| collection.name == "synthetic-ui 1.1.0"));
}

// covers: E5-5.1
#[test]
fn updating_a_source_leaves_the_old_page_the_library_copy_and_its_pages_as_they_were() {
    let mut lab = lab_after_release("update-frozen", &["Default", "Hover"]);
    let frozen = freeze(&lab);
    let depth = lab.bench.session.undo_depth();

    recapture(&mut lab.bench);

    let after = freeze(&lab);
    assert_eq!(after.old_page, frozen.old_page);
    assert_eq!(after.library_page, frozen.library_page);
    assert_eq!(after.copy, frozen.copy);
    assert_eq!(after.old_collection, frozen.old_collection);
    assert_eq!(after.pages.len(), frozen.pages.len() + 1, "one page, the new import page");
    assert!(frozen.pages.iter().all(|page| after.pages.contains(page)));
    assert_eq!(lab.bench.session.undo_depth().1, depth.1);
}

fn assert_nominal_differences(listed: &Value) {
    let changed = of_kind(listed, "changed");
    assert!(changed.variant.contains("State=Default"), "{}", changed.variant);
    assert!(changed.path.ends_with("Icon"), "{}", changed.path);
    assert!(changed.field.contains("fill"), "{}", changed.field);
    let added = of_kind(listed, "added");
    assert!(added.variant.contains("State=Hover") && added.path.ends_with("Badge"), "{} {}", added.variant, added.path);
    let removed = of_kind(listed, "removed");
    assert!(removed.variant.contains("State=Default") && removed.path.ends_with("Ripple"), "{} {}", removed.variant, removed.path);
    for difference in [&changed, &added, &removed] {
        assert!(!difference.field.is_empty());
        assert_eq!(difference.id, format!("{}|{}|{}", difference.variant, difference.path, difference.field));
        assert_eq!(difference.state, "clean");
    }
}

// covers: E5-5.2
#[test]
fn the_diff_of_an_intact_copy_lists_the_changed_added_and_removed_layers_all_clean() {
    let mut lab = recaptured("diff-nominal", &["Default", "Hover"]);

    let listed = list(&mut lab.bench, lab.copy);

    assert_eq!((listed["name"].as_str(), listed["from"].as_str(), listed["to"].as_str()), (Some("Button"), Some(VERSION), Some(NEXT_VERSION)));
    assert_eq!(id_in(&listed["base"], "page"), lab.old_page);
    assert_eq!(id_in(&listed["base"], "set"), lab.old_set);
    let doc = doc(&lab.bench);
    let latest_page = page_named(doc, &import_page_name(PACKAGE, NEXT_VERSION));
    assert_eq!(id_in(&listed["latest"], "page"), latest_page);
    assert_eq!(name_of(doc, id_in(&listed["latest"], "set")), "Button");
    assert_eq!(found(&listed).len(), 3, "{listed}");
    assert_nominal_differences(&listed);
    assert_eq!(listed["summary"], json!({"clean": 3, "modified": 0, "unknown": 0}));
}

// covers: E5-5.2
#[test]
fn the_diff_is_pure_the_same_at_each_call_and_the_same_keys_for_another_render_order() {
    let mut lab = recaptured("diff-pure", &["Default", "Hover"]);
    let (before, depth, jobs) = (lab.bench.document_text(0), lab.bench.session.undo_depth(), lab.bench.submitted().len());

    let first = list(&mut lab.bench, lab.copy);
    let second = list(&mut lab.bench, lab.copy);

    assert_eq!(first, second);
    assert_eq!(lab.bench.document_text(0), before);
    assert_eq!(lab.bench.session.undo_depth(), depth);
    assert_eq!(lab.bench.submitted().len(), jobs, "the diff submits no capture job");
    let mut reversed = recaptured("diff-reversed", &["Hover", "Default"]);
    let other = list(&mut reversed.bench, reversed.copy);
    assert_eq!(sorted_ids(&other), sorted_ids(&first));
}

// covers: E5-5.3
#[test]
fn a_difference_on_the_fill_the_user_changed_is_modified_and_the_others_stay_clean() {
    let (mut lab, _icon) = modified_lab("diff-modified");

    let listed = list(&mut lab.bench, lab.copy);

    assert_eq!(of_kind(&listed, "changed").state, "modified");
    assert_eq!(of_kind(&listed, "added").state, "clean");
    assert_eq!(of_kind(&listed, "removed").state, "clean");
    assert_eq!(listed["summary"], json!({"clean": 2, "modified": 1, "unknown": 0}));
}

// covers: E5-5.3
#[test]
fn without_an_import_page_for_the_original_version_every_difference_is_unknown() {
    let mut lab = recaptured("diff-unknown", &["Default", "Hover"]);
    run(&mut lab.bench, "page.delete", &json!({"id": lab.old_page}));

    let listed = list(&mut lab.bench, lab.copy);

    assert!(listed["base"].is_null(), "{listed}");
    let all = found(&listed);
    assert!(!all.is_empty(), "{listed}");
    assert!(all.iter().all(|difference| difference.state == "unknown"), "{listed}");
    assert_eq!(listed["summary"], json!({"clean": 0, "modified": 0, "unknown": all.len()}));
}

// covers: E5-5.4
#[test]
fn applying_without_ids_takes_the_clean_differences_only_in_one_undo_step() {
    let (mut lab, icon) = modified_lab("apply-clean");
    let listed = list(&mut lab.bench, lab.copy);
    let (changed, added, removed) = (of_kind(&listed, "changed"), of_kind(&listed, "added"), of_kind(&listed, "removed"));
    let (before, depth) = (lab.bench.document_text(0), lab.bench.session.undo_depth());
    let edited = fills_of(doc(&lab.bench), icon);

    let result = run(&mut lab.bench, APPLY, &json!({"copy": lab.copy}));

    let mut clean = vec![added.id.clone(), removed.id.clone()];
    clean.sort();
    assert_eq!(strings(&result, "applied"), clean);
    let skipped = result["skipped"].as_array().unwrap_or_else(|| panic!("no skipped in {result}"));
    let reason = skipped.iter().find(|entry| entry["id"] == json!(changed.id)).map(|entry| text_in(entry, "reason"));
    assert!(reason.is_some_and(|reason| !reason.is_empty()), "the modified difference is skipped with its reason: {result}");
    let doc = doc(&lab.bench);
    assert!(layer_named(doc, variant_in(doc, lab.copy, "Default"), "Ripple").is_none(), "the removed layer is gone");
    let parent = layer_named(doc, variant_in(doc, lab.copy, "Hover"), "Button").unwrap_or_else(|| panic!("no Button root in the hover variant"));
    assert!(children_of(doc, parent).iter().any(|id| name_of(doc, *id) == "Badge"), "the added layer sits in the parent of the same path");
    assert_eq!(fills_of(doc, icon), edited, "the modified fill is intact");
    assert_eq!(source_version(doc, lab.copy), VERSION, "a difference remains");
    assert_eq!(lab.bench.session.undo_depth().0, depth.0 + 1);
    run(&mut lab.bench, "edit.undo", &json!({}));
    assert_eq!(lab.bench.document_text(0), before, "one undo undoes everything");
}

// covers: E5-5.5
#[test]
fn taking_the_new_value_overwrites_the_users_and_the_version_moves_when_nothing_is_left() {
    let (mut lab, icon) = modified_lab("apply-ids");
    run(&mut lab.bench, APPLY, &json!({"copy": lab.copy}));
    let listed = list(&mut lab.bench, lab.copy);
    let changed = of_kind(&listed, "changed");
    let user_fill = fills_of(doc(&lab.bench), icon);

    let result = run(&mut lab.bench, APPLY, &json!({"copy": lab.copy, "ids": [changed.id]}));

    assert_eq!(strings(&result, "applied"), std::slice::from_ref(&changed.id));
    assert_eq!(result["skipped"].as_array().map(Vec::len), Some(0), "{result}");
    let doc = doc(&lab.bench);
    let new_icon = icon_of(doc, id_in(&listed["latest"], "set"));
    assert_eq!(fills_of(doc, icon), fills_of(doc, new_icon), "the new value is in the copy");
    assert_ne!(fills_of(doc, icon), user_fill);
    assert_eq!(source_version(doc, lab.copy), NEXT_VERSION);
    assert_eq!(result["version"], json!(NEXT_VERSION));
    let after = list(&mut lab.bench, lab.copy);
    assert_eq!((after["from"].as_str(), after["to"].as_str()), (Some(NEXT_VERSION), Some(NEXT_VERSION)));
    assert_eq!(found(&after).len(), 0, "{after}");
}

// covers: E5-5.5
#[test]
fn a_difference_whose_layer_has_gone_from_the_copy_is_ignored_with_its_reason() {
    let mut lab = recaptured("apply-gone", &["Default", "Hover"]);
    let changed = of_kind(&list(&mut lab.bench, lab.copy), "changed");
    let icon = icon_of(doc(&lab.bench), lab.copy);
    run(&mut lab.bench, "edit.delete", &json!({"ids": [icon]}));

    let result = run(&mut lab.bench, APPLY, &json!({"copy": lab.copy, "ids": [changed.id]}));

    assert_eq!(result["applied"].as_array().map(Vec::len), Some(0), "{result}");
    let skipped = result["skipped"].as_array().unwrap_or_else(|| panic!("no skipped in {result}"));
    let reason = skipped.iter().find(|entry| entry["id"] == json!(changed.id)).map(|entry| text_in(entry, "reason"));
    assert!(reason.is_some_and(|reason| !reason.is_empty()), "{result}");
}

// covers: E5-5.6
#[test]
fn a_failed_render_names_the_component_and_writes_nothing_to_the_library() {
    let service = Evolving { snapshot: evolving_button, failing_at: Some(NEXT_VERSION) };
    let mut lab = lab_at_first_version("update-fails", service, &["Default", "Hover"]);
    move_to_next_version(&mut lab.bench);
    let (before, depth) = (lab.bench.document_text(0), lab.bench.session.undo_depth());

    run(&mut lab.bench, UPDATE, &json!({"package": PACKAGE}));
    let events = pump_all(&mut lab.bench);

    let failure = events.iter().find_map(|event| event.outcome.as_ref().err()).unwrap_or_else(|| panic!("no failure in {events:?}"));
    assert!(failure.message.contains("Button"), "{}", failure.message);
    assert!(failure.message.contains("page crashed: boom"), "{}", failure.message);
    assert_eq!(lab.bench.document_text(0), before, "no page, no layer, no collection");
    assert_eq!(lab.bench.session.undo_depth(), depth);

    let listed = list(&mut lab.bench, lab.copy);
    assert!(listed["latest"].is_null(), "{listed}");
    assert_eq!(found(&listed).len(), 0, "{listed}");
    assert!(text_in(&listed, "note").to_lowercase().contains("recapture first"), "{listed}");
    assert_eq!(lab.bench.document_text(0), before);
}

fn assert_refused(bench: &mut Bench, command: &str, what: &str, params: &Value, fragments: &[&str]) {
    let tab = bench.session.active;
    let (before, depth, jobs) = (bench.document_text(tab), bench.session.undo_depth(), bench.submitted().len());

    let result = bench.session.execute(command, params);

    let Err(error) = result else { panic!("{what}: accepted") };
    assert!(matches!(error, EngineError::BadParams { .. }), "{what}: {error:?}");
    let message = error.to_string();
    assert!(fragments.is_empty() || fragments.iter().any(|fragment| message.contains(fragment)), "{what}: {message}");
    assert_eq!(bench.submitted().len(), jobs, "{what}: a job was submitted");
    assert_eq!(bench.document_text(tab), before, "{what}: the document changed");
    assert_eq!(bench.session.undo_depth(), depth, "{what}: an undo step appeared");
}

// covers: E5-5.7
#[test]
fn an_update_with_a_bad_package_or_component_list_is_refused_and_submits_nothing() {
    let mut lab = lab_after_release("update-refusals", &["Default"]);
    let cases = [
        ("no package", json!({}), vec!["package"]),
        ("package that is not text", json!({"package": 5}), vec!["package"]),
        ("package that is not a source", json!({"package": "other-ui"}), vec!["other-ui"]),
        ("invalid package", json!({"package": "../evil"}), vec!["package", "../evil"]),
        ("unknown component", json!({"package": PACKAGE, "components": ["Nope"]}), vec!["Nope"]),
        ("components that is not a list", json!({"package": PACKAGE, "components": "Button"}), vec!["components"]),
        ("component that is not text", json!({"package": PACKAGE, "components": [5]}), vec!["components"]),
    ];
    for (what, params, fragments) in &cases {
        assert_refused(&mut lab.bench, UPDATE, what, params, fragments);
    }
}

// covers: E5-5.7
#[test]
fn an_update_of_a_removed_source_is_refused_even_when_its_import_page_is_still_there() {
    let mut lab = lab_at_first_version("update-removed", button_service(), &["Default"]);
    run(&mut lab.bench, "project.removeSource", &json!({"package": PACKAGE}));
    pump_all(&mut lab.bench);

    assert_refused(&mut lab.bench, UPDATE, "removed source", &json!({"package": PACKAGE}), &["remov", PACKAGE]);
}

// covers: E5-5.7
#[test]
fn an_update_with_nothing_imported_is_refused() {
    let mut bench = Bench::evolving("update-empty", button_service());

    assert_refused(&mut bench, UPDATE, "nothing imported", &json!({"package": PACKAGE}), &["import"]);
}

// covers: E5-5.7
#[test]
fn an_update_without_a_capture_service_or_outside_a_project_is_refused() {
    let lab = lab_after_release("update-silent", &["Default"]);
    let mut silent = lab.bench.reopened_without_backend();
    assert_refused(&mut silent, UPDATE, "no backend", &json!({"package": PACKAGE}), &["capture service not available"]);

    let mut outside = lab_after_release("update-outside", &["Default"]).bench;
    outside.session.execute("file.new", &json!({})).unwrap();
    assert_refused(&mut outside, UPDATE, "outside a project", &json!({"package": PACKAGE}), &["project"]);
}

// covers: E5-5.7
#[test]
fn a_diff_or_an_apply_on_a_copy_that_is_not_readable_is_refused_and_changes_nothing() {
    let mut lab = recaptured("copy-refusals", &["Default", "Hover"]);
    let frame = id_in(&run(&mut lab.bench, "frame.create", &json!({"x": 0, "y": 0, "width": 100, "height": 100})), "id");
    let cases = [
        ("no copy", json!({})),
        ("copy as text", json!({"copy": "nope"})),
        ("copy that does not exist", json!({"copy": 999_999})),
        ("copy without a source", json!({"copy": frame})),
    ];
    for command in [DIFF, APPLY] {
        for (what, params) in &cases {
            assert_refused(&mut lab.bench, command, what, params, &[]);
        }
    }
    for broken in ["{not json", "{\"package\": \"synthetic-ui\"}", "[]"] {
        run(&mut lab.bench, "node.set", &json!({"ids": [lab.copy], "props": {"pluginData": {SOURCE: broken}}}));
        for command in [DIFF, APPLY] {
            assert_refused(&mut lab.bench, command, broken, &json!({"copy": lab.copy}), &[]);
        }
    }
}

// covers: E5-5.7
#[test]
fn a_diff_or_an_apply_outside_a_project_is_refused() {
    let mut lab = recaptured("copy-outside", &["Default", "Hover"]);
    lab.bench.session.execute("file.new", &json!({})).unwrap();

    for command in [DIFF, APPLY] {
        assert_refused(&mut lab.bench, command, "outside a project", &json!({"copy": lab.copy}), &["project"]);
    }
}

// covers: E5-5.7
#[test]
fn ids_that_are_unknown_or_not_text_refuse_the_whole_apply() {
    let mut lab = recaptured("apply-refusals", &["Default", "Hover"]);
    let known = of_kind(&list(&mut lab.bench, lab.copy), "added").id;
    let copy = lab.copy;
    let cases = [
        ("unknown id", json!({"copy": copy, "ids": ["nope|nope|nope"]}), vec!["nope|nope|nope"]),
        ("known and unknown ids", json!({"copy": copy, "ids": [known, "nope|nope|nope"]}), vec!["nope|nope|nope"]),
        ("ids as text", json!({"copy": copy, "ids": known}), vec!["ids"]),
        ("id that is a number", json!({"copy": copy, "ids": [5]}), vec!["ids"]),
        ("id that is a list", json!({"copy": copy, "ids": [[known]]}), vec!["ids"]),
        ("id that is null", json!({"copy": copy, "ids": [null]}), vec!["ids"]),
    ];
    for (what, params, fragments) in &cases {
        assert_refused(&mut lab.bench, APPLY, what, params, fragments);
    }
}

// covers: E5-5.7
#[test]
fn more_than_two_thousand_differences_are_cut_and_the_note_says_so() {
    let service = Evolving { snapshot: crowded_button, failing_at: None };
    let mut lab = lab_at_first_version("diff-crowded", service, &["Default"]);
    move_to_next_version(&mut lab.bench);
    recapture(&mut lab.bench);

    let listed = list(&mut lab.bench, lab.copy);

    assert_eq!(found(&listed).len(), MAX_DIFFERENCES, "the list stops at {MAX_DIFFERENCES}");
    assert!(!text_in(&listed, "note").is_empty(), "{}", listed["note"]);
}

// covers: E5-5.7
#[test]
fn absurd_parameters_never_panic_never_submit_work_and_never_touch_the_document() {
    let mut lab = recaptured("absurd", &["Default", "Hover"]);
    let (copy, long) = (lab.copy, "p".repeat(300));
    let update = [
        json!(null),
        json!([1, 2]),
        json!("x"),
        json!({}),
        json!({"package": ["a"]}),
        json!({"package": {"x": 1}}),
        json!({"package": "\u{0}"}),
        json!({"package": long}),
        json!({"package": PACKAGE, "components": {"x": 1}}),
        json!({"package": PACKAGE, "components": [["Button"]]}),
        json!({"package": PACKAGE, "components": [1e308]}),
        json!({"package": PACKAGE, "components": [""]}),
        json!({"package": PACKAGE, "components": ["\u{0}"]}),
        json!({"package": PACKAGE, "components": [long]}),
    ];
    for params in &update {
        assert_refused(&mut lab.bench, UPDATE, &params.to_string(), params, &[]);
    }
    let copies = [
        json!(null),
        json!([1]),
        json!("x"),
        json!({}),
        json!({"copy": null}),
        json!({"copy": "5"}),
        json!({"copy": [copy]}),
        json!({"copy": 1e308}),
        json!({"copy": -1}),
        json!({"copy": 999_999_999}),
        json!({"copy": {"id": copy}}),
        json!({"copy": true}),
    ];
    for command in [DIFF, APPLY] {
        for params in &copies {
            assert_refused(&mut lab.bench, command, &params.to_string(), params, &[]);
        }
    }
    let ids = [
        json!({"copy": copy, "ids": {"a": 1}}),
        json!({"copy": copy, "ids": [1e308]}),
        json!({"copy": copy, "ids": [""]}),
        json!({"copy": copy, "ids": [long]}),
    ];
    for params in &ids {
        assert_refused(&mut lab.bench, APPLY, &params.to_string(), params, &[]);
    }
}
