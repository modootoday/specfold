//! What a text trips: denied terms, denied patterns and the built-in secret formats, with the
//! spans where a match does not count (code in documentation, regex literals in scripts) and a
//! cheap first pass that skips files nothing can match.

mod code_spans;
mod jslex;
pub(super) mod pattern;
pub(super) mod prefilter;
pub mod secrets;
pub mod terms;
