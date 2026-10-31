//! The queue of the "Choose your components" surface: it belongs to the file it started from, whatever
//! tab comes to the front, and it stops cleanly. The bench is the one of `import_window_ui_tests`.

use egui::{Key, Modifiers};
use serde_json::json;

use crate::import_window_ui_tests::{Bench, IMPORT, LIST, OTHER, PACKAGE, VERSION, mentions, source};

fn open_pair(tag: &str) -> Bench {
    let mut b = Bench::pair(tag, &["Alert", "Button", "Chip"], &["Button", "Slider"]);
    b.open(None);
    b.wait_until("both cards are listed", |b| b.has("Slider") && b.has("Alert"));
    b
}

fn new_tab_in_front(b: &mut Bench) -> u64 {
    b.run("file.new", json!({}));
    b.ui.app.session.ds().expect("a file is in front").uid
}

fn front(b: &Bench) -> u64 {
    b.ui.app.session.ds().expect("a file is in front").uid
}

#[test]
fn the_queue_stays_on_its_file_when_another_tab_comes_to_the_front() {
    let mut b = open_pair("queue-tab");
    let project_doc = front(&b);
    b.click("Alert");
    b.click("Slider");
    b.hold();
    let mark = b.mark();
    b.click("Import");
    let other = new_tab_in_front(&mut b);
    assert_ne!(other, project_doc);

    b.release_all();
    b.wait_until("the summary is drawn", |b| b.shows("Imported 2 of 2 components"));
    let calls = b.ran_since(mark, IMPORT);
    let order: Vec<&str> = calls.iter().map(|c| c["package"].as_str().unwrap_or_default()).collect();
    assert_eq!(order, [PACKAGE, OTHER], "the queue goes on against its own project");
    assert_eq!(front(&b), other, "the tab the user chose stays in front");
    let shown = b.shown();
    assert!(!mentions(&shown, "Couldn't import"), "no false failure: {shown:?}");
}

#[test]
fn closing_the_file_of_the_queue_stops_it_and_counts_the_rest_as_failed_with_a_message() {
    let mut b = open_pair("queue-closed");
    let project_doc = front(&b);
    b.click("Alert");
    b.click("Slider");
    b.hold();
    let mark = b.mark();
    b.click("Import");
    new_tab_in_front(&mut b);
    let index = b.ui.app.session.docs.iter().position(|d| d.uid == project_doc).expect("the file is open");
    b.run("file.close", json!({"index": index, "force": true}));

    b.release_all();
    b.settle();
    assert_eq!(b.ran_since(mark, IMPORT).len(), 1, "nothing starts once the file is gone");
    let failure = b.ui.app.ui.last_error.clone().unwrap_or_default();
    assert!(failure.contains("Slider") && failure.contains("closed"), "the message says what happened: {failure:?}");
}

#[test]
fn cancel_during_a_run_keeps_the_surface_until_the_running_component_ends_and_starts_nothing_else() {
    let mut b = open_pair("stop-cancel");
    b.click("Alert");
    b.click("Slider");
    b.hold();
    let mark = b.mark();
    b.click("Import");
    b.click("Cancel");
    assert!(b.window_open(), "the surface stays while the job runs: {:?}", b.shown());
    assert!(b.shows("Stopping"), "and says so: {:?}", b.shown());

    b.release_all();
    b.settle();
    assert!(!b.window_open(), "it goes once the job ended");
    assert_eq!(b.ran_since(mark, IMPORT).len(), 1, "the next component never starts");
    let toast = b.ui.app.ui.toast.as_ref().map(|t| t.0.clone()).unwrap_or_default();
    assert!(toast.contains("Stopped after 1 of 2"), "{toast:?}");
}

#[test]
fn escape_during_a_run_stops_like_cancel() {
    let mut b = open_pair("stop-escape");
    b.click("Alert");
    b.click("Slider");
    b.hold();
    let mark = b.mark();
    b.click("Import");
    b.press(Key::Escape, Modifiers::NONE);
    assert!(b.window_open() && b.shows("Stopping"), "{:?}", b.shown());
    b.release_all();
    b.settle();
    assert!(!b.window_open());
    assert_eq!(b.ran_since(mark, IMPORT).len(), 1);
}

#[test]
fn a_batch_whose_surface_lives_in_a_second_window_goes_on_to_its_summary() {
    let mut b = Bench::pair("second-window", &["Alert", "Button", "Chip"], &["Button", "Slider"]);
    for package in [PACKAGE, OTHER] {
        b.run(LIST, json!({"package": package}));
    }
    b.settle();
    let index = b.ui.app.session.active;
    let ctx = b.ui.ctx.clone();
    ctx.set_embed_viewports(false);
    let window = crate::windows::open(&mut b.ui.app, &ctx, Some(index), None, None).expect("a second window");
    let wanted = [(PACKAGE, "Alert"), (OTHER, "Slider")];
    crate::windows::as_window(&mut b.ui.app, window, |app| crate::dialogs::import_component::start_for_test(app, &wanted));
    for _ in 0..20 {
        b.ui.frame(vec![]);
    }
    let summary = crate::windows::as_window(&mut b.ui.app, window, |app| crate::dialogs::import_component::summary_for_test(app)).flatten();
    assert_eq!(summary, Some((2, 2)), "the second window gets its events");
}

