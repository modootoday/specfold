//! Documents in other repositories. `[remotes.<name>] path = "../shared"` lets a reference or a
//! relation name a document there as `<name>:<id>` (`shared:adr:org/20261030000000`). The other
//! repository is read from a directory already on disk and never written. A remote whose
//! directory is absent (a CI job without that checkout) is skipped, not reported; a remote that
//! is present but lacks the id is an error like any missing document. With `[owners] mount`,
//! targets missing here are looked up in the workspace that mounts this repository.

mod load;
mod parent;
mod query;
mod types;

pub use load::{dir, load, load_dir};
pub use parent::{PARENT, Parent, parent, resolve_in_parent, root_of, superseded_in_parent};
pub use query::{consumers, resolve, rev, split, status};
pub use types::{Consumer, Loaded, Remote};
