//! File ▸ Import a component…: the "Choose your components" surface. It lists the components
//! `uigen.listComponents` read for every source library of the project, one card per library
//! family, and runs `uigen.importComponent` with `stage: true` for each ticked component, one after
//! another, until the set is in the import page. Only a user opens it (`dialog.importComponent`);
//! the control channel never does.

use std::collections::VecDeque;
use std::sync::Arc;

use egui::Ui;
use figcraft_engine::capture::{CaptureEvent, CaptureOutcome};
use figcraft_engine::{file_access, Inspected, InspectedComponent, MAX_RENDERS, PropKind, render_count};
use serde_json::{Value, json};

use crate::FigApp;
use crate::i18n::{tr, trf};
use crate::source_family::{Family, SourceFacts, dependencies_of, group_sources};
use crate::theme::Tokens;

mod view;

use view::{Act, CardView, Model, Progress, RowView, STATES, Status};

const LIST: &str = "uigen.listComponents";
const IMPORT: &str = "uigen.importComponent";
const SETTINGS: &str = "project.settings";
const SHOW: &str = "view.assets";

#[derive(Clone, Debug)]
struct Read {
    package: String,
    version: String,
    root: String,
    job: Option<u64>,
    failure: Option<String>,
}

#[derive(Clone, Debug)]
struct Batch {
    doc: u64,
    root: String,
    queue: VecDeque<(String, Value)>,
    current: Option<(String, u64)>,
    total: usize,
    done: usize,
    failures: Vec<String>,
}

#[derive(Clone, Debug, Default)]
enum Run {
    #[default]
    Idle,
    Importing(Batch),
    Summary {
        doc: u64,
        done: usize,
        total: usize,
        failures: Vec<String>,
    },
}

#[derive(Clone, Debug)]
pub struct ImportWindow {
    first: Option<String>,
    reads: Vec<Read>,
    search: String,
    ticked: Vec<(String, String)>,
    states: [bool; 5],
    light: bool,
    dark: bool,
    animations: bool,
    message: Option<String>,
    run: Run,
    just_opened: bool,
}

impl ImportWindow {
    fn new(first: Option<&str>) -> Self {
        Self {
            first: first.map(str::to_string),
            reads: Vec::new(),
            search: String::new(),
            ticked: Vec::new(),
            states: [true; 5],
            light: true,
            dark: true,
            animations: true,
            message: None,
            run: Run::Idle,
            just_opened: true,
        }
    }
}

struct Source {
    package: String,
    version: String,
    framework: String,
}

/// `dialog.importComponent`: open the surface for the project of the file in front, on `package`.
pub(crate) fn request(app: &mut FigApp, package: Option<&str>) {
    if cfg!(target_arch = "wasm32") {
        return;
    }
    app.ui.import_window = Some(ImportWindow::new(package));
}

/// Take the capture events the surface shows; `false` leaves the event to the toast.
pub(crate) fn pump_event(app: &mut FigApp, event: &CaptureEvent) -> bool {
    let Some(state) = app.ui.import_window.as_mut() else { return false };
    match (event.command.as_str(), &event.outcome) {
        (LIST, outcome) => match state.reads.iter_mut().find(|read| read.job == Some(event.job)) {
            Some(read) => {
                read.job = None;
                read.failure = read_failure(outcome, &read.version);
                true
            }
            None => false,
        },
        (IMPORT, outcome) => finish(app, event.job, outcome),
        _ => false,
    }
}

fn read_failure(outcome: &CaptureOutcome, asked: &str) -> Option<String> {
    match outcome {
        Ok(result) => {
            let reported = result.get("version").and_then(Value::as_str);
            (reported.is_some() && reported != Some(asked)).then(|| tr("The version read is not the one the project asks for").to_string())
        }
        Err(error) => Some(error.message.clone()),
    }
}

fn finish(app: &mut FigApp, job: u64, outcome: &CaptureOutcome) -> bool {
    let Some(Run::Importing(batch)) = app.ui.import_window.as_mut().map(|state| &mut state.run) else { return false };
    let Some((name, _)) = batch.current.take_if(|current| current.1 == job) else { return false };
    batch.done += 1;
    if let Err(error) = outcome {
        log::warn!("{IMPORT} {name}: {}", error.message);
        batch.failures.push(trf("Couldn't import {name}: {cause}", &[("name", &name), ("cause", &error.message)]));
    }
    start_next(app);
    true
}

