//! The imports in progress. An import owns its capture jobs: the `inspect` job (whose id the command
//! returned and the final event carries) and the renders it submits when `inspect` is back. One failed
//! job ends it; the jobs still running are skipped when they come back.

use std::collections::BTreeMap;

use figcraft_doc::NodeId;
use figcraft_dom::Snapshot;
use serde_json::Value;

use super::assemble::{self, Finished, Variant};
use super::plan::{Cell, Plan, State, Theme, plan};
use crate::Session;
use crate::capture::{CaptureError, CaptureErrorKind, CaptureEvent, CaptureJob, CaptureOutcome, JobId};

pub(crate) const COMMAND: &str = "uigen.importComponent";

pub(crate) struct Request {
    pub root: String,
    pub package: String,
    pub version: String,
    pub component: String,
    pub style_prop: String,
    pub size_prop: String,
    pub styles: Option<Vec<String>>,
    pub sizes: Option<Vec<String>>,
    pub states: Option<Vec<State>>,
    pub themes: Vec<Theme>,
    pub animations: bool,
}

pub(crate) struct Destination {
    pub stage: bool,
    pub uid: u64,
    pub page: NodeId,
    pub parent: NodeId,
    pub at: Option<(f64, f64)>,
}

struct Render {
    snapshot: Snapshot,
    png: Option<String>,
}

struct Rendering {
    cells: Vec<Cell>,
    warnings: Vec<String>,
    rows: usize,
    columns: usize,
    waiting: BTreeMap<JobId, (usize, usize)>,
    renders: Vec<Vec<Option<Render>>>,
}

enum Stage {
    Inspecting,
    Rendering(Rendering),
}

pub(crate) struct PendingImport {
    head: JobId,
    jobs: Vec<JobId>,
    request: Request,
    destination: Destination,
    stage: Stage,
    done: Option<Finished>,
}

/// Submit the `inspect` job of `request`; its id is the import's.
pub(crate) fn start(s: &mut Session, request: Request, destination: Destination) -> Result<JobId, CaptureError> {
    let job = CaptureJob::Inspect { root: request.root.clone(), package: request.package.clone() };
    let head = s.submit_capture(COMMAND, &request.package, job)?;
    s.capture.imports.push(PendingImport { head, jobs: vec![head], request, destination, stage: Stage::Inspecting, done: None });
    Ok(head)
}

/// Take in the result of `job`. An import that has all its renders writes the set, or waits for the
/// gesture, version or text preview its tab is in; the event comes out when the import ends, in
/// success or failure.
pub(crate) fn advance(s: &mut Session, job: JobId, outcome: CaptureOutcome) -> Option<CaptureEvent> {
    let position = s.capture.imports.iter().position(|pending| pending.jobs.contains(&job))?;
    let mut pending = s.capture.imports.remove(position);
    let result = match pending.receive(s, job, outcome) {
        Ok(None) => None,
        Ok(Some(finished)) if busy(s, &pending.destination) => {
            pending.done = Some(finished);
            None
        }
        Ok(Some(finished)) => Some(assemble::write(s, &pending.request, &pending.destination, finished)),
        Err(error) => Some(Err(error)),
    };
    let Some(result) = result else {
        s.capture.imports.insert(position, pending);
        return None;
    };
    Some(conclude(s, &pending, result))
}

/// The jobs the import started by `head` has submitted so far (its inspect, then its renders).
pub(crate) fn jobs_of(s: &Session, head: JobId) -> Vec<JobId> {
    s.capture.imports.iter().find(|pending| pending.head == head).map_or_else(Vec::new, |pending| pending.jobs.clone())
}

/// Write the imports that were waiting for their tab to be free again.
pub(crate) fn resume(s: &mut Session) -> Vec<CaptureEvent> {
    let mut events = Vec::new();
    while let Some(position) = s.capture.imports.iter().position(|pending| pending.done.is_some() && !busy(s, &pending.destination)) {
        let mut pending = s.capture.imports.remove(position);
        if let Some(finished) = pending.done.take() {
            let result = assemble::write(s, &pending.request, &pending.destination, finished);
            events.push(conclude(s, &pending, result));
        }
    }
    events
}

fn busy(s: &Session, destination: &Destination) -> bool {
    s.docs.iter().find(|ds| ds.uid == destination.uid).is_some_and(|ds| ds.interaction.is_some() || ds.preview.is_some() || ds.text_preview.is_some())
}

fn conclude(s: &mut Session, pending: &PendingImport, result: Result<Value, CaptureError>) -> CaptureEvent {
    let ended = result.as_ref().map(|_| ()).map_err(|error| error.message.clone());
    for id in &pending.jobs {
        s.capture.finish(*id, if *id == pending.head { ended.clone() } else { Err("skipped: the import ended".to_string()) });
    }
    CaptureEvent { job: pending.head, command: COMMAND.to_string(), package: pending.request.package.clone(), outcome: result }
}

