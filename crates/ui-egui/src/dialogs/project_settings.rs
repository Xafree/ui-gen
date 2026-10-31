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
use crate::source_family::{SourceFacts, dependencies_of, group_sources};
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
pub struct FamilyRow {
    pub name: String,
    pub version: String,
    pub framework: String,
    pub license: String,
    pub status: String,
    work: Work,
    pub packages: Vec<String>,
    active: Vec<String>,
}

impl FamilyRow {
    pub fn head(&self) -> &str {
        self.packages.first().map_or("", String::as_str)
    }
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
    families: Vec<FamilyRow>,
    failures: Vec<Failure>,
    project_name: String,
    root: String,
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

    pub fn families(&self) -> &[FamilyRow] {
        &self.families
    }

    /// What Escape ends before the page: a removal being asked, a version being changed, a typed line.
    pub(in crate::dialogs) fn takes_escape(&mut self, ctx: &egui::Context) -> bool {
        if self.pending_removal.take().is_some() {
            return true;
        }
        if self.changing.take().is_some() {
            self.change_error = None;
            return true;
        }
        let typed = page::had_focus(ctx, package_field()) && !self.package_input.is_empty();
        if typed {
            self.package_input.clear();
            self.add_error = None;
        }
        typed
    }

    fn removal(&self, package: &str) -> (Vec<String>, Vec<String>) {
        let Some(family) = self.families.iter().find(|family| family.packages.iter().any(|p| p == package)) else {
            return (vec![package.to_string()], Vec::new());
        };
        let needed: Vec<String> = self
            .families
            .iter()
            .filter(|other| other.name != family.name && other.status != "removed")
            .filter_map(|other| self.sources.iter().find(|row| row.package == other.head()))
            .flat_map(|head| dependencies_of(&self.root, &head.package, &head.version))
            .collect();
        family.active.iter().cloned().partition(|member| member == family.head() || !needed.contains(member))
    }

    pub(in crate::dialogs) fn removal_note(&self, package: &str) -> String {
        let (goes, stays) = self.removal(package);
        let mut note = trf("Removes {packages}", &[("packages", &goes.join(", "))]);
        if !stays.is_empty() {
            note.push_str(" · ");
            note.push_str(&trf("Keeps {packages}: another library needs them", &[("packages", &stays.join(", "))]));
        }
        note
    }

    fn version_targets(&self, package: &str) -> Vec<String> {
        let Some(family) = self.families.iter().find(|family| family.packages.iter().any(|p| p == package)) else {
            return vec![package.to_string()];
        };
        let shares_head = |name: &&String| self.sources.iter().any(|row| row.package == **name && row.version == family.version);
        let same: Vec<String> = family.active.iter().filter(shares_head).cloned().collect();
        if same.is_empty() { family.active.iter().take(1).cloned().collect() } else { same }
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
        self.root = binding.root.clone();
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
        self.families = family_rows(&binding.root, &rows);
        self.sources = rows;
        self.failures = failures;
        self.themes.refresh(app, &binding.root, &binding.config);
        true
    }

    fn failure(&self, package: &str) -> Option<String> {
        self.failures.iter().find(|f| f.packages.iter().any(|p| p == package)).map(|f| failure_text(&f.command, &f.packages.join(", "), &f.message))
    }
}

fn family_rows(root: &str, rows: &[SourceRow]) -> Vec<FamilyRow> {
    let dependencies: Vec<Vec<String>> =
        rows.iter().map(|r| if r.status == "removed" { Vec::new() } else { dependencies_of(root, &r.package, &r.version) }).collect();
    let borrowed: Vec<Vec<&str>> = dependencies.iter().map(|list| list.iter().map(String::as_str).collect()).collect();
    let facts: Vec<SourceFacts> = rows
        .iter()
        .zip(&borrowed)
        .map(|(r, depends_on)| SourceFacts {
            package: &r.package,
            version: &r.version,
            framework: &r.framework,
            components: usize::from(!r.framework.is_empty()),
            depends_on,
        })
        .collect();
    group_sources(&facts)
        .into_iter()
        .map(|family| {
            let members: Vec<&SourceRow> = family.packages.iter().filter_map(|p| rows.iter().find(|r| r.package == *p)).collect();
            let head = members.first();
            let any = |status: &str| members.iter().any(|r| r.status == status);
            let status = if members.iter().all(|r| r.status == "removed") {
                "removed"
            } else if any("installing") {
                "installing"
            } else {
                "ready"
            };
            let work = [Work::Removing, Work::Installing].into_iter().find(|w| members.iter().any(|r| r.work == *w)).unwrap_or(Work::Idle);
            FamilyRow {
                name: family.name,
                version: family.version,
                framework: head.map(|r| r.framework.clone()).unwrap_or_default(),
                license: head.map(|r| r.license.clone()).unwrap_or_default(),
                status: status.to_string(),
                work,
                active: members.iter().filter(|r| r.status != "removed").map(|r| r.package.clone()).collect(),
                packages: family.packages,
            }
        })
        .collect()
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
        for window in crate::windows::all_ids(app) {
            crate::windows::as_window(app, window, |app| {
                edit(app, |state| state.themes.invalidate());
                sync(app);
            });
        }
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
    if let Some(text) = super::install_refusal::text(error) {
        return text;
    }
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
    let packages = app.ui.project_settings.as_ref().map_or_else(|| vec![package.to_string()], |state| state.version_targets(package));
    let params = match packages.as_slice() {
        [single] => json!({"package": single, "version": version}),
        _ => json!({"packages": packages, "version": version}),
    };
    if checked.is_ok() && app.run(SET_VERSION, params).is_some() {
        edit(app, |state| state.changing = None);
    }
    sync(app);
}

fn remove_package(app: &mut FigApp, package: &str) {
    let packages = app.ui.project_settings.as_ref().map_or_else(|| vec![package.to_string()], |state| state.removal(package).0);
    edit(app, |state| state.pending_removal = None);
    for package in packages {
        app.run(REMOVE, json!({"package": package}));
    }
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
