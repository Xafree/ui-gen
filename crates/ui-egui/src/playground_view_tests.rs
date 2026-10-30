//! The Playground tab (E7-6.1, E7-6.4, E7-6.5), driven through egui headless: the menu command
//! opens a tab that replaces the canvas and shows the images the capture service sends, and each
//! of the six states (empty, loading, error, busy service, success, stale) has its own rendering and
//! action. The service is a scripted fake backend; the strings of the 22 catalogs are checked by
//! `i18n/tests.rs`. This file also holds the harness `playground_input_tests.rs` reuses.

use std::collections::{BTreeSet, VecDeque};
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use base64::Engine as _;
use egui::accesskit::{Node, Role};
use egui::epaint::ClippedShape;
use egui::{Color32, Event, ImageData, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape, TextureId, Vec2, pos2, vec2};
use figcraft_color::Color;
use figcraft_doc::{Collection, Document, Mode, Variable, VariableType, VariableValue};
use figcraft_engine::Session;
use figcraft_engine::capture::{CaptureBackend, CaptureError, CaptureErrorKind, CaptureJob, JobId};
use figcraft_engine::playground::Status;
use serde_json::{Value, json};

use crate::{FigApp, NoServices};

pub(crate) const IMAGE: [usize; 2] = [160, 100];
pub(crate) const RED: [u8; 3] = [230, 30, 30];
const PREVIEW_FR: &str = "Aperçu du playground";
pub(crate) const DOC_TAB: &str = "lib";
const STEP: f64 = 0.05;
static NEXT: AtomicUsize = AtomicUsize::new(0);

pub(crate) type Outcome = Result<Value, CaptureError>;

#[derive(Default)]
struct State {
    submitted: Vec<CaptureJob>,
    ready: Vec<(JobId, Outcome)>,
    held: Vec<(JobId, Outcome)>,
    hold: BTreeSet<String>,
    open: Option<Outcome>,
    frames: VecDeque<Outcome>,
}

#[derive(Clone, Default)]
pub(crate) struct Calls(Arc<Mutex<State>>);

impl Calls {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap()
    }

    pub(crate) fn params(&self, method: &str) -> Vec<Value> {
        self.lock()
            .submitted
            .iter()
            .filter_map(|job| match job {
                CaptureJob::Playground { method: m, params, .. } if m == method => Some(params.clone()),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn count(&self, method: &str) -> usize {
        self.params(method).len()
    }

    pub(crate) fn playground_jobs(&self) -> usize {
        self.lock().submitted.iter().filter(|job| matches!(job, CaptureJob::Playground { .. })).count()
    }

    pub(crate) fn input_events(&self) -> Vec<Value> {
        self.params("playground.input").iter().flat_map(|p| p["events"].as_array().cloned().unwrap_or_default()).collect()
    }

    pub(crate) fn fail_open(&self, kind: CaptureErrorKind, message: &str) {
        self.lock().open = Some(Err(CaptureError { kind, message: message.to_string() }));
    }

    pub(crate) fn open_ok(&self) {
        self.lock().open = None;
    }

    pub(crate) fn push_frame(&self, outcome: Outcome) {
        self.lock().frames.push_back(outcome);
    }

    pub(crate) fn hold(&self, method: &str) {
        self.lock().hold.insert(method.to_string());
    }

    pub(crate) fn release(&self) {
        let mut state = self.lock();
        state.hold.clear();
        let held = std::mem::take(&mut state.held);
        state.ready.extend(held);
    }
}

struct Fake(Calls);

impl CaptureBackend for Fake {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let mut state = self.0.lock();
        let id = u64::try_from(state.submitted.len() + 1).unwrap();
        if let CaptureJob::Playground { method, params, .. } = &job {
            let outcome = match method.as_str() {
                "playground.open" => {
                    state.open.clone().unwrap_or_else(|| Ok(json!({"defined": ["ui-button"], "missing": [], "errors": [], "shape": true})))
                }
                "playground.frame" => state.frames.pop_front().unwrap_or_else(|| Ok(json!({"seq": params["after"], "changed": false}))),
                _ => Ok(json!({})),
            };
            if state.hold.contains(method) {
                state.held.push((id, outcome));
            } else {
                state.ready.push((id, outcome));
            }
        }
        state.submitted.push(job);
        id
    }

    fn poll(&mut self) -> Vec<(JobId, Outcome)> {
        std::mem::take(&mut self.0.lock().ready)
    }

    fn shutdown(&mut self) {}
}

pub(crate) fn jpeg_frame(seq: u64, width: u32, height: u32, rgb: [u8; 3]) -> Outcome {
    let picture = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(width, height, image::Rgb(rgb)));
    let mut bytes = Vec::new();
    picture.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Jpeg).unwrap();
    Ok(
        json!({"seq": seq, "changed": true, "format": "jpeg", "width": width, "height": height, "data": base64::engine::general_purpose::STANDARD.encode(bytes)}),
    )
}

