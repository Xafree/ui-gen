//! `project.setSourceVersion` with a `packages` list moves several sources to one version in a
//! single job, and `uigen.json` changes only when that job succeeds, all packages in one write.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::capture::{CaptureBackend, CaptureError, CaptureJob, JobId};
use crate::{EngineError, Session};

static NEXT: AtomicUsize = AtomicUsize::new(0);

type Outcome = Result<Value, CaptureError>;

#[derive(Default)]
struct Calls {
    submitted: Vec<CaptureJob>,
    ready: Vec<(JobId, Outcome)>,
}

struct FakeBackend(Arc<Mutex<Calls>>);

impl CaptureBackend for FakeBackend {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let mut calls = self.0.lock().unwrap();
        calls.submitted.push(job);
        u64::try_from(calls.submitted.len()).unwrap()
    }

    fn poll(&mut self) -> Vec<(JobId, Outcome)> {
        std::mem::take(&mut self.0.lock().unwrap().ready)
    }

    fn shutdown(&mut self) {}
}

struct Bench {
    session: Session,
    dir: PathBuf,
    calls: Arc<Mutex<Calls>>,
}

impl Bench {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("figcraft-sources-family-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let sources = json!([
            {"package": "@acme/ui", "version": "1.0.0", "framework": "react", "license": "MIT", "removed": false},
            {"package": "@acme/styles", "version": "1.0.0", "license": "MIT", "removed": false},
            {"package": "lit", "version": "3.3.2", "framework": "web-components", "removed": false},
            {"package": "gone", "version": "1.0.0", "removed": true}
        ]);
        let manifest = json!({"version": 1, "name": "mylib", "sources": sources});
        std::fs::write(dir.join("uigen.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
        let mut doc = figcraft_doc::Document::new("lib");
        let page = doc.pages[0];
        doc.node_mut(page).unwrap().name = "Button".to_string();
        std::fs::write(dir.join("lib.figcraft"), figcraft_format::save_bytes(&doc, None).unwrap()).unwrap();
        let calls = Arc::new(Mutex::new(Calls::default()));
        let mut session = Session::empty();
        session.set_capture_backend(Box::new(FakeBackend(Arc::clone(&calls))));
        let opened = session.execute("project.open", &json!({"path": dir.to_string_lossy()})).unwrap();
        assert_eq!(opened["opened"], json!(true), "{opened}");
        Self { session, dir, calls }
    }

    fn set_version(&mut self, params: &Value) -> Result<Value, EngineError> {
        self.session.execute("project.setSourceVersion", params)
    }

    fn submitted(&self) -> Vec<CaptureJob> {
        self.calls.lock().unwrap().submitted.clone()
    }

    fn versions(&self) -> Vec<(String, String)> {
        let manifest: Value = serde_json::from_slice(&std::fs::read(self.dir.join("uigen.json")).unwrap()).unwrap();
        let sources = manifest["sources"].as_array().unwrap().clone();
        sources.iter().map(|s| (s["package"].as_str().unwrap().to_string(), s["version"].as_str().unwrap().to_string())).collect()
    }
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn message(result: Result<Value, EngineError>) -> String {
    match result {
        Err(EngineError::BadParams { msg, .. }) => msg,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn entry(package: &str, version: &str) -> Value {
    json!({"package": package, "version": version, "framework": "react", "license": "MIT"})
}

#[test]
fn a_package_list_installs_every_package_at_the_version_in_one_job() {
    let mut bench = Bench::new();
    let result = bench.set_version(&json!({"packages": ["@acme/ui", "@acme/styles"], "version": "2.0.0"})).unwrap();
    let jobs = bench.submitted();
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    match &jobs[0] {
        CaptureJob::InstallMany { packages, .. } => {
            let shape: Vec<(&str, &str)> = packages.iter().map(|p| (p.name.as_str(), p.version.as_str())).collect();
            assert_eq!(shape, [("@acme/ui", "2.0.0"), ("@acme/styles", "2.0.0")]);
        }
        other => panic!("expected InstallMany, got {other:?}"),
    }
    let before = bench.versions();

    let job = result["job"].as_u64().unwrap();
    bench.calls.lock().unwrap().ready.push((job, Ok(json!({"packages": [entry("@acme/ui", "2.0.0"), entry("@acme/styles", "2.0.0")]}))));
    let events = bench.session.pump_capture();
    assert_eq!(events.len(), 1, "{events:?}");
    assert!(events[0].outcome.is_ok(), "{events:?}");
    assert_eq!(before[0].1, "1.0.0");
    assert_eq!(
        bench.versions(),
        [("@acme/ui", "2.0.0"), ("@acme/styles", "2.0.0"), ("lit", "3.3.2"), ("gone", "1.0.0")].map(|(p, v)| (p.to_string(), v.to_string()))
    );
}

#[test]
fn a_list_of_one_package_behaves_like_the_single_package_form() {
    let mut bench = Bench::new();
    bench.set_version(&json!({"packages": ["lit"], "version": "3.4.0"})).unwrap();
    assert!(matches!(bench.submitted().as_slice(), [CaptureJob::Install { package, version, .. }] if package == "lit" && version == "3.4.0"));
}

#[test]
fn a_bad_package_list_is_refused_and_submits_nothing() {
    let mut bench = Bench::new();
    for (params, needle) in [
        (json!({"packages": [], "version": "2.0.0"}), "non-empty"),
        (json!({"packages": "@acme/ui", "version": "2.0.0"}), "array"),
        (json!({"packages": ["@acme/ui", 3], "version": "2.0.0"}), "string"),
        (json!({"packages": ["@acme/ui", "@acme/ui"], "version": "2.0.0"}), "twice"),
        (json!({"packages": ["@acme/ui", "gone"], "version": "2.0.0"}), "gone"),
        (json!({"packages": ["@acme/ui", "not-a-source"], "version": "2.0.0"}), "not-a-source"),
        (json!({"packages": ["@acme/ui", "@acme/styles"]}), "version"),
    ] {
        let shown = message(bench.set_version(&params));
        assert!(shown.contains(needle), "{params}: {shown}");
    }
    assert!(bench.submitted().is_empty());
}