/// Run `work` with file `doc` in front, then bring back the file that was; `None` once it is closed.
fn on_doc<R>(app: &mut FigApp, doc: u64, work: impl FnOnce(&mut FigApp) -> R) -> Option<R> {
    let at = app.session.docs.iter().position(|ds| ds.uid == doc)?;
    let front = app.session.ds().ok().map(|ds| ds.uid);
    app.session.active = at;
    let result = work(app);
    if let Some(back) = front.and_then(|uid| app.session.docs.iter().position(|ds| ds.uid == uid)) {
        app.session.active = back;
    }
    Some(result)
}

fn still_in_project(app: &FigApp, doc: u64, root: &str) -> bool {
    let binding = app.session.docs.iter().find(|ds| ds.uid == doc).and_then(|ds| ds.project.as_ref());
    binding.is_some_and(|binding| file_access::same_file(&binding.root, root))
}

fn abandon(batch: &mut Batch, first: String) {
    let cause = tr("the file it was started from is closed");
    for name in std::iter::once(first).chain(batch.queue.drain(..).map(|(name, _)| name)) {
        batch.done += 1;
        batch.failures.push(trf("Couldn't import {name}: {cause}", &[("name", &name), ("cause", &cause)]));
    }
}

fn start_next(app: &mut FigApp) {
    loop {
        let (doc, root, next) = match app.ui.import_window.as_mut().map(|state| &mut state.run) {
            Some(Run::Importing(batch)) => (batch.doc, batch.root.clone(), batch.queue.pop_front()),
            _ => return,
        };
        let Some((name, params)) = next else {
            conclude(app);
            return;
        };
        if !still_in_project(app, doc, &root) {
            if let Some(Run::Importing(batch)) = app.ui.import_window.as_mut().map(|state| &mut state.run) {
                abandon(batch, name);
            }
            conclude(app);
            return;
        }
        let started = on_doc(app, doc, |app| app.run(IMPORT, params)).flatten();
        let cause = if started.is_none() { Some(app.ui.last_error.take().unwrap_or_default()) } else { None };
        let Some(Run::Importing(batch)) = app.ui.import_window.as_mut().map(|state| &mut state.run) else { return };
        match (started, cause) {
            (Some(started), _) => {
                batch.current = Some((name, started.get("job").and_then(Value::as_u64).unwrap_or(0)));
                return;
            }
            (None, cause) => {
                batch.done += 1;
                batch.failures.push(trf("Couldn't import {name}: {cause}", &[("name", &name), ("cause", &cause.unwrap_or_default())]));
            }
        }
    }
}

fn conclude(app: &mut FigApp) {
    if let Some(state) = app.ui.import_window.as_mut()
        && let Run::Importing(batch) = std::mem::take(&mut state.run)
    {
        let done = batch.total - batch.failures.len();
        state.run = Run::Summary { doc: batch.doc, done, total: batch.total, failures: batch.failures };
    }
    close_if_nothing_can_show_it(app);
}

