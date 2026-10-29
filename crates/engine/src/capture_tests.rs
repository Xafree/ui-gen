//! The capture port: a session without a backend stays quiet, one with a backend is asked for results.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::Session;
use crate::capture::{CaptureBackend, CaptureError, CaptureErrorKind, CaptureJob, JobId};

type Outcome = Result<Value, CaptureError>;

#[derive(Default)]
struct Calls {
    submitted: Vec<CaptureJob>,
    ready: Vec<(JobId, Outcome)>,
    polls: usize,
    shutdowns: usize,
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
        let mut calls = self.calls.lock().unwrap();
        calls.polls += 1;
        std::mem::take(&mut calls.ready)
    }

    fn shutdown(&mut self) {
        self.calls.lock().unwrap().shutdowns += 1;
    }
}

fn session_with_backend() -> (Session, Arc<Mutex<Calls>>) {
    let calls = Arc::new(Mutex::new(Calls::default()));
    let mut session = Session::new();
    session.set_capture_backend(Box::new(FakeBackend { calls: Arc::clone(&calls) }));
    (session, calls)
}

fn install_job() -> CaptureJob {
    CaptureJob::Install { root: "/p".to_string(), package: "react".to_string(), version: "19.0.0".to_string() }
}

fn submit(session: &mut Session, command: &str, package: &str, job: CaptureJob) -> JobId {
    match session.submit_capture(command, package, job) {
        Ok(id) => id,
        Err(error) => panic!("{command} was refused: {}", error.message),
    }
}

// covers: E2-4.3
#[test]
fn without_a_backend_pump_capture_returns_nothing() {
    let mut session = Session::new();
    assert!(session.pump_capture().is_empty());
    assert!(session.pump_capture().is_empty());
}

// covers: E2-4.3
#[test]
fn with_a_backend_that_has_no_result_pump_capture_polls_it_and_returns_nothing() {
    let (mut session, calls) = session_with_backend();
    assert!(session.pump_capture().is_empty());
    assert!(calls.lock().unwrap().polls >= 1);
    assert!(calls.lock().unwrap().submitted.is_empty());
    assert_eq!(calls.lock().unwrap().shutdowns, 0);
}

// covers: E2-4.3
#[test]
fn submit_capture_without_a_backend_is_unavailable() {
    let mut session = Session::new();
    match session.submit_capture("project.source.add", "react", install_job()) {
        Err(error) => assert!(matches!(error.kind, CaptureErrorKind::Unavailable), "{}", error.message),
        Ok(id) => panic!("job {id} was accepted without a backend"),
    }
}

// covers: E2-4.3
#[test]
fn a_finished_job_comes_out_of_pump_capture_once_with_its_command_and_package() {
    let (mut session, calls) = session_with_backend();
    let id = submit(&mut session, "project.source.add", "react", install_job());
    assert_eq!(calls.lock().unwrap().submitted.len(), 1);
    assert!(session.pump_capture().is_empty());

    calls.lock().unwrap().ready.push((id, Ok(json!({"package": "react", "version": "19.0.0"}))));
    let events = session.pump_capture();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].job, id);
    assert_eq!(events[0].command, "project.source.add");
    assert_eq!(events[0].package, "react");
    assert!(matches!(&events[0].outcome, Ok(value) if *value == json!({"package": "react", "version": "19.0.0"})));
    assert!(session.pump_capture().is_empty());
}

// covers: E2-4.3
#[test]
fn a_failed_job_comes_out_with_its_error_kind_and_message_untouched() {
    let (mut session, calls) = session_with_backend();
    let first = submit(&mut session, "project.source.add", "react", install_job());
    let second = submit(&mut session, "project.source.remove", "lit", CaptureJob::Uninstall { root: "/p".to_string(), package: "lit".to_string() });
    assert_ne!(first, second);

    let failure = CaptureError { kind: CaptureErrorKind::Install, message: "npm exited with code 1".to_string() };
    calls.lock().unwrap().ready.push((second, Err(failure)));
    let events = session.pump_capture();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].job, second);
    assert_eq!(events[0].command, "project.source.remove");
    assert_eq!(events[0].package, "lit");
    match &events[0].outcome {
        Err(error) => {
            assert!(matches!(error.kind, CaptureErrorKind::Install));
            assert_eq!(error.message, "npm exited with code 1");
        }
        Ok(value) => panic!("expected an error, got {value}"),
    }
}
