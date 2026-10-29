//! Localhost JSON-lines control server: each line `{"id", "method", "params"}` is queued for the UI
//! thread; the reply line `{"id", "ok", "result" | "error"}` is written when the frame handled it.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use figcraft_ui_egui::{ControlReply, ControlRequest, RequestGate, RequestState};
use serde_json::{Value, json};

type Queue = Arc<Mutex<Vec<ControlRequest>>>;
type CtxSlot = Arc<Mutex<Option<egui::Context>>>;

const MAX_LINE: usize = 32 * 1024 * 1024;
const MAX_AUTH_LINE: usize = 1024;
const TOKEN_BYTES: usize = 32;
const MAX_CONNECTIONS: usize = 8;
const MAX_QUEUED_REQUESTS: usize = 8;
const TIMEOUT: Duration = Duration::from_secs(60);
/// How long the accepting thread waits after a failed accept. Accepting can keep failing, for
/// example while the process has no file descriptors left, and retrying at once would keep a
/// core busy until it recovers.
const ACCEPT_RETRY: Duration = Duration::from_millis(100);

/// Why a request's transport wait ended without a control reply.
#[derive(Debug, PartialEq, Eq)]
enum WaitFailure {
    Timeout,
    /// The UI side dropped its sender before replying (the app is closing).
    Disconnected,
}

/// Wait for the UI's reply, and settle a timeout under the same gate the UI's final commit uses.
/// A commit that got the gate first has already queued its reply: consume that reply instead of
/// reporting a timeout. If the gate is still open, cancel it so the UI writes nothing.
fn wait_for_reply(rx: &std::sync::mpsc::Receiver<ControlReply>, gate: &RequestGate, timeout: Duration) -> Result<ControlReply, WaitFailure> {
    match rx.recv_timeout(timeout) {
        Ok(reply) => Ok(reply),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => after_timeout(rx, gate),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(WaitFailure::Disconnected),
    }
}

/// The timeout branch of [`wait_for_reply`]. Kept separate so it can be tested deterministically
/// against a gate whose commit already happened.
fn after_timeout(rx: &std::sync::mpsc::Receiver<ControlReply>, gate: &RequestGate) -> Result<ControlReply, WaitFailure> {
    match gate.cancel_if_open() {
        RequestState::Replied => rx.try_recv().map_err(|_| WaitFailure::Disconnected),
        RequestState::Open | RequestState::Cancelled => Err(WaitFailure::Timeout),
    }
}

/// Deferred requests for a hidden window must not make the UI's queue grow without a bound.
pub fn drain_queue(queue: &Queue, target: &mut Vec<ControlRequest>) {
    target.retain(|request| !request.is_cancelled());
    let mut queue = queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    queue.retain(|request| !request.is_cancelled());
    let count = queue.len().min(MAX_QUEUED_REQUESTS.saturating_sub(target.len()));
    target.extend(queue.drain(..count));
}

fn generate_token() -> String {
    let mut bytes = [0u8; TOKEN_BYTES];
    if getrandom::fill(&mut bytes).is_err() {
        return String::new();
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn token_matches(expected: &str, given: &str) -> bool {
    // Every byte of the expected token is compared, so the time taken does not tell how much of a guess is right.
    let given = given.as_bytes();
    let mut diff = expected.len() ^ given.len();
    for (i, &byte) in expected.as_bytes().iter().enumerate() {
        diff |= usize::from(byte ^ given.get(i).copied().unwrap_or(0));
    }
    diff == 0 && !expected.is_empty()
}

fn write_token_file(dir: &Path, port: u16, token: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = figcraft_engine::control_token::control_token_path(dir, port);
    match std::fs::remove_file(&path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(&path)?.write_all(token.as_bytes())?;
    Ok(path)
}

pub fn start(port: u16, queue: Queue, ctx: CtxSlot) {
    let token = generate_token();
    if token.is_empty() {
        log::error!("control server: the system has no secure random numbers, so no session token can be made");
        eprintln!("UIGEN control server not started: no secure random numbers for the session token");
        return;
    }
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            log::error!("control server: can't bind 127.0.0.1:{port}: {e} (another app may own it; pick another port)");
            eprintln!("UIGEN control server failed to bind port {port}: {e}");
            return;
        }
    };
    eprintln!("UIGEN control server listening on 127.0.0.1:{port}");
    let token_file = crate::services::config_dir()
        .ok_or_else(|| std::io::Error::other("no configuration folder"))
        .and_then(|dir| write_token_file(&dir, port, &token));
    match token_file {
        Ok(path) => eprintln!("UIGEN control token: {token} (also in {})", path.display()),
        Err(e) => {
            log::error!("control server: can't write the token file: {e}");
            eprintln!("UIGEN control token: {token} (the token file could not be written: {e})");
        }
    }
    let token: Arc<str> = Arc::from(token);
    let spawned = std::thread::Builder::new().name("figcraft-control".into()).spawn(move || {
        let mut workers = Vec::new();
        for stream in accepted(listener.incoming(), || std::thread::sleep(ACCEPT_RETRY)) {
            spawn_connection(stream, queue.clone(), ctx.clone(), &mut workers, TIMEOUT, token.clone());
        }
    });
    if let Err(e) = spawned {
        log::error!("control server thread: {e}");
    }
}

/// The connections `incoming` accepts, calling `wait` after each failed accept.
fn accepted<T>(incoming: impl Iterator<Item = std::io::Result<T>>, mut wait: impl FnMut()) -> impl Iterator<Item = T> {
    incoming.filter_map(move |stream| stream.map_err(|_| wait()).ok())
}

fn spawn_connection(
    stream: TcpStream,
    queue: Queue,
    ctx: CtxSlot,
    workers: &mut Vec<std::thread::JoinHandle<()>>,
    timeout: Duration,
    token: Arc<str>,
) {
    workers.retain(|worker| !worker.is_finished());
    if workers.len() >= MAX_CONNECTIONS {
        return;
    }
    match std::thread::Builder::new().name("figcraft-control-conn".into()).spawn(move || {
        let _ = serve(stream, queue, ctx, timeout, &token);
    }) {
        Ok(worker) => workers.push(worker),
        Err(e) => log::error!("control connection thread: {e}"),
    }
}

/// Read one line of at most `limit` bytes (plus its newline) into `line`, within `timeout` in
/// total: a client that sends one byte at a time cannot keep the connection open.
fn read_frame(reader: &mut BufReader<TcpStream>, line: &mut Vec<u8>, limit: usize, timeout: Duration) -> std::io::Result<usize> {
    let deadline = Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        reader.get_ref().set_read_timeout(Some(left))?;
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(line.len());
        }
        let room = limit + 1 - line.len();
        let window = available.get(..available.len().min(room)).unwrap_or(available);
        let newline = window.iter().position(|&b| b == b'\n');
        let used = newline.map_or(window.len(), |i| i + 1);
        line.extend_from_slice(window.get(..used).unwrap_or(window));
        reader.consume(used);
        if newline.is_some() || line.len() > limit {
            return Ok(line.len());
        }
    }
}

