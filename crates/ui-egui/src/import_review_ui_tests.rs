//! Review fixes of the import interface: the window follows only its own import and its own project
//! tab, a version the service did not read is said in the user's language, the "Copy to the current
//! page" menu item says what it did with an "Undo" like a drop, and the Components tab keeps its
//! import button, its drop hint and its list honest. The service is a scripted backend; the answers
//! are synthetic.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use egui::accesskit::{Node, Role};
use egui::{Event, Key, Modifiers, Pos2, pos2, vec2};
use figcraft_doc::NodeId;
use figcraft_engine::capture::{CaptureBackend, CaptureError, CaptureJob, JobId};
use serde_json::{Value, json};

use crate::import_report_tests::shown_texts;
use crate::layers_drag_tests::Ui;
use crate::live_tests::{flat, texts};
use crate::{LeftTab, RightTab};

static NEXT: AtomicUsize = AtomicUsize::new(0);

const PACKAGE: &str = "synthetic-ui";
const VERSION: &str = "1.0.0";
const MAX_FRAMES: usize = 300;
const RAIL_WIDTH: f32 = 56.0;
const IMPORT_BUTTON: &str = "Import a component…";
const COPY: &str = "Copy to the current page";
const IMPORTED_AFTER_ALERT: &str = "Imported a component from synthetic-ui";

type Outcome = Result<Value, CaptureError>;

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-import-review-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
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

fn snapshot() -> Value {
    json!({
        "version": 1,
        "viewport": { "width": 400, "height": 200, "deviceScaleFactor": 1 },
        "root": {
            "type": "element", "tag": "button", "label": "Button", "className": "syn-button",
            "box": { "x": 20, "y": 20, "width": 100, "height": 36 },
            "style": { "backgroundColor": "rgb(25, 118, 210)" },
            "children": [{
                "type": "text", "text": "Button",
                "box": { "x": 30, "y": 26, "width": 40, "height": 16 },
                "lineBox": { "x": 30, "y": 26, "width": 40, "height": 16 },
                "style": { "color": "rgb(255, 255, 255)", "fontSize": "14px" }
            }]
        }
    })
}

struct Script {
    hold: bool,
    allowed: usize,
    version: String,
    names: Vec<String>,
    submitted: Vec<CaptureJob>,
    queue: Vec<(JobId, Outcome)>,
}

struct Backend(Arc<Mutex<Script>>);

impl CaptureBackend for Backend {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let mut script = self.0.lock().unwrap();
        let outcome = match &job {
            CaptureJob::Inspect { .. } => {
                let props = json!([{"name": "label", "type": "string", "values": null}]);
                let components: Vec<Value> = script.names.iter().map(|name| json!({"name": name, "props": props})).collect();
                Ok(json!({"package": PACKAGE, "version": script.version, "framework": "react", "components": components, "notes": []}))
            }
            CaptureJob::Render { .. } => Ok(json!({"snapshot": snapshot(), "png": ".uigen/captures/synthetic.png"})),
            CaptureJob::Install { package, version, .. } => {
                Ok(json!({"package": package, "version": version, "framework": "react", "license": "MIT"}))
            }
            CaptureJob::InstallMany { .. } | CaptureJob::Uninstall { .. } | CaptureJob::Playground { .. } => Ok(json!({})),
        };
        script.submitted.push(job);
        let id = u64::try_from(script.submitted.len()).unwrap();
        script.queue.push((id, outcome));
        id
    }

    fn poll(&mut self) -> Vec<(JobId, Outcome)> {
        let mut script = self.0.lock().unwrap();
        let count = if script.hold { script.allowed.min(script.queue.len()) } else { script.queue.len() };
        script.allowed -= if script.hold { count } else { 0 };
        script.queue.drain(..count).collect()
    }

    fn shutdown(&mut self) {}
}

