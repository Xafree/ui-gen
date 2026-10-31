//! The Compare tab of the right panel (`panel.compare`, `ui.set {rightTab: "compare"}`): an imported
//! component set measured against the original its import kept (`uigen.compare`, one variant per
//! frame), the detail window of a row, and the Updates section (`uigen.updateSource`,
//! `uigen.sourceDiff`, `uigen.applyDifferences`). The service is a scripted backend that writes
//! synthetic PNG originals into a temporary project; nothing here is a third-party capture.
//!
//! Not read here: the red pixels of the Difference mode (a texture, not a shape), the reason shown
//! on hover over the text badge, badge contrast, and the browser build (a `cfg!` condition).

use std::collections::BTreeSet;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use egui::accesskit::{Node, Role, Toggled};
use egui::{Event, Key, Modifiers, Pos2, pos2};
use figcraft_doc::NodeId;
use figcraft_engine::capture::{CaptureBackend, CaptureError, CaptureErrorKind, CaptureJob, JobId};
use image::{ImageFormat, Rgba, RgbaImage};
use serde_json::{Value, json};

use crate::a11y::focused_name;
use crate::i18n;
use crate::layers_drag_tests::Ui;
use crate::live_tests::{flat, texts};

static NEXT: AtomicUsize = AtomicUsize::new(0);

const PACKAGE: &str = "synthetic-ui";
const VERSION: &str = "1.0.0";
const NEXT_VERSION: &str = "1.1.0";
const COMPARE: &str = "uigen.compare";
const UPDATE: &str = "uigen.updateSource";
const DIFF: &str = "uigen.sourceDiff";
const APPLY: &str = "uigen.applyDifferences";
const MAX_FRAMES: usize = 300;
const MAX_TABS: usize = 80;
const RIGHT_PANEL_X: f32 = 1000.0;
const USER_FILL: &str = "#FF00FF";
const STATES: [&str; 4] = ["Default", "Hover", "Pressed", "Disabled"];

const EMPTY: &str = "Select an imported component to compare it with its original.";
const NOT_IMPORTED: &str = "This component was not imported: there is no original to compare.";
const NOT_DISTINCT: &str = "Dark mode can't be compared: the library does not follow the system colour scheme.";
const MISSING: &str = "The original capture is missing. Recapture the source to compare.";
const RECAPTURE_FIRST: &str = "Recapture first: 1.1.0 has not been captured yet.";
const NO_DIFFERENCE: &str = "No difference with 1.1.0.";

const NEW_TEXTS: [&str; 33] = [
    "Original",
    "Library",
    "Shapes {percent}",
    "Text",
    "OK",
    "Differs",
    "Side by side",
    "Overlay",
    "Difference",
    "Select an imported component to compare it with its original.",
    "This component was not imported: there is no original to compare.",
    "The original capture is missing. Recapture the source to compare.",
    "The original capture is too large to compare.",
    "Dark mode can't be compared: the library does not follow the system colour scheme.",
    "Size differs: original {original}, library {library}",
    "Measuring {done} of {total}…",
    "Couldn't compare {name}: {cause}",
    "Recapture",
    "Recapturing {package}… {done} of {total}",
    "Updates",
    "Check for changes",
    "No difference with {version}.",
    "{count} differences with {version}",
    "Recapture first: {version} has not been captured yet.",
    "Modified in your copy",
    "Unknown: no earlier capture to tell",
    "Apply unmodified changes",
    "Take the new value",
    "{count} applied",
    "Couldn't apply: {cause}",
    "Compare with the original",
    "Update source components",
    "List source differences",
];

type Outcome = Result<Value, CaptureError>;
type Painted = (String, Pos2, egui::Vec2);

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-compare-ui-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
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

#[derive(Default)]
struct Shared {
    version: String,
    hold: bool,
    evolving: bool,
    dark_same: bool,
    fail_at: Option<String>,
    skew: BTreeSet<String>,
    ready: Vec<(JobId, Outcome)>,
    submitted: u64,
}

fn fill_of(shared: &Shared, state: &str, theme: &str) -> [u8; 4] {
    match (state, theme) {
        ("hover", _) => [21, 101, 192, 255],
        (_, "dark") if !shared.dark_same => [144, 202, 249, 255],
        _ => [25, 118, 210, 255],
    }
}

fn painted_layer(label: &str, fill: &str, box_: [f64; 4]) -> Value {
    let [x, y, width, height] = box_;
    json!({
        "type": "element", "tag": "span", "label": label,
        "box": {"x": x, "y": y, "width": width, "height": height},
        "style": {"backgroundColor": fill}
    })
}

fn evolving_layers(version: &str, state: &str) -> Vec<Value> {
    let (next, hover) = (version == NEXT_VERSION, state == "hover");
    let icon = match (hover, next) {
        (true, _) => "rgb(40, 50, 60)",
        (false, true) => "rgb(200, 30, 30)",
        (false, false) => "rgb(10, 20, 30)",
    };
    let mut layers = vec![painted_layer("Icon", icon, [74.0, 24.0, 12.0, 12.0])];
    if hover || !next {
        layers.push(painted_layer("Ripple", "rgb(70, 80, 90)", [88.0, 24.0, 10.0, 10.0]));
    }
    if hover && next {
        layers.push(painted_layer("Badge", "rgb(0, 150, 0)", [100.0, 24.0, 10.0, 10.0]));
    }
    layers
}

