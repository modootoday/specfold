//! What specfold suggests when a name does not match: the closest known name, by edit distance.

mod names;

pub use names::{distance, nearest, nearest_or_prefixed};
