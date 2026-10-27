//! specfold-core: typed Markdown documents, their owners, the graph between them and the checks
//! over it. The CLI crate is a thin layer over this library.

pub mod audit;
pub mod authoring;
pub mod census;
pub mod check;
pub mod config;
pub mod corpus;
pub mod files;
pub mod graph;
pub mod guards;
pub mod import;
pub mod markdown;
pub mod model;
pub mod placement;
pub mod plugins;
pub mod search;
pub mod trace;
pub mod workspace;

pub use config::Config;
pub use corpus::{Corpus, LoadOptions};
pub use graph::Graph;
pub use model::{Doc, DocId};
