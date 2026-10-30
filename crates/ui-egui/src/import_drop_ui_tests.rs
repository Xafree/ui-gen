//! Dragging a tile of an "Imported · <package> <version>" section onto the canvas: while it is
//! dragged the ghost follows the pointer and, over the canvas only, a hint says what the drop does
//! ("Drop to make it the component of this page" on a blank page, "Drop to replace {name} on this
//! page" on an occupied one) while no frame or instance is highlighted, even with Alt held.
//! Releasing over the canvas runs `uigen.placeImported` once for the set on the open page; releasing
//! elsewhere does nothing. A toast says what happened and offers "Undo" ("{old} replaced by {new}",
//! "Copied {name} into your library"); a refusal of the engine is shown as a toast and changes
//! nothing. A local tile keeps its own drop. The import is a real staged one
//! (`uigen.importComponent` with `stage: true`) against a scripted capture backend.
//!
//! The 22 catalogs holding every new text are checked by `i18n/tests.rs` (every key looked up is
//! in the English catalog, no catalog has a stray key, placeholders match).

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use egui::accesskit::{Node, Role};
use egui::{Event, Modifiers, Pos2, pos2, vec2};
use figcraft_doc::{NodeId, NodeKind};
use figcraft_engine::capture::{CaptureBackend, CaptureError, CaptureJob, JobId};
use serde_json::{Value, json};

use crate::LeftTab;
use crate::layers_drag_tests::Ui;
use crate::live_tests::{flat, texts};

static NEXT: AtomicUsize = AtomicUsize::new(0);

const PACKAGE: &str = "synthetic-ui";
const VERSION: &str = "1.0.0";
const RAIL_WIDTH: f32 = 56.0;
const MAX_FRAMES: usize = 300;
const PLACE: &str = "uigen.placeImported";
const BLANK_HINT: &str = "Drop to make it the component of this page";
const REPLACE_HINT_START: &str = "Drop to replace ";
const COPIED: &str = "Copied Button into your library";
const LOCAL: &str = "Local part";

type Outcome = Result<Value, CaptureError>;
type Fingerprint = Vec<(NodeId, String, Vec<NodeId>)>;

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-import-drop-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
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

