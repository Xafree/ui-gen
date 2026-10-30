//! The Playground tab's top bar and settings bar (E7-7.1 to E7-7.4, and the code window, the
//! keyboard and the catalogs of E7-7.6), driven through egui headless on the harness of
//! `playground_view_tests.rs`. The service is a scripted fake backend; what the interface asks of the
//! engine is read from the session journal, what it shows from the accessibility tree and the painted
//! text. Control names (the English keys of the catalogs):
//! buttons "Undo setting", "Redo setting", "Revert settings", "Random settings", "Light mode",
//! "Dark mode", "Save theme", "Show code"; sliders "Accent" (OKLCH hue of the effective accent,
//! degrees), "Base hue" (degrees), "Base tint" (0 to 1); combo boxes "Radius", "Radius shape", "Font",
//! "Theme", whose value is the choice shown and whose items are menu items.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, Mutex};

use egui::accesskit::{Node, Role, Toggled};
use egui::{Event, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, TouchPhase, pos2, vec2};
use figcraft_color::{Color, Oklch};
use figcraft_doc::{Collection, Document, Mode, Variable, VariableType, VariableValue};
use figcraft_engine::capture::{CaptureBackend, CaptureError, CaptureErrorKind, CaptureJob, JobId};
use figcraft_engine::playground::Status;
use serde_json::{Value, json};

use crate::i18n::{self, Lang, Text};
use crate::playground_view_tests::{Bench, Library, Outcome, RED, View, jpeg_frame};

const BUTTONS: [&str; 8] =
    ["Undo setting", "Redo setting", "Revert settings", "Random settings", "Light mode", "Dark mode", "Save theme", "Show code"];
const SLIDERS: [&str; 3] = ["Accent", "Base hue", "Base tint"];
const COMBOS: [&str; 4] = ["Radius", "Radius shape", "Font", "Theme"];
const GENERIC_FONTS: [&str; 4] = ["system-ui", "serif", "monospace", "ui-rounded"];
const RADII: [(&str, &str); 5] = [("None", "zero"), ("Small", "small"), ("Medium", "medium"), ("Large", "large"), ("Full", "full")];
const SHAPES: [(&str, &str); 3] = [("Round", "round"), ("Squircle", "squircle"), ("Bevel", "bevel")];

pub(crate) struct Script {
    submitted: Vec<CaptureJob>,
    ready: Vec<(JobId, Outcome)>,
    held: Vec<(JobId, Outcome)>,
    hold_open: bool,
    open_error: Option<String>,
    shape: bool,
}

pub(crate) type Shared = Arc<Mutex<Script>>;

struct Fake(Shared);

