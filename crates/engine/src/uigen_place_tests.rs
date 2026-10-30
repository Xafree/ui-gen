//! `uigen.imported` (the sets of the import pages, one section per source) and `uigen.placeImported`
//! (an independent copy of an imported set on a component page of the library). The service is a
//! scripted backend; the import pages are filled by a real staged import with synthetic snapshots.

use figcraft_doc::proto::Action;
use figcraft_doc::{Document, NodeId, NodeKind};
use serde_json::{Value, json};

use crate::capture::{CaptureBackend, CaptureJob, CaptureOutcome, JobId};
use crate::params::id_of;
use crate::uigen_import::{import_page_name, is_import_page, sections};
use crate::uigen_import_support::{Bench, IMPORT, PACKAGE, VERSION, synthetic_button, synthetic_source};
use crate::{EngineError, Result, Session};

const LIST: &str = "uigen.imported";
const PLACE: &str = "uigen.placeImported";
const EFFECT: &str = "uigen.effect";
const SOURCE: &str = "uigen.source";
const ROLE: &str = "uigen.role";
const NEXT_VERSION: &str = "1.1.0";
const MAX_PUMPS: usize = 64;
const FILLER: usize = 3000;

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
        "style": {"backgroundColor": "rgb(10, 20, 30)"}, "animations": animations
    });
    if let (Some(class_name), Some(object)) = (class_name, node.as_object_mut()) {
        object.insert("className".to_owned(), json!(class_name));
    }
    node
}

fn snapshot(style: &str, size: &str, state: &str, theme: &str, filler: usize) -> Value {
    let mut snapshot = synthetic_button(style, size, state, theme);
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
        for index in 0..filler {
            children.push(element(
                &format!("Filler {index}"),
                None,
                json!([]),
                [20.0 + f64::from(u8::try_from(index % 50).unwrap_or(0)), 20.0, 4.0, 4.0],
            ));
        }
    }
    snapshot
}

struct Backend {
    version: String,
    filler: usize,
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
                Ok(json!({"snapshot": snapshot(style, size, state, theme, self.filler), "png": ".uigen/captures/synthetic.png"}))
            }
            CaptureJob::Install { package, version, .. } => {
                self.version = version.clone();
                Ok(json!({"package": package, "version": version, "framework": "react", "license": "MIT"}))
            }
            CaptureJob::Uninstall { .. } => Ok(json!({})),
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

fn bench(tag: &str, filler: usize) -> Bench {
    let mut bench = Bench::without_backend(tag, &[synthetic_source(false)]);
    bench.session.set_capture_backend(Box::new(Backend { version: VERSION.to_owned(), filler, submitted: 0, ready: Vec::new() }));
    bench
}

