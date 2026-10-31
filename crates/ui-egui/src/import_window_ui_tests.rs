//! The "Import a component" window (`dialog.importComponent`, asked by `uigen.importWindow`): it
//! lists the components of a source library, sets the import options from the props of the one
//! chosen, runs `uigen.importComponent` with `stage: true` and follows the capture jobs until the
//! set is in the import page. A user opens it from File, the Actions palette or the Components
//! tab; a programmatic call never does; Escape closes it. Every state of the list (loading, ready,
//! error, empty, no source) and of the import (running, imported, failed) is drawn.
//!
//! The service is a scripted backend that can hold its answers back (`hold` / `release`), so a
//! test sees the window while a job is still running. The answers are synthetic.
//!
//! The 22 catalogs: `i18n/tests.rs` checks that every key the code looks up is in the English
//! catalog and that no catalog has a stray key or a wrong placeholder; it does not fail on a missing
//! translation, so the last test here fails on any language that lacks a text of the window.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use egui::accesskit::{Node, Role, Toggled};
use egui::{Event, Key, Modifiers, Pos2, pos2};
use figcraft_doc::NodeId;
use figcraft_engine::capture::{CaptureBackend, CaptureError, CaptureErrorKind, CaptureJob, JobId, JobState};
use figcraft_engine::catalog::MENU;
use figcraft_engine::{MAX_RENDERS, render_count};
use serde_json::{Value, json};

use crate::LeftTab;
use crate::a11y::focused_name;
use crate::i18n::{self, Lang, Text};
use crate::import_report_tests::shown_texts;
use crate::layers_drag_tests::Ui;

static NEXT: AtomicUsize = AtomicUsize::new(0);

const PACKAGE: &str = "synthetic-ui";
const VERSION: &str = "1.0.0";
const OTHER: &str = "other-ui";
const OTHER_VERSION: &str = "2.0.0";
const GONE: &str = "gone-ui";
const TITLE: &str = "Import a component";
const WINDOW: &str = "uigen.importWindow";
const LIST: &str = "uigen.listComponents";
const IMPORT: &str = "uigen.importComponent";
const DIALOG: &str = "dialog.importComponent";
const PALETTE_ROW: &str = "command:uigen.importWindow";
const MAX_FRAMES: usize = 300;
const STATES: [&str; 4] = ["Default", "Hover", "Pressed", "Disabled"];

type Outcome = Result<Value, CaptureError>;
type Fingerprint = Vec<(NodeId, String, Vec<NodeId>)>;

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-import-window-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
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

fn enum_prop(name: &str, values: &[&str]) -> Value {
    json!({"name": name, "type": "enum", "values": values})
}

fn plain_prop(name: &str, kind: &str) -> Value {
    json!({"name": name, "type": kind, "values": null})
}

fn numbered(prefix: &str, count: usize) -> Vec<String> {
    (1..=count).map(|n| format!("{prefix}{n}")).collect()
}

fn component(name: &str) -> Value {
    let props = match name {
        "Alert" => vec![plain_prop("title", "string")],
        "Button" => vec![enum_prop("variant", &["solid", "outline"]), enum_prop("size", &["sm", "md"]), plain_prop("disabled", "boolean")],
        "Loader" => vec![enum_prop("variant", &["solid", "outline"]), plain_prop("loading", "boolean")],
        "Chip" => vec![plain_prop("label", "string")],
        "Big" => {
            let (styles, sizes) = (numbered("v", 10), numbered("s", 4));
            vec![
                enum_prop("variant", &styles.iter().map(String::as_str).collect::<Vec<_>>()),
                enum_prop("size", &sizes.iter().map(String::as_str).collect::<Vec<_>>()),
            ]
        }
        "Edge" => {
            let (styles, sizes) = (numbered("a", 4), numbered("b", 4));
            vec![
                enum_prop("variant", &styles.iter().map(String::as_str).collect::<Vec<_>>()),
                enum_prop("size", &sizes.iter().map(String::as_str).collect::<Vec<_>>()),
            ]
        }
        _ => vec![plain_prop("label", "string")],
    };
    json!({"name": name, "props": props})
}

fn inspect_of(package: &str, version: &str, names: &[&str], notes: &[&str]) -> Outcome {
    let components: Vec<Value> = names.iter().map(|name| component(name)).collect();
    Ok(json!({"package": package, "version": version, "framework": "react", "components": components, "notes": notes}))
}

fn failure(message: &str) -> CaptureError {
    CaptureError { kind: CaptureErrorKind::Protocol, message: message.to_string() }
}

fn source(package: &str, version: &str, removed: bool) -> Value {
    json!({"package": package, "version": version, "framework": "react", "license": "MIT", "removed": removed})
}

struct FailingRender {
    state: &'static str,
    theme: &'static str,
    error: CaptureError,
}

struct Shared {
    submitted: Vec<CaptureJob>,
    queue: Vec<(JobId, Outcome)>,
    hold: bool,
    allowed: usize,
    inspects: Vec<(String, Outcome)>,
    failing_render: Option<FailingRender>,
}

struct FakeBackend {
    shared: Arc<Mutex<Shared>>,
}

