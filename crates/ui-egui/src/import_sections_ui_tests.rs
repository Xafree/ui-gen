//! Components tab: one "Imported · <package> <version>" section per source of the library project,
//! after "Local styles" and before the enabled libraries. A section lists the component sets of its
//! import page as tiles (thumbnail of the default variant, kept apart from the local ones), folds,
//! is searched, says "source removed", and its tiles offer "Copy to the current page" and "Show in
//! the import page" (pointer and keyboard). The search row and each section have an "Import a
//! component…" button that runs `uigen.importWindow`. The import pages are made by a real staged
//! import (`uigen.importComponent` with `stage: true`) against a scripted capture backend.
//!
//! The 22 catalogs holding every new text are checked by `i18n/tests.rs`; the button's absence in the
//! browser build is a `cfg!(target_arch = "wasm32")` condition that a native test cannot flip.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use egui::accesskit::{Node, Role};
use egui::{Event, Key, Modifiers, Pos2, Vec2, pos2, vec2};
use figcraft_doc::{NodeId, NodeKind};
use figcraft_engine::capture::{CaptureBackend, CaptureError, CaptureJob, JobId};
use serde_json::{Value, json};

use crate::a11y::focused_name;
use crate::layers_drag_tests::Ui;
use crate::live_tests::{flat, texts};
use crate::{LeftTab, RightTab};

static NEXT: AtomicUsize = AtomicUsize::new(0);

const PACKAGE: &str = "synthetic-ui";
const VERSION: &str = "1.0.0";
const OTHER: &str = "other-ui";
const OTHER_VERSION: &str = "2.0.0";
const RAIL_WIDTH: f32 = 56.0;
const MAX_FRAMES: usize = 300;
const IMPORT_BUTTON: &str = "Import a component…";
const COPY: &str = "Copy to the current page";
const SHOW: &str = "Show in the import page";
const NOTHING: &str = "Nothing imported from this source yet.";
const DEV_MODE_REFUSAL: &str = "Dev Mode only inspects: switch to Design (Shift+D) to edit";
const TILE_HINT: &str = "Drag onto the canvas to copy it into your library.";

type Outcome = Result<Value, CaptureError>;
type Painted = (String, Pos2, Vec2);

fn title_of(package: &str, version: &str) -> String {
    format!("Imported · {package} {version}")
}

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-import-sections-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
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

