//! The review points of the Playground tab's view: the hidden document's shortcuts, the pace of the
//! image requests, the images decoded off the interface thread, the input kept within what the
//! service takes, Escape closing the view's windows, the save window's `apply` and stale warning,
//! what the export reports, and a view smaller than the page's smallest window.

use std::time::Duration;

use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, PointerButton, RawInput, Rect, ViewportId, pos2, vec2};
use figcraft_engine::capture::CaptureEvent;
use figcraft_engine::playground::{Frame, FrameFormat};
use serde_json::{Value, json};

use crate::playground_bars::Snapshot;
use crate::playground_bars_tests::{Tokens, ready};
use crate::playground_decode::Decoder;
use crate::playground_input::{Frame as PageFrame, Relay};
use crate::playground_view_tests::{Bench, IMAGE, RED, View, jpeg_frame};

fn node_named(b: &Bench, name: &str) -> Value {
    json!(b.v.app.session.doc().unwrap().nodes().find(|n| n.name == name).unwrap().id)
}

fn node_count(b: &Bench) -> usize {
    b.v.app.session.doc().unwrap().nodes().count()
}

fn key(key: Key, pressed: bool) -> Event {
    Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE }
}

fn press(button: PointerButton, at: egui::Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos: at, button, pressed, modifiers: Modifiers::NONE }
}

fn focus_the_view(b: &mut Bench) {
    let at = b.v.zone().center();
    b.v.frame(vec![Event::PointerMoved(at), press(PointerButton::Primary, at, true)]);
    b.v.frame(vec![press(PointerButton::Primary, at, false)]);
    b.v.settle(0.3);
}

// covers: E7-6.3
#[test]
fn delete_with_the_playground_in_front_leaves_the_hidden_document_unchanged() {
    let mut b = Bench::ready("review-delete", "en");
    let button = node_named(&b, "Button");
    b.v.app.run("select.set", json!({"ids": [button]}));
    let before = (node_count(&b), b.v.app.session.ds().unwrap().revision);

    b.v.press(Key::Delete, Modifiers::NONE);
    b.v.press(Key::Backspace, Modifiers::NONE);
    b.v.settle(0.3);

    assert_eq!((node_count(&b), b.v.app.session.ds().unwrap().revision), before, "the document behind the Playground tab did not change");
}

// covers: E7-6.3
#[test]
fn a_shortcut_that_does_not_touch_the_hidden_document_still_works_with_the_playground_in_front() {
    let mut b = Bench::ready("review-keep", "en");
    assert!(!b.v.app.ui.preferences);

    b.v.press(Key::Comma, Modifiers::COMMAND);
    b.v.settle(0.3);

    assert!(b.v.app.ui.preferences, "Preferences opens from its shortcut");
}

// covers: E7-6.5
#[test]
fn a_page_that_stays_the_same_is_no_longer_asked_for_images_nor_does_it_wake_the_interface() {
    let mut b = Bench::ready("review-idle", "en");
    b.v.settle(3.0);
    let asked = b.calls.count("playground.frame");

    b.v.settle(3.0);
    assert_eq!(b.calls.count("playground.frame"), asked, "no request while the page does not change");

    let time = b.v.ctx.input(|i| i.time);
    let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), b.v.screen)), time: Some(time + 0.05), ..Default::default() };
    let app = &mut b.v.app;
    let out = b.v.ctx.run_ui(input, |ui| app.ui(ui));
    let delay = out.viewport_output.get(&ViewportId::ROOT).map(|viewport| viewport.repaint_delay);
    assert!(delay.is_some_and(|delay| delay > Duration::from_secs(60)), "an idle view asks for no repaint soon: {delay:?}");
}

