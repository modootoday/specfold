//! specfold-core: typed Markdown documents, their owners, the graph between them and the checks
//! over it. The CLI crate is a thin layer over this library.

// Library code only: tests may panic (AGENTS.md).
#![deny(clippy::panic)]
// Output belongs to the CLI; core reports through return values (stderr tracing aside).
#![deny(clippy::print_stdout)]

pub mod assets;
pub mod audit;
pub mod authoring;
pub mod check;
pub mod config;
pub mod corpus;
pub mod export;
pub mod files;
pub mod graph;
pub mod guards;
pub mod import;
pub mod markdown;
pub mod model;
pub mod placement;
pub mod plugins;
pub mod remotes;
pub mod runner;
pub mod search;
pub mod shell;
pub mod translation;
pub mod workspace;

pub use config::Config;
pub use corpus::{Corpus, LoadOptions};
pub use corpus::{census, trace};
pub use graph::Graph;
pub use model::{Doc, DocId};
