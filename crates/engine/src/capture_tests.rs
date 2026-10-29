//! The capture port: a session without a backend stays quiet, one with a backend is asked for results.

use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::Session;
use crate::capture::{CaptureBackend, CaptureError, CaptureJob, JobId};

#[derive(Default)]
struct Calls {
    submitted: Vec<CaptureJob>,
    polls: usize,
    shutdowns: usize,
}

struct FakeBackend {
    calls: Arc<Mutex<Calls>>,
    ready: Vec<(JobId, Result<Value, CaptureError>)>,
}

impl FakeBackend {
    fn new(calls: Arc<Mutex<Calls>>) -> Self {
        Self { calls, ready: Vec::new() }
    }
}

impl CaptureBackend for FakeBackend {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let mut calls = self.calls.lock().unwrap();
        calls.submitted.push(job);
        u64::try_from(calls.submitted.len()).unwrap()
    }

    fn poll(&mut self) -> Vec<(JobId, Result<Value, CaptureError>)> {
        self.calls.lock().unwrap().polls += 1;
        std::mem::take(&mut self.ready)
    }

    fn shutdown(&mut self) {
        self.calls.lock().unwrap().shutdowns += 1;
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
    let calls = Arc::new(Mutex::new(Calls::default()));
    let mut session = Session::new();
    session.set_capture_backend(Box::new(FakeBackend::new(Arc::clone(&calls))));
    assert!(session.pump_capture().is_empty());
    assert!(calls.lock().unwrap().polls >= 1);
    assert!(calls.lock().unwrap().submitted.is_empty());
    assert_eq!(calls.lock().unwrap().shutdowns, 0);
}