fn close_if_nothing_can_show_it(app: &mut FigApp) {
    let Some(ImportWindow { run: Run::Summary { doc, failures, .. }, .. }) = app.ui.import_window.as_ref() else { return };
    if app.session.docs.iter().any(|ds| ds.uid == *doc) || app.session.project().is_some() {
        return;
    }
    let told = failures.join("
");
    app.ui.import_window = None;
    if !told.is_empty() {
        app.show_error(told);
    }
}

fn active_sources(app: &FigApp) -> Option<(String, Vec<Source>)> {
    let binding = app.session.project()?;
    let sources = binding
        .config
        .sources
        .iter()
        .filter(|source| !source.removed)
        .map(|source| Source {
            package: source.package.clone(),
            version: source.version.clone(),
            framework: source.framework.clone().unwrap_or_default(),
        })
        .collect();
    Some((binding.root.clone(), sources))
}

fn fetch(app: &mut FigApp, state: &mut ImportWindow, package: &str, refresh: bool) {
    let params = if refresh { json!({"package": package, "refresh": true}) } else { json!({"package": package}) };
    let (job, failure) = match app.run(LIST, params) {
        Some(started) => (started.get("job").and_then(Value::as_u64), None),
        None => (None, Some(app.ui.last_error.clone().unwrap_or_default())),
    };
    if let Some(read) = state.reads.iter_mut().find(|read| read.package == package) {
        read.job = job;
        read.failure = failure;
    }
}

/// Keep the surface on the sources the project has, and read the components of the versions it has.
fn settle(app: &mut FigApp, state: &mut ImportWindow) -> Option<(String, Vec<Source>)> {
    let (root, sources) = active_sources(app)?;
    state.reads.retain(|read| read.root == root && sources.iter().any(|s| s.package == read.package && s.version == read.version));
    for source in &sources {
        if state.reads.iter().any(|read| read.package == source.package) {
            continue;
        }
        state.reads.push(Read { package: source.package.clone(), version: source.version.clone(), root: root.clone(), job: None, failure: None });
        fetch(app, state, &source.package, false);
    }
    Some((root, sources))
}

fn is_visual(name: &str) -> bool {
    let hook = name.strip_prefix("use").is_some_and(|rest| rest.chars().next().is_some_and(char::is_uppercase));
    let first_upper = name.chars().next().is_some_and(char::is_uppercase);
    first_upper && !hook && !name.ends_with("Context") && !name.ends_with("Provider")
}

fn offered(found: &Inspected) -> impl Iterator<Item = &InspectedComponent> {
    found.components.iter().filter(|component| is_visual(&component.name))
}

fn families(app: &FigApp, root: &str, sources: &[Source]) -> Vec<Family> {
    let counts: Vec<usize> = sources
        .iter()
        .map(|s| match app.session.inspected(&s.package) {
            Some(found) => offered(&found).count(),
            None => usize::from(!s.framework.is_empty()),
        })
        .collect();
    let dependencies: Vec<Vec<String>> = sources.iter().map(|s| dependencies_of(root, &s.package, &s.version)).collect();
    let borrowed: Vec<Vec<&str>> = dependencies.iter().map(|list| list.iter().map(String::as_str).collect()).collect();
    let facts: Vec<SourceFacts> = sources
        .iter()
        .zip(&counts)
        .zip(&borrowed)
        .map(|((s, &components), depends_on)| SourceFacts {
            package: &s.package,
            version: &s.version,
            framework: &s.framework,
            components,
            depends_on,
        })
        .collect();
    group_sources(&facts)
}

fn cards_of(app: &FigApp, state: &ImportWindow, families: &[Family]) -> Vec<CardView> {
    let mut ordered: Vec<&Family> = families.iter().collect();
    if let Some(first) = &state.first
        && let Some(at) = ordered.iter().position(|family| family.packages.contains(first))
    {
        let lead = ordered.remove(at);
        ordered.insert(0, lead);
    }
    let all: Vec<(String, Arc<Inspected>)> = families
        .iter()
        .flat_map(|family| family.packages.iter())
        .filter_map(|package| app.session.inspected(package).map(|found| (package.clone(), found)))
        .collect();
    let is_ticked = |package: &str, name: &str| state.ticked.iter().any(|(p, n)| p == package && n == name);
    ordered
        .into_iter()
        .filter_map(|family| {
            let rows: Vec<RowView> = family
                .packages
                .iter()
                .filter_map(|package| all.iter().find(|(p, _)| p == package))
                .flat_map(|(package, found)| {
                    offered(found).map(move |component| RowView {
                        package: package.clone(),
                        name: component.name.clone(),
                        ticked: is_ticked(package, &component.name),
                        taken_in: state
                            .ticked
                            .iter()
                            .find(|(p, n)| p != package && n.eq_ignore_ascii_case(&component.name))
                            .map(|(p, _)| p.clone())
                            .filter(|_| !is_ticked(package, &component.name)),
                    })
                })
                .collect();
            let ticked = rows.iter().filter(|row| row.ticked).count();
            (!rows.is_empty()).then(|| CardView { label: family.label.clone(), ticked, total: rows.len(), rows })
        })
        .collect()
}

fn statuses_of(app: &FigApp, state: &ImportWindow, sources: &[Source]) -> Vec<Status> {
    sources
        .iter()
        .filter(|s| app.session.inspected(&s.package).is_none())
        .map(|s| match state.reads.iter().find(|read| read.package == s.package).and_then(|read| read.failure.clone()) {
            Some(cause) => Status::Failed { package: s.package.clone(), cause },
            None => Status::Loading(s.package.clone()),
        })
        .collect()
}

fn notes_of(app: &FigApp, sources: &[Source]) -> Vec<String> {
    sources.iter().filter_map(|s| app.session.inspected(&s.package)).flat_map(|found| found.notes.clone()).collect()
}

fn progress_of(run: &Run) -> Progress {
    match run {
        Run::Idle => Progress::Idle,
        Run::Importing(batch) => Progress::Importing { done: batch.done, total: batch.total },
        Run::Summary { done, total, failures, .. } => Progress::Summary { done: *done, total: *total, failures: failures.clone() },
    }
}

impl Run {
    fn doc(&self) -> Option<u64> {
        match self {
            Self::Idle => None,
            Self::Importing(batch) => Some(batch.doc),
            Self::Summary { doc, .. } => Some(*doc),
        }
    }
}

/// While a run lasts the surface stays on the file it started from, whatever tab is in front.
pub fn show(app: &mut FigApp, ui: &mut Ui) {
    let doc = app.ui.import_window.as_ref().and_then(|state| state.run.doc());
    let open = doc.and_then(|doc| app.session.docs.iter().position(|ds| ds.uid == doc));
    let show_components = match (doc, open) {
        (Some(doc), Some(_)) => on_doc(app, doc, |app| draw_and_act(app, ui)).unwrap_or(false),
        _ => draw_and_act(app, ui),
    };
    if show_components {
        if let Some(at) = open {
            app.session.active = at;
        }
        app.ui.import_window = None;
        app.ui.project_settings = None;
        app.run(SHOW, json!({}));
    }
}

fn draw_and_act(app: &mut FigApp, ui: &mut Ui) -> bool {
    let acts = draw(app, ui);
    let show_components = acts.iter().any(|act| matches!(act, Act::Show));
    for act in acts {
        perform(app, act);
    }
    show_components
}

fn draw(app: &mut FigApp, ui: &mut Ui) -> Vec<Act> {
    let Some(mut state) = app.ui.import_window.take() else { return Vec::new() };
    let Some((root, sources)) = settle(app, &mut state) else {
        if matches!(state.run, Run::Importing(_)) {
            app.ui.import_window = Some(state);
        }
        return Vec::new();
    };
    let project = app.session.project().map(|binding| binding.config.name.clone()).unwrap_or_default();
    let families = families(app, &root, &sources);
    let cards = cards_of(app, &state, &families);
    let statuses = statuses_of(app, &state, &sources);
    let nothing_to_offer = (cards.is_empty() && statuses.is_empty() && !sources.is_empty()).then(|| notes_of(app, &sources));
    let valid: Vec<(String, String)> =
        cards.iter().flat_map(|card| card.rows.iter().filter(|row| row.ticked).map(|row| (row.package.clone(), row.name.clone()))).collect();
    let t = Tokens::get(ui.ctx());
    let mut acts = Vec::new();
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.bg)).show(ui, |ui| {
        let mut model = Model {
            project: &project,
            cards,
            statuses,
            no_source: sources.is_empty(),
            nothing_to_offer,
            count: valid.len(),
            states: state.states,
            light: state.light,
            dark: state.dark,
            animations: state.animations,
            search: &mut state.search,
            progress: progress_of(&state.run),
            message: state.message.clone(),
            at_top: std::mem::take(&mut state.just_opened),
        };
        view::show(ui, &t, &mut model, &mut acts);
    });
    state.ticked = valid;
    app.ui.import_window = Some(state);
    acts
}

