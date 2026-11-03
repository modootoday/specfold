//! Reading and writing the Markdown files documents are made of.

mod blocks;
pub mod body;
pub mod counts;
pub mod frontmatter;
pub mod link_dest;
pub mod sections;
pub mod tables;
pub mod tasks;

pub use blocks::{Fence, atx};
