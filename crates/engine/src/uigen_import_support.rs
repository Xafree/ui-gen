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

#[derive(Clone)]
pub(crate) enum Png {
    Shared,
    PerVariant,
    Fixed(String),
    Missing,
}

pub(crate) type SnapshotOf = fn(&str, &str, &str, &str) -> Value;

pub(crate) fn variant_png(style: &str, size: &str, state: &str, theme: &str) -> String {
    format!(".uigen/captures/{PACKAGE}/{VERSION}/button-{style}-{size}-{state}-{theme}.png")
}

pub(crate) struct Scenario {
    inspect: CaptureOutcome,
    failing_render: Option<FailingRender>,
    png: Png,
    snapshot: SnapshotOf,
}

impl Scenario {
    pub(crate) fn new(inspect: CaptureOutcome) -> Self {
        Self { inspect, failing_render: None, png: Png::Shared, snapshot: synthetic_button }
    }

    pub(crate) fn failing(mut self, render: FailingRender) -> Self {
        self.failing_render = Some(render);
        self
    }

    pub(crate) fn png(mut self, png: Png) -> Self {
        self.png = png;
        self
    }

    pub(crate) fn snapshots(mut self, snapshot: SnapshotOf) -> Self {
        self.snapshot = snapshot;
        self
    }