fn source(package: &str, version: &str) -> Value {
    json!({"package": package, "version": version, "framework": "react", "license": "MIT", "removed": false})
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
        "components": [{"name": "Button", "props": props}, {"name": "Chip", "props": props}],
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
            CaptureJob::InstallMany { .. } | CaptureJob::Uninstall { .. } | CaptureJob::Playground { .. } => Ok(json!({})),
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

/// An open library project (page "Card"), the Components tab showing, a scripted capture service.
struct Lab {
    ui: Ui,
    _dir: Dir,
}

fn id_in(v: &Value, key: &str) -> NodeId {
    NodeId(v[key].as_u64().unwrap_or_else(|| panic!("no {key} in {v}")))
}

impl Lab {
    fn new(tag: &str, sources: &[Value]) -> Self {
        figcraft_text::scan_system_fonts();
        let dir = Dir::new(tag);
        let manifest = json!({"version": 1, "name": "mylib", "designFiles": ["lib.figcraft"], "sources": sources});
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
        ui.app.ui.left_collapsed = false;
        let mut lab = Self { ui, _dir: dir };
        lab.settle();
        lab
    }

    fn with_imports(tag: &str, sources: &[Value], components: &[&str]) -> Self {
        let mut lab = Self::new(tag, sources);
        for component in components {
            lab.import(component);
        }
        lab
    }

    fn show(&mut self) {
        self.ui.app.ui.left_tab = LeftTab::Assets;
        self.ui.app.ui.left_collapsed = false;
        self.settle();
        self.settle();
    }

    fn settle(&mut self) {
        for _ in 0..3 {
            self.ui.frame(vec![]);
        }
    }

    fn run(&mut self, id: &str, params: Value) -> Value {
        self.ui.app.session.execute(id, &params).unwrap_or_else(|e| panic!("{id}: {e}"))
    }

    fn import(&mut self, component: &str) {
        let params = json!({"package": PACKAGE, "component": component, "states": ["Default"], "themes": ["light"], "stage": true});
        assert!(self.ui.app.run("uigen.importComponent", params).is_some(), "the import of {component} starts");
        for _ in 0..MAX_FRAMES {
            self.ui.app.session.pump_capture();
            self.ui.frame(vec![]);
            if self.sets_named(component, PACKAGE, VERSION).is_some() {
                self.settle();
                return;
            }
        }
        panic!("{component} never reached its import page");
    }

    fn sections(&mut self) -> Vec<Value> {
        self.run("uigen.imported", json!({}))["sections"].as_array().cloned().unwrap_or_default()
    }

    fn sets_named(&mut self, name: &str, package: &str, version: &str) -> Option<NodeId> {
        let sections = self.sections();
        let section = sections.iter().find(|s| s["package"] == package && s["version"] == version)?;
        let found = section["components"].as_array()?.iter().find(|c| c["name"] == name)?;
        Some(id_in(found, "set"))
    }

    fn set(&mut self, name: &str) -> NodeId {
        self.sets_named(name, PACKAGE, VERSION).unwrap_or_else(|| panic!("{name} is not imported"))
    }

    fn import_page(&mut self) -> NodeId {
        let sections = self.sections();
        id_in(&sections[0], "page")
    }

    fn local_component(&mut self, name: &str) -> NodeId {
        let frame = id_in(&self.run("frame.create", json!({"x": 0, "y": 0, "width": 120, "height": 60, "name": name})), "id");
        id_in(&self.run("component.create", json!({"ids": [frame]})), "id")
    }

    fn remove_source(&mut self) {
        self.run("project.removeSource", json!({"package": PACKAGE}));
        for _ in 0..MAX_FRAMES {
            self.ui.app.session.pump_capture();
            self.ui.frame(vec![]);
            if self.sections().first().is_some_and(|s| s["removed"] == true) {
                self.settle();
                return;
            }
        }
        panic!("the source was never removed");
    }

    fn mark(&self) -> usize {
        self.ui.app.session.journal.len()
    }

    fn ran_since(&self, mark: usize, command: &str) -> Vec<Value> {
        self.ui.app.session.journal[mark..].iter().filter(|(c, _)| c == command).map(|(_, p)| p.clone()).collect()
    }

    fn panel_texts(&mut self) -> Vec<Painted> {
        self.ui.frame(vec![]);
        let width = RAIL_WIDTH + self.ui.app.ui.left_width;
        let out = self.ui.frame_output(vec![]);
        texts(&flat(&out.shapes)).into_iter().filter(|(_, p, _)| p.x >= RAIL_WIDTH && p.x < width).collect()
    }

    fn all_texts(&mut self) -> Vec<Painted> {
        self.ui.frame(vec![]);
        let out = self.ui.frame_output(vec![]);
        texts(&flat(&out.shapes))
    }

    fn nodes(&mut self) -> Vec<Node> {
        self.ui.ctx.enable_accesskit();
        self.ui.frame(vec![]);
        self.ui.frame_output(vec![]).platform_output.accesskit_update.expect("an AccessKit update").nodes.into_iter().map(|(_, n)| n).collect()
    }

    fn centres_of(&mut self, role: Role, label: &str) -> Vec<Pos2> {
        let mut found: Vec<Pos2> = self
            .nodes()
            .iter()
            .filter(|n| n.role() == role && n.label().is_some_and(|l| l.trim() == label))
            .filter_map(|n| n.bounds())
            .map(|b| pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32))
            .collect();
        found.sort_by(|a, b| a.y.total_cmp(&b.y));
        found
    }

    fn click(&mut self, at: Pos2) {
        self.ui.button(at, true);
        self.ui.button(at, false);
        self.settle();
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

    fn click_item(&mut self, label: &str) {
        let at = *self.centres_of(Role::MenuItem, label).first().unwrap_or_else(|| panic!("no menu item {label:?}"));
        self.click(at);
    }

    fn press(&mut self, key: Key, modifiers: Modifiers) {
        self.ui.frame(vec![
            Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers },
            Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers },
        ]);
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

    fn title(&mut self, package: &str, version: &str) -> Option<Painted> {
        let wanted = title_of(package, version);
        self.panel_texts().into_iter().find(|(t, _, _)| t.starts_with(&wanted))
    }

    fn row_text(&mut self, package: &str, version: &str) -> String {
        let (_, at, size) = self.title(package, version).unwrap_or_else(|| panic!("no section title for {package} {version}"));
        let mid = at.y + size.y / 2.0;
        let mut row: Vec<Painted> = self.panel_texts().into_iter().filter(|(_, p, s)| (p.y + s.y / 2.0 - mid).abs() < 8.0).collect();
        row.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
        row.into_iter().map(|(t, _, _)| t).collect::<Vec<_>>().join(" ")
    }

    fn labelled(&mut self, name: &str) -> usize {
        self.panel_texts().iter().filter(|(t, _, _)| t == name).count()
    }

    fn type_in_search(&mut self, query: &str) {
        let width = RAIL_WIDTH + self.ui.app.ui.left_width;
        let field = self
            .nodes()
            .iter()
            .filter(|n| matches!(n.role(), Role::TextInput | Role::MultilineTextInput | Role::SearchInput))
            .filter_map(|n| n.bounds())
            .map(|b| pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32))
            .filter(|p| p.x > RAIL_WIDTH && p.x < width)
            .min_by(|a, b| a.y.total_cmp(&b.y))
            .expect("the search field of the Components tab");
        self.click(field);
        self.ui.frame(vec![Event::Text(query.to_string())]);
        self.settle();
    }
}