fn write_project(dir: &Dir) {
    let sources = json!([{"package": PACKAGE, "version": VERSION, "framework": "react", "license": "MIT", "removed": false}]);
    let manifest = json!({"version": 1, "name": "mylib", "designFiles": ["lib.figcraft"], "sources": sources});
    std::fs::write(dir.0.join("uigen.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    let mut doc = figcraft_doc::Document::new("lib");
    let page = doc.pages[0];
    doc.node_mut(page).unwrap().name = "Card".to_string();
    std::fs::write(dir.0.join("lib.figcraft"), figcraft_format::save_bytes(&doc, None).unwrap()).unwrap();
}

struct Lab {
    ui: Ui,
    script: Arc<Mutex<Script>>,
    dirs: Vec<Dir>,
}

impl Lab {
    fn new(tag: &str, language: &str) -> Self {
        figcraft_text::scan_system_fonts();
        let dir = Dir::new(tag);
        write_project(&dir);
        let mut ui = Ui::new(figcraft_engine::Session::empty());
        ui.app.session.prefs.language = language.into();
        assert!(ui.app.open_project_at(dir.0.clone(), false), "the project opens");
        let names = vec!["Alert".to_string(), "Chip".to_string()];
        let script =
            Arc::new(Mutex::new(Script { hold: false, allowed: 0, version: VERSION.to_string(), names, submitted: Vec::new(), queue: Vec::new() }));
        ui.app.session.set_capture_backend(Box::new(Backend(Arc::clone(&script))));
        ui.app.ui.home = false;
        ui.app.ui.toast = None;
        ui.app.ui.left_collapsed = false;
        let mut lab = Self { ui, script, dirs: vec![dir] };
        lab.settle();
        lab
    }

    fn open_other_project(&mut self, tag: &str) {
        let dir = Dir::new(tag);
        write_project(&dir);
        assert!(self.ui.app.open_project_at(dir.0.clone(), false), "the second project opens");
        self.dirs.push(dir);
        self.settle();
    }

    fn settle(&mut self) {
        for _ in 0..3 {
            self.ui.frame(vec![]);
        }
    }

    fn hold(&mut self) {
        self.script.lock().unwrap().hold = true;
    }

    fn release(&mut self, count: usize) {
        self.script.lock().unwrap().allowed += count;
        self.settle();
    }

    fn release_all(&mut self) {
        self.script.lock().unwrap().hold = false;
        self.settle();
    }

    fn inspects(&self) -> usize {
        self.script.lock().unwrap().submitted.iter().filter(|job| matches!(job, CaptureJob::Inspect { .. })).count()
    }

    fn run(&mut self, id: &str, params: Value) -> Value {
        self.ui.app.session.execute(id, &params).unwrap_or_else(|e| panic!("{id}: {e}"))
    }

    fn mark(&self) -> usize {
        self.ui.app.session.journal.len()
    }

    fn ran_since(&self, mark: usize, command: &str) -> Vec<Value> {
        self.ui.app.session.journal[mark..].iter().filter(|(c, _)| c == command).map(|(_, p)| p.clone()).collect()
    }

    fn open_window(&mut self) {
        assert!(self.ui.app.run("uigen.importWindow", json!({"package": PACKAGE})).is_some());
        self.settle();
    }

    fn shown(&mut self) -> Vec<String> {
        self.ui.frame(vec![]);
        shown_texts(self.ui.frame_output(vec![]).shapes)
    }

    fn shows(&mut self, needle: &str) -> bool {
        self.shown().iter().any(|text| text.contains(needle))
    }

    fn wait_for(&mut self, needle: &str) {
        for _ in 0..MAX_FRAMES {
            if self.shows(needle) {
                self.settle();
                return;
            }
        }
        panic!("{needle:?} never shown; on screen: {:?}", self.shown());
    }

    fn nodes(&mut self) -> Vec<Node> {
        self.ui.ctx.enable_accesskit();
        self.ui.frame(vec![]);
        self.ui.frame_output(vec![]).platform_output.accesskit_update.expect("an AccessKit update").nodes.into_iter().map(|(_, n)| n).collect()
    }

    fn centres_of(&mut self, role: Option<Role>, label: &str) -> Vec<Pos2> {
        let mut found: Vec<Pos2> = self
            .nodes()
            .iter()
            .filter(|n| role.map_or(n.role() != Role::Label, |role| n.role() == role) && n.label().is_some_and(|l| l.trim() == label))
            .filter_map(|n| n.bounds())
            .map(|b| pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32))
            .collect();
        found.sort_by(|a, b| a.y.total_cmp(&b.y));
        found
    }

    fn click_at(&mut self, at: Pos2) {
        self.ui.button(at, true);
        self.ui.button(at, false);
        self.settle();
    }

    fn click(&mut self, label: &str) {
        let at = *self.centres_of(None, label).first().unwrap_or_else(|| panic!("no control named {label:?}"));
        self.click_at(at);
    }

    fn right_click(&mut self, at: Pos2) {
        for pressed in [true, false] {
            self.ui.frame(vec![
                Event::PointerMoved(at),
                Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: Modifiers::NONE },
            ]);
        }
        self.settle();
    }

    fn press(&mut self, key: Key) {
        let modifiers = Modifiers::NONE;
        self.ui.frame(vec![
            Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers },
            Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers },
        ]);
        self.settle();
    }

    fn progress(&mut self) -> Option<(usize, usize)> {
        let line = self.shown().into_iter().find_map(|text| text.strip_prefix("Importing… ").map(str::to_string))?;
        let (done, total) = line.split_once(" of ")?;
        Some((done.trim().parse().ok()?, total.trim().parse().ok()?))
    }

    fn toast(&self) -> Option<String> {
        self.ui.app.ui.toast.as_ref().map(|t| t.0.clone())
    }

    fn import_now(&mut self, component: &str) {
        let params = json!({"package": PACKAGE, "component": component, "states": ["Default"], "themes": ["light"], "stage": true});
        assert!(self.ui.app.run("uigen.importComponent", params).is_some(), "the import of {component} starts");
        for _ in 0..MAX_FRAMES {
            self.ui.app.session.pump_capture();
            self.ui.frame(vec![]);
            if self.imported_set(component).is_some() {
                self.settle();
                return;
            }
        }
        panic!("{component} never reached its import page");
    }

    fn imported_set(&mut self, name: &str) -> Option<NodeId> {
        let sections = self.run("uigen.imported", json!({}))["sections"].as_array().cloned().unwrap_or_default();
        let found = sections.iter().filter_map(|s| s["components"].as_array()).flatten().find(|c| c["name"] == name)?.clone();
        found["set"].as_u64().map(NodeId)
    }

    fn show_components(&mut self) {
        self.ui.app.ui.left_tab = LeftTab::Assets;
        self.ui.app.ui.left_collapsed = false;
        self.settle();
        self.settle();
    }

    fn panel_texts(&mut self) -> Vec<(String, Pos2, egui::Vec2)> {
        self.ui.frame(vec![]);
        let width = RAIL_WIDTH + self.ui.app.ui.left_width;
        let out = self.ui.frame_output(vec![]);
        texts(&flat(&out.shapes)).into_iter().filter(|(_, p, _)| p.x >= RAIL_WIDTH && p.x < width).collect()
    }

    fn tile(&mut self, name: &str) -> Pos2 {
        let (_, at, size) = self
            .panel_texts()
            .into_iter()
            .filter(|(t, _, _)| t == name)
            .max_by(|a, b| a.1.y.total_cmp(&b.1.y))
            .unwrap_or_else(|| panic!("no tile {name:?} in the panel"));
        at + vec2(8.0, size.y / 2.0)
    }

    fn copy_from_menu(&mut self, name: &str) {
        let at = self.tile(name);
        self.right_click(at);
        let item = *self.centres_of(Some(Role::MenuItem), COPY).first().unwrap_or_else(|| panic!("no menu item {COPY:?}"));
        self.click_at(item);
    }

    fn drop_hints(&mut self, tile: &str) -> Vec<String> {
        let from = self.tile(tile);
        let over = self.ui.app.canvas.rect.center();
        self.ui.drag_to(from, over);
        let hints =
            shown_texts(self.ui.frame_output(vec![Event::PointerMoved(over)]).shapes).into_iter().filter(|t| t.starts_with("Drop to")).collect();
        self.ui.button(over, false);
        self.settle();
        hints
    }
}

