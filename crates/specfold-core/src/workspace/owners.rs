//! Owners of documents.

mod discover;
mod patterns;
mod reader;
mod types;

pub use patterns::{PathMatch, matcher};
pub use types::{Owner, Owners};
