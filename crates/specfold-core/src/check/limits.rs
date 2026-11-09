//! Limits a project keeps outside its documents: size budgets, generated files and their
//! sources, read from spec.toml or from the file that owns them.

pub mod budgets;
pub mod generated;
pub mod source;