pub(crate) struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-playground-ui-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub(crate) fn root(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Library {
    Full,
    NoComponents,
}

fn exec(s: &mut Session, command: &str, params: Value) -> Value {
    s.execute(command, &params).unwrap()
}

fn component(s: &mut Session, name: &str, x: f64) {
    let frame = exec(s, "frame.create", json!({"x": x, "y": 0, "width": 100, "height": 40, "name": name}))["id"].clone();
    exec(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 40, "parent": frame, "props": {"name": "Bg", "fill": "#ff0000"}}));
    exec(s, "text.create", json!({"text": "Click", "x": 10, "y": 10, "parent": frame, "props": {"name": "Label"}}));
    exec(s, "component.create", json!({"ids": [frame]}));
}

fn library_document(library: Library) -> Document {
    let mut doc = Document::new("lib");
    let page = doc.pages[0];
    doc.node_mut(page).unwrap().name = "Library".to_string();
    doc.variables.collections.push(Collection {
        id: "c-theme".to_string(),
        name: "Theme".to_string(),
        modes: vec![Mode { id: "m-light".to_string(), name: "Light".to_string() }, Mode { id: "m-dark".to_string(), name: "Dark".to_string() }],
        extends: None,
    });
    let color = |hex: &str| VariableValue::Color(Color::from_hex(hex).unwrap());
    doc.variables.variables.push(Variable {
        id: "v:primary".to_string(),
        name: "color/primary".to_string(),
        collection: "c-theme".to_string(),
        kind: VariableType::Color,
        values: [("m-light".to_string(), color("#E5532D")), ("m-dark".to_string(), color("#FF7A59"))].into_iter().collect(),
        description: String::new(),
        scopes: None,
        hidden_from_publishing: false,
        extends: None,
        overrides: BTreeSet::new(),
        code_syntax: Default::default(),
    });
    let mut s = Session::with_document(doc);
    if library == Library::Full {
        component(&mut s, "Button", 0.0);
        component(&mut s, "Card", 200.0);
    }
    s.doc().unwrap().clone()
}

