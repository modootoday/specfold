//! The repository around the documents: its owners, what it publishes and its version control.

pub mod exposure;
pub mod owners;
pub mod vcs;

pub use owners::{Owner, Owners, escape_glob, literal_prefix};
