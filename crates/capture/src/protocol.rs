use std::time::Duration;

use figcraft_engine::capture::{CaptureError, CaptureErrorKind, CaptureJob};
use serde_json::{Map, Value, json};

use crate::config::Timeouts;
use crate::error;

pub(crate) const PROTOCOL_VERSION: u64 = 1;
pub(crate) const SHUTDOWN_METHOD: &str = "shutdown";
const SNIPPET_CHARS: usize = 120;

pub(crate) struct Request {
    pub method: &'static str,
    pub params: Value,
    pub timeout: Duration,
}

pub(crate) fn request_of(job: CaptureJob, timeouts: &Timeouts) -> Result<Request, CaptureError> {
    Ok(match job {
        CaptureJob::Install { root, package, version } => {
            Request { method: "install", params: json!({"dir": root, "package": package, "version": version}), timeout: timeouts.install }
        }
        CaptureJob::InstallMany { root, packages } => {
            let packages: Vec<Value> = packages.iter().map(|spec| json!({"name": spec.name, "version": spec.version})).collect();
            Request { method: "install", params: json!({"dir": root, "packages": packages}), timeout: timeouts.install }
        }
        CaptureJob::Uninstall { root, package } => {
            Request { method: "uninstall", params: json!({"dir": root, "package": package}), timeout: timeouts.install }
        }
        CaptureJob::Inspect { root, package } => {
            Request { method: "inspect", params: json!({"dir": root, "package": package}), timeout: timeouts.inspect }
        }
        CaptureJob::Render { root, package, component, props, sample_text, state, theme } => {
            let mut params = json!({"dir": root, "package": package, "component": component, "props": props, "state": state, "theme": theme});
            if let Some(text) = sample_text {
                params["sampleText"] = json!(text);
            }
            Request { method: "render", params, timeout: timeouts.render }
        }
        CaptureJob::Playground { root, method, params } => playground_request(&root, &method, params, timeouts)?,
    })
}

fn playground_request(root: &str, method: &str, params: Value, timeouts: &Timeouts) -> Result<Request, CaptureError> {
    let (method, timeout) = match method {
        "playground.open" => ("playground.open", timeouts.playground_open),
        "playground.frame" => ("playground.frame", timeouts.playground_frame),
        "playground.setTemplate" => ("playground.setTemplate", timeouts.playground),
        "playground.apply" => ("playground.apply", timeouts.playground),
        "playground.resize" => ("playground.resize", timeouts.playground),
        "playground.input" => ("playground.input", timeouts.playground),
        "playground.close" => ("playground.close", timeouts.playground),
        other => return Err(error(CaptureErrorKind::InvalidRequest, format!("`{other}` is not a playground method"))),
    };
    let mut fields: Map<String, Value> = match params {
        Value::Object(fields) => fields,
        Value::Null => Map::new(),
        _ => return Err(error(CaptureErrorKind::InvalidRequest, format!("`{method}` takes an object of parameters"))),
    };
    fields.insert("dir".to_string(), json!(root));
    Ok(Request { method, params: Value::Object(fields), timeout })
}

pub(crate) fn encode(id: u64, method: &str, params: &Value) -> Vec<u8> {
    let mut line = json!({"id": id, "method": method, "params": params}).to_string().into_bytes();
    line.push(b'\n');
    line
}

pub(crate) fn check_hello(line: &[u8]) -> Result<(), CaptureError> {
    let hello = parse(line)?;
    match hello.get("protocol").and_then(Value::as_u64) {
        Some(PROTOCOL_VERSION) => Ok(()),
        Some(other) => Err(error(CaptureErrorKind::Protocol, format!("capture service speaks protocol {other}, UIGEN expects {PROTOCOL_VERSION}"))),
        None => Err(error(CaptureErrorKind::Protocol, "the capture service greeting has no protocol number")),
    }
}

pub(crate) fn decode_response(line: &[u8], expected_id: u64) -> Result<Value, CaptureError> {
    let response = parse(line)?;
    let id = response.get("id").and_then(Value::as_u64);
    if id != Some(expected_id) {
        return Err(error(CaptureErrorKind::Protocol, format!("the capture service answered request {id:?} while {expected_id} was pending")));
    }
    match response.get("ok").and_then(Value::as_bool) {
        Some(true) => response.get("result").cloned().ok_or_else(|| error(CaptureErrorKind::Protocol, "the capture service answer has no result")),
        Some(false) => Err(service_error(response.get("error"))),
        None => Err(error(CaptureErrorKind::Protocol, "the capture service answer has no `ok` flag")),
    }
}

fn parse(line: &[u8]) -> Result<Value, CaptureError> {
    serde_json::from_slice(line).map_err(|cause| {
        let shown: String = String::from_utf8_lossy(line).chars().take(SNIPPET_CHARS).collect();
        error(CaptureErrorKind::Protocol, format!("the capture service sent a line that is not JSON ({cause}): {shown}"))
    })
}

fn service_error(details: Option<&Value>) -> CaptureError {
    let name = details.and_then(|details| details.get("kind")).and_then(Value::as_str).unwrap_or_default();
    let message = details.and_then(|details| details.get("message")).and_then(Value::as_str).unwrap_or("the capture service reported an error");
    match kind_of(name) {
        Some(kind) => error(kind, message),
        None => error(CaptureErrorKind::Protocol, format!("the capture service reported an unknown error kind `{name}`: {message}")),
    }
}

fn kind_of(name: &str) -> Option<CaptureErrorKind> {
    Some(match name {
        "invalidRequest" => CaptureErrorKind::InvalidRequest,
        "install" => CaptureErrorKind::Install,
        "uninstall" => CaptureErrorKind::Uninstall,
        "notInstalled" => CaptureErrorKind::NotInstalled,
        "componentNotFound" => CaptureErrorKind::ComponentNotFound,
        "frameworkUnsupported" => CaptureErrorKind::FrameworkUnsupported,
        "render" => CaptureErrorKind::Render,
        "tooLarge" => CaptureErrorKind::TooLarge,
        "internal" => CaptureErrorKind::Internal,
        _ => return None,
    })
}
