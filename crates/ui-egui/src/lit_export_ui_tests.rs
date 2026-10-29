//! File ▸ Export library as Lit…: the window that plans the package with `export.lit`, lists the
//! components with their tag, lets the user check which ones to export, previews the files and
//! writes them to a folder. Programmatic calls never open it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use egui::accesskit::Node;
use egui::{Pos2, pos2};
use figcraft_doc::NodeId;
use figcraft_engine::Session;
use figcraft_engine::catalog::MENU;
use figcraft_engine::keymap::KEYS;
use serde_json::{Value, json};

use crate::dialogs::lit_export::{self, ComponentRow, LitExport};
use crate::import_report_tests::shown_texts;
use crate::layers_drag_tests::Ui;
use crate::{DialogFuture, FigApp, Services};

const EXPORT_BUTTON: &str = "Export package";

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

fn hollow_component(s: &mut Session, name: &str, x: f64) -> NodeId {
    let frame = id_of(&call(s, "frame.create", json!({"x": x, "y": 0, "width": 100, "height": 40, "name": name})));
    id_of(&call(s, "component.create", json!({"ids": [frame]})))
}

fn brand_color(s: &mut Session) {
    call(s, "variable.createCollection", json!({"name": "Colors", "modes": ["Light", "Dark"]}));
    call(
        s,
        "variable.create",
        json!({"collection": "Colors", "name": "Colors/Brand 500", "type": "color", "values": {"Light": "#3366FF", "Dark": "#6699FF"}}),
    );
}

/// A Button, a Card and an empty Ghost component.
fn library() -> (Session, NodeId, NodeId, NodeId) {
    let mut s = Session::new();
    let button = component(&mut s, "Button", 0.0);
    let card = component(&mut s, "Card", 200.0);
    let ghost = hollow_component(&mut s, "Ghost", 400.0);
    (s, button, card, ghost)
}

fn settle(u: &mut Ui) {
    for _ in 0..3 {
        u.frame(vec![]);
    }
}

fn open(u: &mut Ui) {
    assert!(u.app.run("export.litWindow", json!({})).is_some());
    settle(u);
}

fn state(u: &Ui) -> &LitExport {
    u.app.ui.lit_export.as_ref().expect("the Lit export window is open")
}

fn row(u: &Ui, id: NodeId) -> &ComponentRow {
    state(u).components().iter().find(|r| r.id == id).unwrap_or_else(|| panic!("no row for {id}"))
}

fn painted(u: &mut Ui) -> Vec<String> {
    u.frame(vec![]);
    shown_texts(u.frame_output(vec![]).shapes)
}

fn planned(u: &mut Ui, params: Value) -> Value {
    call(&mut u.app.session, "export.lit", params)
}

fn toast(app: &FigApp) -> String {
    app.ui.toast.as_ref().map(|t| t.0.clone()).unwrap_or_default()
}

fn nodes(u: &mut Ui) -> Vec<Node> {
    u.ctx.enable_accesskit();
    u.frame(vec![]);
    u.frame_output(vec![]).platform_output.accesskit_update.expect("an AccessKit update").nodes.into_iter().map(|(_, n)| n).collect()
}

fn centre_of(u: &mut Ui, label: &str) -> Pos2 {
    let all = nodes(u);
    let n = all.iter().find(|n| n.label().is_some_and(|l| l.trim() == label)).unwrap_or_else(|| panic!("no {label:?} on screen"));
    let b = n.bounds().unwrap_or_else(|| panic!("{label:?} has no bounds"));
    pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
}

fn click_export(u: &mut Ui) {
    let at = centre_of(u, EXPORT_BUTTON);
    u.button(at, true);
    u.button(at, false);
    for _ in 0..5 {
        u.frame(vec![]);
    }
}

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-lit-ui-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn files(&self) -> Vec<String> {
        let mut found = Vec::new();
        collect(&self.0, &self.0, &mut found);
        found.sort();
        found
    }

    fn has(&self, rel: &str) -> bool {
        self.files().iter().any(|f| f == rel)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn collect(root: &Path, at: &Path, found: &mut Vec<String>) {
    for entry in std::fs::read_dir(at).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, found);
        } else {
            found.push(path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"));
        }
    }
}

