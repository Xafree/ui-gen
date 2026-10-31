//! `uigen.importComponent`: the token theme. One collection per package and version, a mode per
//! theme, a variable per value the imported layers read, and the layers bound to them. The service
//! is a scripted backend; the snapshots are synthetic buttons.

use std::sync::{Arc, Mutex};

use figcraft_color::{Color, Paint, PaintKind};
use figcraft_doc::{Collection, Document, Node, NodeId, Variable, VariableValue};
use serde_json::{Value, json};

use crate::capture::{CaptureBackend, CaptureErrorKind, CaptureJob, CaptureOutcome, JobId};
use crate::lit_export::{TokenOptions, tokens_css};
use crate::params::id_of;
use crate::uigen_import_support::{Bench, IMPORT, PACKAGE, Scenario, button_inspect, service_failure, synthetic_source};

const COLLECTION: &str = "synthetic-ui 1.0.0";
const NOTE: &str = "UIGEN import:";

const PRIMARY: &str = "rgb(25, 118, 210)";
const PRIMARY_DARK: &str = "rgb(144, 202, 249)";
const REPLACEMENT: &str = "rgb(200, 30, 30)";
const WHITE: &str = "rgb(255, 255, 255)";
const NEAR_BLACK: &str = "rgb(18, 18, 18)";
const BORDER: &str = "rgb(13, 71, 161)";
const BORDER_DARK: &str = "rgb(100, 181, 246)";
const ICON: &str = "rgb(238, 238, 238)";
const ICON_DARK: &str = "rgb(66, 66, 66)";

const FILL: [&str; 4] = ["fill", "fills", "fills/0", "fills/0/color"];
const STROKE: [&str; 3] = ["stroke", "strokes", "strokes/0/color"];
const RADII: [&str; 4] = ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"];
const PADDING: [&str; 4] = ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"];

struct Look {
    primary: &'static str,
    icon_in_dark: bool,
    expose: bool,
}

fn look() -> Look {
    Look { primary: PRIMARY, icon_in_dark: true, expose: false }
}

fn button(look: &Look, style: &str, size: &str, theme: &str) -> Value {
    let dark = theme == "dark";
    let solid = style == "solid";
    let fill = match (solid, dark) {
        (true, false) => look.primary,
        (true, true) => PRIMARY_DARK,
        (false, false) => WHITE,
        (false, true) => NEAR_BLACK,
    };
    let (ink, border) = if dark { (NEAR_BLACK, BORDER_DARK) } else { (WHITE, BORDER) };
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
    let vars = if look.expose && solid { json!({"backgroundColor": "--syn-primary"}) } else { json!({}) };
    let icon_color = if dark { ICON_DARK } else { ICON };
    let text_width = width - 56.0;
    let mut children = Vec::new();
    if !dark || look.icon_in_dark {
        children.push(json!({
            "type": "element", "tag": "span", "label": "Icon",
            "box": {"x": 36, "y": 28, "width": 16, "height": 16},
            "style": {"backgroundColor": icon_color}
        }));
    }
    children.push(json!({
        "type": "text", "text": "Go",
        "box": {"x": 60, "y": 28, "width": text_width, "height": 20},
        "lineBox": {"x": 60, "y": 28, "width": text_width, "height": 20},
        "style": {"color": ink, "fontSize": "14px"}
    }));
    json!({
        "version": 1,
        "viewport": {"width": 400, "height": 200, "deviceScaleFactor": 1},
        "root": {
            "type": "element", "tag": "button", "label": "Button", "className": "syn-button",
            "box": {"x": 20, "y": 20, "width": width, "height": 36},
            "style": css, "vars": vars, "children": children
        }
    })
}

