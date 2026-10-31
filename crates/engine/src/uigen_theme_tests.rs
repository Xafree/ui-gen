//! `uigen.themes` (the token themes of the file, with the layers that follow each) and
//! `uigen.applyTheme` (the layers of the library follow another collection, in one undo step, and
//! `uigen.json` remembers it). The library is filled by a real staged import of a synthetic button,
//! copied into the library with `uigen.placeImported`; the second theme, "Brand", is built variable
//! by variable from the imported one.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use figcraft_doc::{Document, NodeId, VariableType};
use serde_json::{Value, json};

use crate::capture::{CaptureBackend, CaptureErrorKind, CaptureJob, CaptureOutcome, JobId};
use crate::cmd::find_command;
use crate::devinspect::dev_mode_allows;
use crate::library::is_remote_collection;
use crate::params::id_of;
use crate::project::{component_pages, component_roots};
use crate::uigen_import::is_import_page;
use crate::uigen_import_support::{Bench, IMPORT, PACKAGE, VERSION, button_inspect, service_failure, synthetic_source};
use crate::{EngineError, Result, Session};

const THEMES: &str = "uigen.themes";
const APPLY: &str = "uigen.applyTheme";
const COLLECTION: &str = "synthetic-ui 1.0.0";
const BRAND: &str = "Brand";
const SOLID_FILL: &str = "couleur/button/fond/solid";
const BORDER: &str = "couleur/button/bordure";
const RADIUS: &str = "rayon/button/rayon";
const EXTRAS: usize = 60;
const MAX_UNMATCHED: usize = 50;
const MAX_SWATCHES: usize = 8;

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn button(style: &str, size: &str, theme: &str) -> Value {
    let dark = theme == "dark";
    let fill = match (style == "solid", dark) {
        (true, false) => "rgb(25, 118, 210)",
        (true, true) => "rgb(144, 202, 249)",
        (false, false) => "rgb(255, 255, 255)",
        (false, true) => "rgb(18, 18, 18)",
    };
    let (ink, border) = if dark { ("rgb(18, 18, 18)", "rgb(100, 181, 246)") } else { ("rgb(255, 255, 255)", "rgb(13, 71, 161)") };
    let width = if size == "sm" { 90.0 } else { 106.0 };
    let mut css = serde_json::Map::new();
    for (key, value) in [
        ("display", "flex"),
        ("flexDirection", "row"),
        ("justifyContent", "flex-start"),
        ("alignItems", "flex-start"),
        ("columnGap", "8px"),
        ("rowGap", "0px"),
        ("paddingTop", "8px"),
        ("paddingRight", "16px"),
        ("paddingBottom", "8px"),
        ("paddingLeft", "16px"),
        ("backgroundColor", fill),
        ("borderTopLeftRadius", "6px"),
        ("borderTopRightRadius", "6px"),
        ("borderBottomRightRadius", "6px"),
        ("borderBottomLeftRadius", "6px"),
    ] {
        css.insert(key.to_owned(), json!(value));
    }
    for side in ["Top", "Right", "Bottom", "Left"] {
        css.insert(format!("border{side}Width"), json!("1px"));
        css.insert(format!("border{side}Style"), json!("solid"));
        css.insert(format!("border{side}Color"), json!(border));
    }
    json!({
        "version": 1,
        "viewport": {"width": 400, "height": 200, "deviceScaleFactor": 1},
        "root": {
            "type": "element", "tag": "button", "label": "Button", "className": "syn-button",
            "box": {"x": 20, "y": 20, "width": width, "height": 36},
            "style": css, "vars": {},
            "children": [{
                "type": "text", "text": "Go",
                "box": {"x": 60, "y": 28, "width": width - 56.0, "height": 20},
                "lineBox": {"x": 60, "y": 28, "width": width - 56.0, "height": 20},
                "style": {"color": ink, "fontSize": "14px"}
            }]
        }
    })
}

struct Backend {
    submitted: u64,
    ready: Vec<(JobId, CaptureOutcome)>,
}