fn y_of(texts: &[Painted], text: &str) -> f32 {
    texts.iter().find(|(t, _, _)| t == text).map(|(_, p, _)| p.y).unwrap_or_else(|| panic!("no text {text:?} in {:?}", names(texts)))
}

fn y_of_title(texts: &[Painted], package: &str, version: &str) -> f32 {
    let wanted = title_of(package, version);
    texts.iter().find(|(t, _, _)| t.starts_with(&wanted)).map(|(_, p, _)| p.y).unwrap_or_else(|| panic!("no title {wanted:?} in {:?}", names(texts)))
}

fn names(texts: &[Painted]) -> Vec<&str> {
    texts.iter().map(|(t, _, _)| t.as_str()).collect()
}

fn has_title(texts: &[Painted], package: &str, version: &str) -> bool {
    let wanted = title_of(package, version);
    texts.iter().any(|(t, _, _)| t.starts_with(&wanted))
}

fn thumbnails(ctx: &egui::Context) -> Vec<(egui::TextureId, String)> {
    let manager = ctx.tex_manager();
    let manager = manager.read();
    let mut found: Vec<(egui::TextureId, String)> =
        manager.allocated().filter(|(_, meta)| meta.name.starts_with("asset-thumb-")).map(|(id, meta)| (*id, meta.name.clone())).collect();
    found.sort_by(|a, b| a.1.cmp(&b.1).then(format!("{:?}", a.0).cmp(&format!("{:?}", b.0))));
    found
}

fn thumbnail_of(ctx: &egui::Context, node: NodeId) -> Vec<egui::TextureId> {
    let name = format!("asset-thumb-{}", node.0);
    thumbnails(ctx).into_iter().filter(|(_, n)| *n == name).map(|(id, _)| id).collect()
}

fn default_variant(lab: &mut Lab, set: NodeId) -> NodeId {
    let doc = lab.ui.app.session.doc().unwrap();
    figcraft_engine::components::default_variant(doc, set).expect("a set has a default variant")
}

// covers: E4-4.1
#[test]
fn an_imported_set_is_a_tile_under_its_section_after_local_styles_and_not_among_local_components() {
    let mut lab = Lab::with_imports("tile", &[source(PACKAGE, VERSION)], &["Button"]);
    lab.local_component("Card");
    lab.show();

    let shown = lab.panel_texts();
    let (components, styles, section) = (y_of(&shown, "Local components"), y_of(&shown, "Local styles"), y_of_title(&shown, PACKAGE, VERSION));
    assert!(components < styles && styles < section, "Local components, Local styles, then the section: {:?}", names(&shown));
    assert_eq!(lab.labelled("Button"), 1, "one tile named Button: {:?}", names(&shown));
    let tile = y_of(&shown, "Button");
    assert!(tile > section, "the Button tile sits under the section title ({tile} after {section})");
    let local = y_of(&shown, "Card");
    assert!(local > components && local < styles, "the local component stays among the local components");
    assert!(!shown.iter().any(|(t, _, _)| t == NOTHING), "a source with a page is not empty: {:?}", names(&shown));
}

