//! The little git specfold needs, through the git command line. Every function degrades to
//! "unknown" when git is missing or the directory is not a repository.

mod changes;
mod first_add;
mod history;
mod refs;
mod repo;

pub use changes::{added_lines, changed_in_range, changed_since, show, show_staged, staged};
pub use first_add::first_commit_stamp;
pub use history::{CommitText, Revision, commits, history, last_change};
pub use refs::{head, is_ancestor_of_head, upstream};
pub use repo::{git_root, is_repo};