impl CaptureBackend for Backend {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let outcome = match &job {
            CaptureJob::Inspect { .. } => button_inspect(true, false),
            CaptureJob::Render { props, theme, .. } => {
                let style = props.get("variant").and_then(Value::as_str).unwrap_or("solid");
                let size = props.get("size").and_then(Value::as_str).unwrap_or("md");
                Ok(json!({"snapshot": button(style, size, theme), "png": ".uigen/captures/synthetic.png"}))
            }
            CaptureJob::Install { .. } | CaptureJob::InstallMany { .. } | CaptureJob::Uninstall { .. } | CaptureJob::Playground { .. } => {
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

struct Lab {
    bench: Bench,
    start: NodeId,
    annotation: NodeId,
}

fn id_in(result: &Value, key: &str) -> NodeId {
    id_of(&result[key]).unwrap_or_else(|| panic!("no {key} in {result}"))
}

fn run(bench: &mut Bench, command: &str, params: &Value) -> Value {
    bench.session.execute(command, params).unwrap_or_else(|error| panic!("{command} {params}: {error}"))
}

fn doc(bench: &Bench) -> &Document {
    bench.session.docs[0].doc.as_ref()
}

fn collection_id(doc: &Document, name: &str) -> String {
    doc.variables.collections.iter().find(|collection| collection.name == name).unwrap_or_else(|| panic!("no collection {name:?}")).id.clone()
}

fn variables_named(doc: &Document, collection: &str) -> Vec<(String, String, VariableType)> {
    let id = collection_id(doc, collection);
    doc.variables
        .variables
        .iter()
        .filter(|variable| variable.collection == id)
        .map(|variable| (variable.id.clone(), variable.name.clone(), variable.kind))
        .collect()
}

fn variable_id(doc: &Document, collection: &str, name: &str) -> String {
    variables_named(doc, collection)
        .into_iter()
        .find(|(_, candidate, _)| candidate == name)
        .unwrap_or_else(|| panic!("no variable {name:?} in {collection:?}"))
        .0
}

fn kind_name(kind: VariableType) -> &'static str {
    match kind {
        VariableType::Color => "color",
        VariableType::Number => "number",
        VariableType::String => "string",
        VariableType::Boolean => "boolean",
    }
}

fn subtree(doc: &Document, id: NodeId) -> Vec<NodeId> {
    let mut out = vec![id];
    if let Some(node) = doc.node(id) {
        for child in &node.children {
            out.extend(subtree(doc, *child));
        }
    }
    out
}

fn library_layers(doc: &Document) -> Vec<NodeId> {
    component_pages(doc).iter().flat_map(|info| component_roots(doc, info.page)).flat_map(|root| subtree(doc, root)).collect()
}

fn import_layers(doc: &Document) -> Vec<NodeId> {
    doc.pages.iter().filter(|page| is_import_page(doc, **page)).flat_map(|page| subtree(doc, *page)).collect()
}

fn every_layer(doc: &Document) -> Vec<NodeId> {
    doc.pages.iter().flat_map(|page| subtree(doc, *page)).collect()
}

fn bindings_of(doc: &Document, layers: &[NodeId]) -> Vec<(NodeId, String, String)> {
    layers
        .iter()
        .flat_map(|id| {
            doc.node(*id).map(|node| node.bound.iter().map(|(field, variable)| (*id, field.clone(), variable.clone())).collect::<Vec<_>>())
        })
        .flatten()
        .collect()
}

fn serialized_bindings(doc: &Document) -> Vec<(NodeId, String)> {
    every_layer(doc).into_iter().filter_map(|id| doc.node(id).map(|node| (id, serde_json::to_string(&node.bound).unwrap_or_default()))).collect()
}

fn values_without_bindings(doc: &Document) -> Vec<String> {
    every_layer(doc)
        .into_iter()
        .filter_map(|id| doc.node(id))
        .map(|node| {
            let mut copy = node.clone();
            copy.bound.clear();
            format!("{copy:?}")
        })
        .collect()
}

fn values_outside(doc: &Document, library: &BTreeSet<NodeId>) -> Vec<String> {
    every_layer(doc).into_iter().filter(|id| !library.contains(id)).filter_map(|id| doc.node(id)).map(|node| format!("{node:?}")).collect()
}

fn variable_owner(doc: &Document, variable: &str) -> Option<String> {
    doc.variables.variables.iter().find(|candidate| candidate.id == variable).map(|candidate| candidate.collection.clone())
}

fn followers(doc: &Document, layers: &[NodeId], collection: &str) -> usize {
    let wanted = collection_id(doc, collection);
    layers
        .iter()
        .filter(|id| {
            doc.node(**id).is_some_and(|node| node.bound.values().any(|variable| variable_owner(doc, variable).as_deref() == Some(wanted.as_str())))
        })
        .count()
}

fn key(name: &str) -> String {
    name.trim().to_lowercase()
}

fn equivalent(doc: &Document, from_variable: &str, target: &str) -> Option<String> {
    let source = doc.variables.variables.iter().find(|variable| variable.id == from_variable)?;
    let wanted = collection_id(doc, target);
    doc.variables
        .variables
        .iter()
        .find(|variable| variable.collection == wanted && key(&variable.name) == key(&source.name) && variable.kind == source.kind)
        .map(|variable| variable.id.clone())
}

fn expected_rebound(doc: &Document, layers: &[NodeId], from: &str, to: &str) -> (usize, usize) {
    let from_id = collection_id(doc, from);
    let mut changed = BTreeSet::new();
    let mut bindings = 0;
    for (layer, _, variable) in bindings_of(doc, layers) {
        if variable_owner(doc, &variable).as_deref() == Some(from_id.as_str()) && equivalent(doc, &variable, to).is_some() {
            bindings += 1;
            changed.insert(layer);
        }
    }
    (bindings, changed.len())
}

fn lab(tag: &str) -> Lab {
    let mut bench = Bench::without_backend(tag, &[synthetic_source(false)]);
    bench.session.set_capture_backend(Box::new(Backend { submitted: 0, ready: Vec::new() }));
    let start = bench.session.docs[0].page;
    let params = json!({"package": PACKAGE, "component": "Button", "states": ["Default"], "themes": ["light", "dark"], "stage": true});
    bench.import(&params).unwrap();
    let events = bench.pump_to_end();
    let event = events.iter().find(|event| event.command == IMPORT).unwrap_or_else(|| panic!("no import event in {events:?}"));
    let result = match &event.outcome {
        Ok(result) => result.clone(),
        Err(error) => panic!("the import failed: {}", error.message),
    };
    let set = id_in(&result, "set");
    run(&mut bench, "uigen.placeImported", &json!({"set": set, "page": start}));
    let annotation = id_in(&run(&mut bench, "frame.create", &json!({"x": 400, "y": 60, "width": 80, "height": 40, "name": "Note"})), "id");
    run(&mut bench, "node.set", &json!({"ids": [annotation], "props": {"pluginData": {"uigen.role": "note"}}}));
    let solid = variable_id(doc(&bench), COLLECTION, SOLID_FILL);
    run(&mut bench, "variable.bind", &json!({"ids": [annotation], "field": "fill", "variable": solid}));
    assert!(!library_layers(doc(&bench)).is_empty());
    assert!(followers(doc(&bench), &library_layers(doc(&bench)), COLLECTION) > 0, "the library copy follows the imported collection");
    assert!(followers(doc(&bench), &import_layers(doc(&bench)), COLLECTION) > 0, "the import page follows it too");
    Lab { bench, start, annotation }
}

fn color_for(index: usize) -> String {
    format!("#{:02X}{:02X}{:02X}", 20 + index % 200, 60 + index % 150, 100 + index % 100)
}

fn mirror_value(kind: VariableType, index: usize) -> Value {
    match kind {
        VariableType::Color => json!(color_for(index)),
        VariableType::Number => json!(99),
        VariableType::String => json!("x"),
        VariableType::Boolean => json!(true),
    }
}

fn add_brand(lab: &mut Lab, skip: &[&str], retype: &[&str]) {
    let mirrored = variables_named(doc(&lab.bench), COLLECTION);
    run(&mut lab.bench, "variable.createCollection", &json!({"name": BRAND, "modes": ["Light", "Dark"]}));
    for (index, (_, name, kind)) in mirrored.into_iter().enumerate() {
        if skip.contains(&name.as_str()) {
            continue;
        }
        let kind =
            if retype.contains(&name.as_str()) { if kind == VariableType::Color { VariableType::Number } else { VariableType::Color } } else { kind };
        let written = if name == SOLID_FILL { format!("  {}  ", name.to_uppercase()) } else { name };
        let values = json!({"Light": mirror_value(kind, index), "Dark": mirror_value(kind, index + 1)});
        run(&mut lab.bench, "variable.create", &json!({"collection": BRAND, "name": written, "type": kind_name(kind), "values": values}));
    }
}

fn lab_with_brand(tag: &str) -> Lab {
    let mut lab = lab(tag);
    add_brand(&mut lab, &[], &[]);
    lab
}

fn manifest_path(bench: &Bench) -> PathBuf {
    let root = bench.session.project().map(|binding| binding.root.clone()).unwrap_or_default();
    PathBuf::from(root).join("uigen.json")
}

fn read_manifest(bench: &Bench) -> Value {
    serde_json::from_slice(&std::fs::read(manifest_path(bench)).unwrap()).unwrap()
}

fn manifest_bytes(bench: &Bench) -> Vec<u8> {
    std::fs::read(manifest_path(bench)).unwrap()
}

fn write_manifest(bench: &Bench, manifest: &Value) {
    std::fs::write(manifest_path(bench), serde_json::to_string_pretty(manifest).unwrap()).unwrap();
}

fn undo_steps(bench: &Bench) -> usize {
    bench.session.undo_depth().0
}

fn applied(bench: &mut Bench) -> Value {
    run(bench, "project.info", &json!({}))["project"]["appliedTheme"].clone()
}

fn row<'a>(result: &'a Value, name: &str) -> &'a Value {
    result["themes"]
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["name"] == json!(name)))
        .unwrap_or_else(|| panic!("no theme {name:?} in {result}"))
}

