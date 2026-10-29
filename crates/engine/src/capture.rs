//! The capture port: what the engine asks of the service that installs npm packages and renders
//! their components. The engine never starts a process; the desktop apps install an adapter
//! (`figcraft-capture`) with [`Session::set_capture_backend`], the browser build installs none.

use std::collections::HashMap;
use std::fmt;

use serde_json::Value;

use crate::Session;

pub type JobId = u64;

#[derive(Debug, Clone, PartialEq)]
pub enum CaptureJob {
    Install { root: String, package: String, version: String },
    Uninstall { root: String, package: String },
    Inspect { root: String, package: String },
    Render { root: String, package: String, component: String, props: Value, state: String, theme: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureErrorKind {
    Unavailable,
    Start,
    Timeout,
    ServiceExited,
    Protocol,
    InvalidRequest,
    Install,
    Uninstall,
    NotInstalled,
    ComponentNotFound,
    FrameworkUnsupported,
    Render,
    TooLarge,
    Internal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaptureError {
    pub kind: CaptureErrorKind,
    pub message: String,
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CaptureError {}

pub type CaptureOutcome = Result<Value, CaptureError>;

pub trait CaptureBackend: Send {
    fn submit(&mut self, job: CaptureJob) -> JobId;
    fn poll(&mut self) -> Vec<(JobId, CaptureOutcome)>;
    fn shutdown(&mut self);
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaptureEvent {
    pub job: JobId,
    pub command: String,
    pub package: String,
    pub outcome: CaptureOutcome,
}

#[derive(Default)]
pub(crate) struct Capture {
    backend: Option<Box<dyn CaptureBackend>>,
    pending: HashMap<JobId, (String, String)>,
}

impl Capture {
    fn stop(&mut self) {
        self.pending.clear();
        if let Some(mut backend) = self.backend.take() {
            backend.shutdown();
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Session {
    pub fn set_capture_backend(&mut self, backend: Box<dyn CaptureBackend>) {
        self.capture.stop();
        self.capture.backend = Some(backend);
    }

    pub fn submit_capture(&mut self, command: &str, package: &str, job: CaptureJob) -> Result<JobId, CaptureError> {
        let capture = &mut self.capture;
        let Some(backend) = capture.backend.as_mut() else {
            return Err(CaptureError { kind: CaptureErrorKind::Unavailable, message: "capture service unavailable".to_string() });
        };
        let id = backend.submit(job);
        capture.pending.insert(id, (command.to_string(), package.to_string()));
        Ok(id)
    }

    pub fn pump_capture(&mut self) -> Vec<CaptureEvent> {
        let capture = &mut self.capture;
        let Some(backend) = capture.backend.as_mut() else {
            return Vec::new();
        };
        backend
            .poll()
            .into_iter()
            .filter_map(|(job, outcome)| {
                let (command, package) = capture.pending.remove(&job)?;
                Some(CaptureEvent { job, command, package, outcome })
            })
            .collect()
    }
}
