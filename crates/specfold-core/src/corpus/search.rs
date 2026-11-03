//! Searching the corpus by words, type, owner, status and tags.

mod find;
mod types;

pub use find::find;
pub use types::{Hit, Query};
