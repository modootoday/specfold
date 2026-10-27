//! YAML frontmatter: split, parse, render.

pub mod edit;
mod render;
mod split;
mod yaml;

pub use render::render;
pub use split::{Split, split};
pub use yaml::{Parsed, parse};
