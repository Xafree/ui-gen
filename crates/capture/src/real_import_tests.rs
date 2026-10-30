//! The component import end to end: the real capture service (Node, Chromium, the npm registry of
//! this machine) behind a real engine session. Every test is `#[ignore]`d because it downloads:
//! run them by hand with `cargo test -p figcraft-capture real_import -- --ignored --test-threads=1`.

use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use figcraft_doc::proto::Trigger;
use figcraft_doc::{Document, NodeId, NodeKind, VariableType};
use figcraft_engine::Session;
use figcraft_engine::capture::{CaptureError, CaptureErrorKind, CaptureEvent, CaptureJob};
use figcraft_engine::uigen_compare::sizes_agree;
use serde_json::{Value, json};

use crate::service_tests::TempDir;
use crate::{CaptureService, locate};

const PACKAGE: &str = "@mui/material";
const VERSION: &str = "7.3.11";
const ADD_SOURCE: &str = "project.addSource";
const IMPORT: &str = "uigen.importComponent";
const EFFECT: &str = "uigen.effect";
const INSPECT: &str = "test.inspect";
const IDENTICAL_DARK: &str = "the dark render is identical to the light one: the library does not follow prefers-color-scheme";
const JOB_LIMIT: Duration = Duration::from_secs(900);

fn project_with_mui(label: &str) -> (TempDir, Session) {
    let dir = TempDir::new(label);
    let config = locate().expect("node and the sidecar script must be found (UIGEN_NODE, UIGEN_SIDECAR_DIR)");
    let mut session = Session::empty();
    session.set_capture_backend(Box::new(CaptureService::start(config, Box::new(|| {}))));
    let opened = session.execute("project.open", &json!({"path": dir.path().display().to_string(), "create": true})).unwrap();
    assert_eq!(opened["opened"], json!(true), "{opened}");

    let started = session.execute(ADD_SOURCE, &json!({"package": PACKAGE, "version": VERSION})).unwrap();
    let event = wait_for(&mut session, ADD_SOURCE, &started);
    if let Err(error) = event.outcome {
        panic!("the install failed: {}", error.message);
    }
    (dir, session)
}

fn wait_for(session: &mut Session, command: &str, started: &Value) -> CaptureEvent {
    let job = started["job"].as_u64().unwrap_or_else(|| panic!("no job id in {started}"));
    let deadline = Instant::now() + JOB_LIMIT;
    loop {
        if let Some(event) = session.pump_capture().into_iter().find(|event| event.command == command) {
            assert_eq!(event.job, job);
            assert_eq!(event.package, PACKAGE);
            return event;
        }
        assert!(Instant::now() < deadline, "no {command} event within {JOB_LIMIT:?}");
        thread::sleep(Duration::from_millis(50));
    }
}

fn inspected_values(session: &mut Session, dir: &TempDir, component: &str, prop: &str) -> Option<Vec<String>> {
    let job = CaptureJob::Inspect { root: dir.path().display().to_string(), package: PACKAGE.to_string() };
    let id = session.submit_capture(INSPECT, PACKAGE, job).expect("the inspect job is submitted");
    let event = wait_for(session, INSPECT, &json!({"job": id}));
    let listing = event.outcome.unwrap_or_else(|error| panic!("inspect failed: {}", error.message));
    let components = listing["components"].as_array().expect("inspect lists components");
    let found = components.iter().find(|candidate| candidate["name"] == json!(component))?;
    let listed = found["props"].as_array()?.iter().find(|candidate| candidate["name"] == json!(prop) && candidate["type"] == json!("enum"))?;
    Some(listed["values"].as_array()?.iter().filter_map(|value| value.as_str().map(str::to_string)).collect())
}

fn import(session: &mut Session, component: &str) -> CaptureEvent {
    let params = json!({
        "package": PACKAGE,
        "component": component,
        "styles": ["contained", "outlined"],
        "sizes": ["small", "medium"],
        "states": ["Default", "Hover", "Pressed", "Disabled"]
    });
    let started = session.execute(IMPORT, &params).unwrap();
    wait_for(session, IMPORT, &started)
}

