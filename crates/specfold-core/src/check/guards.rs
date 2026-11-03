//! Guards: named checks over any file in the repository and over commit messages, such as the
//! gate that keeps internal names out of a public package.

mod compile;
mod lex;
mod prove;
mod run;
mod scan;

pub use lex::{secrets, terms};
pub use run::{run, run_net, write_baselines};
pub use scan::blobs::{builtin_secrets, reads_content, scan_blobs};
pub use scan::range::{self, GuardRange};

#[cfg(test)]
mod tests;