fn snapshot(shared: &Shared, size: &str, state: &str, theme: &str) -> Value {
    let [r, g, b, _] = fill_of(shared, state, theme);
    let (width, height) = if size == "sm" { (80.0, 28.0) } else { (100.0, 36.0) };
    let text_box = json!({"x": 30, "y": 26, "width": 44, "height": 16});
    let mut children = vec![json!({
        "type": "text", "text": "Button", "box": text_box, "lineBox": text_box,
        "style": {"color": "rgb(255, 255, 255)", "fontSize": "14px", "fontFamily": "Inter", "platformFont": "Inter", "fontWeight": "400"}
    })];
    if shared.evolving {
        children.extend(evolving_layers(&shared.version, state));
    }
    json!({
        "version": 1,
        "viewport": {"width": 400, "height": 200, "deviceScaleFactor": 1},
        "root": {
            "type": "element", "tag": "button", "label": "Button", "className": "syn-button",
            "box": {"x": 20, "y": 20, "width": width, "height": height},
            "style": {"backgroundColor": format!("rgb({r}, {g}, {b})")},
            "children": children
        }
    })
}

fn write_original(root: &str, relative: &str, size: &str, fill: [u8; 4]) {
    let (width, height) = if size == "sm" { (80, 28) } else { (100, 36) };
    let mut image = RgbaImage::from_pixel(width, height, Rgba(fill));
    for column in 10..54 {
        for row in 6..22 {
            image.put_pixel(column, row, Rgba([255, 255, 255, 255]));
        }
    }
    let mut bytes = Vec::new();
    image.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png).unwrap();
    let path = PathBuf::from(root).join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn listing(version: &str) -> Value {
    let props = json!([
        {"name": "variant", "type": "enum", "values": ["solid", "outline"]},
        {"name": "size", "type": "enum", "values": ["sm", "md"]}
    ]);
    json!({
        "package": PACKAGE, "version": version, "framework": "react",
        "components": [{"name": "Button", "props": props}, {"name": "Chip", "props": props}],
        "notes": []
    })
}

fn render(shared: &Shared, job: &CaptureJob) -> Outcome {
    let CaptureJob::Render { root, package, component, props, state, theme, .. } = job else { return Ok(json!({})) };
    if shared.fail_at.as_deref() == Some(shared.version.as_str()) {
        return Err(CaptureError { kind: CaptureErrorKind::Render, message: "page crashed: boom".to_string() });
    }
    let style = props.get("variant").and_then(Value::as_str).unwrap_or("solid");
    let size = props.get("size").and_then(Value::as_str).unwrap_or("md");
    let relative = format!(".uigen/captures/{package}/{}/{component}-{style}-{size}-{state}-{theme}.png", shared.version);
    let fill = if shared.skew.contains(state) { [200, 30, 30, 255] } else { fill_of(shared, state, theme) };
    write_original(root, &relative, size, fill);
    Ok(json!({"snapshot": snapshot(shared, size, state, theme), "png": relative}))
}

struct Service(Arc<Mutex<Shared>>);

impl CaptureBackend for Service {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let mut shared = self.0.lock().unwrap();
        let outcome = match &job {
            CaptureJob::Inspect { .. } => Ok(listing(&shared.version)),
            CaptureJob::Render { .. } => render(&shared, &job),
            CaptureJob::Install { package, version, .. } => {
                shared.version.clone_from(version);
                Ok(json!({"package": package, "version": version, "framework": "react", "license": "MIT"}))
            }
            CaptureJob::InstallMany { .. } | CaptureJob::Uninstall { .. } | CaptureJob::Playground { .. } => Ok(json!({})),
        };
        shared.submitted += 1;
        let id = shared.submitted;
        shared.ready.push((id, outcome));
        id
    }

    fn poll(&mut self) -> Vec<(JobId, Outcome)> {
        let mut shared = self.0.lock().unwrap();
        if shared.hold { Vec::new() } else { std::mem::take(&mut shared.ready) }
    }

    fn shutdown(&mut self) {}
}

fn centre(node: &Node) -> Option<Pos2> {
    let b = node.bounds()?;
    Some(pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32))
}

fn size_of(node: &Node) -> (f64, f64) {
    node.bounds().map_or((0.0, 0.0), |b| (b.x1 - b.x0, b.y1 - b.y0))
}

fn mentions(shown: &[String], needle: &str) -> bool {
    shown.iter().any(|t| t.contains(needle))
}

fn count_of(shown: &[String], text: &str) -> usize {
    shown.iter().filter(|t| t.trim() == text).count()
}

fn selected(node: &Node) -> bool {
    node.is_selected() == Some(true) || node.toggled() == Some(Toggled::True)
}

fn pair(line: &str) -> Option<(usize, usize)> {
    let (done, total) = line.trim_end_matches('…').split_once(" of ")?;
    Some((done.trim().parse().ok()?, total.trim().parse().ok()?))
}

fn measuring(shown: &[String]) -> Option<(usize, usize)> {
    shown.iter().find_map(|t| t.trim().strip_prefix("Measuring ").and_then(pair))
}

fn recapturing(shown: &[String]) -> Option<(usize, usize)> {
    let head = format!("Recapturing {PACKAGE}… ");
    shown.iter().find_map(|t| t.trim().strip_prefix(head.as_str()).and_then(pair))
}

struct Lab {
    ui: Ui,
    shared: Arc<Mutex<Shared>>,
    root: PathBuf,
    _dir: Dir,
}

impl Lab {
    fn new(tag: &str, evolving: bool, dark_same: bool) -> Self {
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
        let shared = Arc::new(Mutex::new(Shared { version: VERSION.to_string(), evolving, dark_same, ..Shared::default() }));
        ui.app.session.set_capture_backend(Box::new(Service(Arc::clone(&shared))));
        ui.app.ui.home = false;
        ui.app.ui.toast = None;
        let root = dir.0.clone();
        let mut lab = Self { ui, shared, root, _dir: dir };
        lab.settle();
        lab
    }

    fn with_button(tag: &str, states: &[&str]) -> (Self, NodeId) {
        let mut lab = Self::new(tag, false, false);
        let copy = lab.import_and_place("Button", states);
        (lab, copy)
    }

    fn settle(&mut self) {
        for _ in 0..3 {
            self.ui.frame(vec![]);
        }
    }

    fn run(&mut self, id: &str, params: Value) -> Value {
        self.ui.app.session.execute(id, &params).unwrap_or_else(|e| panic!("{id}: {e}"))
    }

