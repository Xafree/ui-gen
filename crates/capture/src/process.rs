use std::io::{BufRead, ErrorKind, Read, Write};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use figcraft_engine::capture::{CaptureError, CaptureErrorKind};
use serde_json::{Value, json};

use crate::config::CaptureConfig;
use crate::error;
use crate::protocol::{SHUTDOWN_METHOD, check_hello, decode_response, encode};

const STOP_POLL: Duration = Duration::from_millis(50);
const STDERR_TAIL_BYTES: usize = 2048;
const HELLO: &str = "hello";

enum Incoming {
    Line(Vec<u8>),
    TooLong,
    Failed(String),
}

enum Wait {
    Line(Vec<u8>),
    TooLong,
    Failed(String),
    Closed,
    TimedOut,
    Stopped,
}

type StderrTail = Arc<Mutex<Vec<u8>>>;

pub(crate) struct Client {
    child: Child,
    requests: Sender<Vec<u8>>,
    lines: Receiver<Incoming>,
    stderr: StderrTail,
    stop: Arc<AtomicBool>,
    max_response_bytes: usize,
    next_id: u64,
}

impl Client {
    pub(crate) fn launch(config: &CaptureConfig, stop: &Arc<AtomicBool>) -> Result<Self, CaptureError> {
        let mut command = Command::new(&config.node);
        command.arg(&config.script).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        detach(&mut command);
        let mut child = command.spawn().map_err(|cause| {
            error(
                CaptureErrorKind::Start,
                format!("cannot start node at `{}`: {cause}; install Node.js or set UIGEN_NODE to the node executable", config.node.display()),
            )
        })?;
        let (Some(stdin), Some(stdout), Some(stderr_pipe)) = (child.stdin.take(), child.stdout.take(), child.stderr.take()) else {
            kill_tree(&mut child);
            return Err(error(CaptureErrorKind::Start, "the capture service has no stdio pipes"));
        };
        let (requests, queued) = channel();
        let (sender, lines) = channel();
        let stderr = StderrTail::default();
        let limit = config.max_response_bytes;
        let drained = Arc::clone(&stderr);
        let started = spawn("uigen-capture-writer", move || write_requests(stdin, &queued))
            .and_then(|()| spawn("uigen-capture-reader", move || read_lines(std::io::BufReader::new(stdout), limit, &sender)))
            .and_then(|()| spawn("uigen-capture-stderr", move || drain(stderr_pipe, &drained)));
        let mut client = Self { child, requests, lines, stderr, stop: Arc::clone(stop), max_response_bytes: limit, next_id: 1 };
        started?;
        client.expect_hello(config.timeouts.hello)?;
        Ok(client)
    }

    pub(crate) fn call(&mut self, method: &str, params: &Value, delay: Duration) -> Result<Value, CaptureError> {
        let id = self.next_id;
        self.next_id += 1;
        if self.requests.send(encode(id, method, params)).is_err() {
            return Err(error(CaptureErrorKind::ServiceExited, self.exit_message(method)));
        }
        match self.wait(Instant::now() + delay, true) {
            Wait::Line(line) => decode_response(&line, id),
            Wait::TooLong => Err(self.too_long(method)),
            Wait::Failed(cause) => Err(error(CaptureErrorKind::ServiceExited, format!("cannot read the answer to `{method}`: {cause}"))),
            Wait::Closed => Err(error(CaptureErrorKind::ServiceExited, self.exit_message(method))),
            Wait::TimedOut => Err(error(CaptureErrorKind::Timeout, format!("`{method}` took longer than {delay:?}"))),
            Wait::Stopped => Err(error(CaptureErrorKind::ServiceExited, "the capture service was stopped")),
        }
    }

    pub(crate) fn shutdown(self, delay: Duration) {
        let id = self.next_id;
        if self.requests.send(encode(id, SHUTDOWN_METHOD, &json!({}))).is_err() {
            return;
        }
        let deadline = Instant::now() + delay;
        while matches!(self.wait(deadline, false), Wait::Line(_)) {}
    }

    fn expect_hello(&mut self, delay: Duration) -> Result<(), CaptureError> {
        match self.wait(Instant::now() + delay, true) {
            Wait::Line(line) => check_hello(&line),
            Wait::TooLong => Err(self.too_long(HELLO)),
            Wait::Failed(cause) => Err(error(CaptureErrorKind::Start, format!("cannot read the capture service greeting: {cause}"))),
            Wait::Closed => Err(error(CaptureErrorKind::Start, format!("the capture service exited before greeting{}", self.exit_details()))),
            Wait::TimedOut => Err(error(CaptureErrorKind::Timeout, format!("the capture service did not greet within {delay:?}"))),
            Wait::Stopped => Err(error(CaptureErrorKind::ServiceExited, "the capture service was stopped")),
        }
    }