impl CaptureBackend for Fake {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let mut guard = self.0.lock().unwrap();
        let script = &mut *guard;
        let id = u64::try_from(script.submitted.len() + 1).unwrap();
        if let CaptureJob::Playground { method, params, .. } = &job {
            let outcome = match method.as_str() {
                "playground.open" => match &script.open_error {
                    Some(message) => Err(CaptureError { kind: CaptureErrorKind::Render, message: message.clone() }),
                    None => Ok(json!({"defined": ["ui-button"], "missing": [], "errors": [], "shape": script.shape})),
                },
                "playground.frame" if params["after"].as_u64().unwrap_or(0) == 0 => jpeg_frame(1, 160, 100, RED),
                "playground.frame" => Ok(json!({"seq": params["after"], "changed": false})),
                _ => Ok(json!({})),
            };
            if method == "playground.open" && script.hold_open {
                script.held.push((id, outcome));
            } else {
                script.ready.push((id, outcome));
            }
        }
        script.submitted.push(job);
        id
    }

    fn poll(&mut self) -> Vec<(JobId, Outcome)> {
        std::mem::take(&mut self.0.lock().unwrap().ready)
    }

    fn shutdown(&mut self) {}
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tokens {
    AccentOnly,
    Rich,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Start {
    Ready,
    Opening,
    Failing,
}

pub(crate) struct Studio {
    pub(crate) b: Bench,
    pub(crate) script: Shared,
}

fn color(hex: &str) -> VariableValue {
    VariableValue::Color(Color::from_hex(hex).unwrap())
}

fn variable(id: &str, name: &str, collection: &str, kind: VariableType, values: [(&str, VariableValue); 2], scopes: Option<&str>) -> Variable {
    Variable {
        id: id.to_string(),
        name: name.to_string(),
        collection: collection.to_string(),
        kind,
        values: values.into_iter().map(|(mode, value)| (mode.to_string(), value)).collect(),
        description: String::new(),
        scopes: scopes.map(|s| vec![s.to_string()]),
        hidden_from_publishing: false,
        extends: None,
        overrides: BTreeSet::new(),
        code_syntax: Default::default(),
    }
}

fn themed(id: &str, name: &str, collection: &str, modes: [&str; 2], light_dark: [VariableValue; 2], scopes: Option<&str>) -> Variable {
    let kind = match &light_dark[0] {
        VariableValue::Color(_) => VariableType::Color,
        VariableValue::Number(_) => VariableType::Number,
        _ => VariableType::String,
    };
    let [light, dark] = light_dark;
    variable(id, name, collection, kind, [(modes[0], light), (modes[1], dark)], scopes)
}

fn collection(id: &str, name: &str, modes: [&str; 2]) -> Collection {
    Collection {
        id: id.to_string(),
        name: name.to_string(),
        modes: vec![Mode { id: modes[0].to_string(), name: "Light".to_string() }, Mode { id: modes[1].to_string(), name: "Dark".to_string() }],
        extends: None,
    }
}

fn enrich(doc: &mut Document) {
    let theme = ["m-light", "m-dark"];
    let alt = ["m-alt-light", "m-alt-dark"];
    let plain = ["m-plain-light", "m-plain-dark"];
    let text = |s: &str| VariableValue::String(s.to_string());
    doc.variables.collections.push(collection("c-alt", "Alt", alt));
    doc.variables.collections.push(collection("c-plain", "Plain", plain));
    doc.variables.variables.extend([
        themed("v:surface", "color/surface", "c-theme", theme, [color("#F4EEE8"), color("#1E1A16")], None),
        themed("v:radius", "radius/md", "c-theme", theme, [VariableValue::Number(8.0), VariableValue::Number(8.0)], Some("CORNER_RADIUS")),
        themed("v:font", "font/family", "c-theme", theme, [text("Inter"), text("Inter")], Some("FONT_FAMILY")),
        themed("v:alt-primary", "color/primary", "c-alt", alt, [color("#2563EB"), color("#60A5FA")], None),
        themed("v:alt-surface", "color/surface", "c-alt", alt, [color("#E8EEF8"), color("#161A20")], None),
        themed("v:alt-radius", "radius/md", "c-alt", alt, [VariableValue::Number(4.0), VariableValue::Number(4.0)], Some("CORNER_RADIUS")),
        themed("v:alt-font", "font/family", "c-alt", alt, [text("Georgia"), text("Georgia")], Some("FONT_FAMILY")),
        themed("v:plain-surface", "color/surface", "c-plain", plain, [color("#ECECEC"), color("#202020")], None),
    ]);
}

pub(crate) fn studio(tag: &str, language: &str, tokens: Tokens, start: Start, shape: bool) -> Studio {
    let mut b = Bench::without_backend(tag, language, Library::Full);
    let script = Arc::new(Mutex::new(Script {
        submitted: Vec::new(),
        ready: Vec::new(),
        held: Vec::new(),
        hold_open: start == Start::Opening,
        open_error: (start == Start::Failing).then(|| "bundle failed in components/button.ts: unexpected token".to_string()),
        shape,
    }));
    b.v.app.session.set_capture_backend(Box::new(Fake(script.clone())));
    if tokens == Tokens::Rich {
        enrich(b.v.app.session.doc_mut().unwrap());
    }
    assert!(b.v.app.run("playground.open", json!({})).is_some(), "the menu command opens the playground");
    b.v.settle(1.0);
    let expected = match start {
        Start::Ready => Status::Ready,
        Start::Opening => Status::Opening,
        Start::Failing => Status::Failed("bundle failed in components/button.ts: unexpected token".to_string()),
    };
    if start != Start::Failing {
        assert_eq!(*b.v.app.session.playground_status(), expected);
    }
    Studio { b, script }
}

pub(crate) fn ready(tag: &str, tokens: Tokens) -> Studio {
    studio(tag, "en", tokens, Start::Ready, true)
}

pub(crate) fn hue_gap(a: f64, b: f64) -> f64 {
    ((a - b + 540.0).rem_euclid(360.0) - 180.0).abs()
}

pub(crate) fn hue_of(hex: &str) -> f64 {
    f64::from(Oklch::from_color(Color::from_hex(hex).unwrap()).h)
}

fn pointer(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

impl Studio {
    pub(crate) fn state(&mut self) -> Value {
        self.b.v.app.session.execute("playground.state", &json!({})).unwrap()
    }

    pub(crate) fn calls(&self, command: &str) -> Vec<Value> {
        self.b.v.app.session.journal.iter().filter(|(id, _)| id == command).map(|(_, params)| params.clone()).collect()
    }

    pub(crate) fn jobs(&self, method: &str) -> usize {
        let script = self.script.lock().unwrap();
        script.submitted.iter().filter(|job| matches!(job, CaptureJob::Playground { method: m, .. } if m == method)).count()
    }

    pub(crate) fn release_open(&mut self) {
        let mut script = self.script.lock().unwrap();
        script.hold_open = false;
        let held = std::mem::take(&mut script.held);
        script.ready.extend(held);
    }

    pub(crate) fn run(&mut self, command: &str, params: Value) -> Value {
        let result = self.b.v.app.run(command, params);
        self.b.v.settle(0.3);
        result.unwrap_or_else(|| panic!("{command} was refused"))
    }

    pub(crate) fn node(&self, role: Role, label: &str) -> &Node {
        assert!(self.b.v.has(role, label), "no {role:?} named {label:?} on screen");
        self.b.v.node(role, label).unwrap()
    }

    pub(crate) fn press(&mut self, label: &str) {
        assert!(self.b.v.has(Role::Button, label), "no button named {label:?} on screen");
        self.b.v.click_node(Role::Button, label);
        self.b.v.settle(0.3);
    }

    pub(crate) fn disabled(&self, role: Role, label: &str) -> bool {
        self.node(role, label).is_disabled()
    }

    pub(crate) fn painted(&self, needle: &str) -> bool {
        let needle = needle.to_lowercase();
        self.b.v.texts().iter().any(|text| text.to_lowercase().contains(&needle))
    }

    fn slider(&self, label: &str) -> (Rect, f64) {
        let node = self.node(Role::Slider, label);
        (View::bounds(node), node.numeric_value().unwrap_or(f64::NAN))
    }

    fn drag_start(&mut self, label: &str, fraction: f32) {
        let (rect, _) = self.slider(label);
        let at = pos2(rect.min.x + rect.width() * fraction, rect.center().y);
        self.b.v.frame(vec![Event::PointerMoved(at), pointer(at, true)]);
    }

    fn drag_to(&mut self, label: &str, fraction: f32) {
        let (rect, _) = self.slider(label);
        let at = pos2(rect.min.x + rect.width() * fraction, rect.center().y);
        self.b.v.frame(vec![Event::PointerMoved(at)]);
    }

    fn drag_end(&mut self, label: &str, fraction: f32) {
        let (rect, _) = self.slider(label);
        let at = pos2(rect.min.x + rect.width() * fraction, rect.center().y);
        self.b.v.frame(vec![pointer(at, false)]);
        self.b.v.settle(0.3);
    }

    fn item(&self, label: &str) -> Option<Rect> {
        [Role::MenuItem, Role::MenuItemCheckBox].into_iter().find_map(|role| self.b.v.node(role, label).map(View::bounds))
    }

    fn open_menu(&mut self, combo: &str) {
        assert!(self.b.v.has(Role::ComboBox, combo), "no combo box named {combo:?} on screen");
        self.b.v.click_node(Role::ComboBox, combo);
        self.b.v.settle(0.3);
    }

    pub(crate) fn pick(&mut self, combo: &str, label: &str) {
        self.open_menu(combo);
        let at = self.item(label).unwrap_or_else(|| panic!("the {combo:?} menu offers no {label:?}"));
        self.b.v.click(at.center());
        self.b.v.settle(0.3);
    }

    fn pick_far(&mut self, combo: &str, near: &str, label: &str) -> bool {
        self.open_menu(combo);
        let anchor = self.item(near).unwrap_or_else(|| panic!("the {combo:?} menu offers no {near:?}")).center();
        for _ in 0..120 {
            if let Some(rect) = self.item(label)
                && Rect::from_min_size(Pos2::ZERO, self.b.v.screen).contains(rect.center())
            {
                self.b.v.click(rect.center());
                self.b.v.settle(0.3);
                return true;
            }
            let wheel = Event::MouseWheel {
                unit: MouseWheelUnit::Line,
                delta: vec2(0.0, -4.0),
                phase: TouchPhase::Move,
                source: egui::MouseWheelSource::Unknown,
                modifiers: Modifiers::NONE,
            };
            self.b.v.frame(vec![Event::PointerMoved(anchor), wheel]);
        }
        false
    }

    pub(crate) fn tab_to(&mut self, name: &str) {
        for _ in 0..200 {
            if crate::a11y::focused_name(&self.b.v.ctx) == name {
                return;
            }
            self.b.v.press(Key::Tab, Modifiers::NONE);
        }
        panic!("the keyboard never reaches {name:?}");
    }

    pub(crate) fn copied_by(&mut self, label: &str) -> Vec<String> {
        let at = View::bounds(self.node(Role::Button, label)).center();
        self.b.v.frame(vec![Event::PointerMoved(at), pointer(at, true)]);
        let time = self.b.v.ctx.input(|i| i.time) + 1.0 / 60.0;
        self.b.v.app.clock.set_test_now(Some(time));
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.b.v.screen)),
            time: Some(time),
            events: vec![pointer(at, false)],
            ..Default::default()
        };
        let app = &mut self.b.v.app;
        let out = self.b.v.ctx.run_ui(input, |ui| app.ui(ui));
        let copied = out
            .platform_output
            .commands
            .iter()
            .filter_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
            .collect();
        self.b.v.settle(0.3);
        copied
    }
}