struct PicksFolder(PathBuf);

impl Services for PicksFolder {
    fn pick_folder(&mut self, _title: &str) -> DialogFuture {
        Box::pin(std::future::ready(Some(self.0.clone())))
    }
}

#[test]
fn the_file_menu_offers_the_lit_export_with_a_shortcut_that_leaves_file_export_its_own() {
    // covers: E6-8.1
    let item = MENU.iter().find(|m| m.id == "export.litWindow").expect("a menu item runs export.litWindow");
    assert_eq!((item.menu, item.label), ("File", "Export library as Lit…"));
    assert!(crate::menus::available("export.litWindow"));
    let lit = KEYS.iter().find(|b| b.id == "export.litWindow").expect("export.litWindow has a shortcut");
    let export = KEYS.iter().find(|b| b.id == "file.export").expect("file.export keeps its shortcut");
    assert_eq!(export.mac, &["Shift+Cmd+E"]);
    assert!(!lit.mac.is_empty());
    assert!(lit.mac.iter().all(|k| !export.mac.contains(k)), "{:?} collides with file.export", lit.mac);
}

#[test]
fn opening_the_window_lists_every_component_with_its_tag_checked_and_the_empty_one_unchecked() {
    // covers: E6-8.2
    let (s, button, card, ghost) = library();
    let mut u = Ui::new(s);
    assert!(u.app.ui.lit_export.is_none());
    open(&mut u);
    assert_eq!(state(&u).components().len(), 3);
    let (b, c, g) = (row(&u, button), row(&u, card), row(&u, ghost));
    assert_eq!((b.tag.as_str(), b.status.as_str(), b.checked), ("ui-button", "ok", true));
    assert_eq!((c.tag.as_str(), c.status.as_str(), c.checked), ("ui-card", "ok", true));
    assert_eq!((g.status.as_str(), g.checked), ("empty", false));
    assert_eq!(state(&u).ids(), vec![button, card]);
    assert!(state(&u).files().contains(&"components/ui-button.ts"));
    assert!(state(&u).can_export());
    let shown = painted(&mut u);
    for tag in ["ui-button", "ui-card"] {
        assert!(shown.iter().any(|t| t.contains(tag)), "{tag} is on screen: {shown:?}");
    }
}

#[test]
fn an_invalid_name_is_flagged_and_blocks_export_until_the_component_is_renamed() {
    // covers: E6-8.3
    let mut s = Session::new();
    component(&mut s, "Primary Button", 0.0);
    let twin = component(&mut s, "Primary Button", 200.0);
    let target = Folder::new("invalid");
    let mut u = Ui::new(s);
    u.app.services = Box::new(PicksFolder(target.0.clone()));
    open(&mut u);
    assert!(state(&u).components().iter().any(|r| r.status == "invalid" && r.issues.iter().any(|i| i == "duplicate")));
    assert!(!state(&u).can_export());
    let plan = planned(&mut u, json!({}));
    assert!(plan["components"].as_array().unwrap().iter().any(|c| !c["issues"].as_array().unwrap().is_empty()));
    let shown = painted(&mut u);
    for message in plan["components"].as_array().unwrap().iter().flat_map(|c| c["issues"].as_array().unwrap()).filter_map(|i| i["message"].as_str()) {
        assert!(shown.iter().any(|t| t.contains(message)), "the issue {message:?} is on screen: {shown:?}");
    }
    click_export(&mut u);
    assert!(target.files().is_empty(), "the inactive button writes nothing: {:?}", target.files());

    assert!(u.app.run("node.set", json!({"id": twin, "props": {"name": "Secondary Button"}})).is_some());
    settle(&mut u);
    assert!(state(&u).components().iter().all(|r| r.status == "ok" && r.issues.is_empty() && r.checked));
    assert!(state(&u).can_export());
    click_export(&mut u);
    assert!(target.has("components/ui-primary-button.ts") && target.has("components/ui-secondary-button.ts"), "{:?}", target.files());
}

