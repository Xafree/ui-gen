//! A toast can carry one action button ("Undo"), driven through egui headless: a plain toast is
//! unchanged, the button is a named AccessKit button that runs its command once and removes the
//! toast, it lasts 8 s with a single wake at the deadline, it vanishes when anything else changed
//! (another edit, another tab) or another toast replaces it, its click never reaches the canvas
//! under it, and its label is translated.

use std::time::Duration;

use egui::accesskit::{Node, Role};
use egui::{Pos2, pos2};
use figcraft_doc::NodeId;
use figcraft_engine::Session;
use serde_json::json;

use crate::clock::{TOAST_ACTION_SECS, TOAST_SECS};
use crate::i18n::{Lang, Text};
use crate::idle_tests::wait_for_render;
use crate::layers_drag_tests::Ui;

/// The shortest repaint delay that counts as idle.
const IDLE: Duration = Duration::from_secs(1);

fn id(v: serde_json::Value) -> NodeId {
    NodeId(v["id"].as_u64().unwrap())
}

/// Two rectangles on the page, the second one last in the history; nothing selected.
fn two_rectangles() -> (Ui, NodeId, NodeId) {
    figcraft_text::scan_system_fonts();
    let mut s = Session::new();
    let a = id(s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 40, "height": 40})).unwrap());
    let b = id(s.execute("shape.rectangle", &json!({"x": 100, "y": 10, "width": 40, "height": 40})).unwrap());
    s.execute("select.none", &json!({})).unwrap();
    (Ui::new(s), a, b)
}

fn frames(u: &mut Ui, n: usize) {
    for _ in 0..n {
        u.frame(vec![]);
    }
}

fn tree(u: &mut Ui) -> Vec<Node> {
    u.ctx.enable_accesskit();
    u.frame(vec![]);
    u.frame_output(vec![]).platform_output.accesskit_update.expect("an AccessKit update").nodes.into_iter().map(|(_, n)| n).collect()
}

fn button_labels(nodes: &[Node]) -> Vec<String> {
    let mut labels: Vec<String> = nodes.iter().filter(|n| n.role() == Role::Button).filter_map(|n| n.label().map(str::to_owned)).collect();
    labels.sort();
    labels
}

fn undo_button(u: &mut Ui) -> Option<Node> {
    let mut found: Vec<Node> = tree(u).into_iter().filter(|n| n.role() == Role::Button && n.label() == Some("Undo")).collect();
    assert!(found.len() <= 1, "one Undo button, found {}", found.len());
    found.pop()
}

fn centre(n: &Node) -> Pos2 {
    let b = n.bounds().unwrap();
    pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
}

fn click(u: &mut Ui, at: Pos2) {
    u.button(at, true);
    u.button(at, false);
}

/// Draws frames until three in a row ask for no repaint sooner than [`IDLE`]; returns the last delay.
fn settle(u: &mut Ui) -> Duration {
    let mut quiet = 0;
    for _ in 0..2000 {
        let out = u.frame_raw(egui::RawInput::default());
        let delay = out.viewport_output.get(&egui::ViewportId::ROOT).map_or(Duration::MAX, |v| v.repaint_delay);
        let rendering = wait_for_render(&u.app);
        quiet = if delay >= IDLE && !rendering { quiet + 1 } else { 0 };
        if quiet >= 3 {
            return delay;
        }
    }
    panic!("still asking for frames after 2000 with no input");
}

fn toast_text(u: &Ui) -> Option<&str> {
    u.app.ui.toast.as_ref().map(|t| t.0.as_str())
}

// covers: L1-T1
#[test]
fn a_toast_without_action_is_unchanged() {
    let (mut u, ..) = two_rectangles();
    frames(&mut u, 3);
    let before = button_labels(&tree(&mut u));
    u.app.toast("Saved");
    assert_eq!(toast_text(&u), Some("Saved"));
    assert!(u.app.ui.toast_action.is_none());
    frames(&mut u, 3);
    assert_eq!(button_labels(&tree(&mut u)), before, "a plain toast draws no button");
    assert_eq!(TOAST_SECS, 3.0);
    let now = u.app.now_secs();
    u.advance_clock(now + 2.0);
    u.frame(vec![]);
    assert!(u.app.ui.toast.is_some(), "still showing after 2 s");
    u.advance_clock(now + 3.0);
    u.frame(vec![]);
    assert!(u.app.ui.toast.is_none(), "gone after 3 s");
}

// covers: L1-T2
#[test]
fn clicking_the_toast_button_runs_its_command_once_and_removes_the_toast() {
    let (mut u, a, b) = two_rectangles();
    u.app.toast_with_action("Layer added", "Undo", "edit.undo", json!({}));
    assert_eq!(toast_text(&u), Some("Layer added"));
    frames(&mut u, 3);
    let button = undo_button(&mut u).expect("a button named Undo");
    assert!(u.app.session.doc().unwrap().contains(b));
    click(&mut u, centre(&button));
    frames(&mut u, 2);
    let doc = u.app.session.doc().unwrap();
    assert!(!doc.contains(b), "Undo took the last change back");
    assert!(doc.contains(a), "and only that one: the command ran once");
    assert!(u.app.ui.toast.is_none());
    assert!(u.app.ui.toast_action.is_none());
    assert!(undo_button(&mut u).is_none());
}