impl CaptureBackend for FakeBackend {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let mut shared = self.shared.lock().unwrap();
        let outcome = match &job {
            CaptureJob::Inspect { package, .. } => {
                let scripted = shared.inspects.iter().find(|(name, _)| name == package).map(|(_, outcome)| outcome.clone());
                scripted.unwrap_or_else(|| Err(failure(&format!("{package} is not installed"))))
            }
            CaptureJob::Render { props, state, theme, .. } => match &shared.failing_render {
                Some(failing) if failing.state == state && failing.theme == theme => Err(failing.error.clone()),
                _ => {
                    let style = props.get("variant").and_then(Value::as_str).unwrap_or("solid");
                    let size = props.get("size").and_then(Value::as_str).unwrap_or("md");
                    Ok(json!({"snapshot": synthetic_button(style, size, state, theme), "png": ".uigen/captures/synthetic.png"}))
                }
            },
            CaptureJob::Install { package, version, .. } => {
                Ok(json!({"package": package, "version": version, "framework": "react", "license": "MIT"}))
            }
            CaptureJob::InstallMany { .. } | CaptureJob::Uninstall { .. } | CaptureJob::Playground { .. } => Ok(json!({})),
        };
        shared.submitted.push(job);
        let id = u64::try_from(shared.submitted.len()).unwrap();
        shared.queue.push((id, outcome));
        id
    }

    fn poll(&mut self) -> Vec<(JobId, Outcome)> {
        let mut shared = self.shared.lock().unwrap();
        let count = if shared.hold { shared.allowed.min(shared.queue.len()) } else { shared.queue.len() };
        shared.allowed -= if shared.hold { count } else { 0 };
        shared.queue.drain(..count).collect()
    }

    fn shutdown(&mut self) {}
}