#[test]
fn unchecking_a_component_drops_it_from_the_ids_and_the_listed_files() {
    // covers: E6-8.4
    let (s, button, card, _) = library();
    let mut u = Ui::new(s);
    open(&mut u);
    assert!(state(&u).files().contains(&"components/ui-card.ts"));
    lit_export::set_checked(&mut u.app, card, false);
    settle(&mut u);
    assert_eq!(state(&u).ids(), vec![button]);
    assert!(!row(&u, card).checked && row(&u, button).checked, "the unchecked row stays listed");
    let files = state(&u).files();
    assert!(files.contains(&"components/ui-button.ts") && !files.contains(&"components/ui-card.ts"), "{files:?}");
    lit_export::set_checked(&mut u.app, card, true);
    assert_eq!(state(&u).ids(), vec![button, card]);
    assert!(state(&u).files().contains(&"components/ui-card.ts"));
}

#[test]
fn selecting_a_file_previews_its_text() {
    // covers: E6-8.5
    let mut s = Session::new();
    brand_color(&mut s);
    component(&mut s, "Button", 0.0);
    let mut u = Ui::new(s);
    open(&mut u);
    let plan = planned(&mut u, json!({}));
    let tokens = plan["files"].as_array().unwrap().iter().find(|f| f["path"] == "tokens/tokens.css").expect("tokens.css is planned");
    let text = tokens["text"].as_str().unwrap().to_string();
    let line = text.lines().find(|l| l.contains("brand-500")).expect("the brand colour is a custom property").trim().to_string();
    u.app.ui.lit_export.as_mut().expect("the window is open").select("tokens/tokens.css");
    settle(&mut u);
    assert_eq!(state(&u).selected(), Some("tokens/tokens.css"));
    assert_eq!(state(&u).preview(), Some(text.as_str()));
    let shown = painted(&mut u);
    assert!(shown.iter().any(|t| t.contains(&line)), "the preview shows {line:?}: {shown:?}");
}

#[test]
fn every_note_of_the_plan_is_shown() {
    // covers: E6-8.5
    let mut s = Session::new();
    let icon = component(&mut s, "Heart", 0.0);
    let frame = id_of(&call(&mut s, "frame.create", json!({"x": 400, "y": 0, "width": 120, "height": 40, "name": "Chip"})));
    call(&mut s, "instance.create", json!({"component": icon, "x": 8, "y": 8, "parent": frame}));
    let chip = id_of(&call(&mut s, "component.create", json!({"ids": [frame]})));
    let mut u = Ui::new(s);
    open(&mut u);
    lit_export::set_checked(&mut u.app, icon, false);
    settle(&mut u);
    let plan = planned(&mut u, json!({"ids": [chip]}));
    let notes: Vec<String> = plan["notes"].as_array().unwrap().iter().filter_map(|n| n.as_str().map(str::to_string)).collect();
    assert!(notes.iter().any(|n| n.to_lowercase().contains("heart")), "the dependency is noted: {notes:?}");
    let shown = painted(&mut u);
    for note in &notes {
        assert!(state(&u).notes().contains(&note.as_str()), "{note:?} is in the window state");
        assert!(shown.iter().any(|t| t.contains(note.as_str())), "{note:?} is on screen: {shown:?}");
    }
}

