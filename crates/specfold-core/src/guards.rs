//! Guards: named checks over any file in the repository and over commit messages, such as the
//! gate that keeps internal names out of a public package.

mod blobs;
mod cache;
mod code_spans;
mod compile;
mod jslex;
mod pattern;
mod prefilter;
mod prove;
pub mod range;
mod run;
mod scan;
pub mod secrets;
pub mod terms;

pub use blobs::{builtin_secrets, reads_content, scan_blobs};
pub use range::GuardRange;
pub use run::{run, run_net, write_baselines};

#[cfg(test)]
mod tests;
