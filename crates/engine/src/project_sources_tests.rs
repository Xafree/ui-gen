//! Source libraries of a project: `project.addSource`, `project.setSourceVersion` and
//! `project.removeSource` hand an npm job to the capture service, and `uigen.json` changes only
//! when `pump_capture` brings the success back. `capture.status` lists the jobs.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::capture::{CaptureBackend, CaptureError, CaptureErrorKind, CaptureEvent, CaptureJob, JobId};
use crate::file_access::{Roots, confined};
use crate::{EngineError, Result, Session};

static NEXT: AtomicUsize = AtomicUsize::new(0);

type Outcome = std::result::Result<Value, CaptureError>;

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-sources-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
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

#[derive(Default)]
struct Calls {
    submitted: Vec<CaptureJob>,
    ready: Vec<(JobId, Outcome)>,
}

struct FakeBackend {
    calls: Arc<Mutex<Calls>>,
}

impl CaptureBackend for FakeBackend {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let mut calls = self.calls.lock().unwrap();
        calls.submitted.push(job);
        u64::try_from(calls.submitted.len()).unwrap()
    }

    fn poll(&mut self) -> Vec<(JobId, Outcome)> {
        std::mem::take(&mut self.calls.lock().unwrap().ready)
    }

    fn shutdown(&mut self) {}
}

