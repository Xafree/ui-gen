use std::env;
use std::path::{Path, PathBuf};

use figcraft_engine::capture::{CaptureError, CaptureErrorKind};

use crate::config::{CaptureConfig, DEFAULT_MAX_RESPONSE_BYTES, Timeouts};

const NODE_VARIABLE: &str = "UIGEN_NODE";
const SIDECAR_DIR_VARIABLE: &str = "UIGEN_SIDECAR_DIR";
const SCRIPT_NAME: &str = "service.mjs";
const SIDECAR_DIR_NAME: &str = "sidecar";

pub fn locate() -> Result<CaptureConfig, CaptureError> {
    Ok(CaptureConfig {
        node: locate_node()?,
        script: locate_script()?,
        timeouts: Timeouts::default(),
        max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
    })
}

fn start_error(message: String) -> CaptureError {
    CaptureError { kind: CaptureErrorKind::Start, message }
}

fn variable(name: &str) -> Option<PathBuf> {
    env::var_os(name).filter(|value| !value.is_empty()).map(PathBuf::from)
}

fn locate_node() -> Result<PathBuf, CaptureError> {
    if let Some(node) = variable(NODE_VARIABLE) {
        return Ok(node);
    }
    let directories: Vec<PathBuf> = env::var_os("PATH").map(|path| env::split_paths(&path).collect()).unwrap_or_default();
    let executable = format!("node{}", env::consts::EXE_SUFFIX);
    if let Some(found) = directories.iter().map(|directory| directory.join(&executable)).find(|candidate| candidate.is_file()) {
        return Ok(found);
    }
    let searched = directories.iter().map(|directory| directory.display().to_string()).collect::<Vec<_>>().join(", ");
    Err(start_error(format!("node was not found in PATH (searched: {searched}); install Node.js or set {NODE_VARIABLE} to the node executable")))
}

fn locate_script() -> Result<PathBuf, CaptureError> {
    if let Some(directory) = variable(SIDECAR_DIR_VARIABLE) {
        let script = directory.join(SCRIPT_NAME);
        return if script.is_file() {
            Ok(script)
        } else {
            Err(start_error(format!("{SIDECAR_DIR_VARIABLE} is set but {} does not exist", script.display())))
        };
    }
    let candidates = script_candidates();
    if let Some(found) = candidates.iter().find(|candidate| candidate.is_file()) {
        return Ok(found.clone());
    }
    let tried = candidates.iter().map(|candidate| candidate.display().to_string()).collect::<Vec<_>>().join(", ");
    Err(start_error(format!(
        "the capture service script was not found (tried: {tried}); set {SIDECAR_DIR_VARIABLE} to the folder holding {SCRIPT_NAME}"
    )))
}

fn script_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(directory) = env::current_exe().ok().as_deref().and_then(Path::parent) {
        candidates.push(directory.join(SIDECAR_DIR_NAME).join(SCRIPT_NAME));
    }
    #[cfg(debug_assertions)]
    candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(SIDECAR_DIR_NAME).join(SCRIPT_NAME));
    candidates
}
