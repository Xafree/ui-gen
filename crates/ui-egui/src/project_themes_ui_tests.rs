//! File ▸ Project settings…, "Token themes" section under "Source libraries": one row per variable
//! collection of the library project (name, origin, modes, variable count, layers that follow it,
//! up to 8 swatches, "Applied" on the applied one) read from `uigen.themes`; "Apply" runs
//! `uigen.applyTheme` once with a toast that undoes it. The library is filled by a real staged
//! import of a synthetic button against a scripted capture backend that an import can be held on.
//!
//! The catalogs of the 22 languages are checked here for the new texts; the section's absence in the
//! browser build is a `cfg!(target_arch = "wasm32")` condition that a native test cannot flip.
//! The names of the tokens without equivalent, shown on hover or in a collapsible list, are not read.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use egui::accesskit::{Node, Role};
use egui::{Event, Key, Modifiers, Pos2, pos2};
use figcraft_doc::{NodeId, VariableType};
use figcraft_engine::capture::{CaptureBackend, CaptureError, CaptureJob, JobId};
use serde_json::{Value, json};

use crate::a11y::focused_name;
use crate::i18n;
use crate::layers_drag_tests::Ui;
use crate::live_tests::{flat, texts};

static NEXT: AtomicUsize = AtomicUsize::new(0);

const PACKAGE: &str = "synthetic-ui";
const VERSION: &str = "1.0.0";
const COLLECTION: &str = "synthetic-ui 1.0.0";
const BRAND: &str = "Brand";
const ALIEN: &str = "Alien";
const BORDER: &str = "couleur/button/bordure";
const RADIUS: &str = "rayon/button/rayon";
const APPLY: &str = "uigen.applyTheme";
const MAX_FRAMES: usize = 300;
const MAX_TABS: usize = 80;

const TITLE: &str = "Token themes";
const EMPTY: &str = "No token theme yet. Import a component or tokens to create one.";
const OPEN_VARIABLES: &str = "Open the Variables window";
const IMPORT_RUNNING: &str = "an import is running";
const OUT_OF_SYNC_TAIL: &str = ", not the applied theme. Apply it again to sync.";

const NEW_TEXTS: [&str; 11] = [
    "Token themes",
    "No token theme yet. Import a component or tokens to create one.",
    "Applied",
    "Apply",
    "{count} layers use it",
    "Imported from {package} {version}",
    "{theme} applied: {layers} layers follow it",
    "{count} tokens have no equivalent in {theme} and keep their previous value",
    "Couldn't apply {theme}: {cause}",
    "The layers follow {theme}, not the applied theme. Apply it again to sync.",
    "Open the Variables window",
];

type Outcome = Result<Value, CaptureError>;
type Painted = (String, Pos2, egui::Vec2);

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-themes-ui-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn synthetic_button(style: &str, size: &str, theme: &str) -> Value {
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

fn inspected() -> Value {
    let props = json!([
        {"name": "variant", "type": "enum", "values": ["solid", "outline"]},
        {"name": "size", "type": "enum", "values": ["sm", "md"]}
    ]);
    json!({
        "package": PACKAGE,
        "version": VERSION,
        "framework": "react",
        "components": [{"name": "Button", "props": props}],
        "notes": []
    })
}

struct Gate {
    hold: Arc<AtomicBool>,
    submitted: u64,
    ready: Vec<(JobId, Outcome)>,
}

impl CaptureBackend for Gate {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let outcome = match &job {
            CaptureJob::Inspect { .. } => Ok(inspected()),
            CaptureJob::Render { props, theme, .. } => {
                let style = props.get("variant").and_then(Value::as_str).unwrap_or("solid");
                let size = props.get("size").and_then(Value::as_str).unwrap_or("md");
                Ok(json!({"snapshot": synthetic_button(style, size, theme), "png": ".uigen/captures/synthetic.png"}))
            }
            CaptureJob::Install { package, version, .. } => {
                Ok(json!({"package": package, "version": version, "framework": "react", "license": "MIT"}))
            }
            CaptureJob::Uninstall { .. } => Ok(json!({})),
        };
        self.submitted += 1;
        self.ready.push((self.submitted, outcome));
        self.submitted
    }

    fn poll(&mut self) -> Vec<(JobId, Outcome)> {
        if self.hold.load(Ordering::SeqCst) { Vec::new() } else { std::mem::take(&mut self.ready) }
    }

    fn shutdown(&mut self) {}
}

