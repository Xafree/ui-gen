//! The queue of the "Choose your components" surface: it belongs to the file it started from, whatever
//! tab comes to the front, and it stops cleanly. The bench is the one of `import_window_ui_tests`.

use serde_json::json;

use crate::import_window_ui_tests::{Bench, IMPORT, OTHER, PACKAGE, mentions};

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
