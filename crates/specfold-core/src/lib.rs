//! specfold-core: typed Markdown documents, their owners, the graph between them and the checks
//! over it. The CLI crate is a thin layer over this library.

// Library code only: tests may panic (AGENTS.md).
#![deny(clippy::panic)]
// CLI-20: no unwrap outside tests (clippy.toml allow-unwrap-in-tests).
#![deny(clippy::unwrap_used)]
// Output belongs to the CLI; core reports through return values (stderr tracing aside).
#![deny(clippy::print_stdout)]

pub mod assets;
pub mod authoring;
pub mod check;
pub mod config;
pub mod corpus;
pub mod export;
pub mod model;
pub mod paths;
pub mod plugins;
pub mod suggest;
pub mod workspace;

pub use config::Config;
pub use corpus::graph::Graph;
pub use corpus::{Corpus, LoadOptions};
pub use corpus::{census, trace};
pub use model::{Doc, DocId};