    fn set_of(&mut self, component: &str) -> Option<NodeId> {
        let sections = self.run("uigen.imported", json!({}));
        let section = sections["sections"].as_array()?.iter().find(|s| s["package"] == PACKAGE && s["version"] == VERSION)?;
        let found = section["components"].as_array()?.iter().find(|c| c["name"] == component)?;
        found["set"].as_u64().map(NodeId)
    }

    fn import_and_place(&mut self, component: &str, states: &[&str]) -> NodeId {
        let params = json!({
            "package": PACKAGE, "component": component, "styles": ["solid"], "sizes": ["md"],
            "states": states, "themes": ["light", "dark"], "stage": true
        });
        assert!(self.ui.app.run("uigen.importComponent", params).is_some(), "the import starts");
        let set = (0..MAX_FRAMES)
            .find_map(|_| {
                self.ui.app.session.pump_capture();
                self.ui.frame(vec![]);
                self.set_of(component)
            })
            .unwrap_or_else(|| panic!("the import of {component} never reached its import page"));
        let page = self.ui.app.session.doc().unwrap().pages[0];
        let placed = self.run("uigen.placeImported", json!({"set": set.0, "page": page.0}));
        self.settle();
        figcraft_engine::params::id_of(&placed["set"]).unwrap_or_else(|| panic!("no copy in {placed}"))
    }

    fn select(&mut self, copy: NodeId) {
        self.run("select.set", json!({"ids": [copy.0]}));
        self.settle();
    }

    fn open(&mut self) {
        self.ui.app.run("panel.compare", json!({}));
    }

    fn frame_shown(&mut self) -> Vec<String> {
        texts(&flat(&self.ui.frame_output(vec![]).shapes)).into_iter().map(|(t, _, _)| t).collect()
    }

    fn painted(&mut self) -> Vec<Painted> {
        self.ui.frame(vec![]);
        texts(&flat(&self.ui.frame_output(vec![]).shapes))
    }

    fn shown(&mut self) -> Vec<String> {
        self.painted().into_iter().map(|(t, _, _)| t).collect()
    }

    fn nodes(&mut self) -> Vec<Node> {
        self.ui.ctx.enable_accesskit();
        self.ui.frame(vec![]);
        self.ui.frame_output(vec![]).platform_output.accesskit_update.expect("an AccessKit update").nodes.into_iter().map(|(_, n)| n).collect()
    }

    fn control(&mut self, label: &str) -> Option<Node> {
        self.nodes().into_iter().find(|n| n.role() != Role::Label && n.label().is_some_and(|l| l.trim() == label))
    }

    fn images(&mut self, prefix: &str) -> Vec<Node> {
        self.nodes().into_iter().filter(|n| n.label().is_some_and(|l| l.starts_with(prefix))).collect()
    }

    fn wait_for_rows(&mut self, rows: usize) {
        for _ in 0..MAX_FRAMES {
            self.ui.frame(vec![]);
            if self.images("Original of ").len() >= rows {
                self.settle();
                return;
            }
        }
        panic!("the {rows} rows never appeared; on screen: {:?}", self.shown());
    }

    fn centre_of(&mut self, label: &str) -> Pos2 {
        let node = self.control(label).unwrap_or_else(|| panic!("no control named {label:?} on screen"));
        centre(&node).unwrap_or_else(|| panic!("{label:?} has no bounds"))
    }

    fn click_at(&mut self, at: Pos2) {
        self.ui.button(at, true);
        self.ui.button(at, false);
        self.settle();
    }

    fn click(&mut self, label: &str) {
        let at = self.centre_of(label);
        self.click_at(at);
    }

    fn row_label_at(&mut self, state: &str) -> Pos2 {
        let painted = self.painted();
        let found = painted.iter().find(|(t, p, _)| p.x > RIGHT_PANEL_X && (t.trim() == state || t.trim().ends_with(&format!("State={state}"))));
        let (_, p, size) = found.unwrap_or_else(|| panic!("no row label {state:?} in the right panel: {painted:?}"));
        *p + *size / 2.0
    }

