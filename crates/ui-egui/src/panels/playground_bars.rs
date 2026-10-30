//! The Playground tab's top bar, model selector and settings bar. They read `playground.state` and
//! act through the `playground.*` commands; nothing here touches the document.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::RangeInclusive;
use std::sync::Arc;

use egui::{CornerRadius, Id, Margin, Rect, Response, ScrollArea, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use figcraft_color::{Color, Oklch};
use figcraft_engine::capture::CaptureEvent;
use figcraft_engine::playground::Status;
use serde_json::{Value, json};

use crate::FigApp;
use crate::a11y::{self, Region};
use crate::i18n::tr;
use crate::icons::{self, Icon};
use crate::theme::Tokens;
use crate::widgets::{self, FIELD_H};

const STATE: &str = "playground.state";
const SET: &str = "playground.set";
const TEMPLATE: &str = "playground.template";
const OPENED: [&str; 2] = ["playground.open", "playground.refresh"];
const GENERIC_FONTS: [&str; 4] = ["system-ui", "serif", "monospace", "ui-rounded"];
const MAX_FONT_CHARS: usize = 64;
const FORBIDDEN_IN_FONT: [char; 7] = [';', '{', '}', '(', ')', '\\', '"'];
const SLIDER_WIDTH: f32 = 96.0;
const ICON_CELL: f32 = 24.0;
const MODE_CELL: f32 = 28.0;
const MENU_ROW_HEIGHT: f32 = 26.0;
const FONT_MENU_HEIGHT: f32 = 320.0;
const FONT_MENU_WIDTH: f32 = 180.0;
const HUE: RangeInclusive<f64> = 0.0..=360.0;
const TINT: RangeInclusive<f64> = 0.0..=1.0;
const DEFAULT_LIGHTNESS: f32 = 0.65;
const DEFAULT_CHROMA: f32 = 0.15;
const CHROMA: RangeInclusive<f32> = 0.05..=0.37;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct Found {
    accent: bool,
    base: bool,
    font: bool,
    radius: bool,
}

/// One thing the export could not do, as `playground.state` lists it.
#[derive(Clone, Default)]
pub struct Note {
    pub component: String,
    pub message: String,
}

/// What `playground.state` says, read once a frame.
#[derive(Clone, Default)]
pub struct Snapshot {
    pub input_waiting: usize,
    pub apply_waiting: bool,
    pub missing: Vec<String>,
    pub notes: Vec<Note>,
    pub signature: u64,
    dark: bool,
    theme: String,
    themes: Vec<String>,
    template: String,
    radius: Option<String>,
    shape: String,
    font: Option<String>,
    shown_font: Option<String>,
    accent: Option<Oklch>,
    base: Option<(f64, f64)>,
    found: Found,
    can_undo: bool,
    can_redo: bool,
    changed: bool,
}

impl Snapshot {
    pub fn read(state: &Value) -> Self {
        let text = |value: &Value| value.as_str().map(str::to_string);
        let texts = |value: &Value| value.as_array().map(|names| names.iter().filter_map(text).collect()).unwrap_or_default();
        let mut hasher = DefaultHasher::new();
        [&state["settings"], &state["template"], &state["applied"], &state["canUndo"], &state["canRedo"], &state["themes"]]
            .iter()
            .for_each(|part| part.to_string().hash(&mut hasher));
        let waiting = Self {
            input_waiting: state["inputWaiting"].as_u64().and_then(|count| usize::try_from(count).ok()).unwrap_or(0),
            apply_waiting: state["applyWaiting"].as_bool().unwrap_or(false),
            missing: texts(&state["missing"]),
            notes: state["notes"].as_array().map(|notes| notes.iter().map(Note::read).collect()).unwrap_or_default(),
            signature: hasher.finish(),
            ..Self::default()
        };
        let settings = &state["settings"];
        if settings.is_null() {
            return waiting;
        }
        let effective = &state["effective"];
        let found = |key: &str| state["targets"][key].as_u64().is_some_and(|count| count > 0);
        let flag = |key: &str| state[key].as_bool().unwrap_or(false);
        let shape = text(&settings["radiusShape"]).unwrap_or_else(|| "round".to_string());
        let dark = settings["mode"] == "dark";
        let touched = ["accent", "base", "font", "radius", "theme"].iter().any(|key| !settings[*key].is_null());
        Self {
            dark,
            theme: text(&settings["theme"]).or_else(|| text(&state["applied"])).unwrap_or_default(),
            themes: texts(&state["themes"]),
            template: text(&state["template"]).unwrap_or_default(),
            radius: text(&settings["radius"]),
            font: text(&settings["font"]),
            shown_font: text(&effective["font"]),
            accent: effective["accent"].as_str().and_then(Color::from_hex).map(Oklch::from_color),
            base: effective["base"]["hue"].as_f64().zip(effective["base"]["tint"].as_f64()),
            found: Found { accent: found("accent"), base: found("base"), font: found("font"), radius: found("radius") },
            can_undo: flag("canUndo"),
            can_redo: flag("canRedo"),
            changed: touched || shape != "round" || dark,
            shape,
            ..waiting
        }
    }

    pub fn waiting(&self) -> bool {
        self.input_waiting > 0 || self.apply_waiting
    }
}

impl Note {
    fn read(note: &Value) -> Self {
        let text = |key: &str| note[key].as_str().unwrap_or_default().to_string();
        Self { component: text("component"), message: text("message") }
    }
}

/// What the bars keep between frames.
#[derive(Default)]
pub struct Bars {
    shape: bool,
    pub errors: Vec<String>,
    fonts: Option<Arc<[String]>>,
}

impl Bars {
    fn installed_fonts(&mut self) -> Arc<[String]> {
        let fonts = self.fonts.get_or_insert_with(|| {
            let mut names: Vec<String> = figcraft_text::family_names().into_iter().filter(|name| is_font_name(name)).collect();
            names.sort();
            names.dedup();
            names.into()
        });
        Arc::clone(fonts)
    }
}

fn is_font_name(name: &str) -> bool {
    !name.trim().is_empty() && name.chars().count() <= MAX_FONT_CHARS && !name.chars().any(|c| c.is_control() || FORBIDDEN_IN_FONT.contains(&c))
}

/// Whether `event` is the end of a `playground.open` or `playground.refresh` job; it tells whether
/// the page's Chromium knows `corner-shape`.
pub fn note_event(app: &mut FigApp, event: &CaptureEvent) -> bool {
    if !OPENED.contains(&event.command.as_str()) {
        return false;
    }
    let bars = &mut app.ui.playground.bars;
    bars.errors.clear();
    if let Ok(answer) = &event.outcome {
        bars.shape = answer["shape"].as_bool().unwrap_or(false);
        bars.errors =
            answer["errors"].as_array().map(|errors| errors.iter().filter_map(|e| e.as_str().map(str::to_string)).collect()).unwrap_or_default();
    }
    true
}

/// The state of the playground now: the playground commands leave the document's revision and
/// journal alone, so nothing but the state itself says that a setting changed.
pub fn snapshot(app: &mut FigApp) -> Snapshot {
    app.session.execute(STATE, &json!({})).map(|state| Snapshot::read(&state)).unwrap_or_default()
}

/// The themes of the document and the one the settings are written from.
pub fn theme_choices(app: &mut FigApp) -> (Vec<String>, String) {
    let snapshot = snapshot(app);
    (snapshot.themes, snapshot.theme)
}

/// Draws the bars at the top of the area `ui` covers; the height they take.
pub fn show(app: &mut FigApp, ui: &mut Ui, status: &Status, snap: &Snapshot) -> f32 {
    let t = Tokens::get(ui.ctx());
    let ready = *status == Status::Ready;
    let band = egui::Frame::NONE.fill(t.panel).inner_margin(Margin::symmetric(12, 6)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
        top_bar(app, ui, &t, snap, ready);
        settings_bar(app, ui, &t, snap, ready);
    });
    let rect = band.response.rect;
    ui.painter().hline(rect.x_range(), rect.max.y - 0.5, Stroke::new(1.0, t.border));
    a11y::area(&mut app.ui.nav, Region::Toolbar, rect);
    rect.height()
}

