//! Importing a component: the capture jobs of `uigen.importComponent` (one `inspect`, then a `render`
//! per variant and theme) and, once every render is back, the component set written in one undo step.

mod assemble;
mod catalog;
mod motion;
mod pending;
mod plan;
mod reference;
mod staging;
mod tokens;

pub use crate::uigen_compare::RefText;
pub(crate) use catalog::{COMMAND as LIST_COMMAND, Cache, receive};
pub use catalog::{Inspected, InspectedComponent, InspectedProp, PropKind, render_count};
pub(crate) use pending::{COMMAND, Destination, PendingImport, Request, advance, jobs_of, resume, start};
pub use plan::MAX_RENDERS;
pub(crate) use plan::{State, Theme, usable_value};
pub use reference::{OPTIONS_KEY, REFERENCE_KEY, Reference, ThemeReference, reference_of};
#[cfg(test)]
pub(crate) use staging::IMPORT_PAGE_KEY;
pub(crate) use staging::SOURCE_KEY;
pub(crate) use staging::import_page_name;
pub use staging::{ImportSection, ImportedSet, is_import_page, sections};
