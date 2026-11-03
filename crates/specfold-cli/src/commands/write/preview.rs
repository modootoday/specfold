//! Shows a plan, and applies it only when asked.

mod finish;
mod hunks;
mod trash;

pub use finish::{bound, files, finish, finish_bounded};
pub use trash::{STAMP, TRASH, fresh_bin, keep_untracked, undo};