// covers: E7-7.1
#[test]
fn playground_accent_slider_previews_while_dragged_and_commits_once_on_release() {
    let mut s = ready("drag", Tokens::AccentOnly);
    let (_, before) = s.slider("Accent");
    let (sets, applies) = (s.calls("playground.set").len(), s.jobs("playground.apply"));

    s.drag_start("Accent", 0.2);
    s.drag_to("Accent", 0.5);
    s.drag_to("Accent", 0.8);
    let during: Vec<Value> = s.calls("playground.set").split_off(sets);
    assert!(!during.is_empty(), "the page follows the drag: playground.set is called before the release");
    assert!(during.iter().all(|p| p["commit"] == json!(false)), "every call during the drag is commit: false: {during:?}");
    assert!(s.jobs("playground.apply") > applies, "the page gets the overrides while dragging");
    let state = s.state();
    assert_eq!(state["canUndo"], json!(false), "no history step before the release");
    assert!(state["settings"]["accent"].is_string(), "{state}");

    s.drag_end("Accent", 0.8);
    let last = s.calls("playground.set").pop().expect("a playground.set at the release");
    assert_ne!(last["commit"], json!(false), "the release commits: {last}");
    let state = s.state();
    assert_eq!(state["canUndo"], json!(true));
    let (_, after) = s.slider("Accent");
    assert!(hue_gap(after, before) > 20.0, "the knob moved: {before} to {after}");
    let hue = hue_of(state["settings"]["accent"].as_str().unwrap());
    assert!(hue_gap(hue, after) <= 6.0, "the accent has the hue of the knob: {hue} against {after}");

    s.press("Undo setting");
    let state = s.state();
    assert_eq!(s.calls("playground.undo").len(), 1);
    assert_eq!((state["settings"]["accent"].clone(), state["canUndo"].clone()), (Value::Null, json!(false)), "the drag was a single history step");
    s.press("Redo setting");
    let state = s.state();
    assert_eq!((s.calls("playground.redo").len(), state["canRedo"].clone()), (1, json!(false)));
    assert!(state["settings"]["accent"].is_string());
}

