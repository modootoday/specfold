//! The repository around the documents: its owners, where a document is placed, the projects
//! inside it and in other repositories, what it publishes and its version control.

pub mod exposure;
pub mod owners;
pub mod placement;
pub mod projects;
pub mod remotes;
pub mod vcs;
pub mod visibility;

pub use owners::{Owner, Owners, escape_glob, literal_prefix};
pub use projects::{walk, walk_across, walk_untracked};
