use egui::{Event, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, Rect};
use serde_json::{Value, json};

pub const MAX_EVENTS: usize = 64;
const MAX_HELD: usize = 256;
const MAX_WHEEL: f64 = 4000.0;
const MAX_TEXT_CHARS: usize = 256;
const LINE_HEIGHT: f32 = 40.0;
const LEFT: usize = 0;
const MIDDLE: usize = 1;
const RIGHT: usize = 2;

/// What one frame gives the page view to relay.
pub struct Frame<'a> {
    pub events: &'a [Event],
    pub pointer: Option<Pos2>,
    pub zone: Rect,
    pub hovered: bool,
    pub focused: bool,
    pub escape_taken: bool,
}

/// What a frame's input comes to for the page: the events of `playground.input`, and a Tab that
/// leaves the view (`1` forward, `-1` back).
#[derive(Default)]
pub struct Relayed {
    pub events: Vec<Value>,
    pub leave: Option<i32>,
}

/// Translates egui's input over the page view into the closed set of events the page accepts. It
/// remembers the buttons it pressed on the page so each one is released there, wherever the
/// pointer ends up.
#[derive(Default)]
pub struct Relay {
    held: [bool; 3],
    outbox: Vec<Value>,
    refusal: Option<String>,
}

impl Relay {
    pub fn queue(&mut self, events: Vec<Value>) {
        for event in events {
            match self.outbox.last_mut() {
                Some(last) if is_move(last) && is_move(&event) => *last = event,
                _ => self.outbox.push(event),
            }
        }
        self.forget_oldest();
    }

    fn forget_oldest(&mut self) {
        if self.outbox.len() > MAX_HELD {
            self.outbox.drain(..self.outbox.len() - MAX_HELD);
        }
    }

    pub fn take(&mut self, room: usize) -> Vec<Value> {
        let count = self.outbox.len().min(room).min(MAX_EVENTS);
        self.outbox.drain(..count).collect()
    }

    pub fn put_back(&mut self, mut batch: Vec<Value>) {
        batch.append(&mut self.outbox);
        self.outbox = batch;
        self.forget_oldest();
    }

    pub fn waiting(&self) -> bool {
        !self.outbox.is_empty()
    }

    pub fn first_refusal(&mut self, reason: &str) -> bool {
        let is_new = self.refusal.as_deref() != Some(reason);
        self.refusal = Some(reason.to_string());
        is_new
    }

    pub fn accepted(&mut self) {
        self.refusal = None;
    }

    pub fn translate(&mut self, frame: &Frame) -> Relayed {
        let mut out = Relayed::default();
        let mut typing = frame.focused;
        for event in frame.events {
            match event {
                Event::PointerMoved(at) if self.over(frame, *at) => replace_move(&mut out.events, point("move", frame.zone, *at)),
                Event::PointerButton { pos, button, pressed, .. } => self.button(frame, *pos, *button, *pressed, &mut out.events),
                Event::MouseWheel { unit, delta, .. } => {
                    if let Some(at) = frame.pointer.filter(|at| inside(frame, *at)) {
                        out.events.extend(wheel(frame.zone, at, *unit, *delta));
                    }
                }
                Event::Key { key: Key::Tab, pressed: true, modifiers, .. } if typing => {
                    out.leave = Some(if modifiers.shift { -1 } else { 1 });
                    typing = false;
                }
                Event::Key { key: Key::Escape, pressed: true, .. } if frame.escape_taken => {}
                Event::Key { key, pressed: true, modifiers, .. } if typing => out.events.extend(key_event(*key, *modifiers)),
                Event::Text(text) if typing => out.events.extend(text_event(text)),
                _ => {}
            }
        }
        if out.events.len() > MAX_EVENTS {
            out.events.drain(..out.events.len() - MAX_EVENTS);
        }
        out
    }

    fn over(&self, frame: &Frame, at: Pos2) -> bool {
        inside(frame, at) || self.held.iter().any(|held| *held)
    }

