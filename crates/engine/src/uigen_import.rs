//! Importing a component: the capture jobs of `uigen.importComponent` (one `inspect`, then a `render`
//! per variant and theme) and, once every render is back, the component set written in one undo step.

mod assemble;
mod motion;
mod pending;
mod plan;
mod staging;
mod tokens;

pub(crate) use pending::{COMMAND, Destination, PendingImport, Request, advance, resume, start};
pub(crate) use plan::{MAX_RENDERS, State, Theme, usable_value};
pub(crate) use staging::import_page_name;
#[cfg(test)]
pub(crate) use staging::{IMPORT_PAGE_KEY, is_import_page};
