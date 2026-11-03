//! Builds the import plan: one row per source for review, and the file changes that apply it.

mod aliases;
mod build;
mod files;
mod links;
mod names;
mod notes;
mod row;
mod types;

pub use aliases::alias_table;
pub use build::build;
pub use types::{ImportPlan, Row};

#[cfg(test)]
mod tests;