struct Backend {
    look: Arc<Mutex<Look>>,
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
                let snapshot = button(&self.look.lock().unwrap(), style, size, theme);
                Ok(json!({"snapshot": snapshot, "png": ".uigen/captures/synthetic.png"}))
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

fn bench(tag: &str, look: Look) -> (Bench, Arc<Mutex<Look>>) {
    let look = Arc::new(Mutex::new(look));
    let mut bench = Bench::without_backend(tag, &[synthetic_source(false)]);
    bench.session.set_capture_backend(Box::new(Backend { look: Arc::clone(&look), submitted: 0, ready: Vec::new() }));
    (bench, look)
}

fn params(themes: &[&str]) -> Value {
    json!({"package": PACKAGE, "component": "Button", "states": ["Default"], "themes": themes})
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

fn doc(bench: &Bench) -> &Document {
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

fn variant(doc: &Document, set: NodeId, name: &str) -> NodeId {
    doc.node(set).unwrap().children.iter().copied().find(|id| doc.node(*id).unwrap().name == name).unwrap_or_else(|| panic!("no variant {name:?}"))
}

fn first_layer(doc: &Document, variant: NodeId, wanted: impl Fn(&Node) -> bool) -> &Node {
    subtree(doc, variant)
        .into_iter()
        .map(|id| doc.node(id).unwrap())
        .find(|node| wanted(node))
        .unwrap_or_else(|| panic!("no such layer in the variant"))
}

fn layer_named<'a>(doc: &'a Document, variant: NodeId, name: &str) -> &'a Node {
    first_layer(doc, variant, |node| node.name == name)
}

fn has_solid(paints: &[Paint]) -> bool {
    paints.iter().any(|paint| paint.visible && matches!(paint.kind, PaintKind::Solid { .. }))
}

fn background_of(doc: &Document, variant: NodeId) -> &Node {
    first_layer(doc, variant, |node| has_solid(&node.fills))
}

fn rounded_layer(doc: &Document, variant: NodeId) -> &Node {
    first_layer(doc, variant, |node| node.corners.radii.iter().any(|radius| *radius > 0.0))
}

fn bound_variable<'a>(doc: &'a Document, node: &Node, keys: &[&str]) -> Option<&'a Variable> {
    let id = keys.iter().find_map(|key| node.bound.get(*key))?;
    doc.variables.get(id)
}

fn must_bind<'a>(doc: &'a Document, node: &Node, keys: &[&str]) -> &'a Variable {
    bound_variable(doc, node, keys).unwrap_or_else(|| panic!("{:?} is not bound through {keys:?}: {:?}", node.name, node.bound))
}

fn collection<'a>(doc: &'a Document, name: &str) -> &'a Collection {
    doc.variables.collections.iter().find(|collection| collection.name == name).unwrap_or_else(|| panic!("no collection {name:?}"))
}

fn collection_count(doc: &Document, name: &str) -> usize {
    doc.variables.collections.iter().filter(|collection| collection.name == name).count()
}

fn mode_names(doc: &Document, name: &str) -> Vec<String> {
    collection(doc, name).modes.iter().map(|mode| mode.name.clone()).collect()
}

fn variables_of<'a>(doc: &'a Document, name: &str) -> Vec<&'a Variable> {
    let id = &collection(doc, name).id;
    doc.variables.variables.iter().filter(|variable| variable.collection == *id).collect()
}

fn named<'a>(doc: &'a Document, name: &str) -> &'a Variable {
    variables_of(doc, COLLECTION)
        .into_iter()
        .find(|variable| variable.name == name)
        .unwrap_or_else(|| panic!("no variable {name:?} among {:?}", names(doc)))
}

fn names(doc: &Document) -> Vec<String> {
    variables_of(doc, COLLECTION).into_iter().map(|variable| variable.name.clone()).collect()
}

fn count_named(doc: &Document, name: &str) -> usize {
    names(doc).iter().filter(|candidate| candidate.as_str() == name).count()
}

fn value_in<'a>(doc: &'a Document, variable: &'a Variable, mode: &str) -> &'a VariableValue {
    let owner = doc.variables.collection(&variable.collection).unwrap();
    let mode = owner.modes.iter().find(|candidate| candidate.name == mode).unwrap_or_else(|| panic!("no mode {mode:?}"));
    variable.values.get(&mode.id).unwrap_or_else(|| panic!("{} has no value in {}", variable.name, mode.name))
}

fn hex_in(doc: &Document, variable: &Variable, mode: &str) -> String {
    match value_in(doc, variable, mode) {
        VariableValue::Color(color) => color.to_hex(),
        other => panic!("{} is {other:?} in {mode}", variable.name),
    }
}

