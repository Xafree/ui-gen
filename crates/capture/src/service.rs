use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{Builder, JoinHandle};

use figcraft_engine::capture::{CaptureBackend, CaptureError, CaptureErrorKind, CaptureJob, CaptureOutcome, JobId};

use crate::config::CaptureConfig;
use crate::error;
use crate::process::Client;
use crate::protocol::request_of;

type Wake = Box<dyn Fn() + Send>;

pub struct CaptureService {
    next_id: JobId,
    jobs: Option<Sender<(JobId, CaptureJob)>>,
    results: Receiver<(JobId, CaptureOutcome)>,
    immediate: Vec<(JobId, CaptureOutcome)>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    down: CaptureError,
}

impl CaptureService {
    pub fn start(config: CaptureConfig, wake: Wake) -> Self {
        let (jobs, queue) = channel();
        let (finished, results) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let mut worker = Worker { config, wake, stop: Arc::clone(&stop), client: None };
        let spawned = Builder::new().name("uigen-capture".to_string()).spawn(move || worker.run(&queue, &finished));
        let (jobs, worker, down) = match spawned {
            Ok(handle) => (Some(jobs), Some(handle), error(CaptureErrorKind::Unavailable, "the capture service is shut down")),
            Err(cause) => (None, None, error(CaptureErrorKind::Start, format!("cannot start the capture thread: {cause}"))),
        };
        Self { next_id: 1, jobs, results, immediate: Vec::new(), stop, worker, down }
    }
}

impl CaptureBackend for CaptureService {
    fn submit(&mut self, job: CaptureJob) -> JobId {
        let id = self.next_id;
        self.next_id += 1;
        let queued = self.jobs.as_ref().is_some_and(|jobs| jobs.send((id, job)).is_ok());
        if !queued {
            self.immediate.push((id, Err(self.down.clone())));
        }
        id
    }

    fn poll(&mut self) -> Vec<(JobId, CaptureOutcome)> {
        let mut ready = std::mem::take(&mut self.immediate);
        ready.extend(self.results.try_iter());
        ready
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.jobs = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for CaptureService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct Worker {
    config: CaptureConfig,
    wake: Wake,
    stop: Arc<AtomicBool>,
    client: Option<Client>,
}

impl Worker {
    fn run(&mut self, queue: &Receiver<(JobId, CaptureJob)>, finished: &Sender<(JobId, CaptureOutcome)>) {
        while let Ok((id, job)) = queue.recv() {
            if self.stop.load(Ordering::SeqCst) {
                break;
            }
            let outcome = self.execute(job);
            if finished.send((id, outcome)).is_ok() {
                (self.wake)();
            }
        }
        if let Some(client) = self.client.take() {
            client.shutdown(self.config.timeouts.shutdown);
        }
    }

    fn execute(&mut self, job: CaptureJob) -> CaptureOutcome {
        let request = request_of(job, &self.config.timeouts);
        let client = match self.client.as_mut() {
            Some(client) => client,
            None => self.client.insert(Client::launch(&self.config, &self.stop)?),
        };
        let outcome = client.call(request.method, &request.params, request.timeout);
        if outcome.as_ref().is_err_and(|failure| breaks_service(failure.kind)) {
            self.client = None;
        }
        outcome
    }
}

fn breaks_service(kind: CaptureErrorKind) -> bool {
    matches!(kind, CaptureErrorKind::Timeout | CaptureErrorKind::ServiceExited | CaptureErrorKind::Protocol)
}
