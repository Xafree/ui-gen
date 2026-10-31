//! The left panel of a tab of a library project, driven through egui headless: the rail without its
//! Components entry, the `Library` dropdown (families, counts, empty states, folds per tab), rows
//! that drag onto the canvas, and a tab outside a project that keeps its rail and its panel.

use egui::accesskit::{Node, Role};
use egui::epaint::ClippedShape;
use egui::{Event, Modifiers, PointerButton, Pos2, Shape, pos2};
use figcraft_doc::NodeKind;
use figcraft_engine::Session;
use serde_json::json;

use crate::LeftTab;
use crate::assets_panel::library_tab::{LibraryPanel, family_label};
use crate::layers_drag_tests::Ui;
use crate::source_family::{SourceFacts, group_sources};

fn project_ui(tag: &str, sources: serde_json::Value) -> (Ui, std::path::PathBuf) {
    figcraft_text::scan_system_fonts();
    let dir = std::env::temp_dir().join(format!("uigen-library-panel-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest =
        json!({"version": 1, "name": "test lib", "designFiles": ["lib.figcraft"], "tagPrefix": "ui-", "litOutDir": "lit", "sources": sources});
    std::fs::write(dir.join("uigen.json"), manifest.to_string()).unwrap();
    let mut doc = figcraft_doc::Document::new("lib");
    let page = doc.pages[0];
    doc.node_mut(page).unwrap().name = "Card".to_string();
    std::fs::write(dir.join("lib.figcraft"), figcraft_format::save_bytes(&doc, None).unwrap()).unwrap();
    let mut ui = Ui::new(Session::empty());
    ui.app.session.prefs.language = "en".into();
    assert!(ui.app.open_project_at(dir.clone(), false), "the project opens");
    ui.app.ui.home = false;
    ui.app.ui.left_collapsed = false;
    settle(&mut ui);
    (ui, dir)
}

fn settle(ui: &mut Ui) {
    for _ in 0..3 {
        ui.frame(vec![]);
    }
}

fn heroui() -> serde_json::Value {
    json!([
        {"package": "@heroui/styles", "version": "3.2.7", "framework": "unknown", "license": "MIT", "removed": false},
        {"package": "@heroui/react", "version": "3.2.7", "framework": "react", "license": "MIT", "removed": false},
    ])
}

fn tree(u: &mut Ui) -> Vec<Node> {
    u.ctx.enable_accesskit();
    u.frame(vec![]);
    let out = u.frame_output(vec![]);
    out.platform_output.accesskit_update.expect("an AccessKit update").nodes.into_iter().map(|(_, n)| n).collect()
}

fn texts(u: &mut Ui) -> Vec<(String, Pos2)> {
    fn walk(shape: &Shape, out: &mut Vec<(String, Pos2)>) {
        match shape {
            Shape::Text(t) => out.push((t.galley.text().to_string(), t.pos)),
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let shapes: Vec<ClippedShape> = u.frame_output(vec![]).shapes;
    let mut out = Vec::new();
    shapes.iter().for_each(|c| walk(&c.shape, &mut out));
    out
}

fn y_of(list: &[(String, Pos2)], text: &str) -> f32 {
    list.iter().find(|(s, _)| s == text).unwrap_or_else(|| panic!("{text:?} is not painted: {list:?}")).1.y
}

fn has(list: &[(String, Pos2)], text: &str) -> bool {
    list.iter().any(|(s, _)| s == text)
}

fn centre(nodes: &[Node], role: Role, label: &str) -> Pos2 {
    let n = nodes.iter().find(|n| n.role() == role && n.label() == Some(label)).unwrap_or_else(|| panic!("no {role:?} named {label:?}"));
    let b = n.bounds().expect("bounds");
    pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
}

fn click(u: &mut Ui, at: Pos2) {
    u.frame(vec![Event::PointerMoved(at)]);
    u.frame(vec![Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }]);
    u.frame(vec![Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }]);
    settle(u);
}

fn tabs(nodes: &[Node]) -> Vec<String> {
    nodes.iter().filter(|n| n.role() == Role::Tab).filter_map(|n| n.label().map(str::to_string)).collect()
}

#[test]
fn a_library_tab_stacks_header_library_pages_and_layers_with_counts_even_at_zero() {
    let (mut u, _dir) = project_ui("stack", heroui());
    let list = texts(&mut u);
    let order = ["test lib", "Library", "Local components", "Local styles", "heroui 3.2.7", "Pages", "Card", "Layers"];
    let ys: Vec<f32> = order.iter().map(|text| y_of(&list, text)).collect();
    assert!(ys.windows(2).all(|w| w[0] < w[1]), "stacked top to bottom: {ys:?}");
    assert!(list.iter().filter(|(s, _)| s == "0").count() >= 3, "local components, local styles and the family say 0: {list:?}");
}

#[test]
fn a_styles_only_package_never_gets_its_own_row() {
    let sources = json!([
        {"package": "@acme/theme", "version": "1.0.0", "framework": "unknown", "license": "MIT", "removed": false},
        {"package": "@heroui/styles", "version": "3.2.7", "framework": "unknown", "license": "MIT", "removed": false},
        {"package": "@heroui/react", "version": "3.2.7", "framework": "react", "license": "MIT", "removed": false},
    ]);
    let (mut u, _dir) = project_ui("styles-only", sources);
    let list = texts(&mut u);
    assert!(has(&list, "heroui 3.2.7"), "{list:?}");
    assert!(!list.iter().any(|(s, _)| s.contains("@acme/theme") || s.contains("@heroui/styles")), "{list:?}");
}

#[test]
fn a_family_of_one_package_also_says_its_version() {
    let sources = [SourceFacts { package: "daisy", version: "4.0.1", framework: "react", components: 3, depends_on: &[] }];
    assert_eq!(family_label(&group_sources(&sources)[0]), "daisy 4.0.1");
}

#[test]
fn the_rail_has_no_components_entry_in_a_library_tab_only() {
    let (mut u, _dir) = project_ui("rail", heroui());
    let nodes = tree(&mut u);
    let names = tabs(&nodes);
    assert!(names.iter().any(|n| n == "File"), "{names:?}");
    assert!(!names.iter().any(|n| n == "Components"), "{names:?}");
    let mut plain = Ui::new(Session::new());
    plain.app.ui.left_collapsed = false;
    plain.app.session.prefs.language = "en".into();
    settle(&mut plain);
    let names = tabs(&tree(&mut plain));
    assert!(names.iter().any(|n| n == "File") && names.iter().any(|n| n == "Components"), "{names:?}");
}

#[test]
fn a_normal_tab_keeps_its_components_panel_and_never_shows_the_dropdown() {
    figcraft_text::scan_system_fonts();
    let mut u = Ui::new(Session::new());
    u.app.session.prefs.language = "en".into();
    u.app.ui.left_collapsed = false;
    u.app.ui.left_tab = LeftTab::Assets;
    settle(&mut u);
    let list = texts(&mut u);
    assert!(has(&list, "Components") && has(&list, "Local components"), "{list:?}");
    assert!(!has(&list, "Library") && !has(&list, "Pages"), "{list:?}");
    assert_eq!(u.app.left_tab_shown(), LeftTab::Assets);
    u.app.ui.left_tab = LeftTab::File;
    settle(&mut u);
    let list = texts(&mut u);
    assert!(has(&list, "Pages") && has(&list, "Layers") && !has(&list, "Library"), "{list:?}");
}

#[test]
fn view_assets_in_a_library_tab_opens_the_panel_and_the_dropdown() {
    let (mut u, _dir) = project_ui("reveal", heroui());
    let uid = u.app.session.ds().unwrap().uid;
    u.app.ui.library_panel.folds.entry(uid).or_default().insert("library".into(), false);
    u.app.ui.left_collapsed = true;
    settle(&mut u);
    u.app.run("view.assets", json!({}));
    settle(&mut u);
    assert!(!u.app.ui.left_collapsed, "the panel opens");
    assert!(has(&texts(&mut u), "Local components"), "the dropdown opens");
    assert_eq!(u.app.left_tab_shown(), LeftTab::File, "and the rail stays on File");
}

#[test]
fn a_fold_is_kept_per_tab_and_only_local_styles_start_closed() {
    let mut panel = LibraryPanel::default();
    assert!(panel.is_open(1, "local") && panel.is_open(1, "family/heroui"), "open by default");
    assert!(!panel.is_open(1, "styles"), "the local styles start closed");
    panel.folds.entry(1).or_default().insert("local".into(), false);
    assert!(!panel.is_open(1, "local"), "kept in the tab");
    assert!(panel.is_open(2, "local"), "another tab has its own");
}

#[test]
fn clicking_a_row_folds_it_and_its_empty_state_follows() {
    let (mut u, _dir) = project_ui("fold", heroui());
    let empty = "Nothing imported from this source yet.";
    assert!(has(&texts(&mut u), empty), "the family says it is empty");
    let at = centre(&tree(&mut u), Role::Button, "heroui 3.2.7");
    click(&mut u, at);
    assert!(!has(&texts(&mut u), empty), "folded");
    assert!(!has(&texts(&mut u), "No styles yet."), "the local styles start closed");
    let at = centre(&tree(&mut u), Role::Button, "Local styles");
    click(&mut u, at);
    assert!(has(&texts(&mut u), "No styles yet."), "their empty state shows once open");
}

#[test]
fn the_plus_of_the_library_title_opens_the_source_libraries_page() {
    let (mut u, _dir) = project_ui("plus", heroui());
    let at = centre(&tree(&mut u), Role::Button, "Add a library");
    click(&mut u, at);
    assert!(u.app.ui.project_settings.is_some(), "the source libraries page opens");
}

#[test]
fn dragging_a_local_component_row_to_the_canvas_creates_an_instance() {
    let (mut u, _dir) = project_ui("drag", heroui());
    let made = u.app.session.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 40, "height": 40})).unwrap();
    u.app.session.execute("select.set", &json!({"ids": [made["id"]]})).unwrap();
    u.app.session.execute("component.create", &json!({})).unwrap();
    u.app.session.execute("object.rename", &json!({"name": "Chip"})).unwrap();
    settle(&mut u);
    let instances = |u: &Ui| u.app.session.doc().unwrap().nodes().filter(|n| matches!(n.kind, NodeKind::Instance { .. })).count();
    assert_eq!(instances(&u), 0);
    let from = centre(&tree(&mut u), Role::ListBoxOption, "Chip");
    let canvas = u.app.canvas.rect.center();
    u.frame(vec![Event::PointerMoved(from)]);
    u.frame(vec![Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }]);
    u.frame(vec![Event::PointerMoved(from + egui::vec2(30.0, 0.0))]);
    u.frame(vec![Event::PointerMoved(canvas)]);
    u.frame(vec![Event::PointerButton { pos: canvas, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }]);
    settle(&mut u);
    assert_eq!(instances(&u), 1, "the drop made an instance");
}

#[test]
fn the_selected_variants_contents_show_in_layers_as_outside_a_library() {
    let (mut u, _dir) = project_ui("reveal-layer", heroui());
    let frame = u.app.session.execute("node.create", &json!({"type": "frame", "x": 0, "y": 0, "width": 100, "height": 100})).unwrap();
    let inner = u
        .app
        .session
        .execute("node.create", &json!({"type": "rectangle", "parent": frame["id"], "x": 5, "y": 5, "width": 40, "height": 40}))
        .unwrap();
    u.app.session.execute("select.set", &json!({"ids": [inner["id"]]})).unwrap();
    u.app.session.execute("object.rename", &json!({"name": "Inside"})).unwrap();
    settle(&mut u);
    assert!(has(&texts(&mut u), "Inside"), "a selection on the canvas reveals its layer");
}
