//! The asset corpus: agent skills, rules, subagents and instruction files declared under
//! `[[assets]]` in spec.toml. Assets have no ids and are only read; checks report on them, and
//! only `spec sync assets` writes them, into roots that declare `copy_of`.

mod asset;
mod load;
pub mod sync;
mod template;

pub use asset::Asset;
pub use load::load;
pub use template::template;

#[cfg(test)]
mod tests;
