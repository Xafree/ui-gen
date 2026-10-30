//! The capture service driver against a fake sidecar.
//!
//! The fake is `tests/fixtures/fake_sidecar.rs`, compiled once with `rustc` (beside the test
//! executable, reused across runs) and used as the config's `node`; the config's `script` is its
//! scenario file. A relaunched test executable cannot play the sidecar: libtest prints its own
//! banner on stdout before any test runs, which would corrupt the protocol stream. The driver must
//! therefore spawn `config.node` with exactly one argument, `config.script`, and talk over the
//! child's stdin and stdout.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use figcraft_engine::capture::{CaptureBackend, CaptureError, CaptureErrorKind, CaptureJob, CaptureOutcome, JobId};
use serde_json::json;

use crate::{CaptureConfig, CaptureService, Timeouts};

const FAKE_SOURCE: &str = include_str!("../tests/fixtures/fake_sidecar.rs");
const WAIT_LIMIT: Duration = Duration::from_secs(20);
const ROOT: &str = "C:/projects/demo";
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

pub(crate) struct TempDir(PathBuf);

impl TempDir {
    pub(crate) fn new(label: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let name = format!("figcraft-capture-{}-{}-{label}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
        let path = std::env::temp_dir().join(name);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fake_node() -> &'static Path {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT.get_or_init(build_fake_node)
}

fn build_fake_node() -> PathBuf {
    let exe_dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
    let mut hasher = DefaultHasher::new();
    FAKE_SOURCE.hash(&mut hasher);
    let suffix = std::env::consts::EXE_SUFFIX;
    let target = exe_dir.join(format!("fake-sidecar-{:016x}{suffix}", hasher.finish()));
    if target.exists() {
        return target;
    }
    let partial = exe_dir.join(format!("fake-sidecar-partial-{}{suffix}", std::process::id()));
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake_sidecar.rs");
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc)
        .args(["--edition", "2021", "--crate-name", "fake_sidecar", "-C", "debuginfo=0", "-o"])
        .arg(&partial)
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "rustc could not build the fake sidecar: {}", String::from_utf8_lossy(&output.stderr));
    if fs::rename(&partial, &target).is_err() {
        let _ = fs::remove_file(&partial);
    }
    target
}

fn timeouts() -> Timeouts {
    Timeouts {
        hello: Duration::from_secs(10),
        install: Duration::from_secs(10),
        inspect: Duration::from_secs(10),
        render: Duration::from_secs(10),
        playground_open: Duration::from_secs(10),
        playground_frame: Duration::from_secs(10),
        playground: Duration::from_secs(10),
        shutdown: Duration::from_secs(2),
    }
}

fn fake_config(dir: &TempDir, settings: &[(&str, &str)], timeouts: Timeouts, max_response_bytes: usize) -> CaptureConfig {
    let mut lines = vec![format!("dir={}", dir.path().display())];
    lines.extend(settings.iter().map(|(key, value)| format!("{key}={value}")));
    let script = dir.path().join("scenario.txt");
    fs::write(&script, lines.join("\n")).unwrap();
    CaptureConfig { node: fake_node().to_path_buf(), script, timeouts, max_response_bytes }
}

fn start_service(config: CaptureConfig) -> (CaptureService, Arc<AtomicUsize>) {
    let wakes = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&wakes);
    let service = CaptureService::start(
        config,
        Box::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }),
    );
    (service, wakes)
}

fn fake_service(dir: &TempDir, settings: &[(&str, &str)], timeouts: Timeouts) -> (CaptureService, Arc<AtomicUsize>) {
    start_service(fake_config(dir, settings, timeouts, MAX_RESPONSE_BYTES))
}

fn inspect_job() -> CaptureJob {
    CaptureJob::Inspect { root: ROOT.to_string(), package: "@acme/ui".to_string() }
}

fn next_result(service: &mut CaptureService) -> (JobId, CaptureOutcome) {
    let deadline = Instant::now() + WAIT_LIMIT;
    loop {
        if let Some(first) = service.poll().into_iter().next() {
            return first;
        }
        assert!(Instant::now() < deadline, "no result within {WAIT_LIMIT:?}");
        thread::sleep(Duration::from_millis(10));
    }
}

