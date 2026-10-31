//! File ▸ Project settings…: the window of a library project. Its "Source libraries" section adds
//! an npm package, changes its version and removes it through the `project.*` commands; the
//! capture service works in the background, so the rows follow the capture jobs of the project and its
//! sources and no frame waits. Programmatic `project.settings` calls never open it.

use egui::{Align, Context, CornerRadius, Label, Layout, Margin, RichText, ScrollArea, Stroke, Ui, vec2};
use figcraft_engine::capture::JobState;
use figcraft_engine::{EngineError, file_access};
use figcraft_project::ProjectError;
use figcraft_project::npm;
use serde_json::json;

use super::project_themes::{self, ThemeAct, Themes};
use super::window;
use crate::FigApp;
use crate::i18n::{tr, tr_noop, trf};
use crate::theme::{Tokens, regular, semibold};

const ADD: &str = "project.addSource";
const ADD_MANY: &str = "project.addSources";
const SET_VERSION: &str = "project.setSourceVersion";
const REMOVE: &str = "project.removeSource";
const IMPORT: &str = "uigen.importComponent";

const PACKAGE_W: f32 = 180.0;
const VERSION_W: f32 = 90.0;
const FRAMEWORK_W: f32 = 100.0;
const LICENSE_W: f32 = 70.0;
const ACTIONS_W: f32 = 220.0;
pub(super) const GAP: f32 = 8.0;
pub(super) const CARD_MARGIN: i8 = 10;
const CELL_H: f32 = 28.0;
const LIST_HEIGHT: f32 = 320.0;
pub(super) const INNER_W: f32 = PACKAGE_W + VERSION_W + FRAMEWORK_W + LICENSE_W + ACTIONS_W + 4.0 * GAP;
const WIDTH: f32 = INNER_W + 2.0 * (CARD_MARGIN as f32 + 1.0);
const ADD_PACKAGE_W: f32 = 360.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Work {
    Idle,
    Installing,
    Removing,
}

#[derive(Clone, Debug)]
pub struct SourceRow {
    pub package: String,
    pub version: String,
    pub framework: String,
    pub license: String,
    pub status: String,
    work: Work,
}

#[derive(Clone, Debug)]
struct Failure {
    packages: Vec<String>,
    command: String,
    message: String,
}

struct Job {
    command: String,
    packages: Vec<String>,
    state: String,
    error: String,
}

#[derive(Clone, Debug, Default)]
pub struct ProjectSettings {
    sources: Vec<SourceRow>,
    failures: Vec<Failure>,
    package_input: String,
    add_error: Option<String>,
    pending_removal: Option<String>,
    changing: Option<(String, String)>,
    change_error: Option<String>,
    focus_first: bool,
    pub(super) themes: Themes,
}

enum Act {
    Package(String),
    Add,
    AskRemoval(String),
    CancelRemoval,
    ConfirmRemoval(String),
    Change(String),
    Draft(String),
    Apply(String),
    CancelChange,
    Close,
    Themes(ThemeAct),
}

impl ProjectSettings {
    pub fn sources(&self) -> &[SourceRow] {
        &self.sources
    }

    pub fn add_error(&self) -> Option<&str> {
        self.add_error.as_deref()
    }

    pub fn pending_removal(&self) -> Option<&str> {
        self.pending_removal.as_deref()
    }

