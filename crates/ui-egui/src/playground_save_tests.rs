//! Saving the playground's settings as a theme (E7-7.5): the "Save theme" button opens a
//! confirmation, the document changes only on "Save", an empty or taken name is told in the window,
//! and a failure is shown without a crash. Names of the window's controls (the English keys of the
//! catalogs): text field "Theme name", check box "Overwrite the chosen theme", buttons "Save" and
//! "Cancel"; messages "Give the theme a name" and "A theme with this name already exists". The
//! harness is `playground_bars_tests.rs`.

use egui::accesskit::Role;
use egui::{Event, Key, Modifiers};
use figcraft_doc::VariableValue;
use serde_json::json;

use crate::playground_bars_tests::{Studio, Tokens, ready};

const EMPTY: &str = "Give the theme a name";
const TAKEN: &str = "A theme with this name already exists";

fn collections(s: &Studio) -> Vec<String> {
    s.b.v.app.session.doc().unwrap().variables.collections.iter().map(|c| c.name.clone()).collect()
}

fn revision(s: &Studio) -> u64 {
    s.b.v.app.session.docs[s.b.v.app.session.active].revision
}

fn primary_in_light(s: &Studio) -> Option<VariableValue> {
    let doc = s.b.v.app.session.doc().unwrap();
    doc.variables.variables.iter().find(|v| v.id == "v:primary").and_then(|v| v.values.get("m-light").cloned())
}

fn open_window(s: &mut Studio) {
    s.press("Save theme");
    assert!(s.b.v.has(Role::TextInput, "Theme name"), "the confirmation has a name field: {}", s.b.v.everything_said());
    assert!(s.b.v.has(Role::Button, "Save") && s.b.v.has(Role::Button, "Cancel"), "and a Save and a Cancel button");
}

fn type_name(s: &mut Studio, text: &str) {
    assert!(s.b.v.has(Role::TextInput, "Theme name"), "the name field is on screen");
    s.b.v.click_node(Role::TextInput, "Theme name");
    s.b.v.settle(0.1);
    s.b.v.press(Key::A, Modifiers::COMMAND);
    s.b.v.press(Key::Backspace, Modifiers::NONE);
    if !text.is_empty() {
        s.b.v.frame(vec![Event::Text(text.to_string())]);
    }
    s.b.v.settle(0.2);
}

fn toast(s: &Studio) -> String {
    s.b.v.app.ui.toast.as_ref().map(|(text, _)| text.clone()).unwrap_or_default()
}

// covers: E7-7.5
#[test]
fn playground_save_asks_first_then_creates_the_theme_in_one_undo_step_and_names_it_in_the_toast() {
    let mut s = ready("save", Tokens::Rich);
    s.run("playground.set", json!({"accent": "#2563eb"}));
    let (before, revision_before) = (collections(&s), revision(&s));

    open_window(&mut s);
    type_name(&mut s, "Ocean");
    assert_eq!((collections(&s), revision(&s)), (before.clone(), revision_before), "the document is untouched until Save");
    assert!(s.calls("playground.saveTheme").is_empty());

    s.press("Save");
    let saved = s.calls("playground.saveTheme");
    assert_eq!(saved.len(), 1, "{saved:?}");
    assert_eq!(saved[0]["name"], json!("Ocean"));
    assert_ne!(saved[0]["update"], json!(true), "a new theme, not an update");
    assert!(collections(&s).contains(&"Ocean".to_string()), "{:?}", collections(&s));
    assert!(toast(&s).contains("Ocean"), "the toast names the theme: {:?}", toast(&s));
    assert!(!s.b.v.has(Role::Button, "Save"), "the window closed");

    s.run("edit.undo", json!({}));
    assert_eq!(collections(&s), before, "one undo takes the theme back");
}

// covers: E7-7.5
#[test]
fn playground_save_window_closes_with_cancel_and_escape_and_saves_nothing() {
    let mut s = ready("save-cancel", Tokens::Rich);
    s.run("playground.set", json!({"accent": "#2563eb"}));
    let before = collections(&s);

    open_window(&mut s);
    type_name(&mut s, "Ocean");
    s.press("Cancel");
    assert!(!s.b.v.has(Role::Button, "Save"), "Cancel closes the window");

    open_window(&mut s);
    s.b.v.press(Key::Escape, Modifiers::NONE);
    s.b.v.settle(0.3);
    assert!(!s.b.v.has(Role::Button, "Save"), "Escape closes the window");
    assert_eq!(collections(&s), before);
    assert!(s.calls("playground.saveTheme").is_empty());
}

// covers: E7-7.5
#[test]
fn playground_save_an_empty_or_taken_name_is_told_in_the_window_and_saves_nothing() {
    let mut s = ready("save-names", Tokens::Rich);
    s.run("playground.set", json!({"accent": "#2563eb"}));
    let before = collections(&s);
    open_window(&mut s);

    type_name(&mut s, "");
    s.press("Save");
    assert!(s.b.v.says(EMPTY), "an empty name is told: {}", s.b.v.everything_said());
    assert!(s.b.v.has(Role::Button, "Save"), "the window stays open");

    type_name(&mut s, "Theme");
    s.press("Save");
    assert!(s.b.v.says(TAKEN), "a taken name is told: {}", s.b.v.everything_said());
    assert!(s.b.v.has(Role::Button, "Save"), "the window stays open");
    assert!(s.calls("playground.saveTheme").is_empty(), "nothing was sent");
    assert_eq!(collections(&s), before);
}

// covers: E7-7.5
#[test]
fn playground_save_can_overwrite_the_chosen_theme_instead_of_adding_one() {
    let mut s = ready("save-update", Tokens::Rich);
    s.run("playground.set", json!({"accent": "#2563eb"}));
    let (names, original) = (collections(&s), primary_in_light(&s));

    open_window(&mut s);
    assert!(s.b.v.has(Role::CheckBox, "Overwrite the chosen theme"), "the window offers to overwrite the theme");
    s.b.v.click_node(Role::CheckBox, "Overwrite the chosen theme");
    s.b.v.settle(0.2);
    s.press("Save");

    let saved = s.calls("playground.saveTheme");
    assert_eq!(saved.len(), 1, "{saved:?}");
    assert_eq!((saved[0]["update"].clone(), saved[0]["name"].clone()), (json!(true), json!("Theme")), "the name is the chosen theme's");
    assert_eq!(collections(&s), names, "no collection is added");
    assert_ne!(primary_in_light(&s), original, "the accent was written into the theme");
}

// covers: E7-7.5
#[test]
fn playground_save_failure_is_shown_and_does_not_crash() {
    let mut s = ready("save-failure", Tokens::Rich);
    s.run("playground.set", json!({"accent": "#2563eb"}));
    let library = s.b.v.app.session.active;
    open_window(&mut s);
    type_name(&mut s, "Fresh");

    assert!(s.b.v.app.run("file.new", json!({})).is_some());
    s.b.v.settle(0.3);
    assert!(s.b.v.has(Role::Button, "Save"), "the window is still there");
    s.press("Save");
    assert!(s.b.v.says("switch to the document the playground shows first"), "the failure is on screen: {}", s.b.v.everything_said());
    assert!(s.calls("playground.saveTheme").is_empty());

    assert!(s.b.v.app.run("doc.switch", json!({"index": library})).is_some());
    assert!(!collections(&s).contains(&"Fresh".to_string()));
}
