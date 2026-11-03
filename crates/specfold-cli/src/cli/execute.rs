//! Runs one command line to an output, without printing. The binary and the MCP server share it.

mod dir_rewrite;
mod run;
mod usage_errors;

pub use run::{Executed, execute, utf8_args};