fn kind_name(kind: VariableType) -> &'static str {
    match kind {
        VariableType::Color => "color",
        VariableType::Number => "number",
        VariableType::String => "string",
        VariableType::Boolean => "boolean",
    }
}

fn mirror_value(kind: VariableType, index: usize) -> Value {
    match kind {
        VariableType::Color => json!(format!("#{:02X}{:02X}{:02X}", 20 + index % 200, 60 + index % 150, 100 + index % 100)),
        VariableType::Number => json!(99),
        VariableType::String => json!("x"),
        VariableType::Boolean => json!(true),
    }
}

fn centre(node: &Node) -> Option<Pos2> {
    let b = node.bounds()?;
    Some(pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32))
}

fn count_of(shown: &[String], text: &str) -> usize {
    shown.iter().filter(|t| t.trim() == text).count()
}

fn mentions(shown: &[String], needle: &str) -> bool {
    shown.iter().any(|t| t.contains(needle))
}

fn y_of(painted: &[Painted], text: &str) -> f32 {
    painted.iter().rev().find(|(t, _, _)| t.trim() == text).map(|(_, p, _)| p.y).unwrap_or_else(|| panic!("no text {text:?} on screen"))
}

fn is_apply_label(label: &str) -> bool {
    let label = label.trim();
    label == "Apply"
}

struct Lab {
    ui: Ui,
    hold: Arc<AtomicBool>,
    _dir: Dir,
}