fn write_project(dir: &Dir, sources: &[Value]) {
    let manifest = json!({"version": 1, "name": "mylib", "designFiles": ["lib.figcraft"], "sources": sources});
    std::fs::write(dir.0.join("uigen.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    let mut doc = figcraft_doc::Document::new("lib");
    let page = doc.pages[0];
    doc.node_mut(page).unwrap().name = "Card".to_string();
    std::fs::write(dir.0.join("lib.figcraft"), figcraft_format::save_bytes(&doc, None).unwrap()).unwrap();
}

/// A library project open in the app, its capture service scripted by the test.
struct Bench {
    ui: Ui,
    shared: Arc<Mutex<Shared>>,
    _dir: Dir,
}

impl Bench {
    fn with(tag: &str, sources: &[Value], inspects: Vec<(String, Outcome)>) -> Self {
        figcraft_text::scan_system_fonts();
        let dir = Dir::new(tag);
        write_project(&dir, sources);
        let mut ui = Ui::new(figcraft_engine::Session::empty());
        ui.app.session.prefs.language = "en".into();
        assert!(ui.app.open_project_at(dir.0.clone(), false), "the project opens");
        let shared =
            Arc::new(Mutex::new(Shared { submitted: Vec::new(), queue: Vec::new(), hold: false, allowed: 0, inspects, failing_render: None }));
        ui.app.session.set_capture_backend(Box::new(FakeBackend { shared: Arc::clone(&shared) }));
        ui.app.ui.toast = None;
        ui.app.ui.home = false;
        let mut bench = Self { ui, shared, _dir: dir };
        bench.settle();
        bench
    }

    /// One source, `PACKAGE`, that exports `names`.
    fn nominal(tag: &str, names: &[&str]) -> Self {
        Self::with(tag, &[source(PACKAGE, VERSION, false)], vec![(PACKAGE.to_string(), inspect_of(PACKAGE, VERSION, names, &[]))])
    }

    fn hold(&mut self) {
        self.shared.lock().unwrap().hold = true;
    }

    /// Let the service answer the next `count` jobs (and go on holding the others back).
    fn release(&mut self, count: usize) {
        self.shared.lock().unwrap().allowed += count;
        self.settle();
    }

    fn release_all(&mut self) {
        self.shared.lock().unwrap().hold = false;
        self.settle();
    }

    fn set_inspect(&mut self, package: &str, outcome: Outcome) {
        let mut shared = self.shared.lock().unwrap();
        shared.inspects.retain(|(name, _)| name != package);
        shared.inspects.push((package.to_string(), outcome));
    }

    fn fail_render(&mut self, state: &'static str, theme: &'static str, message: &str) {
        self.shared.lock().unwrap().failing_render = Some(FailingRender { state, theme, error: failure(message) });
    }

    fn submitted(&self) -> Vec<CaptureJob> {
        self.shared.lock().unwrap().submitted.clone()
    }

    fn inspected_packages(&self) -> Vec<String> {
        self.submitted()
            .into_iter()
            .filter_map(|job| match job {
                CaptureJob::Inspect { package, .. } => Some(package),
                _ => None,
            })
            .collect()
    }

    fn settle(&mut self) {
        for _ in 0..3 {
            self.ui.frame(vec![]);
        }
    }

    fn wait_until(&mut self, what: &str, done: impl Fn(&mut Self) -> bool) {
        for _ in 0..MAX_FRAMES {
            self.ui.frame(vec![]);
            if done(self) {
                self.settle();
                return;
            }
        }
        panic!("never happened: {what}; on screen: {:?}", self.shown());
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

    fn open(&mut self, package: Option<&str>) {
        let params = package.map_or_else(|| json!({}), |package| json!({"package": package}));
        assert!(self.ui.app.run(WINDOW, params).is_some(), "{WINDOW} runs");
        self.settle();
    }

    fn shown(&mut self) -> Vec<String> {
        self.ui.frame(vec![]);
        shown_texts(self.ui.frame_output(vec![]).shapes)
    }

    fn shows(&mut self, needle: &str) -> bool {
        self.shown().iter().any(|text| text.contains(needle))
    }

    fn window_open(&mut self) -> bool {
        self.shown().iter().any(|text| text == TITLE)
    }

    fn nodes(&mut self) -> Vec<Node> {
        self.ui.ctx.enable_accesskit();
        self.ui.frame(vec![]);
        self.ui.frame_output(vec![]).platform_output.accesskit_update.expect("an AccessKit update").nodes.into_iter().map(|(_, n)| n).collect()
    }

    fn control(&mut self, label: &str) -> Option<Node> {
        self.nodes().into_iter().find(|n| n.role() != Role::Label && n.label().is_some_and(|l| l.trim() == label))
    }

    fn has(&mut self, label: &str) -> bool {
        self.control(label).is_some()
    }

    fn centre_of(&mut self, label: &str) -> Pos2 {
        let node = self.control(label).unwrap_or_else(|| panic!("no control named {label:?} on screen"));
        let b = node.bounds().unwrap_or_else(|| panic!("{label:?} has no bounds"));
        pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
    }

    fn click(&mut self, label: &str) {
        let at = self.centre_of(label);
        self.ui.button(at, true);
        self.ui.button(at, false);
        self.settle();
    }

    fn checked(&mut self, label: &str) -> bool {
        let node = self.control(label).unwrap_or_else(|| panic!("no control named {label:?} on screen"));
        match node.toggled() {
            Some(Toggled::True) => true,
            Some(Toggled::False) => false,
            other => panic!("{label:?} is not a checkbox: {other:?}"),
        }
    }

    fn press(&mut self, key: Key, modifiers: Modifiers) {
        self.ui.frame(vec![
            Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers },
            Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers },
        ]);
        self.settle();
    }

    fn type_text(&mut self, text: &str) {
        self.ui.frame(vec![Event::Text(text.into())]);
        self.settle();
    }

    fn toast(&self) -> Option<String> {
        self.ui.app.ui.toast.as_ref().map(|t| t.0.clone())
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

    fn imported(&mut self) -> Vec<(String, u64)> {
        let sections = self.run("uigen.imported", json!({}))["sections"].as_array().cloned().unwrap_or_default();
        sections
            .iter()
            .filter_map(|s| s["components"].as_array())
            .flatten()
            .map(|c| (c["name"].as_str().unwrap_or_default().to_string(), c["variants"].as_u64().unwrap_or_default()))
            .collect()
    }

    /// `(done, total)` of the "Importing… {done} of {total}" line on screen.
    fn progress(&mut self) -> Option<(usize, usize)> {
        let line = self.shown().into_iter().find_map(|text| text.strip_prefix("Importing… ").map(str::to_string))?;
        let (done, total) = line.split_once(" of ")?;
        Some((done.trim().parse().ok()?, total.trim().parse().ok()?))
    }

    /// Opens the window on `PACKAGE`, waits for the components and chooses `name`.
    fn open_and_choose(&mut self, name: &str) {
        self.open(Some(PACKAGE));
        self.wait_until("the components are listed", |b| b.has(name));
        self.click(name);
    }
}

fn mentions(shown: &[String], needle: &str) -> bool {
    shown.iter().any(|text| text.contains(needle))
}

fn variants_line(styles: usize, sizes: usize, states: usize, themes: usize) -> String {
    format!("{} variants · {} renders", render_count(styles, sizes, states, 1), render_count(styles, sizes, states, themes))
}

fn sorted(value: &Value) -> Vec<String> {
    let mut items: Vec<String> =
        value.as_array().map(|items| items.iter().filter_map(|item| item.as_str().map(str::to_string)).collect()).unwrap_or_default();
    items.sort();
    items
}

fn strings(items: &[&str]) -> Vec<String> {
    let mut items: Vec<String> = items.iter().map(ToString::to_string).collect();
    items.sort();
    items
}

// Harness: what the tests below rest on, checked against the engine as it is today.

#[test]
fn harness_the_window_command_asks_for_the_dialog_and_the_control_channel_never_opens_it() {
    let mut b = Bench::nominal("h-request", &["Button"]);
    b.ui.app.session.ui_requests.clear();
    b.run(WINDOW, json!({"package": PACKAGE}));
    let request = b.ui.app.session.ui_requests.iter().find(|r| r.kind == DIALOG).expect("a dialog request");
    assert_eq!(request.data["package"], json!(PACKAGE));
    b.ui.app.session.ui_requests.clear();

    let ctx = b.ui.ctx.clone();
    let call = json!({"command": WINDOW, "params": {"package": PACKAGE}});
    crate::control::handle(&mut b.ui.app, &ctx, "engine.execute", &call).unwrap();
    b.settle();
    assert!(b.ui.app.session.ui_requests.is_empty(), "the control channel drops the request: {:?}", b.ui.app.session.ui_requests);
}

#[test]
fn harness_the_scripted_service_holds_and_releases_its_answers_to_the_listing() {
    let mut b = Bench::nominal("h-list", &["Alert", "Button", "Chip"]);
    b.hold();
    let job = b.run(LIST, json!({"package": PACKAGE}));
    assert!(job["job"].is_u64(), "a job is started: {job}");
    b.ui.app.session.pump_capture();
    assert!(b.ui.app.session.inspected(PACKAGE).is_none(), "nothing is known while the service holds its answer");
    assert!(matches!(b.ui.app.session.capture_jobs().first().map(|j| j.state.clone()), Some(JobState::Running)));

    b.ui.app.session.pump_capture();
    b.shared.lock().unwrap().allowed += 1;
    let events = b.ui.app.session.pump_capture();
    assert_eq!(events.len(), 1, "{events:?}");
    let listed = b.ui.app.session.inspected(PACKAGE).expect("the components are kept in the session");
    let names: Vec<&str> = listed.components.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Alert", "Button", "Chip"]);
    let button = listed.components.iter().find(|c| c.name == "Button").unwrap();
    let props: Vec<&str> = button.props.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(props, ["variant", "size", "disabled"]);
}

#[test]
fn harness_a_staged_import_follows_its_jobs_and_one_failed_render_ends_it() {
    let mut b = Bench::nominal("h-import", &["Button"]);
    b.hold();
    let params = json!({"package": PACKAGE, "component": "Button", "stage": true});
    b.run(IMPORT, params.clone());
    let renders = render_count(2, 2, 5, 2);
    b.shared.lock().unwrap().allowed += 1;
    b.ui.app.session.pump_capture();
    let jobs: Vec<_> = b.ui.app.session.capture_jobs().into_iter().filter(|j| j.command == IMPORT).collect();
    assert_eq!(jobs.len(), 1 + renders, "the inspect job and every render are known once the inspect is back");
    let done = jobs.iter().filter(|j| j.state == JobState::Done).count();
    assert!(done <= 1, "{done} jobs are done: only the inspect may count");

    b.shared.lock().unwrap().hold = false;
    let mut ended = Vec::new();
    for _ in 0..20 {
        ended.extend(b.ui.app.session.pump_capture());
    }
    let event = ended.iter().find(|e| e.command == IMPORT).expect("the import ends");
    let result = event.outcome.as_ref().expect("it succeeds");
    assert_eq!(result["variants"], json!(renders / 2));
    assert!(b.imported().contains(&("Button".to_string(), (renders / 2) as u64)));

    b.fail_render("pressed", "dark", "Playwright: timeout rendering the pressed state");
    b.run(IMPORT, json!({"package": PACKAGE, "component": "Button", "stage": true, "states": ["Default", "Pressed"]}));
    let mut ended = Vec::new();
    for _ in 0..20 {
        ended.extend(b.ui.app.session.pump_capture());
    }
    let event = ended.iter().find(|e| e.command == IMPORT).expect("the second import ends");
    let error = event.outcome.as_ref().expect_err("it fails");
    assert!(error.message.contains("Playwright: timeout rendering the pressed state"), "{}", error.message);
}

// E4-6.1 — opening and closing the window

// covers: E4-6.1
#[test]
fn the_command_opens_the_window_on_the_requested_source_for_a_user_only_and_escape_closes_it() {
    let sources = [source(PACKAGE, VERSION, false), source(OTHER, OTHER_VERSION, false), source(GONE, "3.0.0", true)];
    let inspects = vec![
        (PACKAGE.to_string(), inspect_of(PACKAGE, VERSION, &["Button"], &[])),
        (OTHER.to_string(), inspect_of(OTHER, OTHER_VERSION, &["Zed"], &[])),
    ];
    let mut b = Bench::with("open", &sources, inspects);
    assert!(!b.window_open(), "the window is closed until it is asked for");

    let mark = b.mark();
    b.open(Some(OTHER));
    assert!(b.window_open(), "the command opens the window for the project of the tab");
    let listed: Vec<Value> = b.ran_since(mark, LIST);
    assert!(!listed.is_empty() && listed.iter().all(|p| p["package"] == json!(OTHER)), "the requested source is the one read: {listed:?}");
    let shown = b.shown();
    assert!(mentions(&shown, OTHER), "the source is named: {shown:?}");
    assert!(mentions(&shown, "Zed"), "its components are listed: {shown:?}");
    assert!(!mentions(&shown, "Button") && !mentions(&shown, GONE), "another or a removed source is not shown: {shown:?}");

    b.press(Key::Escape, Modifiers::NONE);
    assert!(!b.window_open(), "Escape closes the window");

    let ctx = b.ui.ctx.clone();
    let call = json!({"command": WINDOW, "params": {"package": OTHER}});
    crate::control::handle(&mut b.ui.app, &ctx, "engine.execute", &call).unwrap();
    b.settle();
    assert!(!b.window_open(), "a call by the control channel opens nothing");

    let mark = b.mark();
    b.open(None);
    assert!(b.window_open());
    let listed: Vec<Value> = b.ran_since(mark, LIST);
    assert!(listed.iter().all(|p| p["package"] == json!(PACKAGE)), "without a package the first source is read: {listed:?}");
}

// covers: E4-6.1
#[test]
fn the_file_menu_and_the_actions_palette_open_the_window() {
    let item = MENU.iter().find(|m| m.id == WINDOW).expect("a menu item runs uigen.importWindow");
    assert_eq!(item.menu, "File");
    assert_eq!(item.label, "Import a component…");
    assert!(crate::menus::available(WINDOW));

    let groups = crate::menus::command_sections("import a component");
    let found = groups.iter().any(|(_, rows)| rows.iter().any(|row| row.key == PALETTE_ROW));
    assert!(found, "the Actions palette finds the command: {:?}", groups.iter().map(|(g, _)| g).collect::<Vec<_>>());

    let mut b = Bench::nominal("palette", &["Button"]);
    assert!(!b.window_open());
    crate::menus::run_action(&mut b.ui.app, PALETTE_ROW);
    b.settle();
    assert!(b.window_open(), "running the palette row opens the window");
}

// E4-6.2 — the states of the list

// covers: E4-6.2
#[test]
fn while_the_service_reads_the_components_the_window_says_so_and_cannot_import_then_it_lists_them() {
    let mut b = Bench::nominal("loading", &["Alert", "Button", "Chip"]);
    b.hold();
    b.open(Some(PACKAGE));
    let shown = b.shown();
    assert!(mentions(&shown, &format!("Reading the components of {PACKAGE}…")), "the loading state is drawn: {shown:?}");
    assert_eq!(b.inspected_packages(), [PACKAGE], "one inspect job asked");
    let mark = b.mark();
    if b.has("Import") {
        b.click("Import");
    }
    assert!(b.ran_since(mark, IMPORT).is_empty(), "nothing is imported while the list is read");
    assert!(!mentions(&b.shown(), "Button"), "no component is listed yet");

    b.release(1);
    let shown = b.shown();
    assert!(!mentions(&shown, "Reading the components"), "the loading state ends: {shown:?}");
    for name in ["Alert", "Button", "Chip"] {
        assert!(mentions(&shown, name), "{name} is listed: {shown:?}");
    }
}

// covers: E4-6.2
#[test]
fn the_search_field_filters_the_listed_components() {
    let mut b = Bench::nominal("search", &["Alert", "Button", "Chip"]);
    b.open(Some(PACKAGE));
    b.wait_until("the components are listed", |b| b.has("Search components"));
    let shown = b.shown();
    assert!(mentions(&shown, "Button") && mentions(&shown, "Chip"), "{shown:?}");

    b.click("Search components");
    b.type_text("chi");
    b.press(Key::Enter, Modifiers::NONE);
    let shown = b.shown();
    assert!(mentions(&shown, "Chip"), "the match stays: {shown:?}");
    assert!(!mentions(&shown, "Button") && !mentions(&shown, "Alert"), "the others go: {shown:?}");
}

// covers: E4-6.2
#[test]
fn a_failed_reading_says_why_and_try_again_asks_the_service_again_with_refresh() {
    let cause = "npm error: synthetic-ui has no type declarations";
    let mut b = Bench::with("list-error", &[source(PACKAGE, VERSION, false)], vec![(PACKAGE.to_string(), Err(failure(cause)))]);
    b.open(Some(PACKAGE));
    b.wait_until("the error is drawn", |b| b.has("Try again"));
    let shown = b.shown();
    let message = format!("Couldn't read the components of {PACKAGE}: {cause}");
    assert!(mentions(&shown, &message), "the error names the package and the cause: {shown:?}");
    assert!(!mentions(&shown, "Reading the components"), "{shown:?}");
    assert!(b.inspected_packages().len() == 1, "{:?}", b.inspected_packages());

    b.set_inspect(PACKAGE, inspect_of(PACKAGE, VERSION, &["Button"], &[]));
    let mark = b.mark();
    b.click("Try again");
    let calls = b.ran_since(mark, LIST);
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!(calls[0]["package"], json!(PACKAGE));
    assert_eq!(calls[0]["refresh"], json!(true), "the cache is asked again");
    b.wait_until("the components are listed", |b| b.has("Button"));
    assert!(!mentions(&b.shown(), "Couldn't read the components"), "the error is gone");
}

// covers: E4-6.2
#[test]
fn a_package_with_no_component_says_so_and_shows_the_notes_of_the_reading() {
    let note = "No exported component was found in the type declarations";
    let mut b = Bench::with("empty", &[source(PACKAGE, VERSION, false)], vec![(PACKAGE.to_string(), inspect_of(PACKAGE, VERSION, &[], &[note]))]);
    b.open(Some(PACKAGE));
    b.wait_until("the empty state is drawn", |b| b.shows("This package exports no component."));
    let shown = b.shown();
    assert!(mentions(&shown, note), "the notes of the inspection are shown: {shown:?}");
    assert!(!mentions(&shown, "Couldn't read") && !mentions(&shown, "Reading the components"), "{shown:?}");
}

// covers: E4-6.2
#[test]
fn without_a_source_the_window_sends_the_user_to_the_project_settings() {
    let mut b = Bench::with("no-source", &[], vec![]);
    b.open(None);
    let shown = b.shown();
    assert!(mentions(&shown, "No source library yet. Add one in the project settings."), "{shown:?}");
    assert!(b.inspected_packages().is_empty(), "there is nothing to read");
    assert!(b.ui.app.ui.project_settings.is_none());

    let mark = b.mark();
    b.click("Open project settings");
    assert_eq!(b.ran_since(mark, "project.settings").len(), 1, "the button runs project.settings");
    assert!(b.ui.app.ui.project_settings.is_some(), "the project settings window opens");
}

// E4-6.3 — the options follow the props of the component

// covers: E4-6.3
#[test]
fn choosing_button_sets_the_options_from_its_props() {
    let mut b = Bench::nominal("options", &["Alert", "Button", "Chip"]);
    b.open_and_choose("Button");
    for value in ["solid", "outline", "sm", "md"] {
        assert!(b.checked(value), "{value} is checked: style is the variant enum, size the size enum");
    }
    for state in STATES {
        assert!(b.checked(state), "{state} is checked");
    }
    assert!(!b.checked("Focus"), "Focus is not");
    assert!(!b.has("Loading"), "Loading is offered only for a component with a boolean `loading` prop");
    assert!(b.checked("Light") && b.checked("Dark"), "both themes");
    assert!(b.checked("Animations"), "animations are on");
    assert!(mentions(&b.shown(), "Transitions between states, ripple and spinner"));
    let line = variants_line(2, 2, 4, 2);
    assert!(mentions(&b.shown(), &line), "the count is on screen ({line}): {:?}", b.shown());
}

// covers: E4-6.3
#[test]
fn every_change_of_the_options_updates_the_variants_and_renders_line() {
    let mut b = Bench::nominal("recount", &["Alert", "Button", "Chip"]);
    b.open_and_choose("Button");

    b.click("Hover");
    assert!(!b.checked("Hover"));
    assert!(mentions(&b.shown(), &variants_line(2, 2, 3, 2)), "{:?}", b.shown());
    b.click("Dark");
    assert!(mentions(&b.shown(), &variants_line(2, 2, 3, 1)), "{:?}", b.shown());
    b.click("outline");
    b.click("md");
    assert!(mentions(&b.shown(), &variants_line(1, 1, 3, 1)), "{:?}", b.shown());
    b.click("Focus");
    assert!(b.checked("Focus"));
    assert!(mentions(&b.shown(), &variants_line(1, 1, 4, 1)), "{:?}", b.shown());
}

// covers: E4-6.3
#[test]
fn a_component_with_no_variant_or_size_enum_counts_one_per_state_and_theme() {
    let mut b = Bench::nominal("plain", &["Alert", "Chip"]);
    b.open_and_choose("Chip");
    assert!(!b.has("solid") && !b.has("sm"), "no style or size value to choose");
    assert!(mentions(&b.shown(), &variants_line(0, 0, 4, 2)), "{:?}", b.shown());
}

// covers: E4-6.3
#[test]
fn loading_is_offered_with_a_boolean_loading_prop_and_starts_unchecked() {
    let mut b = Bench::nominal("loader", &["Alert", "Loader"]);
    b.open_and_choose("Loader");
    assert!(b.has("Loading"), "a boolean `loading` prop offers the Loading state");
    assert!(!b.checked("Loading"));
    b.click("Loading");
    assert!(b.checked("Loading"));
    assert!(mentions(&b.shown(), &variants_line(2, 0, 5, 2)), "{:?}", b.shown());
}

// E4-6.4 — what the Import button sends

// covers: E4-6.4
#[test]
fn import_runs_the_command_with_the_options_as_they_are_checked() {
    let mut b = Bench::nominal("request", &["Alert", "Button"]);
    b.hold();
    b.open(Some(PACKAGE));
    b.release(1);
    b.wait_until("the components are listed", |b| b.has("Button"));
    b.click("Button");
    let mark = b.mark();
    b.click("Import");
    let calls = b.ran_since(mark, IMPORT);
    assert_eq!(calls.len(), 1, "one import: {calls:?}");
    let call = &calls[0];
    assert_eq!(call["package"], json!(PACKAGE));
    assert_eq!(call["component"], json!("Button"));
    assert_eq!(call["styleProp"], json!("variant"));
    assert_eq!(call["sizeProp"], json!("size"));
    assert_eq!(sorted(&call["styles"]), strings(&["outline", "solid"]));
    assert_eq!(sorted(&call["sizes"]), strings(&["md", "sm"]));
    assert_eq!(sorted(&call["states"]), strings(&STATES));
    assert_eq!(sorted(&call["themes"]), strings(&["dark", "light"]));
    assert_eq!(call["animations"], json!(true));
    assert_eq!(call["stage"], json!(true), "the window always stages the import");
}

// covers: E4-6.4
#[test]
fn an_unchecked_value_leaves_the_list_the_window_sends() {
    let mut b = Bench::nominal("request-unchecked", &["Alert", "Button"]);
    b.hold();
    b.open(Some(PACKAGE));
    b.release(1);
    b.wait_until("the components are listed", |b| b.has("Button"));
    b.click("Button");
    for label in ["outline", "md", "Hover", "Dark", "Animations"] {
        b.click(label);
        assert!(!b.checked(label), "{label} is unchecked");
    }
    let mark = b.mark();
    b.click("Import");
    let calls = b.ran_since(mark, IMPORT);
    assert_eq!(calls.len(), 1, "{calls:?}");
    let call = &calls[0];
    assert_eq!(sorted(&call["styles"]), strings(&["solid"]));
    assert_eq!(sorted(&call["sizes"]), strings(&["sm"]));
    assert_eq!(sorted(&call["states"]), strings(&["Default", "Disabled", "Pressed"]));
    assert_eq!(sorted(&call["themes"]), strings(&["light"]));
    assert_eq!(call["animations"], json!(false));
    assert_eq!(call["stage"], json!(true));
}

// covers: E4-6.4
#[test]
fn import_is_off_when_a_group_of_options_has_no_choice_left() {
    let mut b = Bench::nominal("nothing-checked", &["Alert", "Button"]);
    b.open_and_choose("Button");
    let groups: [&[&str]; 4] = [&STATES, &["Light", "Dark"], &["solid", "outline"], &["sm", "md"]];
    for group in groups {
        for label in group {
            b.click(label);
        }
        let mark = b.mark();
        b.click("Import");
        assert!(b.ran_since(mark, IMPORT).is_empty(), "nothing is imported with {group:?} all unchecked");
        for label in group {
            b.click(label);
            assert!(b.checked(label), "{label} is back");
        }
    }
    let mark = b.mark();
    b.click("Import");
    assert_eq!(b.ran_since(mark, IMPORT).len(), 1, "with a choice in every group the import runs");
}

// covers: E4-6.4
#[test]
fn more_than_the_render_limit_says_so_under_the_options_and_turns_import_off() {
    let mut b = Bench::nominal("too-many", &["Alert", "Big"]);
    b.open_and_choose("Big");
    let shown = b.shown();
    let reason = format!("More than {MAX_RENDERS} renders: choose fewer styles, sizes, states or themes");
    assert!(mentions(&shown, &reason), "the reason is on screen: {shown:?}");
    assert!(mentions(&shown, &variants_line(10, 4, 4, 2)), "the line still counts: {shown:?}");
    let mark = b.mark();
    b.click("Import");
    assert!(b.ran_since(mark, IMPORT).is_empty(), "nothing is imported over the limit");
}

// covers: E4-6.4
#[test]
fn exactly_the_render_limit_is_allowed() {
    let mut b = Bench::nominal("at-limit", &["Alert", "Edge"]);
    b.hold();
    b.open(Some(PACKAGE));
    b.release(1);
    b.wait_until("the components are listed", |b| b.has("Edge"));
    b.click("Edge");
    assert_eq!(render_count(4, 4, 4, 2), MAX_RENDERS);
    let shown = b.shown();
    assert!(!mentions(&shown, "More than"), "no reason at the limit: {shown:?}");
    assert!(mentions(&shown, &variants_line(4, 4, 4, 2)), "{shown:?}");
    let mark = b.mark();
    b.click("Import");
    assert_eq!(b.ran_since(mark, IMPORT).len(), 1, "128 renders are imported");
}

// E4-6.5 — the import, from start to end

// covers: E4-6.5
#[test]
fn the_form_is_off_while_the_import_runs_and_its_progress_follows_the_jobs() {
    let mut b = Bench::nominal("progress", &["Alert", "Button"]);
    b.hold();
    b.open(Some(PACKAGE));
    b.release(1);
    b.wait_until("the components are listed", |b| b.has("Button"));
    b.click("Button");
    let renders = render_count(2, 2, 4, 2);
    let total = 1 + renders;
    let mark = b.mark();
    b.click("Import");
    assert_eq!(b.ran_since(mark, IMPORT).len(), 1);
    assert_eq!(b.progress(), Some((0, total)), "nothing is done yet: {:?}", b.shown());

    let hover = b.checked("Hover");
    b.click("Hover");
    assert_eq!(b.checked("Hover"), hover, "the form does not change while the import runs");
    let mark = b.mark();
    b.click("Import");
    assert!(b.ran_since(mark, IMPORT).is_empty(), "a second import cannot start");

    b.release(1);
    let (done, shown_total) = b.progress().expect("the progress stays on screen");
    assert_eq!(shown_total, total);
    assert!(done <= 1, "only the inspect is back: {done}");

    b.release(16);
    let (done, shown_total) = b.progress().expect("the progress stays on screen");
    assert_eq!(shown_total, total);
    assert!((16..=17).contains(&done), "16 renders are back: {done}");

    b.release(15);
    let (done, shown_total) = b.progress().expect("the progress stays on screen");
    assert_eq!(shown_total, total);
    assert!((31..=32).contains(&done), "31 renders are back: {done}");
    assert!(!mentions(&b.shown(), "Imported Button"), "not imported before the last render");
    assert!(b.imported().is_empty(), "nothing is written before the last render");
}

// covers: E4-6.5
#[test]
fn a_finished_import_is_summarised_in_the_window_without_a_toast() {
    let mut b = Bench::nominal("success", &["Alert", "Button"]);
    b.open_and_choose("Button");
    b.click("Import");
    b.wait_until("the import is summarised", |b| b.shows("Imported Button: 16 variants"));
    let shown = b.shown();
    assert!(!mentions(&shown, "Importing…"), "the progress is gone: {shown:?}");
    assert!(b.has("Show in Components") && b.has("Import another"), "both next steps are offered");
    assert_eq!(b.toast(), None, "the window says it, the toast is not raised");
    assert_eq!(b.ui.app.ui.last_error, None);
    assert_eq!(b.imported(), [("Button".to_string(), 16)], "the set is in the import page");

    let mark = b.mark();
    b.click("Import another");
    let shown = b.shown();
    assert!(!mentions(&shown, "Imported Button"), "the summary is gone: {shown:?}");
    assert!(b.has("Import") && b.has("Search components"), "the form is back");
    b.click("Hover");
    assert!(!b.checked("Hover"), "and it can be edited");
    assert!(b.ran_since(mark, IMPORT).is_empty(), "Import another imports nothing");
    assert_eq!(b.toast(), None);
}

// covers: E4-6.5
#[test]
fn show_in_components_opens_the_components_tab() {
    let mut b = Bench::nominal("show", &["Alert", "Button"]);
    b.open_and_choose("Button");
    b.click("Import");
    b.wait_until("the import is summarised", |b| b.has("Show in Components"));
    b.ui.app.ui.left_tab = LeftTab::File;
    b.ui.app.ui.left_collapsed = true;
    b.click("Show in Components");
    assert_eq!(b.ui.app.ui.left_tab, LeftTab::Assets, "the Components tab is showing");
    assert!(!b.ui.app.ui.left_collapsed, "and its panel is open");
}

// covers: E4-6.5
#[test]
fn a_failed_import_says_why_keeps_the_form_editable_and_adds_nothing() {
    let cause = "Playwright: timeout rendering the pressed state";
    let mut b = Bench::nominal("failure", &["Alert", "Button"]);
    b.fail_render("pressed", "dark", cause);
    b.open_and_choose("Button");
    let before = b.fingerprint();
    b.click("Import");
    b.wait_until("the failure is drawn", |b| b.shows("Couldn't import Button: "));
    let shown = b.shown();
    assert!(
        shown.iter().any(|t| t.contains("Couldn't import Button: ") && t.contains(cause)),
        "the failure names the component and the cause: {shown:?}"
    );
    assert!(!mentions(&shown, "Importing…") && !mentions(&shown, "Imported Button"), "{shown:?}");
    assert_eq!(b.toast(), None, "the window says it, no toast");
    assert_eq!(b.ui.app.ui.last_error, None);
    assert_eq!(b.fingerprint(), before, "the document is as it was");
    assert!(b.imported().is_empty());

    b.click("Hover");
    assert!(!b.checked("Hover"), "the form can be edited");
    b.click("Hover");
    b.shared.lock().unwrap().failing_render = None;
    let mark = b.mark();
    b.click("Import");
    assert_eq!(b.ran_since(mark, IMPORT).len(), 1, "and imported again");
    b.wait_until("the second import is summarised", |b| b.shows("Imported Button: 16 variants"));
}

// covers: E4-6.5
#[test]
fn with_the_window_closed_the_import_goes_on_and_the_toast_of_lot_3_is_raised() {
    let mut b = Bench::nominal("closed", &["Alert", "Button"]);
    b.hold();
    b.open(Some(PACKAGE));
    b.release(1);
    b.wait_until("the components are listed", |b| b.has("Button"));
    b.click("Button");
    b.click("Import");
    b.press(Key::Escape, Modifiers::NONE);
    assert!(!b.window_open(), "Escape closes the window during the import");
    b.release_all();
    b.wait_until("the toast is raised", |b| b.toast().is_some());
    assert_eq!(b.toast().as_deref(), Some(format!("Imported a component from {PACKAGE}").as_str()));
    assert_eq!(b.imported(), [("Button".to_string(), 16)], "closing did not cancel the import");
}

// E4-6.6 — keyboard, labels, catalogs

// covers: E4-6.6
#[test]
fn the_window_works_from_the_keyboard_alone() {
    let mut b = Bench::nominal("keyboard", &["Alert", "Button", "Chip"]);
    crate::menus::run_action(&mut b.ui.app, PALETTE_ROW);
    b.settle();
    b.wait_until("the components are listed", |b| b.shows("Button"));
    b.ui.ctx.enable_accesskit();
    let page = b.ui.app.session.page().unwrap();

    b.press(Key::Tab, Modifiers::NONE);
    assert!(!focused_name(&b.ui.ctx).is_empty(), "the first Tab lands on a named control of the window, not on the canvas");

    let required: BTreeSet<&str> = [
        "Search components",
        "Button",
        "solid",
        "outline",
        "sm",
        "md",
        "Default",
        "Hover",
        "Focus",
        "Pressed",
        "Disabled",
        "Light",
        "Dark",
        "Animations",
    ]
    .into_iter()
    .collect();
    let mut visited = BTreeSet::new();
    let tab_to = |b: &mut Bench, wanted: &str, visited: &mut BTreeSet<String>| {
        for _ in 0..120 {
            let name = focused_name(&b.ui.ctx);
            visited.insert(name.clone());
            if name == wanted {
                return;
            }
            b.press(Key::Tab, Modifiers::NONE);
        }
        panic!("Tab never reached {wanted:?}; visited {visited:?}");
    };

    tab_to(&mut b, "Button", &mut visited);
    b.press(Key::Enter, Modifiers::NONE);
    assert!(mentions(&b.shown(), &variants_line(2, 2, 4, 2)), "Enter chose Button: {:?}", b.shown());

    tab_to(&mut b, "Hover", &mut visited);
    assert!(b.checked("Hover"));
    b.press(Key::Space, Modifiers::NONE);
    assert!(!b.checked("Hover"), "Space toggles the focused option");

    b.press(Key::Tab, Modifiers::SHIFT);
    let previous = focused_name(&b.ui.ctx);
    assert!(!previous.is_empty() && previous != "Hover", "Shift+Tab goes back to another named control: {previous:?}");
    b.press(Key::Tab, Modifiers::NONE);
    assert_eq!(focused_name(&b.ui.ctx), "Hover", "and Tab forward again");

    for _ in 0..120 {
        visited.insert(focused_name(&b.ui.ctx));
        if required.iter().all(|label| visited.contains(*label)) && visited.contains("Import") {
            break;
        }
        b.press(Key::Tab, Modifiers::NONE);
    }
    for label in &required {
        assert!(visited.contains(*label), "Tab reaches {label:?}; visited {visited:?}");
    }

    tab_to(&mut b, "Import", &mut visited);
    let mark = b.mark();
    b.press(Key::Enter, Modifiers::NONE);
    assert_eq!(b.ran_since(mark, IMPORT).len(), 1, "Enter on Import starts the import");
    assert_eq!(b.ui.app.session.page().unwrap(), page, "Tab never moved the canvas selection to another page");
    assert!(b.ui.app.session.selection().is_empty(), "nor selected a layer");
}

fn keys_in(src: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for call in ["tr(\"", "trf(\"", "trn(\"", "tr_noop(\""] {
        for (at, _) in src.match_indices(call) {
            if src[..at].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let mut key = String::new();
            let mut chars = src[at + call.len()..].chars();
            while let Some(c) = chars.next() {
                match c {
                    '"' => {
                        found.insert(key);
                        break;
                    }
                    '\\' => key.extend(chars.next().map(|n| if n == 'n' { '\n' } else { n })),
                    c => key.push(c),
                }
            }
        }
    }
    found
}

const NEW_KEYS: [&str; 24] = [
    "Import a component",
    "Source",
    "Search components",
    "Reading the components of {package}…",
    "Couldn't read the components of {package}: {cause}",
    "This package exports no component.",
    "No source library yet. Add one in the project settings.",
    "Open project settings",
    "Sizes",
    "States",
    "Themes",
    "Animations",
    "Transitions between states, ripple and spinner",
    "{variants} variants · {renders} renders",
    "More than {max} renders: choose fewer styles, sizes, states or themes",
    "Importing… {done} of {total}",
    "Imported {name}: {variants} variants",
    "Couldn't import {name}: {cause}",
    "Show in Components",
    "Import another",
    "Hover",
    "Focus",
    "Pressed",
    "Disabled",
];

const COMMANDS: [&str; 4] = ["uigen.importWindow", "uigen.listComponents", "uigen.imported", "uigen.placeImported"];

fn text_of<'a>(lang: &'a Lang, key: &str) -> Option<&'a str> {
    lang.ui.iter().find(|(k, _)| *k == key).map(|(_, t)| match t {
        Text::Plain(s) | Text::Plural { other: s, .. } => *s,
    })
}

