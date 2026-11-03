//! The corpus: every document under every owner's spec directory, the graph derived from it,
//! searches and counts over it, and the translations among its documents.

pub mod census;
pub mod graph;
mod load;
pub mod location;
pub mod overlay;
mod roots;
pub mod search;
pub mod trace;
pub mod translation;
mod types;

pub use load::{assemble, scan, walk};

pub use types::{AliasTable, Corpus, LoadOptions, SpecRoot, Stray};
