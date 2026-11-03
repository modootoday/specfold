//! Writing documents: create, change, move, fix and import them, and keep their attachments.
//! Every operation returns a plan; nothing touches the disk until the plan is applied.

pub mod edit;
pub mod files;
pub mod fix;
pub mod forbid;
pub mod import;
pub mod mv;
pub mod new;
pub mod plan;
pub mod refs;
pub mod set;
pub mod successor;
pub mod takes_effect;

pub use plan::{Change, Plan};