fn run_job(service: &mut CaptureService, job: CaptureJob) -> CaptureOutcome {
    let id = service.submit(job);
    let (answered, outcome) = next_result(service);
    assert_eq!(answered, id);
    outcome
}

fn first_error(dir: &TempDir, settings: &[(&str, &str)], timeouts: Timeouts) -> CaptureError {
    let (mut service, _wakes) = fake_service(dir, settings, timeouts);
    run_job(&mut service, inspect_job()).unwrap_err()
}

fn launches(dir: &TempDir) -> usize {
    fs::read_to_string(dir.path().join("launches")).unwrap().lines().count()
}

fn heartbeat_len(dir: &TempDir) -> u64 {
    fs::metadata(dir.path().join("heartbeat")).map_or(0, |meta| meta.len())
}

fn wait_for_heartbeat(dir: &TempDir) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while heartbeat_len(dir) == 0 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    heartbeat_len(dir) > 0
}

fn assert_service_process_is_gone(dir: &TempDir) {
    thread::sleep(Duration::from_millis(250));
    let before = heartbeat_len(dir);
    thread::sleep(Duration::from_millis(250));
    assert_eq!(heartbeat_len(dir), before, "the fake service process is still running");
}

fn fault_then_recovery(dir: &TempDir, fault: &str, timeouts: Timeouts, max_response_bytes: usize) -> CaptureError {
    let config = fake_config(dir, &[("fault", fault)], timeouts, max_response_bytes);
    let (mut service, _wakes) = start_service(config);
    let error = run_job(&mut service, inspect_job()).unwrap_err();
    let next = run_job(&mut service, inspect_job()).unwrap();
    assert_eq!(next["request"]["method"], "inspect");
    assert_eq!(launches(dir), 2, "the failed service must be replaced by exactly one new process");
    error
}

fn mentions_delay(message: &str, delay: Duration) -> bool {
    let millis = delay.as_millis();
    let seconds = delay.as_secs_f64();
    [format!("{delay:?}"), format!("{millis}ms"), format!("{millis} ms"), format!("{seconds}s"), format!("{seconds} s")]
        .iter()
        .any(|form| message.contains(form.as_str()))
}

// covers: E2-5.1
#[test]
fn submit_returns_at_once_and_a_slow_result_arrives_by_poll_with_a_wake() {
    let dir = TempDir::new("slow");
    let (mut service, wakes) = fake_service(&dir, &[("delay_ms", "2000")], timeouts());

    let started = Instant::now();
    let id = service.submit(inspect_job());
    let blocked_for = started.elapsed();
    assert!(blocked_for < Duration::from_millis(50), "submit blocked for {blocked_for:?}");
    assert!(service.poll().is_empty());

    let (answered, outcome) = next_result(&mut service);
    assert_eq!(answered, id);
    assert_eq!(outcome.unwrap()["request"]["method"], "inspect");
    let deadline = Instant::now() + Duration::from_secs(1);
    while wakes.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(wakes.load(Ordering::SeqCst) >= 1, "wake was never called");
}

// covers: E2-5.1
#[test]
fn each_job_becomes_the_protocol_request_of_its_method() {
    let dir = TempDir::new("mapping");
    let (mut service, _wakes) = fake_service(&dir, &[], timeouts());
    let props = json!({"variant": "primary", "disabled": false});
    let cases = [
        (
            CaptureJob::Install { root: ROOT.to_string(), package: "@acme/ui".to_string(), version: "1.2.3".to_string() },
            "install",
            json!({"dir": ROOT, "package": "@acme/ui", "version": "1.2.3"}),
        ),
        (CaptureJob::Uninstall { root: ROOT.to_string(), package: "@acme/ui".to_string() }, "uninstall", json!({"dir": ROOT, "package": "@acme/ui"})),
        (inspect_job(), "inspect", json!({"dir": ROOT, "package": "@acme/ui"})),
        (
            CaptureJob::Render {
                root: ROOT.to_string(),
                package: "@acme/ui".to_string(),
                component: "Button".to_string(),
                props: props.clone(),
                sample_text: None,
                state: "hover".to_string(),
                theme: "dark".to_string(),
            },
            "render",
            json!({"dir": ROOT, "package": "@acme/ui", "component": "Button", "props": props, "state": "hover", "theme": "dark"}),
        ),
    ];
    for (job, method, params) in cases {
        let result = run_job(&mut service, job).unwrap();
        assert_eq!(result["request"]["method"], method);
        assert_eq!(result["request"]["params"], params);
        assert!(result["request"]["id"].as_u64().is_some_and(|id| id >= 1));
    }
}

