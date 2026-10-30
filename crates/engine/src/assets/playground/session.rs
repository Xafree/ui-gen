//! The playground a session has open: its status, the document and export it shows, the jobs it has
//! in flight with the capture service, and the latest image the page sent back. Every call goes
//! through the capture port and is tracked here, so none of them appears with the capture jobs.

use std::collections::BTreeMap;
use std::sync::Arc;

use base64::Engine as _;
use figcraft_doc::Document;
use serde_json::{Value, json};

use super::Template;
use crate::Session;
use crate::capture::{CaptureError, CaptureErrorKind, CaptureEvent, CaptureOutcome, JobId};
use crate::lit_export::ExportPlan;

const MAX_FRAME_BYTES: usize = 8 << 20;
const MAX_FRAME_BASE64: usize = MAX_FRAME_BYTES.div_ceil(3) * 4;
const OPEN_COMMAND: &str = "playground.open";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Closed,
    Opening,
    Ready,
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameFormat {
    Jpeg,
    Png,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub seq: u64,
    pub format: FrameFormat,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Window {
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

pub(crate) struct Target {
    pub uid: u64,
    pub root: String,
    pub doc: Arc<Document>,
    pub template: Template,
    pub plan: ExportPlan,
    pub window: Window,
}

enum Purpose {
    Open { command: &'static str, current: bool },
    SetTemplate,
    Frame,
    Input,
}

#[derive(Default)]
pub(crate) struct PlaygroundState {
    status: Status,
    target: Option<Target>,
    jobs: BTreeMap<JobId, Purpose>,
    last_seq: u64,
    latest: Option<Frame>,
}

impl PlaygroundState {
    pub(crate) fn owns(&self, job: JobId) -> bool {
        self.jobs.contains_key(&job)
    }

    pub(crate) fn status(&self) -> &Status {
        &self.status
    }

    pub(crate) fn target(&self) -> Option<&Target> {
        self.target.as_ref()
    }

    pub(crate) fn target_mut(&mut self) -> Option<&mut Target> {
        self.target.as_mut()
    }

    fn fail(&mut self, message: String) {
        if matches!(self.status, Status::Opening | Status::Ready) {
            self.status = Status::Failed(message);
        }
    }

    fn waiting_for_frame(&self) -> bool {
        self.jobs.values().any(|purpose| matches!(purpose, Purpose::Frame))
    }

    fn forget_pending(&mut self) {
        self.jobs.retain(|_, purpose| matches!(purpose, Purpose::Open { .. }));
        for purpose in self.jobs.values_mut() {
            if let Purpose::Open { current, .. } = purpose {
                *current = false;
            }
        }
    }
}

fn breaks_service(kind: CaptureErrorKind) -> bool {
    matches!(kind, CaptureErrorKind::Timeout | CaptureErrorKind::ServiceExited | CaptureErrorKind::Protocol | CaptureErrorKind::Unavailable)
}

fn frame_of(answer: &Value, after: u64) -> Option<Frame> {
    let seq = answer.get("seq")?.as_u64().filter(|seq| *seq > after)?;
    if !answer.get("changed")?.as_bool()? {
        return None;
    }
    let format = match answer.get("format")?.as_str()? {
        "jpeg" => FrameFormat::Jpeg,
        "png" => FrameFormat::Png,
        _ => return None,
    };
    let dimension = |key: &str| u32::try_from(answer.get(key)?.as_u64()?).ok().filter(|size| *size > 0);
    let (width, height) = (dimension("width")?, dimension("height")?);
    let text = answer.get("data")?.as_str().filter(|text| text.len() <= MAX_FRAME_BASE64)?;
    let data = base64::engine::general_purpose::STANDARD.decode(text).ok().filter(|data| !data.is_empty() && data.len() <= MAX_FRAME_BYTES)?;
    Some(Frame { seq, format, width, height, data })
}

pub(crate) fn advance(s: &mut Session, job: JobId, outcome: CaptureOutcome) -> Option<CaptureEvent> {
    let state = &mut s.capture.playground;
    match state.jobs.remove(&job)? {
        Purpose::Open { command, current } => {
            if current {
                match &outcome {
                    Ok(_) => state.status = Status::Ready,
                    Err(cause) => state.status = Status::Failed(cause.message.clone()),
                }
            }
            Some(CaptureEvent { job, command: command.to_string(), package: String::new(), outcome })
        }
        Purpose::Frame => {
            match outcome {
                Ok(answer) => {
                    if let Some(frame) = frame_of(&answer, state.last_seq) {
                        state.last_seq = frame.seq;
                        state.latest = Some(frame);
                    }
                }
                Err(cause) if cause.kind == CaptureErrorKind::TooLarge => {}
                Err(cause) => state.fail(cause.message),
            }
            None
        }
        Purpose::SetTemplate => {
            if let Err(cause) = outcome {
                state.fail(cause.message);
            }
            None
        }
        Purpose::Input => {
            if let Err(cause) = outcome
                && breaks_service(cause.kind)
            {
                state.fail(cause.message);
            }
            None
        }
    }
}

impl Session {
    pub fn playground_status(&self) -> &Status {
        self.capture.playground.status()
    }

    /// Ask for a newer image of the page when it is ready and nothing precedes the request.
    pub fn playground_poll(&mut self) -> bool {
        self.playground_sync();
        let state = &self.capture.playground;
        if *state.status() != Status::Ready || state.waiting_for_frame() || self.capture.busy() {
            return false;
        }
        let after = state.last_seq;
        self.relay_playground("playground.frame", json!({"after": after}), Purpose::Frame).is_ok()
    }

    /// The latest image the page sent, once.
    pub fn playground_take_frame(&mut self) -> Option<Frame> {
        self.capture.playground.latest.take()
    }

    /// Close the playground whose document tab is gone.
    pub(crate) fn playground_sync(&mut self) {
        let gone = self.capture.playground.target().is_some_and(|target| !self.docs.iter().any(|ds| ds.uid == target.uid));
        if gone {
            self.playground_close();
        }
    }

    /// Back to `Closed`; the page is told to close, and the answers still on their way are dropped.
    pub(crate) fn playground_close(&mut self) {
        let state = &mut self.capture.playground;
        let Some(target) = state.target.take() else { return };
        state.status = Status::Closed;
        state.latest = None;
        state.last_seq = 0;
        state.forget_pending();
        if let Err(cause) = self.submit_playground(&target.root, "playground.close", json!({})) {
            log::warn!("playground.close was not sent: {}", cause.message);
        }
    }

    /// The page is being opened for `target`, by the job the command answered with.
    pub(crate) fn playground_opening(&mut self, command: &'static str, target: Target, params: Value) -> Result<JobId, CaptureError> {
        let job = self.submit_playground(&target.root, OPEN_COMMAND, params)?;
        let state = &mut self.capture.playground;
        state.status = Status::Opening;
        state.target = Some(target);
        state.last_seq = 0;
        state.latest = None;
        state.jobs.insert(job, Purpose::Open { command, current: true });
        Ok(job)
    }

    pub(crate) fn playground_set_template(&mut self) -> Result<JobId, CaptureError> {
        self.relay_playground("playground.setTemplate", json!({}), Purpose::SetTemplate)
    }

    pub(crate) fn playground_input(&mut self, events: Value) -> Result<JobId, CaptureError> {
        self.relay_playground("playground.input", json!({"events": events}), Purpose::Input)
    }

    fn relay_playground(&mut self, method: &str, params: Value, purpose: Purpose) -> Result<JobId, CaptureError> {
        let root = self
            .capture
            .playground
            .target()
            .map(|target| target.root.clone())
            .ok_or_else(|| CaptureError { kind: CaptureErrorKind::InvalidRequest, message: "the playground is not open".to_string() })?;
        let job = self.submit_playground(&root, method, params)?;
        self.capture.playground.jobs.insert(job, purpose);
        Ok(job)
    }
}