fn document(session: &Session) -> &Document {
    session.docs[session.active].doc.as_ref()
}

fn document_text(session: &Session) -> String {
    serde_json::to_string(document(session)).unwrap()
}

fn page_children(session: &Session) -> Vec<NodeId> {
    let doc = document(session);
    doc.node(session.docs[session.active].page).unwrap().children.clone()
}

fn subtree(doc: &Document, id: NodeId, out: &mut Vec<NodeId>) {
    out.push(id);
    for child in &doc.node(id).unwrap().children {
        subtree(doc, *child, out);
    }
}

fn assert_readable(error: &CaptureError) {
    assert!(error.message.trim().len() > 10, "the message is not readable: {:?}", error.message);
}

fn height_of(doc: &Document, set: NodeId, variant: &str) -> f64 {
    let found = doc.node(set).unwrap().children.iter().find(|id| doc.node(**id).unwrap().name == variant);
    doc.node(*found.unwrap_or_else(|| panic!("no variant {variant:?}"))).unwrap().height
}

fn assert_variants_are_labelled_and_sized(doc: &Document, set: NodeId) {
    let mut layers = Vec::new();
    for variant in &doc.node(set).unwrap().children {
        subtree(doc, *variant, &mut layers);
    }
    let labels: Vec<&str> = layers
        .iter()
        .filter_map(|id| match &doc.node(*id).unwrap().kind {
            NodeKind::Text(text) => Some(text.characters.as_str()),
            _ => None,
        })
        .collect();
    assert!(labels.iter().any(|label| label.eq_ignore_ascii_case("Button")), "no TEXT layer reads Button (MUI upper-cases it): {labels:?}");
    let small = height_of(doc, set, "Style=contained, Size=small, State=Default");
    let medium = height_of(doc, set, "Style=contained, Size=medium, State=Default");
    eprintln!("MUI Button: TEXT layers {labels:?}, height small = {small}, medium = {medium}");
    assert!(small < medium, "small ({small}) is not shorter than medium ({medium})");
}

// covers: E3-8.1
#[test]
#[ignore = "downloads @mui/material from npm and drives Chromium; run by hand"]
fn real_import_of_the_mui_button_gives_sixteen_variants_a_light_dark_theme_a_ripple_and_a_hover_transition() {
    let (dir, mut session) = project_with_mui("real-import");
    let variants = inspected_values(&mut session, &dir, "Button", "variant").expect("inspect reads `variant` of Button as an enum");
    assert!(variants.iter().any(|value| value == "contained"), "{variants:?}");
    let before = document_text(&session);
    assert!(page_children(&session).is_empty(), "the new project's page is not empty");

    let event = import(&mut session, "Button");
    let result = match event.outcome {
        Ok(result) => result,
        Err(error) => panic!("the import failed: {}", error.message),
    };
    assert_eq!(result["variants"], json!(16), "{result}");
    assert!(result["warnings"].is_array(), "{result}");

    let doc = document(&session);
    let set = figcraft_engine::params::id_of(&result["set"]).unwrap_or_else(|| panic!("no set in {result}"));
    let node = doc.node(set).unwrap();
    assert!(matches!(node.kind, NodeKind::ComponentSet { .. }), "{:?}", node.kind);
    assert_eq!(node.children.len(), 16);
    assert_variants_are_labelled_and_sized(doc, set);

    let collection_id = result["collection"].as_str().unwrap_or_else(|| panic!("no collection in {result}"));
    let collection = doc.variables.collections.iter().find(|collection| collection.id == collection_id).expect("the collection exists");
    assert_eq!(collection.name, format!("{PACKAGE} {VERSION}"));
    let modes: Vec<&str> = collection.modes.iter().map(|mode| mode.name.as_str()).collect();
    assert_eq!(modes, ["Light", "Dark"]);
    let (light, dark) = (&collection.modes[0].id, &collection.modes[1].id);
    let differs = doc
        .variables
        .variables
        .iter()
        .filter(|variable| variable.collection == collection.id)
        .any(|variable| variable.values.get(light) != variable.values.get(dark));
    let warned = result["warnings"].as_array().is_some_and(|warnings| warnings.iter().any(|warning| warning == IDENTICAL_DARK));
    assert!(differs || warned, "Light and Dark hold the same values and nothing says so: {result}");
    eprintln!("MUI dark theme: a variable differs between Light and Dark = {differs}, identical-dark warning = {warned}");

    let mut layers = Vec::new();
    for variant in &node.children {
        subtree(doc, *variant, &mut layers);
    }
    assert!(
        layers.iter().any(|id| doc.node(*id).unwrap().plugin_data.get(EFFECT).map(String::as_str) == Some("ripple")),
        "no layer is flagged {EFFECT} = ripple"
    );
    let hover_durations: Vec<u32> = node
        .children
        .iter()
        .flat_map(|variant| doc.node(*variant).unwrap().interactions.iter())
        .filter(|interaction| interaction.trigger == Trigger::OnHover)
        .map(|interaction| interaction.duration_ms)
        .collect();
    assert!(hover_durations.iter().any(|duration| *duration > 0), "no OnHover interaction has a duration: {hover_durations:?}");

    session.execute("edit.undo", &json!({})).unwrap();
    assert!(page_children(&session).is_empty(), "one undo does not empty the page");
    assert_eq!(document_text(&session), before);
}

