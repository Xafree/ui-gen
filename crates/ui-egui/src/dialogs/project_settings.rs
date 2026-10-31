//! File ▸ Project settings…: the window of a library project. Its "Source libraries" section adds
//! an npm package, changes its version and removes it through the `project.*` commands; the
//! capture service works in the background, so the rows follow the capture jobs of the project and its
//! sources and no frame waits. Programmatic `project.settings` calls never open it.

use egui::{Label, RichText, Ui};
use figcraft_engine::capture::JobState;
use figcraft_engine::{EngineError, file_access};
use figcraft_project::ProjectError;
use figcraft_project::npm;
use serde_json::json;

pub(in crate::dialogs) mod page;

use super::project_themes::{self, ThemeAct, Themes};
use crate::FigApp;
use crate::i18n::{tr_noop, trf};
use crate::theme::{Tokens, regular};

const ADD: &str = "project.addSource";
const ADD_MANY: &str = "project.addSources";
const SET_VERSION: &str = "project.setSourceVersion";
const REMOVE: &str = "project.removeSource";
const IMPORT: &str = "uigen.importComponent";

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

#[derive(Clone, Debug)]
enum RunState {
    Running,
    Done,
    Failed(String),
}

#[derive(Clone, Debug)]
struct LastRun {
    command: String,
    packages: Vec<String>,
    state: RunState,
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
    project_name: String,
    package_input: String,
    last_run: Option<LastRun>,
    add_error: Option<String>,
    pending_removal: Option<String>,
    changing: Option<(String, String)>,
    change_error: Option<String>,
    focus_first: bool,
    pub(super) themes: Themes,
}

enum Act {
    Add,
    Browse(Option<String>),
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
        self.project_name = binding.config.name.clone();
        if let Some(run) = self.last_run.as_mut()
            && let Some(job) = jobs
                .iter()
                .rev()
                .find(|j| j.command == ADD && j.packages.len() == run.packages.len() && run.packages.iter().all(|p| j.packages.contains(p)))
        {
            run.state = match job.state.as_str() {
                "failed" => RunState::Failed(job.error.clone()),
                "done" => RunState::Done,
                _ => RunState::Running,
            };
        }
        self.project_name = binding.config.name.clone();
        if let Some(run) = self.last_run.as_mut()
            && let Some(job) = jobs
                .iter()
                .rev()
                .find(|j| j.command == ADD && j.packages.len() == run.packages.len() && run.packages.iter().all(|p| j.packages.contains(p)))
        {
            run.state = match job.state.as_str() {
                "failed" => RunState::Failed(job.error.clone()),
                "done" => RunState::Done,
                _ => RunState::Running,
            };
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
    let parsed = npm::parse_install_line(line);
    let refusal = match &parsed {
        Err(error) => Some(refusal_text(error)),
        Ok(_) => match app.session.execute(ADD_MANY, &json!({"line": line})) {
            Ok(_) => None,
            Err(EngineError::BadParams { msg, .. }) => Some(msg),
            Err(other) => Some(other.to_string()),
        },
    };
    let run = parsed.ok().filter(|_| refusal.is_none()).map(|specs| LastRun {
        command: specs.iter().fold("npm install".to_string(), |line, spec| {
            if spec.version == "latest" { format!("{line} {}", spec.name) } else { format!("{line} {}@{}", spec.name, spec.version) }
        }),
        packages: specs.into_iter().map(|spec| spec.name).collect(),
        state: RunState::Running,
    });
    edit(app, |state| {
        if refusal.is_none() {
            state.package_input.clear();
        }
        if run.is_some() {
            state.last_run = run;
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
        Act::Browse(package) => super::import_component::request(app, package.as_deref()),
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

pub fn show(app: &mut FigApp, ui: &mut Ui) {
    let Some(mut state) = app.ui.project_settings.take() else { return };
    if !state.refresh(app) {
        return;
    }
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let can_install = !cfg!(target_arch = "wasm32");
    let focus_first = state.focus_first;
    let mut acts = Vec::new();
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.bg)).show(ui, |ui| {
        acts = page::show(ui, &t, &mut state, can_install, focus_first);
    });
    state.focus_first &= !ctx.memory(|m| m.has_focus(package_field()));
    app.ui.project_settings = Some(state);
    for act in acts {
        perform(app, act);
    }
}

pub(super) fn note(ui: &mut Ui, text: &str, color: egui::Color32) {
    ui.add(Label::new(RichText::new(text).color(color).font(regular(12.5))).wrap());
}
