//! Review fixes of the Lit export window: renaming an invalid component in place, replanning once
//! an edit has settled, a dependency shown as still exported, the full recap after an export, the
//! files left alone, the notes that outlive a toggle and the `force` switch.

use std::path::PathBuf;

use egui::accesskit::{Node, Role};
use egui::{Event, Key, Modifiers, Pos2, pos2};
use figcraft_doc::NodeId;
use figcraft_engine::Session;
use serde_json::{Value, json};

use crate::dialogs::lit_export::{self, ComponentRow, LitExport};
use crate::import_report_tests::shown_texts;
use crate::layers_drag_tests::Ui;
use crate::{DialogFuture, FigApp, Services};

const EXPORT_BUTTON: &str = "Export package";
const BUTTON_FILE: &str = "components/ui-button.ts";

fn call(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd}: {e}"))
}

fn id_of(v: &Value) -> NodeId {
    NodeId(v["id"].as_u64().unwrap_or_else(|| panic!("no id in {v}")))
}

fn component(s: &mut Session, name: &str, x: f64) -> NodeId {
    let frame = id_of(&call(s, "frame.create", json!({"x": x, "y": 0, "width": 100, "height": 40, "name": name})));
    call(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 40, "parent": frame, "props": {"name": "Bg", "fill": "#ff0000"}}));
    call(s, "text.create", json!({"text": "Click", "x": 10, "y": 10, "parent": frame, "props": {"name": "Label"}}));
    id_of(&call(s, "component.create", json!({"ids": [frame]})))
}

fn button_and_card() -> (Session, NodeId, NodeId) {
    let mut s = Session::new();
    let button = component(&mut s, "Button", 0.0);
    let card = component(&mut s, "Card", 200.0);
    (s, button, card)
}

fn frames(u: &mut Ui, count: usize) {
    for _ in 0..count {
        u.frame(vec![]);
    }
}

fn open(u: &mut Ui) {
    assert!(u.app.run("export.litWindow", json!({})).is_some());
    frames(u, 3);
}

fn state(u: &Ui) -> &LitExport {
    u.app.ui.lit_export.as_ref().expect("the Lit export window is open")
}

fn row(u: &Ui, id: NodeId) -> &ComponentRow {
    state(u).components().iter().find(|r| r.id == id).unwrap_or_else(|| panic!("no row for {id}"))
}

fn rename(u: &mut Ui, id: NodeId, name: &str) {
    assert!(u.app.run("node.set", json!({"id": id, "props": {"name": name}})).is_some());
}

fn painted(u: &mut Ui) -> Vec<String> {
    u.frame(vec![]);
    shown_texts(u.frame_output(vec![]).shapes)
}

fn on_screen(shown: &[String], text: &str) -> bool {
    shown.iter().any(|t| t.contains(text))
}

fn toast(app: &FigApp) -> String {
    app.ui.toast.as_ref().map(|t| t.0.clone()).unwrap_or_default()
}

fn nodes(u: &mut Ui) -> Vec<Node> {
    u.ctx.enable_accesskit();
    u.frame(vec![]);
    u.frame_output(vec![]).platform_output.accesskit_update.expect("an AccessKit update").nodes.into_iter().map(|(_, n)| n).collect()
}

fn centre(n: &Node) -> Pos2 {
    let b = n.bounds().expect("the control has bounds");
    pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
}

fn centre_of(u: &mut Ui, label: &str) -> Pos2 {
    let all = nodes(u);
    centre(all.iter().find(|n| n.label().is_some_and(|l| l.trim() == label)).unwrap_or_else(|| panic!("no {label:?} on screen")))
}

fn click(u: &mut Ui, at: Pos2) {
    u.button(at, true);
    u.button(at, false);
}

fn click_export(u: &mut Ui) {
    let at = centre_of(u, EXPORT_BUTTON);
    click(u, at);
    frames(u, 5);
}

fn press_enter(u: &mut Ui) {
    u.frame(vec![
        Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE },
        Event::Key { key: Key::Enter, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE },
    ]);
}