fn top_bar(app: &mut FigApp, ui: &mut Ui, t: &Tokens, snap: &Snapshot, ready: bool) {
    ui.horizontal(|ui| {
        ui.set_min_height(FIELD_H);
        template_menu(app, ui, t, snap, ready);
        ui.add_space(8.0);
        for (icon, name, enabled, command) in [
            (Icon::Undo, tr("Undo setting"), snap.can_undo, "playground.undo"),
            (Icon::Redo, tr("Redo setting"), snap.can_redo, "playground.redo"),
            (Icon::Reset, tr("Revert settings"), snap.changed, "playground.revert"),
            (Icon::Swap, tr("Random settings"), true, "playground.random"),
        ] {
            if icon_button(ui, t, icon, name, ready && enabled) {
                app.run(command, json!({}));
            }
        }
        ui.add_space(8.0);
        if let Some(mode) = mode_toggle(ui, t, snap.dark, ready) {
            set(app, json!({"mode": mode}), true);
        }
        ui.add_space(8.0);
        if text_button(ui, tr("Save theme"), ready) {
            app.ui.playground.save_open = true;
        }
        if text_button(ui, tr("Show code"), ready) {
            app.ui.playground.code_open = true;
        }
    });
}

fn template_menu(app: &mut FigApp, ui: &mut Ui, t: &Tokens, snap: &Snapshot, ready: bool) {
    let choices = [
        (tr("Components"), "components"),
        (tr("Dashboard"), "dashboard"),
        (tr("Mail"), "mail"),
        (tr("Chat"), "chat"),
        (tr("Finance"), "finance"),
        (tr("CRM"), "crm"),
    ];
    let shown = choices.iter().find(|(_, id)| *id == snap.template).map_or("", |(label, _)| *label);
    let control = Control { name: tr("Template"), enabled: ready, reason: None };
    if let Some(name) = combo(ui, t, &control, shown, |ui| pick(ui, &choices, &snap.template)) {
        app.run(TEMPLATE, json!({"name": name}));
    }
}