// covers: E3-8.2
#[test]
#[ignore = "downloads @mui/material from npm and drives Chromium; run by hand"]
fn real_import_of_an_unknown_component_fails_with_component_not_found_and_leaves_the_document_alone() {
    let (_dir, mut session) = project_with_mui("real-import-nope");
    let before = document_text(&session);
    let depth = session.undo_depth();

    let event = import(&mut session, "Nope");
    let Err(error) = event.outcome else { panic!("the import of an unknown component succeeded") };

    assert_eq!(error.kind, CaptureErrorKind::ComponentNotFound);
    assert_readable(&error);
    assert!(error.message.contains("Nope"), "{}", error.message);
    assert_eq!(document_text(&session), before);
    assert_eq!(session.undo_depth(), depth);
}

fn staged_import(session: &mut Session, component: &str, styles: &[&str], sizes: &[&str], states: &[&str]) -> Value {
    let params = json!({"package": PACKAGE, "component": component, "styles": styles, "sizes": sizes, "states": states, "stage": true});
    let started = session.execute(IMPORT, &params).unwrap();
    match wait_for(session, IMPORT, &started).outcome {
        Ok(result) => result,
        Err(error) => panic!("the staged import of {component} failed: {}", error.message),
    }
}

fn enum_values(component: &Value, prop: &str) -> Vec<String> {
    let props = component["props"].as_array().unwrap_or_else(|| panic!("no props in {component}"));
    let found = props.iter().find(|candidate| candidate["name"] == json!(prop)).unwrap_or_else(|| panic!("no `{prop}` prop in {component}"));
    assert_eq!(found["type"], json!("enum"), "{found}");
    found["values"].as_array().unwrap().iter().filter_map(|value| value.as_str().map(str::to_string)).collect()
}

fn page_named(session: &Session, page: NodeId) -> (String, Vec<NodeId>) {
    let node = document(session).node(page).unwrap();
    (node.name.clone(), node.children.clone())
}

fn assert_one_set_of(session: &Session, page: NodeId, page_name: &str, name: &str, variants: usize) -> NodeId {
    let (actual_name, children) = page_named(session, page);
    assert_eq!(actual_name, page_name);
    let [set] = children.as_slice() else { panic!("page {actual_name} holds {} layers, expected one set", children.len()) };
    let node = document(session).node(*set).unwrap();
    assert!(matches!(node.kind, NodeKind::ComponentSet { .. }), "{:?}", node.kind);
    assert_eq!(node.name, name);
    assert_eq!(node.children.len(), variants);
    *set
}