fn write_project(dir: &Dir, sources: &[Value]) {
    let manifest = json!({"version": 1, "name": "mylib", "sources": sources});
    std::fs::write(dir.path("uigen.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    let mut doc = figcraft_doc::Document::new("lib");
    let page = doc.pages[0];
    doc.node_mut(page).unwrap().name = "Button".to_string();
    std::fs::write(dir.path("lib.figcraft"), figcraft_format::save_bytes(&doc, None).unwrap()).unwrap();
}

struct Bench {
    session: Session,
    dir: Dir,
    calls: Arc<Mutex<Calls>>,
}

impl Bench {
    fn new(tag: &str, sources: &[Value]) -> Self {
        let dir = Dir::new(tag);
        write_project(&dir, sources);
        let calls = Arc::new(Mutex::new(Calls::default()));
        let mut session = Session::empty();
        session.set_capture_backend(Box::new(FakeBackend { calls: Arc::clone(&calls) }));
        let opened = session.execute("project.open", &json!({"path": dir.root()})).unwrap();
        assert_eq!(opened["opened"], json!(true), "{opened}");
        Self { session, dir, calls }
    }

    fn run(&mut self, command: &str, params: &Value) -> Result<Value> {
        self.session.execute(command, params)
    }

    fn manifest_bytes(&self) -> Vec<u8> {
        std::fs::read(self.dir.path("uigen.json")).unwrap()
    }

    fn manifest_sources(&self) -> Value {
        let manifest: Value = serde_json::from_slice(&self.manifest_bytes()).unwrap();
        manifest["sources"].clone()
    }

    fn info_sources(&mut self) -> Value {
        let info = self.run("project.info", &json!({})).unwrap();
        info["project"]["sources"].clone()
    }

    fn submitted(&self) -> Vec<CaptureJob> {
        self.calls.lock().unwrap().submitted.clone()
    }

    fn finish(&mut self, job: &Value, outcome: Outcome) -> Vec<CaptureEvent> {
        self.calls.lock().unwrap().ready.push((job_id(job), outcome));
        self.session.pump_capture()
    }
}

fn job_id(result: &Value) -> JobId {
    result["job"].as_u64().unwrap_or_else(|| panic!("no job id in {result}"))
}

fn lit_source(version: &str, removed: bool) -> Value {
    json!({"package": "lit", "version": version, "framework": "web-components", "license": "BSD-3-Clause", "removed": removed})
}

fn installed(package: &str, version: &str, framework: &str, license: &str) -> Outcome {
    Ok(json!({"package": package, "version": version, "framework": framework, "license": license}))
}

fn failure(kind: CaptureErrorKind, message: &str) -> Outcome {
    Err(CaptureError { kind, message: message.to_string() })
}

fn same_folder(shown: &str, expected: &Path) -> bool {
    matches!((std::fs::canonicalize(shown), std::fs::canonicalize(expected)), (Ok(a), Ok(b)) if a == b)
}

fn bad_message(result: Result<Value>) -> String {
    match result {
        Err(EngineError::BadParams { msg, .. }) => msg,
        other => panic!("expected BadParams, got {other:?}"),
    }
}

fn error_text(result: Result<Value>) -> String {
    match result {
        Err(EngineError::Internal { cmd, msg }) => panic!("{cmd} panicked: {msg}"),
        Err(EngineError::UnknownCommand(cmd)) => panic!("{cmd} is not registered"),
        Err(e) => e.to_string(),
        Ok(v) => panic!("expected an error, got {v}"),
    }
}

fn assert_single_install(bench: &Bench, package: &str, version: &str) {
    let jobs = bench.submitted();
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    match &jobs[0] {
        CaptureJob::Install { root, package: asked, version: wanted } => {
            assert!(same_folder(root, &bench.dir.0), "{root}");
            assert_eq!(asked, package);
            assert_eq!(wanted, version);
        }
        other => panic!("expected an Install job, got {other:?}"),
    }
}

fn assert_single_uninstall(bench: &Bench, package: &str) {
    let jobs = bench.submitted();
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    match &jobs[0] {
        CaptureJob::Uninstall { root, package: asked } => {
            assert!(same_folder(root, &bench.dir.0), "{root}");
            assert_eq!(asked, package);
        }
        other => panic!("expected an Uninstall job, got {other:?}"),
    }
}

// covers: E2-6.1
#[test]
fn add_source_submits_an_install_and_uigen_json_gets_the_source_only_when_it_succeeds() {
    let mut bench = Bench::new("add", &[]);
    let before = bench.manifest_bytes();

    let result = bench.run("project.addSource", &json!({"package": "@mui/material"})).unwrap();
    let job = job_id(&result);

    assert_single_install(&bench, "@mui/material", "latest");
    assert_eq!(bench.manifest_bytes(), before, "nothing is written before the install succeeds");

    let events = bench.finish(&result, installed("@mui/material", "7.3.11", "react", "MIT"));
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0].job, job);
    assert_eq!(events[0].command, "project.addSource");
    assert_eq!(events[0].package, "@mui/material");
    assert!(events[0].outcome.is_ok(), "{:?}", events[0].outcome);

    let expected = json!([{"package": "@mui/material", "version": "7.3.11", "framework": "react", "license": "MIT", "removed": false}]);
    assert_eq!(bench.manifest_sources(), expected);
    assert_eq!(bench.info_sources(), expected);
}

// covers: E2-6.1
#[test]
fn add_source_with_an_exact_version_asks_for_that_version() {
    let mut bench = Bench::new("add-version", &[]);
    let result = bench.run("project.addSource", &json!({"package": "lit", "version": "3.3.2"})).unwrap();
    assert_single_install(&bench, "lit", "3.3.2");
    bench.finish(&result, installed("lit", "3.3.2", "web-components", "BSD-3-Clause"));
    assert_eq!(bench.manifest_sources(), json!([lit_source("3.3.2", false)]));
}

// covers: E2-6.2
#[test]
fn a_failed_install_leaves_uigen_json_untouched_and_the_event_carries_the_service_message() {
    let mut bench = Bench::new("add-fails", &[lit_source("3.3.2", false)]);
    let before = bench.manifest_bytes();
    let result = bench.run("project.addSource", &json!({"package": "left-pad"})).unwrap();

    let events = bench.finish(&result, failure(CaptureErrorKind::Install, "npm exited with code 1: 404 left-pad"));

    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0].package, "left-pad");
    match &events[0].outcome {
        Err(error) => assert_eq!(error.message, "npm exited with code 1: 404 left-pad"),
        Ok(value) => panic!("expected the install failure, got {value}"),
    }
    assert_eq!(bench.manifest_bytes(), before);
    assert_eq!(bench.info_sources(), json!([lit_source("3.3.2", false)]));
}

