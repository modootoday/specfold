//! External programs: plugins that add rules, importers, exporters, audits and event handlers
//! over a JSON protocol, and the shell commands documents name. Only programs named in
//! spec.toml or a document ever run.

pub mod payload;
pub mod process;
pub mod protocol;
pub mod registry;
pub mod rules;
pub mod runner;

pub use protocol::{Description, PROTOCOL};
pub use registry::{Loaded, load};

#[cfg(all(test, unix))]
mod tests;