// covers: E4-7.1
#[test]
#[ignore = "downloads @mui/material from npm and drives Chromium; run by hand"]
fn real_staged_import_lands_in_the_import_page_and_placing_it_makes_the_page_a_button_then_archives_it() {
    let (_dir, mut session) = project_with_mui("real-import-stage");
    let blank = session.docs[session.active].page;
    assert!(page_children(&session).is_empty(), "the new project's page is not empty");

    let started = session.execute("uigen.listComponents", &json!({"package": PACKAGE})).unwrap();
    let listing =
        wait_for(&mut session, "uigen.listComponents", &started).outcome.unwrap_or_else(|error| panic!("listComponents failed: {}", error.message));
    let components = listing["components"].as_array().expect("listComponents lists components");
    let button = components.iter().find(|candidate| candidate["name"] == json!("Button")).expect("listComponents lists Button");
    let variants = enum_values(button, "variant");
    assert!(variants.iter().any(|value| value == "contained") && variants.iter().any(|value| value == "outlined"), "{variants:?}");
    let sizes = enum_values(button, "size");
    assert!(sizes.iter().any(|value| value == "small") && sizes.iter().any(|value| value == "medium"), "{sizes:?}");
    let cached = session.execute("uigen.listComponents", &json!({"package": PACKAGE})).unwrap();
    assert_eq!(cached["cached"], json!(true), "{cached}");

    let all_states = ["Default", "Hover", "Pressed", "Disabled"];
    let staged = staged_import(&mut session, "Button", &["contained", "outlined"], &["small", "medium"], &all_states);
    assert_eq!(staged["variants"], json!(16), "{staged}");
    let import_page = figcraft_engine::params::id_of(&staged["page"]).unwrap_or_else(|| panic!("no page in {staged}"));
    let staged_set = assert_one_set_of(&session, import_page, &format!("- Imported · {PACKAGE} {VERSION}"), "Button", 16);
    assert_variants_are_labelled_and_sized(document(&session), staged_set);
    assert!(page_children(&session).is_empty(), "the staged import wrote on the current page");

    let chip = staged_import(&mut session, "Chip", &["filled"], &["medium"], &["Default"]);
    let chip_variants = chip["variants"]
        .as_u64()
        .and_then(|count| usize::try_from(count).ok())
        .filter(|count| *count >= 1)
        .unwrap_or_else(|| panic!("no variants in {chip}"));
    let listed = session.execute("uigen.imported", &json!({})).unwrap();
    let section = &listed["sections"][0];
    assert_eq!(section["package"], json!(PACKAGE), "{listed}");
    let sets = section["components"].as_array().expect("the section lists its sets");
    let find = |name: &str| sets.iter().find(|candidate| candidate["name"] == json!(name)).unwrap_or_else(|| panic!("no {name} in {listed}"));
    assert_eq!(find("Button")["variants"], json!(16));
    let (button_set, chip_set) = (find("Button")["set"].clone(), find("Chip")["set"].clone());
    let before = document_text(&session);
    let pages_before = document(&session).pages.len();

    let first = session.execute("uigen.placeImported", &json!({"set": button_set})).unwrap();
    assert_eq!(first["replaced"], Value::Null, "{first}");
    assert_eq!(first["name"], json!("Button"), "{first}");
    assert_eq!(figcraft_engine::params::id_of(&first["page"]), Some(blank), "{first}");
    assert_one_set_of(&session, blank, "Button", "Button", 16);
    assert_eq!(document(&session).pages.len(), pages_before);

    let second = session.execute("uigen.placeImported", &json!({"set": chip_set})).unwrap();
    assert_eq!(second["name"], json!("Chip"), "{second}");
    assert_eq!(second["replaced"]["name"], json!("Button"), "{second}");
    let archive = figcraft_engine::params::id_of(&second["replaced"]["page"]).unwrap_or_else(|| panic!("no archive page in {second}"));
    assert_eq!(
        document(&session).pages.iter().position(|page| *page == archive),
        document(&session).pages.iter().position(|page| *page == blank).map(|at| at + 1)
    );
    assert_one_set_of(&session, blank, "Chip", "Chip", chip_variants);
    assert_one_set_of(&session, archive, "Button", "Button", 16);
    assert_eq!(document(&session).pages.len(), pages_before + 1);

    session.execute("edit.undo", &json!({})).unwrap();
    session.execute("edit.undo", &json!({})).unwrap();
    assert!(page_children(&session).is_empty(), "two undos do not give back the blank page");
    assert_eq!(document_text(&session), before);
}