// covers: E7-6.5
#[test]
fn an_input_asks_for_images_again_for_a_moment() {
    let mut b = Bench::ready("review-wake", "en");
    b.v.settle(3.0);
    let asked = b.calls.count("playground.frame");

    focus_the_view(&mut b);
    b.v.frame(vec![Event::Text("a".to_string())]);
    b.v.settle(0.5);

    assert!(b.calls.count("playground.frame") > asked, "the page reacts to the input: its next image is asked for");
}

// covers: E7-6.5
#[test]
fn a_new_image_is_painted_once_decoded_off_the_interface_thread() {
    let mut b = Bench::ready("review-decode", "en");
    b.calls.push_frame(jpeg_frame(2, IMAGE[0] as u32, IMAGE[1] as u32, [20, 20, 230]));
    b.v.frame(vec![Event::PointerMoved(pos2(5.0, 5.0))]);
    let started = std::time::Instant::now();
    let blue = |b: &Bench| b.v.image().is_some_and(|(_, pixel)| pixel.b() > 180 && pixel.r() < 90);
    while !blue(&b) && started.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(10));
        b.v.settle(0.1);
    }
    assert!(blue(&b), "the newest image replaced the first one");
}

fn jpeg(width: u32, height: u32, rgb: [u8; 3]) -> Frame {
    let answer = jpeg_frame(1, width, height, rgb).unwrap();
    let data = base64_decode(answer["data"].as_str().unwrap());
    Frame { seq: 1, format: FrameFormat::Jpeg, width, height, data }
}

fn base64_decode(text: &str) -> Vec<u8> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.decode(text).unwrap()
}

