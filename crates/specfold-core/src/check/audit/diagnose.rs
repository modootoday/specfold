//! The deterministic part of an audit: the same corpus and history give the same items.

mod coverage;
mod debt;
mod drift;
mod exceptions;
mod finished;
mod gap;
mod open_items;
mod orphan;
mod run;
mod stale;
mod stalled;

use run::archive;
pub use run::run;
