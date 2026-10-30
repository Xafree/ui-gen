//! File ▸ Import a component…: the window that imports one component of a source library. It lists
//! the components `uigen.listComponents` read, sets the options from the props of the one chosen,
//! runs `uigen.importComponent` with `stage: true` and follows the capture jobs until the set is in
//! the import page. Only a user opens it (`dialog.importComponent`); the control channel never does.

use std::sync::Arc;

use egui::{Align, Align2, Context, CornerRadius, Layout, RichText, ScrollArea, Sense, Ui, pos2, vec2};
use figcraft_engine::capture::{CaptureEvent, CaptureOutcome, JobState};
use figcraft_engine::{Inspected, InspectedComponent, InspectedProp, MAX_RENDERS, PropKind, render_count};
use serde_json::{Value, json};

use super::window;
use crate::FigApp;
use crate::a11y;
use crate::i18n::{tr, trf};
use crate::theme::{Tokens, regular, semibold};
use crate::widgets::{self, FIELD_H};

const LIST: &str = "uigen.listComponents";
const IMPORT: &str = "uigen.importComponent";
const SETTINGS: &str = "project.settings";
const SHOW: &str = "view.assets";

const WIDTH: f32 = 520.0;
const LIST_HEIGHT: f32 = 132.0;
const ROW_H: f32 = 24.0;
const SOURCE_W: f32 = 280.0;
const OPTION_PADDING: f32 = 34.0;
const STATES: [&str; 6] = ["Default", "Hover", "Focus", "Pressed", "Disabled", "Loading"];
const CHECKED_STATES: [&str; 4] = ["Default", "Hover", "Pressed", "Disabled"];

type Picks = Vec<(String, bool)>;

#[derive(Clone, Debug)]
struct Choice {
    component: String,
    style_prop: Option<String>,
    size_prop: Option<String>,
    styles: Picks,
    sizes: Picks,
    states: Vec<(&'static str, bool)>,
    light: bool,
    dark: bool,
    animations: bool,
}

#[derive(Clone, Debug, Default)]
enum Run {
    #[default]
    Form,
    Importing {
        component: String,
        first_job: u64,
        renders: usize,
    },
    Done {
        component: String,
        variants: usize,
        warnings: usize,
    },
    Failed {
        component: String,
        cause: String,
    },
}

#[derive(Clone, Debug, Default)]
pub struct ImportWindow {
    package: Option<String>,
    asked: Option<(String, String)>,
    read_job: Option<u64>,
    failure: Option<String>,
    search: String,
    focus_search: bool,
    choice: Option<Choice>,
    run: Run,
}

enum Listing {
    NoSource,
    Loading,
    Failed(String),
    Ready(Arc<Inspected>),
}

struct View {
    sources: Vec<(String, String)>,
    listing: Listing,
    progress: (usize, usize),
}

enum Act {
    Source(String),
    TryAgain,
    OpenSettings,
    Import,
    Another,
    Show,
    Close,
}

fn checked(picks: &[(String, bool)]) -> Vec<&str> {
    picks.iter().filter(|(_, on)| *on).map(|(value, _)| value.as_str()).collect()
}

fn state_label(state: &str) -> &'static str {
    match state {
        "Default" => tr("Default"),
        "Hover" => tr("Hover"),
        "Focus" => tr("Focus"),
        "Pressed" => tr("Pressed"),
        "Disabled" => tr("Disabled"),
        _ => tr("Loading"),
    }
}

impl Choice {
    fn of(component: &InspectedComponent) -> Self {
        let enum_prop = |name: &str| component.props.iter().find(|prop| prop.name == name && prop.kind == PropKind::Enum && !prop.values.is_empty());
        let picks = |prop: Option<&InspectedProp>| -> Picks {
            prop.map_or_else(Vec::new, |prop| prop.values.iter().map(|value| (value.clone(), true)).collect())
        };
        let loading = component.props.iter().any(|prop| prop.name == "loading" && prop.kind == PropKind::Boolean);
        let (style, size) = (enum_prop("variant"), enum_prop("size"));
        Self {
            component: component.name.clone(),
            style_prop: style.map(|prop| prop.name.clone()),
            size_prop: size.map(|prop| prop.name.clone()),
            styles: picks(style),
            sizes: picks(size),
            states: STATES.into_iter().filter(|state| loading || *state != "Loading").map(|state| (state, CHECKED_STATES.contains(&state))).collect(),
            light: true,
            dark: true,
            animations: true,
        }
    }