fn is_hex_color(value: &Value) -> bool {
    value.as_str().is_some_and(|text| text.len() == 7 && text.starts_with('#') && text[1..].chars().all(|digit| digit.is_ascii_hexdigit()))
}

fn bad_params(result: Result<Value>, what: &str) {
    match result {
        Err(EngineError::BadParams { .. }) => {}
        other => panic!("{what}: expected a refusal, got {other:?}"),
    }
}

struct Frozen {
    document: String,
    manifest: Vec<u8>,
    undo: usize,
}

fn freeze(bench: &Bench) -> Frozen {
    Frozen { document: bench.document_text(0), manifest: manifest_bytes(bench), undo: undo_steps(bench) }
}

fn assert_frozen(bench: &Bench, before: &Frozen, what: &str) {
    assert_eq!(bench.document_text(0), before.document, "{what}: the document changed");
    assert_eq!(manifest_bytes(bench), before.manifest, "{what}: uigen.json changed");
    assert_eq!(undo_steps(bench), before.undo, "{what}: an undo step appeared");
}

// covers: E5-4.1
#[test]
fn themes_lists_both_collections_in_file_order_with_modes_counts_followers_origin_and_swatches() {
    let mut lab = lab_with_brand("themes-list");

    let result = run(&mut lab.bench, THEMES, &json!({}));

    assert!(result["applied"].is_null(), "{result}");
    let rows = result["themes"].as_array().unwrap_or_else(|| panic!("no themes in {result}"));
    let names: Vec<&str> = rows.iter().filter_map(|row| row["name"].as_str()).collect();
    assert_eq!(names, [COLLECTION, BRAND]);
    let doc = doc(&lab.bench);
    assert_eq!(rows[0]["collection"], json!(collection_id(doc, COLLECTION)));
    assert_eq!(rows[1]["collection"], json!(collection_id(doc, BRAND)));
    assert_eq!(rows[0]["modes"], json!(["Light", "Dark"]));
    assert_eq!(rows[1]["modes"], json!(["Light", "Dark"]));
    assert_eq!(rows[0]["variables"], json!(variables_named(doc, COLLECTION).len()));
    assert_eq!(rows[1]["variables"], json!(variables_named(doc, BRAND).len()));

    let library = library_layers(doc);
    let followed = followers(doc, &library, COLLECTION);
    let everywhere = followers(doc, &every_layer(doc), COLLECTION);
    assert!(followed < everywhere, "the import page and the annotation follow it too: {followed} of {everywhere}");
    assert_eq!(rows[0]["boundLayers"], json!(followed));
    assert_eq!(rows[1]["boundLayers"], json!(0));
    assert!(rows.iter().all(|row| row["applied"] == json!(false)), "{result}");

    assert_eq!(rows[0]["origin"], json!({"package": PACKAGE, "version": VERSION}));
    assert!(rows[1]["origin"].is_null(), "{}", rows[1]);

    let swatches = rows[0]["swatches"].as_array().unwrap_or_else(|| panic!("no swatches in {}", rows[0]));
    assert!(!swatches.is_empty() && swatches.len() <= MAX_SWATCHES, "{swatches:?}");
    assert!(swatches.iter().all(|swatch| is_hex_color(&swatch["light"]) && is_hex_color(&swatch["dark"])), "{swatches:?}");
    let solid =
        swatches.iter().find(|swatch| swatch["name"] == json!(SOLID_FILL)).unwrap_or_else(|| panic!("no swatch for {SOLID_FILL}: {swatches:?}"));
    assert!(solid["light"].as_str().is_some_and(|hex| hex.eq_ignore_ascii_case("#1976D2")), "{solid}");
    assert!(solid["dark"].as_str().is_some_and(|hex| hex.eq_ignore_ascii_case("#90CAF9")), "{solid}");
}

