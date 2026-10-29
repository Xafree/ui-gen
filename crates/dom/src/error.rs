use thiserror::Error;

use crate::{MAX_DEPTH, MAX_JSON_BYTES, MAX_MARKUP_BYTES, MAX_NODES, SUPPORTED_VERSION};

#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("snapshot is {size} bytes, over the {MAX_JSON_BYTES} bytes limit")]
    TooLarge { size: usize },
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported snapshot version {found}, expected version {SUPPORTED_VERSION}")]
    UnsupportedVersion { found: u32 },
    #[error("snapshot has more than {MAX_NODES} nodes")]
    TooManyNodes,
    #[error("snapshot tree is deeper than {MAX_DEPTH} levels (depth)")]
    TooDeep,
    #[error("svg markup of {node} is {size} bytes, over the {MAX_MARKUP_BYTES} bytes markup limit")]
    MarkupTooLarge { node: String, size: usize },
    #[error("invalid box on {node}: x and y must be finite, width and height finite and not negative")]
    InvalidBox { node: String },
}