// covers: E7-7.1
#[test]
fn playground_undo_redo_revert_and_random_are_greyed_when_they_can_do_nothing_and_call_their_commands() {
    let mut s = ready("history", Tokens::AccentOnly);
    for name in ["Undo setting", "Redo setting", "Revert settings"] {
        assert!(s.disabled(Role::Button, name), "{name:?} is greyed with an empty history");
    }
    assert!(!s.disabled(Role::Button, "Random settings"), "Random settings always applies");
    s.b.v.click_node(Role::Button, "Undo setting");
    s.b.v.settle(0.3);
    assert!(s.calls("playground.undo").is_empty(), "a greyed button sends nothing");

    s.run("playground.set", json!({"accent": "#2563eb"}));
    assert!(!s.disabled(Role::Button, "Undo setting") && !s.disabled(Role::Button, "Revert settings"));
    assert!(s.disabled(Role::Button, "Redo setting"));

    s.press("Revert settings");
    let state = s.state();
    assert_eq!(s.calls("playground.revert").len(), 1);
    assert_eq!((state["settings"]["accent"].clone(), state["canUndo"].clone(), state["canRedo"].clone()), (Value::Null, json!(true), json!(false)));
    assert!(s.disabled(Role::Button, "Revert settings"), "nothing left to revert");

    s.press("Random settings");
    assert_eq!(s.calls("playground.random").len(), 1);
    assert!(s.state()["settings"]["accent"].is_string(), "the theme has an accent token: the draw sets it");
    assert!(!s.disabled(Role::Button, "Revert settings"));
}