fn staged(component: &str, states: &[&str]) -> Value {
    json!({"package": PACKAGE, "component": component, "states": states, "themes": ["light"], "stage": true})
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

fn pump_until(bench: &mut Bench, command: &str) {
    for _ in 0..MAX_PUMPS {
        if bench.session.pump_capture().iter().any(|event| event.command == command) {
            return;
        }
    }
    panic!("no {command} event");
}

struct Lab {
    bench: Bench,
    start: NodeId,
    import_page: NodeId,
    set: NodeId,
}

fn lab(tag: &str) -> Lab {
    let mut bench = bench(tag, 0);
    let start = bench.session.docs[0].page;
    let result = import_ok(&mut bench, &staged("Button", &["Default", "Hover"]));
    let import_page = id_in(&result, "page");
    let set = id_in(&result, "set");
    Lab { bench, start, import_page, set }
}

fn id_in(result: &Value, key: &str) -> NodeId {
    id_of(&result[key]).unwrap_or_else(|| panic!("no {key} in {result}"))
}

fn document_of(bench: &Bench) -> &Document {
    bench.session.docs[0].doc.as_ref()
}

fn run(bench: &mut Bench, command: &str, params: &Value) -> Value {
    bench.session.execute(command, params).unwrap_or_else(|error| panic!("{command}: {error}"))
}

fn place(bench: &mut Bench, params: &Value) -> Result<Value> {
    bench.session.execute(PLACE, params)
}

fn place_ok(bench: &mut Bench, params: &Value) -> Value {
    place(bench, params).unwrap_or_else(|error| panic!("{PLACE}: {error}"))
}

fn imported(bench: &mut Bench) -> Value {
    run(bench, LIST, &json!({}))
}

fn undo(bench: &mut Bench) {
    run(bench, "edit.undo", &json!({}));
}

fn name_of(doc: &Document, id: NodeId) -> String {
    doc.node(id).unwrap().name.clone()
}

fn children_of(doc: &Document, id: NodeId) -> Vec<NodeId> {
    doc.node(id).unwrap().children.clone()
}

fn names_of(doc: &Document, ids: &[NodeId]) -> Vec<String> {
    ids.iter().map(|id| name_of(doc, *id)).collect()
}

fn subtree(doc: &Document, id: NodeId) -> Vec<NodeId> {
    let mut out = vec![id];
    for child in &doc.node(id).unwrap().children {
        out.extend(subtree(doc, *child));
    }
    out
}

fn dump(doc: &Document, root: NodeId) -> String {
    let nodes: Vec<String> = subtree(doc, root).into_iter().map(|id| format!("{:?}", doc.node(id).unwrap())).collect();
    nodes.join("\n")
}

fn sets_on(doc: &Document, page: NodeId) -> Vec<NodeId> {
    children_of(doc, page).into_iter().filter(|id| matches!(doc.node(*id).unwrap().kind, NodeKind::ComponentSet { .. })).collect()
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

fn change_targets(doc: &Document, set: NodeId) -> Vec<NodeId> {
    children_of(doc, set)
        .into_iter()
        .flat_map(|variant| doc.node(variant).unwrap().interactions.clone())
        .flat_map(|interaction| interaction.actions)
        .filter_map(|action| match action {
            Action::ChangeTo { dest } => Some(dest),
            _ => None,
        })
        .collect()
}

fn components(bench: &mut Bench) -> Vec<Value> {
    run(bench, "project.info", &json!({}))["components"].as_array().cloned().unwrap_or_default()
}

fn rename(bench: &mut Bench, page: NodeId, name: &str) {
    run(bench, "page.rename", &json!({"id": page, "name": name}));
}

fn add_page(bench: &mut Bench, name: &str) -> NodeId {
    id_in(&run(bench, "page.add", &json!({"name": name})), "id")
}

fn go_to(bench: &mut Bench, page: NodeId) {
    run(bench, "page.select", &json!({"id": page}));
}

fn add_layer(bench: &mut Bench, name: &str, role: Option<&str>) -> NodeId {
    let layer = id_in(&run(bench, "frame.create", &json!({"x": 40, "y": 60, "width": 100, "height": 60, "name": name})), "id");
    if let Some(role) = role {
        run(bench, "node.set", &json!({"ids": [layer], "props": {"pluginData": {ROLE: role}}}));
    }
    layer
}

struct Section {
    package: String,
    version: String,
    page: Option<NodeId>,
    removed: bool,
    sets: Vec<(NodeId, String, u64)>,
}

fn sections_of(listed: &Value) -> Vec<Section> {
    listed["sections"]
        .as_array()
        .unwrap_or_else(|| panic!("no sections in {listed}"))
        .iter()
        .map(|section| Section {
            package: section["package"].as_str().unwrap_or_default().to_owned(),
            version: section["version"].as_str().unwrap_or_default().to_owned(),
            page: if section["page"].is_null() { None } else { Some(id_in(section, "page")) },
            removed: section["removed"].as_bool().unwrap_or_else(|| panic!("no removed in {section}")),
            sets: section["components"]
                .as_array()
                .unwrap_or_else(|| panic!("no components in {section}"))
                .iter()
                .map(|set| (id_in(set, "set"), set["name"].as_str().unwrap_or_default().to_owned(), set["variants"].as_u64().unwrap_or(u64::MAX)))
                .collect(),
        })
        .collect()
}

// covers: E4-3.1
#[test]
fn a_source_with_nothing_imported_is_an_empty_section_without_a_page() {
    let mut bench = Bench::without_backend("list-empty", &[synthetic_source(false)]);

    let found = sections_of(&imported(&mut bench));

    assert_eq!(found.len(), 1);
    assert_eq!((found[0].package.as_str(), found[0].version.as_str()), (PACKAGE, VERSION));
    assert_eq!(found[0].page, None);
    assert!(!found[0].removed);
    assert!(found[0].sets.is_empty());
}

// covers: E4-3.1
#[test]
fn the_sets_of_the_import_page_are_listed_in_page_order_and_the_copies_in_the_library_are_not() {
    let mut lab = lab("list-nominal");
    let chip = id_in(&import_ok(&mut lab.bench, &staged("Chip", &["Default"])), "set");

    let listed = imported(&mut lab.bench);

    let found = sections_of(&listed);
    assert_eq!(found.len(), 1, "{listed}");
    assert_eq!((found[0].package.as_str(), found[0].version.as_str(), found[0].removed), (PACKAGE, VERSION, false));
    assert_eq!(found[0].page, Some(lab.import_page));
    let doc = document_of(&lab.bench);
    let expected: Vec<(NodeId, String, u64)> =
        sets_on(doc, lab.import_page).into_iter().map(|set| (set, name_of(doc, set), children_of(doc, set).len() as u64)).collect();
    assert_eq!(found[0].sets, expected);
    assert_eq!(found[0].sets.len(), 2);
    assert!(found[0].sets.contains(&(lab.set, "Button".to_owned(), 8)), "{:?}", found[0].sets);
    assert!(found[0].sets.contains(&(chip, "Chip".to_owned(), 4)), "{:?}", found[0].sets);

    place_ok(&mut lab.bench, &json!({"set": lab.set}));
    assert_eq!(imported(&mut lab.bench), listed, "the copy in the library is no imported set");
}

// covers: E4-3.1
#[test]
fn the_pure_sections_function_gives_what_the_command_lists() {
    let lab = lab("list-pure");
    let binding = lab.bench.session.project().unwrap();

    let found = sections(document_of(&lab.bench), &binding.config);

    assert_eq!(found.len(), 1);
    assert_eq!((found[0].package.as_str(), found[0].version.as_str(), found[0].removed), (PACKAGE, VERSION, false));
    assert_eq!(found[0].page, Some(lab.import_page));
    assert_eq!(found[0].components.len(), 1);
    assert_eq!((found[0].components[0].set, found[0].components[0].name.as_str(), found[0].components[0].variants), (lab.set, "Button", 8));
    assert!(is_import_page(document_of(&lab.bench), lab.import_page));
    assert_eq!(name_of(document_of(&lab.bench), lab.import_page), import_page_name(PACKAGE, VERSION));
}

// covers: E4-3.1
#[test]
fn a_new_version_gives_an_empty_current_section_then_the_old_page_marked_removed() {
    let mut lab = lab("list-version");
    run(&mut lab.bench, "project.setSourceVersion", &json!({"package": PACKAGE, "version": NEXT_VERSION}));
    pump_until(&mut lab.bench, "project.setSourceVersion");

    let found = sections_of(&imported(&mut lab.bench));

    assert_eq!(found.len(), 2);
    assert_eq!((found[0].version.as_str(), found[0].page, found[0].removed, found[0].sets.len()), (NEXT_VERSION, None, false, 0));
    assert_eq!(
        (found[1].package.as_str(), found[1].version.as_str(), found[1].page, found[1].removed),
        (PACKAGE, VERSION, Some(lab.import_page), true)
    );
    assert_eq!(found[1].sets, vec![(lab.set, "Button".to_owned(), 8)]);
}

// covers: E4-3.1
#[test]
fn a_removed_source_keeps_only_its_import_page_marked_removed() {
    let mut lab = lab("list-removed");
    run(&mut lab.bench, "project.removeSource", &json!({"package": PACKAGE}));
    pump_until(&mut lab.bench, "project.removeSource");

    let found = sections_of(&imported(&mut lab.bench));

    assert_eq!(found.len(), 1, "a removed source without a page has no section");
    assert_eq!(
        (found[0].package.as_str(), found[0].version.as_str(), found[0].page, found[0].removed),
        (PACKAGE, VERSION, Some(lab.import_page), true)
    );
    assert_eq!(found[0].sets, vec![(lab.set, "Button".to_owned(), 8)]);
}

// covers: E4-3.1
#[test]
fn outside_a_project_the_list_is_refused() {
    let mut session = Session::empty();

    let error = session.execute(LIST, &json!({})).unwrap_err();

    assert!(matches!(error, EngineError::BadParams { .. }), "{error:?}");
}

fn assert_same_copy(doc: &Document, set: NodeId, copy: NodeId) {
    let source_ids = subtree(doc, set);
    let source_variants = names_of(doc, &children_of(doc, set));
    let copy_ids = subtree(doc, copy);
    assert_eq!(copy_ids.len(), source_ids.len());
    assert!(copy_ids.iter().all(|id| !source_ids.contains(id)), "no id of the copy is an id of the source");
    assert_eq!(names_of(doc, &children_of(doc, copy)), source_variants);
    assert_eq!(names_of(doc, &copy_ids), names_of(doc, &source_ids));
    for (original, duplicate) in source_ids.iter().zip(&copy_ids) {
        assert_eq!(doc.node(*original).unwrap().bound, doc.node(*duplicate).unwrap().bound, "bindings of {}", name_of(doc, *original));
    }
    assert_eq!(doc.node(copy).unwrap().plugin_data.get(SOURCE), doc.node(set).unwrap().plugin_data.get(SOURCE));
    assert!(doc.node(copy).unwrap().plugin_data.contains_key(SOURCE));
    assert_eq!(effects(doc, copy), effects(doc, set));

    let copy_variants = children_of(doc, copy);
    let targets = change_targets(doc, copy);
    assert_eq!(targets.len(), change_targets(doc, set).len());
    assert!(targets.iter().all(|dest| copy_variants.contains(dest)), "every ChangeTo of the copy aims at a variant of the copy: {targets:?}");
}

// covers: E4-3.2
#[test]
fn on_a_blank_page_the_copy_has_new_ids_the_same_variants_and_links_and_the_page_takes_the_name_of_the_set() {
    let mut lab = lab("place-blank");
    rename(&mut lab.bench, lab.start, "Component 1");
    let before = lab.bench.document_text(0);
    let depth = lab.bench.session.undo_depth();
    let doc = document_of(&lab.bench);
    let source_dump = dump(doc, lab.import_page);
    assert!(!change_targets(doc, lab.set).is_empty(), "the source must carry transitions");
    assert!(!effects(doc, lab.set).is_empty(), "the source must carry effects");
    assert!(subtree(doc, lab.set).iter().any(|id| !doc.node(*id).unwrap().bound.is_empty()), "the source must carry variable bindings");

    let result = place_ok(&mut lab.bench, &json!({"set": lab.set}));

    let doc = document_of(&lab.bench);
    let copy = id_in(&result, "set");
    assert_eq!(id_in(&result, "page"), lab.start);
    assert_eq!(result["name"], json!("Button"));
    assert!(result["replaced"].is_null(), "{result}");
    assert_eq!(children_of(doc, lab.start), vec![copy]);
    assert_eq!(name_of(doc, lab.start), "Button");
    assert!(matches!(doc.node(copy).unwrap().kind, NodeKind::ComponentSet { .. }));

    assert_same_copy(doc, lab.set, copy);

    assert_eq!(lab.bench.session.selection(), vec![copy]);
    assert_eq!(dump(doc, lab.import_page), source_dump, "the import page is untouched");

    assert_eq!(lab.bench.session.undo_depth().0, depth.0 + 1);
    undo(&mut lab.bench);
    assert_eq!(lab.bench.document_text(0), before, "one undo undoes the copy and the name");
    assert_eq!(lab.bench.session.undo_depth().0, depth.0);
}

// covers: E4-3.2
#[test]
fn a_copy_aimed_at_another_page_makes_that_page_the_current_one() {
    let mut lab = lab("place-other-page");
    let other = add_page(&mut lab.bench, "Spare");
    go_to(&mut lab.bench, lab.start);

    let result = place_ok(&mut lab.bench, &json!({"set": lab.set, "page": other}));

    let doc = document_of(&lab.bench);
    let copy = id_in(&result, "set");
    assert_eq!(id_in(&result, "page"), other);
    assert_eq!(children_of(doc, other), vec![copy]);
    assert_eq!(name_of(doc, other), "Button");
    assert!(children_of(doc, lab.start).is_empty(), "the former page is untouched");
    assert_eq!(lab.bench.session.docs[0].page, other);
}

// covers: E4-3.3
#[test]
fn editing_a_fill_of_the_copy_leaves_the_source_alone_and_deleting_the_copy_leaves_the_variables() {
    let mut lab = lab("place-independent");
    let copy = id_in(&place_ok(&mut lab.bench, &json!({"set": lab.set})), "set");
    let doc = document_of(&lab.bench);
    let source_dump = dump(doc, lab.import_page);
    let variables = format!("{:?}", doc.variables);
    let layer = subtree(doc, copy)
        .into_iter()
        .find(|id| {
            let node = doc.node(*id).unwrap();
            !node.fills.is_empty() && !node.bound.is_empty()
        })
        .unwrap_or_else(|| panic!("no bound fill in the copy"));
    let fills = format!("{:?}", doc.node(layer).unwrap().fills);

    run(&mut lab.bench, "fill.set", &json!({"ids": [layer], "index": 0, "color": "#FF00FF"}));

    let doc = document_of(&lab.bench);
    assert_ne!(format!("{:?}", doc.node(layer).unwrap().fills), fills, "the fill of the copy changed");
    assert_eq!(dump(doc, lab.import_page), source_dump, "no layer of the source changed");
    assert_eq!(format!("{:?}", doc.variables), variables, "no variable changed");

    run(&mut lab.bench, "edit.delete", &json!({"ids": [copy]}));

    let doc = document_of(&lab.bench);
    assert!(doc.node(copy).is_none());
    assert_eq!(dump(doc, lab.import_page), source_dump, "the source survives the copy");
    assert_eq!(format!("{:?}", doc.variables), variables, "the variable collection survives the copy");
    assert!(doc.variables.collections.iter().any(|collection| collection.name == "synthetic-ui 1.0.0"));
}

fn occupy(bench: &mut Bench, page: NodeId, name: &str) -> Vec<NodeId> {
    rename(bench, page, name);
    vec![add_layer(bench, "Hero card", None), add_layer(bench, "Spec note", Some("note")), add_layer(bench, "Reference shot", Some("reference"))]
}

// covers: E4-3.4
#[test]
fn an_occupied_page_hands_all_its_layers_to_a_new_page_right_after_and_keeps_only_the_copy() {
    let mut lab = lab("place-occupied");
    occupy(&mut lab.bench, lab.start, "Hero");
    let before = lab.bench.document_text(0);
    let depth = lab.bench.session.undo_depth();
    let doc = document_of(&lab.bench);
    let pages = doc.pages.clone();
    let at = pages.iter().position(|page| *page == lab.start).unwrap();
    let old_layers = names_of(doc, &children_of(doc, lab.start));
    assert_eq!(old_layers.len(), 3);
    let library = components(&mut lab.bench).len();

    let result = place_ok(&mut lab.bench, &json!({"set": lab.set}));

    let doc = document_of(&lab.bench);
    let copy = id_in(&result, "set");
    let archive = id_in(&result["replaced"], "page");
    assert_eq!(result["replaced"]["name"], json!("Hero"));
    assert_ne!(archive, lab.start);
    assert_eq!(id_in(&result, "page"), lab.start);
    assert_eq!(doc.pages.len(), pages.len() + 1);
    assert_eq!(doc.pages[at], lab.start);
    assert_eq!(doc.pages[at + 1], archive, "the archive page comes right after");
    assert_eq!(name_of(doc, archive), "Hero");
    let mut kept = names_of(doc, &children_of(doc, archive));
    kept.sort();
    let mut expected = old_layers;
    expected.sort();
    assert_eq!(kept, expected, "layers, notes and reference all moved");
    let note = children_of(doc, archive).into_iter().find(|id| name_of(doc, *id) == "Spec note").unwrap();
    assert_eq!(doc.node(note).unwrap().plugin_data.get(ROLE).map(String::as_str), Some("note"));

    assert_eq!(children_of(doc, lab.start), vec![copy], "the target page holds only the copy");
    assert_eq!(name_of(doc, lab.start), "Button");
    assert_eq!(components(&mut lab.bench).len(), library + 1);

    assert_eq!(lab.bench.session.undo_depth().0, depth.0 + 1);
    undo(&mut lab.bench);
    assert_eq!(lab.bench.document_text(0), before, "the old content, the old name and no archive page");
    assert_eq!(lab.bench.session.undo_depth().0, depth.0);
}

// covers: E4-3.4
#[test]
fn when_the_old_name_is_the_new_one_apart_from_the_case_the_archive_page_is_numbered() {
    let mut lab = lab("place-same-name");
    occupy(&mut lab.bench, lab.start, "button");

    let result = place_ok(&mut lab.bench, &json!({"set": lab.set}));

    let doc = document_of(&lab.bench);
    let archive = id_in(&result["replaced"], "page");
    assert_eq!(name_of(doc, archive), "Button 2");
    assert_eq!(name_of(doc, lab.start), "Button");
    assert_eq!(result["replaced"]["name"], json!("button"));
}

// covers: E4-3.5
#[test]
fn a_blank_page_already_named_like_the_set_keeps_its_name() {
    let mut lab = lab("name-own");
    assert_eq!(name_of(document_of(&lab.bench), lab.start), "Button");

    place_ok(&mut lab.bench, &json!({"set": lab.set}));

    assert_eq!(name_of(document_of(&lab.bench), lab.start), "Button", "its own old name is not a taken name");
}

// covers: E4-3.5
#[test]
fn a_set_named_like_another_component_gives_the_page_the_next_free_number() {
    let mut lab = lab("name-taken");
    rename(&mut lab.bench, lab.start, "Spare");
    let first = add_page(&mut lab.bench, "Button");
    go_to(&mut lab.bench, lab.start);

    place_ok(&mut lab.bench, &json!({"set": lab.set}));

    assert_eq!(name_of(document_of(&lab.bench), lab.start), "Button 2");
    assert_eq!(name_of(document_of(&lab.bench), first), "Button");

    let third = add_page(&mut lab.bench, "Third");
    place_ok(&mut lab.bench, &json!({"set": lab.set, "page": third}));
    assert_eq!(name_of(document_of(&lab.bench), third), "Button 3");
}

fn assert_refused(bench: &mut Bench, what: &str, params: &Value) {
    let before = bench.document_text(0);
    let depth = bench.session.undo_depth();

    let result = place(bench, params);

    let Err(error) = result else { panic!("{what}: accepted") };
    assert!(matches!(error, EngineError::BadParams { .. }), "{what}: {error:?}");
    assert!(!error.to_string().is_empty(), "{what}");
    assert_eq!(bench.document_text(0), before, "{what}: the document changed");
    assert_eq!(bench.session.undo_depth(), depth, "{what}: an undo step appeared");
}

// covers: E4-3.6
#[test]
fn a_set_or_a_target_that_is_not_allowed_is_refused_and_the_document_stays_identical() {
    let mut lab = lab("place-refusals");
    let frame = add_layer(&mut lab.bench, "Frame", None);
    let variant = children_of(document_of(&lab.bench), lab.set)[0];
    let divider = id_in(&run(&mut lab.bench, "page.add", &json!({"name": "- Divider"})), "id");
    go_to(&mut lab.bench, lab.start);
    let copy = id_in(&place_ok(&mut lab.bench, &json!({"set": lab.set, "page": lab.start})), "set");
    let set = lab.set;
    let cases = vec![
        ("no set", json!({})),
        ("set as text", json!({"set": "nope"})),
        ("set as a list", json!({"set": [set]})),
        ("set that does not exist", json!({"set": 999_999})),
        ("set is a frame", json!({"set": frame})),
        ("set is a variant", json!({"set": variant})),
        ("set is a page", json!({"set": lab.import_page})),
        ("set is a copy in the library", json!({"set": copy})),
        ("page is a frame", json!({"set": set, "page": frame})),
        ("page is a divider", json!({"set": set, "page": divider})),
        ("page is an import page", json!({"set": set, "page": lab.import_page})),
        ("page as text", json!({"set": set, "page": "x"})),
        ("page that does not exist", json!({"set": set, "page": 999_999})),
    ];
    for (what, params) in &cases {
        assert_refused(&mut lab.bench, what, params);
    }
}

// covers: E4-3.6
#[test]
fn outside_a_project_placing_is_refused() {
    let mut session = Session::empty();

    let error = session.execute(PLACE, &json!({"set": 1})).unwrap_err();

    assert!(matches!(error, EngineError::BadParams { .. }), "{error:?}");
}

// covers: E4-3.6
#[test]
fn a_copy_of_more_than_max_nodes_layers_is_refused_and_the_document_stays_identical() {
    let mut bench = bench("place-too-big", FILLER);
    let params = json!({"package": PACKAGE, "component": "Button", "styles": ["solid"], "sizes": ["sm", "md"], "states": ["Default"], "themes": ["light"], "stage": true});
    let set = id_in(&import_ok(&mut bench, &params), "set");
    let weight = subtree(document_of(&bench), set).len();
    assert!(weight > figcraft_dom::MAX_NODES, "the staged set must be larger than MAX_NODES, it has {weight} layers");

    assert_refused(&mut bench, "set larger than MAX_NODES", &json!({"set": set}));
    let message = place(&mut bench, &json!({"set": set})).unwrap_err().to_string();
    let named = ["5000", "layer", "node", "many", "large"];
    assert!(named.iter().any(|fragment| message.contains(fragment)), "{message}");
}