fn synthetic_button(style: &str, size: &str, state: &str, theme: &str) -> Value {
    let (width, height) = if size == "sm" { (80.0, 28.0) } else { (100.0, 36.0) };
    let (fill, ink) = match (style, state, theme) {
        ("solid", "hover", _) => ("rgb(21, 101, 192)", "rgb(255, 255, 255)"),
        ("solid", _, "dark") => ("rgb(144, 202, 249)", "rgb(0, 0, 0)"),
        ("solid", _, _) => ("rgb(25, 118, 210)", "rgb(255, 255, 255)"),
        (_, "hover", _) => ("rgba(25, 118, 210, 0.04)", "rgb(25, 118, 210)"),
        _ => ("rgba(0, 0, 0, 0)", "rgb(25, 118, 210)"),
    };
    json!({
        "version": 1,
        "viewport": { "width": 400, "height": 200, "deviceScaleFactor": 1 },
        "root": {
            "type": "element",
            "tag": "button",
            "label": "Button",
            "className": "syn-button",
            "box": { "x": 20, "y": 20, "width": width, "height": height },
            "style": { "backgroundColor": fill },
            "children": [{
                "type": "text",
                "text": "Button",
                "box": { "x": 30, "y": 26, "width": 40, "height": 16 },
                "lineBox": { "x": 30, "y": 26, "width": 40, "height": 16 },
                "style": { "color": ink, "fontSize": "14px" }
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

struct FakeBackend {
    submitted: u64,
    ready: Vec<(JobId, Outcome)>,
}

impl CaptureBackend for FakeBackend {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let outcome = match &job {
            CaptureJob::Inspect { .. } => Ok(inspected()),
            CaptureJob::Render { props, state, theme, .. } => {
                let style = props.get("variant").and_then(Value::as_str).unwrap_or("solid");
                let size = props.get("size").and_then(Value::as_str).unwrap_or("md");
                Ok(json!({"snapshot": synthetic_button(style, size, state, theme), "png": ".uigen/captures/synthetic.png"}))
            }
            CaptureJob::Install { package, version, .. } => {
                Ok(json!({"package": package, "version": version, "framework": "react", "license": "MIT"}))
            }
            CaptureJob::Uninstall { .. } | CaptureJob::Playground { .. } => Ok(json!({})),
        };
        self.submitted += 1;
        self.ready.push((self.submitted, outcome));
        self.submitted
    }

    fn poll(&mut self) -> Vec<(JobId, Outcome)> {
        std::mem::take(&mut self.ready)
    }

    fn shutdown(&mut self) {}
}

fn id_in(v: &Value, key: &str) -> NodeId {
    NodeId(v[key].as_u64().unwrap_or_else(|| panic!("no {key} in {v}")))
}

/// An open library project (blank page "Card") with `Button` imported, the Components tab showing.
struct Lab {
    ui: Ui,
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
        ui.app.session.set_capture_backend(Box::new(FakeBackend { submitted: 0, ready: Vec::new() }));
        ui.app.ui.home = false;
        ui.app.ui.toast = None;
        let mut lab = Self { ui, _dir: dir };
        lab.settle();
        lab.import_button();
        lab.ui.app.ui.left_tab = LeftTab::Assets;
        lab.ui.app.ui.left_collapsed = false;
        lab.settle();
        lab.settle();
        lab.ui.app.ui.toast = None;
        lab
    }

    fn import_button(&mut self) {
        let params = json!({"package": PACKAGE, "component": "Button", "states": ["Default"], "themes": ["light"], "stage": true});
        assert!(self.ui.app.run("uigen.importComponent", params).is_some(), "the import of Button starts");
        for _ in 0..MAX_FRAMES {
            self.ui.app.session.pump_capture();
            self.ui.frame(vec![]);
            if self.sets().iter().any(|(name, _)| name == "Button") {
                self.settle();
                return;
            }
        }
        panic!("Button never reached its import page");
    }

    fn settle(&mut self) {
        for _ in 0..3 {
            self.ui.frame(vec![]);
        }
    }

    fn run(&mut self, id: &str, params: Value) -> Value {
        self.ui.app.session.execute(id, &params).unwrap_or_else(|e| panic!("{id}: {e}"))
    }

    fn sets(&mut self) -> Vec<(String, NodeId)> {
        let sections = self.run("uigen.imported", json!({}))["sections"].as_array().cloned().unwrap_or_default();
        sections
            .iter()
            .filter_map(|s| s["components"].as_array())
            .flatten()
            .map(|c| (c["name"].as_str().unwrap_or_default().to_string(), id_in(c, "set")))
            .collect()
    }

    fn button_set(&mut self) -> NodeId {
        self.sets().into_iter().find(|(name, _)| name == "Button").map(|(_, id)| id).expect("Button is imported")
    }

    fn import_page(&mut self) -> NodeId {
        let sections = self.run("uigen.imported", json!({}))["sections"].as_array().cloned().unwrap_or_default();
        id_in(&sections[0], "page")
    }

    fn local_component(&mut self) -> NodeId {
        let frame = id_in(&self.run("frame.create", json!({"x": 0, "y": 0, "width": 120, "height": 60, "name": LOCAL})), "id");
        id_in(&self.run("component.create", json!({"ids": [frame]})), "id")
    }

    fn page(&self) -> NodeId {
        self.ui.app.session.page().unwrap()
    }

    fn page_name(&self) -> String {
        self.ui.app.session.doc().unwrap().node(self.page()).unwrap().name.clone()
    }

    fn fingerprint(&self) -> Fingerprint {
        let doc = self.ui.app.session.doc().unwrap();
        doc.pages
            .iter()
            .map(|page| {
                let node = doc.node(*page).unwrap();
                (*page, node.name.clone(), node.children.clone())
            })
            .collect()
    }

    fn mark(&self) -> usize {
        self.ui.app.session.journal.len()
    }

    fn ran_since(&self, mark: usize, command: &str) -> Vec<Value> {
        self.ui.app.session.journal[mark..].iter().filter(|(c, _)| c == command).map(|(_, p)| p.clone()).collect()
    }

    fn painted(&mut self, at: Pos2, modifiers: Modifiers) -> (Vec<String>, Vec<egui::Shape>) {
        let out = self.ui.frame_with(vec![Event::PointerMoved(at)], modifiers);
        let shapes = flat(&out.shapes);
        (texts(&shapes).into_iter().map(|(t, _, _)| t).collect(), shapes)
    }

    fn hints_at(&mut self, at: Pos2) -> Vec<String> {
        self.painted(at, Modifiers::NONE).0.into_iter().filter(|t| t.starts_with("Drop to make") || t.starts_with(REPLACE_HINT_START)).collect()
    }

    fn tile(&mut self, name: &str) -> Pos2 {
        self.ui.frame(vec![]);
        let width = RAIL_WIDTH + self.ui.app.ui.left_width;
        let out = self.ui.frame_output(vec![]);
        let (_, at, size) = texts(&flat(&out.shapes))
            .into_iter()
            .filter(|(t, p, _)| t == name && p.x >= RAIL_WIDTH && p.x < width)
            .max_by(|a, b| a.1.y.total_cmp(&b.1.y))
            .unwrap_or_else(|| panic!("no tile {name:?} in the panel"));
        at + vec2(8.0, size.y / 2.0)
    }

    fn over_canvas(&self) -> Pos2 {
        self.ui.app.canvas.rect.center()
    }

    fn off_canvas(&self) -> Pos2 {
        let rect = self.ui.app.canvas.rect;
        let at = pos2(rect.max.x + 30.0, rect.center().y);
        assert!(!rect.contains(at) && at.x < 1440.0, "a point beside the canvas: {at:?} vs {rect:?}");
        at
    }

    fn start_drag(&mut self, tile: &str, to: Pos2) {
        let from = self.tile(tile);
        self.ui.drag_to(from, to);
    }

    fn release(&mut self, at: Pos2) {
        self.ui.button(at, false);
        self.settle();
    }

    fn nodes(&mut self) -> Vec<Node> {
        self.ui.ctx.enable_accesskit();
        self.ui.frame(vec![]);
        self.ui.frame_output(vec![]).platform_output.accesskit_update.expect("an AccessKit update").nodes.into_iter().map(|(_, n)| n).collect()
    }

    fn undo_button(&mut self) -> Option<Pos2> {
        let mut found: Vec<Pos2> = self
            .nodes()
            .iter()
            .filter(|n| n.role() == Role::Button && n.label() == Some("Undo"))
            .filter_map(|n| n.bounds())
            .map(|b| pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32))
            .collect();
        assert!(found.len() <= 1, "one Undo button, found {}", found.len());
        found.pop()
    }

    fn click_undo(&mut self) {
        let at = self.undo_button().expect("a button named Undo in the toast");
        self.ui.button(at, true);
        self.ui.button(at, false);
        self.settle();
    }

    fn toast(&self) -> Option<&str> {
        self.ui.app.ui.toast.as_ref().map(|t| t.0.as_str())
    }

    fn action(&self) -> Option<(&str, &str)> {
        self.ui.app.ui.toast_action.as_ref().map(|a| (a.label.as_str(), a.command.as_str()))
    }
}

fn ghost_at(shapes: &[egui::Shape], at: Pos2) -> bool {
    shapes.iter().any(|s| matches!(s, egui::Shape::Rect(r) if (r.rect.width() - 64.0).abs() < 0.5 && (r.rect.height() - 64.0).abs() < 0.5 && r.rect.center().distance(at) < 0.5))
}

fn copies_on(lab: &Lab, page: NodeId) -> Vec<NodeId> {
    let doc = lab.ui.app.session.doc().unwrap();
    doc.node(page).unwrap().children.iter().copied().filter(|c| matches!(doc.node(*c).unwrap().kind, NodeKind::ComponentSet { .. })).collect()
}

// covers: E4-5.1
#[test]
fn over_the_canvas_of_a_blank_page_the_hint_says_the_tile_becomes_its_component_and_nothing_is_highlighted() {
    let mut lab = Lab::new("hint-blank");
    let (canvas, outside) = (lab.over_canvas(), lab.off_canvas());
    lab.start_drag("Button", outside);
    assert!(lab.hints_at(outside).is_empty(), "no hint beside the canvas: {:?}", lab.hints_at(outside));

    lab.painted(canvas, Modifiers::NONE);
    let (shown, _) = lab.painted(canvas, Modifiers::NONE);

    assert!(shown.iter().any(|t| t.contains(BLANK_HINT)), "the hint reads {BLANK_HINT:?}: {shown:?}");
    assert!(!shown.iter().any(|t| t.starts_with(REPLACE_HINT_START)), "not the replace hint: {shown:?}");
    let overlay = &lab.ui.app.session.tool.overlay;
    assert!(overlay.swap_target.is_none() && overlay.drop_target.is_none() && overlay.insert_line.is_none(), "nothing is highlighted");

    lab.painted(outside, Modifiers::NONE);
    assert!(lab.hints_at(outside).is_empty(), "the hint goes when the pointer leaves the canvas");
}

// covers: E4-5.1
#[test]
fn over_the_canvas_of_an_occupied_page_the_hint_names_the_page_and_no_instance_is_highlighted_even_with_alt() {
    let mut lab = Lab::new("hint-occupied");
    let component = lab.local_component();
    lab.settle();
    let canvas = lab.over_canvas();
    let at = lab.ui.app.to_page(canvas);
    lab.run("instance.place", json!({"component": component, "x": at.x, "y": at.y}));
    lab.settle();
    let name = lab.page_name();

    lab.start_drag("Button", canvas);
    let (shown, _) = lab.painted(canvas, Modifiers::NONE);

    let wanted = format!("{REPLACE_HINT_START}{name} on this page");
    assert!(shown.iter().any(|t| t.contains(&wanted)), "the hint reads {wanted:?}: {shown:?}");
    assert!(!shown.iter().any(|t| t.contains(BLANK_HINT)), "not the blank-page hint: {shown:?}");

    lab.painted(canvas, Modifiers::ALT);
    let (shown, _) = lab.painted(canvas, Modifiers::ALT);
    let overlay = &lab.ui.app.session.tool.overlay;
    assert!(overlay.swap_target.is_none(), "an instance is under the pointer, Alt is held, and none is highlighted: {:?}", overlay.swap_target);
    assert!(overlay.drop_target.is_none() && overlay.insert_line.is_none(), "no frame or slot either");
    assert!(shown.iter().any(|t| t.contains(&wanted)), "Alt does not change the hint: {shown:?}");
}

// covers: E4-5.2
#[test]
fn the_ghost_follows_the_pointer_and_releasing_beside_the_canvas_does_nothing() {
    let mut lab = Lab::new("ghost");
    let (canvas, outside) = (lab.over_canvas(), lab.off_canvas());
    let later = canvas + vec2(60.0, 40.0);
    let before = lab.fingerprint();
    let mark = lab.mark();

    lab.start_drag("Button", canvas);
    for at in [canvas, later, outside] {
        let (_, shapes) = lab.painted(at, Modifiers::NONE);
        assert!(ghost_at(&shapes, at), "the 64x64 ghost is centred on the pointer at {at:?}");
    }
    lab.ui.app.ui.toast = None;
    lab.release(outside);

    assert!(lab.ran_since(mark, PLACE).is_empty(), "nothing is placed");
    assert_eq!(lab.fingerprint(), before, "the document is unchanged");
    assert!(lab.toast().is_none(), "no toast: {:?}", lab.toast());
    let (_, shapes) = lab.painted(outside, Modifiers::NONE);
    assert!(!ghost_at(&shapes, outside), "the ghost goes with the drag");
}

// covers: E4-5.2
// covers: E4-5.4
#[test]
fn dropping_on_the_canvas_of_a_blank_page_places_the_set_once_and_the_toast_undoes_it() {
    let mut lab = Lab::new("drop-blank");
    let (set, page, canvas) = (lab.button_set(), lab.page(), lab.over_canvas());
    let (before, former) = (lab.fingerprint(), lab.page_name());
    let mark = lab.mark();

    lab.start_drag("Button", canvas);
    lab.painted(canvas, Modifiers::NONE);
    lab.release(canvas);

    let placed = lab.ran_since(mark, PLACE);
    assert_eq!(placed.len(), 1, "uigen.placeImported ran once: {placed:?}");
    assert_eq!(placed[0]["set"], json!(set.0), "for the dragged set");
    assert!(placed[0].get("page").is_none_or(|p| p.is_null() || *p == json!(page.0)), "on the open page: {:?}", placed[0]);
    assert_eq!(lab.page_name(), "Button", "the page took the name of the set");
    assert_eq!(copies_on(&lab, page).len(), 1, "with its copy");
    assert_eq!(lab.toast(), Some(COPIED));
    assert_eq!(lab.action(), Some(("Undo", "edit.undo")), "the toast offers Undo");
    assert!(lab.hints_at(canvas).is_empty(), "the hint is gone after the drop");

    lab.click_undo();

    assert_eq!(lab.fingerprint(), before, "Undo restored the page: layers and name {former:?}");
    assert!(lab.undo_button().is_none(), "and the button is gone");
}

// covers: E4-5.2
// covers: E4-5.3
#[test]
fn dropping_on_the_canvas_of_an_occupied_page_replaces_its_content_and_undo_brings_it_back() {
    let mut lab = Lab::new("drop-occupied");
    let component = lab.local_component();
    lab.settle();
    let (set, page, canvas) = (lab.button_set(), lab.page(), lab.over_canvas());
    let (before, former) = (lab.fingerprint(), lab.page_name());
    let mark = lab.mark();

    lab.start_drag("Button", canvas);
    lab.painted(canvas, Modifiers::NONE);
    lab.release(canvas);

    let placed = lab.ran_since(mark, PLACE);
    assert_eq!(placed.len(), 1, "uigen.placeImported ran once: {placed:?}");
    assert_eq!(placed[0]["set"], json!(set.0));
    assert_eq!(lab.toast(), Some(format!("{former} replaced by Button").as_str()), "the toast names both");
    assert_eq!(lab.action(), Some(("Undo", "edit.undo")));
    let after = lab.fingerprint();
    assert_eq!(after.len(), before.len() + 1, "the former content has a page of its own");
    assert!(lab.ui.app.session.doc().unwrap().node(component).is_some_and(|n| n.parent != Some(page)), "the local component left the page");

    lab.click_undo();

    assert_eq!(lab.fingerprint(), before, "Undo restored the content, the name {former:?} and removed the archive page");
    assert_eq!(lab.page_name(), former);
}

// covers: E4-5.3
#[test]
fn the_undo_action_goes_as_soon_as_another_change_passes() {
    let mut lab = Lab::new("action-goes");
    lab.local_component();
    lab.settle();
    let canvas = lab.over_canvas();

    lab.start_drag("Button", canvas);
    lab.painted(canvas, Modifiers::NONE);
    lab.release(canvas);
    assert!(lab.undo_button().is_some(), "the button shows after the drop");

    lab.run("shape.rectangle", json!({"x": 400, "y": 400, "width": 30, "height": 30}));
    lab.settle();

    assert!(lab.action().is_none(), "another change removes the action");
    assert!(lab.undo_button().is_none(), "and the button");
}

// covers: E4-5.4
#[test]
fn a_drop_the_engine_refuses_shows_its_error_and_leaves_the_document_alone() {
    let mut lab = Lab::new("refused");
    let (set, page) = (lab.button_set(), lab.import_page());
    lab.run("page.select", json!({"id": page}));
    lab.settle();
    let expected = lab.ui.app.session.execute(PLACE, &json!({"set": set})).expect_err("an import page is refused").to_string();
    let (before, canvas) = (lab.fingerprint(), lab.over_canvas());
    lab.ui.app.ui.last_error = None;

    lab.start_drag("Button", canvas);
    lab.painted(canvas, Modifiers::NONE);
    lab.release(canvas);

    assert_eq!(lab.toast(), Some(expected.as_str()), "the engine's own message is the toast");
    assert_eq!(lab.ui.app.ui.last_error.as_deref(), Some(expected.as_str()));
    assert!(lab.action().is_none(), "an error offers no Undo");
    assert_eq!(lab.fingerprint(), before, "the document is identical");
    assert_eq!(lab.page(), page, "still on the import page");
    lab.settle();
}

// covers: E4-5.5
#[test]
fn a_local_tile_keeps_its_drop_an_instance_and_no_import_hint() {
    let mut lab = Lab::new("local");
    let component = lab.local_component();
    lab.settle();
    let canvas = lab.over_canvas();
    let mark = lab.mark();

    lab.start_drag(LOCAL, canvas);
    let (shown, shapes) = lab.painted(canvas, Modifiers::NONE);
    assert!(ghost_at(&shapes, canvas), "the ghost follows the pointer");
    assert!(!shown.iter().any(|t| t.starts_with("Drop to make") || t.starts_with(REPLACE_HINT_START)), "no import hint: {shown:?}");
    lab.release(canvas);

    let placed = lab.ran_since(mark, "instance.place");
    assert_eq!(placed.len(), 1, "one instance: {placed:?}");
    assert_eq!(placed[0]["component"], json!(component.0), "of the dragged component");
    assert!(lab.ran_since(mark, PLACE).is_empty(), "uigen.placeImported is for imported tiles only");
}