// covers: E4-4.1
#[test]
fn a_project_without_an_import_page_shows_an_empty_section_per_source() {
    let mut lab = Lab::new("empty", &[source(PACKAGE, VERSION), source(OTHER, OTHER_VERSION)]);
    lab.show();

    let shown = lab.panel_texts();
    let styles = y_of(&shown, "Local styles");
    let (first, second) = (y_of_title(&shown, PACKAGE, VERSION), y_of_title(&shown, OTHER, OTHER_VERSION));
    assert!(styles < first && first < second, "one section per source, in the order of the manifest: {:?}", names(&shown));
    assert_eq!(shown.iter().filter(|(t, _, _)| t == NOTHING).count(), 2, "each says nothing is imported yet: {:?}", names(&shown));
    assert!(!shown.iter().any(|(t, _, _)| t.contains("source removed")), "neither is removed");
}

// covers: E4-4.1
#[test]
fn outside_a_project_the_panel_has_no_import_section_and_no_import_button() {
    figcraft_text::scan_system_fonts();
    let mut ui = Ui::new(figcraft_engine::Session::new());
    ui.app.session.prefs.language = "en".into();
    ui.app.ui.left_collapsed = false;
    ui.app.ui.left_tab = LeftTab::Assets;
    let mut lab = Lab { ui, _dir: Dir::new("outside") };
    lab.settle();

    let shown = lab.all_texts();
    assert!(shown.iter().any(|(t, _, _)| t == "Local components"), "the panel shows: {:?}", names(&shown));
    assert!(!shown.iter().any(|(t, _, _)| t.starts_with("Imported ·") || t == NOTHING), "no import section: {:?}", names(&shown));
    assert!(lab.centres_of(Role::Button, IMPORT_BUTTON).is_empty(), "no import button outside a project");
}

// covers: E4-4.2
#[test]
fn a_removed_source_keeps_its_page_section_titled_source_removed() {
    let mut lab = Lab::with_imports("removed", &[source(PACKAGE, VERSION)], &["Button"]);
    lab.show();
    assert!(!lab.row_text(PACKAGE, VERSION).contains("source removed"), "a live source is not marked");

    lab.remove_source();

    let row = lab.row_text(PACKAGE, VERSION);
    assert!(row.contains("source removed"), "the title is followed by source removed: {row:?}");
    assert!(row.find(&title_of(PACKAGE, VERSION)) < row.find("source removed"), "the title comes first: {row:?}");
    assert_eq!(lab.labelled("Button"), 1, "its tiles stay: {:?}", names(&lab.panel_texts()));
}

// covers: E4-4.2
#[test]
fn clicking_the_section_title_folds_and_unfolds_its_tiles() {
    let mut lab = Lab::with_imports("fold", &[source(PACKAGE, VERSION)], &["Button"]);
    lab.show();
    assert_eq!(lab.labelled("Button"), 1);

    let (_, at, size) = lab.title(PACKAGE, VERSION).expect("the section title");
    let title = at + vec2(4.0, size.y / 2.0);
    lab.click(title);
    assert_eq!(lab.labelled("Button"), 0, "folded: no tile");
    assert!(lab.title(PACKAGE, VERSION).is_some(), "the title stays when folded");

    lab.click(title);
    assert_eq!(lab.labelled("Button"), 1, "unfolded: the tile is back");
}

// covers: E4-4.2
#[test]
fn searching_keeps_the_matching_sets_and_hides_the_sections_left_empty() {
    let mut lab = Lab::with_imports("search", &[source(PACKAGE, VERSION), source(OTHER, OTHER_VERSION)], &["Button", "Chip"]);
    lab.show();
    assert_eq!((lab.labelled("Button"), lab.labelled("Chip")), (1, 1));
    assert!(has_title(&lab.panel_texts(), OTHER, OTHER_VERSION), "the second source is listed while nothing is searched");

    lab.type_in_search("chip");

    let shown = lab.panel_texts();
    assert_eq!((lab.labelled("Button"), lab.labelled("Chip")), (0, 1), "only the matching set stays: {:?}", names(&shown));
    assert!(has_title(&shown, PACKAGE, VERSION), "its section stays");
    assert!(!has_title(&shown, OTHER, OTHER_VERSION), "the empty section is hidden: {:?}", names(&shown));
    assert!(!shown.iter().any(|(t, _, _)| t == NOTHING), "so is its empty-state text: {:?}", names(&shown));

    lab.type_in_search("zzz");

    let shown = lab.panel_texts();
    assert!(!has_title(&shown, PACKAGE, VERSION), "no match, no section: {:?}", names(&shown));
}

