use figcraft_doc::{DocError, NodeId};
use thiserror::Error;

use crate::{
    MAX_ANIMATION_PROPERTIES, MAX_ANIMATIONS, MAX_DEPTH, MAX_JSON_BYTES, MAX_MARKUP_BYTES, MAX_NODES, MAX_VAR_NAME_CHARS, MAX_VARS, SUPPORTED_VERSION,
};

const MAX_JSON_MESSAGE_CHARS: usize = 200;

pub(crate) fn capped(text: &str, limit: usize) -> String {
    let mut chars = text.chars();
    let head: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() { format!("{head}…") } else { head }
}

#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("snapshot is {size} bytes, over the {MAX_JSON_BYTES} bytes limit")]
    TooLarge { size: usize },
    #[error("invalid JSON: {0}")]
    Json(String),
    #[error("unsupported snapshot version {found}, expected version {SUPPORTED_VERSION}")]
    UnsupportedVersion { found: u32 },
    #[error("snapshot has more than {MAX_NODES} nodes")]
    TooManyNodes,
    #[error("snapshot tree is deeper than {MAX_DEPTH} levels (depth)")]
    TooDeep,
    #[error("svg markup of {node} is {size} bytes, over the {MAX_MARKUP_BYTES} bytes markup limit")]
    MarkupTooLarge { node: String, size: usize },
    #[error("invalid vars entry on {node}: the name {name:?} must be -- then 1 to {MAX_VAR_NAME_CHARS} letters, digits, - or _")]
    InvalidVar { node: String, name: String },
    #[error("{node} has {count} vars entries, over the {MAX_VARS} limit")]
    TooManyVars { node: String, count: usize },
    #[error("{node} has {count} animations, over the {MAX_ANIMATIONS} limit")]
    TooManyAnimations { node: String, count: usize },
    #[error("{node} has an animation with {count} properties, over the {MAX_ANIMATION_PROPERTIES} limit")]
    TooManyProperties { node: String, count: usize },
    #[error("{field} of {node} is {size} characters, over the {limit} characters limit")]
    FieldTooLong { node: String, field: &'static str, size: usize, limit: usize },
    #[error("invalid box on {node}: x and y must be finite, width and height finite and not negative")]
    InvalidBox { node: String },
}

impl From<serde_json::Error> for SnapshotError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(capped(&error.to_string(), MAX_JSON_MESSAGE_CHARS))
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum ImportError {
    #[error("parent layer {0} does not exist")]
    ParentNotFound(NodeId),
    #[error("nothing to import: {0}")]
    NothingToImport(&'static str),
    #[error("could not add a layer: {0}")]
    Insert(#[from] DocError),
}
