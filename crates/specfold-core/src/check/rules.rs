//! The rule catalog, one module per family.

mod files;
mod lifecycle;
mod links;
mod registry;
mod schema;
pub(crate) mod support;

pub use lifecycle::{is_date, overdue_exceptions};
pub use registry::catalog;
pub use schema::unknown_key;
