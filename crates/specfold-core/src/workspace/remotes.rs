//! Documents in other repositories. `[remotes.<name>] path = "../shared"` lets a reference or a
//! relation name a document there as `<name>:<id>` (`shared:adr:org/20261030000000`). The other
//! repository is read from a directory already on disk and never written. A remote whose
//! directory is absent (a CI job without that checkout) is skipped, not reported; a remote that
//! is present but lacks the id is an error like any missing document.

mod load;
mod query;
mod types;

pub use load::{dir, load};
pub use query::{consumers, resolve, rev, split, status};
pub use types::{Consumer, Loaded, Remote};