// covers: E4-6.6
#[test]
fn every_language_translates_the_window_texts_and_the_four_command_labels() {
    let languages = i18n::languages();
    assert_eq!(languages.len(), 22);
    let dialogs = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/dialogs");
    let mut sources = vec![dialogs.join("import_component.rs")];
    if let Ok(entries) = std::fs::read_dir(dialogs.join("import_component")) {
        sources.extend(entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "rs")));
    }
    let mut keys = BTreeSet::new();
    for path in &sources {
        keys.extend(keys_in(&std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))));
    }
    for key in NEW_KEYS {
        assert!(keys.contains(key), "the window uses the key {key:?}: {keys:?}");
    }

    let mut missing = Vec::new();
    for lang in languages.iter().filter(|l| l.code != "en") {
        let own: Vec<&Lang> = [Some(lang), languages.get(lang.parent)].into_iter().flatten().filter(|l| l.code != "en").collect();
        for key in &keys {
            if !own.iter().any(|l| l.ui.iter().any(|(k, _)| k == key)) {
                missing.push(format!("{}: {key:?}", lang.code));
            }
        }
        for command in COMMANDS {
            if !own.iter().any(|l| l.commands.iter().any(|(k, _)| *k == command)) {
                missing.push(format!("{}: [commands] {command}", lang.code));
            }
        }
    }
    assert!(missing.is_empty(), "untranslated:\n{}", missing.join("\n"));

    let fr = languages.iter().find(|l| l.code == "fr").expect("French is baked in");
    for key in [
        "Import a component",
        "Search components",
        "This package exports no component.",
        "Importing… {done} of {total}",
        "Show in Components",
        "Import another",
    ] {
        assert!(text_of(fr, key).is_some_and(|text| text != key), "fr translates {key:?}");
    }
}
