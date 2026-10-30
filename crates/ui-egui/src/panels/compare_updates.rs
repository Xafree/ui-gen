//! The Updates section of the Compare tab: the version a copy comes from against the one the
//! project asks for, a recapture of the source (`uigen.updateSource`) followed by the differences
//! (`uigen.sourceDiff`), and their application (`uigen.applyDifferences`) on a click, never alone.

use egui::{Color32, Frame, Margin, Response, RichText, Sense, Ui};
use figcraft_doc::NodeId;
use figcraft_engine::capture::{JobId, JobState, JobStatus};
use serde_json::{Value, json};

use crate::FigApp;
use crate::i18n::{tr, trf};
use crate::theme::{Tokens, regular, semibold};
use crate::widgets::{self, PAD};

const UPDATE: &str = "uigen.updateSource";
const DIFF: &str = "uigen.sourceDiff";
const APPLY: &str = "uigen.applyDifferences";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Added,
    Removed,
    Changed,
}

impl Kind {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "added" => Self::Added,
            "removed" => Self::Removed,
            "changed" => Self::Changed,
            _ => return None,
        })
    }

    fn label(self) -> &'static str {
        match self {
            Self::Added => tr("Added"),
            Self::Removed => tr("Removed"),
            Self::Changed => tr("Changed"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Origin {
    Clean,
    Modified,
    Unknown,
}

impl Origin {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "clean" => Self::Clean,
            "modified" => Self::Modified,
            "unknown" => Self::Unknown,
            _ => return None,
        })
    }

    fn label(self) -> Option<&'static str> {
        match self {
            Self::Clean => None,
            Self::Modified => Some(tr("Modified in your copy")),
            Self::Unknown => Some(tr("Unknown: no earlier capture to tell")),
        }
    }
}

struct Entry {
    id: String,
    path: String,
    kind: Kind,
    field: String,
    origin: Origin,
}

impl Entry {
    fn of(value: &Value) -> Option<Self> {
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        Some(Self {
            id: text("id")?,
            path: text("path")?,
            kind: Kind::parse(&text("kind")?)?,
            field: text("field")?,
            origin: Origin::parse(&text("state")?)?,
        })
    }

    fn field_label(&self) -> Option<&str> {
        Some(match self.field.as_str() {
            "layer" => return None,
            "fill" => tr("Fill"),
            "stroke" => tr("Stroke"),
            "effects" => tr("Effects"),
            "corner radius" => tr("Corner radius"),
            "size" => tr("Size"),
            "position" => tr("Position"),
            "opacity" => tr("Opacity"),
            "blend" => tr("Blend"),
            "visibility" => tr("Visibility"),
            "clip content" => tr("Clip content"),
            "text" => tr("Text"),
            "font" => tr("Font"),
            "auto layout" => tr("Auto layout"),
            other => other,
        })
    }
}

struct Diff {
    from: String,
    to: String,
    captured: bool,
    entries: Vec<Entry>,
}

impl Diff {
    fn of(result: &Value) -> Option<Self> {
        let text = |key: &str| result.get(key).and_then(Value::as_str).map(str::to_owned);
        Some(Self {
            from: text("from")?,
            to: text("to")?,
            captured: result.get("latest").is_some_and(Value::is_object),
            entries: result.get("differences")?.as_array()?.iter().filter_map(Entry::of).collect(),
        })
    }

    fn versions(&self) -> String {
        if self.from == self.to { self.to.clone() } else { format!("{} → {}", self.from, self.to) }
    }

    fn clean(&self) -> usize {
        self.entries.iter().filter(|entry| entry.origin == Origin::Clean).count()
    }
}

struct Cached {
    uid: u64,
    revision: u64,
    set: NodeId,
    diff: Option<Diff>,
}

#[derive(Clone, PartialEq, Eq)]
enum Outcome {
    Running,
    Done,
    Failed(String),
}

struct Import {
    head: JobId,
    component: String,
    jobs: Vec<JobId>,
    outcome: Outcome,
}

struct Recapture {
    uid: u64,
    package: String,
    imports: Vec<Import>,
}

impl Recapture {
    fn running(&self) -> bool {
        self.imports.iter().any(|import| import.outcome == Outcome::Running)
    }

    fn failures(&self) -> impl Iterator<Item = (&str, &str)> {
        self.imports.iter().filter_map(|import| match &import.outcome {
            Outcome::Failed(cause) => Some((import.component.as_str(), cause.as_str())),
            _ => None,
        })
    }

    fn follow(&mut self, app: &FigApp) {
        let statuses = app.session.capture_jobs();
        for import in self.imports.iter_mut().filter(|import| import.outcome == Outcome::Running) {
            let jobs = app.session.import_jobs(import.head);
            if jobs.is_empty() {
                import.outcome = ended(&statuses, &import.jobs);
            } else {
                import.jobs = jobs;
            }
        }
    }
}

fn ended(statuses: &[JobStatus], jobs: &[JobId]) -> Outcome {
    let failure = jobs.iter().filter_map(|id| statuses.iter().find(|status| status.job == *id)).find_map(|status| match &status.state {
        JobState::Failed(cause) => Some(cause.clone()),
        _ => None,
    });
    failure.map_or(Outcome::Done, Outcome::Failed)
}

#[derive(Default)]
pub(crate) struct Updates {
    cached: Option<Cached>,
    recapture: Option<Recapture>,
}

enum Act {
    Check,
    ApplyClean,
    Take(String),
}

fn is_active_source(app: &FigApp, package: &str) -> bool {
    app.session.project().is_some_and(|binding| binding.config.sources.iter().any(|source| source.package == package && !source.removed))
}