// covers: E2-5.2
#[test]
fn every_protocol_error_kind_becomes_its_capture_error_kind_with_the_message_intact() {
    let expected = [
        ("invalidRequest", CaptureErrorKind::InvalidRequest),
        ("install", CaptureErrorKind::Install),
        ("uninstall", CaptureErrorKind::Uninstall),
        ("notInstalled", CaptureErrorKind::NotInstalled),
        ("componentNotFound", CaptureErrorKind::ComponentNotFound),
        ("frameworkUnsupported", CaptureErrorKind::FrameworkUnsupported),
        ("render", CaptureErrorKind::Render),
        ("tooLarge", CaptureErrorKind::TooLarge),
        ("internal", CaptureErrorKind::Internal),
    ];
    let message = "npm ERR! 404 'left-pad' est introuvable (voir le journal) \u{e9}";
    let kinds = expected.map(|(name, _)| name).join(",");
    let dir = TempDir::new("kinds");
    let (mut service, _wakes) = fake_service(&dir, &[("kinds", kinds.as_str()), ("message", message)], timeouts());
    for (name, kind) in expected {
        let error = run_job(&mut service, inspect_job()).unwrap_err();
        assert_eq!(error.kind, kind, "protocol kind {name}");
        assert_eq!(error.message, message, "protocol kind {name}");
    }
}

// covers: E2-5.3
#[test]
fn a_missing_node_is_a_start_error_naming_the_path_and_uigen_node() {
    let dir = TempDir::new("no-node");
    let missing = dir.path().join("no-such-node");
    let config =
        CaptureConfig { node: missing.clone(), script: dir.path().join("service.mjs"), timeouts: timeouts(), max_response_bytes: MAX_RESPONSE_BYTES };
    let (mut service, _wakes) = start_service(config);

    let error = run_job(&mut service, inspect_job()).unwrap_err();

    assert_eq!(error.kind, CaptureErrorKind::Start);
    assert!(error.message.contains(&missing.display().to_string()), "message: {}", error.message);
    assert!(error.message.contains("UIGEN_NODE"), "message: {}", error.message);
}

// covers: E2-5.3
#[test]
fn a_greeting_with_protocol_2_is_a_protocol_error_with_a_readable_message() {
    let dir = TempDir::new("hello-v2");
    let error = first_error(&dir, &[("hello", "v2")], timeouts());
    assert_eq!(error.kind, CaptureErrorKind::Protocol);
    assert!(error.message.contains("protocol 2"), "message: {}", error.message);
    assert!(error.message.contains("expects 1"), "message: {}", error.message);
}

// covers: E2-5.3
#[test]
fn a_service_that_exits_without_greeting_is_a_start_or_protocol_error() {
    let dir = TempDir::new("hello-absent");
    let error = first_error(&dir, &[("hello", "absent")], timeouts());
    assert!(matches!(error.kind, CaptureErrorKind::Start | CaptureErrorKind::Protocol), "kind: {:?}", error.kind);
    assert!(!error.message.trim().is_empty());
}

// covers: E2-5.3
#[test]
fn a_service_that_never_greets_fails_the_job_once_the_hello_delay_is_over() {
    let dir = TempDir::new("hello-silent");
    let short_hello = Timeouts { hello: Duration::from_millis(500), ..timeouts() };
    let error = first_error(&dir, &[("hello", "silent")], short_hello);
    let allowed = [CaptureErrorKind::Start, CaptureErrorKind::Protocol, CaptureErrorKind::Timeout];
    assert!(allowed.contains(&error.kind), "kind: {:?}", error.kind);
    assert!(!error.message.trim().is_empty());
}