// covers: E5-4.1
#[test]
fn themes_cuts_swatches_at_eight_adds_no_undo_step_and_refuses_outside_a_project() {
    let mut lab = lab_with_brand("themes-limits");
    run(&mut lab.bench, "variable.createCollection", &json!({"name": "Wide"}));
    for index in 0..12 {
        run(
            &mut lab.bench,
            "variable.create",
            &json!({"collection": "Wide", "name": format!("c{index}"), "type": "color", "value": color_for(index)}),
        );
    }
    let before = freeze(&lab.bench);

    let result = run(&mut lab.bench, THEMES, &json!({}));

    let wide = row(&result, "Wide");
    assert_eq!(wide["variables"], json!(12));
    assert_eq!(wide["swatches"].as_array().map(Vec::len), Some(MAX_SWATCHES), "{wide}");
    assert_frozen(&lab.bench, &before, "listing the themes");

    run(&mut lab.bench, "file.new", &json!({}));
    bad_params(lab.bench.session.execute(THEMES, &json!({})), "outside a project");
}

// covers: E5-4.2
#[test]
fn applying_brand_rebinds_the_library_layers_to_same_name_variables_and_changes_no_value() {
    let mut lab = lab_with_brand("apply-nominal");
    let mut manifest = read_manifest(&lab.bench);
    manifest["studio"] = json!({"k": [1, 2]});
    write_manifest(&lab.bench, &manifest);
    let library = library_layers(doc(&lab.bench));
    let before_bindings = bindings_of(doc(&lab.bench), &every_layer(doc(&lab.bench)));
    let serialized_before = serialized_bindings(doc(&lab.bench));
    let values_before = values_without_bindings(doc(&lab.bench));
    let values_outside_before = values_outside(doc(&lab.bench), &library.iter().copied().collect());
    let variables_before = format!("{:?}", doc(&lab.bench).variables);
    let (expected_bindings, expected_layers) = expected_rebound(doc(&lab.bench), &library, COLLECTION, BRAND);
    assert!(expected_bindings > 0 && expected_layers > 0);
    let depth = undo_steps(&lab.bench);

    let result = run(&mut lab.bench, APPLY, &json!({"theme": BRAND}));

    assert_eq!(result["rebound"], json!(expected_bindings), "{result}");
    assert_eq!(result["layers"], json!(expected_layers), "{result}");
    assert!(result["previous"].is_null(), "{result}");
    assert_eq!(result["unmatched"], json!([]), "{result}");
    assert_eq!(result["unchanged"], json!(false), "{result}");
    assert_eq!(undo_steps(&lab.bench), depth + 1);

    let doc = doc(&lab.bench);
    let after_bindings = bindings_of(doc, &every_layer(doc));
    assert_eq!(after_bindings.len(), before_bindings.len());
    let library_ids: BTreeSet<NodeId> = library.iter().copied().collect();
    for (before, after) in before_bindings.iter().zip(&after_bindings) {
        assert_eq!((before.0, &before.1), (after.0, &after.1), "the same fields stay bound");
        let rebindable = library_ids.contains(&before.0) && variable_owner(doc, &before.2) == Some(collection_id(doc, COLLECTION));
        if rebindable {
            assert_eq!(equivalent(doc, &before.2, BRAND).as_deref(), Some(after.2.as_str()), "layer {:?} field {}", before.0, before.1);
        } else {
            assert_eq!(before.2, after.2, "layer {:?} field {} must not move", before.0, before.1);
        }
    }
    assert_eq!(values_outside(doc, &library_ids), values_outside_before, "the import pages and annotations do not move");
    assert_ne!(values_without_bindings(doc), values_before, "the library layers take the values of the theme");
    assert_eq!(format!("{:?}", doc.variables), variables_before, "no variable changes");
    assert_eq!(applied(&mut lab.bench), json!(BRAND));
    let saved = read_manifest(&lab.bench);
    assert_eq!(saved["appliedTheme"], json!(BRAND));
    assert_eq!(saved["studio"], json!({"k": [1, 2]}), "unknown keys are kept");
    assert_eq!(saved["name"], json!("mylib"));
    assert_eq!(saved["sources"], manifest["sources"]);

    run(&mut lab.bench, "edit.undo", &json!({}));

    assert_eq!(serialized_bindings(lab.bench.session.docs[0].doc.as_ref()), serialized_before, "one undo restores the previous bindings");
    assert_eq!(read_manifest(&lab.bench)["appliedTheme"], json!(BRAND), "the manifest is outside the history");
    assert_eq!(applied(&mut lab.bench), json!(BRAND));
}

