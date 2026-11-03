//! `spec graph`: the whole graph as a tree, Mermaid or DOT; impact, chain and governs.

pub mod boxes;
mod chain;
mod dot;
mod mermaid;
mod queries;
mod run;
mod tree;

pub use run::run;
