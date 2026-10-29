use std::path::PathBuf;
use std::time::Duration;

pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeouts {
    pub hello: Duration,
    pub install: Duration,
    pub inspect: Duration,
    pub render: Duration,
    pub shutdown: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            hello: Duration::from_secs(15),
            install: Duration::from_secs(600),
            inspect: Duration::from_secs(60),
            render: Duration::from_secs(30),
            shutdown: Duration::from_secs(5),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub node: PathBuf,
    pub script: PathBuf,
    pub timeouts: Timeouts,
    pub max_response_bytes: usize,
}