// covers: E5-4.3
#[test]
fn a_variable_with_no_equivalent_keeps_its_binding_is_listed_and_the_import_pages_never_move() {
    let mut lab = lab("apply-unmatched");
    add_brand(&mut lab, &[BORDER], &[RADIUS]);
    let library = library_layers(doc(&lab.bench));
    let imports = import_layers(doc(&lab.bench));
    let kept: Vec<String> = [BORDER, RADIUS].iter().map(|name| variable_id(doc(&lab.bench), COLLECTION, name)).collect();
    let before = bindings_of(doc(&lab.bench), &every_layer(doc(&lab.bench)));
    assert!(before.iter().any(|(layer, _, variable)| library.contains(layer) && kept.contains(variable)), "the library uses both variables");
    let imports_before = bindings_of(doc(&lab.bench), &imports);
    let (expected_bindings, _) = expected_rebound(doc(&lab.bench), &library, COLLECTION, BRAND);

    let result = run(&mut lab.bench, APPLY, &json!({"theme": BRAND}));

    let unmatched: Vec<&str> =
        result["unmatched"].as_array().unwrap_or_else(|| panic!("no unmatched in {result}")).iter().filter_map(Value::as_str).collect();
    assert!(unmatched.contains(&BORDER), "name absent from Brand: {unmatched:?}");
    assert!(unmatched.contains(&RADIUS), "same name, other type: {unmatched:?}");
    assert!(!unmatched.contains(&SOLID_FILL), "{unmatched:?}");
    assert_eq!(result["rebound"], json!(expected_bindings), "{result}");

    let doc = doc(&lab.bench);
    let after = bindings_of(doc, &every_layer(doc));
    for (layer, field, variable) in before.iter().filter(|(layer, _, variable)| library.contains(layer) && kept.contains(variable)) {
        assert!(after.contains(&(*layer, field.clone(), variable.clone())), "layer {layer:?} field {field} keeps {variable}");
    }
    assert_eq!(bindings_of(doc, &imports), imports_before, "import pages and their sets are never rebound");
}

