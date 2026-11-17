//! L rules: every reference resolves to one document, written as an id; plain Markdown links
//! lead somewhere (L8, L9), citations from outside the corpus resolve (L6), and pinned remote
//! documents are watched (L7); in a mounted repository, governed paths start with the mount (L10).

mod anchors;
mod cites;
mod cites_scan;
mod governs;
mod md_links;
mod mounted;
mod path_cites;
mod pins;
mod relations;
mod resolve;
mod targets;

pub use anchors::AnchorsUnique;
pub use cites::CitesOutside;
pub use governs::GovernsExists;
pub use md_links::{ImageAlt, LocalLinks};
pub use mounted::GovernsMounted;
pub use path_cites::NoPathCites;
pub use pins::PinsHold;
pub use relations::RelationsResolve;
pub use resolve::LinksResolve;