    fn counts(&self) -> [usize; 4] {
        let on = |picks: &Picks| picks.iter().filter(|(_, on)| *on).count();
        let states = self.states.iter().filter(|(_, on)| *on).count();
        [on(&self.styles), on(&self.sizes), states, usize::from(self.light) + usize::from(self.dark)]
    }

    fn has_gap(&self) -> bool {
        let [styles, sizes, states, themes] = self.counts();
        (self.style_prop.is_some() && styles == 0) || (self.size_prop.is_some() && sizes == 0) || states == 0 || themes == 0
    }

    fn totals(&self) -> (usize, usize) {
        if self.has_gap() {
            return (0, 0);
        }
        let [styles, sizes, states, themes] = self.counts();
        (render_count(styles, sizes, states, 1), render_count(styles, sizes, states, themes))
    }

    fn reason(&self) -> Option<String> {
        if self.has_gap() {
            return Some(tr("Choose at least one value in each group.").to_string());
        }
        (self.totals().1 > MAX_RENDERS)
            .then(|| trf("More than {max} renders: choose fewer styles, sizes, states or themes", &[("max", &MAX_RENDERS)]))
    }

    fn params(&self, package: &str) -> Value {
        let themes: Vec<&str> = [(self.light, "light"), (self.dark, "dark")].into_iter().filter(|(on, _)| *on).map(|(_, theme)| theme).collect();
        let states: Vec<&str> = self.states.iter().filter(|(_, on)| *on).map(|(state, _)| *state).collect();
        let mut params = json!({
            "package": package,
            "component": self.component,
            "states": states,
            "themes": themes,
            "animations": self.animations,
            "stage": true,
        });
        let axes = [("styleProp", "styles", &self.style_prop, &self.styles), ("sizeProp", "sizes", &self.size_prop, &self.sizes)];
        for (prop_key, values_key, prop, picks) in axes {
            if let (Some(prop), Some(map)) = (prop, params.as_object_mut()) {
                map.insert(prop_key.to_string(), json!(prop));
                map.insert(values_key.to_string(), json!(checked(picks)));
            }
        }
        params
    }
}

impl ImportWindow {
    fn switch(&mut self, package: Option<String>) {
        self.package = package;
        self.asked = None;
        self.read_job = None;
        self.failure = None;
        self.search.clear();
        self.choice = None;
        self.run = Run::Form;
    }

    fn read(&mut self, outcome: &CaptureOutcome) {
        self.failure = match outcome {
            Ok(result) => {
                let asked = self.asked.as_ref().map(|(_, version)| version.as_str());
                let reported = result.get("version").and_then(Value::as_str);
                (asked.is_some() && reported.is_some() && asked != reported)
                    .then(|| tr("The version read is not the one the project asks for").to_string())
            }
            Err(error) => Some(error.message.clone()),
        };
    }

    fn finish(&mut self, job: u64, outcome: &CaptureOutcome) -> bool {
        let Run::Importing { component, first_job, .. } = &self.run else { return false };
        if *first_job != job {
            return false;
        }
        let component = component.clone();
        self.run = match outcome {
            Ok(result) => Run::Done {
                component,
                variants: result.get("variants").and_then(Value::as_u64).map_or(0, |n| usize::try_from(n).unwrap_or(0)),
                warnings: result.get("warnings").and_then(Value::as_array).map_or(0, Vec::len),
            },
            Err(error) => {
                log::warn!("{IMPORT} {component}: {}", error.message);
                Run::Failed { component, cause: error.message.clone() }
            }
        };
        true
    }
}

