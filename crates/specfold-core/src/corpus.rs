//! The corpus: every document under every owner's spec directory, and counts over it.

pub mod census;
mod load;
pub mod location;
pub mod overlay;
mod roots;
pub mod trace;
mod types;

pub use load::{assemble, scan, walk};

pub use types::{AliasTable, Corpus, LoadOptions, SpecRoot, Stray};