    fn refresh(&mut self, app: &mut FigApp) -> bool {
        let Some(binding) = app.session.project().cloned() else { return false };
        let jobs = jobs(app, &binding.root);
        let working = |package: &str, command: &str| {
            jobs.iter().any(|j| j.packages.iter().any(|p| p == package) && j.command == command && matches!(j.state.as_str(), "queued" | "running"))
        };
        let mut rows: Vec<SourceRow> = binding
            .config
            .sources
            .iter()
            .map(|source| {
                let status = match (working(&source.package, ADD), source.removed) {
                    (true, _) => "installing",
                    (false, true) => "removed",
                    (false, false) => "ready",
                };
                let work = if working(&source.package, SET_VERSION) {
                    Work::Installing
                } else if working(&source.package, REMOVE) {
                    Work::Removing
                } else {
                    Work::Idle
                };
                SourceRow {
                    package: source.package.clone(),
                    version: source.version.clone(),
                    framework: source.framework.clone().unwrap_or_default(),
                    license: source.license.clone().unwrap_or_default(),
                    status: status.to_string(),
                    work,
                }
            })
            .collect();
        for package in jobs.iter().filter(|j| j.command == ADD).flat_map(|j| &j.packages) {
            if working(package, ADD) && !rows.iter().any(|r| r.package == *package) {
                rows.push(SourceRow {
                    package: package.clone(),
                    version: String::new(),
                    framework: String::new(),
                    license: String::new(),
                    status: "installing".to_string(),
                    work: Work::Installing,
                });
            }
        }
        let mut failures: Vec<Failure> = Vec::new();
        for job in &jobs {
            failures.retain(|f| !f.packages.iter().any(|p| job.packages.contains(p)));
            if job.state == "failed" {
                failures.push(Failure { packages: job.packages.clone(), command: job.command.clone(), message: job.error.clone() });
            }
        }
        self.sources = rows;
        self.failures = failures;
        self.themes.refresh(app, &binding.root, &binding.config);
        true
    }

    fn failure(&self, package: &str) -> Option<String> {
        self.failures.iter().find(|f| f.packages.iter().any(|p| p == package)).map(|f| failure_text(&f.command, &f.packages.join(", "), &f.message))
    }
}

fn jobs(app: &FigApp, root: &str) -> Vec<Job> {
    app.session
        .capture_jobs()
        .into_iter()
        .filter(|job| [ADD, ADD_MANY, SET_VERSION, REMOVE].contains(&job.command.as_str()) && file_access::same_file(&job.root, root))
        .map(|job| {
            let (state, error) = match job.state {
                JobState::Queued => ("queued", String::new()),
                JobState::Running => ("running", String::new()),
                JobState::Done => ("done", String::new()),
                JobState::Failed(message) => ("failed", message),
            };
            let command = if job.command == ADD_MANY { ADD.to_string() } else { job.command };
            Job { command, packages: job.packages, state: state.to_string(), error }
        })
        .collect()
}

fn failure_text(command: &str, package: &str, cause: &str) -> String {
    let key = if command == REMOVE { tr_noop("Couldn't remove {package}: {cause}") } else { tr_noop("Couldn't install {package}: {cause}") };
    trf(key, &[("package", &package), ("cause", &cause)])
}

/// `dialog.projectSettings`: open the window for the project of the file in front.
pub(crate) fn request(app: &mut FigApp) {
    let mut state = ProjectSettings { focus_first: true, ..Default::default() };
    if state.refresh(app) {
        app.ui.project_settings = Some(state);
    }
}

/// Bring back the jobs the capture service finished and say how each ended.
pub(crate) fn pump(app: &mut FigApp) {
    let mut imported = false;
    for event in app.session.pump_capture() {
        imported |= event.command == IMPORT;
        if crate::playground_bars::note_event(app, &event) || super::import_component::pump_event(app, &event) {
            continue;
        }
        let package = event.package.as_str();
        if event.command == IMPORT {
            match &event.outcome {
                Ok(_) => app.toast(trf("Imported a component from {package}", &[("package", &package)])),
                Err(error) => {
                    log::warn!("{IMPORT} {package}: {}", error.message);
                    app.show_error(trf("Couldn't import from {package}: {cause}", &[("package", &package), ("cause", &error.message)]));
                }
            }
            continue;
        }
        if ![ADD, ADD_MANY, SET_VERSION, REMOVE].contains(&event.command.as_str()) {
            continue;
        }
        match &event.outcome {
            Ok(_) if event.command == REMOVE => app.toast(trf("Removed {package}", &[("package", &package)])),
            Ok(_) => app.toast(trf("Installed {package}", &[("package", &package)])),
            Err(error) => {
                log::warn!("{} {package}: {}", event.command, error.message);
                app.show_error(failure_text(&event.command, package, &error.message));
            }
        }
    }
    if imported {
        edit(app, |state| state.themes.invalidate());
        sync(app);
    }
}

