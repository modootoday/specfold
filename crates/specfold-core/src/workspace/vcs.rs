//! The little git specfold needs, through the git command line. Every function degrades to
//! "unknown" when git is missing or the directory is not a repository.

mod changes;
mod first_add;
mod history;
mod passing;
mod refs;
mod repo;
mod submodules;
mod tracked;

pub use changes::{added_lines, changed_in_range, changed_since, gone, show, show_staged, staged};
pub use first_add::first_commit_stamp;
pub use history::{CommitText, Revision, commits, commits_since, history, last_change};
pub use passing::{Passing, passing, range_end};
pub use refs::{commit_diff, commit_log, head, is_ancestor_of_head, parent_or_empty, resolve, upstream};
pub use repo::{git_root, is_repo};
pub use submodules::{Expansion, Side, expand as expand_submodules, gitlinks, submodule_changes};
pub use tracked::{TrackedSet, blob_id, committed, ignored, past_blobs, tracked};