// covers: E2-6.3
#[test]
fn invalid_requests_are_refused_with_their_cause_and_submit_nothing() {
    let mut bench = Bench::new("refused", &[lit_source("3.3.2", false)]);
    let before = bench.manifest_bytes();
    let cases = [
        ("project.addSource", json!({}), "package"),
        ("project.addSource", json!({"package": 5}), "package"),
        ("project.addSource", json!({"package": ""}), "package"),
        ("project.addSource", json!({"package": "../evil"}), "package"),
        ("project.addSource", json!({"package": "file:../evil"}), "package"),
        ("project.addSource", json!({"package": "Upper"}), "package"),
        ("project.addSource", json!({"package": "-rf"}), "package"),
        ("project.addSource", json!({"package": "left-pad", "version": "^1.0.0"}), "version"),
        ("project.addSource", json!({"package": "left-pad", "version": "next"}), "version"),
        ("project.addSource", json!({"package": "left-pad", "version": 5}), "version"),
        ("project.addSource", json!({"package": "lit"}), "lit"),
        ("project.setSourceVersion", json!({"package": "left-pad", "version": "1.0.0"}), "left-pad"),
        ("project.setSourceVersion", json!({"package": "lit", "version": "~3.4.0"}), "version"),
        ("project.setSourceVersion", json!({"package": "lit"}), "version"),
        ("project.setSourceVersion", json!({"version": "3.4.0"}), "package"),
        ("project.removeSource", json!({"package": "left-pad"}), "left-pad"),
        ("project.removeSource", json!({}), "package"),
    ];
    for (command, params, cause) in cases {
        let message = bad_message(bench.run(command, &params));
        assert!(message.to_lowercase().contains(cause), "{command} {params}: {message}");
    }
    assert!(bench.submitted().is_empty(), "{:?}", bench.submitted());
    assert_eq!(bench.manifest_bytes(), before);
}

// covers: E2-6.3
#[test]
fn source_commands_outside_a_project_are_refused_and_submit_nothing() {
    let calls = Arc::new(Mutex::new(Calls::default()));
    let mut session = Session::new();
    session.set_capture_backend(Box::new(FakeBackend { calls: Arc::clone(&calls) }));
    assert!(!session.in_lib_project());
    for (command, params) in [
        ("project.addSource", json!({"package": "lit"})),
        ("project.setSourceVersion", json!({"package": "lit", "version": "3.4.0"})),
        ("project.removeSource", json!({"package": "lit"})),
    ] {
        let message = bad_message(session.execute(command, &params));
        assert!(message.to_lowercase().contains("project"), "{command}: {message}");
    }
    assert!(calls.lock().unwrap().submitted.is_empty());
}

// covers: E2-6.4
#[test]
fn set_source_version_changes_the_version_only_after_the_install_succeeds() {
    let mut bench = Bench::new("set-version", &[lit_source("3.3.2", false)]);
    let before = bench.manifest_bytes();

    let result = bench.run("project.setSourceVersion", &json!({"package": "lit", "version": "3.4.0"})).unwrap();

    assert_single_install(&bench, "lit", "3.4.0");
    assert_eq!(bench.manifest_bytes(), before, "the old version stays while the install runs");

    let events = bench.finish(&result, installed("lit", "3.4.0", "web-components", "BSD-3-Clause"));
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0].command, "project.setSourceVersion");
    assert_eq!(bench.manifest_sources(), json!([lit_source("3.4.0", false)]));
    assert_eq!(bench.info_sources(), json!([lit_source("3.4.0", false)]));
}

