//! The command line. This definition is also the source of `spec help --json` and of the MCP
//! tool list, so the three can never disagree.

mod command;
pub mod global;
pub mod read;
pub mod setup;
pub mod verify;
pub mod write;

pub use command::{Cli, Cmd};
