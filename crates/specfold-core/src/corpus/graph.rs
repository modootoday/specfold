//! The graph derived from documents. It is computed on every run and never stored.

mod build;
pub mod query;
mod types;

pub use types::{DOC_RELATIONS, Edge, Graph, MENTIONS, Resolution};

#[cfg(test)]
mod tests;