/// `dialog.importComponent`: open the window for the project of the file in front, on `package`.
pub(crate) fn request(app: &mut FigApp, package: Option<&str>) {
    if cfg!(target_arch = "wasm32") {
        return;
    }
    app.ui.import_window = Some(ImportWindow { package: package.map(str::to_string), focus_search: true, ..ImportWindow::default() });
}

/// Take the capture events the window shows; `false` leaves the event to the toast.
pub(crate) fn pump_event(app: &mut FigApp, event: &CaptureEvent) -> bool {
    let Some(state) = app.ui.import_window.as_mut() else { return false };
    match (event.command.as_str(), &event.outcome) {
        (LIST, outcome) if state.read_job == Some(event.job) => {
            state.read_job = None;
            state.read(outcome);
            true
        }
        (IMPORT, outcome) => state.finish(event.job, outcome),
        _ => false,
    }
}

fn edit(app: &mut FigApp, change: impl FnOnce(&mut ImportWindow)) {
    if let Some(state) = app.ui.import_window.as_mut() {
        change(state);
    }
}

fn active_sources(app: &FigApp) -> Option<Vec<(String, String)>> {
    let binding = app.session.project()?;
    Some(binding.config.sources.iter().filter(|source| !source.removed).map(|source| (source.package.clone(), source.version.clone())).collect())
}

fn fetch(app: &mut FigApp, refresh: bool) {
    let Some(package) = app.ui.import_window.as_ref().and_then(|state| state.package.clone()) else { return };
    let Some(root) = app.session.project().map(|binding| binding.root.clone()) else { return };
    let version = active_sources(app).and_then(|sources| sources.into_iter().find(|(name, _)| *name == package)).map(|(_, version)| version);
    edit(app, |state| {
        state.asked = version.map(|version| (root, version));
        state.read_job = None;
        state.failure = None;
        state.choice = None;
    });
    let params = if refresh { json!({"package": package, "refresh": true}) } else { json!({"package": package}) };
    match app.run(LIST, params) {
        Some(started) => edit(app, |state| state.read_job = started.get("job").and_then(Value::as_u64)),
        None => {
            let cause = app.ui.last_error.clone().unwrap_or_default();
            edit(app, |state| state.failure = Some(cause));
        }
    }
}

/// Keep the window on a source the project still has, and read the components of the version it has.
fn settle(app: &mut FigApp) -> bool {
    let Some(sources) = active_sources(app) else { return false };
    let Some(root) = app.session.project().map(|binding| binding.root.clone()) else { return false };
    let Some(state) = app.ui.import_window.as_mut() else { return false };
    let moved = state.asked.as_ref().is_some_and(|(asked_root, _)| *asked_root != root);
    let kept = state.package.clone().filter(|package| sources.iter().any(|(name, _)| name == package));
    let package = kept.or_else(|| sources.first().map(|(name, _)| name.clone()));
    if moved || state.package != package {
        state.switch(package);
    }
    let version = state.package.as_ref().and_then(|package| sources.iter().find(|(name, _)| name == package)).map(|(_, version)| version.clone());
    let wanted = version.map(|version| (root, version));
    if wanted.is_some() && state.asked != wanted {
        fetch(app, false);
    }
    true
}

fn progress(app: &FigApp, first_job: u64, renders: usize) -> (usize, usize) {
    let total = 1 + renders;
    let ours = app.session.import_jobs(first_job);
    let done = app.session.capture_jobs().iter().filter(|job| job.state == JobState::Done && ours.contains(&job.job)).count();
    (done.min(total), total)
}

fn view_of(app: &FigApp, state: &ImportWindow) -> View {
    let listing = match state.package.as_deref() {
        None => Listing::NoSource,
        Some(package) => match (app.session.inspected(package), &state.failure) {
            (Some(found), _) => Listing::Ready(found),
            (None, Some(cause)) => Listing::Failed(cause.clone()),
            (None, None) => Listing::Loading,
        },
    };
    let progress = match &state.run {
        Run::Importing { first_job, renders, .. } => progress(app, *first_job, *renders),
        _ => (0, 0),
    };
    View { sources: active_sources(app).unwrap_or_default(), listing, progress }
}

