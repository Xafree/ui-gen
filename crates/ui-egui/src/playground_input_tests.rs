//! The Playground tab relays the window and the user's input to the page (E7-6.2, E7-6.3): the
//! view's area is the page's viewport, the pointer and the wheel go over it in page coordinates,
//! the focused view relays the closed list of keys and typed text, Tab leaves it, and UIGEN's
//! shortcuts stand aside while it has the focus. The service is the fake backend of
//! `playground_view_tests.rs`.

use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, MouseWheelUnit, PointerButton, TouchPhase, pos2, vec2};
use figcraft_engine::ToolId;
use serde_json::Value;

use crate::playground_view_tests::{Bench, DOC_TAB};

const KEYS: [&str; 14] = [
    "Enter",
    "Tab",
    "Escape",
    "Backspace",
    "Delete",
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Space",
];

fn of_type<'a>(events: &'a [Value], kind: &str) -> Vec<&'a Value> {
    events.iter().filter(|e| e["type"] == kind).collect()
}

fn at(event: &Value, x: f64, y: f64) -> bool {
    let near = |key: &str, expected: f64| event[key].as_f64().is_some_and(|v| (v - expected).abs() <= 1.0);
    near("x", x) && near("y", y)
}

fn key_names(events: &[Value]) -> Vec<String> {
    of_type(events, "key").iter().map(|e| e["key"].as_str().unwrap().to_string()).collect()
}

fn typed(events: &[Value]) -> Vec<String> {
    of_type(events, "text").iter().map(|e| e["text"].as_str().unwrap().to_string()).collect()
}

fn text(b: &mut Bench, s: &str) {
    b.v.frame(vec![Event::Text(s.to_string())]);
    b.v.settle(0.3);
}

// covers: E7-6.2
#[test]
fn playground_resizing_the_window_sends_one_grouped_resize_with_the_size_of_the_view() {
    let mut b = Bench::ready("resize", "en");
    let before = b.calls.count("playground.resize");
    for size in [(1400.0, 880.0), (1350.0, 860.0), (1300.0, 840.0), (1250.0, 820.0), (1200.0, 800.0)] {
        b.v.screen = vec2(size.0, size.1);
        b.v.frame(vec![]);
    }
    b.v.settle(1.5);
    let sent = b.calls.params("playground.resize");
    assert_eq!(sent.len(), before + 1, "five window sizes in a row make one resize: {sent:?}");
    let zone = b.v.zone();
    let last = sent.last().unwrap();
    assert!(last["width"].as_f64().is_some_and(|w| (w - f64::from(zone.width())).abs() <= 1.0), "{last} against {zone:?}");
    assert!(last["height"].as_f64().is_some_and(|h| (h - f64::from(zone.height())).abs() <= 1.0), "{last} against {zone:?}");
    assert!(last["scale"].as_f64().is_some_and(|s| (1.0..=2.0).contains(&s)), "{last}");
    b.v.settle(1.5);
    assert_eq!(b.calls.count("playground.resize"), before + 1, "a window that stays as it is sends nothing more");
}

// covers: E7-6.2
#[test]
fn playground_pointer_over_the_view_is_relayed_in_page_coordinates() {
    let mut b = Bench::ready("pointer", "en");
    let zone = b.v.zone();
    let (hover, press) = (zone.min + vec2(30.0, 40.0), zone.min + vec2(60.0, 70.0));
    let button = |pressed| Event::PointerButton { pos: press, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    b.v.frame(vec![Event::PointerMoved(hover)]);
    b.v.settle(0.3);
    b.v.frame(vec![Event::PointerMoved(press), button(true)]);
    b.v.settle(0.3);
    b.v.frame(vec![button(false)]);
    b.v.settle(0.3);
    let wheel = Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: vec2(0.0, -40.0),
        phase: TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers: Modifiers::NONE,
    };
    b.v.frame(vec![Event::PointerMoved(press), wheel]);
    b.v.settle(0.3);

    let events = b.calls.input_events();
    assert!(of_type(&events, "move").iter().any(|e| at(e, 30.0, 40.0)), "a move at the page point (30, 40): {events:?}");
    assert!(of_type(&events, "down").iter().any(|e| at(e, 60.0, 70.0) && e["button"] == "left"), "{events:?}");
    assert!(of_type(&events, "up").iter().any(|e| at(e, 60.0, 70.0) && e["button"] == "left"), "{events:?}");
    let wheels = of_type(&events, "wheel");
    assert!(
        wheels.iter().any(|e| at(e, 60.0, 70.0) && e["dx"].as_f64() == Some(0.0) && e["dy"].as_f64().is_some_and(|dy| (dy - 40.0).abs() < 0.5)),
        "scrolling down is a positive dy: {events:?}"
    );
}