fn edit(app: &mut FigApp, change: impl FnOnce(&mut ProjectSettings)) {
    if let Some(state) = app.ui.project_settings.as_mut() {
        change(state);
    }
}

fn sync(app: &mut FigApp) {
    let Some(mut state) = app.ui.project_settings.take() else { return };
    if state.refresh(app) {
        app.ui.project_settings = Some(state);
    }
}

fn refusal_text(error: &ProjectError) -> String {
    match error {
        ProjectError::Invalid { field, reason } if field == "line" => reason.clone(),
        ProjectError::Invalid { field, reason } => format!("{field}: {reason}"),
        other => other.to_string(),
    }
}

/// Install the packages typed on `line` (`name`, `name@1.2.0`, after an optional `npm install`) as
/// sources of the project in one go. A refused line starts nothing and keeps what was typed.
pub fn add_packages(app: &mut FigApp, line: &str) {
    let refusal = match npm::parse_install_line(line) {
        Err(error) => Some(refusal_text(&error)),
        Ok(_) => match app.session.execute(ADD_MANY, &json!({"line": line})) {
            Ok(_) => None,
            Err(EngineError::BadParams { msg, .. }) => Some(msg),
            Err(other) => Some(other.to_string()),
        },
    };
    edit(app, |state| {
        if refusal.is_none() {
            state.package_input.clear();
        }
        state.add_error = refusal;
    });
    sync(app);
}

/// Install another version of source `package`; the row shows it once the install has succeeded.
pub fn change_version(app: &mut FigApp, package: &str, version: &str) {
    let version = version.trim();
    let checked = npm::validate_version(version, true);
    edit(app, |state| state.change_error = checked.as_ref().err().map(ToString::to_string));
    if checked.is_ok() && app.run(SET_VERSION, json!({"package": package, "version": version})).is_some() {
        edit(app, |state| state.changing = None);
    }
    sync(app);
}

fn remove_package(app: &mut FigApp, package: &str) {
    edit(app, |state| state.pending_removal = None);
    app.run(REMOVE, json!({"package": package}));
    sync(app);
}

fn perform(app: &mut FigApp, act: Act) {
    match act {
        Act::Package(text) => edit(app, |s| s.package_input = text),
        Act::Add => {
            let Some(state) = app.ui.project_settings.as_ref() else { return };
            let line = state.package_input.clone();
            add_packages(app, &line);
        }
        Act::AskRemoval(package) => edit(app, |s| {
            s.pending_removal = Some(package);
            s.changing = None;
        }),
        Act::CancelRemoval => edit(app, |s| s.pending_removal = None),
        Act::ConfirmRemoval(package) => remove_package(app, &package),
        Act::Change(package) => edit(app, |s| {
            let current = s.sources.iter().find(|r| r.package == package).map(|r| r.version.clone()).unwrap_or_default();
            s.changing = Some((package, current));
            s.change_error = None;
            s.pending_removal = None;
        }),
        Act::Draft(text) => edit(app, |s| {
            if let Some((_, draft)) = s.changing.as_mut() {
                *draft = text;
            }
        }),
        Act::Apply(package) => {
            let draft = app.ui.project_settings.as_ref().and_then(|s| s.changing.as_ref()).map(|(_, draft)| draft.clone()).unwrap_or_default();
            change_version(app, &package, &draft);
        }
        Act::CancelChange => edit(app, |s| {
            s.changing = None;
            s.change_error = None;
        }),
        Act::Close => app.ui.project_settings = None,
        Act::Themes(act) => {
            project_themes::perform(app, act);
            sync(app);
        }
    }
}

fn package_field() -> egui::Id {
    egui::Id::unique(("project_settings", "package"))
}

pub fn show(app: &mut FigApp, ctx: &Context) {
    let Some(mut state) = app.ui.project_settings.take() else { return };
    if !state.refresh(app) {
        return;
    }
    let t = Tokens::get(ctx);
    let can_install = !cfg!(target_arch = "wasm32");
    let mut open = true;
    let mut acts = Vec::new();
    let focus_first = state.focus_first;
    window(tr("Project settings"), ctx).open(&mut open).show(ctx, |ui| {
        ui.set_width(WIDTH);
        acts = body(ui, &t, &state, can_install, focus_first);
    });
    if !open {
        return;
    }
    state.focus_first &= !crate::widgets::text_field_focused(ctx, package_field());
    app.ui.project_settings = Some(state);
    for act in acts {
        perform(app, act);
    }
}

