//! L rules: every reference resolves to one document, written as an id.

mod anchors;
mod governs;
mod path_cites;
mod relations;
mod resolve;
mod targets;

pub use anchors::AnchorsUnique;
pub use governs::GovernsExists;
pub use path_cites::NoPathCites;
pub use relations::RelationsResolve;
pub use resolve::LinksResolve;