impl Lab {
    fn new(tag: &str) -> Self {
        figcraft_text::scan_system_fonts();
        let dir = Dir::new(tag);
        let source = json!({"package": PACKAGE, "version": VERSION, "framework": "react", "license": "MIT", "removed": false});
        let manifest = json!({"version": 1, "name": "mylib", "designFiles": ["lib.figcraft"], "sources": [source]});
        std::fs::write(dir.0.join("uigen.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
        let mut doc = figcraft_doc::Document::new("lib");
        let page = doc.pages[0];
        doc.node_mut(page).unwrap().name = "Card".to_string();
        std::fs::write(dir.0.join("lib.figcraft"), figcraft_format::save_bytes(&doc, None).unwrap()).unwrap();
        let mut ui = Ui::new(figcraft_engine::Session::empty());
        ui.app.session.prefs.language = "en".into();
        assert!(ui.app.open_project_at(dir.0.clone(), false), "the project opens");
        let hold = Arc::new(AtomicBool::new(false));
        ui.app.session.set_capture_backend(Box::new(Gate { hold: Arc::clone(&hold), submitted: 0, ready: Vec::new() }));
        ui.app.ui.home = false;
        ui.app.ui.toast = None;
        let mut lab = Self { ui, hold, _dir: dir };
        lab.settle();
        lab
    }

    fn with_library(tag: &str) -> Self {
        let mut lab = Self::new(tag);
        lab.start_import();
        lab.finish_import();
        let set = lab.set().expect("the button set was imported");
        let page = lab.ui.app.session.doc().unwrap().pages[0];
        lab.run("uigen.placeImported", json!({"set": set.0, "page": page.0}));
        lab.settle();
        lab
    }

    fn with_brand(tag: &str, skip: &[&str]) -> Self {
        let mut lab = Self::with_library(tag);
        lab.add_brand(skip);
        lab.settle();
        lab
    }

    fn settle(&mut self) {
        for _ in 0..3 {
            self.ui.frame(vec![]);
        }
    }

    fn run(&mut self, id: &str, params: Value) -> Value {
        self.ui.app.session.execute(id, &params).unwrap_or_else(|e| panic!("{id}: {e}"))
    }

    fn start_import(&mut self) {
        let params = json!({"package": PACKAGE, "component": "Button", "states": ["Default"], "themes": ["light", "dark"], "stage": true});
        assert!(self.ui.app.run("uigen.importComponent", params).is_some(), "the import starts");
    }

    fn finish_import(&mut self) {
        self.hold.store(false, Ordering::SeqCst);
        for _ in 0..MAX_FRAMES {
            self.ui.app.session.pump_capture();
            self.ui.frame(vec![]);
            if self.set().is_some() {
                self.settle();
                return;
            }
        }
        panic!("the import never reached its import page");
    }

    fn set(&mut self) -> Option<NodeId> {
        let sections = self.run("uigen.imported", json!({}));
        let section = sections["sections"].as_array()?.iter().find(|s| s["package"] == PACKAGE && s["version"] == VERSION)?;
        let found = section["components"].as_array()?.iter().find(|c| c["name"] == "Button")?;
        found["set"].as_u64().map(NodeId)
    }

    fn add_brand(&mut self, skip: &[&str]) {
        let doc = self.ui.app.session.doc().unwrap();
        let imported = doc.variables.collections.iter().find(|c| c.name == COLLECTION).map(|c| c.id.clone()).expect("the imported collection");
        let mirrored: Vec<(String, VariableType)> =
            doc.variables.variables.iter().filter(|v| v.collection == imported).map(|v| (v.name.clone(), v.kind)).collect();
        self.run("variable.createCollection", json!({"name": BRAND, "modes": ["Light", "Dark"]}));
        for (index, (name, kind)) in mirrored.into_iter().enumerate().filter(|(_, (name, _))| !skip.contains(&name.as_str())) {
            let values = json!({"Light": mirror_value(kind, index), "Dark": mirror_value(kind, index + 1)});
            self.run("variable.create", json!({"collection": BRAND, "name": name, "type": kind_name(kind), "values": values}));
        }
    }

    fn add_alien(&mut self) {
        self.run("variable.createCollection", json!({"name": ALIEN}));
        let variable = json!({"collection": ALIEN, "name": "nothing like the imported names", "type": "color", "value": "#123456"});
        self.run("variable.create", variable);
        self.settle();
    }

    fn themes(&mut self) -> Value {
        self.run("uigen.themes", json!({}))
    }

    fn open_window(&mut self) {
        assert!(self.ui.app.run("project.settings", json!({})).is_some());
        self.settle();
    }

    fn painted(&mut self) -> Vec<Painted> {
        self.ui.frame(vec![]);
        let out = self.ui.frame_output(vec![]);
        texts(&flat(&out.shapes))
    }

    fn shown(&mut self) -> Vec<String> {
        self.painted().into_iter().map(|(t, _, _)| t).collect()
    }

    fn nodes(&mut self) -> Vec<Node> {
        self.ui.ctx.enable_accesskit();
        self.ui.frame(vec![]);
        self.ui.frame_output(vec![]).platform_output.accesskit_update.expect("an AccessKit update").nodes.into_iter().map(|(_, n)| n).collect()
    }

    fn centre_of(&mut self, role: Role, label: &str) -> Pos2 {
        let nodes = self.nodes();
        nodes
            .iter()
            .filter(|n| n.role() == role && n.label().is_some_and(|l| l.trim() == label))
            .find_map(centre)
            .unwrap_or_else(|| panic!("no {role:?} {label:?} on screen"))
    }

    fn apply_buttons(&mut self) -> Vec<Pos2> {
        let mut found: Vec<Pos2> =
            self.nodes().iter().filter(|n| n.role() == Role::Button && n.label().is_some_and(is_apply_label)).filter_map(centre).collect();
        found.sort_by(|a, b| a.y.total_cmp(&b.y));
        found
    }

    fn color_wells(&mut self) -> Vec<Node> {
        self.nodes().into_iter().filter(|n| n.role() == Role::ColorWell).collect()
    }

    fn click(&mut self, at: Pos2) {
        self.ui.button(at, true);
        self.ui.button(at, false);
        self.settle();
    }

    fn click_apply(&mut self, row: usize) {
        let buttons = self.apply_buttons();
        let at = *buttons.get(row).unwrap_or_else(|| panic!("no Apply button at row {row}: {buttons:?}"));
        self.click(at);
    }

    fn press(&mut self, key: Key, modifiers: Modifiers) {
        let events = [true, false].map(|pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers }).to_vec();
        self.ui.frame_with(events, modifiers);
        self.settle();
    }

    fn mark(&self) -> usize {
        self.ui.app.session.journal.len()
    }

    fn ran_since(&self, mark: usize, command: &str) -> Vec<Value> {
        self.ui.app.session.journal[mark..].iter().filter(|(c, _)| c == command).map(|(_, p)| p.clone()).collect()
    }

    fn bindings(&self) -> Vec<String> {
        let doc = self.ui.app.session.doc().unwrap();
        doc.pages
            .iter()
            .flat_map(|page| doc.subtree(*page))
            .filter_map(|id| doc.node(id))
            .map(|n| serde_json::to_string(&n.bound).unwrap_or_default())
            .collect()
    }

    fn applied_in_manifest(&mut self) -> Value {
        self.run("project.info", json!({}))["project"]["appliedTheme"].clone()
    }
}

fn nearest_row<'a>(painted: &[Painted], text: &str, rows: &[&'a str]) -> &'a str {
    let at = y_of(painted, text);
    rows.iter().min_by(|a, b| (y_of(painted, a) - at).abs().total_cmp(&(y_of(painted, b) - at).abs())).copied().expect("a row")
}