// covers: E2-6.4
#[test]
fn a_failed_version_change_keeps_the_old_version() {
    let mut bench = Bench::new("set-version-fails", &[lit_source("3.3.2", false)]);
    let before = bench.manifest_bytes();
    let result = bench.run("project.setSourceVersion", &json!({"package": "lit", "version": "9.9.9"})).unwrap();

    let events = bench.finish(&result, failure(CaptureErrorKind::Install, "No matching version found for lit@9.9.9"));

    assert!(matches!(&events[0].outcome, Err(error) if error.message == "No matching version found for lit@9.9.9"), "{events:?}");
    assert_eq!(bench.manifest_bytes(), before);
    assert_eq!(bench.info_sources(), json!([lit_source("3.3.2", false)]));
}

// covers: E2-6.5
#[test]
fn remove_source_keeps_the_source_marked_removed_and_leaves_the_captures_alone() {
    let mut bench = Bench::new("remove", &[lit_source("3.3.2", false)]);
    let captures = bench.dir.path(".uigen/captures");
    std::fs::create_dir_all(&captures).unwrap();
    std::fs::write(captures.join("lit-button.png"), b"synthetic png bytes").unwrap();

    let result = bench.run("project.removeSource", &json!({"package": "lit"})).unwrap();
    assert_single_uninstall(&bench, "lit");
    let events = bench.finish(&result, Ok(json!({"package": "lit"})));

    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0].command, "project.removeSource");
    assert_eq!(bench.manifest_sources(), json!([lit_source("3.3.2", true)]));
    assert_eq!(bench.info_sources(), json!([lit_source("3.3.2", true)]));
    assert_eq!(std::fs::read(captures.join("lit-button.png")).unwrap(), b"synthetic png bytes");

    let mut reopened = Session::empty();
    reopened.execute("project.open", &json!({"path": bench.dir.root()})).unwrap();
    let info = reopened.execute("project.info", &json!({})).unwrap();
    assert_eq!(info["project"]["sources"], json!([lit_source("3.3.2", true)]), "{info}");
}

// covers: E2-6.5
#[test]
fn adding_a_removed_source_again_brings_it_back() {
    let mut bench = Bench::new("readd", &[lit_source("3.3.2", true)]);

    let result = bench.run("project.addSource", &json!({"package": "lit"})).unwrap();
    assert_single_install(&bench, "lit", "latest");
    bench.finish(&result, installed("lit", "3.4.0", "web-components", "BSD-3-Clause"));

    assert_eq!(bench.manifest_sources(), json!([lit_source("3.4.0", false)]));
    assert_eq!(bench.info_sources(), json!([lit_source("3.4.0", false)]));
}

// covers: E2-6.6
#[test]
fn without_a_backend_every_source_command_says_the_service_is_not_available_and_writes_nothing() {
    let dir = Dir::new("no-backend");
    write_project(&dir, &[lit_source("3.3.2", false)]);
    let before = std::fs::read(dir.path("uigen.json")).unwrap();
    let mut session = Session::empty();
    session.execute("project.open", &json!({"path": dir.root()})).unwrap();

    for (command, params) in [
        ("project.addSource", json!({"package": "@mui/material"})),
        ("project.setSourceVersion", json!({"package": "lit", "version": "3.4.0"})),
        ("project.removeSource", json!({"package": "lit"})),
    ] {
        let message = bad_message(session.execute(command, &params));
        assert!(message.contains("capture service not available"), "{command}: {message}");
    }
    assert_eq!(std::fs::read(dir.path("uigen.json")).unwrap(), before);
}

// covers: E2-6.7
#[test]
fn add_source_outside_the_write_roots_is_refused_without_submitting() {
    let mut bench = Bench::new("roots", &[]);
    let before = bench.manifest_bytes();
    let elsewhere = Dir::new("roots-elsewhere");
    let read_only = Arc::new(Roots::new(&[bench.dir.root()], &[]).unwrap());
    let other_folder = Arc::new(Roots::new(&[bench.dir.root()], &[elsewhere.root()]).unwrap());

    for roots in [&read_only, &other_folder] {
        let text = error_text(confined(Some(roots), || bench.session.execute("project.addSource", &json!({"package": "lit"}))));
        assert!(text.contains("automation path rejected"), "{text}");
    }
    assert!(bench.submitted().is_empty(), "{:?}", bench.submitted());
    assert_eq!(bench.manifest_bytes(), before);
}

