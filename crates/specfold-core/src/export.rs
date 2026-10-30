//! Exporting the corpus: the extended plugin payload (bodies and resolved links) and the
//! built-in `markdown` exporter that writes a tree for Obsidian, MkDocs, Docusaurus or a GitHub
//! wiki.

pub mod links;
pub mod markdown;
pub mod payload;

/// The exporter name specfold keeps for itself; a configured plugin may not use it.
pub const BUILT_IN: &str = "markdown";