// covers: E7-7.2
#[test]
fn playground_radius_and_radius_shape_menus_send_each_choice() {
    let mut s = ready("menus-radius", Tokens::Rich);
    for (label, id) in RADII {
        s.pick("Radius", label);
        let state = s.state();
        assert_eq!(state["settings"]["radius"], json!(id), "{label}");
        assert_eq!(s.calls("playground.set").last().unwrap()["radius"], json!(id));
        assert_eq!(s.node(Role::ComboBox, "Radius").value(), Some(label), "the menu shows its choice");
    }
    for (label, id) in SHAPES {
        s.pick("Radius shape", label);
        assert_eq!(s.state()["settings"]["radiusShape"], json!(id), "{label}");
        assert_eq!(s.calls("playground.set").last().unwrap()["radiusShape"], json!(id));
    }
}

// covers: E7-7.2
#[test]
fn playground_font_menu_offers_the_generic_families_and_the_installed_ones() {
    let mut s = ready("menus-font", Tokens::Rich);
    for family in GENERIC_FONTS {
        s.pick("Font", family);
        assert_eq!(s.state()["settings"]["font"], json!(family));
        assert_eq!(s.calls("playground.set").last().unwrap()["font"], json!(family));
    }
    let valid = |name: &String| (1..=64).contains(&name.chars().count()) && !name.contains([';', '{', '}', '(', ')', '\\', '"', '\n']);
    let mut installed = figcraft_text::family_names();
    installed.retain(valid);
    installed.sort();
    if let Some(family) = installed.first() {
        assert!(s.pick_far("Font", "serif", family), "the Font menu offers the installed family {family:?}");
        assert_eq!(s.state()["settings"]["font"], json!(family));
    }
}

// covers: E7-7.2
#[test]
fn playground_theme_menu_lists_the_collections_of_the_document_and_sends_the_choice() {
    let mut s = ready("menus-theme", Tokens::Rich);
    s.open_menu("Theme");
    for name in ["Theme", "Alt", "Plain"] {
        assert!(s.item(name).is_some(), "the Theme menu lists the collection {name:?}");
    }
    s.b.v.press(Key::Escape, Modifiers::NONE);
    s.pick("Theme", "Alt");
    assert_eq!(s.state()["settings"]["theme"], json!("Alt"));
    assert_eq!(s.calls("playground.set").last().unwrap()["theme"], json!("Alt"));
    assert_eq!(s.node(Role::ComboBox, "Theme").value(), Some("Alt"));
}

// covers: E7-7.2
#[test]
fn playground_radius_shape_is_greyed_when_the_service_does_not_know_corner_shape() {
    let mut known = ready("shape-known", Tokens::Rich);
    assert!(!known.disabled(Role::ComboBox, "Radius shape"));
    let mut unknown = studio("shape-unknown", "en", Tokens::Rich, Start::Ready, false);
    assert!(unknown.disabled(Role::ComboBox, "Radius shape"), "shape: false greys the menu");
    unknown.b.v.click_node(Role::ComboBox, "Radius shape");
    unknown.b.v.settle(0.3);
    assert!(unknown.item("Squircle").is_none(), "a greyed menu does not open");
    assert!(unknown.calls("playground.set").is_empty());
    assert_eq!(known.state()["settings"]["radiusShape"], json!("round"));
}

// covers: E7-7.3
#[test]
fn playground_mode_selector_sends_the_mode_and_reflects_the_setting() {
    let mut s = ready("mode", Tokens::Rich);
    let on = |s: &Studio, label: &str| s.node(Role::Button, label).toggled() == Some(Toggled::True);
    assert!(on(&s, "Light mode") && !on(&s, "Dark mode"), "light is the default");
    s.press("Dark mode");
    assert_eq!(s.state()["settings"]["mode"], json!("dark"));
    assert_eq!(s.calls("playground.set").last().unwrap()["mode"], json!("dark"));
    assert!(on(&s, "Dark mode") && !on(&s, "Light mode"));
    s.run("playground.set", json!({"mode": "light"}));
    assert!(on(&s, "Light mode") && !on(&s, "Dark mode"), "the selector follows the setting changed elsewhere");
}