const COMPARE: &str = "uigen.compare";
const UPDATE_SOURCE: &str = "uigen.updateSource";
const BRAND: &str = "Brand";
const FONT_ABSENT: &str = "police absente";

fn run(session: &mut Session, command: &str, params: &Value) -> Value {
    session.execute(command, params).unwrap_or_else(|error| panic!("{command} {params}: {error}"))
}

fn wait_for_jobs(session: &mut Session, jobs: &[u64]) {
    let deadline = Instant::now() + JOB_LIMIT;
    let mut pending: Vec<u64> = jobs.to_vec();
    while !pending.is_empty() {
        for event in session.pump_capture() {
            if event.command != IMPORT {
                continue;
            }
            if let Err(error) = event.outcome {
                panic!("the recapture failed: {}", error.message);
            }
            pending.retain(|job| *job != event.job);
        }
        assert!(Instant::now() < deadline, "no import event for jobs {pending:?} within {JOB_LIMIT:?}");
        thread::sleep(Duration::from_millis(50));
    }
}

fn compared_rows(result: &Value) -> &Vec<Value> {
    result["rows"].as_array().unwrap_or_else(|| panic!("no rows in {result}"))
}

fn print_rows(label: &str, result: &Value) {
    for row in compared_rows(result) {
        eprintln!(
            "[{label}] {} status={} original={}x{} library={}x{} shapes%={} text={} reason={:?} (maxDx {}, maxDy {}, maxDwidth {})",
            row["variant"],
            row["status"],
            row["original"]["width"],
            row["original"]["height"],
            row["library"]["width"],
            row["library"]["height"],
            row["shapes"]["percent"],
            row["text"]["verdict"],
            row["text"]["reason"],
            row["text"]["maxDx"],
            row["text"]["maxDy"],
            row["text"]["maxDwidth"]
        );
    }
}

fn assert_all_rows(result: &Value, status: &str) {
    let rows = compared_rows(result);
    assert_eq!(rows.len(), 2, "{result}");
    for row in rows {
        assert_eq!(row["status"], json!(status), "{row}");
    }
}

fn size_of(size: &Value) -> (u32, u32) {
    let side = |axis: &str| size[axis].as_u64().and_then(|n| u32::try_from(n).ok()).unwrap_or(u32::MAX);
    (side("width"), side("height"))
}

fn assert_rows_match_the_original(result: &Value) {
    assert_rows_count_match_the_original(result, 2);
}

fn assert_rows_count_match_the_original(result: &Value, count: usize) {
    assert_eq!(compared_rows(result).len(), count, "{result}");
    for row in compared_rows(result) {
        let percent = row["shapes"]["percent"].as_f64().unwrap_or(f64::NAN);
        assert!(percent < 1.0, "shapes differ by {percent} %: {row}");
        assert_eq!(row["shapes"]["verdict"], json!("ok"), "{row}");
        assert!(
            sizes_agree(size_of(&row["original"]), size_of(&row["library"])),
            "the library raster is more than 1 px from the original box: {row}"
        );
        let font_absent = row["text"]["reason"].as_str().is_some_and(|reason| reason.contains(FONT_ABSENT));
        if font_absent {
            eprintln!("the font of the original is not installed on this machine: the text geometry is not asserted ({})", row["text"]["reason"]);
        } else {
            assert_eq!(row["text"]["verdict"], json!("ok"), "{row}");
            assert_eq!(row["status"], json!("ok"), "{row}");
        }
    }
}

