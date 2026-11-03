//! Translated documents: a translation names its source with `rel.translation_of` and records
//! the source it was made from as `source_rev`, a hash of the source body (the main file and the
//! bundle's chapters). No git history is needed to see that a translation is behind: the hash is
//! compared with the source as it is now. Where git has the recorded revision, the sections and
//! paragraphs that changed since are named as well.

mod layout;
mod report;
mod rev;
mod types;

pub use layout::{inferred, is_translation, lang_of, lang_problem, source_of, source_ref};
pub use report::{report, state};
pub use rev::{
    changes_since, describe, parts_at_rev, parts_of, rev_of, rev_of_parts, rev_of_text, rev_problem, section_changes,
};
pub use types::{Inferred, KEY, Part, RELATION, Report, SectionChange, State, Translation};

#[cfg(test)]
mod tests;