pub(super) fn note(ui: &mut Ui, text: &str, color: egui::Color32) {
    ui.add(Label::new(RichText::new(text).color(color).font(regular(11.0))).wrap());
}

fn body(ui: &mut Ui, t: &Tokens, state: &ProjectSettings, can_install: bool, focus_first: bool) -> Vec<Act> {
    let mut acts = Vec::new();
    ui.label(RichText::new(tr("Source libraries")).font(semibold(15.0)).color(t.text));
    if !can_install {
        note(ui, tr("Installing packages is only available in the desktop app"), t.text_secondary);
    }
    ui.add_space(8.0);
    add_row(ui, state, can_install, focus_first, &mut acts);
    if let Some(error) = state.add_error() {
        note(ui, error, t.danger);
    }
    ui.add_space(12.0);
    table(ui, t, state, can_install, &mut acts);
    ui.add_space(12.0);
    if can_install {
        let mut themes = Vec::new();
        project_themes::section(ui, t, &state.themes, &mut themes);
        acts.extend(themes.into_iter().map(Act::Themes));
        ui.add_space(12.0);
    }
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if crate::widgets::button(ui, tr("Close"), false, true).clicked() {
            acts.push(Act::Close);
        }
    });
    acts
}

fn add_row(ui: &mut Ui, state: &ProjectSettings, can_install: bool, focus_first: bool, acts: &mut Vec<Act>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        let package = package_field();
        if focus_first {
            crate::widgets::focus_text_field(ui, package);
        }
        if let Some(text) = crate::widgets::text_field(ui, package, &state.package_input, ADD_PACKAGE_W, tr("Package")) {
            acts.push(Act::Package(text));
        }
        if crate::widgets::button(ui, tr("Run"), true, can_install).clicked() {
            acts.push(Act::Add);
        }
    });
}

fn cell(ui: &mut Ui, width: f32, add: impl FnOnce(&mut Ui)) {
    ui.allocate_ui_with_layout(vec2(width, CELL_H), Layout::left_to_right(Align::Center), |ui| {
        ui.set_min_size(vec2(width, CELL_H));
        ui.set_max_width(width);
        add(ui);
    });
}

fn text_cell(ui: &mut Ui, width: f32, text: RichText) {
    cell(ui, width, |ui| {
        ui.add(Label::new(text).truncate());
    });
}

fn table(ui: &mut Ui, t: &Tokens, state: &ProjectSettings, can_install: bool, acts: &mut Vec<Act>) {
    let orphans: Vec<String> = state
        .failures
        .iter()
        .filter(|f| !f.packages.iter().any(|p| state.sources.iter().any(|r| r.package == *p)))
        .map(|f| failure_text(&f.command, &f.packages.join(", "), &f.message))
        .collect();
    if state.sources.is_empty() && orphans.is_empty() {
        note(ui, tr("No source library yet"), t.text_secondary);
        return;
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        ui.add_space(f32::from(CARD_MARGIN) + 1.0);
        for (width, title) in [(PACKAGE_W, tr("Package")), (VERSION_W, tr("Version")), (FRAMEWORK_W, tr("Framework")), (LICENSE_W, tr("License"))] {
            text_cell(ui, width, RichText::new(title).color(t.text_tertiary).font(regular(11.0)));
        }
    });
    ScrollArea::vertical().id_salt("project_settings_sources").max_height(LIST_HEIGHT).auto_shrink([false, true]).show(ui, |ui| {
        for row in &state.sources {
            card(ui, t, state, row, can_install, acts);
            ui.add_space(6.0);
        }
        for message in &orphans {
            note(ui, message, t.danger);
        }
    });
}