// covers: E2-6.7
#[test]
fn absurd_parameters_on_the_source_commands_never_panic_submit_or_write() {
    let mut bench = Bench::new("absurd", &[lit_source("3.3.2", false)]);
    let before = bench.manifest_bytes();
    let long = "x".repeat(10_000);
    let junk = [
        json!(null),
        json!([1, 2, 3]),
        json!("x"),
        json!(5),
        json!({}),
        json!({"package": null}),
        json!({"package": ["left-pad"]}),
        json!({"package": {"name": "left-pad"}}),
        json!({"package": "\u{0}"}),
        json!({"package": "left-pad\n--registry=http://evil"}),
        json!({"package": long, "version": long}),
        json!({"package": "left-pad", "version": null}),
        json!({"package": "left-pad", "version": {"a": 1}}),
        json!({"package": "left-pad", "version": "1e308"}),
        json!({"package": "left-pad", "version": "9".repeat(100)}),
        json!({"package": "left-pad", "version": "1.0.0 || 2.0.0"}),
    ];
    for command in ["project.addSource", "project.setSourceVersion", "project.removeSource"] {
        for params in &junk {
            match bench.run(command, params) {
                Err(EngineError::Internal { msg, .. }) => panic!("{command} panicked on {params}: {msg}"),
                Err(EngineError::UnknownCommand(cmd)) => panic!("{cmd} is not registered"),
                Err(_) => {}
                Ok(value) => panic!("{command} accepted {params}: {value}"),
            }
        }
    }
    for params in &junk {
        if let Err(EngineError::Internal { msg, .. }) = bench.run("capture.status", params) {
            panic!("capture.status panicked on {params}: {msg}");
        }
    }
    assert!(bench.submitted().is_empty(), "{:?}", bench.submitted());
    assert_eq!(bench.manifest_bytes(), before);
}

fn status_of(bench: &mut Bench, job: JobId) -> Value {
    let status = bench.run("capture.status", &json!({})).unwrap();
    let jobs = status["jobs"].as_array().unwrap_or_else(|| panic!("no jobs list in {status}"));
    jobs.iter().find(|entry| entry["job"].as_u64() == Some(job)).cloned().unwrap_or_else(|| panic!("job {job} is not listed in {status}"))
}

// covers: E2-6.1
#[test]
fn capture_status_lists_a_submitted_job_then_shows_it_done() {
    let mut bench = Bench::new("status-done", &[]);
    let result = bench.run("project.addSource", &json!({"package": "lit"})).unwrap();
    let job = job_id(&result);

    let waiting = status_of(&mut bench, job);
    assert_eq!(waiting["command"], json!("project.addSource"), "{waiting}");
    assert_eq!(waiting["package"], json!("lit"), "{waiting}");
    assert!(matches!(waiting["state"].as_str(), Some("queued" | "running")), "{waiting}");

    bench.finish(&result, installed("lit", "3.3.2", "web-components", "BSD-3-Clause"));
    let done = status_of(&mut bench, job);
    assert_eq!(done["state"], json!("done"), "{done}");
    assert!(matches!(done.get("error"), None | Some(Value::Null)), "{done}");
}

// covers: E2-6.2
#[test]
fn capture_status_shows_a_failed_job_with_the_service_message() {
    let mut bench = Bench::new("status-failed", &[]);
    let result = bench.run("project.addSource", &json!({"package": "left-pad"})).unwrap();
    let job = job_id(&result);

    bench.finish(&result, failure(CaptureErrorKind::Install, "npm exited with code 1"));

    let failed = status_of(&mut bench, job);
    assert_eq!(failed["state"], json!("failed"), "{failed}");
    assert!(failed["error"].as_str().is_some_and(|e| e.contains("npm exited with code 1")), "{failed}");
}