fn rename_fields(u: &mut Ui) -> Vec<Pos2> {
    let mut found: Vec<Pos2> = nodes(u).iter().filter(|n| n.role() == Role::TextInput && n.label() == Some("Rename")).map(centre).collect();
    found.sort_by(|a, b| a.y.total_cmp(&b.y));
    found
}

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-lit-review-ui-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn put(&self, rel: &str, text: &str) {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.0.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct PicksFolder(PathBuf);

impl Services for PicksFolder {
    fn pick_folder(&mut self, _title: &str) -> DialogFuture {
        Box::pin(std::future::ready(Some(self.0.clone())))
    }
}

fn twins() -> (Session, NodeId, NodeId) {
    let mut s = Session::new();
    let first = component(&mut s, "Primary Button", 0.0);
    let twin = component(&mut s, "Primary Button", 200.0);
    (s, first, twin)
}

#[test]
fn an_invalid_row_has_a_rename_field_that_renames_the_component_and_can_be_undone() {
    let (s, first, twin) = twins();
    let mut u = Ui::new(s);
    open(&mut u);
    assert_eq!(row(&u, twin).status, "invalid");
    let fields = rename_fields(&mut u);
    assert_eq!(fields.len(), 2, "one rename field per invalid row: {fields:?}");

    click(&mut u, fields[1]);
    u.frame(vec![Event::Text("Secondary Button".into())]);
    press_enter(&mut u);
    frames(&mut u, 3);
    assert_eq!(u.app.session.doc().unwrap().node(twin).unwrap().name, "Secondary Button");
    assert_eq!(u.app.session.doc().unwrap().node(first).unwrap().name, "Primary Button");
    assert!(state(&u).components().iter().all(|r| r.status == "ok" && r.issues.is_empty()));
    assert!(state(&u).can_export());
    assert!(rename_fields(&mut u).is_empty(), "a valid row has no rename field");

    assert!(u.app.run("edit.undo", json!({})).is_some());
    frames(&mut u, 3);
    assert_eq!(u.app.session.doc().unwrap().node(twin).unwrap().name, "Primary Button");
    assert_eq!(row(&u, twin).status, "invalid");
    assert!(!state(&u).can_export());
}

#[test]
fn a_blank_new_name_changes_nothing() {
    let (s, _, twin) = twins();
    let mut u = Ui::new(s);
    open(&mut u);
    let fields = rename_fields(&mut u);
    click(&mut u, fields[1]);
    u.frame(vec![Event::Text("   ".into())]);
    press_enter(&mut u);
    frames(&mut u, 3);
    assert_eq!(u.app.session.doc().unwrap().node(twin).unwrap().name, "Primary Button");
    assert_eq!(row(&u, twin).status, "invalid");
}

#[test]
fn the_first_edit_replans_at_once_and_a_burst_waits_until_it_settles() {
    let (s, button, card) = button_and_card();
    let mut u = Ui::new(s);
    open(&mut u);
    rename(&mut u, button, "Primary Button");
    frames(&mut u, 3);
    assert_eq!(row(&u, button).tag, "ui-primary-button", "a lone edit shows up within a few frames");

    rename(&mut u, card, "Secondary Card");
    frames(&mut u, 3);
    assert_eq!(row(&u, card).tag, "ui-card", "an edit right after another one waits");
    rename(&mut u, card, "Tertiary Card");
    frames(&mut u, 12);
    assert_eq!(row(&u, card).tag, "ui-card", "every edit of the burst postpones the plan");
    frames(&mut u, 30);
    assert_eq!(row(&u, card).tag, "ui-tertiary-card");
}

#[test]
fn nothing_is_planned_while_the_pointer_is_down() {
    let (s, button, _) = button_and_card();
    let mut u = Ui::new(s);
    open(&mut u);
    let at = pos2(700.0, 880.0);
    u.button(at, true);
    rename(&mut u, button, "Primary Button");
    frames(&mut u, 60);
    assert_eq!(row(&u, button).tag, "ui-button", "a drag in progress is not planned");
    u.button(at, false);
    frames(&mut u, 3);
    assert_eq!(row(&u, button).tag, "ui-primary-button", "the edit is planned once the pointer is up");
}

#[test]
fn the_rows_and_files_with_nothing_unchecked_are_those_of_the_explicit_selection() {
    let (s, _, _) = button_and_card();
    let mut u = Ui::new(s);
    open(&mut u);
    let ids = state(&u).ids();
    let plan = call(&mut u.app.session, "export.lit", json!({"ids": ids}));
    let paths: Vec<&str> = plan["files"].as_array().unwrap().iter().filter_map(|f| f["path"].as_str()).collect();
    assert_eq!(state(&u).files(), paths);
}

#[test]
fn an_unchecked_dependency_says_it_is_still_exported() {
    let mut s = Session::new();
    let icon = component(&mut s, "Heart", 0.0);
    let frame = id_of(&call(&mut s, "frame.create", json!({"x": 400, "y": 0, "width": 120, "height": 40, "name": "Chip"})));
    call(&mut s, "instance.create", json!({"component": icon, "x": 8, "y": 8, "parent": frame}));
    call(&mut s, "component.create", json!({"ids": [frame]}));
    let mut u = Ui::new(s);
    open(&mut u);
    assert!(!on_screen(&painted(&mut u), "Still exported"));
    lit_export::set_checked(&mut u.app, icon, false);
    frames(&mut u, 3);
    assert_eq!(row(&u, icon).status, "dependency");
    assert!(state(&u).files().contains(&"components/ui-heart.ts"));
    let shown = painted(&mut u);
    assert!(on_screen(&shown, "Still exported: another component needs it"), "{shown:?}");
}

#[test]
fn the_recap_counts_stale_and_kept_files_and_the_window_lists_the_skipped_ones() {
    let (s, _, card) = button_and_card();
    let target = Folder::new("recap");
    target.put("mine.txt", "mine\n");
    let mut u = Ui::new(s);
    u.app.services = Box::new(PicksFolder(target.0.clone()));
    open(&mut u);
    click_export(&mut u);
    let first = toast(&u.app);
    assert!(first.contains("1 kept"), "{first:?}");
    assert!(!first.contains("stale"), "{first:?}");

    target.put(BUTTON_FILE, &format!("{}// by hand\n", target.read(BUTTON_FILE)));
    lit_export::set_checked(&mut u.app, card, false);
    frames(&mut u, 3);
    click_export(&mut u);
    let second = toast(&u.app);
    assert!(second.contains("1 stale") && second.contains("1 kept") && second.contains("1 skipped"), "{second:?}");
    let shown = painted(&mut u);
    assert!(on_screen(&shown, BUTTON_FILE), "the skipped file is listed: {shown:?}");
    assert!(on_screen(&shown, "Edited since the last export"), "{shown:?}");
}

#[test]
fn the_notes_of_an_export_stay_when_a_component_is_checked_again() {
    let (s, _, card) = button_and_card();
    let target = Folder::new("notes");
    target.put(".uigen/export.json", "not json");
    let mut u = Ui::new(s);
    u.app.services = Box::new(PicksFolder(target.0.clone()));
    open(&mut u);
    click_export(&mut u);
    let has_note = |u: &Ui| state(u).notes().iter().any(|n| n.contains("export.json was ignored"));
    assert!(has_note(&u), "{:?}", state(&u).notes());
    lit_export::set_checked(&mut u.app, card, false);
    frames(&mut u, 3);
    assert!(has_note(&u), "the first toggle keeps the export notes: {:?}", state(&u).notes());
    assert!(on_screen(&painted(&mut u), "export.json was ignored"));
}

#[test]
fn the_overwrite_switch_replaces_files_edited_by_hand() {
    let (s, _, _) = button_and_card();
    let target = Folder::new("force");
    let mut u = Ui::new(s);
    u.app.services = Box::new(PicksFolder(target.0.clone()));
    open(&mut u);
    click_export(&mut u);
    let generated = target.read(BUTTON_FILE);
    target.put(BUTTON_FILE, &format!("{generated}// by hand\n"));
    click_export(&mut u);
    assert!(target.read(BUTTON_FILE).ends_with("// by hand\n"), "an edited file is kept without the switch");
    assert!(toast(&u.app).contains("1 skipped"), "{:?}", toast(&u.app));

    let at = centre_of(&mut u, "Overwrite modified files");
    click(&mut u, at);
    frames(&mut u, 3);
    click_export(&mut u);
    assert_eq!(target.read(BUTTON_FILE), generated, "the switch replaces the edited file");
    assert!(toast(&u.app).contains("0 skipped"), "{:?}", toast(&u.app));
}
