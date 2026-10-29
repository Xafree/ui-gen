//! The page rule in the UI, driven through egui headless: in a library project the Pages "+" adds a
//! component named in the UI language ("Composant 2" / "Component 2"), outside a project it still
//! adds "Page N"; deleting a component says so and offers Undo, which puts it back where it was
//! with its layers; outside a project the deletion toast is the old plain one; any other change
//! takes the Undo button away; Dev Mode still refuses to add, rename or delete pages; and the new
//! word is in every catalog.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use egui::accesskit::{Node, Role};
use egui::{Pos2, pos2};
use figcraft_doc::NodeId;
use figcraft_engine::Session;
use serde_json::{Value, json};

use crate::i18n::{self, Lang, Text};
use crate::layers_drag_tests::Ui;

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("figcraft-project-pages-ui-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
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

fn id(v: Value) -> NodeId {
    NodeId(v["id"].as_u64().unwrap())
}

fn frames(u: &mut Ui, n: usize) {
    for _ in 0..n {
        u.frame(vec![]);
    }
}

fn project_ui(language: &str, tag: &str) -> (Ui, Dir) {
    figcraft_text::scan_system_fonts();
    let dir = Dir::new(tag);
    let mut u = Ui::new(Session::empty());
    u.app.session.prefs.language = language.into();
    frames(&mut u, 2);
    assert!(u.app.open_project_at(dir.0.clone(), true), "the project opens");
    frames(&mut u, 3);
    assert!(u.app.session.in_lib_project());
    (u, dir)
}

fn plain_ui(language: &str) -> Ui {
    figcraft_text::scan_system_fonts();
    let mut u = Ui::new(Session::new());
    u.app.session.prefs.language = language.into();
    frames(&mut u, 3);
    assert!(!u.app.session.in_lib_project());
    u
}

fn pages(u: &Ui) -> Vec<NodeId> {
    u.app.session.doc().unwrap().pages.clone()
}

fn page_names(u: &Ui) -> Vec<String> {
    let doc = u.app.session.doc().unwrap();
    doc.pages.iter().map(|p| doc.node(*p).unwrap().name.clone()).collect()
}

fn components(u: &mut Ui) -> Vec<(String, u64)> {
    let info = u.app.session.execute("project.info", &json!({})).unwrap();
    info["components"].as_array().unwrap().iter().map(|c| (c["name"].as_str().unwrap().to_owned(), c["layers"].as_u64().unwrap())).collect()
}

fn tree(u: &mut Ui) -> Vec<Node> {
    u.ctx.enable_accesskit();
    u.frame(vec![]);
    u.frame_output(vec![]).platform_output.accesskit_update.expect("an AccessKit update").nodes.into_iter().map(|(_, n)| n).collect()
}

fn button(u: &mut Ui, label: &str) -> Option<Node> {
    let mut found: Vec<Node> = tree(u).into_iter().filter(|n| n.role() == Role::Button && n.label() == Some(label)).collect();
    assert!(found.len() <= 1, "one button named {label:?}, found {}", found.len());
    found.pop()
}

fn centre(n: &Node) -> Pos2 {
    let b = n.bounds().unwrap();
    pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
}

fn click_button(u: &mut Ui, label: &str) {
    let node = button(u, label).unwrap_or_else(|| panic!("a button named {label:?}"));
    let at = centre(&node);
    u.button(at, true);
    u.button(at, false);
    frames(u, 2);
}

fn toast(u: &Ui) -> Option<&str> {
    u.app.ui.toast.as_ref().map(|(message, _)| message.as_str())
}

/// A project of three components: the first one made with it, then `Alpha` (two rectangles) and
/// `Beta`. Returns the ids of all three.
fn three_components(language: &str, tag: &str) -> (Ui, Dir, [NodeId; 3]) {
    let (mut u, dir) = project_ui(language, tag);
    let first = u.app.session.page().unwrap();
    let alpha = id(u.app.session.execute("page.add", &json!({"name": "Alpha"})).unwrap());
    for x in [0, 100] {
        u.app.session.execute("shape.rectangle", &json!({"x": x, "y": 0, "width": 40, "height": 40})).unwrap();
    }
    let beta = id(u.app.session.execute("page.add", &json!({"name": "Beta"})).unwrap());
    u.app.session.execute("page.select", &json!({"id": alpha})).unwrap();
    frames(&mut u, 3);
    (u, dir, [first, alpha, beta])
}

// covers: L1-U1
#[test]
fn in_a_project_the_pages_plus_button_adds_a_component_named_in_the_french_ui() {
    let (mut u, _dir) = project_ui("fr", "add-fr");
    assert_eq!(components(&mut u).iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), ["Composant 1"]);
    click_button(&mut u, i18n::tr("Add new page"));
    let names: Vec<String> = components(&mut u).into_iter().map(|c| c.0).collect();
    assert_eq!(names, ["Composant 1", "Composant 2"]);
}

// covers: L1-U1
#[test]
fn in_a_project_the_pages_plus_button_adds_a_component_named_in_the_english_ui() {
    let (mut u, _dir) = project_ui("en", "add-en");
    click_button(&mut u, i18n::tr("Add new page"));
    let names: Vec<String> = components(&mut u).into_iter().map(|c| c.0).collect();
    assert_eq!(names, ["Component 1", "Component 2"]);
}