    fn press(&mut self, key: Key, modifiers: Modifiers) {
        let events = [true, false].map(|pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers }).to_vec();
        self.ui.frame_with(events, modifiers);
        self.settle();
    }

    fn tab_to(&mut self, matches: impl Fn(&str) -> bool) {
        for _ in 0..MAX_TABS {
            self.press(Key::Tab, Modifiers::NONE);
            if matches(&focused_name(&self.ui.ctx)) {
                return;
            }
        }
        panic!("Tab never reached the wanted control; last focus {:?}", focused_name(&self.ui.ctx));
    }

    fn mark(&self) -> usize {
        self.ui.app.session.journal.len()
    }

    fn ran_since(&self, mark: usize, command: &str) -> Vec<Value> {
        self.ui.app.session.journal[mark..].iter().filter(|(c, _)| c == command).map(|(_, p)| p.clone()).collect()
    }

    fn document(&self) -> String {
        serde_json::to_string(self.ui.app.session.doc().unwrap()).unwrap()
    }

    fn copy_dump(&self, copy: NodeId) -> String {
        let doc = self.ui.app.session.doc().unwrap();
        doc.subtree(copy).into_iter().map(|id| format!("{:?}", doc.node(id).unwrap())).collect::<Vec<_>>().join(
            "
",
        )
    }

    fn variant(&self, copy: NodeId, state: &str) -> NodeId {
        let doc = self.ui.app.session.doc().unwrap();
        let wanted = format!("State={state}");
        let found = doc.node(copy).unwrap().children.iter().copied().find(|id| doc.node(*id).unwrap().name.ends_with(&wanted));
        found.unwrap_or_else(|| panic!("no variant {wanted:?}"))
    }

    fn layer(&self, root: NodeId, name: &str) -> NodeId {
        let doc = self.ui.app.session.doc().unwrap();
        doc.subtree(root).into_iter().find(|id| doc.node(*id).is_some_and(|n| n.name == name)).unwrap_or_else(|| panic!("no layer {name:?}"))
    }

    fn resize_variant(&mut self, copy: NodeId, state: &str, width: u32) {
        let variant = self.variant(copy, state);
        self.run("node.set", json!({"id": variant.0, "props": {"width": width}}));
        self.settle();
    }

    fn release_to_next_version(&mut self) {
        self.run("project.setSourceVersion", json!({"package": PACKAGE, "version": NEXT_VERSION}));
        for _ in 0..30 {
            self.ui.app.session.pump_capture();
            self.ui.frame(vec![]);
        }
        assert_eq!(self.shared.lock().unwrap().version, NEXT_VERSION, "the service moved to the new version");
    }

    fn differences(&mut self, copy: NodeId) -> Vec<Value> {
        self.run(DIFF, json!({"copy": copy.0}))["differences"].as_array().cloned().unwrap_or_default()
    }

    fn evolved(tag: &str) -> (Self, NodeId) {
        let mut lab = Self::new(tag, true, false);
        let copy = lab.import_and_place("Button", &["Default", "Hover"]);
        lab.release_to_next_version();
        lab.run(UPDATE, json!({"package": PACKAGE}));
        for _ in 0..60 {
            lab.ui.app.session.pump_capture();
            lab.ui.frame(vec![]);
        }
        (lab, copy)
    }

    fn evolved_and_modified(tag: &str) -> (Self, NodeId, String) {
        let (mut lab, copy) = Self::evolved(tag);
        let icon = lab.layer(lab.variant(copy, "Default"), "Icon");
        lab.run("fill.set", json!({"ids": [icon.0], "index": 0, "color": USER_FILL}));
        let diffs = lab.differences(copy);
        let modified = diffs.iter().find(|d| d["state"] == "modified").unwrap_or_else(|| panic!("no modified difference in {diffs:?}"));
        let id = modified["id"].as_str().unwrap().to_string();
        lab.select(copy);
        (lab, copy, id)
    }
}

// covers: E5-7.1
#[test]
fn in_a_project_the_tab_row_ends_with_compare_and_every_way_in_opens_it() {
    let (mut lab, _copy) = Lab::with_button("tab", &["Default"]);
    lab.run("select.set", json!({"ids": []}));
    let mut tabs: Vec<(f32, String)> = lab
        .nodes()
        .iter()
        .filter(|n| n.role() == Role::Tab)
        .filter_map(|n| Some((centre(n)?.x, n.label()?.trim().to_string())))
        .filter(|(_, label)| ["Design", "Prototype", "Compare"].contains(&label.as_str()))
        .collect();
    tabs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let order: Vec<&str> = tabs.iter().map(|(_, label)| label.as_str()).collect();
    assert_eq!(order, ["Design", "Prototype", "Compare"]);

    assert!(lab.ui.app.run("panel.compare", json!({})).is_some(), "panel.compare runs");
    lab.settle();
    assert!(mentions(&lab.shown(), EMPTY));
    assert!(lab.control("Compare").is_some_and(|n| selected(&n)), "the tab is the selected one");

    lab.ui.app.run("view.design", json!({}));
    lab.settle();
    assert!(!mentions(&lab.shown(), EMPTY));
    lab.click("Compare");
    assert!(mentions(&lab.shown(), EMPTY), "a click on the tab opens it");

    lab.ui.app.run("view.design", json!({}));
    lab.settle();
    let ctx = lab.ui.ctx.clone();
    crate::control::handle(&mut lab.ui.app, &ctx, "ui.set", &json!({"rightTab": "compare"})).unwrap();
    lab.settle();
    assert!(mentions(&lab.shown(), EMPTY), "ui.set opens it");
}

// covers: E5-7.1
#[test]
fn with_the_tab_open_the_top_mode_selector_activates_no_segment() {
    let mut lab = Lab::new("modes", false, false);
    lab.open();
    lab.settle();

    let nodes = lab.nodes();
    for mode in ["Draw", "Design", "Prototype", "Inspect"] {
        let button = nodes.iter().find(|n| n.role() == Role::Button && n.label() == Some(mode)).unwrap_or_else(|| panic!("no mode button {mode}"));
        assert_ne!(button.toggled(), Some(Toggled::True), "{mode} must not look active");
    }
}

// covers: E5-7.1
#[test]
fn outside_a_project_there_is_no_tab_and_neither_the_command_nor_ui_set_opens_it() {
    let mut project = Lab::new("contrast", false, false);
    project.open();
    project.settle();
    assert!(mentions(&project.shown(), EMPTY), "in a project the same command opens the tab");
    drop(project);
    figcraft_text::scan_system_fonts();
    let mut ui = Ui::new(figcraft_engine::Session::new());
    ui.app.session.prefs.language = "en".into();
    ui.app.ui.home = false;
    ui.app.run("panel.compare", json!({}));
    let ctx = ui.ctx.clone();
    let _ = crate::control::handle(&mut ui.app, &ctx, "ui.set", &json!({"rightTab": "compare"}));
    for _ in 0..3 {
        ui.frame(vec![]);
    }

    ui.ctx.enable_accesskit();
    ui.frame(vec![]);
    let nodes: Vec<Node> =
        ui.frame_output(vec![]).platform_output.accesskit_update.expect("an AccessKit update").nodes.into_iter().map(|(_, n)| n).collect();
    assert!(!nodes.iter().any(|n| n.role() == Role::Tab && n.label().is_some_and(|l| l.trim() == "Compare")), "no Compare tab");
    let design = nodes.iter().find(|n| n.role() == Role::Tab && n.label().is_some_and(|l| l.trim() == "Design"));
    assert!(design.is_some_and(selected), "the Design tab stays the selected one");
    ui.frame(vec![]);
    let shown: Vec<String> = texts(&flat(&ui.frame_output(vec![]).shapes)).into_iter().map(|(t, _, _)| t).collect();
    assert!(!mentions(&shown, EMPTY), "{shown:?}");
}

