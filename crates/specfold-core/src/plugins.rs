//! Plugins: external programs that add rules, importers, exporters, audits and event handlers
//! over a JSON protocol. Only programs named in spec.toml ever run.

pub mod payload;
pub mod process;
pub mod protocol;
pub mod registry;
pub mod rules;

pub use protocol::{Description, PROTOCOL};
pub use registry::{Loaded, load};

#[cfg(all(test, unix))]
mod tests;
