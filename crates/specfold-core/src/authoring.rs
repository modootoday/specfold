//! Writing documents: create, change, move and fix. Every operation returns a plan; nothing
//! touches the disk until the plan is applied.

pub mod edit;
pub mod fix;
pub mod forbid;
pub mod mv;
pub mod new;
pub mod plan;
pub mod refs;
pub mod set;

pub use plan::{Change, Plan};