fn number_in(doc: &Document, variable: &Variable, mode: &str) -> f64 {
    match value_in(doc, variable, mode) {
        VariableValue::Number(number) => *number,
        other => panic!("{} is {other:?} in {mode}", variable.name),
    }
}

fn css(value: &str) -> String {
    Color::parse_css(value).unwrap().to_hex()
}

fn notes_in(doc: &Document, root: NodeId) -> usize {
    subtree(doc, root).into_iter().map(|id| doc.node(id).unwrap().annotations.iter().filter(|note| note.label.starts_with(NOTE)).count()).sum()
}

fn notes_on(node: &Node) -> usize {
    node.annotations.iter().filter(|note| note.label.starts_with(NOTE)).count()
}

const SOLID_SM: &str = "Style=solid, Size=sm, State=Default";

// covers: E3-6.1
#[test]
fn the_collection_has_a_light_and_a_dark_mode_and_the_background_variable_holds_both_renderings() {
    let mut bench = Bench::new("theme-modes", &[synthetic_source(false)], Scenario::new(button_inspect(true, false)));
    let result = import_ok(&mut bench, &params(&["light", "dark"]));

    let doc = doc(&bench);
    assert_eq!(collection_count(doc, COLLECTION), 1);
    assert_eq!(mode_names(doc, COLLECTION), ["Light", "Dark"]);

    let solid = variant(doc, set_of(&result), SOLID_SM);
    let background = must_bind(doc, background_of(doc, solid), &FILL);
    assert_eq!(background.collection, collection(doc, COLLECTION).id);
    assert_eq!(hex_in(doc, background, "Light"), css(PRIMARY));
    assert_eq!(hex_in(doc, background, "Dark"), css(PRIMARY_DARK));
}

// covers: E3-6.2
#[test]
fn every_colour_radius_and_padding_of_the_imported_layers_is_bound_to_a_variable() {
    let (mut bench, _) = bench("all-bound", look());
    let result = import_ok(&mut bench, &params(&["light", "dark"]));

    let doc = doc(&bench);
    let set = set_of(&result);
    let (mut fills, mut strokes, mut radii, mut paddings) = (0, 0, 0, 0);
    for variant in doc.node(set).unwrap().children.clone() {
        for id in subtree(doc, variant) {
            let node = doc.node(id).unwrap();
            if has_solid(&node.fills) {
                must_bind(doc, node, &FILL);
                fills += 1;
            }
            if has_solid(&node.strokes) {
                must_bind(doc, node, &STROKE);
                strokes += 1;
            }
            for (radius, field) in node.corners.radii.iter().zip(RADII) {
                if *radius > 0.0 {
                    must_bind(doc, node, &[field]);
                    radii += 1;
                }
            }
            for (padding, field) in node.layout.iter().flat_map(|layout| layout.padding).zip(PADDING) {
                if padding > 0.0 {
                    must_bind(doc, node, &[field]);
                    paddings += 1;
                }
            }
        }
    }
    assert!(fills >= 12, "background, icon and text of four variants: {fills}");
    assert!(strokes >= 4, "{strokes}");
    assert!(radii >= 16, "{radii}");
    assert!(paddings >= 16, "{paddings}");

    let solid = variant(doc, set, SOLID_SM);
    let rounded = rounded_layer(doc, solid);
    let radius = must_bind(doc, rounded, &["topLeftRadius"]);
    assert_eq!((number_in(doc, radius, "Light"), number_in(doc, radius, "Dark")), (6.0, 6.0));
    let top = must_bind(doc, rounded, &["paddingTop"]);
    let left = must_bind(doc, rounded, &["paddingLeft"]);
    assert_eq!(number_in(doc, top, "Light"), 8.0);
    assert_eq!(number_in(doc, left, "Dark"), 16.0);
    let border = must_bind(doc, rounded, &STROKE);
    assert_eq!((hex_in(doc, border, "Light"), hex_in(doc, border, "Dark")), (css(BORDER), css(BORDER_DARK)));
}

