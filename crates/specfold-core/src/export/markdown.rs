//! The built-in `markdown` exporter: a plain Markdown tree laid out for one target (Obsidian,
//! MkDocs, Docusaurus or a GitHub wiki). `[[references]]` become that target's links; a
//! reference that leads nowhere in the tree (unresolved, in a remote, or to a document left out)
//! is written as plain text and reported. The output directory must be empty or a previous
//! export, recognised by its manifest, whose files alone are replaced.

mod out;
mod tree;
mod types;

pub use out::check_out;
pub use tree::{build, github_slug, write};
pub use types::{MANIFEST, Options, Target, Tree, Unresolved};
