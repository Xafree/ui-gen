//! Closing a tab with unsaved changes asks Save / Don't Save / Cancel, naming the file; Alt held
//! closes at once, and a programmatic `file.close` still refuses without opening anything.

use egui::{Event, Key, Modifiers, RawInput};
use figcraft_engine::Session;
use serde_json::json;

use crate::tab_close::Stage;
use crate::windows::CloseAnswer;
use crate::windows_tests::{SaveAs, frame, rects_in, settle, temp_dir};
use crate::{FigApp, NoServices};

fn dirty_session(path: Option<&std::path::Path>) -> Session {
    let mut s = Session::new();
    if let Some(p) = path {
        s.execute("file.saveAs", &json!({"path": p.to_string_lossy()})).unwrap();
    }
    s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 10, "height": 10})).unwrap();
    s
}

fn asking(path: Option<&std::path::Path>, save_as: Vec<std::path::PathBuf>) -> (egui::Context, FigApp, std::rc::Rc<std::cell::Cell<usize>>) {
    let asked = std::rc::Rc::new(std::cell::Cell::new(0));
    let ctx = egui::Context::default();
    let mut app = FigApp::new(dirty_session(path), Box::new(SaveAs { paths: save_as, asked: asked.clone() }));
    frame(&ctx, &mut app, false);
    crate::tabs::request_close_tab(&mut app, 0, false);
    frame(&ctx, &mut app, false);
    assert!(app.ui.tab_close.is_some(), "the question shows");
    (ctx, app, asked)
}

#[test]
fn closing_a_clean_tab_asks_nothing() {
    let ctx = egui::Context::default();
    let mut app = FigApp::new(Session::new(), Box::new(NoServices));
    frame(&ctx, &mut app, false);
    crate::tabs::request_close_tab(&mut app, 0, false);
    assert!(app.ui.tab_close.is_none());
    assert!(app.session.docs.is_empty(), "closed");
}

#[test]
fn closing_a_tab_with_unsaved_changes_asks_instead_of_closing() {
    let (_, app, _) = asking(None, vec![]);
    assert_eq!(app.session.docs.len(), 1, "still open");
    assert!(app.ui.toast.is_none(), "no toast: the question is the answer");
}

#[test]
fn the_question_names_the_file_and_offers_save_dont_save_and_cancel() {
    let (ctx, mut app, _) = asking(None, vec![]);
    ctx.enable_accesskit();
    frame(&ctx, &mut app, false);
    let mut out = ctx.run_ui(RawInput::default(), |ui| app.ui(ui));
    out.textures_delta.clear();
    let nodes = out.platform_output.accesskit_update.expect("an AccessKit update").nodes;
    let buttons: Vec<String> =
        nodes.iter().filter(|(_, n)| n.role() == egui::accesskit::Role::Button).filter_map(|(_, n)| n.label().map(str::to_string)).collect();
    for want in ["Don't Save", "Cancel", "Save"] {
        assert!(buttons.iter().any(|b| b == want), "{want} in {buttons:?}");
    }
    let name = app.session.doc().unwrap().name.clone();
    let texts: Vec<String> = nodes.iter().filter_map(|(_, n)| n.value().or_else(|| n.label()).map(str::to_string)).collect();
    assert!(texts.iter().any(|t| t.contains(&name) && t.contains("unsaved")), "the file {name:?} is named in {texts:?}");
}

#[test]
fn the_question_blocks_the_region_navigation() {
    let (_, app, _) = asking(None, vec![]);
    assert!(crate::a11y::blocked(&app));
}

#[test]
fn alt_closes_at_once_and_discards() {
    let ctx = egui::Context::default();
    let mut app = FigApp::new(dirty_session(None), Box::new(NoServices));
    frame(&ctx, &mut app, false);
    crate::tabs::request_close_tab(&mut app, 0, true);
    assert!(app.ui.tab_close.is_none());
    assert!(app.session.docs.is_empty());
}

