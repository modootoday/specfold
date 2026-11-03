//! X1, X2 and G1: the commands a document names actually pass, and an active source of truth
//! says how it is verified. X1 and G1 only run when asked (`spec verify`,
//! `spec check --run-commands`), because they execute project code.

mod declared;
mod generated;
mod job;
mod passes;
mod select;

pub use declared::VerifyDeclared;
pub use generated::GeneratedInSync;
pub use passes::VerifyPasses;