// covers: E5-6.2
#[test]
fn without_a_collection_the_section_says_so_and_the_button_opens_the_variables_window() {
    let mut lab = Lab::new("empty");
    assert_eq!(lab.themes()["themes"], json!([]));
    lab.open_window();

    let shown = lab.shown();
    assert!(mentions(&shown, TITLE), "{shown:?}");
    assert!(mentions(&shown, EMPTY), "{shown:?}");
    assert!(lab.apply_buttons().is_empty(), "nothing to apply");
    assert!(lab.ui.app.ui.variables.is_none());

    let at = lab.centre_of(Role::Button, OPEN_VARIABLES);
    lab.click(at);
    assert!(lab.ui.app.ui.variables.is_some(), "the button opens the Variables window");
}

// covers: E5-6.1
#[test]
fn the_section_sits_under_source_libraries_with_one_row_per_collection_and_its_facts() {
    let mut lab = Lab::with_brand("rows", &[]);
    let themes = lab.themes();
    let imported = themes["themes"][0].clone();
    assert_eq!(imported["name"], COLLECTION);
    let followed = imported["boundLayers"].as_u64().unwrap_or(0);
    assert!(followed >= 2, "the placed copy follows the imported collection: {themes}");
    lab.open_window();

    let painted = lab.painted();
    let shown: Vec<String> = painted.iter().map(|(t, _, _)| t.clone()).collect();
    assert!(y_of(&painted, TITLE) > y_of(&painted, "Source libraries"), "the section is under the sources");
    assert!(y_of(&painted, COLLECTION) < y_of(&painted, BRAND), "rows keep the order of the file");
    assert_eq!(count_of(&shown, &format!("Imported from {PACKAGE} {VERSION}")), 1, "only the imported collection has an origin: {shown:?}");
    assert_eq!(count_of(&shown, &format!("{followed} layers use it")), 1, "{shown:?}");
    assert!(mentions(&shown, "Light") && mentions(&shown, "Dark"), "the modes are named: {shown:?}");
    let variables = imported["variables"].as_u64().unwrap_or(0).to_string();
    let counted = shown.iter().any(|t| t.split(|c: char| !c.is_alphanumeric()).any(|word| word == variables));
    assert!(counted, "the variable count {variables} is shown: {shown:?}");
    assert_eq!(count_of(&shown, "Applied"), 0, "no theme is applied yet");
    assert_eq!(lab.apply_buttons().len(), 2, "one Apply per row");
}