#[test]
fn a_programmatic_close_still_refuses_and_opens_nothing() {
    let ctx = egui::Context::default();
    let mut app = FigApp::new(dirty_session(None), Box::new(NoServices));
    frame(&ctx, &mut app, false);
    assert!(app.run("file.close", json!({"index": 0})).is_none());
    frame(&ctx, &mut app, false);
    assert!(app.ui.tab_close.is_none());
    assert_eq!(app.session.docs.len(), 1);
}

#[test]
fn save_saves_a_file_with_a_path_then_closes_its_tab() {
    let d = temp_dir("tab-path");
    let path = d.join("a.figcraft");
    let (ctx, mut app, asked) = asking(Some(&path), vec![]);
    crate::tab_close::answer(&mut app, &ctx, CloseAnswer::Save);
    settle(&ctx, &mut app);
    assert_eq!(rects_in(&path), 1, "saved where it was");
    assert_eq!(asked.get(), 0);
    assert!(app.session.docs.is_empty(), "closed");
    assert!(app.ui.tab_close.is_none());
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn save_asks_where_for_an_untitled_file_and_closes_once_saved() {
    let d = temp_dir("tab-untitled");
    let path = d.join("new.figcraft");
    let (ctx, mut app, asked) = asking(None, vec![path.clone()]);
    crate::tab_close::answer(&mut app, &ctx, CloseAnswer::Save);
    settle(&ctx, &mut app);
    assert_eq!(asked.get(), 1);
    assert_eq!(rects_in(&path), 1, "saved there");
    assert!(app.session.docs.is_empty(), "then the tab closes");
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn a_cancelled_save_as_keeps_the_tab_open() {
    let (ctx, mut app, asked) = asking(None, vec![]);
    crate::tab_close::answer(&mut app, &ctx, CloseAnswer::Save);
    settle(&ctx, &mut app);
    assert_eq!(asked.get(), 1);
    assert_eq!(app.session.docs.len(), 1);
    assert!(app.session.ds().unwrap().is_dirty());
    assert!(app.ui.tab_close.is_none(), "not asking again");
}

#[test]
fn a_failed_save_keeps_the_tab_open_with_its_error() {
    let d = temp_dir("tab-gone");
    let path = d.join("a.figcraft");
    let (ctx, mut app, _) = asking(Some(&path), vec![]);
    std::fs::remove_dir_all(&d).unwrap();
    crate::tab_close::answer(&mut app, &ctx, CloseAnswer::Save);
    settle(&ctx, &mut app);
    assert_eq!(app.session.docs.len(), 1, "the tab stays");
    assert!(app.session.ds().unwrap().is_dirty());
    assert!(app.ui.toast.is_some(), "the error shows");
    assert!(app.ui.tab_close.is_none());
}

#[test]
fn dont_save_closes_without_saving() {
    let d = temp_dir("tab-discard");
    let path = d.join("a.figcraft");
    let (ctx, mut app, _) = asking(Some(&path), vec![]);
    crate::tab_close::answer(&mut app, &ctx, CloseAnswer::DontSave);
    settle(&ctx, &mut app);
    assert!(app.session.docs.is_empty());
    assert_eq!(rects_in(&path), 0, "nothing written");
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn cancel_and_escape_do_nothing() {
    let (ctx, mut app, _) = asking(None, vec![]);
    crate::tab_close::answer(&mut app, &ctx, CloseAnswer::Cancel);
    settle(&ctx, &mut app);
    assert!(app.ui.tab_close.is_none());
    assert_eq!(app.session.docs.len(), 1);
    assert!(app.session.ds().unwrap().is_dirty());

    crate::tabs::request_close_tab(&mut app, 0, false);
    frame(&ctx, &mut app, false);
    assert!(app.ui.tab_close.as_ref().is_some_and(|q| q.stage == Stage::Asking));
    let mut input = RawInput::default();
    input.events.push(Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
    ctx.run_ui(input, |ui| app.ui(ui)).textures_delta.clear();
    assert!(app.ui.tab_close.is_none(), "Escape closes the question");
    assert_eq!(app.session.docs.len(), 1);
    assert!(app.session.ds().unwrap().is_dirty());
}