/// The first line of a connection must be `{"auth": "<token>"}`. Anything else ends it before any
/// request is read.
fn authenticated(reader: &mut BufReader<TcpStream>, out: &mut TcpStream, timeout: Duration, token: &str) -> std::io::Result<bool> {
    let mut line = Vec::new();
    if read_frame(reader, &mut line, MAX_AUTH_LINE, timeout)? == 0 {
        return Ok(false);
    }
    let given = std::str::from_utf8(&line).ok().and_then(|l| serde_json::from_str::<Value>(l).ok()).and_then(|v| match v {
        Value::Object(fields) if fields.len() == 1 => fields.get("auth").and_then(Value::as_str).map(str::to_string),
        _ => None,
    });
    if given.is_some_and(|given| token_matches(token, &given)) {
        return Ok(true);
    }
    let _ = writeln!(out, "{}", json!({"id": null, "ok": false, "error": "authentication failed: the first line must be {\"auth\": \"<token>\"}"}));
    Ok(false)
}

/// Serve one control connection: check the token, then read bounded JSON lines, reject oversized
/// frames and a full queue, queue one request, wait under its gate, and write one reply. Non-JSON
/// input answers with an error and closes the connection so HTTP body lines cannot become commands.
fn serve(mut out: TcpStream, queue: Queue, ctx: CtxSlot, timeout: Duration, token: &str) -> std::io::Result<()> {
    out.set_read_timeout(Some(timeout))?;
    out.set_write_timeout(Some(timeout))?;
    let mut reader = BufReader::new(out.try_clone()?);
    if !authenticated(&mut reader, &mut out, timeout, token)? {
        return Ok(());
    }
    let mut line = Vec::new();
    loop {
        line.clear();
        if read_frame(&mut reader, &mut line, MAX_LINE, timeout)? == 0 {
            break;
        }
        if line.last() == Some(&b'\n') {
            line.pop();
        }
        if line.len() > MAX_LINE {
            writeln!(out, "{}", json!({"id": null, "ok": false, "error": "message too large"}))?;
            break;
        }
        let Ok(line) = std::str::from_utf8(&line) else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(line) {
            Ok(v) => {
                let id = v.get("id").cloned().unwrap_or(Value::Null);
                let method = v.get("method").and_then(Value::as_str).unwrap_or("").to_string();
                let params = v.get("params").cloned().unwrap_or(json!({}));
                let (tx, rx) = std::sync::mpsc::channel();
                let gate = RequestGate::new();
                let mut pending = queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                pending.retain(|request| !request.is_cancelled());
                if pending.len() >= MAX_QUEUED_REQUESTS {
                    drop(pending);
                    writeln!(out, "{}", json!({"id": id, "ok": false, "error": "control request queue is full"}))?;
                    continue;
                }
                pending.push(ControlRequest { id: id.clone(), method, params, reply: tx, state: gate.clone() });
                drop(pending);
                if let Some(c) = ctx.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref() {
                    c.request_repaint();
                }
                match wait_for_reply(&rx, &gate, timeout) {
                    Ok(r) => match r.result {
                        Ok(res) => json!({"id": r.id, "ok": true, "result": res}),
                        Err(e) => json!({"id": r.id, "ok": false, "error": e}),
                    },
                    Err(WaitFailure::Timeout) => {
                        json!({"id": id, "ok": false, "error": "timed out waiting for the UI: screenshots and input need the UIGEN window on screen (is it minimised or covered?)"})
                    }
                    Err(WaitFailure::Disconnected) => {
                        json!({"id": id, "ok": false, "error": "the UIGEN window closed before answering"})
                    }
                }
            }
            Err(e) => {
                // A line that isn't JSON ends the connection: a web page can make the browser send
                // an HTTP request here, and its header lines must not let the JSON lines of its body
                // through as commands.
                writeln!(out, "{}", json!({"id": null, "ok": false, "error": format!("bad JSON: {e}")}))?;
                break;
            }
        };
        writeln!(out, "{reply}")?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "control_server_tests.rs"]
mod tests;
