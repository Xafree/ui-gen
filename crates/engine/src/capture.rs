//! The capture port: what the engine asks of the service that installs npm packages and renders
//! their components. The engine never starts a process; the desktop apps install an adapter
//! (`figcraft-capture`) with [`Session::set_capture_backend`], the browser build installs none.

use std::fmt;

use serde_json::Value;

use crate::Session;
use crate::uigen_import::{self, PendingImport};

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

const MAX_FINISHED_JOBS: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobState {
    Queued,
    Running,
    Done,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobStatus {
    pub job: JobId,
    pub command: String,
    pub package: String,
    pub root: String,
    pub state: JobState,
}

struct JobRecord {
    id: JobId,
    command: String,
    package: String,
    root: String,
    finished: Option<Result<(), String>>,
}

impl CaptureJob {
    pub(crate) fn root(&self) -> &str {
        match self {
            Self::Install { root, .. } | Self::Uninstall { root, .. } | Self::Inspect { root, .. } | Self::Render { root, .. } => root,
        }
    }
}

#[derive(Default)]
pub(crate) struct Capture {
    backend: Option<Box<dyn CaptureBackend>>,
    jobs: Vec<JobRecord>,
    pub(crate) imports: Vec<PendingImport>,
}

impl Capture {
    fn stop(&mut self) {
        self.imports.clear();
        self.jobs.retain(|record| record.finished.is_some());
        if let Some(mut backend) = self.backend.take() {
            backend.shutdown();
        }
    }

    pub(crate) fn finish(&mut self, job: JobId, result: Result<(), String>) {
        if let Some(record) = self.jobs.iter_mut().find(|record| record.id == job && record.finished.is_none()) {
            record.finished = Some(result);
        }
    }

    fn forget_oldest_finished(&mut self) {
        let finished = self.jobs.iter().filter(|record| record.finished.is_some()).count();
        for _ in MAX_FINISHED_JOBS..finished {
            if let Some(oldest) = self.jobs.iter().position(|record| record.finished.is_some()) {
                self.jobs.remove(oldest);
            }
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
        self.capture.jobs.clear();
        self.capture.backend = Some(backend);
    }

    pub fn submit_capture(&mut self, command: &str, package: &str, job: CaptureJob) -> Result<JobId, CaptureError> {
        let capture = &mut self.capture;
        let Some(backend) = capture.backend.as_mut() else {
            return Err(CaptureError { kind: CaptureErrorKind::Unavailable, message: unavailable_message().to_string() });
        };
        let root = job.root().to_string();
        let id = backend.submit(job);
        capture.jobs.push(JobRecord { id, command: command.to_string(), package: package.to_string(), root, finished: None });
        Ok(id)
    }

    /// Bring back the jobs the service finished. A finished install or uninstall of a source is
    /// applied to `uigen.json` first; one that can't be applied comes out as a failure.
    pub fn pump_capture(&mut self) -> Vec<CaptureEvent> {
        let Some(backend) = self.capture.backend.as_mut() else {
            return Vec::new();
        };
        let mut events = Vec::new();
        for (job, outcome) in backend.poll() {
            let Some(position) = self.capture.jobs.iter().position(|record| record.id == job && record.finished.is_none()) else {
                continue;
            };
            let Some(record) = self.capture.jobs.get(position) else { continue };
            let (command, package, root) = (record.command.clone(), record.package.clone(), record.root.clone());
            if command == uigen_import::COMMAND {
                events.extend(uigen_import::advance(self, job, outcome));
                continue;
            }
            let outcome = outcome.and_then(|value| crate::cmd::project_sources::apply(self, &command, &package, &root, &value).map(|()| value));
            if let Some(record) = self.capture.jobs.get_mut(position) {
                record.finished = Some(outcome.as_ref().map(|_| ()).map_err(|e| e.message.clone()));
            }
            events.push(CaptureEvent { job, command, package, outcome });
        }
        self.capture.forget_oldest_finished();
        events
    }

    /// The jobs submitted to the capture service, oldest first, with the project folder each one works in.
    pub fn capture_jobs(&self) -> Vec<JobStatus> {
        let mut running = true;
        self.capture
            .jobs
            .iter()
            .map(|record| {
                let state = match &record.finished {
                    Some(Ok(())) => JobState::Done,
                    Some(Err(message)) => JobState::Failed(message.clone()),
                    None if std::mem::take(&mut running) => JobState::Running,
                    None => JobState::Queued,
                };
                JobStatus { job: record.id, command: record.command.clone(), package: record.package.clone(), root: record.root.clone(), state }
            })
            .collect()
    }
}

#[cfg(target_arch = "wasm32")]
fn unavailable_message() -> &'static str {
    "capture service unavailable in the browser"
}

#[cfg(not(target_arch = "wasm32"))]
fn unavailable_message() -> &'static str {
    "capture service not available"
}