// covers: E5-6.1
// covers: E5-6.6
#[test]
fn each_row_shows_at_most_eight_swatches_described_by_name_and_value() {
    let mut lab = Lab::with_brand("swatches", &[]);
    let themes = lab.themes();
    let expected: Vec<Value> =
        themes["themes"].as_array().unwrap().iter().flat_map(|t| t["swatches"].as_array().cloned().unwrap_or_default()).collect();
    assert!(!expected.is_empty() && themes["themes"].as_array().unwrap().iter().all(|t| t["swatches"].as_array().unwrap().len() <= 8));
    let before = lab.color_wells().len();
    lab.open_window();

    let wells = lab.color_wells();
    assert_eq!(wells.len() - before, expected.len(), "one colour well per swatch");
    for swatch in &expected {
        let name = swatch["name"].as_str().unwrap();
        let hexes = [swatch["light"].as_str().unwrap().to_lowercase(), swatch["dark"].as_str().unwrap().to_lowercase()];
        let described = wells.iter().any(|n| {
            let text =
                format!("{} {} {}", n.label().unwrap_or_default(), n.value().unwrap_or_default(), n.description().unwrap_or_default()).to_lowercase();
            text.contains(&name.to_lowercase()) && hexes.iter().any(|hex| text.contains(hex))
        });
        assert!(described, "a swatch is described by {name:?} and its value");
    }
}

// covers: E5-6.2
// covers: E5-6.3
#[test]
fn apply_runs_the_command_once_with_the_row_name_and_the_toast_offers_undo() {
    let mut lab = Lab::with_brand("apply", &[]);
    let followed = lab.themes()["themes"][0]["boundLayers"].as_u64().unwrap();
    lab.open_window();
    let mark = lab.mark();

    lab.click_apply(1);

    assert_eq!(lab.ran_since(mark, APPLY), vec![json!({"theme": BRAND})], "once, with the name of the clicked row");
    assert!(lab.ui.app.ui.toast_action.is_some(), "the toast carries an action");
    let nodes = lab.nodes();
    assert_eq!(nodes.iter().filter(|n| n.role() == Role::Button && n.label() == Some("Undo")).count(), 1);
    assert_eq!(lab.applied_in_manifest(), json!(BRAND));

    let painted = lab.painted();
    let shown: Vec<String> = painted.iter().map(|(t, _, _)| t.clone()).collect();
    assert!(mentions(&shown, &format!("{BRAND} applied: {followed} layers follow it")), "{shown:?}");
    assert!(!mentions(&shown, "have no equivalent"), "every token has an equivalent: {shown:?}");
    assert_eq!(count_of(&shown, "Applied"), 1, "{shown:?}");
    assert_eq!(nearest_row(&painted, "Applied", &[COLLECTION, BRAND]), BRAND);
}

// covers: E5-6.3
#[test]
fn clicking_undo_restores_the_bindings_and_the_applied_row_does_not_move() {
    let mut lab = Lab::with_brand("undo", &[]);
    let before = lab.bindings();
    lab.open_window();
    lab.click_apply(1);
    assert_ne!(lab.bindings(), before, "the theme moved the bindings");

    let at = lab.centre_of(Role::Button, "Undo");
    lab.click(at);

    assert_eq!(lab.bindings(), before, "undo takes the bindings back");
    assert_eq!(lab.applied_in_manifest(), json!(BRAND), "the manifest is outside the history");
    let painted = lab.painted();
    let shown: Vec<String> = painted.iter().map(|(t, _, _)| t.clone()).collect();
    assert_eq!(count_of(&shown, "Applied"), 1, "{shown:?}");
    assert_eq!(nearest_row(&painted, "Applied", &[COLLECTION, BRAND]), BRAND, "the Applied badge stays on its row");
}

// covers: E5-6.2
#[test]
fn tokens_without_equivalent_are_counted_after_applying() {
    let mut lab = Lab::with_brand("unmatched", &[BORDER, RADIUS]);
    lab.open_window();

    lab.click_apply(1);

    let shown = lab.shown();
    assert!(mentions(&shown, &format!("2 tokens have no equivalent in {BRAND} and keep their previous value")), "{shown:?}");
    assert!(shown.iter().any(|t| t.starts_with(&format!("{BRAND} applied: ")) && t.ends_with(" layers follow it")), "{shown:?}");
}

