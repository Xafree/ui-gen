//! Test support for the playground session: a scripted capture backend that answers the
//! `playground.*` methods at once and holds every other job until the test releases it, and a
//! temporary library project whose document has a themed variable collection and two components.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use figcraft_doc::{Collection, Document, Mode, VariableValue};
use serde_json::{Value, json};

use crate::capture::{CaptureBackend, CaptureError, CaptureErrorKind, CaptureEvent, CaptureJob, CaptureOutcome, JobId};
use crate::components::tests::{ex, idv};
use crate::lit_export::{ExportOptions, plan};
use crate::playground::{Composed, Template, compose};
use crate::playground_targets_tests::{THEME, color, fixture, themed_color};
use crate::{Result, Session};

pub(crate) const APPLIED_THEME: &str = "Theme";
pub(crate) const OTHER_TOKEN: &str = "zebre";
pub(crate) const APPLIED_TOKEN: &str = "primaire";
pub(crate) const OPEN: &str = "playground.open";
pub(crate) const FRAME: &str = "playground.frame";

const ALT: &str = "c-alt";
const ALT_MODE: &str = "m-alt";
const PUMPS: usize = 3;

static NEXT: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct Script {
    pub open: CaptureOutcome,
    pub set_template: CaptureOutcome,
    pub frames: VecDeque<CaptureOutcome>,
}

impl Default for Script {
    fn default() -> Self {
        Self {
            open: Ok(json!({"defined": ["ui-button", "ui-card"], "missing": [], "errors": [], "shape": true})),
            set_template: Ok(json!({})),
            frames: VecDeque::new(),
        }
    }
}

#[derive(Default)]
struct State {
    submitted: Vec<CaptureJob>,
    ready: Vec<(JobId, CaptureOutcome)>,
    held: Vec<(JobId, CaptureOutcome)>,
    script: Script,
}

#[derive(Clone, Default)]
pub(crate) struct Calls(Arc<Mutex<State>>);