// covers: E5-4.3
#[test]
fn the_unmatched_list_stops_at_fifty_names() {
    let mut lab = lab_with_brand("apply-cap");
    run(&mut lab.bench, "page.select", &json!({"id": lab.start}));
    for index in 0..EXTRAS {
        let name = format!("extra {index}");
        let created =
            run(&mut lab.bench, "variable.create", &json!({"collection": COLLECTION, "name": name, "type": "color", "value": color_for(index)}));
        let variable = created["id"].as_str().unwrap_or_default().to_owned();
        let layer = id_in(&run(&mut lab.bench, "frame.create", &json!({"x": 10 * index, "y": 300, "width": 8, "height": 8, "name": name})), "id");
        run(&mut lab.bench, "variable.bind", &json!({"ids": [layer], "field": "fill", "variable": variable}));
    }

    let result = run(&mut lab.bench, APPLY, &json!({"theme": BRAND}));

    assert_eq!(result["unmatched"].as_array().map(Vec::len), Some(MAX_UNMATCHED), "{result}");
    assert!(result["rebound"].as_u64().unwrap_or(0) > 0, "{result}");
}

// covers: E5-4.4
#[test]
fn applying_the_import_collection_after_brand_restores_the_original_bindings() {
    let mut lab = lab_with_brand("apply-back");
    let start = serialized_bindings(doc(&lab.bench));
    let values = values_without_bindings(doc(&lab.bench));
    run(&mut lab.bench, APPLY, &json!({"theme": BRAND}));
    assert_ne!(serialized_bindings(doc(&lab.bench)), start);

    let back = run(&mut lab.bench, APPLY, &json!({"theme": COLLECTION}));

    assert_eq!(back["previous"], json!(BRAND), "{back}");
    assert_eq!(back["unmatched"], json!([]), "{back}");
    assert_eq!(serialized_bindings(doc(&lab.bench)), start);
    assert_eq!(values_without_bindings(doc(&lab.bench)), values);
    assert_eq!(read_manifest(&lab.bench)["appliedTheme"], json!(COLLECTION));
}