// covers: E4-4.3
#[test]
fn an_imported_tile_shows_the_default_variant_cached_and_rendered_again_when_the_set_changes() {
    let mut lab = Lab::with_imports("thumb", &[source(PACKAGE, VERSION)], &["Button", "Chip"]);
    lab.show();
    let (button, chip) = (lab.set("Button"), lab.set("Chip"));
    let (shown_button, shown_chip) = (default_variant(&mut lab, button), default_variant(&mut lab, chip));
    lab.settle();

    let first = thumbnail_of(&lab.ui.ctx, shown_button);
    assert_eq!(first.len(), 1, "the default variant is drawn once at first display: {:?}", thumbnails(&lab.ui.ctx));
    assert_eq!(thumbnail_of(&lab.ui.ctx, shown_chip).len(), 1, "so is the other set's");
    assert!(thumbnail_of(&lab.ui.ctx, button).is_empty(), "the set itself is not drawn: its default variant is");
    let chip_first = thumbnail_of(&lab.ui.ctx, shown_chip);
    let local_cache = crate::assets_panel::cached_thumbnails(&lab.ui.ctx);
    assert!(!local_cache.contains(&button) && !local_cache.contains(&chip), "imported sets are not cached as local ones: {local_cache:?}");

    lab.settle();
    assert_eq!(thumbnail_of(&lab.ui.ctx, shown_button), first, "cached: the same texture on later frames");

    lab.run("node.set", json!({"ids": [shown_button], "props": {"fill": "#FF0000"}}));
    lab.ui.advance_clock(100.0);
    lab.settle();
    lab.settle();

    let after = thumbnail_of(&lab.ui.ctx, shown_button);
    assert_eq!(after.len(), 1, "one thumbnail again");
    assert_ne!(after, first, "a changed set is drawn again");
    assert_eq!(thumbnail_of(&lab.ui.ctx, shown_chip), chip_first, "an untouched set keeps its thumbnail");
}

// covers: E4-4.3
#[test]
fn imported_and_local_thumbnails_are_kept_apart() {
    let mut lab = Lab::with_imports("apart", &[source(PACKAGE, VERSION)], &["Button"]);
    let local = lab.local_component("Card");
    lab.show();
    let button = lab.set("Button");
    let shown = default_variant(&mut lab, button);

    let local_cache = crate::assets_panel::cached_thumbnails(&lab.ui.ctx);
    assert_eq!(local_cache, vec![local], "the local cache holds the local tile only: {local_cache:?}");
    assert_eq!(thumbnail_of(&lab.ui.ctx, local).len(), 1, "the local tile has its own picture");
    assert_eq!(thumbnail_of(&lab.ui.ctx, shown).len(), 1, "the imported tile has its own, not the local one's");
    assert_eq!(
        thumbnails(&lab.ui.ctx).iter().filter(|(_, n)| n.starts_with("asset-thumb-")).count(),
        2,
        "two tiles, two pictures: {:?}",
        thumbnails(&lab.ui.ctx)
    );
}

// covers: E4-4.4
#[test]
fn the_menu_of_an_imported_tile_copies_it_to_the_current_page() {
    let mut lab = Lab::with_imports("copy", &[source(PACKAGE, VERSION)], &["Button"]);
    lab.show();
    let (set, start) = (lab.set("Button"), lab.ui.app.session.page().unwrap());
    let mark = lab.mark();

    let at = lab.tile("Button");
    lab.right_click(at);
    assert!(!lab.centres_of(Role::MenuItem, COPY).is_empty() && !lab.centres_of(Role::MenuItem, SHOW).is_empty(), "both commands are offered");
    lab.click_item(COPY);

    let placed = lab.ran_since(mark, "uigen.placeImported");
    assert_eq!(placed.len(), 1, "one copy: {placed:?}");
    assert_eq!(placed[0]["set"], json!(set.0), "of the clicked set");
    let session = &lab.ui.app.session;
    let doc = session.doc().unwrap();
    assert_eq!(session.page().unwrap(), start, "the copy lands on the current page");
    assert_eq!(doc.node(start).unwrap().name, "Button", "which takes the name of the set");
    let copies: Vec<NodeId> =
        doc.node(start).unwrap().children.iter().copied().filter(|c| matches!(doc.node(*c).unwrap().kind, NodeKind::ComponentSet { .. })).collect();
    assert_eq!(copies.len(), 1, "one set on the page");
    assert_ne!(copies[0], set, "an independent copy");
    assert!(doc.node(set).is_some(), "the imported set stays");
    assert!(lab.ui.app.ui.last_error.is_none(), "no error: {:?}", lab.ui.app.ui.last_error);
}