    fn button(&mut self, frame: &Frame, at: Pos2, button: PointerButton, pressed: bool, events: &mut Vec<Value>) {
        let Some((slot, name)) = button_of(button) else { return };
        let Some(held) = self.held.get_mut(slot) else { return };
        if pressed && inside(frame, at) {
            *held = true;
            events.push(button_event("down", name, frame.zone, at));
        } else if !pressed && *held {
            *held = false;
            events.push(button_event("up", name, frame.zone, at));
        }
    }
}

fn inside(frame: &Frame, at: Pos2) -> bool {
    frame.hovered && frame.zone.contains(at)
}

fn button_of(button: PointerButton) -> Option<(usize, &'static str)> {
    match button {
        PointerButton::Primary => Some((LEFT, "left")),
        PointerButton::Middle => Some((MIDDLE, "middle")),
        PointerButton::Secondary => Some((RIGHT, "right")),
        PointerButton::Extra1 | PointerButton::Extra2 => None,
    }
}

fn is_move(event: &Value) -> bool {
    event["type"] == "move"
}

fn replace_move(events: &mut Vec<Value>, event: Value) {
    if let Some(earlier) = events.iter().position(is_move) {
        events.remove(earlier);
    }
    events.push(event);
}

fn hundredth(value: f32) -> f64 {
    (f64::from(value) * 100.0).round() / 100.0
}

fn page_point(zone: Rect, at: Pos2) -> (f64, f64) {
    let last = |extent: f32| (extent.round() - 1.0).max(0.0);
    (hundredth((at.x - zone.min.x).clamp(0.0, last(zone.width()))), hundredth((at.y - zone.min.y).clamp(0.0, last(zone.height()))))
}

fn point(kind: &str, zone: Rect, at: Pos2) -> Value {
    let (x, y) = page_point(zone, at);
    json!({"type": kind, "x": x, "y": y})
}

fn button_event(kind: &str, button: &str, zone: Rect, at: Pos2) -> Value {
    let (x, y) = page_point(zone, at);
    json!({"type": kind, "x": x, "y": y, "button": button})
}

fn wheel(zone: Rect, at: Pos2, unit: MouseWheelUnit, delta: egui::Vec2) -> Option<Value> {
    let scale = match unit {
        MouseWheelUnit::Point => 1.0,
        MouseWheelUnit::Line => LINE_HEIGHT,
        MouseWheelUnit::Page => zone.height(),
    };
    let reach = |amount: f32| (0.0 - f64::from(amount * scale)).clamp(-MAX_WHEEL, MAX_WHEEL);
    let (dx, dy) = (reach(delta.x), reach(delta.y));
    if !(dx.is_finite() && dy.is_finite()) || (dx == 0.0 && dy == 0.0) {
        return None;
    }
    let (x, y) = page_point(zone, at);
    Some(json!({"type": "wheel", "x": x, "y": y, "dx": dx, "dy": dy}))
}

fn key_name(key: Key) -> Option<&'static str> {
    match key {
        Key::Enter => Some("Enter"),
        Key::Escape => Some("Escape"),
        Key::Backspace => Some("Backspace"),
        Key::Delete => Some("Delete"),
        Key::ArrowUp => Some("ArrowUp"),
        Key::ArrowDown => Some("ArrowDown"),
        Key::ArrowLeft => Some("ArrowLeft"),
        Key::ArrowRight => Some("ArrowRight"),
        Key::Home => Some("Home"),
        Key::End => Some("End"),
        Key::PageUp => Some("PageUp"),
        Key::PageDown => Some("PageDown"),
        Key::Space => Some("Space"),
        _ => None,
    }
}

fn key_event(key: Key, modifiers: Modifiers) -> Option<Value> {
    let name = key_name(key)?;
    let held: Vec<&str> = [(modifiers.shift, "shift"), (modifiers.ctrl || modifiers.command, "ctrl"), (modifiers.alt, "alt")]
        .into_iter()
        .filter_map(|(on, word)| on.then_some(word))
        .collect();
    Some(if held.is_empty() { json!({"type": "key", "key": name}) } else { json!({"type": "key", "key": name, "mods": held}) })
}

fn text_event(text: &str) -> Option<Value> {
    let typed: String = text.chars().filter(|c| !c.is_control()).take(MAX_TEXT_CHARS).collect();
    (!typed.is_empty() && typed != " ").then(|| json!({"type": "text", "text": typed}))
}
