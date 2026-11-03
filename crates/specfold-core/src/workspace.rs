//! The repository around the documents: its owners, what it publishes and its version control.

pub mod exposure;
pub mod owners;
pub mod projects;
pub mod remotes;
pub mod vcs;

pub use owners::{Owner, Owners, escape_glob, literal_prefix};
pub use projects::{walk, walk_across};
