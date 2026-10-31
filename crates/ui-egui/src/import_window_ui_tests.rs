//! The "Import components" surface (`dialog.importComponent`, asked by `uigen.importWindow`): it
//! reads the components of every source library the project has and offers them in one card per
//! source (the choosing itself is in `import_pick_ui_tests`). A user opens it from File, the
//! Actions palette, the Components tab or the Source libraries page; a programmatic call never does;
//! Cancel closes it. Every state of the reading (loading, ready, error, empty, no source) is drawn.
//!
//! The service is a scripted backend that can hold its answers back (`hold` / `release`), so a
//! test sees the surface while a job is still running. The answers are synthetic. The bench is
//! shared with `import_pick_ui_tests`.
//!
//! The 22 catalogs: `i18n/tests.rs` checks that every key the code looks up is in the English
//! catalog and that no catalog has a stray key or a wrong placeholder; it does not fail on a missing
//! translation, so the last test here fails on any language that lacks a text of the surface.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use egui::accesskit::{Node, Role, Toggled};
use egui::{Event, Key, Modifiers, Pos2, pos2};
use figcraft_engine::capture::{CaptureBackend, CaptureError, CaptureErrorKind, CaptureJob, JobId, JobState};
use figcraft_engine::catalog::MENU;
use figcraft_engine::render_count;
use serde_json::{Value, json};

use crate::i18n::{self, Lang, Text};
use crate::import_report_tests::shown_texts;
use crate::layers_drag_tests::Ui;

static NEXT: AtomicUsize = AtomicUsize::new(0);

pub(crate) const PACKAGE: &str = "synthetic-ui";
pub(crate) const VERSION: &str = "1.0.0";
pub(crate) const OTHER: &str = "other-ui";
pub(crate) const OTHER_VERSION: &str = "2.0.0";
pub(crate) const GONE: &str = "gone-ui";
pub(crate) const WINDOW: &str = "uigen.importWindow";
pub(crate) const LIST: &str = "uigen.listComponents";
pub(crate) const IMPORT: &str = "uigen.importComponent";
const DIALOG: &str = "dialog.importComponent";
const PALETTE_ROW: &str = "command:uigen.importWindow";
const MAX_FRAMES: usize = 300;

pub(crate) type Outcome = Result<Value, CaptureError>;

pub(crate) struct Dir(pub(crate) PathBuf);

