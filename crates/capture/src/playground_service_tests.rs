//! The capture service driver and the `playground.*` methods, against the same fake sidecar as
//! `service_tests` (it echoes the request it received, or stays silent on the first request).

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use figcraft_engine::capture::{CaptureBackend, CaptureErrorKind, CaptureJob, CaptureOutcome};
use serde_json::{Value, json};

use crate::service_tests::TempDir;
use crate::{CaptureConfig, CaptureService, Timeouts};

const FAKE_SOURCE: &str = include_str!("../tests/fixtures/fake_sidecar.rs");
const WAIT_LIMIT: Duration = Duration::from_secs(60);
const ROOT: &str = "C:/projects/demo";
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

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
    let partial = exe_dir.join(format!("fake-sidecar-playground-partial-{}{suffix}", std::process::id()));
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

fn service(dir: &TempDir, settings: &[(&str, &str)]) -> CaptureService {
    let mut lines = vec![format!("dir={}", dir.path().display())];
    lines.extend(settings.iter().map(|(key, value)| format!("{key}={value}")));
    let script = dir.path().join("scenario.txt");
    fs::write(&script, lines.join("\n")).unwrap();
    let config = CaptureConfig { node: fake_node().to_path_buf(), script, timeouts: Timeouts::default(), max_response_bytes: MAX_RESPONSE_BYTES };
    CaptureService::start(config, Box::new(|| {}))
}

fn playground(method: &str, params: Value) -> CaptureJob {
    CaptureJob::Playground { root: ROOT.to_string(), method: method.to_string(), params }
}

fn inspect_job() -> CaptureJob {
    CaptureJob::Inspect { root: ROOT.to_string(), package: "@acme/ui".to_string() }
}

fn run_job(service: &mut CaptureService, job: CaptureJob) -> CaptureOutcome {
    let id = service.submit(job);
    let deadline = Instant::now() + WAIT_LIMIT;
    loop {
        if let Some((answered, outcome)) = service.poll().into_iter().next() {
            assert_eq!(answered, id);
            return outcome;
        }
        assert!(Instant::now() < deadline, "no result within {WAIT_LIMIT:?}");
        thread::sleep(Duration::from_millis(10));
    }
}

fn launches(dir: &TempDir) -> usize {
    fs::read_to_string(dir.path().join("launches")).unwrap().lines().count()
}

fn mentions_delay(message: &str, delay: Duration) -> bool {
    let millis = delay.as_millis();
    let seconds = delay.as_secs_f64();
    [format!("{delay:?}"), format!("{millis}ms"), format!("{millis} ms"), format!("{seconds}s"), format!("{seconds} s")]
        .iter()
        .any(|form| message.contains(form.as_str()))
}

fn silent_job_times_out_after(method: &str, params: Value, delay: Duration) {
    let dir = TempDir::new("playground-silent");
    let mut service = service(&dir, &[("fault", "silent")]);

    let started = Instant::now();
    let error = run_job(&mut service, playground(method, params)).unwrap_err();
    let waited = started.elapsed();

    assert_eq!(error.kind, CaptureErrorKind::Timeout, "message: {}", error.message);
    assert!(error.message.contains(method), "message: {}", error.message);
    assert!(mentions_delay(&error.message, delay), "message: {}", error.message);
    assert!(waited >= delay.mul_f64(0.9), "gave up after {waited:?}, before the {delay:?} delay");
    assert!(waited < delay + Duration::from_secs(4), "gave up after {waited:?}, long after the {delay:?} delay");
    let next = run_job(&mut service, inspect_job()).unwrap();
    assert_eq!(next["request"]["method"], "inspect");
    assert_eq!(launches(&dir), 2, "the service that timed out must be replaced by exactly one new process");
}

// covers: E7-4.1
#[test]
fn each_playground_job_becomes_the_protocol_request_of_its_method() {
    let dir = TempDir::new("playground-mapping");
    let mut service = service(&dir, &[]);
    let cases = [
        ("playground.open", json!({"width": 1280, "height": 800, "scale": 1.5, "theme": "dark", "tags": ["ui-button", "ui-card"]})),
        ("playground.setTemplate", json!({})),
        ("playground.apply", json!({"overrides": ":root[data-theme=\"light\"]{--ui-primary:#e5532d}", "theme": "light", "shape": "squircle"})),
        ("playground.resize", json!({"width": 800, "height": 600, "scale": 1})),
        ("playground.frame", json!({"after": 7, "format": "png"})),
        ("playground.input", json!({"events": [{"type": "move", "x": 1.5, "y": 2.5}, {"type": "key", "key": "Enter"}]})),
        ("playground.close", json!({})),
    ];
    for (method, params) in cases {
        let result = run_job(&mut service, playground(method, params.clone())).unwrap();
        let mut expected = params;
        expected["dir"] = json!(ROOT);
        assert_eq!(result["request"]["method"], method);
        assert_eq!(result["request"]["params"], expected, "{method}");
    }
}

// covers: E7-4.2
#[test]
fn a_playground_job_for_another_method_is_refused_and_the_service_keeps_working() {
    let dir = TempDir::new("playground-refused");
    let mut service = service(&dir, &[]);
    for method in ["shutdown", "install", "render", "inspect", "", "playground.", "playground.nope", "Playground.open"] {
        let error = run_job(&mut service, playground(method, json!({}))).unwrap_err();
        assert_eq!(error.kind, CaptureErrorKind::InvalidRequest, "method {method:?}: {}", error.message);
        assert!(!error.message.trim().is_empty(), "method {method:?}");
    }
    let next = run_job(&mut service, inspect_job()).unwrap();
    assert_eq!(next["request"]["method"], "inspect");
}

// covers: E7-4.5
#[test]
fn a_playground_frame_that_gets_no_answer_times_out_after_5_seconds_and_a_new_service_takes_over() {
    silent_job_times_out_after("playground.frame", json!({"after": 0}), Duration::from_secs(5));
}

// covers: E7-4.3
#[test]
fn the_other_playground_methods_time_out_after_10_seconds() {
    silent_job_times_out_after("playground.apply", json!({"theme": "dark"}), Duration::from_secs(10));
}

// covers: E7-4.3
#[test]
fn playground_open_may_take_longer_than_the_other_methods() {
    let dir = TempDir::new("playground-open-slow");
    let mut service = service(&dir, &[("delay_ms", "11000")]);

    let started = Instant::now();
    let result =
        run_job(&mut service, playground("playground.open", json!({"width": 800, "height": 600, "scale": 1, "theme": "light", "tags": []}))).unwrap();

    assert_eq!(result["request"]["method"], "playground.open");
    assert!(started.elapsed() >= Duration::from_secs(10), "the fake answered too early to prove anything");
}
