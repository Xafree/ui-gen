//! Importing a component: the capture jobs of `uigen.importComponent` (one `inspect`, then a `render`
//! per variant and theme) and, once every render is back, the component set written in one undo step.

mod assemble;
mod catalog;
mod motion;
mod pending;
mod plan;
mod staging;
mod tokens;

pub(crate) use catalog::{COMMAND as LIST_COMMAND, Cache, receive};
pub use catalog::{Inspected, InspectedComponent, InspectedProp, PropKind, render_count};
pub(crate) use pending::{COMMAND, Destination, PendingImport, Request, advance, resume, start};
pub use plan::MAX_RENDERS;
pub(crate) use plan::{State, Theme, usable_value};
#[cfg(test)]
pub(crate) use staging::IMPORT_PAGE_KEY;
pub use staging::{ImportSection, ImportedSet, sections};
pub(crate) use staging::{import_page_name, is_import_page};