pub fn show(app: &mut FigApp, ctx: &Context) {
    if app.ui.import_window.is_none() {
        return;
    }
    if !settle(app) {
        app.ui.import_window = None;
        return;
    }
    let Some(mut state) = app.ui.import_window.take() else { return };
    let view = view_of(app, &state);
    let t = Tokens::get(ctx);
    let mut open = true;
    let mut acts = Vec::new();
    window(tr("Import a component"), ctx).open(&mut open).show(ctx, |ui| {
        ui.set_width(WIDTH);
        acts = body(ui, &t, &mut state, &view);
    });
    if !open {
        return;
    }
    app.ui.import_window = Some(state);
    for act in acts {
        perform(app, act);
    }
}

fn perform(app: &mut FigApp, act: Act) {
    match act {
        Act::Source(package) => edit(app, |state| state.switch(Some(package))),
        Act::TryAgain => fetch(app, true),
        Act::OpenSettings => {
            app.ui.import_window = None;
            app.run(SETTINGS, json!({}));
        }
        Act::Import => import(app),
        Act::Another => edit(app, |state| state.run = Run::Form),
        Act::Show => {
            app.ui.import_window = None;
            app.run(SHOW, json!({}));
        }
        Act::Close => app.ui.import_window = None,
    }
}

fn import(app: &mut FigApp) {
    let request = app.ui.import_window.as_ref().and_then(|state| {
        let choice = state.choice.as_ref().filter(|choice| choice.reason().is_none())?;
        Some((choice.params(state.package.as_deref()?), choice.component.clone(), choice.totals().1))
    });
    let Some((params, component, renders)) = request else { return };
    if crate::menus::dev_mode_refuses(app, IMPORT) {
        return;
    }
    let run = match app.run(IMPORT, params) {
        Some(started) => Run::Importing { component, first_job: started.get("job").and_then(Value::as_u64).unwrap_or(0), renders },
        None => Run::Failed { component, cause: app.ui.last_error.clone().unwrap_or_default() },
    };
    edit(app, |state| state.run = run);
}

fn body(ui: &mut Ui, t: &Tokens, state: &mut ImportWindow, view: &View) -> Vec<Act> {
    let mut acts = Vec::new();
    if matches!(view.listing, Listing::NoSource) {
        no_source(ui, t, &mut acts);
    } else if let Run::Done { component, variants, warnings } = state.run.clone() {
        done(ui, t, &component, variants, warnings, &mut acts);
    } else {
        form(ui, t, state, view, &mut acts);
    }
    ui.add_space(12.0);
    footer(ui, state, view, &mut acts);
    acts
}

fn note(ui: &mut Ui, text: &str, color: egui::Color32) {
    ui.add(egui::Label::new(RichText::new(text).color(color).font(regular(11.0))).wrap());
}

fn heading(ui: &mut Ui, t: &Tokens, title: &str) {
    ui.label(RichText::new(title).font(semibold(11.0)).color(t.text_secondary));
}

fn no_source(ui: &mut Ui, t: &Tokens, acts: &mut Vec<Act>) {
    note(ui, tr("No source library yet. Add one in the project settings."), t.text_secondary);
    ui.add_space(8.0);
    if widgets::button(ui, tr("Open project settings"), true, true).clicked() {
        acts.push(Act::OpenSettings);
    }
}

fn done(ui: &mut Ui, t: &Tokens, component: &str, variants: usize, warnings: usize, acts: &mut Vec<Act>) {
    let summary = trf("Imported {name}: {variants} variants", &[("name", &component), ("variants", &variants)]);
    ui.label(RichText::new(summary).font(semibold(13.0)).color(t.text));
    if warnings > 0 {
        note(ui, &trf("Warnings: {n}", &[("n", &warnings)]), t.text_secondary);
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if widgets::button(ui, tr("Show in Components"), true, true).clicked() {
            acts.push(Act::Show);
        }
        if widgets::button(ui, tr("Import another"), false, true).clicked() {
            acts.push(Act::Another);
        }
    });
}

