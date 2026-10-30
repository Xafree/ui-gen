//! Test support for the component import command: a scripted capture backend that answers `inspect`
//! and `render` jobs, a synthetic button snapshot and a temporary library project.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::capture::{CaptureBackend, CaptureError, CaptureErrorKind, CaptureEvent, CaptureJob, CaptureOutcome, JobId};
use crate::{Result, Session};

pub(crate) const IMPORT: &str = "uigen.importComponent";
pub(crate) const PACKAGE: &str = "synthetic-ui";
pub(crate) const VERSION: &str = "1.0.0";

const MAX_PUMPS: usize = 64;
const QUIET_PUMPS: usize = 3;

static NEXT: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn synthetic_button(style: &str, size: &str, state: &str, theme: &str) -> Value {
    let (width, height) = if size == "sm" { (80.0, 28.0) } else { (100.0, 36.0) };
    let (fill, ink) = match (style, state, theme) {
        ("solid", "hover", _) => ("rgb(21, 101, 192)", "rgb(255, 255, 255)"),
        ("solid", _, "dark") => ("rgb(144, 202, 249)", "rgb(0, 0, 0)"),
        ("solid", _, _) => ("rgb(25, 118, 210)", "rgb(255, 255, 255)"),
        (_, "hover", _) => ("rgba(25, 118, 210, 0.04)", "rgb(25, 118, 210)"),
        _ => ("rgba(0, 0, 0, 0)", "rgb(25, 118, 210)"),
    };
    json!({
        "version": 1,
        "viewport": { "width": 400, "height": 200, "deviceScaleFactor": 1 },
        "root": {
            "type": "element",
            "tag": "button",
            "label": "Button",
            "className": "syn-button",
            "box": { "x": 20, "y": 20, "width": width, "height": height },
            "style": { "backgroundColor": fill },
            "children": [{
                "type": "text",
                "text": "Button",
                "box": { "x": 30, "y": 26, "width": 40, "height": 16 },
                "lineBox": { "x": 30, "y": 26, "width": 40, "height": 16 },
                "style": { "color": ink, "fontSize": "14px" }
            }]
        }
    })
}

pub(crate) fn button_inspect(with_size: bool, with_loading: bool) -> CaptureOutcome {
    let mut props = vec![json!({"name": "variant", "type": "enum", "values": ["solid", "outline"]})];
    if with_size {
        props.push(json!({"name": "size", "type": "enum", "values": ["sm", "md"]}));
    }
    props.push(json!({"name": "disabled", "type": "boolean", "values": null}));
    if with_loading {
        props.push(json!({"name": "loading", "type": "boolean", "values": null}));
    }
    Ok(json!({
        "package": PACKAGE,
        "version": VERSION,
        "framework": "react",
        "components": [
            {"name": "Button", "props": props},
            {"name": "x-chip", "props": [{"name": "label", "type": "string", "values": null}]}
        ],
        "notes": []
    }))
}

pub(crate) fn service_failure(kind: CaptureErrorKind, message: &str) -> CaptureError {
    CaptureError { kind, message: message.to_string() }
}

pub(crate) struct FailingRender {
    pub variant: &'static str,
    pub size: &'static str,
    pub state: &'static str,
    pub theme: &'static str,
    pub error: CaptureError,
}

pub(crate) struct Scenario {
    inspect: CaptureOutcome,
    failing_render: Option<FailingRender>,
}

impl Scenario {
    pub(crate) fn new(inspect: CaptureOutcome) -> Self {
        Self { inspect, failing_render: None }
    }

    pub(crate) fn failing(mut self, render: FailingRender) -> Self {
        self.failing_render = Some(render);
        self
    }