// covers: E7-7.3
#[test]
fn playground_changing_the_theme_puts_the_sliders_on_the_values_of_the_new_theme() {
    let mut s = ready("theme-sliders", Tokens::Rich);
    let (_, accent_before) = s.slider("Accent");
    let (_, base_before) = s.slider("Base hue");
    let first = s.state()["effective"].clone();
    assert!(hue_gap(accent_before, hue_of(first["accent"].as_str().unwrap())) <= 2.0, "the knob shows the effective accent: {first}");

    s.pick("Theme", "Alt");
    let now = s.state()["effective"].clone();
    let (_, accent) = s.slider("Accent");
    let (_, base) = s.slider("Base hue");
    let (_, tint) = s.slider("Base tint");
    let expected = hue_of(now["accent"].as_str().unwrap());
    assert!(hue_gap(expected, accent_before) > 30.0, "the two themes have different accents");
    assert!(hue_gap(accent, expected) <= 2.0, "Accent shows the new theme's {expected}, not {accent}");
    assert!(hue_gap(base, now["base"]["hue"].as_f64().unwrap()) <= 2.0, "Base hue shows the new theme's value");
    assert!(hue_gap(base, base_before) > 10.0, "the two themes have different bases");
    assert!((tint - now["base"]["tint"].as_f64().unwrap()).abs() <= 0.02, "Base tint shows the new theme's value: {tint}");
}

// covers: E7-7.4
#[test]
fn playground_a_setting_without_a_token_is_greyed_with_its_reason_and_sends_nothing() {
    let mut s = ready("no-target", Tokens::AccentOnly);
    assert!(!s.disabled(Role::Slider, "Accent"), "the theme has an accent token");
    for (role, name, reason) in [
        (Role::Slider, "Base hue", "No base token in this theme"),
        (Role::Slider, "Base tint", "No base token in this theme"),
        (Role::ComboBox, "Font", "No font token in this theme"),
        (Role::ComboBox, "Radius", "No radius token in this theme"),
    ] {
        assert!(s.disabled(role, name), "{name:?} is greyed");
        let at = View::bounds(s.node(role, name)).center();
        s.b.v.move_to(at);
        s.b.v.settle(1.0);
        assert!(s.painted(reason), "the tooltip of {name:?} says {reason:?}: {:?}", s.b.v.texts());
        s.b.v.move_to(pos2(2.0, 2.0));
        s.b.v.settle(0.3);
    }
    let rect = View::bounds(s.node(Role::Slider, "Base hue"));
    s.b.v.frame(vec![Event::PointerMoved(rect.center()), pointer(rect.center(), true)]);
    s.b.v.frame(vec![pointer(rect.center(), false)]);
    s.b.v.click_node(Role::ComboBox, "Radius");
    s.b.v.settle(0.3);
    assert!(s.item("Large").is_none(), "a greyed menu does not open");
    assert!(s.calls("playground.set").is_empty(), "nothing was sent: {:?}", s.calls("playground.set"));
}

// covers: E7-7.4
#[test]
fn playground_the_reason_is_told_in_french_for_a_theme_without_accent_token() {
    let mut s = studio("no-accent-fr", "fr", Tokens::Rich, Start::Ready, true);
    s.run("playground.set", json!({"theme": "Plain"}));
    assert!(s.disabled(Role::Slider, "Accent"), "the Plain theme has no accent token");
    assert!(!s.disabled(Role::Slider, "Teinte de base"), "it has a base token");
    let at = View::bounds(s.node(Role::Slider, "Accent")).center();
    s.b.v.move_to(at);
    s.b.v.settle(1.0);
    assert!(s.painted("aucun jeton d'accent dans ce thème"), "{:?}", s.b.v.texts());
}

fn assert_inert(s: &mut Studio) {
    let before: usize =
        ["set", "undo", "redo", "revert", "random", "saveTheme", "code"].iter().map(|c| s.calls(&format!("playground.{c}")).len()).sum();
    for (role, names) in [(Role::Button, BUTTONS.as_slice()), (Role::Slider, SLIDERS.as_slice()), (Role::ComboBox, COMBOS.as_slice())] {
        for name in names {
            if s.b.v.has(role, name) {
                assert!(s.disabled(role, name), "{name:?} stays greyed while the page is not ready");
                let at = View::bounds(s.node(role, name)).center();
                s.b.v.click(at);
            }
        }
    }
    let after: usize =
        ["set", "undo", "redo", "revert", "random", "saveTheme", "code"].iter().map(|c| s.calls(&format!("playground.{c}")).len()).sum();
    assert_eq!(before, after, "nothing is sent");
}