// covers: E5-7.2
#[test]
fn with_no_set_selected_and_none_on_the_page_the_panel_says_what_to_select() {
    let mut lab = Lab::new("empty", false, false);
    let mark = lab.mark();

    lab.open();
    lab.settle();

    let shown = lab.shown();
    assert_eq!(count_of(&shown, EMPTY), 1, "{shown:?}");
    assert!(lab.ran_since(mark, COMPARE).is_empty(), "nothing is measured");
}

// covers: E5-7.2
#[test]
fn a_set_without_a_source_says_it_was_not_imported_and_measures_nothing() {
    let (mut lab, copy) = Lab::with_button("not-imported", &["Default"]);
    lab.run("node.set", json!({"id": copy.0, "props": {"pluginData": {"uigen.source": null}}}));
    lab.select(copy);
    let mark = lab.mark();

    lab.open();
    lab.settle();

    let shown = lab.shown();
    assert_eq!(count_of(&shown, NOT_IMPORTED), 1, "{shown:?}");
    assert!(!mentions(&shown, EMPTY));
    assert!(lab.ran_since(mark, COMPARE).is_empty(), "nothing is measured");
}

// covers: E5-7.2
// covers: E5-7.7
#[test]
fn variants_are_measured_one_per_frame_while_the_panel_counts_them() {
    let (mut lab, copy) = Lab::with_button("loading", &STATES);
    lab.select(copy);
    lab.open();

    let mut seen = Vec::new();
    for _ in 0..40 {
        let mark = lab.mark();
        let shown = lab.frame_shown();
        let measured: usize = lab.ran_since(mark, COMPARE).iter().map(|p| p["variants"].as_array().map_or(usize::MAX, Vec::len)).sum();
        assert!(measured <= 1, "at most one variant per frame, measured {measured}");
        match measuring(&shown) {
            Some(progress) => seen.push(progress),
            None if !seen.is_empty() => break,
            None => {}
        }
    }

    assert!(seen.len() >= 3, "the counter is shown while measuring: {seen:?}");
    assert!(seen.iter().all(|(done, total)| *total == 4 && done < total), "{seen:?}");
    assert!(seen.windows(2).all(|w| w[0].0 <= w[1].0), "the count never goes back: {seen:?}");
    lab.wait_for_rows(4);
    let shown = lab.shown();
    assert!(measuring(&shown).is_none());
    for state in STATES {
        assert!(mentions(&shown, state), "row {state}: {shown:?}");
    }
}

// covers: E5-7.2
#[test]
fn the_engine_refusing_the_comparison_shows_its_cause_and_no_rows() {
    let (mut lab, copy) = Lab::with_button("error", &["Default"]);
    for _ in 0..128 {
        lab.run("component.addVariant", json!({"id": copy.0}));
    }
    lab.select(copy);

    lab.open();
    lab.settle();
    lab.settle();

    let shown = lab.shown();
    let failure = shown.iter().find(|t| t.starts_with("Couldn't compare Button: ")).unwrap_or_else(|| panic!("no failure text in {shown:?}"));
    assert!(failure.contains("128"), "the cause is the engine's: {failure:?}");
    assert!(lab.images("Original of ").is_empty());
}

// covers: E5-7.2
// covers: E5-7.3
// covers: E5-7.8
#[test]
fn every_state_gets_a_row_with_both_images_the_shapes_figure_and_an_ok_badge() {
    let (mut lab, copy) = Lab::with_button("success", &STATES);
    lab.select(copy);
    lab.open();
    lab.wait_for_rows(4);

    let engine = lab.run(COMPARE, json!({"set": copy.0}));
    assert!(engine["rows"].as_array().unwrap().iter().all(|r| r["status"] == "ok"), "precondition: {engine}");
    let shown = lab.shown();
    assert!(shown.iter().filter(|t| t.starts_with("Shapes ") && t.chars().any(|c| c.is_ascii_digit())).count() >= 4, "{shown:?}");
    assert!(count_of(&shown, "OK") >= 4, "{shown:?}");
    assert_eq!(count_of(&shown, "Differs"), 0, "{shown:?}");
    for state in STATES {
        let original = lab.images("Original of ").iter().filter(|n| n.label().is_some_and(|l| l.contains(state))).count();
        let library = lab.images("Library version of ").iter().filter(|n| n.label().is_some_and(|l| l.contains(state))).count();
        assert_eq!((original, library), (1, 1), "images of {state}");
    }
}

// covers: E5-7.3
#[test]
fn a_row_that_differs_shows_the_differs_badge_next_to_the_rows_that_match() {
    let mut lab = Lab::new("differs", false, false);
    lab.shared.lock().unwrap().skew.insert("hover".to_string());
    let copy = lab.import_and_place("Button", &["Default", "Hover"]);
    lab.select(copy);

    lab.open();
    lab.wait_for_rows(2);

    let shown = lab.shown();
    assert!(count_of(&shown, "Differs") >= 1, "{shown:?}");
    assert!(count_of(&shown, "OK") >= 1, "{shown:?}");
}

// covers: E5-7.3
// covers: E5-7.8
#[test]
fn images_keep_one_scale_and_a_different_size_is_written_out() {
    let (mut lab, copy) = Lab::with_button("sizes", &["Default", "Hover"]);
    lab.resize_variant(copy, "Hover", 120);
    lab.select(copy);
    lab.open();
    lab.wait_for_rows(2);

    let shown = lab.shown();
    assert_eq!(shown.iter().filter(|t| t.starts_with("Size differs: original ") && t.contains(", library ")).count(), 1, "{shown:?}");
    let hover = |nodes: Vec<Node>| nodes.into_iter().find(|n| n.label().is_some_and(|l| l.contains("Hover"))).expect("a Hover image");
    let (original, library) = (size_of(&hover(lab.images("Original of "))), size_of(&hover(lab.images("Library version of "))));
    assert!(original.0 > 1.0 && library.0 > 1.0, "both images are drawn: {original:?} {library:?}");
    assert!((library.0 / original.0 - 1.2).abs() < 0.03, "the library image is 1.2 times wider: {original:?} {library:?}");
    assert!((library.1 / original.1 - 1.0).abs() < 0.03, "same height at the same scale: {original:?} {library:?}");
}