fn edit(app: &mut FigApp, change: impl FnOnce(&mut ImportWindow)) {
    if let Some(state) = app.ui.import_window.as_mut() {
        change(state);
    }
}

fn perform(app: &mut FigApp, act: Act) {
    match act {
        Act::Toggle(package, name) => edit(app, |state| {
            match state.ticked.iter().position(|(p, n)| *p == package && *n == name) {
                Some(at) => {
                    state.ticked.remove(at);
                }
                None => state.ticked.push((package, name)),
            }
            state.message = None;
        }),
        Act::State(index) => edit(app, |state| {
            state.states[index] = !state.states[index];
            state.message = None;
        }),
        Act::Theme(dark) => edit(app, |state| {
            if dark {
                state.dark = !state.dark;
            } else {
                state.light = !state.light;
            }
            state.message = None;
        }),
        Act::Animations => edit(app, |state| state.animations = !state.animations),
        Act::Import => import(app),
        Act::Cancel => app.ui.import_window = None,
        Act::Retry(package) => {
            if let Some(mut state) = app.ui.import_window.take() {
                fetch(app, &mut state, &package, true);
                app.ui.import_window = Some(state);
            }
        }
        Act::OpenSettings => {
            app.ui.import_window = None;
            app.run(SETTINGS, json!({}));
        }
        Act::Show => {}
    }
}

