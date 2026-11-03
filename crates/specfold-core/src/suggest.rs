//! What specfold suggests: the closest known name when one does not match, and the shell
//! words of the commands it proposes.

mod names;
pub mod shell;

pub use names::{distance, nearest, nearest_or_prefixed};
