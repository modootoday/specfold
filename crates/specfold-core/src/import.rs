//! Importing documents from another layout through a project's import map.

pub mod body_meta;
pub mod convert;
pub mod key;
pub mod map;
pub mod owner;
pub mod plan;
pub mod presets;
pub mod sources;

pub use map::ImportMap;
pub use plan::{ImportPlan, Row, alias_table, build};

#[cfg(test)]
mod tests;