fn form(ui: &mut Ui, t: &Tokens, state: &mut ImportWindow, view: &View, acts: &mut Vec<Act>) {
    let running = matches!(state.run, Run::Importing { .. });
    ui.add_enabled_ui(!running, |ui| {
        source_row(ui, t, state, view, acts);
        ui.add_space(8.0);
        components(ui, t, state, view, acts);
        if let (Listing::Ready(_), Some(choice)) = (&view.listing, state.choice.as_mut()) {
            options(ui, t, choice);
        }
    });
    status(ui, t, state, view);
}

fn source_row(ui: &mut Ui, t: &Tokens, state: &ImportWindow, view: &View, acts: &mut Vec<Act>) {
    let Some(package) = state.package.as_deref() else { return };
    let version = view.sources.iter().find(|(name, _)| name == package).map_or("", |(_, version)| version.as_str());
    let shown = format!("{package} {version}");
    ui.horizontal(|ui| {
        heading(ui, t, tr("Source"));
        if view.sources.len() < 2 {
            ui.label(RichText::new(&shown).monospace().size(11.0).color(t.text));
            return;
        }
        let resp = widgets::dropdown(ui, &shown, SOURCE_W, None);
        a11y::combo(&resp, tr("Source"), &shown, a11y::menu_open(&resp));
        crate::right::menu_popup(&resp).show(|ui| {
            for (name, version) in &view.sources {
                if crate::right::menu_check(ui, name == package, &format!("{name} {version}"), "") {
                    acts.push(Act::Source(name.clone()));
                    ui.close();
                }
            }
        });
    });
}

fn components(ui: &mut Ui, t: &Tokens, state: &mut ImportWindow, view: &View, acts: &mut Vec<Act>) {
    heading(ui, t, tr("Components"));
    ui.add_space(4.0);
    let package = state.package.clone().unwrap_or_default();
    match &view.listing {
        Listing::Ready(found) => ready(ui, t, state, found),
        Listing::Failed(cause) => {
            note(ui, &trf("Couldn't read the components of {package}: {cause}", &[("package", &package), ("cause", cause)]), t.danger);
            ui.add_space(6.0);
            if widgets::button(ui, tr("Try again"), false, true).clicked() {
                acts.push(Act::TryAgain);
            }
        }
        Listing::Loading | Listing::NoSource => {
            note(ui, &trf("Reading the components of {package}…", &[("package", &package)]), t.text_secondary);
        }
    }
}

fn ready(ui: &mut Ui, t: &Tokens, state: &mut ImportWindow, found: &Inspected) {
    if found.components.is_empty() {
        note(ui, tr("This package exports no component."), t.text_secondary);
        for line in &found.notes {
            note(ui, line, t.text_tertiary);
        }
        return;
    }
    let width = ui.available_width();
    let search = widgets::text_input(ui, egui::Id::unique("import_window_search"), &mut state.search, width, tr("Search components"));
    a11y::text(&search, tr("Search components"), &state.search);
    if std::mem::take(&mut state.focus_search) {
        search.request_focus();
    }
    ui.add_space(6.0);
    let query = state.search.trim().to_lowercase();
    let listed: Vec<&InspectedComponent> = found.components.iter().filter(|component| component.name.to_lowercase().contains(&query)).collect();
    if listed.is_empty() {
        note(ui, tr("No matching components"), t.text_secondary);
        return;
    }
    let mut picked = None;
    let rows = listed.len();
    ScrollArea::vertical().id_salt("import_window_components").max_height(LIST_HEIGHT).auto_shrink([false, true]).show_rows(
        ui,
        ROW_H,
        rows,
        |ui, range| {
            for component in listed.get(range).into_iter().flatten().copied() {
                let chosen = state.choice.as_ref().is_some_and(|choice| choice.component == component.name);
                if component_row(ui, t, &component.name, chosen).clicked() {
                    picked = Some(component);
                }
            }
        },
    );
    if let Some(component) = picked {
        state.choice = Some(Choice::of(component));
        if matches!(state.run, Run::Failed { .. }) {
            state.run = Run::Form;
        }
    }
}

