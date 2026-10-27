//! Checks over the corpus: rules, their scope, and the findings they report.

pub mod baseline;
pub mod context;
pub mod finding;
pub mod rule;
pub mod rules;
pub mod runner;
pub mod scope;

pub use context::Options;
pub use finding::{Finding, Fix, Severity};
pub use runner::{Report, Selection, run};
pub use scope::Range;
