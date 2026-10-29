//! Where MCP tool calls end up: a control-channel method call, answered either by an in-process
//! engine session ([`crate::Headless`]) or by a running FigCraft app ([`Attached`]).

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

use base64::Engine as _;
use serde_json::{Value, json};

/// Something that answers the FigCraft control-channel methods (`engine.execute`,
/// `engine.commands`, `document.inspect`, `node.get`, `ui.pointer`, `ui.key`, `ui.type`,
/// `ui.render`, `ui.screenshot`, `ui.inspect`). See `docs/control-protocol.md`.
pub trait Backend {
    /// Call one method. `Ok` carries the `result`, `Err` the error message.
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String>;

    /// True when a real UI window is attached (`ui.screenshot` captures the window).
    fn has_ui(&self) -> bool;

    /// Short human description ("headless", "attached to 127.0.0.1:7979").
    fn describe(&self) -> String;

    /// PNG bytes of the document: `ids` (in page coordinates, their union) or, when empty, the
    /// current page's content. The default goes through `ui.render`, the PNG coming back as data.
    fn render_png(&mut self, ids: &[u64], scale: f64) -> Result<Vec<u8>, String> {
        let mut params = json!({"scale": scale});
        if !ids.is_empty() {
            params["ids"] = json!(ids);
        } else {
            // `ui.render` without ids exports the selection; pass the page's children instead.
            let kids = page_children(self)?;
            if kids.is_empty() {
                return Err("the current page is empty: nothing to render".into());
            }
            params["ids"] = json!(kids);
        }
        decode_data(&self.call("ui.render", params)?)
    }

    /// PNG bytes of what the user sees: the app window when attached, the current viewport when
    /// headless.
    fn screenshot_png(&mut self) -> Result<Vec<u8>, String> {
        capture(self, json!({}))
    }

    /// PNG bytes of the attached app's window `window` (an id from `window.list`).
    fn window_png(&mut self, window: u64) -> Result<Vec<u8>, String> {
        capture(self, json!({"window": window}))
    }
}

/// A window capture, the PNG coming back as data (no file: an app confined to automation roots
/// writes none outside them).
fn capture<B: Backend + ?Sized>(b: &mut B, params: Value) -> Result<Vec<u8>, String> {
    decode_data(&b.call("ui.screenshot", params)?)
}

/// The ids of the current page's top-level layers.
pub fn page_children<B: Backend + ?Sized>(b: &mut B) -> Result<Vec<u64>, String> {
    let doc = b.call("document.inspect", json!({"depth": 1}))?;
    let current = doc.get("currentPage").cloned().unwrap_or(Value::Null);
    let pages = doc.get("pages").and_then(Value::as_array).cloned().unwrap_or_default();
    let page = pages.iter().find(|p| p.get("id") == Some(&current)).or_else(|| pages.first());
    Ok(page
        .and_then(|p| p.get("children"))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|c| c.get("id").and_then(Value::as_u64)).collect())
        .unwrap_or_default())
}

/// Decode a `data` field (base64, optionally a data: URL) from an export result.
pub fn decode_data(v: &Value) -> Result<Vec<u8>, String> {
    let d = v.get("data").and_then(Value::as_str).ok_or("the export returned no data")?;
    let d = d.split_once(',').map(|(_, b)| b).unwrap_or(d);
    base64::engine::general_purpose::STANDARD.decode(d.trim()).map_err(|e| format!("bad base64 from the app: {e}"))
}

/// How long a new connection waits for the app to acknowledge or refuse its token.
const AUTH_ANSWER_TIMEOUT: Duration = Duration::from_secs(5);

/// What the app answers to a connection it has no room for.
const FULL_REFUSAL: &str = "too many connections";

/// The variable that gives the control channel's session token, ahead of the app's token file.
pub const TOKEN_ENV: &str = "FIGCRAFT_CONTROL_TOKEN";

/// The hint every "can't reach the app" message ends with.
pub const START_HINT: &str = "start it with `uigen --control PORT --automation-read-root DIR` (and/or `--automation-write-root DIR`): the control channel needs a folder to confine file access to, and a session token";