// covers: E3-6.3
#[test]
fn a_variable_named_by_role_is_suffixed_by_the_axes_that_change_its_value_and_equal_ones_are_shared() {
    let (mut bench, _) = bench("role-names", look());
    let result = import_ok(&mut bench, &params(&["light", "dark"]));

    let doc = doc(&bench);
    for (name, light, dark) in [
        ("couleur/button/fond/solid", PRIMARY, PRIMARY_DARK),
        ("couleur/button/fond/outline", WHITE, NEAR_BLACK),
        ("couleur/button/label/texte", WHITE, NEAR_BLACK),
        ("couleur/button/bordure", BORDER, BORDER_DARK),
    ] {
        let variable = named(doc, name);
        assert_eq!((hex_in(doc, variable, "Light"), hex_in(doc, variable, "Dark")), (css(light), css(dark)), "{name}");
    }
    let mut backgrounds: Vec<String> = names(doc).into_iter().filter(|name| name.starts_with("couleur/button/fond")).collect();
    backgrounds.sort();
    assert_eq!(backgrounds, ["couleur/button/fond/outline", "couleur/button/fond/solid"], "neither size nor theme suffixes a name");

    let shared = named(doc, "rayon/button/rayon");
    for name in ["rayon/button/rayon", "couleur/button/bordure", "couleur/button/label/texte"] {
        assert_eq!(count_named(doc, name), 1, "{name}: the same name and values make one variable");
    }
    let set = set_of(&result);
    assert_eq!(doc.node(set).unwrap().children.len(), 4);
    for variant in doc.node(set).unwrap().children.clone() {
        let rounded = rounded_layer(doc, variant);
        for field in RADII {
            assert_eq!(must_bind(doc, rounded, &[field]).id, shared.id, "{field} of {}", doc.node(variant).unwrap().name);
        }
    }
}

// covers: E3-6.3
#[test]
fn an_exposed_custom_property_names_the_variable_after_it() {
    let (mut bench, _) = bench("exposed-name", Look { expose: true, ..look() });
    let result = import_ok(&mut bench, &params(&["light", "dark"]));

    let doc = doc(&bench);
    let primary = named(doc, "syn-primary");
    assert_eq!((hex_in(doc, primary, "Light"), hex_in(doc, primary, "Dark")), (css(PRIMARY), css(PRIMARY_DARK)));
    assert_eq!(count_named(doc, "syn-primary"), 1);
    assert!(!names(doc).iter().any(|name| name == "couleur/button/fond/solid"), "{:?}", names(doc));

    let set = set_of(&result);
    for variant_name in [SOLID_SM, "Style=solid, Size=md, State=Default"] {
        let background = must_bind(doc, background_of(doc, variant(doc, set, variant_name)), &FILL);
        assert_eq!(background.id, primary.id, "{variant_name}");
    }
    let outline = must_bind(doc, background_of(doc, variant(doc, set, "Style=outline, Size=sm, State=Default")), &FILL);
    assert_ne!(outline.id, primary.id);
    assert_eq!(hex_in(doc, outline, "Light"), css(WHITE));
}

// covers: E3-6.4
#[test]
fn a_layer_missing_from_the_dark_rendering_keeps_its_light_value_in_both_modes_and_gets_a_note() {
    let (mut kept, _) = bench("dark-keeps", look());
    let kept_result = import_ok(&mut kept, &params(&["light", "dark"]));
    let (mut missing, _) = bench("dark-misses", Look { icon_in_dark: false, ..look() });
    let missing_result = import_ok(&mut missing, &params(&["light", "dark"]));

    let (kept_doc, missing_doc) = (doc(&kept), doc(&missing));
    let kept_icon = layer_named(kept_doc, variant(kept_doc, set_of(&kept_result), SOLID_SM), "Icon");
    let kept_var = must_bind(kept_doc, kept_icon, &FILL);
    assert_eq!((hex_in(kept_doc, kept_var, "Light"), hex_in(kept_doc, kept_var, "Dark")), (css(ICON), css(ICON_DARK)));

    let icon = layer_named(missing_doc, variant(missing_doc, set_of(&missing_result), SOLID_SM), "Icon");
    let icon_var = must_bind(missing_doc, icon, &FILL);
    assert_eq!((hex_in(missing_doc, icon_var, "Light"), hex_in(missing_doc, icon_var, "Dark")), (css(ICON), css(ICON)));
    assert!(notes_on(icon) > notes_on(kept_icon), "the layer is missing from the dark rendering: {:?}", icon.annotations);
    assert!(icon.annotations.iter().all(|note| note.category.is_none()));

    let background = must_bind(missing_doc, background_of(missing_doc, variant(missing_doc, set_of(&missing_result), SOLID_SM)), &FILL);
    assert_ne!(hex_in(missing_doc, background, "Light"), hex_in(missing_doc, background, "Dark"), "what the dark rendering has stays in Dark");
}

