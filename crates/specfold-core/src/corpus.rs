//! The corpus: every document under every owner's spec directory, and counts over it.

mod aliases;
pub mod assemble;
mod cache;
pub mod census;
pub mod location;
pub mod overlay;
mod reader;
mod roots;
pub mod scan;
pub mod trace;
mod types;
pub mod walk;

pub use types::{AliasTable, Corpus, LoadOptions, SpecRoot, Stray};
