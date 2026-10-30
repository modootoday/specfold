//! The little git specfold needs, through the git command line. Every function degrades to
//! "unknown" when git is missing or the directory is not a repository.

mod changes;
mod first_add;
mod history;
mod refs;
mod repo;
mod submodules;
mod tracked;

pub use changes::{added_lines, changed_in_range, changed_since, gone, show, show_staged, staged};
pub use first_add::first_commit_stamp;
pub use history::{CommitText, Revision, commits, commits_since, history, last_change};
pub use refs::{head, is_ancestor_of_head, upstream};
pub use repo::{git_root, is_repo};
pub use submodules::{Side, submodule_changes};
pub use tracked::{committed, tracked};