fn settings_bar(app: &mut FigApp, ui: &mut Ui, t: &Tokens, snap: &Snapshot, ready: bool) {
    ui.horizontal(|ui| {
        accent_slider(app, ui, t, snap, ready);
        base_sliders(app, ui, t, snap, ready);
    });
    ui.horizontal(|ui| {
        radius_menus(app, ui, t, snap, ready);
        font_menu(app, ui, t, snap, ready);
        theme_menu(app, ui, t, snap, ready);
    });
}

fn set(app: &mut FigApp, mut params: Value, commit: bool) {
    params["commit"] = json!(commit);
    app.run(SET, params);
}

struct Control<'a> {
    name: &'a str,
    enabled: bool,
    reason: Option<&'a str>,
}

fn missing(ready: bool, found: bool, reason: &str) -> Option<&str> {
    (ready && !found).then_some(reason)
}

struct Edit {
    value: f64,
    done: bool,
}

fn accent_slider(app: &mut FigApp, ui: &mut Ui, t: &Tokens, snap: &Snapshot, ready: bool) {
    let control = Control {
        name: tr("Accent"),
        enabled: ready && snap.found.accent,
        reason: missing(ready, snap.found.accent, tr("No accent token in this theme")),
    };
    let hue = snap.accent.map_or(0.0, |accent| f64::from(accent.h));
    if let Some(edit) = slider(ui, t, &control, hue, HUE) {
        let memo = Id::unique("playground_accent_chroma");
        let ctx = ui.ctx();
        let chroma = ctx
            .data_mut(|d| d.get_temp::<f32>(memo))
            .unwrap_or_else(|| snap.accent.map_or(DEFAULT_CHROMA, |a| a.c.clamp(*CHROMA.start(), *CHROMA.end())));
        ctx.data_mut(|d| {
            if edit.done {
                d.remove_temp::<f32>(memo);
            } else {
                d.insert_temp(memo, chroma);
            }
        });
        let lightness = snap.accent.map_or(DEFAULT_LIGHTNESS, |accent| accent.l);
        let color = Oklch { l: lightness, c: chroma, h: edit.value as f32 }.to_color();
        set(app, json!({"accent": color.to_css_hex()}), edit.done);
    }
}

fn base_sliders(app: &mut FigApp, ui: &mut Ui, t: &Tokens, snap: &Snapshot, ready: bool) {
    let (hue, tint) = snap.base.unwrap_or((0.0, 0.0));
    let on = ready && snap.found.base;
    let reason = missing(ready, snap.found.base, tr("No base token in this theme"));
    let control = Control { name: tr("Base hue"), enabled: on, reason };
    if let Some(edit) = slider(ui, t, &control, hue, HUE) {
        set(app, json!({"base": {"hue": edit.value, "tint": tint}}), edit.done);
    }
    let control = Control { name: tr("Base tint"), enabled: on, reason };
    if let Some(edit) = slider(ui, t, &control, tint, TINT) {
        set(app, json!({"base": {"hue": hue, "tint": edit.value}}), edit.done);
    }
}