#[test]
fn the_end_of_another_import_neither_ends_nor_advances_the_import_the_window_follows() {
    let mut lab = Lab::new("other-import", "en");
    lab.hold();
    lab.open_window();
    lab.release(1);
    lab.wait_for("Chip");
    lab.click("Alert");
    lab.click("Import");
    lab.press(Key::Escape);
    assert!(!lab.shows("Import a component"), "the window is closed");

    lab.open_window();
    lab.wait_for("Chip");
    lab.click("Chip");
    lab.click("Import");
    lab.release(2);
    lab.release(8);

    assert!(!lab.shows("Imported Chip"), "Alert ended, Chip did not: {:?}", lab.shown());
    let (done, total) = lab.progress().expect("Chip is still running");
    assert!(done <= 1 && total == 9, "only the jobs of Chip count: {done} of {total}");
    assert_eq!(lab.toast().as_deref(), Some(IMPORTED_AFTER_ALERT), "the end of Alert falls back to the toast");

    lab.release_all();
    assert!(lab.shows("Imported Chip"), "{:?}", lab.shown());
}

#[test]
fn a_project_with_the_same_source_in_another_tab_has_its_own_reading() {
    let mut lab = Lab::new("tab-a", "en");
    lab.open_window();
    lab.wait_for("Chip");
    assert_eq!(lab.inspects(), 1);

    lab.open_other_project("tab-b");
    lab.wait_for("Chip");
    assert_eq!(lab.inspects(), 2, "the second project is read on its own");

    lab.ui.app.session.active = 0;
    lab.wait_for("Chip");
    assert_eq!(lab.inspects(), 2, "the first project is still known");
}