impl Calls {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap()
    }

    pub(crate) fn submitted(&self) -> Vec<CaptureJob> {
        self.lock().submitted.clone()
    }

    pub(crate) fn playground(&self, method: &str) -> Vec<Value> {
        self.submitted()
            .into_iter()
            .filter_map(|job| match job {
                CaptureJob::Playground { method: m, params, .. } if m == method => Some(params),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn playground_methods(&self) -> Vec<String> {
        self.submitted()
            .into_iter()
            .filter_map(|job| match job {
                CaptureJob::Playground { method, .. } => Some(method),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn script(&self, change: impl FnOnce(&mut Script)) {
        change(&mut self.lock().script);
    }

    pub(crate) fn release(&self) {
        let mut state = self.lock();
        let held = std::mem::take(&mut state.held);
        state.ready.extend(held);
    }
}

struct ScriptedBackend {
    calls: Calls,
}

fn answer(script: &mut Script, job: &CaptureJob) -> CaptureOutcome {
    match job {
        CaptureJob::Playground { method, params, .. } => match method.as_str() {
            "playground.open" => script.open.clone(),
            "playground.setTemplate" => script.set_template.clone(),
            "playground.frame" => script.frames.pop_front().unwrap_or_else(|| Ok(json!({"seq": params["after"], "changed": false}))),
            _ => Ok(json!({})),
        },
        CaptureJob::Install { package, version, .. } => Ok(json!({"package": package, "version": version, "framework": "react", "license": "MIT"})),
        _ => Ok(json!({})),
    }
}

impl CaptureBackend for ScriptedBackend {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let mut state = self.calls.lock();
        let id = u64::try_from(state.submitted.len() + 1).unwrap();
        let outcome = answer(&mut state.script, &job);
        match job {
            CaptureJob::Playground { .. } => state.ready.push((id, outcome)),
            _ => state.held.push((id, outcome)),
        }
        state.submitted.push(job);
        id
    }

    fn poll(&mut self) -> Vec<(JobId, CaptureOutcome)> {
        std::mem::take(&mut self.calls.lock().ready)
    }

    fn shutdown(&mut self) {}
}

pub(crate) fn failure(kind: CaptureErrorKind, message: &str) -> CaptureError {
    CaptureError { kind, message: message.to_string() }
}

pub(crate) fn image(seq: u64, format: &str, width: u64, height: u64, data: &str) -> CaptureOutcome {
    Ok(json!({"seq": seq, "changed": true, "format": format, "width": width, "height": height, "data": data}))
}

pub(crate) struct Dir(PathBuf);

impl Dir {
    pub(crate) fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-playground-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub(crate) fn root(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }

    pub(crate) fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Library {
    Full,
    NoVariables,
    NoComponents,
    DuplicateNames,
}

fn themed_document(library: Library) -> Document {
    if library == Library::NoVariables {
        return Document::new("lib");
    }
    let mut doc = fixture().doc;
    doc.variables.collections.push(Collection {
        id: ALT.to_string(),
        name: "Alternative".to_string(),
        modes: vec![Mode { id: ALT_MODE.to_string(), name: "Value".to_string() }],
        extends: None,
    });
    let mut other = themed_color(&format!("{OTHER_TOKEN}/unique"), color("#123456"), color("#654321"));
    other.collection = ALT.to_string();
    other.values = std::iter::once((ALT_MODE.to_string(), VariableValue::Color(color("#123456")))).collect();
    doc.variables.variables.push(other);
    doc
}

fn component(s: &mut Session, name: &str, x: f64) {
    let frame = idv(&ex(s, "frame.create", json!({"x": x, "y": 0, "width": 100, "height": 40, "name": name})));
    ex(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 40, "parent": frame, "props": {"name": "Bg", "fill": "#ff0000"}}));
    ex(s, "text.create", json!({"text": "Click", "x": 10, "y": 10, "parent": frame, "props": {"name": "Label"}}));
    ex(s, "component.create", json!({"ids": [frame]}));
}

fn library_document(library: Library) -> Document {
    let mut doc = themed_document(library);
    let page = doc.pages[0];
    doc.node_mut(page).unwrap().name = "Library".to_string();
    let mut s = Session::with_document(doc);
    match library {
        Library::NoComponents => {}
        Library::DuplicateNames => {
            component(&mut s, "Button", 0.0);
            component(&mut s, "Button", 200.0);
        }
        Library::Full | Library::NoVariables => {
            component(&mut s, "Button", 0.0);
            component(&mut s, "Card", 200.0);
        }
    }
    s.doc().unwrap().clone()
}

fn write_project(dir: &Dir, library: Library) {
    let mut manifest = json!({"version": 1, "name": "mylib", "sources": []});
    if library != Library::NoVariables {
        manifest["appliedTheme"] = json!(APPLIED_THEME);
    }
    std::fs::write(dir.path("uigen.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    std::fs::write(dir.path("lib.figcraft"), figcraft_format::save_bytes(&library_document(library), None).unwrap()).unwrap();
}

pub(crate) struct Bench {
    pub session: Session,
    pub calls: Calls,
    pub dir: Dir,
}

impl Bench {
    pub(crate) fn new(tag: &str, library: Library) -> Self {
        let mut bench = Self::without_backend(tag, library);
        bench.attach_backend();
        bench
    }

    pub(crate) fn without_backend(tag: &str, library: Library) -> Self {
        let dir = Dir::new(tag);
        write_project(&dir, library);
        let mut session = Session::empty();
        let opened = session.execute("project.open", &json!({"path": dir.root()})).unwrap();
        assert_eq!(opened["opened"], json!(true), "{opened}");
        Self { session, calls: Calls::default(), dir }
    }

    pub(crate) fn standalone(tag: &str) -> Self {
        let mut bench = Self { session: Session::new(), calls: Calls::default(), dir: Dir::new(tag) };
        bench.attach_backend();
        bench
    }

    fn attach_backend(&mut self) {
        self.session.set_capture_backend(Box::new(ScriptedBackend { calls: self.calls.clone() }));
    }

    pub(crate) fn ready(tag: &str) -> Self {
        let mut bench = Self::new(tag, Library::Full);
        bench.run("playground.open", &json!({"template": "dashboard"})).unwrap();
        bench.pump();
        assert!(matches!(bench.session.playground_status(), crate::playground::Status::Ready), "the scripted open must leave the playground ready");
        bench
    }

    pub(crate) fn run(&mut self, command: &str, params: &Value) -> Result<Value> {
        self.session.execute(command, params)
    }

    pub(crate) fn pump(&mut self) -> Vec<CaptureEvent> {
        (0..PUMPS).flat_map(|_| self.session.pump_capture()).collect()
    }

    pub(crate) fn cycle(&mut self) -> Vec<CaptureEvent> {
        self.session.playground_poll();
        self.session.pump_capture()
    }

    pub(crate) fn state(&mut self) -> Value {
        self.run("playground.state", &json!({})).unwrap()
    }

    pub(crate) fn root(&self) -> String {
        self.dir.root()
    }

    pub(crate) fn staged(&self, rel: &str) -> PathBuf {
        self.dir.path(".uigen/playground").join(rel)
    }

    pub(crate) fn nothing_staged(&self) -> bool {
        !self.dir.path(".uigen/playground").exists()
    }

    pub(crate) fn document_text(&self) -> String {
        serde_json::to_string(self.session.doc().unwrap()).unwrap()
    }

    pub(crate) fn composed(&self, template: Template) -> Composed {
        let options = ExportOptions { collections: Some(vec![THEME.to_string()]), ..ExportOptions::default() };
        compose(template, &plan(self.session.doc().unwrap(), &options))
    }
}

pub(crate) fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}