    fn png_of(&self, style: &str, size: &str, state: &str, theme: &str) -> Option<String> {
        match &self.png {
            Png::Shared => Some(".uigen/captures/synthetic.png".to_string()),
            Png::PerVariant => Some(variant_png(style, size, state, theme)),
            Png::Fixed(path) => Some(path.clone()),
            Png::Missing => None,
        }
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
                let (style, size) = (variant.unwrap_or("solid"), size.unwrap_or("md"));
                let mut answer = json!({"snapshot": (self.snapshot)(style, size, state, theme)});
                if let (Some(png), Some(fields)) = (self.png_of(style, size, state, theme), answer.as_object_mut()) {
                    fields.insert("png".to_string(), json!(png));
                }
                Ok(answer)
            }
            CaptureJob::Install { .. } | CaptureJob::InstallMany { .. } | CaptureJob::Uninstall { .. } | CaptureJob::Playground { .. } => {
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

pub(crate) const NEXT_VERSION: &str = "1.1.0";

const SYNTHETIC_PNG: &str = ".uigen/captures/synthetic.png";
const CROWD_LAYERS: usize = 2100;

pub(crate) type EvolvingSnapshot = fn(&str, &str, &str, &str, &str) -> Value;

fn painted_layer(label: &str, fill: &str, box_: [f64; 4]) -> Value {
    let [x, y, width, height] = box_;
    json!({
        "type": "element", "tag": "span", "label": label,
        "box": {"x": x, "y": y, "width": width, "height": height},
        "style": {"backgroundColor": fill}
    })
}

fn with_layers(mut snapshot: Value, layers: Vec<Value>) -> Value {
    if let Some(children) = snapshot["root"]["children"].as_array_mut() {
        children.extend(layers);
    }
    snapshot
}

/// The button of the version 1.0.0, or of the version 1.1.0 where the default state paints its icon
/// another colour and loses its ripple, and the hover state gains a badge.
pub(crate) fn evolving_button(version: &str, style: &str, size: &str, state: &str, theme: &str) -> Value {
    let (next, hover) = (version == NEXT_VERSION, state == "hover");
    let icon = match (hover, next) {
        (true, _) => "rgb(40, 50, 60)",
        (false, true) => "rgb(200, 30, 30)",
        (false, false) => "rgb(10, 20, 30)",
    };
    let mut layers = vec![painted_layer("Icon", icon, [74.0, 24.0, 12.0, 12.0])];
    if hover || !next {
        layers.push(painted_layer("Ripple", "rgb(70, 80, 90)", [88.0, 24.0, 10.0, 10.0]));
    }
    if hover && next {
        layers.push(painted_layer("Badge", "rgb(0, 150, 0)", [100.0, 24.0, 10.0, 10.0]));
    }
    with_layers(synthetic_button(style, size, state, theme), layers)
}

/// Like [`evolving_button`], and the default state of the version 1.1.0 gains more layers than the
/// update lists.
pub(crate) fn crowded_button(version: &str, style: &str, size: &str, state: &str, theme: &str) -> Value {
    let snapshot = evolving_button(version, style, size, state, theme);
    if version != NEXT_VERSION || state == "hover" {
        return snapshot;
    }
    let crowd = (0..CROWD_LAYERS)
        .map(|index| {
            painted_layer(&format!("Extra {index}"), "rgb(10, 20, 30)", [20.0 + f64::from(u8::try_from(index % 50).unwrap_or(0)), 20.0, 4.0, 4.0])
        })
        .collect();
    with_layers(snapshot, crowd)
}

pub(crate) struct Evolving {
    pub snapshot: EvolvingSnapshot,
    pub failing_at: Option<&'static str>,
}

struct EvolvingBackend {
    calls: Arc<Mutex<Calls>>,
    version: String,
    service: Evolving,
}

impl EvolvingBackend {
    fn listing(&self) -> Value {
        let props = json!([
            {"name": "variant", "type": "enum", "values": ["solid", "outline"]},
            {"name": "size", "type": "enum", "values": ["sm", "md"]}
        ]);
        json!({
            "package": PACKAGE,
            "version": self.version,
            "framework": "react",
            "components": [{"name": "Button", "props": props}, {"name": "Chip", "props": props}],
            "notes": []
        })
    }

    fn answer(&mut self, job: &CaptureJob) -> CaptureOutcome {
        match job {
            CaptureJob::Inspect { .. } => Ok(self.listing()),
            CaptureJob::Render { props, state, theme, .. } => {
                if self.service.failing_at == Some(self.version.as_str()) {
                    return Err(service_failure(CaptureErrorKind::Render, "page crashed: boom"));
                }
                let style = props.get("variant").and_then(Value::as_str).unwrap_or("solid");
                let size = props.get("size").and_then(Value::as_str).unwrap_or("md");
                Ok(json!({"snapshot": (self.service.snapshot)(&self.version, style, size, state, theme), "png": SYNTHETIC_PNG}))
            }
            CaptureJob::Install { package, version, .. } => {
                self.version.clone_from(version);
                Ok(json!({"package": package, "version": version, "framework": "react", "license": "MIT"}))
            }
            CaptureJob::InstallMany { .. } | CaptureJob::Uninstall { .. } | CaptureJob::Playground { .. } => Ok(json!({})),
        }
    }
}

impl CaptureBackend for EvolvingBackend {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let outcome = self.answer(&job);
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

impl Bench {
    /// A project with the source at [`VERSION`] and a service that renders another snapshot per
    /// installed version: `project.setSourceVersion` moves the service to the new version.
    pub(crate) fn evolving(tag: &str, service: Evolving) -> Self {
        let mut bench = Self::without_backend(tag, &[synthetic_source(false)]);
        let backend = EvolvingBackend { calls: Arc::clone(&bench.calls), version: VERSION.to_string(), service };
        bench.session.set_capture_backend(Box::new(backend));
        bench
    }

    /// The same project saved and opened again in a session that has no capture service.
    pub(crate) fn reopened_without_backend(self) -> Self {
        let Self { mut session, calls: _, _dir: dir } = self;
        session.execute("file.save", &json!({})).unwrap();
        drop(session);
        let mut fresh = Session::empty();
        let opened = fresh.execute("project.open", &json!({"path": dir.root()})).unwrap();
        assert_eq!(opened["opened"], json!(true), "{opened}");
        Self { session: fresh, calls: Arc::new(Mutex::new(Calls::default())), _dir: dir }
    }
}
