//! Owners of documents.

mod discover;
mod patterns;
mod reader;
mod types;

pub use patterns::{PathMatch, escape_glob, literal_prefix, matcher};
pub use types::{Owner, Owners};