// covers: L1-T3
#[test]
fn a_toast_with_action_lasts_8_seconds_with_a_single_wake_at_the_deadline() {
    let (mut u, ..) = two_rectangles();
    settle(&mut u);
    assert_eq!(TOAST_ACTION_SECS, 8.0);
    u.app.toast_with_action("Layer added", "Undo", "edit.undo", json!({}));
    let delay = settle(&mut u);
    assert!(delay > Duration::from_secs(7) && delay <= Duration::from_secs(8), "{delay:?}");
    assert!(u.app.ui.toast.is_some());
    let now = u.app.now_secs();
    u.advance_clock(now + 7.0);
    u.frame(vec![]);
    assert!(u.app.ui.toast.is_some(), "still showing after 7 s");
    assert!(u.app.ui.toast_action.is_some());
    u.advance_clock(now + 8.0);
    u.frame(vec![]);
    assert!(u.app.ui.toast.is_none(), "gone after 8 s");
    assert!(u.app.ui.toast_action.is_none());
}

// covers: L1-T4
#[test]
fn the_action_remembers_its_file_and_revision_and_goes_when_the_file_changes() {
    let (mut u, ..) = two_rectangles();
    let (uid, revision) = {
        let ds = u.app.session.ds().unwrap();
        (ds.uid, ds.revision)
    };
    u.app.toast_with_action("Layer added", "Undo", "edit.undo", json!({}));
    let action = u.app.ui.toast_action.as_ref().expect("the action is kept");
    assert_eq!((action.label.as_str(), action.command.as_str()), ("Undo", "edit.undo"));
    assert_eq!(action.params, json!({}));
    assert_eq!((action.doc_uid, action.revision), (uid, revision));
    frames(&mut u, 5);
    assert!(undo_button(&mut u).is_some(), "untouched, the button stays");

    u.app.session.execute("shape.ellipse", &json!({"x": 200, "y": 10, "width": 30, "height": 30})).unwrap();
    u.frame(vec![]);
    assert!(u.app.ui.toast_action.is_none(), "another change removes the action");
    assert!(undo_button(&mut u).is_none());
}

// covers: L1-T4
#[test]
fn the_action_goes_when_another_tab_comes_to_the_front() {
    let (mut u, ..) = two_rectangles();
    let uid = u.app.session.ds().unwrap().uid;
    u.app.toast_with_action("Layer added", "Undo", "edit.undo", json!({}));
    frames(&mut u, 3);
    assert!(undo_button(&mut u).is_some());
    u.app.run("file.new", json!({}));
    assert_ne!(u.app.session.ds().unwrap().uid, uid, "another file is in front");
    frames(&mut u, 2);
    assert!(u.app.ui.toast_action.is_none());
    assert!(undo_button(&mut u).is_none(), "the button would undo in the wrong file");
}

// covers: L1-T5
#[test]
fn a_plain_toast_after_a_toast_with_action_removes_the_action() {
    let (mut u, ..) = two_rectangles();
    u.app.toast_with_action("Layer added", "Undo", "edit.undo", json!({}));
    frames(&mut u, 3);
    assert!(undo_button(&mut u).is_some());
    u.app.toast("Saved");
    assert!(u.app.ui.toast_action.is_none());
    assert_eq!(toast_text(&u), Some("Saved"));
    frames(&mut u, 3);
    assert!(undo_button(&mut u).is_none());
}

// covers: L1-T6
#[test]
fn clicking_the_toast_button_does_not_click_the_canvas_under_it() {
    let (mut u, a, _) = two_rectangles();
    u.app.toast_with_action("Layer added", "Undo", "view.zoomIn", json!({}));
    frames(&mut u, 3);
    let at = centre(&undo_button(&mut u).expect("a button named Undo"));

    u.app.ui.toast = None;
    u.app.ui.toast_action = None;
    frames(&mut u, 2);
    u.app.session.execute("select.set", &json!({"ids": [a]})).unwrap();
    click(&mut u, at);
    assert!(u.app.session.ds().unwrap().selection.is_empty(), "without the toast, this spot is empty canvas: a click clears the selection");

    u.app.session.execute("select.set", &json!({"ids": [a]})).unwrap();
    let zoom = u.app.session.ds().unwrap().view.zoom;
    u.app.toast_with_action("Layer added", "Undo", "view.zoomIn", json!({}));
    frames(&mut u, 3);
    let at_again = centre(&undo_button(&mut u).expect("a button named Undo"));
    assert_eq!(at_again, at);
    click(&mut u, at_again);
    frames(&mut u, 2);
    assert!(u.app.session.ds().unwrap().view.zoom > zoom, "the button's command ran");
    assert_eq!(u.app.session.ds().unwrap().selection, vec![a], "the click did not reach the canvas");
}

fn has_undo(lang: &Lang) -> bool {
    lang.ui.iter().any(|(k, _)| *k == "Undo")
}

// covers: L1-T7
#[test]
fn undo_is_in_every_catalog_and_french_reads_annuler() {
    let langs = crate::i18n::languages();
    assert_eq!(langs.len(), 22);
    for lang in langs {
        let inherited = lang.parent != 0 && langs.get(lang.parent).is_some_and(has_undo);
        assert!(has_undo(lang) || inherited, "{}: no \"Undo\" key in locales/{}.toml", lang.code, lang.code);
    }
    let fr = langs.iter().find(|l| l.code == "fr").expect("French is baked in");
    let text = fr.ui.iter().find(|(k, _)| *k == "Undo").map(|(_, t)| t);
    assert!(matches!(text, Some(Text::Plain("Annuler"))), "French says Annuler");
}