// covers: E5-6.2
#[test]
fn a_refused_apply_says_why_and_leaves_the_list_and_the_bindings_intact() {
    let mut lab = Lab::with_brand("refused", &[]);
    lab.add_alien();
    lab.open_window();
    let before = lab.bindings();
    assert_eq!(lab.apply_buttons().len(), 3);

    lab.click_apply(2);

    let shown = lab.shown();
    let failure =
        shown.iter().find(|t| t.starts_with(&format!("Couldn't apply {ALIEN}: "))).unwrap_or_else(|| panic!("no failure text in {shown:?}"));
    assert!(failure.contains("nothing to apply"), "the cause is the engine's: {failure:?}");
    for name in [COLLECTION, BRAND, ALIEN] {
        assert!(count_of(&shown, name) >= 1, "the row {name:?} stays: {shown:?}");
    }
    assert_eq!(lab.apply_buttons().len(), 3, "the list is intact");
    assert_eq!(lab.bindings(), before);
    assert!(lab.ui.app.ui.toast_action.is_none(), "nothing to undo");
}

// covers: E5-6.4
#[test]
fn the_section_warns_when_the_layers_follow_another_collection_and_stays_silent_when_in_sync() {
    let mut lab = Lab::with_brand("sync", &[]);
    lab.run(APPLY, json!({"theme": BRAND}));
    lab.open_window();
    let shown = lab.shown();
    assert!(!mentions(&shown, "The layers follow"), "in sync: nothing is said: {shown:?}");

    lab.run("edit.undo", json!({}));
    lab.settle();

    let shown = lab.shown();
    let expected = format!("The layers follow {COLLECTION}{OUT_OF_SYNC_TAIL}");
    assert_eq!(count_of(&shown, &expected), 1, "{shown:?}");
}

// covers: E5-6.2
// covers: E5-6.1
#[test]
fn during_an_import_apply_is_disabled_with_its_reason_and_the_section_follows_the_import_to_its_end() {
    let mut lab = Lab::new("loading");
    lab.run("variable.createCollection", json!({"name": BRAND}));
    lab.run("variable.create", json!({"collection": BRAND, "name": "ink", "type": "color", "value": "#112233"}));
    lab.hold.store(true, Ordering::SeqCst);
    lab.start_import();
    lab.open_window();
    let mark = lab.mark();
    assert_eq!(lab.apply_buttons().len(), 1);

    let reasoned = {
        let shown = lab.shown().join(" ").to_lowercase();
        let described = lab
            .nodes()
            .iter()
            .any(|n| format!("{} {}", n.label().unwrap_or_default(), n.description().unwrap_or_default()).to_lowercase().contains(IMPORT_RUNNING));
        shown.contains(IMPORT_RUNNING) || described
    };
    assert!(reasoned, "the reason is given");
    lab.click_apply(0);
    assert!(lab.ran_since(mark, APPLY).is_empty(), "a disabled Apply runs nothing");
    assert!(!mentions(&lab.shown(), "Imported from"), "the import has not ended");

    lab.finish_import();

    let shown = lab.shown();
    assert_eq!(count_of(&shown, &format!("Imported from {PACKAGE} {VERSION}")), 1, "the new collection appears: {shown:?}");
    assert_eq!(lab.apply_buttons().len(), 2);
    assert!(!shown.join(" ").to_lowercase().contains(IMPORT_RUNNING), "Apply is available again: {shown:?}");
}

// covers: E5-6.5
#[test]
fn outside_a_project_there_is_no_window_and_no_section() {
    figcraft_text::scan_system_fonts();
    let mut ui = Ui::new(figcraft_engine::Session::new());
    ui.app.session.prefs.language = "en".into();
    ui.app.ui.home = false;

    ui.app.run("project.settings", json!({}));
    for _ in 0..3 {
        ui.frame(vec![]);
    }

    assert!(ui.app.ui.project_settings.is_none());
    ui.frame(vec![]);
    let shown = crate::import_report_tests::shown_texts(ui.frame_output(vec![]).shapes);
    assert!(!mentions(&shown, TITLE), "{shown:?}");
}

// covers: E5-6.5
#[test]
fn a_programmatic_apply_opens_no_window() {
    let mut lab = Lab::with_brand("programmatic", &[]);
    lab.ui.app.session.ui_requests.clear();

    lab.run(APPLY, json!({"theme": BRAND}));
    let ctx = lab.ui.ctx.clone();
    crate::control::handle(&mut lab.ui.app, &ctx, "engine.execute", &json!({"command": APPLY, "params": {"theme": COLLECTION}})).unwrap();
    lab.settle();

    assert!(lab.ui.app.ui.project_settings.is_none());
    assert!(lab.ui.app.ui.variables.is_none());
    assert!(lab.ui.app.session.ui_requests.is_empty(), "{:?}", lab.ui.app.session.ui_requests);
}