// covers: E7-6.2
#[test]
fn playground_pointer_outside_the_view_relays_nothing() {
    let mut b = Bench::ready("outside", "en");
    let zone = b.v.zone();
    assert!(zone.min.y > 20.0, "the bars are above the view: {zone:?}");
    b.v.move_to(zone.center());
    b.v.settle(0.3);
    let relayed = b.calls.input_events().len();
    let out = pos2(zone.center().x, zone.min.y - 12.0);
    let button = |pressed| Event::PointerButton { pos: out, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    b.v.frame(vec![Event::PointerMoved(out)]);
    b.v.frame(vec![button(true)]);
    b.v.frame(vec![button(false)]);
    let wheel = Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: vec2(0.0, -40.0),
        phase: TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers: Modifiers::NONE,
    };
    b.v.frame(vec![wheel]);
    b.v.settle(0.5);
    let events = b.calls.input_events();
    assert_eq!(events.len(), relayed, "nothing outside the view is relayed: {:?}", events.iter().skip(relayed).collect::<Vec<_>>());
}

// covers: E7-6.3
#[test]
fn playground_focused_view_relays_listed_keys_and_typed_text() {
    let mut b = Bench::ready("keys", "en");
    b.v.click(b.v.zone().center());
    b.v.settle(0.3);
    let base = b.calls.input_events().len();
    b.v.press(Key::Enter, Modifiers::NONE);
    b.v.press(Key::ArrowLeft, Modifiers::SHIFT);
    b.v.press(Key::Backspace, Modifiers::NONE);
    b.v.press(Key::Space, Modifiers::NONE);
    b.v.press(Key::F5, Modifiers::NONE);
    b.v.press(Key::A, Modifiers::NONE);
    text(&mut b, "é");
    let events: Vec<Value> = b.calls.input_events().into_iter().skip(base).collect();

    assert_eq!(key_names(&events), ["Enter", "ArrowLeft", "Backspace", "Space"], "{events:?}");
    assert!(key_names(&events).iter().all(|k| KEYS.contains(&k.as_str())));
    let shifted = of_type(&events, "key").into_iter().find(|e| e["key"] == "ArrowLeft").unwrap();
    assert_eq!(shifted["mods"], serde_json::json!(["shift"]));
    assert!(typed(&events).iter().any(|t| t == "é"), "the typed text goes over as text: {events:?}");
}

// covers: E7-6.3
#[test]
fn playground_tab_and_shift_tab_leave_the_view() {
    let mut b = Bench::ready("tab", "en");
    for (modifiers, word) in [(Modifiers::NONE, "z"), (Modifiers::SHIFT, "y")] {
        b.v.click(b.v.zone().center());
        b.v.settle(0.3);
        let base = b.calls.input_events().len();
        b.v.press(Key::Tab, modifiers);
        text(&mut b, word);
        let events: Vec<Value> = b.calls.input_events().into_iter().skip(base).collect();
        assert!(!key_names(&events).iter().any(|k| k == "Tab"), "Tab is never relayed: {events:?}");
        assert!(typed(&events).is_empty(), "the view lost the focus: {events:?}");
    }
}

// covers: E7-6.3
#[test]
fn playground_shortcuts_stand_aside_while_the_view_has_the_focus_and_come_back_after() {
    let mut b = Bench::ready("shortcuts", "en");
    b.v.click(b.v.zone().center());
    b.v.settle(0.3);
    let base = b.calls.input_events().len();
    b.v.frame(vec![Event::Text("r".to_string())]);
    b.v.press(Key::R, Modifiers::NONE);
    b.v.settle(0.3);
    assert_eq!(b.v.app.session.tool.tool, ToolId::Move, "R does not pick the rectangle tool while the view has the focus");
    let events: Vec<Value> = b.calls.input_events().into_iter().skip(base).collect();
    assert!(typed(&events).iter().any(|t| t == "r"), "the page got the letter: {events:?}");

    b.v.click_node(Role::Tab, DOC_TAB);
    b.v.settle(0.3);
    b.v.press(Key::R, Modifiers::NONE);
    assert_eq!(b.v.app.session.tool.tool, ToolId::Rectangle, "the shortcut works again once the view lost the focus");
}
