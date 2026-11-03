//! Project boundaries: which directories inside the repository belong to another project (a
//! nested project, a submodule of its own, a remote checked out here), and the one walk that
//! leaves nested projects out. Every root walk and every submodule list comes from here.

mod inner;
mod nested;
mod submodules;
mod walk;

pub use inner::{is_external, submodule_at};
pub use nested::nested_remotes;
pub use submodules::{submodule_remotes, submodules};
pub use walk::{walk, walk_across};