fn delete_pngs(dir: &Path) -> usize {
    let mut deleted = 0;
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            deleted += delete_pngs(&path);
        } else if path.extension().is_some_and(|extension| extension == "png") {
            std::fs::remove_file(&path).unwrap();
            deleted += 1;
        }
    }
    deleted
}

fn bindings(doc: &Document, layers: &[NodeId]) -> Vec<(NodeId, String, String)> {
    layers
        .iter()
        .flat_map(|id| doc.node(*id).unwrap().bound.iter().map(|(field, variable)| (*id, field.clone(), variable.clone())).collect::<Vec<_>>())
        .collect()
}

fn variable_name(doc: &Document, id: &str) -> String {
    doc.variables.variables.iter().find(|variable| variable.id == id).map(|variable| variable.name.clone()).unwrap_or_default()
}

fn is_colour(doc: &Document, id: &str) -> bool {
    doc.variables.variables.iter().any(|variable| variable.id == id && variable.kind == VariableType::Color)
}

fn variable_collection(doc: &Document, id: &str) -> String {
    doc.variables.variables.iter().find(|variable| variable.id == id).map(|variable| variable.collection.clone()).unwrap_or_default()
}

// covers: E5-8.1
#[test]
#[ignore = "downloads @mui/material from npm and drives Chromium; run by hand"]
fn real_compare_of_every_style_size_and_state_of_the_mui_button_matches_its_original() {
    let (_dir, mut session) = project_with_mui("real-compare-styles");
    let staged = staged_import(&mut session, "Button", &["contained", "outlined"], &["small", "medium"], &["Default", "Hover"]);
    assert_eq!(staged["variants"], json!(8), "{staged}");
    let listed = run(&mut session, "uigen.imported", &json!({}));
    let placed = run(&mut session, "uigen.placeImported", &json!({"set": listed["sections"][0]["components"][0]["set"]}));

    let light = run(&mut session, COMPARE, &json!({"set": placed["set"], "theme": "light"}));
    print_rows("light", &light);
    eprintln!("summary {}", light["summary"]);

    let outlined = compared_rows(&light).iter().filter(|row| row["style"] == json!("outlined")).count();
    assert_eq!(outlined, 4, "{light}");
    assert_rows_count_match_the_original(&light, 8);
}