/// The session token of the app listening on `port`: `env` (the value of [`TOKEN_ENV`]) when it is
/// not empty, else the `control-token-<port>` file the app wrote in its configuration folder
/// `config_dir`. The error names that file and the variable.
pub fn control_token_from(env: Option<&str>, config_dir: Option<&Path>, port: u16) -> Result<String, String> {
    if let Some(token) = env.map(str::trim).filter(|t| !t.is_empty()) {
        return Ok(token.to_string());
    }
    let expected = match config_dir {
        Some(dir) => figcraft_engine::control_token::control_token_path(dir, port).display().to_string(),
        None => format!("control-token-{port} in the UIGEN configuration folder"),
    };
    let missing = |why: String| {
        format!(
            "no control token for port {port}: {why}; the app writes its session token to {expected} when it starts with `--control {port}`; set {TOKEN_ENV} to use another"
        )
    };
    let Some(dir) = config_dir else { return Err(missing("no configuration folder".into())) };
    let path = figcraft_engine::control_token::control_token_path(dir, port);
    match std::fs::read_to_string(&path) {
        Ok(text) if !text.trim().is_empty() => Ok(text.trim().to_string()),
        Ok(_) => Err(missing(format!("{} is empty", path.display()))),
        Err(e) => Err(missing(format!("can't read {}: {e}", path.display()))),
    }
}

/// [`control_token_from`] with the real environment and the app's real configuration folder.
pub fn control_token(port: u16) -> Result<String, String> {
    let env = std::env::var(TOKEN_ENV).ok();
    control_token_from(env.as_deref(), figcraft_engine::control_token::config_dir().as_deref(), port)
}

fn closed_after_token() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::ConnectionAborted, "the app closed the connection (too many connections or not authenticated)")
}

/// The app's answer to the token line: `Ok` for its acknowledgement, else the reason it gave.
fn check_acknowledgement(answer: &str) -> std::io::Result<()> {
    let Ok(v) = serde_json::from_str::<Value>(answer.trim()) else {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("bad answer from the app after the token: {}", answer.trim())));
    };
    if v.get("ok").and_then(Value::as_bool) == Some(true) {
        return Ok(());
    }
    let reason = v.get("error").and_then(Value::as_str).unwrap_or("no reason given");
    if reason == FULL_REFUSAL {
        return Err(std::io::Error::new(std::io::ErrorKind::ConnectionRefused, format!("the app has no connection left for this client ({reason})")));
    }
    let message = if reason.starts_with("authentication failed") { reason.to_string() } else { format!("authentication failed: {reason}") };
    Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, message))
}