fn radius_menus(app: &mut FigApp, ui: &mut Ui, t: &Tokens, snap: &Snapshot, ready: bool) {
    let sizes = [(tr("None"), "zero"), (tr("Small"), "small"), (tr("Medium"), "medium"), (tr("Large"), "large"), (tr("Full"), "full")];
    let size = snap.radius.as_deref().unwrap_or("medium");
    let shown = sizes.iter().find(|(_, id)| *id == size).map_or("", |(label, _)| *label);
    let control = Control {
        name: tr("Radius"),
        enabled: ready && snap.found.radius,
        reason: missing(ready, snap.found.radius, tr("No radius token in this theme")),
    };
    if let Some(id) = combo(ui, t, &control, shown, |ui| pick(ui, &sizes, size)) {
        set(app, json!({"radius": id}), true);
    }
    let shapes = [(tr("Round"), "round"), (tr("Squircle"), "squircle"), (tr("Bevel"), "bevel")];
    let shown = shapes.iter().find(|(_, id)| *id == snap.shape).map_or("", |(label, _)| *label);
    let control = Control { name: tr("Radius shape"), enabled: ready && app.ui.playground.bars.shape, reason: None };
    if let Some(id) = combo(ui, t, &control, shown, |ui| pick(ui, &shapes, &snap.shape)) {
        set(app, json!({"radiusShape": id}), true);
    }
}

fn font_menu(app: &mut FigApp, ui: &mut Ui, t: &Tokens, snap: &Snapshot, ready: bool) {
    let control =
        Control { name: tr("Font"), enabled: ready && snap.found.font, reason: missing(ready, snap.found.font, tr("No font token in this theme")) };
    let installed = if control.enabled { app.ui.playground.bars.installed_fonts() } else { Arc::from([]) };
    let chosen = snap.font.as_deref();
    let shown = chosen.or(snap.shown_font.as_deref()).unwrap_or("—");
    if let Some(family) = combo(ui, t, &control, shown, |ui| font_choices(ui, &installed, chosen)) {
        set(app, json!({"font": family}), true);
    }
}

fn font_choices(ui: &mut Ui, installed: &[String], chosen: Option<&str>) -> Option<String> {
    ui.set_min_width(FONT_MENU_WIDTH);
    ui.spacing_mut().item_spacing.y = 0.0;
    let mut picked = None;
    for family in GENERIC_FONTS {
        if crate::right::menu_check(ui, chosen == Some(family), family, "") {
            picked = Some(family.to_string());
            ui.close();
        }
    }
    ui.separator();
    ScrollArea::vertical().max_height(FONT_MENU_HEIGHT).show_rows(ui, MENU_ROW_HEIGHT, installed.len(), |ui, rows| {
        for family in installed.get(rows).unwrap_or_default() {
            if crate::right::menu_check(ui, chosen == Some(family.as_str()), family, "") {
                picked = Some(family.clone());
                ui.close();
            }
        }
    });
    picked
}

fn theme_menu(app: &mut FigApp, ui: &mut Ui, t: &Tokens, snap: &Snapshot, ready: bool) {
    let control = Control { name: tr("Theme"), enabled: ready && !snap.themes.is_empty(), reason: None };
    let picked = combo(ui, t, &control, &snap.theme, |ui| {
        let mut picked = None;
        for name in &snap.themes {
            if crate::right::menu_check(ui, *name == snap.theme, name, "") {
                picked = Some(name.clone());
                ui.close();
            }
        }
        picked
    });
    if let Some(name) = picked {
        set(app, json!({"theme": name}), true);
    }
}

fn pick<'a>(ui: &mut Ui, choices: &[(&str, &'a str)], current: &str) -> Option<&'a str> {
    let mut picked = None;
    for (label, id) in choices {
        if crate::right::menu_check(ui, *id == current, label, "") {
            picked = Some(*id);
            ui.close();
        }
    }
    picked
}

fn sense(enabled: bool) -> Sense {
    if enabled { Sense::click() } else { Sense::hover() }
}

fn tell(ui: &Ui, resp: &Response, reason: Option<&str>) {
    let Some(reason) = reason else { return };
    ui.interact(resp.rect, resp.id.with("reason"), Sense::hover()).on_hover_text(reason);
    ui.ctx().accesskit_node_builder(resp.id, |node| node.set_description(reason));
}

