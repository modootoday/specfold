//! S rules: every document is where its type says, and its frontmatter has the right shape; with
//! the sections and values a type requires (S7, S8), ids and aliases that name one document
//! each (I1-I3), the owner it lives with (P1), and retired layouts kept out (Z1).

mod dates;
mod forbid;
mod frontmatter;
mod ids;
mod location;
mod owner;
mod required;
mod sections;
mod status;
mod values;

pub use dates::Dates;
pub use forbid::Forbidden;
pub use frontmatter::{Frontmatter, unknown_key};
pub use ids::{AliasTableFrozen, AliasesClean, UniqueIds};
pub use location::{IdMatchesPath, Placement};
pub use owner::OwnerMatches;
pub use required::RequiredByStatus;
pub use sections::RequiredSections;
pub use status::StatusAllowed;
pub use values::RequiredValues;