pub(crate) fn section(app: &mut FigApp, ui: &mut Ui, state: &mut Updates, set: NodeId, package: &str) {
    let Ok(ds) = app.session.ds() else { return };
    let (uid, revision) = (ds.uid, ds.revision);
    if !is_active_source(app, package) {
        return;
    }
    if let Some(recapture) = state.recapture.as_mut() {
        recapture.follow(app);
    }
    if state.cached.as_ref().is_none_or(|cached| (cached.uid, cached.revision, cached.set) != (uid, revision, set)) {
        let diff = app.run(DIFF, json!({"copy": set.0})).as_ref().and_then(Diff::of);
        state.cached = Some(Cached { uid, revision, set, diff });
    }
    let t = Tokens::get(ui.ctx());
    let recapture = state.recapture.as_ref().filter(|recapture| recapture.uid == uid && recapture.package == package);
    let diff = state.cached.as_ref().and_then(|cached| cached.diff.as_ref());
    if recapture.is_some_and(Recapture::running) {
        ui.ctx().request_repaint();
    }
    let acts = paint(ui, &t, diff, recapture, package);
    for act in acts {
        match act {
            Act::Check => recapture_source(app, state, package),
            Act::ApplyClean => apply(app, set, None),
            Act::Take(id) => apply(app, set, Some(&id)),
        }
    }
}

pub(crate) fn recapture_source(app: &mut FigApp, state: &mut Updates, package: &str) {
    let Some(uid) = app.session.ds().ok().map(|ds| ds.uid) else { return };
    let Some(started) = app.run(UPDATE, json!({"package": package})) else { return };
    let ids = started.get("jobs").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_u64);
    let names = started.get("components").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str);
    let imports = ids
        .zip(names)
        .map(|(head, component)| Import { head, component: component.to_owned(), jobs: app.session.import_jobs(head), outcome: Outcome::Running })
        .collect();
    state.recapture = Some(Recapture { uid, package: package.to_owned(), imports });
}

fn apply(app: &mut FigApp, copy: NodeId, id: Option<&str>) {
    let mut params = json!({"copy": copy.0});
    if let Some(id) = id {
        params["ids"] = json!([id]);
    }
    let Some(result) = app.run(APPLY, params) else {
        let cause = app.ui.last_error.clone().unwrap_or_default();
        app.toast(trf("Couldn't apply: {cause}", &[("cause", &cause)]));
        return;
    };
    let count = result.get("applied").and_then(Value::as_array).map_or(0, Vec::len);
    let message = trf("{count} applied", &[("count", &count)]);
    if count > 0 {
        app.toast_with_undo(message);
    } else {
        app.toast(message);
    }
}

fn line(ui: &mut Ui, text: &str, color: Color32, bold: bool) -> Response {
    let font = if bold { semibold(11.0) } else { regular(11.0) };
    Frame::NONE
        .inner_margin(Margin::symmetric(PAD as i8, 2))
        .show(ui, |ui| ui.add(egui::Label::new(RichText::new(text).font(font).color(color)).wrap().sense(Sense::hover())))
        .inner
}

fn paint(ui: &mut Ui, t: &Tokens, diff: Option<&Diff>, recapture: Option<&Recapture>, package: &str) -> Vec<Act> {
    let mut acts = Vec::new();
    let Some(diff) = diff else { return acts };
    widgets::section_header(ui, tr("Updates"), &[], false);
    line(ui, &diff.versions(), t.text_secondary, false);
    let running = recapture.is_some_and(Recapture::running);
    crate::right::row(ui, 32.0, |ui| {
        if widgets::button(ui, tr("Check for changes"), true, !running).on_hover_text(tr("Update source components")).clicked() {
            acts.push(Act::Check);
        }
    });
    if let Some(recapture) = recapture.filter(|recapture| recapture.running()) {
        let done = recapture.imports.iter().filter(|import| import.outcome != Outcome::Running).count();
        let progress =
            trf("Recapturing {package}… {done} of {total}", &[("package", &package), ("done", &done), ("total", &recapture.imports.len())]);
        line(ui, &progress, t.text_secondary, false);
        return acts;
    }
    for (component, cause) in recapture.into_iter().flat_map(Recapture::failures) {
        line(ui, &trf("Couldn't import {name}: {cause}", &[("name", &component), ("cause", &cause)]), t.danger, false);
    }
    if !diff.captured {
        line(ui, &trf("Recapture first: {version} has not been captured yet.", &[("version", &diff.to)]), t.text_secondary, false);
    } else if diff.entries.is_empty() {
        line(ui, &trf("No difference with {version}.", &[("version", &diff.to)]), t.text_secondary, false);
    } else {
        differences(ui, t, diff, &mut acts);
    }
    acts
}

fn differences(ui: &mut Ui, t: &Tokens, diff: &Diff, acts: &mut Vec<Act>) {
    let count = trf("{count} differences with {version}", &[("count", &diff.entries.len()), ("version", &diff.to)]);
    line(ui, &count, t.text, true).on_hover_text(tr("List source differences"));
    crate::right::row(ui, 32.0, |ui| {
        if widgets::button(ui, tr("Apply unmodified changes"), false, diff.clean() > 0).clicked() {
            acts.push(Act::ApplyClean);
        }
    });
    for entry in &diff.entries {
        line(ui, &format!("{} · {}", entry.kind.label(), entry.path), t.text, false);
        if let Some(field) = entry.field_label() {
            line(ui, field, t.text_secondary, false);
        }
        if let Some(origin) = entry.origin.label() {
            line(ui, origin, t.warning, false);
            crate::right::row(ui, 32.0, |ui| {
                if widgets::button(ui, tr("Take the new value"), false, true).clicked() {
                    acts.push(Act::Take(entry.id.clone()));
                }
            });
        }
        ui.add_space(4.0);
    }
}