fn wait_for(decoder: &mut Decoder) -> Option<Option<egui::ColorImage>> {
    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_secs(10) {
        if let Some(picture) = decoder.take() {
            return Some(picture);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

// covers: E7-6.5
#[test]
fn the_decoder_answers_later_and_keeps_the_newest_image() {
    let mut decoder = Decoder::new(&egui::Context::default());
    for rgb in [[230, 20, 20], [20, 230, 20], [20, 20, 230]] {
        decoder.submit(jpeg(64, 48, rgb));
    }
    let started = std::time::Instant::now();
    let mut newest = None;
    while newest.is_none() && started.elapsed() < Duration::from_secs(10) {
        if let Some(Some(picture)) = decoder.take()
            && picture.pixels[0].b() > 180
        {
            newest = Some(picture);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let picture = newest.expect("the newest image came back");
    assert_eq!(picture.size, [64, 48]);
}

// covers: E7-6.5
#[test]
fn an_image_that_cannot_be_decoded_comes_back_as_such() {
    let mut decoder = Decoder::new(&egui::Context::default());
    decoder.submit(Frame { seq: 1, format: FrameFormat::Png, width: 1, height: 1, data: vec![1, 2, 3] });
    assert!(matches!(wait_for(&mut decoder), Some(None)));
}

// covers: E7-6.3
#[test]
fn input_beyond_what_the_service_takes_is_kept_and_sent_later_without_a_refusal() {
    let mut b = Bench::ready("review-bound", "en");
    focus_the_view(&mut b);
    let base = b.calls.input_events().len();
    b.calls.hold("playground.input");
    let sent = 15 * 50;
    for _ in 0..15 {
        let mut events = Vec::new();
        for _ in 0..25 {
            events.push(key(Key::ArrowDown, true));
            events.push(key(Key::ArrowDown, false));
        }
        b.v.frame(events);
    }
    let waiting = b.v.app.session.execute("playground.state", &json!({})).unwrap()["inputWaiting"].as_u64().unwrap();
    assert!(waiting <= 256, "the engine holds at most 256 events: {waiting}");
    assert!(b.v.app.ui.toast.is_none(), "no error told to the user");

    b.calls.release();
    b.v.settle(4.0);
    let keys = b.calls.input_events().len() - base;
    assert_eq!(keys, sent / 2, "every pressed key reached the page, none was refused: {keys}");
    assert_eq!(b.v.app.session.execute("playground.state", &json!({})).unwrap()["inputWaiting"], json!(0));
}

// covers: E7-6.3
#[test]
fn escape_closes_the_code_window_and_is_not_relayed_to_the_page() {
    let mut b = Bench::ready("review-escape", "en");
    b.v.app.ui.playground.code_open = true;
    b.v.settle(0.3);
    let base = b.calls.input_events().len();

    b.v.press(Key::Escape, Modifiers::NONE);
    b.v.settle(0.3);

    assert!(!b.v.app.ui.playground.code_open, "Escape closed the window");
    let relayed: Vec<Value> = b.calls.input_events().into_iter().skip(base).filter(|e| e["key"] == "Escape").collect();
    assert!(relayed.is_empty(), "the page did not get the Escape: {relayed:?}");
}

fn escape_frame(events: &[Event], escape_taken: bool) -> Vec<Value> {
    let zone = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0));
    let frame = PageFrame { events, pointer: None, zone, hovered: true, focused: true, escape_taken };
    Relay::default().translate(&frame).events
}

// covers: E7-6.3
#[test]
fn the_focused_view_relays_escape_unless_it_closes_a_window() {
    let escape = [key(Key::Escape, true)];
    assert_eq!(escape_frame(&escape, false).len(), 1, "Escape goes to the page");
    assert!(escape_frame(&escape, true).is_empty(), "Escape that closes a window stays with the window");
    let enter = [key(Key::Enter, true)];
    assert_eq!(escape_frame(&enter, true).len(), 1, "the other keys still go");
}

fn open_the_save_window(b: &mut Bench) {
    b.v.click_node(Role::Button, "Save theme");
    b.v.settle(0.3);
}

fn type_name(b: &mut Bench, name: &str) {
    b.v.frame(vec![Event::Text(name.to_string())]);
    b.v.settle(0.2);
}

// covers: E7-7.5
#[test]
fn the_save_window_offers_to_apply_the_theme_and_does_not_by_default() {
    let mut s = ready("review-apply", Tokens::AccentOnly);
    open_the_save_window(&mut s.b);
    assert!(s.b.v.has(Role::CheckBox, "Set as the applied theme"), "the checkbox is there with a name");
    type_name(&mut s.b, "Plain one");
    s.press("Save");
    let saved = s.calls("playground.saveTheme");
    assert_eq!(saved.len(), 1);
    assert_ne!(saved[0]["apply"], json!(true), "applying is off unless asked");
    assert_ne!(s.state()["applied"], json!("Plain one"));
}

// covers: E7-7.5
#[test]
fn the_save_window_applies_the_saved_theme_when_the_box_is_ticked() {
    let mut s = ready("review-apply-on", Tokens::AccentOnly);
    open_the_save_window(&mut s.b);
    type_name(&mut s.b, "Applied one");
    s.b.v.click_node(Role::CheckBox, "Set as the applied theme");
    s.b.v.settle(0.2);
    s.press("Save");
    let saved = s.calls("playground.saveTheme");
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0]["apply"], json!(true));
    assert_eq!(s.state()["applied"], json!("Applied one"));
}

// covers: E7-7.5
#[test]
fn the_save_window_warns_when_the_page_is_stale() {
    let mut s = ready("review-save-stale", Tokens::AccentOnly);
    open_the_save_window(&mut s.b);
    assert!(!s.b.v.says("the saved theme may differ"), "no warning while the page is up to date");
    s.b.v.click_node(Role::Button, "Cancel");
    s.b.v.settle(0.2);

    s.b.v.app.run("frame.create", json!({"x": 0, "y": 400, "width": 10, "height": 10}));
    s.b.v.settle(0.3);
    open_the_save_window(&mut s.b);
    assert!(s.b.v.says("the saved theme may differ"), "{}", s.b.v.everything_said());
}

// covers: E7-7.3
#[test]
fn a_setting_is_shown_at_once_although_it_leaves_the_revision_and_the_journal_alone() {
    let mut s = ready("review-state", Tokens::AccentOnly);
    assert!(s.disabled(Role::Button, "Undo setting"));
    let (journal, revision) = (s.b.v.app.session.journal.len(), s.b.v.app.session.ds().unwrap().revision);

    s.run("playground.set", json!({"accent": "#2563eb"}));

    assert_eq!((s.b.v.app.session.journal.len(), s.b.v.app.session.ds().unwrap().revision), (journal, revision));
    assert!(!s.disabled(Role::Button, "Undo setting"), "the view reads the state, not the journal");
}

fn open_event(answer: Value) -> CaptureEvent {
    CaptureEvent { job: 1, command: "playground.open".to_string(), package: String::new(), outcome: Ok(answer) }
}

// covers: E7-6.4
#[test]
fn the_view_lists_what_the_page_and_the_export_report_and_hides_the_empty_lists() {
    let mut b = Bench::ready("review-reports", "en");
    for text in ["Errors reported by the page", "Components missing from the library", "What the export could not do"] {
        assert!(!b.v.says(text), "nothing to report, nothing shown: {text}");
    }

    let bg = node_named(&b, "Bg");
    let values = json!({"Light": "#123456", "Dark": "#654321"});
    b.v.app.run("variable.createCollection", json!({"name": "Other", "modes": ["Light", "Dark"]})).unwrap();
    let created = b.v.app.run("variable.create", json!({"collection": "Other", "name": "zebre/unique", "type": "color", "values": values})).unwrap();
    let bound = json!({"ids": [bg], "field": "fill", "variable": created["id"]});
    b.v.app.run("variable.bind", bound).unwrap();
    b.calls.push_frame(jpeg_frame(2, IMAGE[0] as u32, IMAGE[1] as u32, RED));
    b.v.app.run("playground.refresh", json!({}));
    b.v.settle(1.0);
    crate::playground_bars::note_event(&mut b.v.app, &open_event(json!({"errors": ["ReferenceError: boom is not defined"], "shape": true})));
    assert!(b.v.app.run("playground.template", json!({"name": "dashboard"})).is_some());
    b.v.settle(0.5);

    assert!(b.v.says("Errors reported by the page"), "{}", b.v.everything_said());
    assert!(b.v.says("boom is not defined"));
    assert!(b.v.says("Components missing from the library"), "{}", b.v.everything_said());
    assert!(b.v.says("What the export could not do"), "{}", b.v.everything_said());
    assert!(b.v.says("zebre"), "the note names the token");
}

#[test]
fn the_state_notes_are_read_with_their_component_and_message() {
    let snapshot = Snapshot::read(&json!({
        "notes": [{"code": "token-missing", "component": "ui-button", "message": "x is missing"}],
        "missing": ["Chart"],
        "inputWaiting": 3,
        "applyWaiting": true,
    }));
    assert_eq!(snapshot.notes.len(), 1);
    assert_eq!((snapshot.notes[0].component.as_str(), snapshot.notes[0].message.as_str()), ("ui-button", "x is missing"));
    assert_eq!(snapshot.missing, ["Chart"]);
    assert!(snapshot.waiting());
}

// covers: E7-6.1
#[test]
fn a_view_smaller_than_the_smallest_page_keeps_the_page_size_and_hides_the_notices() {
    let mut b = Bench::ready("review-small", "en");
    b.v.app.run("frame.create", json!({"x": 0, "y": 400, "width": 10, "height": 10}));
    b.v.settle(0.3);
    assert!(b.v.says("The file changed since the page was exported."), "the notice shows while the view is large enough");

    b.v.screen = vec2(700.0, 300.0);
    b.v.settle(0.8);

    let zone = b.v.zone();
    assert!(zone.height() < 240.0, "the view is under the smallest page: {zone:?}");
    let (shown, _) = b.v.image().expect("the picture is painted");
    assert!(shown.width() >= 319.0 && shown.height() >= 239.0, "the picture keeps the smallest window instead of shrinking to the view: {shown:?}");
    assert!(!b.v.says("The file changed since the page was exported."), "no notice hangs over a cropped page");
}

fn within_a_point(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1.0
}

const PANEL_CONTROLS: [(Role, &str); 7] = [
    (Role::Navigation, "Sidebar"),
    (Role::Pane, "Layers"),
    (Role::Pane, "Design"),
    (Role::Tab, "File"),
    (Role::Tab, "Design"),
    (Role::Tab, "Prototype"),
    (Role::Button, "Share: live session, export"),
];

fn panel_controls_on_screen(b: &Bench) -> Vec<&'static str> {
    PANEL_CONTROLS.iter().filter(|(role, label)| b.v.has(*role, label)).map(|(_, label)| *label).collect()
}