// covers: L1-U1
#[test]
fn outside_a_project_the_pages_plus_button_still_adds_page_n_in_any_language() {
    for language in ["en", "fr"] {
        let mut u = plain_ui(language);
        assert_eq!(page_names(&u), ["Page 1"]);
        click_button(&mut u, i18n::tr("Add new page"));
        assert_eq!(page_names(&u), ["Page 1", "Page 2"], "language {language}");
    }
}

// covers: L1-U2
#[test]
fn deleting_a_component_says_so_and_offers_undo_which_restores_it_in_place_with_its_layers() {
    let (mut u, _dir, [first, alpha, beta]) = three_components("en", "undo-en");
    let before = components(&mut u);
    assert_eq!(before, [("Component 1".to_owned(), 0), ("Alpha".to_owned(), 2), ("Beta".to_owned(), 0)]);
    crate::layers::delete_page(&mut u.app, alpha);
    frames(&mut u, 2);
    assert_eq!(pages(&u), [first, beta]);
    assert_eq!(toast(&u), Some("Component deleted"));
    let action = u.app.ui.toast_action.as_ref().expect("the toast carries an action");
    assert_eq!((action.label.as_str(), action.command.as_str()), ("Undo", "edit.undo"));

    click_button(&mut u, "Undo");
    assert_eq!(pages(&u), [first, alpha, beta], "back in the middle, same id");
    assert_eq!(components(&mut u), before, "with its two layers");
    assert!(u.app.ui.toast_action.is_none());
    assert!(button(&mut u, "Undo").is_none());
}

// covers: L1-U2
#[test]
fn the_deletion_toast_and_its_button_are_in_french_in_the_french_ui() {
    let (mut u, _dir, [first, alpha, beta]) = three_components("fr", "undo-fr");
    crate::layers::delete_page(&mut u.app, alpha);
    frames(&mut u, 2);
    assert_eq!(toast(&u), Some("Composant supprimé"));
    click_button(&mut u, "Annuler");
    assert_eq!(pages(&u), [first, alpha, beta]);
}

// covers: L1-U3
#[test]
fn outside_a_project_deleting_a_page_toasts_page_deleted_without_any_action() {
    let mut u = plain_ui("en");
    let second = id(u.app.session.execute("page.add", &json!({"name": "Second"})).unwrap());
    frames(&mut u, 2);
    crate::layers::delete_page(&mut u.app, second);
    frames(&mut u, 2);
    assert_eq!(toast(&u), Some("Page deleted"));
    assert!(u.app.ui.toast_action.is_none());
    assert!(button(&mut u, "Undo").is_none(), "a plain toast draws no button");
}

// covers: L1-U4
#[test]
fn another_change_after_the_deletion_takes_the_undo_button_away() {
    let (mut u, _dir, [_, alpha, _]) = three_components("en", "undo-stale");
    crate::layers::delete_page(&mut u.app, alpha);
    frames(&mut u, 2);
    assert!(button(&mut u, "Undo").is_some(), "the button is there right after the deletion");
    u.app.session.execute("shape.ellipse", &json!({"x": 200, "y": 0, "width": 30, "height": 30})).unwrap();
    frames(&mut u, 2);
    assert!(u.app.ui.toast_action.is_none());
    assert!(button(&mut u, "Undo").is_none(), "Undo would now take back the wrong change");
}

// covers: L1-U5
#[test]
fn dev_mode_still_refuses_to_add_rename_or_delete_a_page_in_a_project() {
    let (mut u, _dir, [first, alpha, beta]) = three_components("en", "dev-mode");
    for command in ["page.add", "page.rename", "page.delete"] {
        assert!(!figcraft_engine::devinspect::dev_mode_allows(command), "{command} is not a Dev Mode command");
    }
    u.app.ui.right_tab = crate::RightTab::Inspect;
    frames(&mut u, 3);
    click_button(&mut u, "Add new page");
    assert_eq!(pages(&u), [first, alpha, beta], "no component added");
    assert!(toast(&u).is_some_and(|m| m.contains("Shift+D")), "it says why: {:?}", toast(&u));
    for command in ["page.rename", "page.delete"] {
        u.app.ui.toast = None;
        assert!(crate::menus::dev_mode_refuses(&mut u.app, command), "{command} is refused");
        assert!(toast(&u).is_some_and(|m| m.contains("Shift+D")));
    }
    assert_eq!(pages(&u), [first, alpha, beta]);
}

fn has_key(lang: &Lang) -> bool {
    lang.ui.iter().any(|(k, text)| *k == "Component deleted" && matches!(text, Text::Plain(s) if !s.trim().is_empty()))
}

// covers: L1-U6
#[test]
fn component_deleted_is_in_every_catalog_and_french_reads_composant_supprime() {
    let langs = i18n::languages();
    assert_eq!(langs.len(), 22);
    for lang in langs {
        let inherited = lang.parent != 0 && langs.get(lang.parent).is_some_and(has_key);
        assert!(has_key(lang) || inherited, "{}: no \"Component deleted\" key in locales/{}.toml", lang.code, lang.code);
    }
    let fr = langs.iter().find(|l| l.code == "fr").expect("French is baked in");
    let text = fr.ui.iter().find(|(k, _)| *k == "Component deleted").map(|(_, t)| t);
    assert!(matches!(text, Some(Text::Plain("Composant supprimé"))), "French says Composant supprimé");
}