// covers: E7-7.4
#[test]
fn playground_bars_are_inert_while_the_page_loads_and_after_it_failed() {
    let mut loading = studio("inert-loading", "en", Tokens::Rich, Start::Opening, true);
    assert_inert(&mut loading);
    loading.release_open();
    loading.b.v.settle(1.0);
    assert!(!loading.disabled(Role::Button, "Random settings"), "the bars wake up once the page is ready");

    let mut failed = studio("inert-failed", "en", Tokens::Rich, Start::Failing, true);
    failed.b.v.settle(0.5);
    assert!(matches!(failed.b.v.app.session.playground_status(), Status::Failed(_)));
    assert_inert(&mut failed);
}

// covers: E7-7.6
#[test]
fn playground_code_window_shows_the_overrides_and_the_template_read_only_and_copies_them() {
    let mut s = ready("code", Tokens::AccentOnly);
    s.run("playground.set", json!({"accent": "#2563eb"}));
    let code = s.run("playground.code", json!({}));
    let first_line = |text: &str| text.lines().map(str::trim).find(|l| l.len() >= 8).unwrap().to_string();
    let (overrides, template) = (first_line(code["overrides"].as_str().unwrap()), first_line(code["template"].as_str().unwrap()));
    assert!(!s.b.v.says(&overrides), "the window is closed at first");

    s.press("Show code");
    assert!(s.b.v.says(&overrides), "the overrides are shown: {overrides:?}");
    assert!(s.b.v.says(&template), "the template is shown: {template:?}");
    let fields = [Role::TextInput, Role::MultilineTextInput].into_iter().flat_map(|role| s.b.v.with_role(role));
    assert!(fields.filter(|n| n.value().is_some_and(|v| v.contains(&overrides))).all(Node::is_read_only), "the code is read only");
    assert_eq!(s.copied_by("Copy overrides"), vec![code["overrides"].as_str().unwrap().to_string()]);
    assert_eq!(s.copied_by("Copy template"), vec![code["template"].as_str().unwrap().to_string()]);
    let revision = s.b.v.app.session.docs[s.b.v.app.session.active].revision;

    let relayed = s.calls("playground.input").len();
    s.b.v.press(Key::Escape, Modifiers::NONE);
    s.b.v.settle(0.3);
    assert!(!s.b.v.says(&overrides), "Escape closes the window");
    assert_eq!(s.calls("playground.input").len(), relayed, "the Escape did not reach the page");
    assert_eq!(s.b.v.app.session.docs[s.b.v.app.session.active].revision, revision, "reading the code changes nothing");
}

// covers: E7-7.6
#[test]
fn playground_has_no_claude_and_no_share_button() {
    let mut s = studio("no-claude", "en", Tokens::Rich, Start::Ready, true);
    let names = |s: &Studio| -> BTreeSet<String> {
        let mut said: BTreeSet<String> = s.b.v.texts().into_iter().map(|t| t.trim().to_string()).collect();
        for role in [Role::Button, Role::Link, Role::Tab, Role::MenuItem] {
            said.extend(s.b.v.with_role(role).iter().filter_map(|n| n.label()).map(|l| l.trim().to_string()));
        }
        said
    };
    let in_playground = names(&s);
    assert!(in_playground.contains("Random settings"), "the bars are on screen");
    s.b.v.click_node(Role::Tab, crate::playground_view_tests::DOC_TAB);
    s.b.v.settle(0.3);
    let elsewhere = names(&s);
    let only_here: Vec<&String> = in_playground.difference(&elsewhere).collect();
    for word in ["Claude", "Share", "Partager", "Partage"] {
        assert!(!only_here.iter().any(|n| n.eq_ignore_ascii_case(word)), "{word} appears in the playground: {only_here:?}");
    }
    assert!(BUTTONS.iter().all(|b| !b.contains("Claude") && !b.contains("Share")));
}

