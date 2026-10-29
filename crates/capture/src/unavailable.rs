use figcraft_engine::capture::{CaptureBackend, CaptureErrorKind, CaptureJob, CaptureOutcome, JobId};

use crate::config::CaptureConfig;
use crate::error;

pub struct CaptureService {
    next_id: JobId,
    ready: Vec<(JobId, CaptureOutcome)>,
    wake: Box<dyn Fn() + Send>,
}

impl CaptureService {
    pub fn start(_config: CaptureConfig, wake: Box<dyn Fn() + Send>) -> Self {
        Self { next_id: 1, ready: Vec::new(), wake }
    }
}

impl CaptureBackend for CaptureService {
    fn submit(&mut self, _job: CaptureJob) -> JobId {
        let id = self.next_id;
        self.next_id += 1;
        self.ready.push((id, Err(error(CaptureErrorKind::Unavailable, "capture service unavailable in the browser"))));
        (self.wake)();
        id
    }

    fn poll(&mut self) -> Vec<(JobId, CaptureOutcome)> {
        std::mem::take(&mut self.ready)
    }

    fn shutdown(&mut self) {}
}