#[test]
fn a_reading_of_another_version_is_said_in_the_language_of_the_user() {
    let mut lab = Lab::new("version", "fr");
    lab.script.lock().unwrap().version = "2.0.0".to_string();
    lab.open_window();
    lab.wait_for("La version lue n’est pas celle que demande le projet");
    assert!(!lab.shows("the version read"), "{:?}", lab.shown());
}

#[test]
fn only_the_visible_rows_of_a_long_list_are_drawn() {
    let mut lab = Lab::new("long-list", "en");
    lab.script.lock().unwrap().names = (1..=300).map(|n| format!("Widget{n:03}")).collect();
    lab.open_window();
    lab.wait_for("Widget001");
    let drawn = lab.shown().iter().filter(|text| text.starts_with("Widget")).count();
    assert!((1..=12).contains(&drawn), "{drawn} rows drawn");
}

#[test]
fn dev_mode_refuses_the_import_from_the_window() {
    let mut lab = Lab::new("dev-mode", "en");
    lab.open_window();
    lab.wait_for("Chip");
    lab.click("Chip");
    lab.ui.app.ui.right_tab = RightTab::Inspect;
    lab.settle();
    let mark = lab.mark();
    lab.click("Import");
    assert!(lab.ran_since(mark, "uigen.importComponent").is_empty(), "the command never ran");
    assert_eq!(lab.toast().as_deref(), Some("Dev Mode only inspects: switch to Design (Shift+D) to edit"));
}

#[test]
fn the_copy_menu_item_says_what_it_did_with_an_undo() {
    let mut lab = Lab::new("copy-toast", "en");
    lab.import_now("Chip");
    lab.show_components();

    lab.copy_from_menu("Chip");
    assert_eq!(lab.toast().as_deref(), Some("Copied Chip into your library"));
    assert!(lab.ui.app.ui.toast_action.as_ref().is_some_and(|action| action.command == "edit.undo"), "an Undo is offered");

    lab.copy_from_menu("Chip");
    assert!(lab.toast().is_some_and(|text| text.contains("replaced by")), "{:?}", lab.toast());
}

#[test]
fn a_source_that_is_removed_has_no_import_button_in_its_section() {
    let mut lab = Lab::new("removed", "en");
    lab.import_now("Chip");
    lab.show_components();
    assert_eq!(lab.centres_of(Some(Role::Button), IMPORT_BUTTON).len(), 2, "the search row and the section");

    lab.run("project.removeSource", json!({"package": PACKAGE}));
    for _ in 0..MAX_FRAMES {
        lab.ui.app.session.pump_capture();
        lab.ui.frame(vec![]);
    }
    lab.show_components();
    assert_eq!(lab.centres_of(Some(Role::Button), IMPORT_BUTTON).len(), 1, "only the search row is left");
}

#[test]
fn the_drop_hint_is_not_shown_over_an_import_page_or_a_divider() {
    let mut lab = Lab::new("hint", "en");
    lab.import_now("Chip");
    lab.show_components();
    let card = lab.ui.app.session.page().unwrap();
    lab.ui.app.session.ds_mut().unwrap().doc_mut().node_mut(card).unwrap().name = "- Divider".to_string();
    lab.settle();
    assert!(lab.drop_hints("Chip").is_empty(), "no hint over a divider");

    let import_page = lab.run("uigen.imported", json!({}))["sections"][0]["page"].as_u64().map(NodeId).unwrap();
    lab.run("page.select", json!({"id": import_page.0}));
    lab.settle();
    assert!(lab.drop_hints("Chip").is_empty(), "no hint over an import page");

    lab.ui.app.session.ds_mut().unwrap().doc_mut().node_mut(card).unwrap().name = "Card".to_string();
    lab.run("page.select", json!({"id": card.0}));
    lab.settle();
    assert_eq!(lab.drop_hints("Chip").len(), 1, "a hint over an ordinary page");
}
