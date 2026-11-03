//! Path arithmetic every context shares: lexical clean-up, relative links, containment, and the
//! real form of a path that may not exist yet. Nothing here reads the working directory.

mod lexical;
mod real;
mod relative;
mod under;

pub use lexical::{join, normalize, normalize_keeping_up};
pub use real::{real_with_lexical_tail, real_with_missing_tail};
pub use relative::{rel_of, relative_from_dir, relative_from_file, slash};
pub use under::{is_below, is_under};