// covers: E5-7.3
#[test]
fn the_dark_theme_is_measured_when_it_is_distinct_and_refused_in_words_when_it_is_not() {
    let (mut lab, copy) = Lab::with_button("dark", &["Default", "Hover"]);
    lab.select(copy);
    lab.open();
    lab.wait_for_rows(2);
    let mark = lab.mark();

    lab.click("Dark");
    lab.settle();
    lab.wait_for_rows(2);

    assert!(lab.ran_since(mark, COMPARE).iter().any(|p| p["theme"] == "dark"), "the dark theme is asked for");
    let shown = lab.shown();
    assert!(!mentions(&shown, NOT_DISTINCT) && mentions(&shown, "Shapes "), "{shown:?}");

    let mut same = Lab::new("dark-same", false, true);
    let copy = same.import_and_place("Button", &["Default", "Hover"]);
    same.select(copy);
    same.open();
    same.wait_for_rows(2);
    same.click("Dark");
    same.settle();
    let shown = same.shown();
    assert!(mentions(&shown, NOT_DISTINCT), "{shown:?}");
    assert!(!mentions(&shown, "Shapes ") && count_of(&shown, "OK") == 0, "no figure for a theme that can't be compared: {shown:?}");
}

// covers: E5-7.3
#[test]
fn a_missing_original_says_so_and_recapture_runs_the_update_of_the_source() {
    let (mut lab, copy) = Lab::with_button("missing", &["Default"]);
    std::fs::remove_dir_all(lab.root.join(".uigen/captures")).unwrap();
    lab.select(copy);
    let mark = lab.mark();

    lab.open();
    lab.settle();
    lab.settle();

    let shown = lab.shown();
    assert!(mentions(&shown, MISSING), "{shown:?}");
    assert!(!mentions(&shown, "Shapes "));
    lab.click("Recapture");
    let started = lab.ran_since(mark, UPDATE);
    assert_eq!(started.len(), 1, "{started:?}");
    assert_eq!(started[0]["package"], PACKAGE);
}

fn open_detail(lab: &mut Lab, copy: NodeId) {
    lab.select(copy);
    lab.open();
    lab.wait_for_rows(2);
    let at = lab.row_label_at("Hover");
    lab.click_at(at);
}

// covers: E5-7.4
#[test]
fn a_click_on_a_row_opens_the_detail_window_with_three_modes_and_escape_closes_it() {
    let (mut lab, copy) = Lab::with_button("detail", &["Default", "Hover"]);
    let before = lab.shown();
    assert!(!mentions(&before, "Overlay"));

    open_detail(&mut lab, copy);

    let shown = lab.shown();
    for mode in ["Side by side", "Overlay", "Difference"] {
        assert!(count_of(&shown, mode) >= 1, "mode {mode}: {shown:?}");
    }
    lab.press(Key::Escape, Modifiers::NONE);
    let shown = lab.shown();
    assert!(!mentions(&shown, "Overlay") && !mentions(&shown, "Side by side"), "{shown:?}");
    assert!(lab.control("Compare").is_some_and(|n| selected(&n)), "the tab stays open");
}

// covers: E5-7.4
// covers: E5-7.8
#[test]
fn the_modes_are_chosen_from_the_keyboard_and_the_overlay_has_a_slider() {
    let (mut lab, copy) = Lab::with_button("modes-keys", &["Default", "Hover"]);
    lab.resize_variant(copy, "Hover", 120);
    open_detail(&mut lab, copy);
    assert!(lab.control("Side by side").is_some_and(|n| selected(&n)), "side by side is the first mode");
    assert!(!lab.nodes().iter().any(|n| n.role() == Role::Slider));

    lab.tab_to(|name| name == "Overlay");
    lab.press(Key::Enter, Modifiers::NONE);
    assert!(lab.control("Overlay").is_some_and(|n| selected(&n)));
    assert!(lab.nodes().iter().any(|n| n.role() == Role::Slider), "the overlay has a slider");

    lab.tab_to(|name| name == "Difference");
    lab.press(Key::Space, Modifiers::NONE);
    assert!(lab.control("Difference").is_some_and(|n| selected(&n)));
    assert!(!lab.control("Overlay").is_some_and(|n| selected(&n)));

    lab.tab_to(|name| name == "Side by side");
    lab.press(Key::Enter, Modifiers::NONE);
    let biggest = |nodes: Vec<Node>| nodes.iter().map(size_of).max_by(|a, b| (a.0 * a.1).total_cmp(&(b.0 * b.1))).unwrap_or_default();
    let (original, library) = (biggest(lab.images("Original of ")), biggest(lab.images("Library version of ")));
    assert!((original.0 / original.1 - 100.0 / 36.0).abs() < 0.1, "the original keeps its proportions: {original:?}");
    assert!((library.0 / library.1 - 120.0 / 36.0).abs() < 0.1, "the variant keeps its proportions: {library:?}");
}

