//! DOM snapshots: the validated model and parser of a rendered component's tree, boxes and
//! computed styles (format version 1, written by the capture service), and their translation
//! into editable layers.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod error;
mod import;
mod model;
mod parse;

pub use error::{ImportError, SnapshotError};
pub use import::{Imported, import_snapshot};
pub use model::{DomBox, DomNode, Element, Snapshot, Source, SvgNode, TextRun, Viewport};
pub use parse::parse_snapshot;

pub const SUPPORTED_VERSION: u32 = 1;
pub const MAX_JSON_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_NODES: usize = 5000;
pub const MAX_DEPTH: usize = 64;
pub const MAX_MARKUP_BYTES: usize = 1024 * 1024;

#[cfg(test)]
mod import_tests;
#[cfg(test)]
mod snapshot_tests;