fn component_row(ui: &mut Ui, t: &Tokens, name: &str, chosen: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    a11y::list_item(&resp, name, chosen);
    if resp.gained_focus() {
        resp.scroll_to_me(None);
    }
    if chosen {
        ui.painter().rect_filled(rect, CornerRadius::same(5), t.selected_row);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(5), t.hover);
    }
    let painter = ui.painter().with_clip_rect(rect.shrink2(vec2(8.0, 0.0)));
    painter.text(pos2(rect.min.x + 8.0, rect.center().y), Align2::LEFT_CENTER, name, regular(11.0), t.text);
    resp
}

fn option(ui: &mut Ui, on: bool, label: &str) -> bool {
    let ink = Tokens::get(ui.ctx()).text;
    let width = ui.painter().layout_no_wrap(label.to_string(), regular(11.0), ink).size().x + OPTION_PADDING;
    ui.allocate_ui(vec2(width, FIELD_H), |ui| widgets::checkbox(ui, on, label)).inner
}

fn group(ui: &mut Ui, t: &Tokens, title: &str, add: impl FnOnce(&mut Ui)) {
    ui.add_space(8.0);
    heading(ui, t, title);
    ui.horizontal_wrapped(add);
}

fn picks_row(ui: &mut Ui, picks: &mut Picks) {
    for (value, on) in picks {
        if option(ui, *on, value) {
            *on = !*on;
        }
    }
}

fn options(ui: &mut Ui, t: &Tokens, choice: &mut Choice) {
    if !choice.styles.is_empty() {
        group(ui, t, tr("Styles"), |ui| picks_row(ui, &mut choice.styles));
    }
    if !choice.sizes.is_empty() {
        group(ui, t, tr("Sizes"), |ui| picks_row(ui, &mut choice.sizes));
    }
    group(ui, t, tr("States"), |ui| {
        for (state, on) in &mut choice.states {
            if option(ui, *on, state_label(state)) {
                *on = !*on;
            }
        }
    });
    group(ui, t, tr("Themes"), |ui| {
        if option(ui, choice.light, tr("Light")) {
            choice.light = !choice.light;
        }
        if option(ui, choice.dark, tr("Dark")) {
            choice.dark = !choice.dark;
        }
    });
    ui.add_space(8.0);
    if option(ui, choice.animations, tr("Animations")) {
        choice.animations = !choice.animations;
    }
    note(ui, tr("Transitions between states, ripple and spinner"), t.text_tertiary);
}

fn status(ui: &mut Ui, t: &Tokens, state: &ImportWindow, view: &View) {
    let (Some(choice), Listing::Ready(_)) = (&state.choice, &view.listing) else { return };
    ui.add_space(10.0);
    let (variants, renders) = choice.totals();
    note(ui, &trf("{variants} variants · {renders} renders", &[("variants", &variants), ("renders", &renders)]), t.text_secondary);
    if let Some(reason) = choice.reason() {
        note(ui, &reason, t.danger);
    }
    match &state.run {
        Run::Importing { .. } => {
            let (done, total) = view.progress;
            note(ui, &trf("Importing… {done} of {total}", &[("done", &done), ("total", &total)]), t.text);
        }
        Run::Failed { component, cause } => {
            note(ui, &trf("Couldn't import {name}: {cause}", &[("name", component), ("cause", cause)]), t.danger);
        }
        Run::Form | Run::Done { .. } => {}
    }
}

fn footer(ui: &mut Ui, state: &ImportWindow, view: &View, acts: &mut Vec<Act>) {
    let ready = matches!(view.listing, Listing::Ready(_)) && matches!(state.run, Run::Form | Run::Failed { .. });
    let importable = ready && state.choice.as_ref().is_some_and(|choice| choice.reason().is_none());
    let offered = !matches!(view.listing, Listing::NoSource) && !matches!(state.run, Run::Done { .. });
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if offered && widgets::button(ui, tr("Import"), true, importable).clicked() {
            acts.push(Act::Import);
        }
        if widgets::button(ui, tr("Close"), false, true).clicked() {
            acts.push(Act::Close);
        }
    });
}