fn icon_button(ui: &mut Ui, t: &Tokens, icon: Icon, name: &str, enabled: bool) -> bool {
    let (rect, _) = ui.allocate_exact_size(vec2(ICON_CELL, ICON_CELL), Sense::hover());
    let resp = ui.interact(rect, ui.scope_id().with(name), sense(enabled)).on_hover_text(name);
    a11y::button(&resp, name);
    if !enabled {
        a11y::disabled(&resp);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(5), t.hover);
    }
    icons::paint(ui.painter(), rect.shrink(5.0), icon, if enabled { t.icon } else { t.text_tertiary });
    enabled && resp.clicked()
}

fn text_button(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    let resp = widgets::button(ui, label, false, enabled);
    if !enabled {
        a11y::disabled(&resp);
    }
    enabled && resp.clicked()
}

fn mode_toggle(ui: &mut Ui, t: &Tokens, dark: bool, enabled: bool) -> Option<&'static str> {
    let cells = [(Icon::Sun, tr("Light mode"), "light"), (Icon::Moon, tr("Dark mode"), "dark")];
    let (rect, _) = ui.allocate_exact_size(vec2(MODE_CELL * 2.0, FIELD_H), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(5), t.field);
    let mut picked = None;
    for (index, (icon, name, mode)) in cells.into_iter().enumerate() {
        let cell = Rect::from_min_size(pos2(rect.min.x + index as f32 * MODE_CELL, rect.min.y), vec2(MODE_CELL, FIELD_H));
        let on = (mode == "dark") == dark;
        let resp = ui.interact(cell, ui.scope_id().with(mode), sense(enabled)).on_hover_text(name);
        a11y::toggle(&resp, name, on);
        if !enabled {
            a11y::disabled(&resp);
        }
        if on {
            ui.painter().rect_filled(cell.shrink(2.0), CornerRadius::same(4), t.panel);
            ui.painter().rect_stroke(cell.shrink(2.0), CornerRadius::same(4), Stroke::new(1.0, t.border), StrokeKind::Inside);
        } else if enabled && resp.hovered() {
            ui.painter().rect_filled(cell.shrink(2.0), CornerRadius::same(4), t.hover);
        }
        let ink = if !enabled {
            t.text_tertiary
        } else if on {
            t.icon
        } else {
            t.icon_secondary
        };
        icons::paint(ui.painter(), Rect::from_center_size(cell.center(), vec2(14.0, 14.0)), icon, ink);
        if enabled && !on && resp.clicked() {
            picked = Some(mode);
        }
    }
    picked
}

fn slider(ui: &mut Ui, t: &Tokens, control: &Control, shown: f64, range: RangeInclusive<f64>) -> Option<Edit> {
    ui.push_id(control.name, |ui| {
        widgets::label(ui, control.name, t.text_secondary);
        ui.spacing_mut().slider_width = SLIDER_WIDTH;
        let mut value = shown;
        let resp = ui.add_enabled(control.enabled, egui::Slider::new(&mut value, range).show_value(false).smart_aim(false));
        a11y::slider(&resp, control.name, value, control.enabled);
        tell(ui, &resp, control.reason);
        edit_of(ui, &resp, value)
    })
    .inner
}

fn edit_of(ui: &Ui, resp: &Response, value: f64) -> Option<Edit> {
    let held = resp.id.with("held");
    let last: Option<f64> = ui.data_mut(|d| d.get_temp(held));
    let down = resp.is_pointer_button_down_on();
    if resp.changed() && down {
        ui.data_mut(|d| d.insert_temp(held, value));
        return Some(Edit { value, done: false });
    }
    if resp.changed() || (last.is_some() && !down) {
        ui.data_mut(|d| d.remove_temp::<f64>(held));
        return Some(Edit { value: if resp.changed() { value } else { last.unwrap_or(value) }, done: true });
    }
    None
}

fn combo<T>(ui: &mut Ui, t: &Tokens, control: &Control, shown: &str, menu: impl FnOnce(&mut Ui) -> Option<T>) -> Option<T> {
    ui.push_id(control.name, |ui| {
        widgets::label(ui, control.name, t.text_secondary);
        let resp = ui.add_enabled_ui(control.enabled, |ui| widgets::dropdown(ui, shown, 110.0, None)).inner;
        a11y::combo(&resp, control.name, shown, a11y::menu_open(&resp));
        if !control.enabled {
            a11y::disabled(&resp);
        }
        tell(ui, &resp, control.reason);
        if !control.enabled {
            return None;
        }
        crate::right::menu_popup(&resp).show(menu).and_then(|opened| opened.inner)
    })
    .inner
}
