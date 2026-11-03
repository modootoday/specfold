//! Loading the corpus: walking each spec directory, scanning its files (cached), grouping them
//! into documents, and reading the alias table.

mod aliases;
pub mod assemble;
mod cache;
mod reader;
pub mod scan;
pub mod walk;