// covers: E4-4.4
#[test]
fn a_local_tile_keeps_its_own_menu() {
    let mut lab = Lab::with_imports("local-menu", &[source(PACKAGE, VERSION)], &["Button"]);
    lab.local_component("Card");
    lab.show();

    let at = lab.tile("Card");
    lab.right_click(at);

    assert!(lab.centres_of(Role::MenuItem, COPY).is_empty() && lab.centres_of(Role::MenuItem, SHOW).is_empty(), "no import commands on a local tile");
    assert!(lab.all_texts().iter().any(|(t, _, _)| t.contains("Hide when publishing")), "the local menu is unchanged: {:?}", names(&lab.all_texts()));
}

// covers: E4-4.4
#[test]
fn show_in_the_import_page_goes_to_that_page_and_selects_the_set() {
    let mut lab = Lab::with_imports("show", &[source(PACKAGE, VERSION)], &["Button", "Chip"]);
    lab.show();
    let (set, page) = (lab.set("Button"), lab.import_page());
    assert_ne!(lab.ui.app.session.page().unwrap(), page, "the library page is current");
    let mark = lab.mark();

    let at = lab.tile("Button");
    lab.right_click(at);
    lab.click_item(SHOW);

    let selected = lab.ran_since(mark, "page.select");
    assert!(selected.iter().any(|p| p["id"] == json!(page.0)), "page.select aims at the import page: {selected:?}");
    assert_eq!(lab.ui.app.session.page().unwrap(), page, "the import page is current");
    assert_eq!(lab.ui.app.session.selection(), vec![set], "the set is selected");
    assert!(lab.ran_since(mark, "uigen.placeImported").is_empty(), "nothing is copied");
}

// covers: E4-4.4
#[test]
fn dev_mode_refuses_the_copy_with_the_usual_message() {
    let mut lab = Lab::with_imports("dev-mode", &[source(PACKAGE, VERSION)], &["Button"]);
    lab.show();
    let start = lab.ui.app.session.page().unwrap();
    lab.ui.app.ui.right_tab = RightTab::Inspect;
    lab.settle();
    let mark = lab.mark();

    let at = lab.tile("Button");
    lab.right_click(at);
    lab.click_item(COPY);

    assert!(lab.ran_since(mark, "uigen.placeImported").is_empty(), "the command never ran");
    assert_eq!(lab.ui.app.ui.toast.as_ref().map(|t| t.0.as_str()), Some(DEV_MODE_REFUSAL));
    assert_eq!(lab.ui.app.session.doc().unwrap().node(start).unwrap().name, "Card", "the page is untouched");
}

fn tab_to_tile(lab: &mut Lab, name: &str) {
    lab.ui.ctx.enable_accesskit();
    let wanted = format!("{name}\n{TILE_HINT}");
    for _ in 0..3 {
        lab.press(Key::F6, Modifiers::NONE);
        lab.settle();
    }
    for _ in 0..400 {
        lab.press(Key::Tab, Modifiers::NONE);
        if focused_name(&lab.ui.ctx) == wanted {
            return;
        }
    }
    panic!("Tab never reached the {name} tile; stopped on {:?}", focused_name(&lab.ui.ctx));
}

fn copy_with_the_keyboard(lab: &mut Lab) {
    for _ in 0..8 {
        if focused_name(&lab.ui.ctx) == COPY {
            break;
        }
        lab.press(Key::ArrowDown, Modifiers::NONE);
        lab.settle();
    }
    assert_eq!(focused_name(&lab.ui.ctx), COPY, "the arrows reach the copy command");
    lab.press(Key::Enter, Modifiers::NONE);
    lab.settle();
}

