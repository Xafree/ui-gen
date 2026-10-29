//! UIGEN capture service driver: runs the Node sidecar (`sidecar/service.mjs`) and speaks its
//! JSON-lines protocol, one job at a time on a dedicated thread, behind the engine's
//! `CaptureBackend` port. In the browser build nothing is launched and every job is unavailable.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use figcraft_engine::capture::{CaptureError, CaptureErrorKind};

mod config;
mod locate;
#[cfg(not(target_arch = "wasm32"))]
mod process;
#[cfg(not(target_arch = "wasm32"))]
mod protocol;
#[cfg_attr(target_arch = "wasm32", path = "unavailable.rs")]
mod service;

pub use config::{CaptureConfig, Timeouts};
pub use locate::locate;
pub use service::CaptureService;

fn error(kind: CaptureErrorKind, message: impl Into<String>) -> CaptureError {
    CaptureError { kind, message: message.into() }
}

#[cfg(test)]
mod locate_tests;
#[cfg(test)]
mod real_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod review_tests;
#[cfg(test)]
mod service_tests;