    fn wait(&self, deadline: Instant, honor_stop: bool) -> Wait {
        loop {
            if honor_stop && self.stop.load(Ordering::SeqCst) {
                return Wait::Stopped;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Wait::TimedOut;
            }
            match self.lines.recv_timeout(remaining.min(STOP_POLL)) {
                Ok(Incoming::Line(line)) => return Wait::Line(line),
                Ok(Incoming::TooLong) => return Wait::TooLong,
                Ok(Incoming::Failed(cause)) => return Wait::Failed(cause),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Wait::Closed,
            }
        }
    }

    fn too_long(&self, method: &str) -> CaptureError {
        error(CaptureErrorKind::Protocol, format!("the answer to `{method}` is longer than {} bytes", self.max_response_bytes))
    }

    fn exit_message(&mut self, method: &str) -> String {
        format!("the capture service exited while running `{method}`{}", self.exit_details())
    }

    fn exit_details(&mut self) -> String {
        let status = self.child.try_wait().ok().flatten().map(|status: ExitStatus| format!(" ({status})")).unwrap_or_default();
        let tail = self.stderr.lock().unwrap_or_else(PoisonError::into_inner);
        let tail = String::from_utf8_lossy(&tail);
        let tail = tail.trim();
        if tail.is_empty() { status } else { format!("{status}: {tail}") }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        kill_tree(&mut self.child);
    }
}

fn spawn(name: &str, work: impl FnOnce() + Send + 'static) -> Result<(), CaptureError> {
    thread::Builder::new()
        .name(name.to_string())
        .spawn(work)
        .map(drop)
        .map_err(|cause| error(CaptureErrorKind::Start, format!("cannot start the {name} thread: {cause}")))
}

fn write_requests(mut stdin: ChildStdin, requests: &Receiver<Vec<u8>>) {
    for line in requests {
        if stdin.write_all(&line).and_then(|()| stdin.flush()).is_err() {
            return;
        }
    }
}

fn read_lines(mut source: impl BufRead, limit: usize, out: &Sender<Incoming>) {
    let mut line = Vec::new();
    loop {
        let chunk = match source.fill_buf() {
            Ok(chunk) => chunk,
            Err(cause) if cause.kind() == ErrorKind::Interrupted => continue,
            Err(cause) => {
                let _ = out.send(Incoming::Failed(cause.to_string()));
                return;
            }
        };
        if chunk.is_empty() {
            if !line.is_empty() {
                let _ = out.send(Incoming::Line(line));
            }
            return;
        }
        let end = chunk.iter().position(|byte| *byte == b'\n');
        let taken = end.map_or(chunk.len(), |end| end + 1);
        let content = chunk.get(..end.unwrap_or(chunk.len())).unwrap_or_default();
        if line.len() + content.len() > limit {
            let _ = out.send(Incoming::TooLong);
            return;
        }
        line.extend_from_slice(content);
        source.consume(taken);
        if end.is_some() {
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if !line.is_empty() && out.send(Incoming::Line(std::mem::take(&mut line))).is_err() {
                return;
            }
        }
    }
}

fn drain(mut source: impl Read, tail: &StderrTail) {
    let mut buffer = [0u8; 4096];
    loop {
        match source.read(&mut buffer) {
            Ok(0) => return,
            Ok(count) => {
                let mut kept = tail.lock().unwrap_or_else(PoisonError::into_inner);
                kept.extend_from_slice(buffer.get(..count).unwrap_or_default());
                let excess = kept.len().saturating_sub(STDERR_TAIL_BYTES);
                kept.drain(..excess);
            }
            Err(cause) if cause.kind() == ErrorKind::Interrupted => {}
            Err(_) => return,
        }
    }
}

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(windows)]
fn kill_descendants(pid: u32) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = Command::new("taskkill")
        .args(["/T", "/F", "/PID", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

#[cfg(unix)]
fn kill_descendants(pid: u32) {
    let _ = Command::new("kill").args(["-KILL", "--", &format!("-{pid}")]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
}

fn kill_tree(child: &mut Child) {
    if matches!(child.try_wait(), Ok(Some(_))) {
        return;
    }
    kill_descendants(child.id());
    let _ = child.kill();
    let _ = child.wait();
}