// covers: E5-4.5
#[test]
fn reapplying_a_followed_theme_is_unchanged_and_in_sync_until_an_undo_alone_breaks_it() {
    let mut lab = lab_with_brand("apply-idempotent");
    let first = run(&mut lab.bench, APPLY, &json!({"theme": BRAND}));
    assert_eq!(first["unchanged"], json!(false), "{first}");
    let in_sync = run(&mut lab.bench, THEMES, &json!({}));
    assert_eq!(in_sync["applied"], json!(BRAND));
    assert_eq!(in_sync["inSync"], json!(true), "{in_sync}");
    assert_eq!(row(&in_sync, BRAND)["applied"], json!(true));
    assert_eq!(row(&in_sync, COLLECTION)["applied"], json!(false));
    let frozen = freeze(&lab.bench);

    let again = run(&mut lab.bench, APPLY, &json!({"theme": BRAND}));

    assert_eq!(again["unchanged"], json!(true), "{again}");
    assert_eq!(again["rebound"], json!(0), "{again}");
    assert_frozen(&lab.bench, &frozen, "reapplying the followed theme");

    run(&mut lab.bench, "edit.undo", &json!({}));
    let broken = run(&mut lab.bench, THEMES, &json!({}));
    assert_eq!(broken["applied"], json!(BRAND), "the manifest does not move with the undo");
    assert_eq!(broken["inSync"], json!(false), "{broken}");

    let repaired = run(&mut lab.bench, APPLY, &json!({"theme": BRAND}));
    assert_eq!(repaired["unchanged"], json!(false), "{repaired}");
    assert!(repaired["rebound"].as_u64().unwrap_or(0) > 0, "{repaired}");
    assert_eq!(run(&mut lab.bench, THEMES, &json!({}))["inSync"], json!(true));
}

// covers: E5-4.6
#[test]
fn both_commands_refuse_outside_a_project_and_the_apply_command_is_an_edit_that_dev_mode_refuses() {
    let mut lab = lab_with_brand("refuse-outside");
    run(&mut lab.bench, "file.new", &json!({}));
    let before = lab.bench.document_text(1);

    bad_params(lab.bench.session.execute(THEMES, &json!({})), THEMES);
    bad_params(lab.bench.session.execute(APPLY, &json!({"theme": BRAND})), APPLY);

    assert_eq!(lab.bench.document_text(1), before);
    assert!(!dev_mode_allows(APPLY), "applying a theme edits the design");
    assert!(dev_mode_allows(THEMES), "listing edits nothing");
    let apply = find_command(APPLY).unwrap_or_else(|| panic!("{APPLY} is not a command"));
    assert_eq!((apply.label, apply.history), ("Apply token theme", true));
    let list = find_command(THEMES).unwrap_or_else(|| panic!("{THEMES} is not a command"));
    assert_eq!((list.label, list.history), ("List token themes", false));
}

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct LibraryFile {
    _dir: TempDir,
    id: String,
    path: String,
}

