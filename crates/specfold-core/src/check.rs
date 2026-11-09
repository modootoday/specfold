//! Checks over the corpus: rules, their scope and the findings they report; guards over any
//! file and commit message; and audits of what passes yet needs attention.

pub mod audit;
pub mod baseline;
pub mod context;
pub mod finding;
pub mod guards;
pub mod limits;
pub mod rule;
pub mod rules;
pub mod runner;
pub mod scope;
pub mod severity;

pub use context::Options;
pub use finding::{Finding, Fix, Severity};
pub use runner::{Report, Selection, run};
pub use scope::Range;