fn card(ui: &mut Ui, t: &Tokens, state: &ProjectSettings, row: &SourceRow, can_install: bool, acts: &mut Vec<Act>) {
    let removed = row.status == "removed";
    let ink = if removed { t.text_tertiary } else { t.text };
    let secondary = if removed { t.text_tertiary } else { t.text_secondary };
    egui::Frame::new().stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(CARD_MARGIN)).show(
        ui,
        |ui| {
            ui.set_width(INNER_W);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = GAP;
                text_cell(ui, PACKAGE_W, RichText::new(&row.package).monospace().size(11.0).color(ink));
                version_cell(ui, t, row, removed);
                text_cell(ui, FRAMEWORK_W, RichText::new(or_dash(&row.framework)).color(secondary).font(regular(11.0)));
                text_cell(ui, LICENSE_W, RichText::new(or_dash(&row.license)).color(secondary).font(regular(11.0)));
                cell(ui, ACTIONS_W, |ui| actions(ui, t, state, row, can_install, acts));
            });
            if let Some(message) = state.failure(&row.package) {
                note(ui, &message, t.danger);
            }
            if state.changing.as_ref().is_some_and(|(package, _)| *package == row.package)
                && let Some(error) = &state.change_error
            {
                note(ui, error, t.danger);
            }
        },
    );
}

fn or_dash(text: &str) -> &str {
    if text.is_empty() { "—" } else { text }
}

fn version_cell(ui: &mut Ui, t: &Tokens, row: &SourceRow, removed: bool) {
    if row.status == "installing" {
        text_cell(ui, VERSION_W, RichText::new(tr("Installing…")).color(t.text_secondary).font(regular(11.0)));
    } else if removed {
        text_cell(ui, VERSION_W, RichText::new(&row.version).monospace().size(11.0).color(t.text_tertiary));
    } else {
        cell(ui, VERSION_W, |ui| {
            egui::Frame::new().fill(t.field).corner_radius(CornerRadius::same(5)).inner_margin(Margin::symmetric(8, 4)).show(ui, |ui| {
                ui.add(Label::new(RichText::new(&row.version).monospace().size(11.0).color(t.text)).truncate());
            });
        });
    }
}

fn actions(ui: &mut Ui, t: &Tokens, state: &ProjectSettings, row: &SourceRow, can_install: bool, acts: &mut Vec<Act>) {
    ui.spacing_mut().item_spacing.x = GAP;
    let dim = |text: &str| RichText::new(text.to_string()).color(t.text_tertiary).font(regular(11.0));
    if row.status == "removed" {
        ui.label(dim(tr("Removed source")));
        return;
    }
    if row.status == "installing" {
        return;
    }
    match row.work {
        Work::Installing => {
            ui.label(dim(tr("Installing…")));
            return;
        }
        Work::Removing => {
            ui.label(dim(tr("Removing…")));
            return;
        }
        Work::Idle => {}
    }
    if state.pending_removal() == Some(row.package.as_str()) {
        if crate::widgets::button(ui, tr("Confirm removal"), true, can_install).clicked() {
            acts.push(Act::ConfirmRemoval(row.package.clone()));
        }
        if crate::widgets::button(ui, tr("Cancel"), false, true).clicked() {
            acts.push(Act::CancelRemoval);
        }
    } else if let Some((_, draft)) = state.changing.as_ref().filter(|(package, _)| *package == row.package) {
        let field = egui::Id::unique(("project_settings_change", row.package.as_str()));
        if let Some(text) = crate::widgets::text_field(ui, field, draft, VERSION_W, tr("Version")) {
            acts.push(Act::Draft(text));
        }
        if crate::widgets::button(ui, tr("Apply"), true, can_install).clicked() {
            acts.push(Act::Apply(row.package.clone()));
        }
        if crate::widgets::button(ui, tr("Cancel"), false, true).clicked() {
            acts.push(Act::CancelChange);
        }
    } else {
        if crate::widgets::button(ui, tr("Change version"), false, can_install).clicked() {
            acts.push(Act::Change(row.package.clone()));
        }
        if crate::widgets::button(ui, tr("Remove"), false, can_install).clicked() {
            acts.push(Act::AskRemoval(row.package.clone()));
        }
    }
}