// covers: E7-7.6
#[test]
fn playground_every_command_of_the_bars_is_reachable_and_runs_from_the_keyboard() {
    let mut s = ready("keyboard", Tokens::AccentOnly);
    s.run("playground.set", json!({"accent": "#2563eb"}));
    for (name, command) in [
        ("Undo setting", "playground.undo"),
        ("Redo setting", "playground.redo"),
        ("Revert settings", "playground.revert"),
        ("Random settings", "playground.random"),
    ] {
        s.tab_to(name);
        s.b.v.press(Key::Enter, Modifiers::NONE);
        s.b.v.settle(0.3);
        assert_eq!(s.calls(command).len(), 1, "Tab then Enter on {name:?} runs {command}");
    }
    s.tab_to("Dark mode");
    s.b.v.press(Key::Enter, Modifiers::NONE);
    s.b.v.settle(0.3);
    assert_eq!(s.state()["settings"]["mode"], json!("dark"));

    s.tab_to("Radius shape");
    s.b.v.press(Key::Enter, Modifiers::NONE);
    s.b.v.settle(0.3);
    assert!(s.item("Squircle").is_some(), "Enter opens the menu");
    s.b.v.press(Key::Escape, Modifiers::NONE);

    s.tab_to("Show code");
    s.b.v.press(Key::Enter, Modifiers::NONE);
    s.b.v.settle(0.3);
    assert!(s.b.v.has(Role::Button, "Copy overrides"), "Enter on Show code opens the code window");
    s.b.v.press(Key::Escape, Modifiers::NONE);
    s.b.v.settle(0.3);

    s.tab_to("Save theme");
    s.b.v.press(Key::Enter, Modifiers::NONE);
    s.b.v.settle(0.3);
    assert!(s.b.v.has(Role::Button, "Save"), "Enter on Save theme opens the confirmation");
}

fn text_of<'a>(lang: &'a Lang, key: &str) -> Option<&'a str> {
    lang.ui.iter().find(|(k, _)| *k == key).map(|(_, t)| match t {
        Text::Plain(s) | Text::Plural { other: s, .. } => *s,
    })
}

fn keys_in(source: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    let mut rest = source;
    while let Some(at) = rest.find("tr(\"") {
        let after = &rest[at + 4..];
        let mut key = String::new();
        let mut chars = after.chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => key.extend(chars.next()),
                '"' => break,
                other => key.push(other),
            }
        }
        keys.insert(key);
        rest = after;
    }
    keys
}

const NEW_KEYS: [&str; 34] = [
    "Undo setting",
    "Redo setting",
    "Revert settings",
    "Random settings",
    "Light mode",
    "Dark mode",
    "Save theme",
    "Show code",
    "Accent",
    "Base hue",
    "Base tint",
    "Radius",
    "Radius shape",
    "Font",
    "Theme",
    "Round",
    "Squircle",
    "Bevel",
    "Theme name",
    "Save",
    "Cancel",
    "Overwrite the chosen theme",
    "Give the theme a name",
    "A theme with this name already exists",
    "No accent token in this theme",
    "No base token in this theme",
    "No font token in this theme",
    "No radius token in this theme",
    "Copy overrides",
    "Copy template",
    "None",
    "Small",
    "Large",
    "Full",
];

// covers: E7-7.6
#[test]
fn playground_new_texts_exist_in_the_22_catalogs() {
    let languages = i18n::languages();
    assert_eq!(languages.len(), 22);
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let sources = [src.join("panels/playground_bars.rs"), src.join("dialogs/playground_save.rs"), src.join("dialogs/playground_code.rs")];
    let mut keys = BTreeSet::new();
    for path in &sources {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        keys.extend(keys_in(&text));
    }
    for key in NEW_KEYS {
        assert!(keys.contains(key), "the interface files write tr(\"{key}\"): {keys:?}");
    }
    let mut missing = Vec::new();
    for lang in languages.iter().filter(|l| l.code != "en") {
        let own: Vec<&Lang> = [Some(lang), languages.get(lang.parent)].into_iter().flatten().filter(|l| l.code != "en").collect();
        for key in &keys {
            if !own.iter().any(|l| l.ui.iter().any(|(k, _)| k == key)) {
                missing.push(format!("{}: {key:?}", lang.code));
            }
        }
    }
    assert!(missing.is_empty(), "untranslated:\n{}", missing.join("\n"));
    let fr = languages.iter().find(|l| l.code == "fr").expect("French is baked in");
    assert_eq!(text_of(fr, "No accent token in this theme").map(str::to_lowercase).as_deref(), Some("aucun jeton d'accent dans ce thème"));
}
