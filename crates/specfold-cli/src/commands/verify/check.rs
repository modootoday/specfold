//! `spec check`: the run, the baseline it holds errors against, the CI formats, and what a run
//! covers (files, rules, commit range).

mod baseline;
mod ci;
mod run;
mod scope;

pub use run::run;
pub(crate) use scope::{check_rule_names, documents_named};