fn write_project(dir: &Dir, library: Library) {
    let manifest = json!({"version": 1, "name": "mylib", "designFiles": ["lib.figcraft"], "sources": [], "appliedTheme": "Theme"});
    std::fs::write(dir.0.join("uigen.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    std::fs::write(dir.0.join("lib.figcraft"), figcraft_format::save_bytes(&library_document(library), None).unwrap()).unwrap();
}

pub(crate) struct Upload {
    pub(crate) id: TextureId,
    pub(crate) size: [usize; 2],
    pub(crate) pixel: Color32,
}

pub(crate) struct View {
    pub(crate) ctx: egui::Context,
    pub(crate) app: FigApp,
    pub(crate) screen: Vec2,
    time: f64,
    nodes: Vec<Node>,
    shapes: Vec<ClippedShape>,
    uploads: Vec<Upload>,
}

impl View {
    fn new(session: Session, language: &str) -> Self {
        figcraft_text::scan_system_fonts();
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = FigApp::new(session, Box::new(NoServices));
        app.session.prefs.language = language.into();
        let mut view = Self { ctx, app, screen: vec2(1440.0, 900.0), time: 0.0, nodes: Vec::new(), shapes: Vec::new(), uploads: Vec::new() };
        view.settle(0.2);
        view
    }

    pub(crate) fn frame_for(&mut self, events: Vec<Event>, step: f64) {
        self.time += step;
        self.app.clock.set_test_now(Some(self.time));
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.screen)), time: Some(self.time), events, ..Default::default() };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.ui(ui));
        for (id, deltas) in &out.textures_delta.set {
            for delta in deltas.iter().filter(|d| d.pos.is_none()) {
                let ImageData::Color(picture) = &delta.image;
                self.uploads.push(Upload { id: *id, size: picture.size, pixel: picture.pixels.first().copied().unwrap_or_default() });
            }
        }
        out.textures_delta.clear();
        if let Some(update) = out.platform_output.accesskit_update {
            self.nodes = update.nodes.into_iter().map(|(_, node)| node).collect();
        }
        self.shapes = out.shapes;
    }

    pub(crate) fn frame(&mut self, events: Vec<Event>) {
        self.frame_for(events, 1.0 / 60.0);
    }

    pub(crate) fn settle(&mut self, secs: f64) {
        for _ in 0..(secs / STEP).ceil() as usize {
            self.frame_for(vec![], STEP);
        }
    }

    pub(crate) fn move_to(&mut self, at: Pos2) {
        self.frame(vec![Event::PointerMoved(at)]);
    }

    pub(crate) fn click(&mut self, at: Pos2) {
        let button = |pressed| Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        self.frame(vec![Event::PointerMoved(at), button(true)]);
        self.frame(vec![button(false)]);
        self.frame(vec![]);
    }

    pub(crate) fn press(&mut self, key: egui::Key, modifiers: Modifiers) {
        let event = |pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
        self.frame(vec![Event::ModifiersChanged(modifiers), event(true), event(false)]);
        self.frame(vec![Event::ModifiersChanged(Modifiers::NONE)]);
    }

    pub(crate) fn node(&self, role: Role, label: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.role() == role && n.label().is_some_and(|l| l.trim() == label))
    }

    pub(crate) fn has(&self, role: Role, label: &str) -> bool {
        self.node(role, label).is_some()
    }

    pub(crate) fn with_role(&self, role: Role) -> Vec<&Node> {
        self.nodes.iter().filter(|n| n.role() == role).collect()
    }

    pub(crate) fn bounds(node: &Node) -> Rect {
        let b = node.bounds().expect("a node with bounds");
        Rect::from_min_max(pos2(b.x0 as f32, b.y0 as f32), pos2(b.x1 as f32, b.y1 as f32))
    }

    pub(crate) fn click_node(&mut self, role: Role, label: &str) {
        let at = Self::bounds(self.node(role, label).unwrap_or_else(|| panic!("no {role:?} named {label:?} on screen"))).center();
        self.click(at);
    }

    /// The rectangle of the page view: the one image-role node on screen.
    pub(crate) fn zone(&self) -> Rect {
        let images = self.with_role(Role::Image);
        assert_eq!(images.len(), 1, "exactly one image-role node (the page view) is on screen, got {}", images.len());
        Self::bounds(images[0])
    }

    pub(crate) fn texts(&self) -> Vec<String> {
        fn walk(shape: &Shape, out: &mut Vec<String>) {
            match shape {
                Shape::Text(t) => out.push(t.galley.text().to_string()),
                Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        self.shapes.iter().for_each(|c| walk(&c.shape, &mut out));
        out
    }

    /// Every string a user or a screen reader gets: painted text and accessibility names.
    pub(crate) fn everything_said(&self) -> String {
        let labels = self.nodes.iter().flat_map(|n| [n.label(), n.description()].into_iter().flatten().map(str::to_string));
        self.texts().into_iter().chain(labels).collect::<Vec<_>>().join("\n")
    }

    pub(crate) fn says(&self, needle: &str) -> bool {
        self.everything_said().to_lowercase().contains(&needle.to_lowercase())
    }

    fn textured(&self) -> Vec<(TextureId, Rect)> {
        fn walk(shape: &Shape, out: &mut Vec<(TextureId, Rect)>) {
            match shape {
                Shape::Mesh(m) => out.push((m.texture_id, m.calc_bounds())),
                Shape::Rect(r) => out.push((r.fill_texture_id(), shape.visual_bounding_rect())),
                Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        self.shapes.iter().for_each(|c| walk(&c.shape, &mut out));
        out
    }

    /// Where, and with which first pixel, the 160 x 100 picture the fake service sent is painted.
    pub(crate) fn image(&self) -> Option<(Rect, Color32)> {
        let shown = self.textured();
        self.uploads.iter().rev().filter(|u| u.size == IMAGE).find_map(|u| shown.iter().find(|(id, _)| *id == u.id).map(|(_, rect)| (*rect, u.pixel)))
    }
}

pub(crate) struct Bench {
    pub(crate) v: View,
    pub(crate) calls: Calls,
    pub(crate) dir: Dir,
}

impl Bench {
    pub(crate) fn new(tag: &str, language: &str, library: Library) -> Self {
        let mut bench = Self::without_backend(tag, language, library);
        bench.v.app.session.set_capture_backend(Box::new(Fake(bench.calls.clone())));
        bench
    }

    pub(crate) fn without_backend(tag: &str, language: &str, library: Library) -> Self {
        let dir = Dir::new(tag);
        write_project(&dir, library);
        let mut v = View::new(Session::empty(), language);
        assert!(v.app.open_project_at(dir.0.clone(), false));
        v.app.ui.toast = None;
        v.app.ui.home = false;
        v.settle(0.3);
        Self { v, calls: Calls::default(), dir }
    }

    /// A library project whose playground is open and shows the first image.
    pub(crate) fn ready(tag: &str, language: &str) -> Self {
        let mut bench = Self::new(tag, language, Library::Full);
        bench.calls.push_frame(jpeg_frame(1, IMAGE[0] as u32, IMAGE[1] as u32, RED));
        assert!(bench.v.app.run("playground.open", json!({})).is_some(), "the menu command opens the playground");
        bench.v.settle(1.0);
        assert_eq!(*bench.v.app.session.playground_status(), Status::Ready);
        assert!(bench.v.image().is_some(), "the first image is painted");
        bench
    }
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1.0
}

fn is_red(pixel: Color32) -> bool {
    pixel.r() > 180 && pixel.g() < 90 && pixel.b() < 90
}

fn tab_name() -> String {
    crate::i18n::tr("Playground").to_string()
}

// covers: E7-6.1
#[test]
fn playground_open_adds_a_tab_in_place_of_the_canvas_and_shows_the_received_image() {
    let mut b = Bench::new("open", "en", Library::Full);
    b.calls.push_frame(jpeg_frame(1, IMAGE[0] as u32, IMAGE[1] as u32, RED));
    assert!(b.v.has(Role::Group, "Canvas"), "the canvas shows before the playground opens");
    assert!(b.v.app.run("playground.open", json!({})).is_some());
    b.v.settle(1.0);

    assert_eq!(*b.v.app.session.playground_status(), Status::Ready);
    let tab = b.v.node(Role::Tab, "Playground").expect("a tab named Playground");
    assert_eq!(tab.is_selected(), Some(true), "the Playground tab is the one on screen");
    assert!(b.v.node(Role::Group, "Canvas").is_none(), "the view replaces the canvas");

    let (painted, pixel) = b.v.image().expect("the received image is a texture of the view");
    let zone = b.v.zone();
    assert!(near(painted.width(), zone.width()) && near(painted.height(), zone.height()), "{painted:?} fills the zone {zone:?}");
    assert!(zone.width() > 400.0 && zone.height() > 300.0, "the view takes the canvas area: {zone:?}");
    assert!(is_red(pixel), "the JPEG was decoded: {pixel:?}");
}

// covers: E7-6.1
#[test]
fn playground_open_twice_keeps_a_single_playground_tab() {
    let mut b = Bench::new("twice", "en", Library::Full);
    assert!(b.v.app.run("playground.open", json!({})).is_some());
    b.v.settle(0.5);
    assert!(b.v.app.run("playground.open", json!({})).is_some());
    b.v.settle(0.5);
    let tabs = b.v.with_role(Role::Tab).into_iter().filter(|n| n.label() == Some("Playground")).count();
    assert_eq!(tabs, 1);
    assert_eq!(*b.v.app.session.playground_status(), Status::Ready);
}

// covers: E7-6.1
#[test]
fn playground_closing_the_tab_sends_close_and_brings_the_canvas_back() {
    let mut b = Bench::ready("close", "en");
    assert_eq!(b.calls.count("playground.close"), 0);
    let tab = View::bounds(b.v.node(Role::Tab, "Playground").unwrap());
    b.v.click(pos2(tab.right() - 16.0, tab.center().y));
    b.v.settle(0.5);
    assert!(b.v.node(Role::Tab, "Playground").is_none(), "the tab is gone");
    assert_eq!(b.calls.count("playground.close"), 1);
    assert_eq!(*b.v.app.session.playground_status(), Status::Closed);
    assert!(b.v.has(Role::Group, "Canvas"), "the canvas is back");
}

// covers: E7-6.1
#[test]
fn playground_open_is_refused_without_a_capture_service_and_adds_no_tab() {
    let mut b = Bench::without_backend("nobackend", "en", Library::Full);
    assert!(b.v.app.run("playground.open", json!({})).is_none(), "the command fails when the service is not available");
    b.v.settle(0.5);
    assert!(b.v.node(Role::Tab, "Playground").is_none());
    assert_eq!(b.calls.playground_jobs(), 0);
}

// covers: E7-6.5
#[test]
fn playground_open_is_a_command_of_the_menu_catalog() {
    assert!(figcraft_engine::catalog::MENU.iter().any(|m| m.id == "playground.open"), "the menu lists playground.open");
}

// covers: E7-6.5
#[test]
fn playground_view_is_an_image_named_apercu_du_playground() {
    let b = Bench::ready("a11y", "fr");
    assert!(b.v.has(Role::Image, PREVIEW_FR), "the view is an image named {PREVIEW_FR:?}");
    assert!(b.v.has(Role::Tab, &tab_name()));
}

// covers: E7-6.5
#[test]
fn playground_asks_for_images_only_while_the_tab_is_shown() {
    let mut b = Bench::ready("shown", "en");
    assert!(b.calls.count("playground.frame") >= 1, "a shown tab asks the service for images");
    b.v.click_node(Role::Tab, DOC_TAB);
    b.v.settle(0.3);
    assert!(b.v.node(Role::Group, "Canvas").is_some(), "the document shows again");
    let asked = b.calls.count("playground.frame");
    b.v.settle(3.0);
    assert_eq!(b.calls.count("playground.frame"), asked, "no request while the tab is hidden");
    b.v.click_node(Role::Tab, "Playground");
    b.v.settle(1.0);
    assert!(b.calls.count("playground.frame") > asked, "requests resume once the tab is shown again");
}

// covers: E7-6.5
#[test]
fn playground_sends_no_second_image_request_while_one_is_in_flight() {
    let mut b = Bench::new("flight", "en", Library::Full);
    b.calls.hold("playground.frame");
    assert!(b.v.app.run("playground.open", json!({})).is_some());
    b.v.settle(2.0);
    assert_eq!(b.calls.count("playground.frame"), 1, "one request, still unanswered");
    b.calls.push_frame(jpeg_frame(1, IMAGE[0] as u32, IMAGE[1] as u32, RED));
    b.calls.release();
    b.v.settle(1.0);
    assert!(b.calls.count("playground.frame") >= 2, "the next request goes out once the answer is in");
}

// covers: E7-6.4
#[test]
fn playground_empty_state_links_to_the_export_window() {
    let mut b = Bench::new("empty", "fr", Library::NoComponents);
    assert!(b.v.app.run("playground.open", json!({})).is_none(), "the engine refuses a library without components");
    b.v.settle(0.5);
    assert!(b.v.has(Role::Tab, &tab_name()), "the Playground tab shows its empty state");
    assert_eq!(b.calls.playground_jobs(), 0, "nothing is asked of the service");
    let links = b.v.with_role(Role::Link);
    assert!(!links.is_empty(), "the empty state offers a link");
    assert!(b.v.app.ui.lit_export.is_none());
    let at = View::bounds(links[0]).center();
    b.v.click(at);
    assert!(b.v.app.ui.lit_export.is_some(), "the link opens the export window");
}

fn loading_is_visible(v: &View) -> bool {
    v.says("chargement") || !v.with_role(Role::ProgressIndicator).is_empty()
}

// covers: E7-6.4
#[test]
fn playground_loading_state_shows_until_the_page_is_open() {
    let mut b = Bench::new("loading", "fr", Library::Full);
    b.calls.hold("playground.open");
    b.calls.push_frame(jpeg_frame(1, IMAGE[0] as u32, IMAGE[1] as u32, RED));
    assert!(b.v.app.run("playground.open", json!({})).is_some());
    b.v.settle(0.5);
    assert_eq!(*b.v.app.session.playground_status(), Status::Opening);
    assert!(b.v.has(Role::Tab, &tab_name()));
    assert!(loading_is_visible(&b.v), "a loading indicator or the word chargement: {}", b.v.everything_said());
    assert!(b.v.image().is_none());
    b.calls.release();
    b.v.settle(1.0);
    assert_eq!(*b.v.app.session.playground_status(), Status::Ready);
    assert!(b.v.image().is_some(), "the image replaces the loading state");
    assert!(!loading_is_visible(&b.v), "the loading state is gone");
}

// covers: E7-6.4
#[test]
fn playground_error_state_names_the_cause_and_retry_opens_the_page_again() {
    let cause = "bundle failed in components/button.ts: unexpected token";
    let mut b = Bench::new("error", "fr", Library::Full);
    b.calls.fail_open(CaptureErrorKind::Render, cause);
    assert!(b.v.app.run("playground.open", json!({})).is_some());
    b.v.settle(1.0);
    assert!(matches!(b.v.app.session.playground_status(), Status::Failed(_)));
    assert!(b.v.says(cause), "the cause of the service is on screen: {}", b.v.everything_said());
    let root = b.dir.root();
    assert!(!b.v.everything_said().contains(&root) && !b.v.everything_said().contains(&root.replace('\\', "/")), "no absolute path is shown");
    assert!(b.v.has(Role::Button, "Réessayer"), "a Réessayer button");
    assert!(b.v.image().is_none());

    b.calls.open_ok();
    b.calls.push_frame(jpeg_frame(1, IMAGE[0] as u32, IMAGE[1] as u32, RED));
    b.v.click_node(Role::Button, "Réessayer");
    b.v.settle(1.0);
    assert_eq!(b.calls.count("playground.open"), 2, "Réessayer opens the page again");
    assert_eq!(*b.v.app.session.playground_status(), Status::Ready);
    assert!(!b.v.says(cause), "the error is gone");
    assert!(b.v.image().is_some());
}

// covers: E7-6.4
#[test]
fn playground_busy_service_keeps_the_last_image_and_asks_for_no_more() {
    let mut b = Bench::ready("busy", "fr");
    let root = b.dir.root();
    let job = CaptureJob::Install { root, package: "left-pad".to_string(), version: "1.0.0".to_string() };
    b.v.app.session.submit_capture("project.addSource", "left-pad", job).unwrap();
    b.v.settle(0.5);
    assert_eq!(b.v.app.session.execute("playground.state", &json!({})).unwrap()["busy"], json!(true));
    assert!(b.v.says("occupé"), "the service is busy: {}", b.v.everything_said());
    assert!(b.v.image().is_some_and(|(_, pixel)| is_red(pixel)), "the last image stays on screen");
    let asked = b.calls.count("playground.frame");
    b.v.settle(1.5);
    assert_eq!(b.calls.count("playground.frame"), asked, "no image request while the service is busy");
}

// covers: E7-6.4
#[test]
fn playground_stale_state_says_the_file_changed_and_actualiser_opens_the_page_again() {
    let mut b = Bench::ready("stale", "fr");
    assert!(!b.v.says("le fichier a changé"));
    assert!(!b.v.has(Role::Button, "Actualiser"));
    b.v.app.run("frame.create", json!({"x": 0, "y": 400, "width": 10, "height": 10}));
    b.v.settle(0.5);
    assert_eq!(b.v.app.session.execute("playground.state", &json!({})).unwrap()["stale"], json!(true));
    assert!(b.v.says("le fichier a changé"), "{}", b.v.everything_said());
    assert!(b.v.has(Role::Button, "Actualiser"), "an Actualiser button");
    assert!(b.v.image().is_some(), "the stale image stays on screen");

    b.calls.push_frame(jpeg_frame(1, IMAGE[0] as u32, IMAGE[1] as u32, RED));
    b.v.click_node(Role::Button, "Actualiser");
    b.v.settle(1.0);
    assert_eq!(b.calls.count("playground.open"), 2, "Actualiser opens the page again on the new file");
    assert_eq!(b.v.app.session.execute("playground.state", &json!({})).unwrap()["stale"], json!(false));
    assert!(!b.v.says("le fichier a changé") && !b.v.has(Role::Button, "Actualiser"));
}
