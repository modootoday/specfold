//! The output contract shared by every command.

pub mod envelope;
pub mod findings;
pub mod render;

pub use envelope::{Code, Output};
pub use render::{Format, Page, render};
