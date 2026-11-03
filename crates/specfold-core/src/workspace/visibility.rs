//! Submodule visibility: the manifest `[public] submodules_manifest` names, the forge that
//! measures it, and the measurement `spec sync visibility` writes.

mod forge;
mod github;
pub mod manifest;
mod measure;

pub use forge::{Forge, ForgeError};
pub use github::{GitHub, repository_of};
pub use manifest::{Entry, Manifest};
pub use measure::{Measured, measure};

/// The command that writes the manifest, named wherever it is found missing or stale.
pub const WRITE_COMMAND: &str = "spec sync visibility --write";

#[cfg(test)]
mod tests;