fn enum_values<'a>(component: &'a InspectedComponent, name: &str) -> Option<(&'a str, &'a [String])> {
    component
        .props
        .iter()
        .find(|prop| prop.name == name && prop.kind == PropKind::Enum && !prop.values.is_empty())
        .map(|prop| (prop.name.as_str(), prop.values.as_slice()))
}

fn params_of(state: &ImportWindow, package: &str, component: &InspectedComponent) -> Value {
    let themes: Vec<&str> = [(state.light, "light"), (state.dark, "dark")].into_iter().filter(|(on, _)| *on).map(|(_, theme)| theme).collect();
    let states: Vec<&str> = STATES.iter().zip(state.states).filter(|(_, on)| *on).map(|(name, _)| *name).collect();
    let mut params = json!({
        "package": package,
        "component": component.name,
        "states": states,
        "themes": themes,
        "animations": state.animations,
        "stage": true,
    });
    for (prop_key, values_key, name) in [("styleProp", "styles", "variant"), ("sizeProp", "sizes", "size")] {
        if let (Some((prop, values)), Some(map)) = (enum_values(component, name), params.as_object_mut()) {
            map.insert(prop_key.to_string(), json!(prop));
            map.insert(values_key.to_string(), json!(values));
        }
    }
    params
}

fn renders_of(state: &ImportWindow, component: &InspectedComponent) -> usize {
    let count = |name: &str| enum_values(component, name).map_or(0, |(_, values)| values.len());
    let themes = usize::from(state.light) + usize::from(state.dark);
    render_count(count("variant"), count("size"), state.states.iter().filter(|on| **on).count(), themes)
}

fn import(app: &mut FigApp) {
    let Some(mut state) = app.ui.import_window.take() else { return };
    let batch = plan(app, &mut state);
    let started = batch.is_some();
    if let Some(batch) = batch {
        state.run = Run::Importing(batch);
    }
    app.ui.import_window = Some(state);
    if started && crate::menus::dev_mode_refuses(app, IMPORT) {
        edit(app, |state| state.run = Run::Idle);
        return;
    }
    if started {
        start_next(app);
    }
}

fn plan(app: &FigApp, state: &mut ImportWindow) -> Option<Batch> {
    if matches!(state.run, Run::Importing(_)) {
        return None;
    }
    let (root, sources) = active_sources(app)?;
    let doc = app.session.ds().ok()?.uid;
    let all = families_in_order(app, state, &sources);
    if all.is_empty() {
        state.message = Some(tr("Choose at least one component.").to_string());
        return None;
    }
    if state.states.iter().all(|on| !on) || !(state.light || state.dark) {
        state.message = Some(tr("Choose at least one value in each group.").to_string());
        return None;
    }
    if all.iter().any(|(_, component)| renders_of(state, component) > MAX_RENDERS) {
        state.message = Some(trf("More than {max} renders: choose fewer styles, sizes, states or themes", &[("max", &MAX_RENDERS)]));
        return None;
    }
    state.message = None;
    let queue: VecDeque<(String, Value)> =
        all.iter().map(|(package, component)| (component.name.clone(), params_of(state, package, component))).collect();
    let total = queue.len();
    Some(Batch { doc, root, queue, current: None, total, done: 0, failures: Vec::new() })
}

fn families_in_order(app: &FigApp, state: &ImportWindow, sources: &[Source]) -> Vec<(String, InspectedComponent)> {
    let root = app.session.project().map(|binding| binding.root.clone()).unwrap_or_default();
    let ordered = families(app, &root, sources);
    let cards = cards_of(app, state, &ordered);
    let mut chosen = Vec::new();
    for card in &cards {
        for row in card.rows.iter().filter(|row| row.ticked) {
            if let Some(component) =
                app.session.inspected(&row.package).and_then(|found| found.components.iter().find(|c| c.name == row.name).cloned())
            {
                chosen.push((row.package.clone(), component));
            }
        }
    }
    chosen
}