impl Dir {
    pub(crate) fn new(tag: &str) -> Self {
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

fn component(name: &str) -> Value {
    let props = match name {
        "Alert" => vec![plain_prop("title", "string")],
        "Button" => vec![enum_prop("variant", &["solid", "outline"]), enum_prop("size", &["sm", "md"]), plain_prop("disabled", "boolean")],
        "Chip" => vec![plain_prop("label", "string")],
        _ => vec![plain_prop("label", "string")],
    };
    json!({"name": name, "props": props})
}

pub(crate) fn inspect_of(package: &str, version: &str, names: &[&str], notes: &[&str]) -> Outcome {
    let components: Vec<Value> = names.iter().map(|name| component(name)).collect();
    Ok(json!({"package": package, "version": version, "framework": "react", "components": components, "notes": notes}))
}

pub(crate) fn failure(message: &str) -> CaptureError {
    CaptureError { kind: CaptureErrorKind::Protocol, message: message.to_string() }
}

pub(crate) fn source(package: &str, version: &str, removed: bool) -> Value {
    json!({"package": package, "version": version, "framework": "react", "license": "MIT", "removed": removed})
}

pub(crate) struct FailingRender {
    pub(crate) state: &'static str,
    pub(crate) theme: &'static str,
    pub(crate) error: CaptureError,
}

pub(crate) struct Shared {
    pub(crate) submitted: Vec<CaptureJob>,
    pub(crate) queue: Vec<(JobId, Outcome)>,
    pub(crate) hold: bool,
    pub(crate) allowed: usize,
    pub(crate) inspects: Vec<(String, Outcome)>,
    pub(crate) failing_render: Option<FailingRender>,
    pub(crate) failing_component: Option<(String, CaptureError)>,
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
            CaptureJob::Render { component, props, state, theme, .. } => match (&shared.failing_component, &shared.failing_render) {
                (Some((name, error)), _) if name == component => Err(error.clone()),
                (_, Some(failing)) if failing.state == state && failing.theme == theme => Err(failing.error.clone()),
                _ => {
                    let style = props.get("variant").and_then(Value::as_str).unwrap_or("solid");
                    let size = props.get("size").and_then(Value::as_str).unwrap_or("md");
                    Ok(json!({"snapshot": synthetic_button(style, size, state, theme), "png": ".uigen/captures/synthetic.png"}))
                }
            },
            CaptureJob::Install { package, version, .. } => {
                Ok(json!({"package": package, "version": version, "framework": "react", "license": "MIT"}))
            }
            CaptureJob::InstallMany { packages, .. } => {
                let installed: Vec<Value> =
                    packages.iter().map(|spec| json!({"package": spec.name, "version": VERSION, "framework": "react", "license": "MIT"})).collect();
                Ok(json!({"packages": installed}))
            }
            CaptureJob::Uninstall { .. } | CaptureJob::Playground { .. } => Ok(json!({})),
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
pub(crate) struct Bench {
    pub(crate) ui: Ui,
    pub(crate) shared: Arc<Mutex<Shared>>,
    _dir: Dir,
}

impl Bench {
    pub(crate) fn with(tag: &str, sources: &[Value], inspects: Vec<(String, Outcome)>) -> Self {
        figcraft_text::scan_system_fonts();
        let dir = Dir::new(tag);
        write_project(&dir, sources);
        let mut ui = Ui::new(figcraft_engine::Session::empty());
        ui.app.session.prefs.language = "en".into();
        assert!(ui.app.open_project_at(dir.0.clone(), false), "the project opens");
        let shared = Arc::new(Mutex::new(Shared {
            submitted: Vec::new(),
            queue: Vec::new(),
            hold: false,
            allowed: 0,
            inspects,
            failing_render: None,
            failing_component: None,
        }));
        ui.app.session.set_capture_backend(Box::new(FakeBackend { shared: Arc::clone(&shared) }));
        ui.app.ui.toast = None;
        ui.app.ui.home = false;
        let mut bench = Self { ui, shared, _dir: dir };
        bench.settle();
        bench
    }

    /// One source, `PACKAGE`, that exports `names`.
    pub(crate) fn nominal(tag: &str, names: &[&str]) -> Self {
        Self::with(tag, &[source(PACKAGE, VERSION, false)], vec![(PACKAGE.to_string(), inspect_of(PACKAGE, VERSION, names, &[]))])
    }

    pub(crate) fn pair(tag: &str, first: &[&str], second: &[&str]) -> Self {
        Self::with(
            tag,
            &[source(PACKAGE, VERSION, false), source(OTHER, OTHER_VERSION, false)],
            vec![(PACKAGE.to_string(), inspect_of(PACKAGE, VERSION, first, &[])), (OTHER.to_string(), inspect_of(OTHER, OTHER_VERSION, second, &[]))],
        )
    }

    pub(crate) fn hold(&mut self) {
        self.shared.lock().unwrap().hold = true;
    }

    /// Let the service answer the next `count` jobs (and go on holding the others back).
    pub(crate) fn release(&mut self, count: usize) {
        self.shared.lock().unwrap().allowed += count;
        self.settle();
    }

    pub(crate) fn release_all(&mut self) {
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

    pub(crate) fn fail_component(&mut self, name: &str, message: &str) {
        self.shared.lock().unwrap().failing_component = Some((name.to_string(), failure(message)));
    }

    fn submitted(&self) -> Vec<CaptureJob> {
        self.shared.lock().unwrap().submitted.clone()
    }

    pub(crate) fn inspected_packages(&self) -> Vec<String> {
        self.submitted()
            .into_iter()
            .filter_map(|job| match job {
                CaptureJob::Inspect { package, .. } => Some(package),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn settle(&mut self) {
        for _ in 0..3 {
            self.ui.frame(vec![]);
        }
    }

    pub(crate) fn wait_until(&mut self, what: &str, done: impl Fn(&mut Self) -> bool) {
        for _ in 0..MAX_FRAMES {
            self.ui.frame(vec![]);
            if done(self) {
                self.settle();
                return;
            }
        }
        panic!("never happened: {what}; on screen: {:?}", self.shown());
    }

    pub(crate) fn run(&mut self, id: &str, params: Value) -> Value {
        self.ui.app.session.execute(id, &params).unwrap_or_else(|e| panic!("{id}: {e}"))
    }

    pub(crate) fn mark(&self) -> usize {
        self.ui.app.session.journal.len()
    }

    pub(crate) fn ran_since(&self, mark: usize, command: &str) -> Vec<Value> {
        self.ui.app.session.journal[mark..].iter().filter(|(c, _)| c == command).map(|(_, p)| p.clone()).collect()
    }

    pub(crate) fn open(&mut self, package: Option<&str>) {
        let params = package.map_or_else(|| json!({}), |package| json!({"package": package}));
        assert!(self.ui.app.run(WINDOW, params).is_some(), "{WINDOW} runs");
        self.settle();
    }

    pub(crate) fn open_settings(&mut self) {
        assert!(self.ui.app.run("project.settings", json!({})).is_some(), "project.settings runs");
        self.settle();
    }

    pub(crate) fn shown(&mut self) -> Vec<String> {
        self.ui.frame(vec![]);
        shown_texts(self.ui.frame_output(vec![]).shapes)
    }

    pub(crate) fn flat(&mut self) -> String {
        self.shown().join(" ")
    }

    pub(crate) fn shows(&mut self, needle: &str) -> bool {
        self.shown().iter().any(|text| text.contains(needle))
    }

    pub(crate) fn window_open(&mut self) -> bool {
        self.flat().contains("02 Components")
    }

    pub(crate) fn nodes(&mut self) -> Vec<Node> {
        self.ui.ctx.enable_accesskit();
        self.ui.frame(vec![]);
        self.ui.frame_output(vec![]).platform_output.accesskit_update.expect("an AccessKit update").nodes.into_iter().map(|(_, n)| n).collect()
    }

    pub(crate) fn controls(&mut self, label: &str) -> Vec<Node> {
        let mut found: Vec<Node> =
            self.nodes().into_iter().filter(|n| n.role() != Role::Label && n.label().is_some_and(|l| l.trim() == label)).collect();
        found.sort_by(|a, b| {
            let key = |n: &Node| n.bounds().map_or((f64::MAX, f64::MAX), |b| (b.y0, b.x0));
            key(a).partial_cmp(&key(b)).unwrap()
        });
        found
    }

    pub(crate) fn control(&mut self, label: &str) -> Option<Node> {
        self.controls(label).into_iter().next()
    }

    pub(crate) fn has(&mut self, label: &str) -> bool {
        self.control(label).is_some()
    }

    fn centre(node: &Node, label: &str) -> Pos2 {
        let b = node.bounds().unwrap_or_else(|| panic!("{label:?} has no bounds"));
        pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
    }

    pub(crate) fn click_nth(&mut self, label: &str, nth: usize) {
        let found = self.controls(label);
        let node = found.get(nth).unwrap_or_else(|| panic!("no control #{nth} named {label:?} on screen ({} found)", found.len()));
        let at = Self::centre(node, label);
        self.ui.button(at, true);
        self.ui.button(at, false);
        self.settle();
    }

    pub(crate) fn click(&mut self, label: &str) {
        self.click_nth(label, 0);
    }

    pub(crate) fn checked_nth(&mut self, label: &str, nth: usize) -> bool {
        let found = self.controls(label);
        let node = found.get(nth).unwrap_or_else(|| panic!("no control #{nth} named {label:?} on screen ({} found)", found.len()));
        match node.toggled() {
            Some(Toggled::True) => true,
            Some(Toggled::False) => false,
            other => panic!("{label:?} is not a checkbox: {other:?}"),
        }
    }

    pub(crate) fn checked(&mut self, label: &str) -> bool {
        self.checked_nth(label, 0)
    }

    pub(crate) fn press(&mut self, key: Key, modifiers: Modifiers) {
        self.ui.frame(vec![
            Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers },
            Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers },
        ]);
        self.settle();
    }

    pub(crate) fn type_text(&mut self, text: &str) {
        self.ui.frame(vec![Event::Text(text.into())]);
        self.settle();
    }

    pub(crate) fn imported(&mut self) -> Vec<(String, u64)> {
        let sections = self.run("uigen.imported", json!({}))["sections"].as_array().cloned().unwrap_or_default();
        sections
            .iter()
            .filter_map(|s| s["components"].as_array())
            .flatten()
            .map(|c| (c["name"].as_str().unwrap_or_default().to_string(), c["variants"].as_u64().unwrap_or_default()))
            .collect()
    }

    /// `(done, total)` of the "Importing… {done} of {total}" line on screen.
    pub(crate) fn progress(&mut self) -> Option<(usize, usize)> {
        let line = self.shown().into_iter().find_map(|text| text.strip_prefix("Importing… ").map(str::to_string))?;
        let (done, total) = line.split_once(" of ")?;
        Some((done.trim().parse().ok()?, total.trim().parse().ok()?))
    }
}

pub(crate) fn mentions(shown: &[String], needle: &str) -> bool {
    shown.iter().any(|text| text.contains(needle))
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

// E4-6.1 — opening and closing the surface

// covers: E4-6.1
#[test]
fn the_command_opens_the_surface_with_the_requested_source_first_for_a_user_only_and_cancel_closes_it() {
    let sources = [source(PACKAGE, VERSION, false), source(OTHER, OTHER_VERSION, false), source(GONE, "3.0.0", true)];
    let inspects = vec![
        (PACKAGE.to_string(), inspect_of(PACKAGE, VERSION, &["Button"], &[])),
        (OTHER.to_string(), inspect_of(OTHER, OTHER_VERSION, &["Zed"], &[])),
    ];
    let mut b = Bench::with("open", &sources, inspects);
    assert!(!b.window_open(), "the surface is closed until it is asked for");

    let mark = b.mark();
    b.open(Some(OTHER));
    assert!(b.window_open(), "the command opens the surface for the project of the tab");
    let listed: BTreeSet<String> = b.ran_since(mark, LIST).iter().filter_map(|p| p["package"].as_str().map(str::to_string)).collect();
    assert_eq!(listed, BTreeSet::from([PACKAGE.to_string(), OTHER.to_string()]), "every active source is read, a removed one is not");
    let shown = b.shown();
    assert!(mentions(&shown, "Zed") && mentions(&shown, "Button"), "the components of both sources are listed: {shown:?}");
    assert!(!mentions(&shown, GONE), "a removed source is not shown: {shown:?}");
    let at = |package: &str| shown.iter().position(|text| text.contains(package)).unwrap_or_else(|| panic!("{package} is not shown: {shown:?}"));
    assert!(at(OTHER) < at(PACKAGE), "the requested source comes first: {shown:?}");

    b.click("Cancel");
    assert!(!b.window_open(), "Cancel closes the surface");

    let ctx = b.ui.ctx.clone();
    let call = json!({"command": WINDOW, "params": {"package": OTHER}});
    crate::control::handle(&mut b.ui.app, &ctx, "engine.execute", &call).unwrap();
    b.settle();
    assert!(!b.window_open(), "a call by the control channel opens nothing");

    b.open(None);
    assert!(b.window_open());
    let shown = b.shown();
    let at = |package: &str| shown.iter().position(|text| text.contains(package)).unwrap_or_else(|| panic!("{package} is not shown: {shown:?}"));
    assert!(at(PACKAGE) < at(OTHER), "without a package the sources keep the project's order: {shown:?}");
}

// covers: E4-6.1
#[test]
fn the_file_menu_and_the_actions_palette_open_the_surface() {
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
    assert!(b.window_open(), "running the palette row opens the surface");
}

// E4-6.2 — the states of the reading

// covers: E4-6.2
#[test]
fn while_the_service_reads_the_components_the_surface_says_so_and_cannot_import_then_it_lists_them() {
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
fn a_package_with_no_component_says_so_shows_the_notes_of_the_reading_and_gets_no_card() {
    let note = "No exported component was found in the type declarations";
    let mut b = Bench::with("empty", &[source(PACKAGE, VERSION, false)], vec![(PACKAGE.to_string(), inspect_of(PACKAGE, VERSION, &[], &[note]))]);
    b.open(Some(PACKAGE));
    b.wait_until("the empty state is drawn", |b| b.shows("This package exports no component."));
    let shown = b.shown();
    assert!(mentions(&shown, note), "the notes of the inspection are shown: {shown:?}");
    assert!(!mentions(&shown, "Couldn't read") && !mentions(&shown, "Reading the components"), "{shown:?}");
    assert!(!mentions(&shown, PACKAGE) && !mentions(&shown, "0 of 0"), "a package with no component has no card: {shown:?}");
}

// covers: E4-6.2
#[test]
fn without_a_source_the_surface_sends_the_user_to_the_project_settings() {
    let mut b = Bench::with("no-source", &[], vec![]);
    b.open(None);
    let shown = b.shown();
    assert!(mentions(&shown, "No source library yet. Add one in the project settings."), "{shown:?}");
    assert!(b.inspected_packages().is_empty(), "there is nothing to read");
    assert!(b.ui.app.ui.project_settings.is_none());

    let mark = b.mark();
    b.click("Open project settings");
    assert_eq!(b.ran_since(mark, "project.settings").len(), 1, "the button runs project.settings");
    assert!(b.ui.app.ui.project_settings.is_some(), "the project settings open");
}

// E4-6.6 — catalogs

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

const NEW_KEYS: [&str; 20] = [
    "Search components",
    "Reading the components of {package}…",
    "Couldn't read the components of {package}: {cause}",
    "This package exports no component.",
    "No source library yet. Add one in the project settings.",
    "Open project settings",
    "States",
    "Themes",
    "Animations",
    "Your selection",
    "Already taken in {package}",
    "Choose at least one component.",
    "Choose at least one value in each group.",
    "Importing… {done} of {total}",
    "Imported {done} of {total} components",
    "Couldn't import {name}: {cause}",
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
fn every_language_translates_the_surface_texts_and_the_four_command_labels() {
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
        assert!(keys.contains(key), "the surface uses the key {key:?}: {keys:?}");
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
        "Search components",
        "This package exports no component.",
        "Your selection",
        "Importing… {done} of {total}",
        "Imported {done} of {total} components",
    ] {
        assert!(text_of(fr, key).is_some_and(|text| text != key), "fr translates {key:?}");
    }
}
