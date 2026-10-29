//! Importing a component: the capture jobs of `uigen.importComponent` (one `inspect`, then a `render`
//! per variant and theme) and, once every render is back, the component set written in one undo step.

mod assemble;
mod pending;
mod plan;

pub(crate) use pending::{COMMAND, Destination, PendingImport, Request, advance, start};
pub(crate) use plan::{MAX_RENDERS, State, Theme, usable_value};