// covers: E5-8.1
#[test]
#[ignore = "downloads @mui/material from npm and drives Chromium; run by hand"]
fn real_compare_of_the_mui_button_matches_its_original_follows_the_references_and_a_brand_theme_rebinds_one_token() {
    let (dir, mut session) = project_with_mui("real-compare");
    let blank = session.docs[session.active].page;
    let staged = staged_import(&mut session, "Button", &["contained"], &["medium"], &["Default", "Hover"]);
    assert_eq!(staged["variants"], json!(2), "{staged}");
    let listed = run(&mut session, "uigen.imported", &json!({}));
    let staged_set = listed["sections"][0]["components"][0]["set"].clone();
    let placed = run(&mut session, "uigen.placeImported", &json!({"set": staged_set}));
    assert_eq!(figcraft_engine::params::id_of(&placed["page"]), Some(blank), "{placed}");
    let copy = figcraft_engine::params::id_of(&placed["set"]).unwrap_or_else(|| panic!("no set in {placed}"));

    let light = run(&mut session, COMPARE, &json!({"set": placed["set"], "theme": "light"}));
    print_rows("light", &light);
    eprintln!("summary {}", light["summary"]);
    assert_rows_match_the_original(&light);

    let dark = run(&mut session, COMPARE, &json!({"set": placed["set"], "theme": "dark"}));
    print_rows("dark", &dark);
    assert_all_rows(&dark, "notComparable");

    let captures = dir.path().join(".uigen").join("captures");
    let deleted = delete_pngs(&captures);
    assert!(deleted >= 2, "only {deleted} PNG deleted in {}", captures.display());
    let missing = run(&mut session, COMPARE, &json!({"set": placed["set"], "theme": "light"}));
    print_rows("missing", &missing);
    assert_all_rows(&missing, "missingReference");

    let update = run(&mut session, UPDATE_SOURCE, &json!({"package": PACKAGE}));
    let jobs: Vec<u64> = update["jobs"].as_array().unwrap_or_else(|| panic!("no jobs in {update}")).iter().filter_map(Value::as_u64).collect();
    assert!(!jobs.is_empty(), "{update}");
    wait_for_jobs(&mut session, &jobs);
    let restored = run(&mut session, COMPARE, &json!({"set": placed["set"], "theme": "light"}));
    print_rows("restored", &restored);
    assert_rows_match_the_original(&restored);

    let themes = run(&mut session, "uigen.themes", &json!({}));
    let collection = format!("{PACKAGE} {VERSION}");
    let listed_theme = themes["themes"]
        .as_array()
        .and_then(|all| all.iter().find(|theme| theme["name"] == json!(collection)))
        .unwrap_or_else(|| panic!("no theme {collection} in {themes}"));
    assert_eq!(listed_theme["origin"], json!({"package": PACKAGE, "version": VERSION}), "{themes}");
    assert!(listed_theme["boundLayers"].as_u64().unwrap_or(0) > 0, "{themes}");

    let mut layers = Vec::new();
    subtree(document(&session), copy, &mut layers);
    let before = bindings(document(&session), &layers);
    let mui_id = document(&session).variables.collections.iter().find(|candidate| candidate.name == collection).unwrap().id.clone();
    let colour_names: Vec<String> = before
        .iter()
        .filter(|(_, _, variable)| is_colour(document(&session), variable))
        .map(|(_, _, variable)| variable_name(document(&session), variable))
        .collect();
    eprintln!("colour variables bound in the library copy: {colour_names:?}");
    let token = colour_names
        .iter()
        .find(|name| name.to_lowercase().contains("primary"))
        .or_else(|| colour_names.iter().find(|name| name.contains("containedBg")))
        .or_else(|| colour_names.first())
        .cloned()
        .expect("the copy binds a colour variable");
    eprintln!("Brand token: {token}");
    run(&mut session, "variable.createCollection", &json!({"name": BRAND, "modes": ["Light", "Dark"]}));
    run(
        &mut session,
        "variable.create",
        &json!({"collection": BRAND, "name": token, "type": "color", "values": {"Light": "#7B1FA2", "Dark": "#CE93D8"}}),
    );

    let applied = run(&mut session, "uigen.applyTheme", &json!({"theme": BRAND}));
    eprintln!("applyTheme {applied}");
    assert_eq!(applied["unchanged"], json!(false), "{applied}");
    let after = bindings(document(&session), &layers);
    assert_eq!(before.len(), after.len());
    let brand_id = document(&session).variables.collections.iter().find(|candidate| candidate.name == BRAND).unwrap().id.clone();
    let mut rebound = 0;
    for ((layer, field, from), (_, _, to)) in before.iter().zip(&after) {
        let doc = document(&session);
        let follows_token = variable_collection(doc, from) == mui_id && variable_name(doc, from).trim().eq_ignore_ascii_case(&token);
        if follows_token {
            assert_eq!(variable_collection(doc, to), brand_id, "{layer:?}.{field} did not move to {BRAND}");
            rebound += 1;
        } else {
            assert_eq!(from, to, "{layer:?}.{field} changed though it is not the token {token}");
        }
    }
    assert!(rebound > 0, "no binding of {token} moved");
    assert_eq!(applied["rebound"], json!(rebound), "{applied}");
}