fn failure(step: &str, cause: CaptureError) -> CaptureError {
    CaptureError { kind: cause.kind, message: format!("{step}: {}", cause.message) }
}

fn snapshot_of(result: &Value) -> Result<Snapshot, String> {
    let snapshot = result.get("snapshot").filter(|snapshot| snapshot.is_object()).ok_or("the result has no snapshot")?;
    let text = serde_json::to_string(snapshot).map_err(|e| e.to_string())?;
    figcraft_dom::parse_snapshot(&text).map_err(|e| e.to_string())
}

impl PendingImport {
    fn receive(&mut self, s: &mut Session, job: JobId, outcome: CaptureOutcome) -> Result<Option<Finished>, CaptureError> {
        if job != self.head {
            s.capture.finish(job, outcome.as_ref().map(|_| ()).map_err(|error| error.message.clone()));
        }
        match &mut self.stage {
            Stage::Inspecting => {
                let inspected = outcome.map_err(|cause| failure("inspect", cause))?;
                let rendering = self.submit_renders(s, plan(&self.request, &inspected)?)?;
                self.stage = Stage::Rendering(rendering);
                Ok(None)
            }
            Stage::Rendering(rendering) => {
                let Some((index, theme_index)) = rendering.waiting.remove(&job) else { return Ok(None) };
                let Some(theme) = self.request.themes.get(theme_index) else { return Ok(None) };
                let Some(cell) = rendering.cells.get(index) else { return Ok(None) };
                let step = format!("render {} {} {}", self.request.component, cell.label, theme.render());
                let rendered = outcome.map_err(|cause| failure(&step, cause))?;
                let snapshot =
                    snapshot_of(&rendered).map_err(|cause| CaptureError { kind: CaptureErrorKind::Protocol, message: format!("{step}: {cause}") })?;
                if let Some(note) = rendered.get("note").and_then(Value::as_str)
                    && !rendering.warnings.iter().any(|warning| warning == note)
                {
                    rendering.warnings.push(note.to_string());
                }
                let png = rendered.get("png").and_then(Value::as_str).map(str::to_string);
                if let Some(slot) = rendering.renders.get_mut(index).and_then(|themed| themed.get_mut(theme_index)) {
                    *slot = Some(Render { snapshot, png });
                }
                if rendering.waiting.is_empty() { rendering.finish(&self.request.themes).map(Some) } else { Ok(None) }
            }
        }
    }

    fn submit_renders(&mut self, s: &mut Session, plan: Plan) -> Result<Rendering, CaptureError> {
        let mut waiting = BTreeMap::new();
        for (index, cell) in plan.cells.iter().enumerate() {
            for (theme_index, theme) in self.request.themes.iter().enumerate() {
                let job = CaptureJob::Render {
                    root: self.request.root.clone(),
                    package: self.request.package.clone(),
                    component: self.request.component.clone(),
                    props: cell.props.clone(),
                    sample_text: Some(self.request.component.clone()),
                    state: cell.state.render().to_string(),
                    theme: theme.render().to_string(),
                };
                let id = s.submit_capture(COMMAND, &self.request.package, job)?;
                self.jobs.push(id);
                waiting.insert(id, (index, theme_index));
            }
        }
        let renders = plan.cells.iter().map(|_| self.request.themes.iter().map(|_| None).collect()).collect();
        Ok(Rendering { cells: plan.cells, warnings: plan.warnings, rows: plan.rows, columns: plan.columns, waiting, renders })
    }
}

impl Rendering {
    fn finish(&mut self, themes: &[Theme]) -> Result<Finished, CaptureError> {
        let cells = std::mem::take(&mut self.cells);
        let renders = std::mem::take(&mut self.renders);
        let mut variants = Vec::with_capacity(cells.len());
        for (cell, themed) in cells.into_iter().zip(renders) {
            let missing = |theme: Option<&Theme>| CaptureError {
                kind: CaptureErrorKind::Internal,
                message: format!("{}: no {} render came back", cell.label, theme.map_or("", |theme| theme.render())),
            };
            let mut themed = themed.into_iter();
            let first = themed.next().flatten().ok_or_else(|| missing(themes.first()))?;
            let rest: Vec<Render> =
                themed.zip(themes.iter().skip(1)).map(|(render, theme)| render.ok_or_else(|| missing(Some(theme)))).collect::<Result<_, _>>()?;
            let pngs = std::iter::once(&first).chain(&rest).map(|render| render.png.clone()).collect();
            variants.push(Variant {
                label: cell.label,
                style: cell.style,
                size: cell.size,
                state: cell.state,
                row: cell.row,
                column: cell.column,
                snapshot: first.snapshot,
                others: rest.into_iter().map(|render| render.snapshot).collect(),
                pngs,
            });
        }
        Ok(Finished { variants, rows: self.rows, columns: self.columns, warnings: std::mem::take(&mut self.warnings) })
    }
}
