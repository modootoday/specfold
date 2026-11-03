//! What a guard reads and how: the range (index, pushed commits, whole tree), cached per-file
//! results, the scan of one compiled guard, and blobs about to leave the repository.

pub(super) mod blobs;
mod cache;
pub mod range;
mod run;

pub(super) use run::{Scanned, run};
