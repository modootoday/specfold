//! S rules: every document is where its type says, and its frontmatter has the right shape.

mod dates;
mod frontmatter;
mod location;
mod required;
mod status;

pub use dates::Dates;
pub use frontmatter::{Frontmatter, unknown_key};
pub use location::{IdMatchesPath, Placement};
pub use required::RequiredByStatus;
pub use status::StatusAllowed;