// covers: E5-7.5
#[test]
fn updates_show_both_versions_and_check_for_changes_follows_the_recapture_then_lists_the_differences() {
    let mut lab = Lab::new("updates", true, false);
    let copy = lab.import_and_place("Button", &["Default", "Hover"]);
    lab.release_to_next_version();
    lab.select(copy);
    lab.open();
    lab.settle();
    let shown = lab.shown();
    assert!(mentions(&shown, "Updates") && mentions(&shown, VERSION) && mentions(&shown, NEXT_VERSION), "{shown:?}");
    assert_eq!(count_of(&shown, RECAPTURE_FIRST), 1, "no capture of 1.1.0 yet: {shown:?}");
    lab.shared.lock().unwrap().hold = true;
    let (mark, before) = (lab.mark(), lab.copy_dump(copy));

    lab.click("Check for changes");

    let started = lab.ran_since(mark, UPDATE);
    assert_eq!(started.len(), 1, "{started:?}");
    assert_eq!(started[0]["package"], PACKAGE);
    let progress = recapturing(&lab.shown()).expect("the progress line");
    assert!(progress.1 >= 1 && progress.0 <= progress.1, "{progress:?}");
    lab.shared.lock().unwrap().hold = false;
    for _ in 0..60 {
        lab.ui.app.session.pump_capture();
        lab.ui.frame(vec![]);
    }

    let shown = lab.shown();
    assert!(recapturing(&shown).is_none(), "{shown:?}");
    for word in ["Added", "Removed", "Changed", "Icon", "Ripple", "Badge"] {
        assert!(mentions(&shown, word), "{word}: {shown:?}");
    }
    assert!(lab.ran_since(mark, APPLY).is_empty(), "nothing is applied without a click");
    assert_eq!(lab.copy_dump(copy), before, "the library copy was only read");
}

// covers: E5-7.5
#[test]
fn a_recapture_with_nothing_new_says_there_is_no_difference() {
    let mut lab = Lab::new("no-difference", false, false);
    let copy = lab.import_and_place("Button", &["Default", "Hover"]);
    lab.release_to_next_version();
    lab.select(copy);
    lab.open();
    lab.settle();

    lab.click("Check for changes");
    for _ in 0..60 {
        lab.ui.app.session.pump_capture();
        lab.ui.frame(vec![]);
    }

    let shown = lab.shown();
    assert_eq!(count_of(&shown, NO_DIFFERENCE), 1, "{shown:?}");
    assert!(!mentions(&shown, "Take the new value"));
}

// covers: E5-7.5
#[test]
fn a_failed_recapture_names_the_component() {
    let mut lab = Lab::new("failed-recapture", false, false);
    let copy = lab.import_and_place("Button", &["Default"]);
    lab.release_to_next_version();
    lab.shared.lock().unwrap().fail_at = Some(NEXT_VERSION.to_string());
    lab.select(copy);
    lab.open();
    lab.settle();

    lab.click("Check for changes");
    for _ in 0..60 {
        lab.ui.app.session.pump_capture();
        lab.ui.frame(vec![]);
    }

    let painted = lab.painted();
    let named = painted.iter().any(|(t, p, _)| p.x > RIGHT_PANEL_X && t.contains("Button") && t.trim() != "Button" && !t.starts_with("Style="));
    assert!(named, "a line of the panel names Button: {painted:?}");
    assert!(recapturing(&lab.shown()).is_none(), "the recapture is over");
    assert!(!mentions(&lab.shown(), NO_DIFFERENCE), "a failure is not 'no difference'");
}

// covers: E5-7.5
// covers: E5-7.6
#[test]
fn differences_are_labelled_by_what_the_user_changed_and_none_is_applied_without_a_click() {
    let (mut lab, copy, _id) = Lab::evolved_and_modified("labels");
    let (mark, before) = (lab.mark(), lab.document());

    lab.open();
    lab.settle();
    lab.settle();

    let shown = lab.shown();
    assert!(mentions(&shown, "Modified in your copy"), "{shown:?}");
    assert_eq!(shown.iter().filter(|t| t.trim() == "Take the new value").count(), 1, "only the modified one offers it: {shown:?}");
    assert!(lab.ran_since(mark, APPLY).is_empty());
    assert_eq!(lab.document(), before);

    let base = lab.run(DIFF, json!({"copy": copy.0}))["base"]["page"].clone();
    lab.run("page.delete", json!({"id": base}));
    lab.settle();
    lab.settle();
    assert!(mentions(&lab.shown(), "Unknown: no earlier capture to tell"));
}

// covers: E5-7.6
#[test]
fn apply_unmodified_changes_runs_once_without_ids_and_the_toast_offers_undo() {
    let (mut lab, copy, _id) = Lab::evolved_and_modified("apply-clean");
    lab.open();
    lab.settle();
    lab.settle();
    let mark = lab.mark();

    lab.click("Apply unmodified changes");

    let ran = lab.ran_since(mark, APPLY);
    assert_eq!(ran.len(), 1, "{ran:?}");
    assert!(ran[0].get("ids").is_none(), "no ids: {:?}", ran[0]);
    assert_eq!(figcraft_engine::params::id_of(&ran[0]["copy"]), Some(copy));
    assert_eq!(lab.ui.app.ui.toast.as_ref().map(|t| t.0.as_str()), Some("2 applied"));
    assert_eq!(lab.ui.app.ui.toast_action.as_ref().map(|a| a.label.as_str()), Some("Undo"));
    let undo = lab.nodes().iter().filter(|n| n.role() == Role::Button && n.label() == Some("Undo")).count();
    assert_eq!(undo, 1);
    let left = lab.differences(copy);
    assert_eq!(left.len(), 1, "only the difference the user changed is left: {left:?}");
}

// covers: E5-7.6
#[test]
fn take_the_new_value_applies_that_difference_alone() {
    let (mut lab, copy, id) = Lab::evolved_and_modified("take-new");
    lab.open();
    lab.settle();
    lab.settle();
    let mark = lab.mark();

    lab.click("Take the new value");

    let ran = lab.ran_since(mark, APPLY);
    assert_eq!(ran.len(), 1, "{ran:?}");
    assert_eq!(ran[0]["ids"], json!([id]));
    assert_eq!(figcraft_engine::params::id_of(&ran[0]["copy"]), Some(copy));
    assert_eq!(lab.ui.app.ui.toast.as_ref().map(|t| t.0.as_str()), Some("1 applied"));
    assert_eq!(lab.ui.app.ui.toast_action.as_ref().map(|a| a.label.as_str()), Some("Undo"));
}