/// Whether `addr` (`HOST:PORT`) names this machine: an IP loopback address or `localhost`.
pub fn is_loopback_addr(addr: &str) -> bool {
    let host = addr.rsplit_once(':').map_or(addr, |(host, _)| host).trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost") || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// The session token for the app at `addr`. The token file in the local configuration folder is the
/// token of an app on this machine, so it is read only for a loopback `addr`; any other host gets
/// the token from `env` (the value of [`TOKEN_ENV`]) or none, and the local file is never sent to it.
pub fn attach_token_from(addr: &str, env: Option<&str>, config_dir: Option<&Path>, port: u16) -> Result<String, String> {
    if is_loopback_addr(addr) {
        return control_token_from(env, config_dir, port);
    }
    env.map(str::trim).filter(|t| !t.is_empty()).map(str::to_string).ok_or_else(|| {
        format!("no control token for {addr}: the control-token-{port} file of this machine belongs to a local app, never to another host; set {TOKEN_ENV} to the session token of the app at {addr}")
    })
}

/// [`attach_token_from`] with the real environment and the app's real configuration folder.
pub fn attach_token(addr: &str, port: u16) -> Result<String, String> {
    let env = std::env::var(TOKEN_ENV).ok();
    attach_token_from(addr, env.as_deref(), figcraft_engine::control_token::config_dir().as_deref(), port)
}

/// A running FigCraft app, reached through its loopback control port (`uigen --control PORT`),
/// which answers only a connection whose first line is `{"auth": "<session token>"}`.
pub struct Attached {
    addr: String,
    token: String,
    conn: Option<(BufReader<TcpStream>, TcpStream)>,
    next_id: u64,
}

impl Attached {
    /// Connect to `addr` (e.g. `127.0.0.1:7979`) with the app's session `token`, failing fast when
    /// nothing is listening. The app acknowledges a good token and refuses a wrong one, so the
    /// error is reported here rather than on the first call.
    pub fn connect(addr: &str, token: &str) -> std::io::Result<Self> {
        let mut r = Self { addr: addr.to_string(), token: token.to_string(), conn: None, next_id: 1 };
        r.reconnect()?;
        Ok(r)
    }

    /// Connect to `127.0.0.1:port`.
    pub fn port(port: u16, token: &str) -> std::io::Result<Self> {
        Self::connect(&format!("127.0.0.1:{port}"), token)
    }

    pub fn addr(&self) -> &str {
        &self.addr
    }

    fn reconnect(&mut self) -> std::io::Result<()> {
        self.conn = None;
        let mut last = std::io::Error::new(std::io::ErrorKind::NotFound, format!("cannot resolve {}", self.addr));
        for sa in self.addr.to_socket_addrs()? {
            match TcpStream::connect_timeout(&sa, Duration::from_millis(800)) {
                Ok(s) => {
                    s.set_nodelay(true).ok();
                    self.conn = Some(self.authenticate(s)?);
                    return Ok(());
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// Send the token as the first line and read the app's one-line answer: `ok` when it accepts
    /// the token, an error (then it closes) when it doesn't or has no connection left. A closed
    /// connection with no line is reported as such.
    fn authenticate(&self, writer: TcpStream) -> std::io::Result<(BufReader<TcpStream>, TcpStream)> {
        let mut writer = writer;
        let mut reader = BufReader::new(writer.try_clone()?);
        writeln!(writer, "{}", json!({"auth": self.token}))?;
        writer.flush()?;
        reader.get_ref().set_read_timeout(Some(AUTH_ANSWER_TIMEOUT))?;
        let mut answer = String::new();
        match reader.read_line(&mut answer) {
            Ok(0) => return Err(closed_after_token()),
            Ok(_) => check_acknowledgement(&answer)?,
            Err(e) if matches!(e.kind(), std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted) => {
                return Err(closed_after_token());
            }
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("the app did not answer the token within {} s", AUTH_ANSWER_TIMEOUT.as_secs()),
                ));
            }
            Err(e) => return Err(e),
        }
        // The app answers a request within 60 s (its own timeout); leave headroom.
        reader.get_ref().set_read_timeout(Some(Duration::from_secs(90)))?;
        Ok((reader, writer))
    }

    /// The app already closed this connection (it drops ones idle past its socket timeout), or
    /// left bytes on it nobody asked for. Checked before a request is written, when replacing the
    /// connection can't repeat anything.
    fn stale(&mut self) -> bool {
        let Some((reader, writer)) = self.conn.as_mut() else { return false };
        if !reader.buffer().is_empty() || writer.set_nonblocking(true).is_err() {
            return true;
        }
        // Nothing to read on a healthy idle connection: the peek would block.
        let idle = matches!(writer.peek(&mut [0]), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock);
        writer.set_nonblocking(false).is_err() || !idle
    }

    fn roundtrip(&mut self, line: &str) -> std::io::Result<String> {
        let Some((reader, writer)) = self.conn.as_mut() else {
            return Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "not connected to the app"));
        };
        writer.write_all(line.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        let mut reply = String::new();
        if reader.read_line(&mut reply)? == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "control channel closed"));
        }
        Ok(reply)
    }
}

impl Backend for Attached {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let line = json!({"id": id, "method": method, "params": params}).to_string();
        if self.conn.is_none() || self.stale() {
            self.reconnect().map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    format!("UIGEN app at {} refused the session token: {e} (request was not sent; check {TOKEN_ENV} or the control-token file of the running app)", self.addr)
                } else {
                    format!("UIGEN app at {} is not reachable: {e} (request was not sent; {START_HINT})", self.addr)
                }
            })?;
        }
        // A lost reply does not mean the app failed to apply the request.
        let reply = self.roundtrip(&line).map_err(|e| {
            self.conn = None;
            format!(
                "UIGEN control call `{method}` at {} failed: {e}; the operation may have completed. Inspect the document before retrying.",
                self.addr
            )
        })?;
        let v: Value = serde_json::from_str(reply.trim()).map_err(|e| format!("bad reply from the app: {e}"))?;
        if v.get("ok").and_then(Value::as_bool) == Some(true) {
            Ok(v.get("result").cloned().unwrap_or(Value::Null))
        } else {
            Err(v.get("error").and_then(Value::as_str).unwrap_or("unknown error").to_string())
        }
    }

    fn has_ui(&self) -> bool {
        true
    }

    fn describe(&self) -> String {
        format!("attached to the UIGEN app at {}", self.addr)
    }
}