// covers: E5-6.5
#[test]
fn escape_closes_the_window_with_the_section_in_it() {
    let mut lab = Lab::with_brand("escape", &[]);
    lab.open_window();
    assert!(mentions(&lab.shown(), TITLE));

    lab.press(Key::Escape, Modifiers::NONE);

    assert!(lab.ui.app.ui.project_settings.is_none());
}

fn tab_to(lab: &mut Lab, name: &str) -> usize {
    for step in 1..=MAX_TABS {
        lab.press(Key::Tab, Modifiers::NONE);
        if focused_name(&lab.ui.ctx) == name {
            return step;
        }
    }
    panic!("Tab never reached {name:?}");
}

// covers: E5-6.6
#[test]
fn tab_and_shift_tab_walk_to_apply_and_space_applies_the_row() {
    let mut lab = Lab::with_brand("keys-space", &[]);
    lab.open_window();
    let steps = tab_to(&mut lab, "Apply");
    assert!(steps >= 1);

    lab.press(Key::Tab, Modifiers::SHIFT);
    assert_ne!(focused_name(&lab.ui.ctx), "Apply", "Shift+Tab goes back");
    lab.press(Key::Tab, Modifiers::NONE);
    assert_eq!(focused_name(&lab.ui.ctx), "Apply", "Tab comes forward again");
    let mark = lab.mark();
    lab.press(Key::Space, Modifiers::NONE);

    assert_eq!(lab.ran_since(mark, APPLY), vec![json!({"theme": COLLECTION})], "the first Apply in tab order is the first row");
}

// covers: E5-6.6
#[test]
fn enter_on_the_focused_apply_applies_the_row() {
    let mut lab = Lab::with_brand("keys-enter", &[]);
    lab.open_window();
    tab_to(&mut lab, "Apply");
    let mark = lab.mark();

    lab.press(Key::Enter, Modifiers::NONE);

    assert_eq!(lab.ran_since(mark, APPLY), vec![json!({"theme": COLLECTION})]);
}

// covers: E5-6.6
#[test]
fn the_empty_state_button_is_reached_and_activated_from_the_keyboard() {
    let mut lab = Lab::new("keys-empty");
    lab.open_window();
    tab_to(&mut lab, OPEN_VARIABLES);

    lab.press(Key::Enter, Modifiers::NONE);

    assert!(lab.ui.app.ui.variables.is_some());
}

// covers: E5-6.6
#[test]
fn every_button_of_the_section_has_an_accessibility_label() {
    let mut lab = Lab::with_brand("labels", &[]);
    lab.open_window();

    let nodes = lab.nodes();
    let unnamed = nodes.iter().filter(|n| n.role() == Role::Button && n.label().is_none_or(|l| l.trim().is_empty())).count();
    assert_eq!(unnamed, 0, "every button is named");
    assert!(nodes.iter().filter(|n| n.role() == Role::Button && n.label().is_some_and(is_apply_label)).count() >= 2);
    assert!(nodes.iter().filter(|n| n.role() == Role::ColorWell).all(|n| n.label().is_some_and(|l| !l.trim().is_empty())));
}

// covers: E5-6.6
#[test]
fn every_language_translates_the_new_section_texts() {
    let languages = i18n::languages();
    assert_eq!(languages.len(), 22);
    let mut missing = Vec::new();
    for lang in languages.iter().filter(|l| l.code != "en") {
        let own: Vec<&i18n::Lang> = [Some(lang), languages.get(lang.parent)].into_iter().flatten().filter(|l| l.code != "en").collect();
        for key in NEW_TEXTS {
            if !own.iter().any(|l| l.ui.iter().any(|(k, _)| *k == key)) {
                missing.push(format!("{}: {key:?}", lang.code));
            }
        }
    }
    assert!(missing.is_empty(), "untranslated:\n{}", missing.join("\n"));
}