// covers: E5-7.6
#[test]
fn a_refusal_of_the_engine_is_shown_and_the_document_stays_identical() {
    let (mut lab, copy, _id) = Lab::evolved_and_modified("refused");
    lab.open();
    lab.settle();
    lab.settle();
    assert!(mentions(&lab.shown(), "Take the new value"), "the list is shown before the copy is broken");
    let revision = lab.ui.app.session.ds().unwrap().revision;
    lab.run("node.set", json!({"id": copy.0, "props": {"pluginData": {"uigen.source": null}}}));
    lab.ui.app.session.ds_mut().unwrap().revision = revision;
    let (mark, before) = (lab.mark(), lab.document());

    lab.click("Take the new value");

    let shown = lab.shown();
    assert!(shown.iter().any(|t| t.starts_with("Couldn't apply: ") && t.len() > "Couldn't apply: ".len()), "{shown:?}");
    assert!(lab.ran_since(mark, APPLY).is_empty(), "the refused command is not journaled");
    assert_eq!(lab.document(), before);
    assert!(lab.ui.app.ui.toast_action.is_none(), "nothing to undo");
}

// covers: E5-7.7
#[test]
fn a_new_document_version_is_measured_again_and_an_unchanged_one_is_not() {
    let (mut lab, copy) = Lab::with_button("versions", &["Default", "Hover"]);
    lab.select(copy);
    lab.open();
    lab.wait_for_rows(2);
    let mark = lab.mark();
    for _ in 0..8 {
        lab.ui.frame(vec![]);
    }
    assert!(lab.ran_since(mark, COMPARE).is_empty(), "an unchanged document costs no measure");
    assert!(!mentions(&lab.shown(), "Size differs"));

    lab.resize_variant(copy, "Default", 120);
    for _ in 0..12 {
        lab.ui.frame(vec![]);
    }

    assert!(!lab.ran_since(mark, COMPARE).is_empty(), "the new version is measured");
    assert!(mentions(&lab.shown(), "Size differs: original "), "and the row shows the new size");
}

// covers: E5-7.7
#[test]
fn changing_the_set_while_measuring_never_shows_rows_of_the_other_component() {
    let mut lab = Lab::new("switch", false, false);
    let button = lab.import_and_place("Button", &STATES);
    let chip = lab.import_and_place("Chip", &["Default", "Hover"]);
    lab.select(button);
    lab.open();
    lab.ui.frame(vec![]);
    lab.ui.frame(vec![]);
    assert!(measuring(&lab.frame_shown()).is_some(), "the Button is still being measured");

    lab.select(chip);
    for _ in 0..40 {
        let mark = lab.mark();
        let shown = lab.frame_shown();
        let measured: usize = lab.ran_since(mark, COMPARE).iter().map(|p| p["variants"].as_array().map_or(usize::MAX, Vec::len)).sum();
        assert!(measured <= 1, "at most one variant per frame, measured {measured}");
        assert!(measuring(&shown).is_none_or(|(_, total)| total == 2), "the count is the Chip's: {shown:?}");
    }

    lab.wait_for_rows(2);
    assert_eq!(lab.images("Original of ").len(), 2, "only the Chip's two rows");
    let shown = lab.shown();
    assert!(!mentions(&shown, "Pressed") && !mentions(&shown, "Disabled"), "{shown:?}");
}

// covers: E5-7.8
#[test]
fn tab_reaches_a_row_and_enter_opens_it_while_escape_closes_the_detail() {
    let (mut lab, copy) = Lab::with_button("keys", &["Default", "Hover"]);
    lab.select(copy);
    lab.open();
    lab.wait_for_rows(2);
    let at = lab.centre_of("Compare");
    lab.click_at(at);

    lab.tab_to(|name| name.contains("Hover"));
    lab.press(Key::Tab, Modifiers::SHIFT);
    assert!(!focused_name(&lab.ui.ctx).contains("Hover"), "Shift+Tab goes back");
    lab.press(Key::Tab, Modifiers::NONE);
    assert!(focused_name(&lab.ui.ctx).contains("Hover"), "Tab comes forward again");
    lab.press(Key::Enter, Modifiers::NONE);
    assert!(mentions(&lab.shown(), "Overlay"), "Enter opens the detail window");
    lab.press(Key::Escape, Modifiers::NONE);
    assert!(!mentions(&lab.shown(), "Overlay"), "Escape closes it");

    lab.tab_to(|name| name.contains("Default"));
    lab.press(Key::Space, Modifiers::NONE);
    assert!(mentions(&lab.shown(), "Overlay"), "Space opens a row too");
}

// covers: E5-7.8
#[test]
fn every_control_of_the_tab_is_named_and_every_image_is_described() {
    let (mut lab, copy, _id) = Lab::evolved_and_modified("labels-a11y");
    lab.open();
    lab.wait_for_rows(2);

    let nodes = lab.nodes();
    let unnamed = nodes.iter().filter(|n| n.role() == Role::Button && n.label().is_none_or(|l| l.trim().is_empty())).count();
    assert_eq!(unnamed, 0, "every button is named");
    for label in ["Light", "Dark", "Check for changes", "Apply unmodified changes", "Take the new value"] {
        assert!(nodes.iter().any(|n| n.role() != Role::Label && n.label().is_some_and(|l| l.trim() == label)), "no control named {label:?}");
    }
    let described = |prefix: &str| nodes.iter().filter(|n| n.label().is_some_and(|l| l.starts_with(prefix) && l.contains("State="))).count();
    assert_eq!((described("Original of "), described("Library version of ")), (2, 2));
    assert!(lab.variant(copy, "Default") != lab.variant(copy, "Hover"));
}

// covers: E5-7.8
#[test]
fn every_language_translates_the_new_texts_of_the_tab() {
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
