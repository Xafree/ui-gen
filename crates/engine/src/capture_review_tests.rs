//! Review fixes of the capture commands: jobs seen with their folder, a restarted backend, results
//! the service must not be believed on, one job at a time per package, a manifest that can't be written.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::capture::{CaptureBackend, CaptureError, CaptureErrorKind, CaptureEvent, CaptureJob, JobId, JobState};
use crate::file_access::{Roots, confined};
use crate::{EngineError, Result, Session};

static NEXT: AtomicUsize = AtomicUsize::new(0);

type Outcome = std::result::Result<Value, CaptureError>;

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        Self::inside(&std::env::temp_dir(), tag)
    }

    fn inside(parent: &Path, tag: &str) -> Self {
        let dir = parent.join(format!("figcraft-review-{tag}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
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

fn backend(session: &mut Session) -> Arc<Mutex<Calls>> {
    let calls = Arc::new(Mutex::new(Calls::default()));
    session.set_capture_backend(Box::new(FakeBackend { calls: Arc::clone(&calls) }));
    calls
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
        let opened_as = dir.root();
        Self::open(dir, &opened_as)
    }

    fn open(dir: Dir, opened_as: &str) -> Self {
        let mut session = Session::empty();
        let calls = backend(&mut session);
        let opened = session.execute("project.open", &json!({"path": opened_as})).unwrap();
        assert_eq!(opened["opened"], json!(true), "{opened}");
        Self { session, dir, calls }
    }

    fn run(&mut self, command: &str, params: &Value) -> Result<Value> {
        self.session.execute(command, params)
    }

    fn manifest_bytes(&self) -> Vec<u8> {
        std::fs::read(self.dir.path("uigen.json")).unwrap()
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

fn lit_source(version: &str) -> Value {
    json!({"package": "lit", "version": version, "framework": "web-components", "license": "BSD-3-Clause", "removed": false})
}

fn bad_message(result: Result<Value>) -> String {
    match result {
        Err(EngineError::BadParams { msg, .. }) => msg,
        other => panic!("expected BadParams, got {other:?}"),
    }
}

fn error_text(result: Result<Value>) -> String {
    match result {
        Err(e) => e.to_string(),
        Ok(v) => panic!("expected an error, got {v}"),
    }
}

fn same_folder(shown: &str, expected: &Path) -> bool {
    matches!((std::fs::canonicalize(shown), std::fs::canonicalize(expected)), (Ok(a), Ok(b)) if a == b)
}

fn installed(package: &str, version: &str, framework: Option<&str>, license: Option<&str>) -> Outcome {
    Ok(json!({"package": package, "version": version, "framework": framework, "license": license}))
}

fn refused_as_protocol(events: &[CaptureEvent]) -> String {
    assert_eq!(events.len(), 1, "{events:?}");
    match &events[0].outcome {
        Err(e) if e.kind == CaptureErrorKind::Protocol => e.message.clone(),
        other => panic!("expected a Protocol error, got {other:?}"),
    }
}

// covers: E2-4.3
#[test]
fn capture_jobs_and_capture_status_carry_the_folder_of_each_job() {
    let mut session = Session::empty();
    backend(&mut session);
    let job = CaptureJob::Uninstall { root: "/p".to_string(), package: "lit".to_string() };
    session.submit_capture("project.removeSource", "lit", job).unwrap();

    let jobs = session.capture_jobs();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].root, "/p");
    assert_eq!(jobs[0].state, JobState::Running);
    let status = session.execute("capture.status", &json!({})).unwrap();
    assert_eq!(status["jobs"][0]["root"], json!("/p"));
}

// covers: E2-4.3
#[test]
fn reading_the_jobs_leaves_the_journal_alone() {
    let mut session = Session::empty();
    backend(&mut session);
    let job = CaptureJob::Uninstall { root: "/p".to_string(), package: "lit".to_string() };
    session.submit_capture("project.removeSource", "lit", job).unwrap();
    let journal = session.journal.len();

    for _ in 0..5 {
        assert_eq!(session.capture_jobs().len(), 1);
    }
    assert_eq!(session.journal.len(), journal);
}

// covers: E2-6.1
#[test]
fn a_project_opened_by_a_relative_path_sends_the_service_an_absolute_folder() {
    let here = std::env::current_dir().unwrap();
    let dir = Dir::inside(&here, "relative");
    write_project(&dir, &[]);
    let relative = dir.0.file_name().unwrap().to_string_lossy().into_owned();
    let mut bench = Bench::open(dir, &relative);

    bench.run("project.addSource", &json!({"package": "lit"})).unwrap();

    match &bench.submitted()[..] {
        [CaptureJob::Install { root, .. }] => {
            assert!(Path::new(root).is_absolute(), "{root}");
            assert!(same_folder(root, &bench.dir.0), "{root}");
        }
        other => panic!("expected one Install job, got {other:?}"),
    }
}

// covers: E2-4.3
#[test]
fn a_second_backend_whose_ids_start_again_still_finishes_its_jobs() {
    let mut session = Session::empty();
    let first = backend(&mut session);
    let job = || CaptureJob::Inspect { root: "/p".to_string(), package: "lit".to_string() };
    let id = session.submit_capture("capture.inspect", "lit", job()).unwrap();
    first.lock().unwrap().ready.push((id, Ok(json!({}))));
    assert_eq!(session.pump_capture().len(), 1);

    let second = backend(&mut session);
    let again = session.submit_capture("capture.inspect", "lit", job()).unwrap();
    assert_eq!(again, id, "a new backend counts from 1 again");
    assert_eq!(session.capture_jobs().iter().filter(|j| j.state == JobState::Running).count(), 1);
    second.lock().unwrap().ready.push((again, Ok(json!({}))));
    assert_eq!(session.pump_capture().len(), 1);

    assert!(session.capture_jobs().iter().all(|j| j.state == JobState::Done), "{:?}", session.capture_jobs());
}

// covers: E2-6.1
#[test]
fn a_result_for_another_package_is_refused_as_a_protocol_error_and_nothing_is_written() {
    let mut bench = Bench::new("other-package", &[]);
    let before = bench.manifest_bytes();
    let result = bench.run("project.addSource", &json!({"package": "lit"})).unwrap();

    let events = bench.finish(&result, installed("left-pad", "1.3.0", None, Some("MIT")));

    assert!(refused_as_protocol(&events).contains("lit"));
    assert_eq!(bench.manifest_bytes(), before);
}

// covers: E2-6.1
#[test]
fn a_result_with_an_unknown_framework_is_refused_and_nothing_is_written() {
    let mut bench = Bench::new("framework", &[]);
    let before = bench.manifest_bytes();
    let result = bench.run("project.addSource", &json!({"package": "lit"})).unwrap();

    let events = bench.finish(&result, installed("lit", "3.3.2", Some("angular"), Some("MIT")));

    assert!(refused_as_protocol(&events).contains("framework"));
    assert_eq!(bench.manifest_bytes(), before);
}

// covers: E2-6.1
#[test]
fn a_result_with_a_license_that_is_too_long_or_has_control_characters_is_refused() {
    for (tag, license) in [("long", "x".repeat(129)), ("control", "MIT\nOR\u{7}Apache".to_string())] {
        let mut bench = Bench::new(&format!("license-{tag}"), &[]);
        let before = bench.manifest_bytes();
        let result = bench.run("project.addSource", &json!({"package": "lit"})).unwrap();

        let events = bench.finish(&result, installed("lit", "3.3.2", Some("web-components"), Some(&license)));

        assert!(refused_as_protocol(&events).contains("license"), "{tag}");
        assert_eq!(bench.manifest_bytes(), before, "{tag}");
    }
}

// covers: E2-6.1
#[test]
fn every_known_framework_and_a_license_of_128_bytes_are_kept() {
    for framework in ["react", "vue", "svelte", "web-components", "unknown"] {
        let mut bench = Bench::new(&format!("known-{framework}"), &[]);
        let result = bench.run("project.addSource", &json!({"package": "lit"})).unwrap();
        let events = bench.finish(&result, installed("lit", "3.3.2", Some(framework), Some(&"x".repeat(128))));
        assert!(events[0].outcome.is_ok(), "{framework}: {:?}", events[0].outcome);
    }
}

// covers: E2-6.7
#[test]
fn a_second_job_on_a_package_already_being_changed_is_refused_until_the_first_ends() {
    let mut bench = Bench::new("in-flight", &[]);
    let first = bench.run("project.addSource", &json!({"package": "lit"})).unwrap();

    let text = bad_message(bench.run("project.addSource", &json!({"package": "lit", "version": "3.3.2"})));
    assert!(text.contains("already in progress"), "{text}");
    assert_eq!(bench.submitted().len(), 1);

    let failure = CaptureError { kind: CaptureErrorKind::Install, message: "npm exited with code 1".to_string() };
    bench.finish(&first, Err(failure));
    bench.run("project.addSource", &json!({"package": "lit"})).unwrap();
    assert_eq!(bench.submitted().len(), 2);
}

// covers: E2-6.7
#[test]
fn changing_the_version_or_removing_a_source_is_refused_while_a_job_runs_on_it() {
    let mut bench = Bench::new("in-flight-active", &[lit_source("3.3.2")]);
    bench.run("project.setSourceVersion", &json!({"package": "lit", "version": "3.4.0"})).unwrap();

    for (command, params) in
        [("project.setSourceVersion", json!({"package": "lit", "version": "3.5.0"})), ("project.removeSource", json!({"package": "lit"}))]
    {
        let text = bad_message(bench.run(command, &params));
        assert!(text.contains("already in progress"), "{command}: {text}");
    }
    assert_eq!(bench.submitted().len(), 1);
}

// covers: E2-6.7
#[test]
fn capture_status_refuses_a_job_that_is_not_a_whole_number() {
    let mut session = Session::empty();
    backend(&mut session);
    for junk in [json!("1"), json!(1.5), json!(-1), json!(true), json!([1]), json!({"job": 1})] {
        let text = bad_message(session.execute("capture.status", &json!({"job": junk})));
        assert!(text.contains("job"), "{junk}: {text}");
    }
}

// covers: E2-6.1
#[test]
fn an_install_that_cannot_be_written_to_uigen_json_says_so_in_plain_words() {
    let mut bench = Bench::new("unwritable", &[]);
    let result = bench.run("project.addSource", &json!({"package": "lit"})).unwrap();
    let mut permissions = std::fs::metadata(bench.dir.path("uigen.json")).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(bench.dir.path("uigen.json"), permissions).unwrap();

    let events = bench.finish(&result, installed("lit", "3.3.2", Some("web-components"), Some("BSD-3-Clause")));

    let mut permissions = std::fs::metadata(bench.dir.path("uigen.json")).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(bench.dir.path("uigen.json"), permissions).unwrap();
    assert_eq!(events.len(), 1);
    match &events[0].outcome {
        Err(e) => {
            assert!(e.message.contains("lit was installed"), "{}", e.message);
            assert!(e.message.contains("uigen.json could not be updated"), "{}", e.message);
        }
        Ok(value) => panic!("expected a failure, got {value}"),
    }
}

// covers: E2-6.7
#[test]
fn a_source_command_is_refused_when_only_the_sources_folder_may_be_written() {
    let mut bench = Bench::new("manifest-roots", &[]);
    let before = bench.manifest_bytes();
    let sources = bench.dir.path(".uigen/sources");
    std::fs::create_dir_all(&sources).unwrap();
    let roots = Arc::new(Roots::new(&[bench.dir.root()], &[sources.to_string_lossy().into_owned()]).unwrap());

    let text = error_text(confined(Some(&roots), || bench.session.execute("project.addSource", &json!({"package": "lit"}))));

    assert!(text.contains("automation path rejected"), "{text}");
    assert!(text.contains("uigen.json"), "{text}");
    assert!(bench.submitted().is_empty(), "{:?}", bench.submitted());
    assert_eq!(bench.manifest_bytes(), before);
}

fn finished_job(session: &mut Session, chained: bool) -> JobId {
    let job = CaptureJob::Uninstall { root: "x".to_string(), package: "lit".to_string() };
    let id = session.submit_capture("capture.test", "lit", job).unwrap();
    if chained {
        session.capture.mark_chained(id);
    }
    session.capture.finish(id, Ok(()));
    id
}

#[test]
fn the_job_a_caller_holds_outlives_the_renders_of_a_long_import() {
    let mut session = Session::empty();
    backend(&mut session);
    let head = finished_job(&mut session, false);
    for _ in 0..300 {
        finished_job(&mut session, true);
    }
    session.pump_capture();
    let kept = session.execute("capture.status", &json!({"job": head})).unwrap();
    assert_eq!(kept["jobs"][0]["job"], json!(head), "{kept}");
}

#[test]
fn a_job_that_is_forgotten_or_never_ran_is_answered_by_name_not_by_an_empty_list() {
    let mut session = Session::empty();
    backend(&mut session);
    let oldest = finished_job(&mut session, false);
    for _ in 0..600 {
        finished_job(&mut session, false);
    }
    session.pump_capture();
    for job in [oldest, 987_654] {
        let text = bad_message(session.execute("capture.status", &json!({"job": job})));
        assert!(text.contains(&job.to_string()) && text.contains("unknown"), "{text}");
    }
}