#[test]
fn escape_closes_a_modal_over_a_surface_first_and_leaves_the_surface() {
    let mut b = open_pair("escape-modal");
    b.ui.app.ui.about = true;
    b.settle();
    b.press(Key::Escape, Modifiers::NONE);
    assert!(!b.ui.app.ui.about, "the modal on top goes");
    assert!(b.window_open(), "the surface behind stays");
    b.press(Key::Escape, Modifiers::NONE);
    assert!(!b.window_open());

    b.open_settings();
    b.ui.app.ui.preferences = true;
    b.settle();
    b.press(Key::Escape, Modifiers::NONE);
    assert!(!b.ui.app.ui.preferences);
    assert!(b.flat().contains("Installed in this project"), "the page stays: {:?}", b.shown());
}

#[test]
fn escape_goes_to_the_focused_search_first_then_to_the_surface() {
    let mut b = open_pair("escape-search");
    b.click("Search components");
    b.type_text("chi");
    b.press(Key::Escape, Modifiers::NONE);
    assert!(b.window_open(), "the field takes the first Escape");
    assert!(b.has("Alert"), "and it emptied: {:?}", b.shown());
    b.press(Key::Escape, Modifiers::NONE);
    assert!(!b.window_open(), "the next one closes the surface");
}

#[test]
fn escape_ends_the_version_being_changed_and_the_removal_being_asked_before_the_page() {
    let mut b = Bench::nominal("escape-sub", &["Alert"]);
    b.open_settings();
    b.click("Change version");
    assert!(b.has("Apply"));
    b.press(Key::Escape, Modifiers::NONE);
    assert!(!b.has("Apply") && b.has("Change version"), "the row comes back: {:?}", b.shown());
    b.click("Remove");
    assert!(b.has("Confirm removal"));
    b.press(Key::Escape, Modifiers::NONE);
    assert!(!b.has("Confirm removal") && b.has("Remove"), "{:?}", b.shown());
    assert!(b.flat().contains("Installed in this project"), "the page stays");
    b.press(Key::Escape, Modifiers::NONE);
    assert!(!b.flat().contains("Installed in this project"), "the next Escape closes it");
}

fn wide_and_slim(tag: &str) -> Bench {
    let values = |prefix: &str, count: usize| (0..count).map(|index| format!("{prefix}{index}")).collect::<Vec<_>>();
    let wide = json!({"name": "Wide", "props": [
        {"name": "variant", "type": "enum", "values": values("s", 9)},
        {"name": "size", "type": "enum", "values": values("z", 4)},
    ]});
    let slim = json!({"name": "Slim", "props": []});
    let inspected = json!({"package": PACKAGE, "version": VERSION, "framework": "react", "components": [wide, slim], "notes": []});
    let mut b = Bench::with(tag, &[source(PACKAGE, VERSION, false)], vec![(PACKAGE.to_string(), Ok(inspected))]);
    b.open(None);
    b.wait_until("the rows are listed", |b| b.has("Wide"));
    b
}

#[test]
fn an_oversized_component_is_named_and_does_not_hold_the_others_back() {
    let mut b = wide_and_slim("oversized-others");
    b.click("Wide");
    b.click("Slim");
    let mark = b.mark();
    b.click("Import");
    b.wait_until("the summary is drawn", |b| b.shows("Imported 1 of 2 components"));
    let calls = b.ran_since(mark, IMPORT);
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!(calls[0]["component"], json!("Slim"));
    assert!(b.shows("Wide: More than"), "the failure names it: {:?}", b.shown());
}

#[test]
fn an_oversized_component_alone_is_named_in_the_refusal() {
    let mut b = wide_and_slim("oversized-alone");
    b.click("Wide");
    b.click("Import");
    assert!(b.shows("Wide: More than"), "{:?}", b.shown());
}

#[test]
fn import_clicked_during_a_run_says_it_is_running_and_starts_nothing() {
    let mut b = open_pair("import-twice");
    b.click("Alert");
    b.click("Slider");
    b.hold();
    let mark = b.mark();
    b.click("Import");
    b.click("Import");
    assert!(b.shows("The import is already running."), "{:?}", b.shown());
    assert_eq!(b.ran_since(mark, IMPORT).len(), 1);
    b.release_all();
    b.wait_until("the summary is drawn", |b| b.shows("Imported 2 of 2 components"));
    assert!(!b.shows("The import is already running."), "the note ends with the run");
}