// covers: E4-4.4
#[test]
fn shift_f10_on_a_focused_imported_tile_opens_its_menu_and_the_copy_runs_from_the_keyboard() {
    let mut lab = Lab::with_imports("kbd-shift", &[source(PACKAGE, VERSION)], &["Button"]);
    lab.show();
    let set = lab.set("Button");
    let mark = lab.mark();

    tab_to_tile(&mut lab, "Button");
    lab.press(Key::F10, Modifiers::SHIFT);
    lab.settle();

    assert!(!lab.centres_of(Role::MenuItem, COPY).is_empty() && !lab.centres_of(Role::MenuItem, SHOW).is_empty(), "Shift+F10 opens the tile menu");
    copy_with_the_keyboard(&mut lab);
    let placed = lab.ran_since(mark, "uigen.placeImported");
    assert_eq!(placed.len(), 1, "the keyboard ran the copy: {placed:?}");
    assert_eq!(placed[0]["set"], json!(set.0));
}

// covers: E4-4.4
#[test]
fn the_menu_key_on_a_focused_imported_tile_opens_its_menu_and_show_runs_from_the_keyboard() {
    let mut lab = Lab::with_imports("kbd-menu", &[source(PACKAGE, VERSION)], &["Button"]);
    lab.show();
    let page = lab.import_page();
    let mark = lab.mark();

    tab_to_tile(&mut lab, "Button");
    lab.press(Key::Menu, Modifiers::NONE);
    lab.settle();

    assert!(!lab.centres_of(Role::MenuItem, SHOW).is_empty(), "the Menu key opens the tile menu");
    for _ in 0..8 {
        if focused_name(&lab.ui.ctx) == SHOW {
            break;
        }
        lab.press(Key::ArrowDown, Modifiers::NONE);
        lab.settle();
    }
    assert_eq!(focused_name(&lab.ui.ctx), SHOW, "the arrows reach the show command");
    lab.press(Key::Enter, Modifiers::NONE);
    lab.settle();
    assert_eq!(lab.ui.app.session.page().unwrap(), page, "the import page is current");
    assert!(lab.ran_since(mark, "page.select").iter().any(|p| p["id"] == json!(page.0)));
}

// covers: E4-4.5
#[test]
fn the_search_row_button_opens_the_import_window_for_any_source_and_a_section_button_for_its_package() {
    let mut lab = Lab::with_imports("window", &[source(PACKAGE, VERSION)], &["Button"]);
    lab.show();
    let buttons = lab.centres_of(Role::Button, IMPORT_BUTTON);
    assert_eq!(buttons.len(), 2, "one in the search row, one in the section: {buttons:?}");
    assert!(buttons.iter().all(|b| b.x > RAIL_WIDTH && b.x < RAIL_WIDTH + lab.ui.app.ui.left_width), "both in the Components tab");

    let mark = lab.mark();
    lab.click(buttons[0]);
    let calls = lab.ran_since(mark, "uigen.importWindow");
    assert_eq!(calls.len(), 1, "the search row runs uigen.importWindow: {calls:?}");
    assert!(calls[0].get("package").is_none_or(Value::is_null), "without a package: {:?}", calls[0]);

    lab.ui.app.ui.import_window = None;
    lab.show();
    let mark = lab.mark();
    let buttons = lab.centres_of(Role::Button, IMPORT_BUTTON);
    lab.click(buttons[1]);
    let calls = lab.ran_since(mark, "uigen.importWindow");
    assert_eq!(calls.len(), 1, "the section runs it too: {calls:?}");
    assert_eq!(calls[0]["package"], json!(PACKAGE), "with its package");
}

// covers: E4-4.5
#[test]
fn an_empty_section_has_its_own_import_button_with_its_package() {
    let mut lab = Lab::new("window-empty", &[source(PACKAGE, VERSION), source(OTHER, OTHER_VERSION)]);
    lab.show();
    let buttons = lab.centres_of(Role::Button, IMPORT_BUTTON);
    assert_eq!(buttons.len(), 3, "the search row and one per section: {buttons:?}");

    let mark = lab.mark();
    lab.click(buttons[2]);

    let calls = lab.ran_since(mark, "uigen.importWindow");
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!(calls[0]["package"], json!(OTHER), "the second section opens the window on its package");
}