    fn answer(&self, job: &CaptureJob) -> CaptureOutcome {
        match job {
            CaptureJob::Inspect { .. } => self.inspect.clone(),
            CaptureJob::Render { props, state, theme, .. } => {
                let variant = props.get("variant").and_then(Value::as_str);
                let size = props.get("size").and_then(Value::as_str);
                if let Some(failing) = &self.failing_render
                    && variant == Some(failing.variant)
                    && size == Some(failing.size)
                    && state == failing.state
                    && theme == failing.theme
                {
                    return Err(failing.error.clone());
                }
                let snapshot = synthetic_button(variant.unwrap_or("solid"), size.unwrap_or("md"), state, theme);
                Ok(json!({"snapshot": snapshot, "png": ".uigen/captures/synthetic.png"}))
            }
            CaptureJob::Install { .. } | CaptureJob::Uninstall { .. } | CaptureJob::Playground { .. } => {
                Err(service_failure(CaptureErrorKind::Internal, "unexpected job"))
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct Calls {
    pub submitted: Vec<CaptureJob>,
    ready: Vec<(JobId, CaptureOutcome)>,
}

struct ScriptedBackend {
    calls: Arc<Mutex<Calls>>,
    scenario: Scenario,
}

impl CaptureBackend for ScriptedBackend {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let outcome = self.scenario.answer(&job);
        let mut calls = self.calls.lock().unwrap();
        calls.submitted.push(job);
        let id = u64::try_from(calls.submitted.len()).unwrap();
        calls.ready.push((id, outcome));
        id
    }

    fn poll(&mut self) -> Vec<(JobId, CaptureOutcome)> {
        std::mem::take(&mut self.calls.lock().unwrap().ready)
    }

    fn shutdown(&mut self) {}
}

pub(crate) struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-import-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn root(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn synthetic_source(removed: bool) -> Value {
    json!({"package": PACKAGE, "version": VERSION, "framework": "react", "license": "MIT", "removed": removed})
}

fn write_project(dir: &Dir, sources: &[Value]) {
    let manifest = json!({"version": 1, "name": "mylib", "sources": sources});
    std::fs::write(dir.0.join("uigen.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    let mut doc = figcraft_doc::Document::new("lib");
    let page = doc.pages[0];
    doc.node_mut(page).unwrap().name = "Button".to_string();
    std::fs::write(dir.0.join("lib.figcraft"), figcraft_format::save_bytes(&doc, None).unwrap()).unwrap();
}

pub(crate) struct Bench {
    pub session: Session,
    pub calls: Arc<Mutex<Calls>>,
    _dir: Dir,
}

impl Bench {
    pub(crate) fn new(tag: &str, sources: &[Value], scenario: Scenario) -> Self {
        let mut bench = Self::without_backend(tag, sources);
        bench.session.set_capture_backend(Box::new(ScriptedBackend { calls: Arc::clone(&bench.calls), scenario }));
        bench
    }

    pub(crate) fn without_backend(tag: &str, sources: &[Value]) -> Self {
        let dir = Dir::new(tag);
        write_project(&dir, sources);
        let mut session = Session::empty();
        let opened = session.execute("project.open", &json!({"path": dir.root()})).unwrap();
        assert_eq!(opened["opened"], json!(true), "{opened}");
        Self { session, calls: Arc::new(Mutex::new(Calls::default())), _dir: dir }
    }

    pub(crate) fn import(&mut self, params: &Value) -> Result<Value> {
        self.session.execute(IMPORT, params)
    }

    pub(crate) fn submitted(&self) -> Vec<CaptureJob> {
        self.calls.lock().unwrap().submitted.clone()
    }

    pub(crate) fn pump_to_end(&mut self) -> Vec<CaptureEvent> {
        let mut events = Vec::new();
        for _ in 0..MAX_PUMPS {
            events.extend(self.session.pump_capture());
            if events.iter().any(|event| event.command == IMPORT) {
                break;
            }
        }
        for _ in 0..QUIET_PUMPS {
            events.extend(self.session.pump_capture());
        }
        events
    }

    pub(crate) fn document_text(&self, tab: usize) -> String {
        serde_json::to_string(self.session.docs[tab].doc.as_ref()).unwrap()
    }
}
