//! The corpus: every document under every owner's spec directory.

mod aliases;
pub mod assemble;
mod cache;
pub mod location;
mod reader;
mod roots;
pub mod scan;
mod types;
pub mod walk;

pub use types::{AliasTable, Corpus, LoadOptions, SpecRoot, Stray};