// covers: E7-6.1
#[test]
fn the_panels_and_the_rail_of_the_hidden_document_are_gone_while_the_playground_is_in_front() {
    let mut b = Bench::ready("review-panels", "en");
    assert_eq!(panel_controls_on_screen(&b), Vec::<&str>::new(), "no control of the hidden document is reachable");
    let outer = b.v.zone();
    assert!(outer.width() >= b.v.screen.x - 2.0, "the view takes the whole width under the tab bar: {outer:?}");

    b.v.click_node(Role::Tab, "lib");
    b.v.settle(0.3);
    assert_eq!(panel_controls_on_screen(&b).len(), PANEL_CONTROLS.len(), "the panels are back with the document: {:?}", panel_controls_on_screen(&b));

    b.v.click_node(Role::Tab, "Playground");
    b.v.settle(0.3);
    assert_eq!(panel_controls_on_screen(&b), Vec::<&str>::new(), "and gone again with the playground");
}

// covers: E7-6.4
#[test]
fn the_notices_have_their_own_band_above_the_page_and_the_page_is_asked_for_the_size_left() {
    let mut b = Bench::ready("review-band", "en");
    let before = b.v.zone();
    b.v.app.run("frame.create", json!({"x": 0, "y": 400, "width": 10, "height": 10}));
    b.v.settle(0.3);
    crate::playground_bars::note_event(&mut b.v.app, &open_event(json!({"errors": ["ReferenceError: boom is not defined"], "shape": true})));
    b.v.settle(0.8);

    let refresh = View::bounds(b.v.node(Role::Button, "Refresh").expect("the stale notice shows"));
    let (picture, _) = b.v.image().expect("the picture is painted");
    let zone = b.v.zone();
    assert!(refresh.max.y <= picture.min.y, "the band ends above the picture: {refresh:?} {picture:?}");
    assert!(zone.min.y > before.min.y && within_a_point(zone.max.y, before.max.y), "the page area shrank from the top: {before:?} {zone:?}");
    let resized = b.calls.params("playground.resize");
    let last = resized.last().expect("a resize went to the service");
    assert_eq!(last["height"].as_f64().map(|h| h.round()), Some(f64::from(zone.height().round())), "the page got the height left: {last}");
}

// covers: E7-6.4
#[test]
fn the_notices_band_is_gone_and_the_page_gets_its_height_back_when_nothing_is_left_to_say() {
    let mut b = Bench::ready("review-band-gone", "en");
    let before = b.v.zone();
    b.v.app.run("frame.create", json!({"x": 0, "y": 400, "width": 10, "height": 10}));
    b.v.settle(0.5);
    assert!(b.v.zone().min.y > before.min.y, "the band takes room while the file is stale");

    b.calls.push_frame(jpeg_frame(2, IMAGE[0] as u32, IMAGE[1] as u32, RED));
    b.v.click_node(Role::Button, "Refresh");
    b.v.settle(1.0);

    let after = b.v.zone();
    assert!(
        within_a_point(after.min.y, before.min.y) && within_a_point(after.max.y, before.max.y),
        "the page area is whole again: {before:?} {after:?}"
    );
}