// covers: E3-6.4
#[test]
fn importing_the_same_package_version_again_reuses_the_collection_and_never_changes_a_value() {
    let (mut bench, look) = bench("reimport", look());
    let first = import_ok(&mut bench, &params(&["light", "dark"]));
    let before: Vec<Variable> = variables_of(doc(&bench), COLLECTION).into_iter().cloned().collect();
    assert!(!before.is_empty());

    let second = import_ok(&mut bench, &params(&["light", "dark"]));
    let doc_after = doc(&bench);
    assert_eq!(collection_count(doc_after, COLLECTION), 1);
    let after: Vec<Variable> = variables_of(doc_after, COLLECTION).into_iter().cloned().collect();
    assert_eq!(after, before, "nothing changes, nothing is added, nothing is duplicated");
    assert_ne!(set_of(&first), set_of(&second));
    let quiet = notes_in(doc_after, set_of(&second));

    look.lock().unwrap().primary = REPLACEMENT;
    let third = import_ok(&mut bench, &params(&["light", "dark"]));
    let doc_end = doc(&bench);
    assert_eq!(collection_count(doc_end, COLLECTION), 1);
    for old in &before {
        let current =
            variables_of(doc_end, COLLECTION).into_iter().find(|variable| variable.id == old.id).unwrap_or_else(|| panic!("{} is gone", old.name));
        assert_eq!(current.values, old.values, "{} keeps its value", old.name);
    }
    let mut seen = names(doc_end);
    let total = seen.len();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), total, "a name is used once");
    let solid = named(doc_end, "couleur/button/fond/solid");
    assert_eq!(hex_in(doc_end, solid, "Light"), css(PRIMARY));
    assert!(notes_in(doc_end, set_of(&third)) > quiet, "the layer that differs from the variable says so");
}

// covers: E3-6.5
#[test]
fn a_light_only_import_gives_a_light_only_collection_and_the_lit_export_cites_its_variables() {
    let (mut bench, _) = bench("light-only", look());
    let result = import_ok(&mut bench, &params(&["light"]));

    let doc = doc(&bench);
    assert_eq!(mode_names(doc, COLLECTION), ["Light"]);
    let variables = variables_of(doc, COLLECTION);
    assert!(!variables.is_empty());
    assert!(variables.iter().all(|variable| variable.values.len() == 1), "{variables:?}");

    let options = TokenOptions { prefix: "ui".to_string(), light: None, dark: None };
    let declared = tokens_css(doc, &[collection(doc, COLLECTION)], &options).declared;
    assert!(!declared.is_empty());

    let code = bench.session.execute("inspect.componentCode", &json!({"id": set_of(&result), "language": "lit"})).unwrap();
    let definition = code["definition"].as_str().unwrap_or_default();
    let cited: Vec<String> = definition
        .match_indices("var(")
        .filter_map(|(at, _)| definition.get(at + 4..))
        .map(|rest| rest.split([')', ',']).next().unwrap_or_default().trim().to_string())
        .collect();
    assert!(!cited.is_empty(), "the component reads its colours from custom properties:\n{definition}");
    for name in &cited {
        assert!(declared.contains(name), "{name} is not declared by tokens.css: {declared:?}");
    }
    assert!(!definition.to_lowercase().contains("1976d2"), "the raw colour stays out of the component:\n{definition}");
}