#[test]
fn export_picks_a_folder_writes_the_checked_components_and_toasts_the_counters() {
    // covers: E6-8.6
    let (s, _, card, _) = library();
    let target = Folder::new("write");
    let mut u = Ui::new(s);
    u.app.services = Box::new(PicksFolder(target.0.clone()));
    open(&mut u);
    lit_export::set_checked(&mut u.app, card, false);
    settle(&mut u);
    click_export(&mut u);
    let written = target.files();
    assert!(target.has("components/ui-button.ts") && target.has("index.ts") && target.has("package.json"), "{written:?}");
    assert!(!target.has("components/ui-card.ts"), "the unchecked component is not written: {written:?}");
    let first = toast(&u.app);
    assert!(first.contains(&format!("{} written", written.len())), "{first:?} counts {} written files", written.len());
    assert!(first.contains("0 skipped"), "{first:?}");

    let edited = target.0.join("components/ui-button.ts");
    let mut text = std::fs::read_to_string(&edited).unwrap();
    text.push_str("\n// edited by hand\n");
    std::fs::write(&edited, &text).unwrap();
    click_export(&mut u);
    let second = toast(&u.app);
    assert!(second.contains("1 skipped") && second.contains("unchanged"), "{second:?}");
    assert_eq!(std::fs::read_to_string(&edited).unwrap(), text, "the edited file is kept");
}

#[test]
fn programmatic_calls_open_no_window_and_still_export() {
    // covers: E6-8.7
    let (mut s, button, card, _) = library();
    s.ui_requests.clear();
    let direct = call(&mut s, "export.lit", json!({}));
    assert!(s.ui_requests.is_empty(), "export.lit asks the app for nothing: {:?}", s.ui_requests);
    assert_eq!(direct["ready"], true);

    let mut u = Ui::new(s);
    let ctx = u.ctx.clone();
    let over_control = crate::control::handle(&mut u.app, &ctx, "engine.execute", &json!({"command": "export.lit", "params": {}})).unwrap();
    let tags: BTreeSet<&str> = over_control["components"].as_array().unwrap().iter().filter_map(|c| c["tag"].as_str()).collect();
    assert!(tags.contains("ui-button") && tags.contains("ui-card"), "{over_control}");
    assert_eq!(over_control["ready"], true);
    settle(&mut u);
    assert!(u.app.ui.lit_export.is_none(), "the control channel opened a window");

    assert!(u.app.run("export.lit", json!({"ids": [button, card]})).is_some());
    settle(&mut u);
    assert!(u.app.ui.lit_export.is_none(), "a programmatic export opened a window");
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

#[test]
fn every_language_translates_the_window_strings_and_the_command_labels() {
    // covers: E6-8.8
    let languages = crate::i18n::languages();
    assert!(languages.len() >= 22, "{} languages", languages.len());
    let dialogs = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/dialogs");
    let mut sources = vec![dialogs.join("lit_export.rs")];
    if let Ok(entries) = std::fs::read_dir(dialogs.join("lit_export")) {
        sources.extend(entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "rs")));
    }
    let mut keys = BTreeSet::new();
    for path in &sources {
        keys.extend(keys_in(&std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))));
    }
    assert!(keys.contains(EXPORT_BUTTON), "the window labels its button {EXPORT_BUTTON:?}: {keys:?}");

    let mut missing = Vec::new();
    for lang in languages.iter().filter(|l| l.code != "en") {
        let own = [Some(lang), languages.get(lang.parent)].into_iter().flatten().filter(|l| l.code != "en");
        let own: Vec<_> = own.collect();
        for key in &keys {
            if !own.iter().any(|l| l.ui.iter().any(|(k, _)| k == key)) {
                missing.push(format!("{}: {key:?}", lang.code));
            }
        }
        for id in ["export.lit", "export.litWindow"] {
            if !own.iter().any(|l| l.commands.iter().any(|(k, _)| *k == id)) {
                missing.push(format!("{}: [commands] {id}", lang.code));
            }
        }
    }
    assert!(missing.is_empty(), "untranslated:\n{}", missing.join("\n"));
}