// covers: E2-5.4
#[test]
fn a_response_longer_than_the_limit_is_a_protocol_error_then_a_new_service_succeeds() {
    let dir = TempDir::new("toolong");
    let error = fault_then_recovery(&dir, "toolong", timeouts(), 4096);
    assert_eq!(error.kind, CaptureErrorKind::Protocol);
    assert!(!error.message.trim().is_empty());
}

// covers: E2-5.4
#[test]
fn a_response_that_is_not_json_is_a_protocol_error_then_a_new_service_succeeds() {
    let dir = TempDir::new("notjson");
    let error = fault_then_recovery(&dir, "notjson", timeouts(), MAX_RESPONSE_BYTES);
    assert_eq!(error.kind, CaptureErrorKind::Protocol);
    assert!(!error.message.trim().is_empty());
}

// covers: E2-5.4
#[test]
fn an_exceeded_delay_is_a_timeout_naming_the_method_and_the_delay_then_a_new_service_succeeds() {
    let dir = TempDir::new("timeout");
    let delay = Duration::from_millis(500);
    let error = fault_then_recovery(&dir, "silent", Timeouts { inspect: delay, ..timeouts() }, MAX_RESPONSE_BYTES);
    assert_eq!(error.kind, CaptureErrorKind::Timeout);
    assert!(error.message.contains("inspect"), "message: {}", error.message);
    assert!(mentions_delay(&error.message, delay), "message: {}", error.message);
}

// covers: E2-5.4
#[test]
fn a_service_that_dies_while_working_is_service_exited_then_a_new_service_succeeds() {
    let dir = TempDir::new("died");
    let error = fault_then_recovery(&dir, "die", timeouts(), MAX_RESPONSE_BYTES);
    assert_eq!(error.kind, CaptureErrorKind::ServiceExited);
}

// covers: E2-5.5
#[test]
fn shutdown_sends_the_shutdown_request_and_the_service_process_ends() {
    let dir = TempDir::new("shutdown");
    let (mut service, _wakes) = fake_service(&dir, &[], timeouts());
    run_job(&mut service, inspect_job()).unwrap();
    assert!(wait_for_heartbeat(&dir), "the fake service never ran");

    let started = Instant::now();
    service.shutdown();

    assert!(dir.path().join("shutdown-received").exists(), "the service never received `shutdown`");
    assert!(started.elapsed() < timeouts().shutdown, "a service that answers must not cost the whole delay");
    assert_service_process_is_gone(&dir);
}

// covers: E2-5.5
#[test]
fn dropping_the_service_sends_the_shutdown_request_and_the_service_process_ends() {
    let dir = TempDir::new("drop");
    let (mut service, _wakes) = fake_service(&dir, &[], timeouts());
    run_job(&mut service, inspect_job()).unwrap();
    assert!(wait_for_heartbeat(&dir), "the fake service never ran");

    drop(service);

    assert!(dir.path().join("shutdown-received").exists(), "the service never received `shutdown`");
    assert_service_process_is_gone(&dir);
}

// covers: E2-5.5
#[test]
fn a_service_that_ignores_shutdown_is_killed_after_the_shutdown_delay() {
    let dir = TempDir::new("kill");
    let shutdown_delay = Duration::from_millis(400);
    let (mut service, _wakes) = fake_service(&dir, &[("shutdown", "ignore")], Timeouts { shutdown: shutdown_delay, ..timeouts() });
    run_job(&mut service, inspect_job()).unwrap();
    assert!(wait_for_heartbeat(&dir), "the fake service never ran");

    let started = Instant::now();
    service.shutdown();
    let waited = started.elapsed();

    assert!(dir.path().join("shutdown-received").exists(), "the service never received `shutdown`");
    assert!(waited >= shutdown_delay, "killed after {waited:?}, before the {shutdown_delay:?} delay");
    assert!(waited < Duration::from_secs(5), "shutdown took {waited:?}");
    assert_service_process_is_gone(&dir);
}