fn library_file() -> LibraryFile {
    let dir = std::env::temp_dir().join(format!("figcraft-theme-lib-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
    std::fs::create_dir_all(&dir).unwrap();
    let guard = TempDir(dir.clone());
    let path = dir.join("theme.figcraft").to_string_lossy().into_owned();
    let mut library = Session::new();
    run_session(&mut library, "variable.createCollection", &json!({"name": "Theme", "modes": ["Light", "Dark"]}));
    run_session(
        &mut library,
        "variable.create",
        &json!({"collection": "Theme", "name": SOLID_FILL, "type": "color", "values": {"Light": "#0000ff", "Dark": "#ffffff"}}),
    );
    let published = run_session(&mut library, "library.publish", &json!({}));
    run_session(&mut library, "file.saveAs", &json!({"path": path}));
    let id = published["id"].as_str().unwrap_or_default().to_owned();
    LibraryFile { _dir: guard, id, path }
}

fn run_session(session: &mut Session, command: &str, params: &Value) -> Value {
    session.execute(command, params).unwrap_or_else(|error| panic!("{command} {params}: {error}"))
}

// covers: E5-4.1
// covers: E5-4.6
#[test]
fn unknown_collections_library_copies_and_themes_with_no_equivalent_are_refused_and_change_nothing() {
    let mut lab = lab_with_brand("refuse-targets");
    let library = library_file();
    run(&mut lab.bench, "library.add", &json!({"path": library.path}));
    run(&mut lab.bench, "library.bindVariable", &json!({"library": library.id, "variable": SOLID_FILL, "field": "fill", "ids": [lab.annotation]}));
    let copy = doc(&lab.bench)
        .variables
        .collections
        .iter()
        .find(|collection| is_remote_collection(doc(&lab.bench), &collection.id))
        .map(|collection| collection.name.clone());
    let copy = copy.unwrap_or_else(|| panic!("the library copy is missing"));
    run(&mut lab.bench, "variable.createCollection", &json!({"name": "Alien"}));
    run(
        &mut lab.bench,
        "variable.create",
        &json!({"collection": "Alien", "name": "nothing like the imported names", "type": "color", "value": "#123456"}),
    );

    let listed = run(&mut lab.bench, THEMES, &json!({}));
    let rows = listed["themes"].as_array().unwrap_or_else(|| panic!("no themes in {listed}"));
    let names: Vec<&str> = rows.iter().filter_map(|row| row["name"].as_str()).collect();
    assert_eq!(names, [COLLECTION, BRAND, "Alien"], "library copies are not themes");

    let before = freeze(&lab.bench);
    for (what, theme) in [("unknown collection", "Nope"), ("library copy", copy.as_str()), ("no equivalent at all", "Alien")] {
        bad_params(lab.bench.session.execute(APPLY, &json!({"theme": theme})), what);
        assert_frozen(&lab.bench, &before, what);
    }
}

// covers: E5-4.6
#[test]
fn absurd_parameters_are_refused_by_apply_and_never_panic_or_write_in_either_command() {
    let mut lab = lab_with_brand("refuse-absurd");
    let before = freeze(&lab.bench);
    let long = "t".repeat(201);
    let junk = [
        json!(null),
        json!([1, 2]),
        json!("Brand"),
        json!({}),
        json!({"theme": null}),
        json!({"theme": ""}),
        json!({"theme": "   "}),
        json!({"theme": 5}),
        json!({"theme": 1e308}),
        json!({"theme": true}),
        json!({"theme": ["Brand"]}),
        json!({"theme": {"name": "Brand"}}),
        json!({"theme": "\u{0}"}),
        json!({"theme": long}),
    ];
    for params in &junk {
        bad_params(lab.bench.session.execute(APPLY, params), &params.to_string());
        assert_frozen(&lab.bench, &before, &params.to_string());
    }
    let listing_junk = [json!(null), json!([1]), json!("x"), json!({"x": 1}), json!({"theme": 5}), json!({"theme": ["Brand"]})];
    for params in &listing_junk {
        match lab.bench.session.execute(THEMES, params) {
            Ok(_) | Err(EngineError::BadParams { .. }) => {}
            other => panic!("{params}: {other:?}"),
        }
        assert_frozen(&lab.bench, &before, &params.to_string());
    }
}

// covers: E5-4.7
#[test]
fn a_manifest_naming_a_collection_that_is_gone_is_reported_without_error() {
    let mut lab = lab_with_brand("themes-gone");
    run(&mut lab.bench, "project.update", &json!({"appliedTheme": "Gone"}));

    let result = run(&mut lab.bench, THEMES, &json!({}));

    assert_eq!(result["applied"], json!("Gone"), "{result}");
    assert_eq!(result["inSync"], json!(false), "{result}");
    let rows = result["themes"].as_array().unwrap_or_else(|| panic!("no themes in {result}"));
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row["applied"] == json!(false)), "{result}");
}
