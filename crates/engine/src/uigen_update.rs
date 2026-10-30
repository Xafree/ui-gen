//! Taking a new capture of an imported component into its copy in the library: what changed between
//! the capture the copy comes from and the current one (`source_diff`), and the application of those
//! differences, a layer or a field at a time.

mod apply;
mod diff;

pub(crate) use apply::{Outcome, apply};
pub use diff::{Difference, DifferenceKind, DifferenceState, Located, MAX_DIFFERENCES, SourceDiff, Summary, source_diff};
