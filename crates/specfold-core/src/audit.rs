//! Audits: what passes every check yet needs attention (diagnose), how documents changed over
//! time (history), and packs for judgement a machine cannot make alone (pack).

pub mod diagnose;
pub mod history;
pub mod pack;
pub mod plugins;
pub mod types;

pub use diagnose::Filter;
pub use types::{Diagnosis, Item, Kind};
