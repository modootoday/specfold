//! Placement: which owner a document belongs to and the path it lives at.

pub mod owner;
pub mod paths;

pub use owner::{Decision, decide, literal_base};
